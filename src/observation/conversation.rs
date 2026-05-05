use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::Value;

use crate::fsutil;

use super::ConversationFile;
pub(super) fn collect_jsonl(
    dir: &Path,
    agent: &str,
    source_kind: &str,
    files: &mut Vec<ConversationFile>,
) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in walkdir::WalkDir::new(dir).follow_links(false) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.path().extension().and_then(|value| value.to_str()) == Some("jsonl") {
            let path = entry.path().to_path_buf();
            files.push(ConversationFile {
                agent: agent.to_string(),
                source_kind: source_kind.to_string(),
                project_path: project_path_from_jsonl(&path).ok().flatten(),
                path,
            });
        }
    }
    Ok(())
}

pub(super) fn collect_single_jsonl(
    file: &Path,
    agent: &str,
    source_kind: &str,
    files: &mut Vec<ConversationFile>,
) {
    if file.is_file() {
        files.push(ConversationFile {
            agent: agent.to_string(),
            source_kind: source_kind.to_string(),
            project_path: project_path_from_jsonl(file).ok().flatten(),
            path: file.to_path_buf(),
        });
    }
}

pub(super) fn conversation_belongs_to_project(
    file: &ConversationFile,
    project_root: &Path,
) -> bool {
    let Some(project_path) = file.project_path.as_deref() else {
        return true;
    };
    let Ok(source_root) = fsutil::normalize_project_root(project_path) else {
        return false;
    };
    let Ok(target_root) = fsutil::normalize_project_root(project_root) else {
        return false;
    };
    source_root == target_root
}

pub(super) fn project_path_from_jsonl(file: &Path) -> Result<Option<PathBuf>> {
    let handle = fs::File::open(file).with_context(|| format!("open {}", file.display()))?;
    let reader = BufReader::new(handle);
    let mut scanned_bytes = 0usize;
    for line in reader.lines().take(200) {
        let line = line.with_context(|| format!("read {}", file.display()))?;
        scanned_bytes += line.len();
        if scanned_bytes > 256 * 1024 {
            break;
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if let Some(path) = find_string_key(&value, &["cwd", "project"]) {
            return Ok(Some(PathBuf::from(path)));
        }
    }
    Ok(None)
}

pub(super) fn find_string_key(value: &Value, keys: &[&str]) -> Option<String> {
    match value {
        Value::Object(map) => {
            for key in keys {
                if let Some(Value::String(path)) = map.get(*key) {
                    return Some(path.clone());
                }
            }
            map.values().find_map(|child| find_string_key(child, keys))
        }
        Value::Array(items) => items.iter().find_map(|child| find_string_key(child, keys)),
        _ => None,
    }
}

pub(super) fn normalize_observation_body(input: &str, file: &Path, source_kind: &str) -> String {
    let is_jsonl = file.extension().and_then(|value| value.to_str()) == Some("jsonl")
        || source_kind.contains("session");
    if !is_jsonl {
        return input.trim().to_string();
    }
    extract_jsonl_messages(input).unwrap_or_else(|| input.trim().to_string())
}

pub(super) fn extract_jsonl_messages(input: &str) -> Option<String> {
    let mut parts = Vec::new();
    for line in input.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if !is_relevant_conversation_line(&value) {
            continue;
        }
        collect_known_text_fields(&value, &mut parts);
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n").trim().to_string())
    }
}

pub(super) fn is_relevant_conversation_line(value: &Value) -> bool {
    if value
        .get("isMeta")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return false;
    }
    let raw = value.to_string().to_lowercase();
    if raw.contains("base_instructions")
        || raw.contains("<local-command")
        || raw.contains("<command-name>")
        || raw.contains("token_count")
    {
        return false;
    }
    if value
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|kind| matches!(kind, "session_meta" | "queue-operation" | "turn_context"))
    {
        return false;
    }
    contains_role(value, "user")
        || contains_type(value, "user")
        || contains_type(value, "user_message")
}

pub(super) fn contains_role(value: &Value, role: &str) -> bool {
    match value {
        Value::Object(map) => {
            if map.get("role").and_then(Value::as_str) == Some(role) {
                return true;
            }
            map.values().any(|child| contains_role(child, role))
        }
        Value::Array(items) => items.iter().any(|child| contains_role(child, role)),
        _ => false,
    }
}

pub(super) fn contains_type(value: &Value, kind: &str) -> bool {
    match value {
        Value::Object(map) => {
            if map.get("type").and_then(Value::as_str) == Some(kind) {
                return true;
            }
            map.values().any(|child| contains_type(child, kind))
        }
        Value::Array(items) => items.iter().any(|child| contains_type(child, kind)),
        _ => false,
    }
}

pub(super) fn collect_known_text_fields(value: &Value, parts: &mut Vec<String>) {
    match value {
        Value::String(text) if !text.trim().is_empty() => {
            parts.push(text.trim().to_string());
        }
        Value::Array(items) => {
            for item in items {
                collect_known_text_fields(item, parts);
            }
        }
        Value::Object(map) => {
            for key in ["message", "content", "text"] {
                if let Some(child) = map.get(key) {
                    collect_known_text_fields(child, parts);
                }
            }
        }
        _ => {}
    }
}
