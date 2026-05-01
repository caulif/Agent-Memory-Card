use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::config;
use crate::fsutil;
use crate::provider;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationRecord {
    pub id: String,
    pub source_kind: String,
    pub source_path: String,
    pub agent: Option<String>,
    pub body: String,
    pub evidence: String,
    pub redacted: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ObservationImportReport {
    pub created: usize,
    pub skipped: usize,
    pub observations: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ConversationFile {
    pub agent: String,
    pub source_kind: String,
    pub path: PathBuf,
}

impl ObservationImportReport {
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("Agent-Kernel observation import\n\n");
        out.push_str(&format!("Created observations: {}\n", self.created));
        out.push_str(&format!("Skipped sources: {}\n", self.skipped));
        if !self.observations.is_empty() {
            out.push_str("\nObservations:\n");
            for id in &self.observations {
                out.push_str(&format!("- {id}\n"));
            }
        }
        out
    }
}

pub fn import_observation_file(
    project_root: &Path,
    file: &Path,
    source_kind: &str,
    agent: Option<&str>,
) -> Result<ObservationImportReport> {
    let root = fsutil::normalize_project_root(project_root)?;
    let text = fs::read_to_string(file).with_context(|| format!("read {}", file.display()))?;
    if text.trim().is_empty() {
        return Ok(ObservationImportReport {
            created: 0,
            skipped: 1,
            observations: Vec::new(),
        });
    }

    let redacted = provider::redact_secrets(&text);
    let id = observation_id(agent, source_kind, &file.display().to_string(), &redacted);
    let record = ObservationRecord {
        id: id.clone(),
        source_kind: source_kind.to_string(),
        source_path: fsutil::path_to_slash(file),
        agent: agent.map(str::to_string),
        body: redacted.clone(),
        evidence: format!("Imported from {}", fsutil::path_to_slash(file)),
        redacted: redacted != text,
        created_at: Utc::now().to_rfc3339(),
    };
    write_observation(&root, &record)?;

    Ok(ObservationImportReport {
        created: 1,
        skipped: 0,
        observations: vec![id],
    })
}

pub fn import_local_conversations(
    project_root: &Path,
    home: &Path,
) -> Result<ObservationImportReport> {
    let mut report = ObservationImportReport {
        created: 0,
        skipped: 0,
        observations: Vec::new(),
    };
    for file in discover_local_conversation_files(home)? {
        let imported = import_observation_file(
            project_root,
            &file.path,
            &file.source_kind,
            Some(&file.agent),
        )?;
        report.created += imported.created;
        report.skipped += imported.skipped;
        report.observations.extend(imported.observations);
    }
    Ok(report)
}

pub fn load_observations(project_root: &Path) -> Result<Vec<ObservationRecord>> {
    let root = fsutil::normalize_project_root(project_root)?;
    let dir = observations_dir(&root);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut records = Vec::new();
    for entry in walkdir::WalkDir::new(dir).follow_links(false) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.path().extension().and_then(|value| value.to_str()) != Some("yml") {
            continue;
        }
        let text = fs::read_to_string(entry.path())?;
        records.push(serde_yaml::from_str::<ObservationRecord>(&text)?);
    }
    records.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(records)
}

pub fn discover_local_conversation_files(home: &Path) -> Result<Vec<ConversationFile>> {
    let mut files = Vec::new();
    collect_jsonl(
        &home.join(".claude").join("projects"),
        "claude-code",
        "claude-code-session",
        &mut files,
    )?;
    collect_jsonl(
        &home.join(".codex").join("sessions"),
        "codex",
        "codex-session",
        &mut files,
    )?;
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

fn collect_jsonl(
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
            files.push(ConversationFile {
                agent: agent.to_string(),
                source_kind: source_kind.to_string(),
                path: entry.path().to_path_buf(),
            });
        }
    }
    Ok(())
}

fn write_observation(project_root: &Path, record: &ObservationRecord) -> Result<()> {
    let path = observation_path(project_root, &record.id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_yaml::to_string(record)?)?;
    Ok(())
}

fn observation_id(agent: Option<&str>, source_kind: &str, source: &str, body: &str) -> String {
    let agent = agent.unwrap_or("local");
    let hash = fsutil::sha256_text(&format!("{source_kind}:{source}:{body}"));
    format!("obs:{agent}:{}", &hash[..12])
}

fn observation_path(project_root: &Path, id: &str) -> PathBuf {
    let safe = id.replace(':', "/").replace(['\\', ' '], "-");
    observations_dir(project_root).join(format!("{safe}.yml"))
}

fn observations_dir(project_root: &Path) -> PathBuf {
    config::kernel_dir(project_root).join("observations")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn import_file_stores_redacted_observation() {
        let temp = tempfile::tempdir().expect("tempdir");
        let transcript = temp.path().join("session.jsonl");
        fs::write(
            &transcript,
            r#"{"type":"user","message":"Always use Vitest. token=abc123456789xyz"}"#,
        )
        .expect("write transcript");

        let report = import_observation_file(
            temp.path(),
            &transcript,
            "claude-code-session",
            Some("claude-code"),
        )
        .expect("import");
        let observations = load_observations(temp.path()).expect("load observations");

        assert_eq!(report.created, 1);
        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].agent.as_deref(), Some("claude-code"));
        assert!(observations[0].body.contains("Always use Vitest"));
        assert!(!observations[0].body.contains("abc123456789xyz"));
    }

    #[test]
    fn discover_local_conversation_files_finds_claude_and_codex_jsonl() {
        let home = tempfile::tempdir().expect("home");
        let claude = home
            .path()
            .join(".claude")
            .join("projects")
            .join("demo")
            .join("session.jsonl");
        let codex = home
            .path()
            .join(".codex")
            .join("sessions")
            .join("2026")
            .join("05")
            .join("01")
            .join("rollout-1.jsonl");
        fs::create_dir_all(claude.parent().expect("claude parent")).expect("claude dir");
        fs::create_dir_all(codex.parent().expect("codex parent")).expect("codex dir");
        fs::write(&claude, "claude").expect("claude file");
        fs::write(&codex, "codex").expect("codex file");

        let files = discover_local_conversation_files(home.path()).expect("discover");

        assert!(files.iter().any(|item| item.agent == "claude-code"));
        assert!(files.iter().any(|item| item.agent == "codex"));
    }
}
