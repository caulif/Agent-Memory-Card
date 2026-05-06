use std::io::{self, BufRead, Write};
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use serde_json::{Value, json};

use crate::{build, draft, fsutil};

const MCP_PROTOCOL_VERSION: &str = "2025-06-18";

pub fn serve_stdio(project_root: &Path) -> Result<()> {
    let root = fsutil::normalize_project_root(project_root)?;
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = stdin.lock();
    let mut writer = stdout.lock();

    while let Some(message) = read_message(&mut reader)? {
        if let Some(response) = handle_message(&root, &message)? {
            write_message(&mut writer, &response)?;
        }
    }

    Ok(())
}

fn read_message<R: BufRead>(reader: &mut R) -> Result<Option<Value>> {
    let mut content_length = None::<usize>;
    loop {
        let mut line = String::new();
        let bytes = reader.read_line(&mut line)?;
        if bytes == 0 {
            return Ok(None);
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        let (name, value) = trimmed
            .split_once(':')
            .ok_or_else(|| anyhow!("invalid MCP header line `{trimmed}`"))?;
        if name.eq_ignore_ascii_case("content-length") {
            content_length = Some(
                value
                    .trim()
                    .parse::<usize>()
                    .context("parse Content-Length header")?,
            );
        }
    }

    let length = content_length.ok_or_else(|| anyhow!("missing Content-Length header"))?;
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body)?;
    Ok(Some(serde_json::from_slice(&body)?))
}

fn write_message<W: Write>(writer: &mut W, response: &Value) -> Result<()> {
    let body = serde_json::to_vec(response)?;
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(&body)?;
    writer.flush()?;
    Ok(())
}

fn handle_message(project_root: &Path, request: &Value) -> Result<Option<Value>> {
    let Some(method) = request.get("method").and_then(Value::as_str) else {
        return Ok(None);
    };
    let id = request.get("id").cloned();
    let params = request.get("params").cloned().unwrap_or_else(|| json!({}));

    let result = match method {
        "initialize" => Some(json!({
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "agent-kernel",
                "version": env!("CARGO_PKG_VERSION")
            }
        })),
        "notifications/initialized" => None,
        "ping" => Some(json!({})),
        "tools/list" => Some(json!({
            "tools": [
                {
                    "name": "list_drafts",
                    "description": "List the current project's draft inbox items.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "approve_draft",
                    "description": "Approve one draft inbox item into a skilllet.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "id": {
                                "type": "string",
                                "description": "Draft id to approve."
                            }
                        },
                        "required": ["id"]
                    }
                },
                {
                    "name": "build_artifacts",
                    "description": "Build or preview Agent-Kernel artifacts for the current project.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "preview": {
                                "type": "boolean",
                                "description": "When true, return build preview without writing files."
                            }
                        }
                    }
                }
            ]
        })),
        "tools/call" => Some(match handle_tool_call(project_root, &params) {
            Ok(result) => result,
            Err(error) => tool_error(error.to_string()),
        }),
        _ => None,
    };

    if let Some(result) = result {
        Ok(Some(jsonrpc_success(id, result)))
    } else if id.is_some() {
        Ok(Some(jsonrpc_error(
            id,
            -32601,
            format!("method `{method}` is not supported"),
        )))
    } else {
        Ok(None)
    }
}

fn handle_tool_call(project_root: &Path, params: &Value) -> Result<Value> {
    let tool_name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("tools/call is missing `name`"))?;
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| Value::Object(Default::default()));

    match tool_name {
        "list_drafts" => {
            let drafts = draft::load_drafts(project_root)?;
            Ok(tool_result(
                format!("Loaded {} draft(s).", drafts.len()),
                json!({ "drafts": drafts }),
            ))
        }
        "approve_draft" => {
            let id = arguments
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("approve_draft requires string argument `id`"))?;
            draft::approve_draft(project_root, id)?;
            Ok(tool_result(
                format!("Approved draft `{id}`."),
                json!({ "approved": id }),
            ))
        }
        "build_artifacts" => {
            let preview = arguments
                .get("preview")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let report = build::build_project(project_root, preview)?;
            Ok(tool_result(
                report.render(),
                json!({
                    "preview": preview,
                    "report": report.render()
                }),
            ))
        }
        _ => Err(anyhow!("unknown tool `{tool_name}`")),
    }
}

fn tool_result(text: String, structured: Value) -> Value {
    json!({
        "content": [{
            "type": "text",
            "text": text
        }],
        "structuredContent": structured
    })
}

fn tool_error(message: String) -> Value {
    json!({
        "content": [{
            "type": "text",
            "text": message
        }],
        "isError": true
    })
}

fn jsonrpc_success(id: Option<Value>, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id.unwrap_or(Value::Null),
        "result": result
    })
}

fn jsonrpc_error(id: Option<Value>, code: i64, message: String) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id.unwrap_or(Value::Null),
        "error": {
            "code": code,
            "message": message
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draft::NewDraft;

    #[test]
    fn initialize_returns_server_info() {
        let temp = tempfile::tempdir().expect("tempdir");
        let response = handle_message(
            temp.path(),
            &json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {}
            }),
        )
        .expect("initialize")
        .expect("response");

        assert_eq!(response["result"]["serverInfo"]["name"], "agent-kernel");
        assert_eq!(
            response["result"]["protocolVersion"],
            Value::String(MCP_PROTOCOL_VERSION.to_string())
        );
    }

    #[test]
    fn tools_list_exposes_minimal_toolset() {
        let temp = tempfile::tempdir().expect("tempdir");
        let response = handle_message(
            temp.path(),
            &json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/list"
            }),
        )
        .expect("tools/list")
        .expect("response");

        let tools = response["result"]["tools"].as_array().expect("tools");
        let names = tools
            .iter()
            .filter_map(|tool| tool.get("name").and_then(Value::as_str))
            .collect::<Vec<_>>();
        assert!(names.contains(&"list_drafts"));
        assert!(names.contains(&"approve_draft"));
        assert!(names.contains(&"build_artifacts"));
    }

    #[test]
    fn approve_draft_tool_promotes_draft() {
        let temp = tempfile::tempdir().expect("tempdir");
        crate::draft::add_draft(
            temp.path(),
            NewDraft {
                id: "project:test".to_string(),
                title: "Test".to_string(),
                kind: "preference".to_string(),
                scope: "project".to_string(),
                body: "Use Bun.".to_string(),
                targets: vec!["codex".to_string()],
                evidence: "manual".to_string(),
                confidence: None,
                reason: None,
                matched_template: None,
                extraction: crate::candidate::ExtractionMetadata::default(),
            },
        )
        .expect("add draft");

        let response = handle_message(
            temp.path(),
            &json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/call",
                "params": {
                    "name": "approve_draft",
                    "arguments": {
                        "id": "project:test"
                    }
                }
            }),
        )
        .expect("approve")
        .expect("response");

        assert_eq!(
            response["result"]["structuredContent"]["approved"],
            "project:test"
        );
        assert!(
            temp.path()
                .join(".agent-kernel")
                .join("skilllets")
                .join("project")
                .join("test.yml")
                .exists()
        );
    }

    #[test]
    fn content_length_round_trip_works() {
        let message = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "ping"
        });
        let mut buffer = Vec::new();
        write_message(&mut buffer, &message).expect("write");
        let mut reader = io::Cursor::new(buffer);
        let decoded = read_message(&mut reader).expect("read").expect("message");
        assert_eq!(decoded, message);
    }
}
