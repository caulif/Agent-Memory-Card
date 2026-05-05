use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use anyhow::Context;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::fsutil;

use super::{AutomationMode, KernelDecision, KernelDisposition, KernelRisk};

const AUDIT_LOG: &str = ".agent-kernel/audit-log.jsonl";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum KernelAuditStatus {
    Authorized,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KernelAuditEntry {
    pub timestamp: DateTime<Utc>,
    pub actor: String,
    pub command: String,
    pub risk: KernelRisk,
    pub disposition: KernelDisposition,
    pub mode: AutomationMode,
    pub status: KernelAuditStatus,
    pub reason: String,
}

impl KernelAuditEntry {
    pub fn from_decision(
        actor: impl Into<String>,
        decision: &KernelDecision,
        mode: AutomationMode,
        status: KernelAuditStatus,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            actor: actor.into(),
            command: decision.command.clone(),
            risk: decision.risk,
            disposition: decision.disposition,
            mode,
            status,
            reason: decision.reason.clone(),
        }
    }
}

pub fn append_audit_entry(project_root: &Path, entry: &KernelAuditEntry) -> anyhow::Result<()> {
    let root = fsutil::normalize_project_root(project_root)?;
    let path = root.join(AUDIT_LOG);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("open {}", path.display()))?;
    serde_json::to_writer(&mut file, entry).context("serialize audit entry")?;
    file.write_all(b"\n").context("write audit newline")?;
    Ok(())
}

pub fn load_audit_entries(project_root: &Path) -> anyhow::Result<Vec<KernelAuditEntry>> {
    let root = fsutil::normalize_project_root(project_root)?;
    let path = root.join(AUDIT_LOG);
    if !path.exists() {
        return Ok(Vec::new());
    }

    let file = fs::File::open(&path).with_context(|| format!("open {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut entries = Vec::new();
    for line in reader.lines() {
        let line = line.context("read audit line")?;
        if line.trim().is_empty() {
            continue;
        }
        entries.push(serde_json::from_str(&line).context("parse audit entry")?);
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_entries_append_and_load_as_jsonl() {
        let temp = tempfile::tempdir().expect("tempdir");
        let decision = KernelDecision {
            command: "approve-draft".to_string(),
            disposition: KernelDisposition::ReviewRequired,
            risk: KernelRisk::High,
            requires_human_review: true,
            reason: "Manual mode keeps mutating kernel commands in human review.".to_string(),
            decision_token: None,
        };
        let entry = KernelAuditEntry::from_decision(
            "tauri",
            &decision,
            AutomationMode::Manual,
            KernelAuditStatus::Blocked,
        );

        append_audit_entry(temp.path(), &entry).expect("append");
        let entries = load_audit_entries(temp.path()).expect("load");

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].command, "approve-draft");
        assert_eq!(entries[0].status, KernelAuditStatus::Blocked);
    }
}
