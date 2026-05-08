use std::io::{self, BufRead, Write};
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use serde_json::{Value, json};

use crate::{build, draft, fsutil, kernel, memory_card};

const MCP_PROTOCOL_VERSION: &str = "2025-06-18";

/// 消息体上限（16 MB），防止恶意或损坏的 Content-Length 导致 OOM。
const MAX_CONTENT_LENGTH: usize = 16 * 1024 * 1024;

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
    if length > MAX_CONTENT_LENGTH {
        return Err(anyhow!(
            "Content-Length {length} exceeds maximum {MAX_CONTENT_LENGTH}"
        ));
    }
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
                    "inputSchema": { "type": "object", "properties": {} }
                },
                {
                    "name": "list_memory_cards",
                    "description": "List the current project's Memory Cards.",
                    "inputSchema": { "type": "object", "properties": {} }
                },
                {
                    "name": "approve_draft",
                    "description": "Approve one draft inbox item into a Memory Card.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "string", "description": "Draft id to approve." }
                        },
                        "required": ["id"]
                    }
                },
                {
                    "name": "update_memory_card",
                    "description": "Update editable fields on one Memory Card.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "string" },
                            "title": { "type": "string" },
                            "body": { "type": "string" },
                            "brief": { "type": "string" },
                            "tags": { "type": "array", "items": { "type": "string" } },
                            "language": { "type": "string" },
                            "kind": { "type": "string" },
                            "scope": { "type": "string" }
                        },
                        "required": ["id"]
                    }
                },
                {
                    "name": "assign_memory_card",
                    "description": "Assign one Memory Card to explicit Agent targets.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "string" },
                            "targets": { "type": "array", "items": { "type": "string" } }
                        },
                        "required": ["id", "targets"]
                    }
                },
                {
                    "name": "merge_memory_cards",
                    "description": "Merge multiple source Memory Cards into a new Memory Card.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "string" },
                            "title": { "type": "string" },
                            "sources": { "type": "array", "items": { "type": "string" } },
                            "targets": { "type": "array", "items": { "type": "string" } }
                        },
                        "required": ["id", "title", "sources"]
                    }
                },
                {
                    "name": "build_artifacts",
                    "description": "Build or preview Agent Memory Kernel artifacts for the current project.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "preview": {
                                "type": "boolean",
                                "description": "When true, return build preview without writing files."
                            }
                        }
                    }
                },
                {
                    "name": "sync_project",
                    "description": "Compile Agent Memory Kernel artifacts for the current project.",
                    "inputSchema": { "type": "object", "properties": {} }
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
            audit_mcp_mutation(
                project_root,
                kernel::KernelCommand::ApproveDraft { id: id.to_string() },
            )?;
            draft::approve_draft(project_root, id)?;
            Ok(tool_result(
                format!("Approved draft `{id}` into a Memory Card."),
                json!({ "approved": id }),
            ))
        }
        "list_memory_cards" => {
            let memory_cards = memory_card::load_memory_cards(project_root)?;
            Ok(tool_result(
                format!("Loaded {} Memory Card(s).", memory_cards.len()),
                json!({ "memory_cards": memory_cards }),
            ))
        }
        "update_memory_card" => {
            let id = required_str(&arguments, "id", "update_memory_card")?;
            let update = memory_card::MemoryCardUpdate {
                title: optional_str(&arguments, "title"),
                body: optional_str(&arguments, "body"),
                brief: optional_str(&arguments, "brief"),
                tags: optional_string_array(&arguments, "tags")?,
                language: optional_str(&arguments, "language"),
                kind: optional_str(&arguments, "kind"),
                scope: optional_str(&arguments, "scope"),
            };
            audit_mcp_mutation(
                project_root,
                kernel::KernelCommand::UpdateMemoryCard { id: id.to_string() },
            )?;
            let updated = memory_card::update_memory_card(project_root, id, update)?;
            Ok(tool_result(
                format!("Updated Memory Card `{id}`."),
                json!({ "memory_card": updated }),
            ))
        }
        "assign_memory_card" => {
            let id = required_str(&arguments, "id", "assign_memory_card")?;
            let targets = required_string_array(&arguments, "targets", "assign_memory_card")?;
            audit_mcp_mutation(
                project_root,
                kernel::KernelCommand::AssignMemoryCard {
                    id: id.to_string(),
                    targets: targets.clone(),
                },
            )?;
            memory_card::set_memory_card_targets(project_root, id, targets)?;
            Ok(tool_result(
                format!("Assigned Memory Card `{id}`."),
                json!({ "assigned": id }),
            ))
        }
        "merge_memory_cards" => {
            let id = required_str(&arguments, "id", "merge_memory_cards")?;
            let title = required_str(&arguments, "title", "merge_memory_cards")?;
            let sources = required_string_array(&arguments, "sources", "merge_memory_cards")?;
            let targets = optional_string_array(&arguments, "targets")?.unwrap_or_default();
            audit_mcp_mutation(
                project_root,
                kernel::KernelCommand::MergeMemoryCards {
                    ids: sources.clone(),
                },
            )?;
            memory_card::merge_memory_cards(project_root, id, title, sources, targets)?;
            Ok(tool_result(
                format!("Merged Memory Card `{id}`."),
                json!({ "merged": id }),
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
        "sync_project" => {
            audit_mcp_mutation(
                project_root,
                kernel::KernelCommand::CompileProject { dry_run: false },
            )?;
            let report = build::sync_project(project_root)?;
            Ok(tool_result(
                report.render(),
                json!({ "report": report.render() }),
            ))
        }
        _ => Err(anyhow!("unknown tool `{tool_name}`")),
    }
}

fn audit_mcp_mutation(project_root: &Path, command: kernel::KernelCommand) -> Result<()> {
    let policy = kernel::KernelPolicy::agent_managed();
    let decision = kernel::enforce_command(&command, &policy)?;
    let entry = kernel::KernelAuditEntry::from_decision(
        "mcp",
        &decision,
        policy.mode,
        kernel::KernelAuditStatus::Authorized,
    );
    kernel::append_audit_entry(project_root, &entry)?;
    Ok(())
}

fn required_str<'a>(arguments: &'a Value, key: &str, tool: &str) -> Result<&'a str> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("{tool} requires string argument `{key}`"))
}

fn optional_str(arguments: &Value, key: &str) -> Option<String> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn required_string_array(arguments: &Value, key: &str, tool: &str) -> Result<Vec<String>> {
    optional_string_array(arguments, key)?
        .ok_or_else(|| anyhow!("{tool} requires array argument `{key}`"))
}

fn optional_string_array(arguments: &Value, key: &str) -> Result<Option<Vec<String>>> {
    let Some(value) = arguments.get(key) else {
        return Ok(None);
    };
    let Some(array) = value.as_array() else {
        return Err(anyhow!("argument `{key}` must be an array"));
    };
    let mut out = Vec::new();
    for item in array {
        let Some(text) = item.as_str() else {
            return Err(anyhow!("argument `{key}` must contain only strings"));
        };
        out.push(text.to_string());
    }
    Ok(Some(out))
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
        assert!(names.contains(&"list_memory_cards"));
        assert!(names.contains(&"approve_draft"));
        assert!(names.contains(&"assign_memory_card"));
        assert!(names.contains(&"sync_project"));
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
                .join("memory-cards")
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

    #[test]
    fn read_message_rejects_too_large_content_length() {
        let header = format!("Content-Length: {}\r\n\r\n", MAX_CONTENT_LENGTH + 1);
        let mut reader = io::Cursor::new(header.as_bytes());
        let result = read_message(&mut reader);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("exceeds maximum"));
    }

    #[test]
    fn read_message_rejects_malformed_content_length() {
        let header = "Content-Length: not-a-number\r\n\r\n";
        let mut reader = io::Cursor::new(header.as_bytes());
        let result = read_message(&mut reader);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("parse Content-Length")
        );
    }

    #[test]
    fn read_message_accepts_content_length_within_limit() {
        let body = serde_json::to_vec(&json!({"method": "ping"})).expect("body");
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        let mut raw = header.into_bytes();
        raw.extend_from_slice(&body);
        let mut reader = io::Cursor::new(raw);
        let result = read_message(&mut reader).expect("read").expect("message");
        assert_eq!(result["method"], "ping");
    }

    #[test]
    fn read_message_rejects_missing_content_length() {
        let header = "Accept: application/json\r\n\r\n{}";
        let mut reader = io::Cursor::new(header.as_bytes());
        let result = read_message(&mut reader);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("missing Content-Length")
        );
    }
}
