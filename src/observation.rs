use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config;
use crate::extract;
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

#[derive(Debug, Clone, Serialize)]
pub struct ObservationSynthesisReport {
    pub created: usize,
    pub skipped: usize,
    pub candidates: usize,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ObservationEvolveReport {
    pub imported: usize,
    pub import_skipped: usize,
    pub drafts_created: usize,
    pub draft_candidates: usize,
    pub synthesis_skipped: usize,
    pub dry_run: bool,
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

impl ObservationSynthesisReport {
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("Agent-Kernel observation synthesis\n\n");
        out.push_str(&format!("Drafts created: {}\n", self.created));
        out.push_str(&format!("Candidate previews: {}\n", self.candidates));
        out.push_str(&format!("Skipped observations: {}\n", self.skipped));
        if self.dry_run {
            out.push_str("Mode: dry run\n");
        }
        out
    }
}

impl ObservationEvolveReport {
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("Agent-Kernel local evolution\n\n");
        out.push_str(&format!("Observations imported: {}\n", self.imported));
        out.push_str(&format!("Import skipped: {}\n", self.import_skipped));
        out.push_str(&format!("Drafts created: {}\n", self.drafts_created));
        out.push_str(&format!("Draft candidates: {}\n", self.draft_candidates));
        out.push_str(&format!("Synthesis skipped: {}\n", self.synthesis_skipped));
        if self.dry_run {
            out.push_str("Mode: dry run\n");
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
    let body = normalize_observation_body(&text, file, source_kind);
    if body.trim().is_empty() {
        return Ok(ObservationImportReport {
            created: 0,
            skipped: 1,
            observations: Vec::new(),
        });
    }

    let redacted = provider::redact_secrets(&body);
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
    let created = write_observation(&root, &record)?;

    Ok(ObservationImportReport {
        created: usize::from(created),
        skipped: usize::from(!created),
        observations: if created { vec![id] } else { Vec::new() },
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

pub fn synthesize_observations_to_drafts(
    project_root: &Path,
    targets: Vec<String>,
    dry_run: bool,
) -> Result<ObservationSynthesisReport> {
    let observations = load_observations(project_root)?;
    let mut report = ObservationSynthesisReport {
        created: 0,
        skipped: 0,
        candidates: 0,
        dry_run,
    };

    for observation in observations {
        let extracted = extract::extract_text_to_drafts(
            project_root,
            &observation.body,
            targets.clone(),
            &format!("observation:{}", observation.id),
            Some("local".to_string()),
            dry_run,
        )?;
        report.created += extracted.created.len();
        report.skipped += extracted.skipped.len();
        report.candidates += extracted.candidates.len();
        if extracted.created.is_empty() && extracted.candidates.is_empty() {
            report.skipped += 1;
        }
    }

    Ok(report)
}

pub fn evolve_local_conversations(
    project_root: &Path,
    home: &Path,
    targets: Vec<String>,
    dry_run: bool,
) -> Result<ObservationEvolveReport> {
    let imported = import_local_conversations(project_root, home)?;
    let synthesized = synthesize_observations_to_drafts(project_root, targets, dry_run)?;
    Ok(ObservationEvolveReport {
        imported: imported.created,
        import_skipped: imported.skipped,
        drafts_created: synthesized.created,
        draft_candidates: synthesized.candidates,
        synthesis_skipped: synthesized.skipped,
        dry_run,
    })
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

fn write_observation(project_root: &Path, record: &ObservationRecord) -> Result<bool> {
    let path = observation_path(project_root, &record.id);
    if path.exists() {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_yaml::to_string(record)?)?;
    Ok(true)
}

fn observation_id(agent: Option<&str>, source_kind: &str, source: &str, body: &str) -> String {
    let agent = agent.unwrap_or("local");
    let hash = fsutil::sha256_text(&format!("{source_kind}:{source}:{body}"));
    format!("obs:{agent}:{}", &hash[..12])
}

fn normalize_observation_body(input: &str, file: &Path, source_kind: &str) -> String {
    let is_jsonl = file.extension().and_then(|value| value.to_str()) == Some("jsonl")
        || source_kind.contains("session");
    if !is_jsonl {
        return input.trim().to_string();
    }
    extract_jsonl_messages(input).unwrap_or_else(|| input.trim().to_string())
}

fn extract_jsonl_messages(input: &str) -> Option<String> {
    let mut parts = Vec::new();
    for line in input.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        collect_known_text_fields(&value, &mut parts);
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n").trim().to_string())
    }
}

fn collect_known_text_fields(value: &Value, parts: &mut Vec<String>) {
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
    use crate::draft;
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
    fn import_jsonl_extracts_message_text_without_wrappers() {
        let temp = tempfile::tempdir().expect("tempdir");
        let transcript = temp.path().join("session.jsonl");
        fs::write(
            &transcript,
            r#"{"type":"user","message":"Always use Vitest for frontend unit tests."}"#,
        )
        .expect("write transcript");

        import_observation_file(temp.path(), &transcript, "codex-session", Some("codex"))
            .expect("import");
        let observations = load_observations(temp.path()).expect("observations");

        assert_eq!(
            observations[0].body,
            "Always use Vitest for frontend unit tests."
        );
    }

    #[test]
    fn import_file_skips_existing_observation() {
        let temp = tempfile::tempdir().expect("tempdir");
        let transcript = temp.path().join("session.jsonl");
        fs::write(
            &transcript,
            r#"{"type":"user","message":"Always use Vitest for frontend unit tests."}"#,
        )
        .expect("write transcript");

        let first =
            import_observation_file(temp.path(), &transcript, "codex-session", Some("codex"))
                .expect("first import");
        let second =
            import_observation_file(temp.path(), &transcript, "codex-session", Some("codex"))
                .expect("second import");

        assert_eq!(first.created, 1);
        assert_eq!(second.created, 0);
        assert_eq!(second.skipped, 1);
        assert_eq!(
            load_observations(temp.path()).expect("observations").len(),
            1
        );
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

    #[test]
    fn synthesize_observations_creates_reviewable_drafts() {
        let temp = tempfile::tempdir().expect("tempdir");
        let transcript = temp.path().join("session.jsonl");
        fs::write(
            &transcript,
            r#"{"type":"user","message":"以后所有 Rust 项目必须先运行 cargo test 再提交。"}"#,
        )
        .expect("write transcript");
        import_observation_file(temp.path(), &transcript, "codex-session", Some("codex"))
            .expect("import");

        let report = synthesize_observations_to_drafts(
            temp.path(),
            vec!["codex".to_string(), "claude-code".to_string()],
            false,
        )
        .expect("synthesize");

        assert_eq!(report.created, 1);
        let drafts = draft::load_drafts(temp.path()).expect("drafts");
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].targets, vec!["codex", "claude-code"]);
        assert!(drafts[0].body.contains("cargo test"));
        assert!(drafts[0].evidence.contains("observation:obs:codex"));
    }

    #[test]
    fn synthesize_dry_run_does_not_write_drafts() {
        let temp = tempfile::tempdir().expect("tempdir");
        let transcript = temp.path().join("session.jsonl");
        fs::write(
            &transcript,
            r#"{"type":"user","message":"Always use Vitest for frontend unit tests."}"#,
        )
        .expect("write transcript");
        import_observation_file(
            temp.path(),
            &transcript,
            "claude-code-session",
            Some("claude-code"),
        )
        .expect("import");

        let report =
            synthesize_observations_to_drafts(temp.path(), vec!["codex".to_string()], true)
                .expect("synthesize");

        assert_eq!(report.created, 0);
        assert_eq!(report.candidates, 1);
        assert!(draft::load_drafts(temp.path()).expect("drafts").is_empty());
    }

    #[test]
    fn evolve_local_conversations_imports_and_synthesizes_drafts() {
        let temp = tempfile::tempdir().expect("project");
        let home = tempfile::tempdir().expect("home");
        let codex = home
            .path()
            .join(".codex")
            .join("sessions")
            .join("2026")
            .join("05")
            .join("01")
            .join("rollout-1.jsonl");
        fs::create_dir_all(codex.parent().expect("codex parent")).expect("codex dir");
        fs::write(
            &codex,
            r#"{"type":"user","message":"Always run cargo clippy before pushing."}"#,
        )
        .expect("write codex session");

        let report =
            evolve_local_conversations(temp.path(), home.path(), vec!["codex".to_string()], false)
                .expect("evolve");

        assert_eq!(report.imported, 1);
        assert_eq!(report.drafts_created, 1);
        let drafts = draft::load_drafts(temp.path()).expect("drafts");
        assert!(drafts[0].body.contains("cargo clippy"));
    }
}
