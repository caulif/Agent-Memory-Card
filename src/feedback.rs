use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::config;
use crate::fsutil;
use crate::textutil;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackEvent {
    pub event_id: String,
    pub item_kind: String,
    pub item_id: String,
    pub decision: String,
    pub body: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub signature: String,
    pub created_at: String,
}

pub fn record_feedback(
    project_root: &Path,
    item_kind: &str,
    item_id: &str,
    decision: &str,
    body: &str,
    reason: Option<String>,
) -> Result<()> {
    config::ensure_kernel_dir(project_root)?;
    let created_at = Utc::now().to_rfc3339();
    let signature = feedback_signature(item_kind, body);
    let event = FeedbackEvent {
        event_id: format!(
            "fb:{}",
            textutil::slug(
                &fsutil::sha256_text(&format!("{item_kind}:{item_id}:{decision}:{created_at}"))
                    [..16]
            )
        ),
        item_kind: item_kind.to_string(),
        item_id: item_id.to_string(),
        decision: decision.to_string(),
        body: body.to_string(),
        reason,
        signature,
        created_at,
    };
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(feedback_path(project_root))
        .context("open feedback log")?;
    writeln!(file, "{}", serde_json::to_string(&event)?).context("write feedback log")?;
    Ok(())
}

pub fn load_feedback(project_root: &Path) -> Result<Vec<FeedbackEvent>> {
    let path = feedback_path(project_root);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).context("parse feedback event"))
        .collect()
}

fn feedback_path(project_root: &Path) -> std::path::PathBuf {
    config::kernel_dir(project_root).join("feedback.jsonl")
}

fn feedback_signature(item_kind: &str, body: &str) -> String {
    let normalized = textutil::tokenize(&body.to_lowercase())
        .into_iter()
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "sig:{}",
        textutil::slug(&fsutil::sha256_text(&format!("{item_kind}:{normalized}"))[..16])
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_feedback_as_jsonl() {
        let temp = tempfile::tempdir().expect("tempdir");

        record_feedback(
            temp.path(),
            "candidate",
            "project:prefer-bun",
            "approved",
            "Use Bun for JavaScript package management.",
            Some("good candidate".to_string()),
        )
        .expect("record feedback");

        let events = load_feedback(temp.path()).expect("load feedback");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].decision, "approved");
        assert_eq!(events[0].item_id, "project:prefer-bun");
        assert!(events[0].signature.starts_with("sig:"));
    }
}
