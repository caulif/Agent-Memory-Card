use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use anyhow::{Context, anyhow};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::fsutil;

use super::{KernelCommand, KernelPolicy};

const CONFIRMATION_LOG: &str = ".agent-kernel/decision-tokens.jsonl";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KernelDecisionTokenRecord {
    pub token: String,
    pub created_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consumed_at: Option<DateTime<Utc>>,
    pub project_path: String,
    pub command_hash: String,
    pub policy_hash: String,
}

pub fn command_payload_hash(command: &KernelCommand, payload: &Value) -> anyhow::Result<String> {
    let canonical = serde_json::to_string(&serde_json::json!({
        "command": command,
        "payload": payload,
    }))?;
    Ok(fsutil::sha256_text(&canonical))
}

pub fn create_decision_token(
    project_root: &Path,
    command: &KernelCommand,
    payload: &Value,
    policy: &KernelPolicy,
) -> anyhow::Result<String> {
    let root = fsutil::normalize_project_root(project_root)?;
    let project_path = fsutil::path_to_slash(&root);
    let command_hash = command_payload_hash(command, payload)?;
    let policy_hash = fsutil::sha256_text(&serde_json::to_string(policy)?);
    let created_at = Utc::now();
    let token = fsutil::sha256_text(&format!(
        "{project_path}|{command_hash}|{policy_hash}|{}",
        created_at.to_rfc3339()
    ));
    let record = KernelDecisionTokenRecord {
        token: token.clone(),
        created_at,
        consumed_at: None,
        project_path,
        command_hash,
        policy_hash,
    };
    append_decision_token(&root, &record)?;
    Ok(token)
}

pub fn verify_decision_token(
    project_root: &Path,
    token: &str,
    command: &KernelCommand,
    payload: &Value,
) -> anyhow::Result<()> {
    let root = fsutil::normalize_project_root(project_root)?;
    let project_path = fsutil::path_to_slash(&root);
    let expected_command_hash = command_payload_hash(command, payload)?;
    let record = load_decision_tokens(&root)?
        .into_iter()
        .find(|record| record.token == token)
        .ok_or_else(|| anyhow!("decision token was not found for this project"))?;

    if record.project_path != project_path {
        return Err(anyhow!("decision token belongs to a different project"));
    }
    if record.command_hash != expected_command_hash {
        return Err(anyhow!(
            "decision token payload does not match this command"
        ));
    }

    Ok(())
}

pub fn consume_decision_token(
    project_root: &Path,
    token: &str,
    command: &KernelCommand,
    payload: &Value,
) -> anyhow::Result<()> {
    let root = fsutil::normalize_project_root(project_root)?;
    verify_decision_token(&root, token, command, payload)?;

    let mut records = load_decision_tokens(&root)?;
    let record = records
        .iter_mut()
        .find(|record| record.token == token)
        .ok_or_else(|| anyhow!("decision token was not found for this project"))?;
    if record.consumed_at.is_some() {
        return Err(anyhow!("decision token has already been used"));
    }
    record.consumed_at = Some(Utc::now());
    save_decision_tokens(&root, &records)
}

pub fn load_decision_tokens(project_root: &Path) -> anyhow::Result<Vec<KernelDecisionTokenRecord>> {
    let root = fsutil::normalize_project_root(project_root)?;
    let path = root.join(CONFIRMATION_LOG);
    if !path.exists() {
        return Ok(Vec::new());
    }

    let file = fs::File::open(&path).with_context(|| format!("open {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut records = Vec::new();
    for line in reader.lines() {
        let line = line.context("read decision token line")?;
        if line.trim().is_empty() {
            continue;
        }
        records.push(serde_json::from_str(&line).context("parse decision token")?);
    }
    Ok(records)
}

fn save_decision_tokens(
    project_root: &Path,
    records: &[KernelDecisionTokenRecord],
) -> anyhow::Result<()> {
    let path = project_root.join(CONFIRMATION_LOG);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let mut file = fs::File::create(&path).with_context(|| format!("open {}", path.display()))?;
    for record in records {
        serde_json::to_writer(&mut file, record).context("serialize decision token")?;
        file.write_all(b"\n")
            .context("write decision token newline")?;
    }
    Ok(())
}

fn append_decision_token(
    project_root: &Path,
    record: &KernelDecisionTokenRecord,
) -> anyhow::Result<()> {
    let path = project_root.join(CONFIRMATION_LOG);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("open {}", path.display()))?;
    serde_json::to_writer(&mut file, record).context("serialize decision token")?;
    file.write_all(b"\n")
        .context("write decision token newline")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decision_token_verifies_matching_command_payload() {
        let temp = tempfile::tempdir().expect("tempdir");
        let command = KernelCommand::ApproveDraft {
            id: "project:prefer-bun".to_string(),
        };
        let payload = serde_json::json!({"id": "project:prefer-bun"});
        let token = create_decision_token(temp.path(), &command, &payload, &KernelPolicy::manual())
            .expect("create token");

        verify_decision_token(temp.path(), &token, &command, &payload).expect("verify token");
    }

    #[test]
    fn decision_token_rejects_mismatched_payload() {
        let temp = tempfile::tempdir().expect("tempdir");
        let command = KernelCommand::ApproveDraft {
            id: "project:prefer-bun".to_string(),
        };
        let token = create_decision_token(
            temp.path(),
            &command,
            &serde_json::json!({"id": "project:prefer-bun"}),
            &KernelPolicy::manual(),
        )
        .expect("create token");

        let err = verify_decision_token(
            temp.path(),
            &token,
            &command,
            &serde_json::json!({"id": "project:use-axios"}),
        )
        .expect_err("payload mismatch should fail");

        assert!(err.to_string().contains("payload"));
    }

    #[test]
    fn decision_token_is_consumed_once() {
        let temp = tempfile::tempdir().expect("tempdir");
        let command = KernelCommand::ApproveDraft {
            id: "project:prefer-bun".to_string(),
        };
        let payload = serde_json::json!({"id": "project:prefer-bun"});
        let token = create_decision_token(temp.path(), &command, &payload, &KernelPolicy::manual())
            .expect("create token");

        consume_decision_token(temp.path(), &token, &command, &payload).expect("consume once");
        let err = consume_decision_token(temp.path(), &token, &command, &payload)
            .expect_err("token replay should fail");

        assert!(err.to_string().contains("already been used"));
    }
}
