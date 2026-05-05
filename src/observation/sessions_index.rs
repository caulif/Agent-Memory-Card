use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SessionsIndexEntry {
    pub session_id: String,
    pub project_path: PathBuf,
    pub jsonl_path: PathBuf,
}

pub(super) fn session_project_path(jsonl: &Path) -> Result<Option<PathBuf>> {
    let Some(parent) = jsonl.parent() else {
        return Ok(None);
    };
    let Some(session_id) = jsonl.file_stem().and_then(|value| value.to_str()) else {
        return Ok(None);
    };
    let index_path = parent.join("sessions-index.json");
    if !index_path.exists() {
        return Ok(None);
    }

    let text = fs::read_to_string(&index_path)
        .with_context(|| format!("read {}", index_path.display()))?;
    let value = serde_json::from_str::<Value>(&text)
        .with_context(|| format!("parse {}", index_path.display()))?;
    if session_id_exists(&value, session_id) {
        return Ok(decode_project_path_from_dir(parent));
    }
    Ok(None)
}

pub(super) fn load_sessions_index(projects_dir: &Path) -> Result<Vec<SessionsIndexEntry>> {
    let mut entries = Vec::new();
    if !projects_dir.exists() {
        return Ok(entries);
    }
    for entry in fs::read_dir(projects_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let project_dir = entry.path();
        let Some(project_path) = decode_project_path_from_dir(&project_dir) else {
            continue;
        };
        let index_path = project_dir.join("sessions-index.json");
        if !index_path.exists() {
            continue;
        }
        let text = fs::read_to_string(&index_path)
            .with_context(|| format!("read {}", index_path.display()))?;
        let value = serde_json::from_str::<Value>(&text)
            .with_context(|| format!("parse {}", index_path.display()))?;
        for session_id in session_ids(&value) {
            let jsonl_path = project_dir.join(format!("{session_id}.jsonl"));
            if jsonl_path.exists() {
                entries.push(SessionsIndexEntry {
                    session_id,
                    project_path: project_path.clone(),
                    jsonl_path,
                });
            }
        }
    }
    Ok(entries)
}

pub(super) fn decode_project_path_from_dir(project_dir: &Path) -> Option<PathBuf> {
    let name = project_dir.file_name()?.to_str()?;
    decode_project_path_from_dirname(name)
}

pub(super) fn decode_project_path_from_dirname(dirname: &str) -> Option<PathBuf> {
    let trimmed = dirname.trim_start_matches('-');
    if trimmed.is_empty() {
        return None;
    }
    let parts = trimmed
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return None;
    }

    #[cfg(windows)]
    {
        if parts.first().is_some_and(|part| part.len() == 1) && parts.get(1).is_some() {
            let mut path = PathBuf::from(format!("{}:\\", parts[0]));
            for part in parts.iter().skip(1) {
                path.push(part);
            }
            return Some(path);
        }
    }

    let mut path = PathBuf::new();
    for part in parts {
        path.push(part);
    }
    if dirname.starts_with('-') {
        let mut absolute = PathBuf::from(std::path::MAIN_SEPARATOR.to_string());
        absolute.push(path);
        Some(absolute)
    } else {
        Some(path)
    }
}

fn session_id_exists(value: &Value, session_id: &str) -> bool {
    session_ids(value).iter().any(|id| id == session_id)
}

fn session_ids(value: &Value) -> Vec<String> {
    match value {
        Value::Object(map) => {
            if let Some(Value::Object(sessions)) = map.get("sessions") {
                return sessions.keys().cloned().collect();
            }
            if map.values().all(Value::is_object) {
                return map.keys().cloned().collect();
            }
            Vec::new()
        }
        Value::Array(items) => items
            .iter()
            .filter_map(|item| {
                item.get("sessionId")
                    .or_else(|| item.get("session_id"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .collect(),
        _ => Vec::new(),
    }
}
