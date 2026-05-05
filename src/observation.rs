use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::candidate::{self, NewCandidate};
use crate::config;
use crate::extract;
use crate::fsutil;
use crate::provider;
use crate::skilllet;
use crate::textutil;

mod conversation;
mod incremental;

use conversation::{
    collect_jsonl, collect_single_jsonl, conversation_belongs_to_project,
    normalize_observation_body,
};
#[cfg(test)]
use incremental::{ObservationIndex, ObservationSourceState, metadata_modified_unix_ms};
use incremental::{
    load_observation_index, read_incremental_conversation_text, save_observation_index,
};

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
pub struct AutoEvolveReport {
    pub projects: usize,
    pub imported: usize,
    pub skipped: usize,
    pub drafts_created: usize,
    pub draft_candidates: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ObservationSynthesisReport {
    pub engine: String,
    pub created: usize,
    pub skipped: usize,
    pub candidates: usize,
    pub drafts: Vec<String>,
    pub candidate_drafts: Vec<String>,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ObservationEvolveReport {
    pub engine: String,
    pub imported: usize,
    pub import_skipped: usize,
    pub drafts_created: usize,
    pub draft_candidates: usize,
    pub synthesis_skipped: usize,
    pub drafts: Vec<String>,
    pub candidate_drafts: Vec<String>,
    pub dry_run: bool,
}

#[derive(Debug, Clone)]
pub struct ConversationFile {
    pub agent: String,
    pub source_kind: String,
    pub path: PathBuf,
    pub project_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize)]
struct AgentSkillletCandidate {
    title: String,
    body: String,
    #[serde(default)]
    brief: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default = "default_candidate_kind")]
    kind: String,
    #[serde(default = "default_candidate_scope")]
    scope: String,
    #[serde(default)]
    confidence: Option<f32>,
    #[serde(default)]
    reason: Option<String>,
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
        out.push_str(&format!("Engine: {}\n", self.engine));
        out.push_str(&format!("Candidates written: {}\n", self.created));
        out.push_str(&format!("Candidate previews: {}\n", self.candidates));
        out.push_str(&format!("Skipped observations: {}\n", self.skipped));
        if !self.drafts.is_empty() {
            out.push_str("\nDrafts:\n");
            for id in &self.drafts {
                out.push_str(&format!("- {id}\n"));
            }
        }
        if !self.candidate_drafts.is_empty() {
            out.push_str("\nCandidate records:\n");
            for id in &self.candidate_drafts {
                out.push_str(&format!("- {id}\n"));
            }
        }
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
        out.push_str(&format!("Engine: {}\n", self.engine));
        out.push_str(&format!("Observations imported: {}\n", self.imported));
        out.push_str(&format!("Import skipped: {}\n", self.import_skipped));
        out.push_str(&format!("Candidates written: {}\n", self.drafts_created));
        out.push_str(&format!("Candidate previews: {}\n", self.draft_candidates));
        out.push_str(&format!("Synthesis skipped: {}\n", self.synthesis_skipped));
        if !self.drafts.is_empty() {
            out.push_str("\nDrafts:\n");
            for id in &self.drafts {
                out.push_str(&format!("- {id}\n"));
            }
        }
        if !self.candidate_drafts.is_empty() {
            out.push_str("\nCandidate records:\n");
            for id in &self.candidate_drafts {
                out.push_str(&format!("- {id}\n"));
            }
        }
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

pub fn import_observation_text(
    project_root: &Path,
    source_path: &Path,
    source_kind: &str,
    agent: Option<&str>,
    text: &str,
) -> Result<ObservationImportReport> {
    let root = fsutil::normalize_project_root(project_root)?;
    let body = normalize_observation_body(text, source_path, source_kind);
    if body.trim().is_empty() {
        return Ok(ObservationImportReport {
            created: 0,
            skipped: 1,
            observations: Vec::new(),
        });
    }

    let redacted = provider::redact_secrets(&body);
    let id = observation_id(
        agent,
        source_kind,
        &source_path.display().to_string(),
        &redacted,
    );
    let record = ObservationRecord {
        id: id.clone(),
        source_kind: source_kind.to_string(),
        source_path: fsutil::path_to_slash(source_path),
        agent: agent.map(str::to_string),
        body: redacted.clone(),
        evidence: format!("Imported from {}", fsutil::path_to_slash(source_path)),
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
    import_local_conversations_incremental(project_root, home)
}

pub fn import_local_conversations_incremental(
    project_root: &Path,
    home: &Path,
) -> Result<ObservationImportReport> {
    let files = discover_local_conversation_files(home)?;
    import_local_conversations_from_files(project_root, &files)
}

fn import_local_conversations_from_files(
    project_root: &Path,
    files: &[ConversationFile],
) -> Result<ObservationImportReport> {
    let root = fsutil::normalize_project_root(project_root)?;
    let mut report = ObservationImportReport {
        created: 0,
        skipped: 0,
        observations: Vec::new(),
    };
    let mut index = load_observation_index(&root)?;
    for file in files {
        if !conversation_belongs_to_project(file, &root) {
            report.skipped += 1;
            continue;
        }
        let Some(increment) = read_incremental_conversation_text(file, &mut index)? else {
            report.skipped += 1;
            continue;
        };
        let imported = import_observation_text(
            &root,
            &file.path,
            &file.source_kind,
            Some(&file.agent),
            &increment,
        )?;
        report.created += imported.created;
        report.skipped += imported.skipped;
        report.observations.extend(imported.observations);
    }
    save_observation_index(&root, &index)?;
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

pub fn count_observations(project_root: &Path) -> Result<usize> {
    let root = fsutil::normalize_project_root(project_root)?;
    count_yml_files(&observations_dir(&root))
}

pub fn synthesize_observations_to_drafts(
    project_root: &Path,
    targets: Vec<String>,
    dry_run: bool,
) -> Result<ObservationSynthesisReport> {
    synthesize_observations_to_drafts_with_engine(project_root, targets, dry_run, "local")
}

pub fn synthesize_observations_to_drafts_with_engine(
    project_root: &Path,
    targets: Vec<String>,
    dry_run: bool,
    engine: &str,
) -> Result<ObservationSynthesisReport> {
    let observations = load_observations(project_root)?;
    let mut report = ObservationSynthesisReport {
        engine: engine.to_string(),
        created: 0,
        skipped: 0,
        candidates: 0,
        drafts: Vec::new(),
        candidate_drafts: Vec::new(),
        dry_run,
    };

    if observations.is_empty() {
        return Ok(report);
    }

    let source = observation_source_summary(&observations);
    let material = synthesis_material(&observations, 80_000);
    let extracted = match engine {
        "local" => extract::extract_high_value_text_to_drafts(
            project_root,
            &material,
            targets.clone(),
            &source,
            Some("local".to_string()),
            true,
            12,
        )?,
        "llm" => extract::extract_high_value_text_to_drafts(
            project_root,
            &material,
            targets.clone(),
            &source,
            None,
            true,
            12,
        )?,
        "claude-code" | "codex" => {
            let (prefiltered, filtered_material) = prefilter_agent_synthesis_material(
                project_root,
                &observations,
                targets.clone(),
                &source,
            )?;
            if prefiltered.candidates.is_empty() {
                report.skipped += observations.len();
                return Ok(report);
            }
            match synthesize_with_agent_engine(
                project_root,
                &filtered_material,
                targets.clone(),
                &source,
                dry_run,
                engine,
            ) {
                Ok(agent_report) => return Ok(agent_report),
                Err(_) => {
                    report.engine = format!("{engine} -> local fallback");
                    extract::extract_high_value_text_to_drafts(
                        project_root,
                        &filtered_material,
                        targets.clone(),
                        &source,
                        Some("local".to_string()),
                        true,
                        12,
                    )?
                }
            }
        }
        _ => extract::extract_high_value_text_to_drafts(
            project_root,
            &material,
            targets.clone(),
            &source,
            Some("local".to_string()),
            true,
            12,
        )?,
    };
    // 对已有 skilllet 做语义去重，避免重复写入已批准的候选项
    let existing_skilllets = skilllet::load_skilllets(project_root)?;
    let original_count = extracted.candidates.len();
    let synthesized_candidates: Vec<extract::ExtractCandidatePreview> = extracted
        .candidates
        .into_iter()
        .filter(|candidate| {
            for skilllet in &existing_skilllets {
                if textutil::jaccard_similarity(&candidate.body, &skilllet.body) >= 0.75 {
                    return false;
                }
            }
            true
        })
        .collect();
    let dedup_skipped = original_count - synthesized_candidates.len();
    let mut created_ids = Vec::new();
    let mut skipped_count = extracted.skipped.len() + dedup_skipped;
    if !dry_run {
        for candidate in &synthesized_candidates {
            let result = candidate::add_candidate(
                project_root,
                NewCandidate {
                    id: candidate.id.clone(),
                    title: candidate.title.clone(),
                    kind: candidate.kind.clone(),
                    scope: candidate.scope.clone(),
                    body: candidate.body.clone(),
                    brief: None,
                    tags: Vec::new(),
                    language: None,
                    targets: targets.clone(),
                    evidence: candidate.evidence.clone(),
                    confidence: candidate.confidence,
                    reason: candidate.reason.clone(),
                    matched_template: candidate.matched_template.clone(),
                    source_observations: observations
                        .iter()
                        .map(|observation| observation.id.clone())
                        .collect(),
                    extraction: candidate::ExtractionMetadata::default(),
                },
            );
            match result {
                Ok(()) => created_ids.push(candidate.id.clone()),
                Err(_) => skipped_count += 1,
            }
        }
    }
    let created_count = created_ids.len();
    let candidate_count = synthesized_candidates.len();
    report.created += created_count;
    report.drafts.extend(created_ids.clone());
    report.candidate_drafts.extend(
        synthesized_candidates
            .iter()
            .map(|candidate| candidate.id.clone()),
    );
    report.skipped += skipped_count;
    report.candidates += candidate_count;
    if created_count == 0 && candidate_count == 0 {
        report.skipped += observations.len();
    }

    Ok(report)
}

fn prefilter_agent_synthesis_material(
    project_root: &Path,
    observations: &[ObservationRecord],
    targets: Vec<String>,
    source: &str,
) -> Result<(extract::ExtractReport, String)> {
    let raw_material = synthesis_material(observations, 80_000);
    let prefiltered = extract::extract_high_value_text_to_drafts(
        project_root,
        &raw_material,
        targets,
        source,
        Some("local".to_string()),
        true,
        12,
    )?;
    let filtered_material = candidate_synthesis_material(&prefiltered.candidates, 8_000);
    Ok((prefiltered, filtered_material))
}

fn candidate_synthesis_material(
    candidates: &[extract::ExtractCandidatePreview],
    max_chars: usize,
) -> String {
    let mut out = String::new();
    for candidate in candidates {
        if out.len() >= max_chars {
            break;
        }
        out.push_str("\n---\n");
        out.push_str(&format!(
            "title: {}\nkind: {}\nscope: {}\nconfidence: {:.0}%\nevidence: {}\nreason: {}\nbody:\n{}\n",
            candidate.title,
            candidate.kind,
            candidate.scope,
            candidate.confidence.unwrap_or(0.0) * 100.0,
            candidate.evidence,
            candidate.reason.as_deref().unwrap_or("local high-value prefilter"),
            candidate.body
        ));
    }
    if out.len() > max_chars {
        out.truncate(max_chars);
    }
    out
}

fn synthesize_with_agent_engine(
    project_root: &Path,
    material: &str,
    targets: Vec<String>,
    source: &str,
    dry_run: bool,
    engine: &str,
) -> Result<ObservationSynthesisReport> {
    let prompt = agent_synthesis_prompt(material);
    let output = run_agent_engine(engine, &prompt, Duration::from_secs(60))?;
    let mut candidates = parse_agent_candidates(&output)?;
    candidates.sort_by(|a, b| {
        b.confidence
            .unwrap_or(0.0)
            .total_cmp(&a.confidence.unwrap_or(0.0))
            .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
    });
    candidates.truncate(8);

    let mut report = ObservationSynthesisReport {
        engine: engine.to_string(),
        created: 0,
        skipped: 0,
        candidates: candidates.len(),
        drafts: Vec::new(),
        candidate_drafts: Vec::new(),
        dry_run,
    };

    for candidate in candidates {
        if !is_usable_agent_candidate(&candidate) {
            report.skipped += 1;
            continue;
        }
        let id = format!("project:{}", textutil::slug(&candidate.title));
        report.candidate_drafts.push(id.clone());
        if dry_run {
            continue;
        }
        candidate::add_candidate(
            project_root,
            NewCandidate {
                id: id.clone(),
                title: candidate.title,
                kind: normalize_candidate_kind(&candidate.kind),
                scope: normalize_candidate_scope(&candidate.scope),
                body: candidate.body,
                brief: candidate.brief,
                tags: candidate.tags,
                language: candidate.language,
                targets: targets.clone(),
                evidence: format!("{source}: synthesized by {engine}"),
                confidence: candidate.confidence.or(Some(0.84)),
                reason: candidate.reason.or_else(|| {
                    Some(format!(
                        "Synthesized by {engine} from local Claude Code / Codex observations."
                    ))
                }),
                matched_template: Some(format!("agent-synthesis:{engine}")),
                source_observations: Vec::new(),
                extraction: candidate::ExtractionMetadata::default(),
            },
        )?;
        report.created += 1;
        report.drafts.push(id);
    }

    Ok(report)
}

fn agent_synthesis_prompt(material: &str) -> String {
    format!(
        r#"You are helping build Agent-Kernel, a local Skilllet evolution engine.

Extract only durable, high-value agent skills from the local conversation material.

Definition:
- A Skill is an agent capability package: triggerable, reusable, procedural, and useful across future tasks.
- A Skilllet is lighter: one stable preference, constraint, convention, workflow, correction, project improvement, root-cause learning, architecture decision, or supplement that can be compiled into Claude Code / Codex instructions or attached to a Skill.
- Keep only items that would still improve future work after the current bug or feature request is finished.

Reject:
- one-off requests like "continue", "fix this", "optimize UI", "how do I start"
- stack traces, terminal output, base instructions, system/developer prompts
- vague project brainstorming without a durable future behavior
- unresolved product requests or bug reports such as "add a progress window", "UI is ugly", "tell me why it is stuck"
- secrets, credentials, personal sensitive content
- raw error logs unless they include the reusable cause and fix

Return only a JSON array, no markdown. Max 12 items.
Each item: title, body, brief, tags, language, kind, scope, confidence, reason.
Allowed kind: preference, constraint, procedure, convention, correction, anti-pattern.
Allowed scope: global, project, agent.
Brief must be a concise Simplified Chinese explanation of what the candidate is for.
Tags must be compact and content-specific, for example axios, bun, frontend, http, js, structured-data, parser, 通用范式, agent-behavior, tool-use, meta-instruction.
Use confidence >= 0.78 only. Body must be concise, general, imperative, and reusable.

Material:
{material}"#
    )
}

fn run_agent_engine(engine: &str, prompt: &str, timeout: Duration) -> Result<String> {
    let mut command = match engine {
        "claude-code" => {
            let mut command = Command::new("claude");
            command.args([
                "-p",
                "--output-format",
                "text",
                "--permission-mode",
                "dontAsk",
                "--max-budget-usd",
                "0.25",
            ]);
            command
        }
        "codex" => {
            let mut command = Command::new("codex");
            command.args([
                "exec",
                "--skip-git-repo-check",
                "--sandbox",
                "read-only",
                "-",
            ]);
            command
        }
        _ => anyhow::bail!("unsupported synthesis engine `{engine}`"),
    };
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().with_context(|| format!("spawn {engine}"))?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(prompt.as_bytes())?;
    }
    drop(child.stdin.take());

    let started = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            let output = child.wait_with_output()?;
            if !output.status.success() {
                anyhow::bail!(
                    "{} exited with {}: {}",
                    engine,
                    output.status,
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            return Ok(String::from_utf8_lossy(&output.stdout).to_string());
        }
        if started.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("{engine} synthesis timed out after {}s", timeout.as_secs());
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn parse_agent_candidates(output: &str) -> Result<Vec<AgentSkillletCandidate>> {
    let trimmed = output.trim();
    if let Ok(candidates) = serde_json::from_str::<Vec<AgentSkillletCandidate>>(trimmed) {
        return Ok(candidates);
    }
    let Some(start) = trimmed.find('[') else {
        anyhow::bail!("agent output did not contain a JSON array");
    };
    let Some(end) = trimmed.rfind(']') else {
        anyhow::bail!("agent output did not contain a complete JSON array");
    };
    serde_json::from_str(&trimmed[start..=end]).context("parse agent JSON candidates")
}

fn is_usable_agent_candidate(candidate: &AgentSkillletCandidate) -> bool {
    let confidence = candidate.confidence.unwrap_or(0.0);
    !candidate.title.trim().is_empty()
        && !candidate.body.trim().is_empty()
        && candidate.body.len() <= 320
        && confidence >= 0.78
}

fn normalize_candidate_kind(kind: &str) -> String {
    match kind {
        "preference" | "constraint" | "procedure" | "convention" | "correction"
        | "anti-pattern" => kind.to_string(),
        _ => "procedure".to_string(),
    }
}

fn normalize_candidate_scope(scope: &str) -> String {
    match scope {
        "global" | "project" | "agent" => scope.to_string(),
        _ => "project".to_string(),
    }
}

fn default_candidate_kind() -> String {
    "procedure".to_string()
}

fn default_candidate_scope() -> String {
    "project".to_string()
}

pub fn evolve_local_conversations(
    project_root: &Path,
    home: &Path,
    targets: Vec<String>,
    dry_run: bool,
) -> Result<ObservationEvolveReport> {
    evolve_local_conversations_with_engine(project_root, home, targets, dry_run, "local")
}

pub fn evolve_local_conversations_with_engine(
    project_root: &Path,
    home: &Path,
    targets: Vec<String>,
    dry_run: bool,
    engine: &str,
) -> Result<ObservationEvolveReport> {
    let files = discover_local_conversation_files(home)?;
    evolve_local_conversations_from_files_with_engine(
        project_root,
        &files,
        targets,
        dry_run,
        engine,
    )
}

fn evolve_local_conversations_from_files_with_engine(
    project_root: &Path,
    files: &[ConversationFile],
    targets: Vec<String>,
    dry_run: bool,
    engine: &str,
) -> Result<ObservationEvolveReport> {
    let imported = import_local_conversations_from_files(project_root, files)?;
    let synthesized =
        synthesize_observations_to_drafts_with_engine(project_root, targets, dry_run, engine)?;
    Ok(ObservationEvolveReport {
        engine: synthesized.engine,
        imported: imported.created,
        import_skipped: imported.skipped,
        drafts_created: synthesized.created,
        draft_candidates: synthesized.candidates,
        synthesis_skipped: synthesized.skipped,
        drafts: synthesized.drafts,
        candidate_drafts: synthesized.candidate_drafts,
        dry_run,
    })
}

pub fn auto_evolve_registered_projects(
    home: &Path,
    engine: &str,
    targets: Vec<String>,
) -> Result<AutoEvolveReport> {
    let files = discover_local_conversation_files(home)?;
    auto_evolve_registered_projects_from_files(home, &files, engine, targets)
}

pub fn auto_evolve_registered_projects_from_files(
    home: &Path,
    files: &[ConversationFile],
    engine: &str,
    targets: Vec<String>,
) -> Result<AutoEvolveReport> {
    let registry = crate::project_registry::load_registry_with_agent_projects(home)?;
    let mut report = AutoEvolveReport {
        projects: 0,
        imported: 0,
        skipped: 0,
        drafts_created: 0,
        draft_candidates: 0,
    };

    for project in registry.projects {
        let project_path = PathBuf::from(&project.path);
        if !project_path.exists() {
            report.skipped += 1;
            continue;
        }
        let evolved = evolve_local_conversations_from_files_with_engine(
            &project_path,
            files,
            targets.clone(),
            false,
            engine,
        )?;
        report.projects += 1;
        report.imported += evolved.imported;
        report.skipped += evolved.import_skipped + evolved.synthesis_skipped;
        report.drafts_created += evolved.drafts_created;
        report.draft_candidates += evolved.draft_candidates;
    }

    Ok(report)
}

pub fn discover_local_conversation_files(home: &Path) -> Result<Vec<ConversationFile>> {
    let mut files = Vec::new();
    collect_jsonl(
        &home.join(".claude").join("projects"),
        "claude-code",
        "claude-code-session",
        &mut files,
    )?;
    collect_single_jsonl(
        &home.join(".claude").join("history.jsonl"),
        "claude-code",
        "claude-code-history",
        &mut files,
    );
    collect_jsonl(
        &home.join(".claude").join("sessions"),
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
    collect_single_jsonl(
        &home.join(".codex").join("history.jsonl"),
        "codex",
        "codex-history",
        &mut files,
    );
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

fn synthesis_material(observations: &[ObservationRecord], max_chars: usize) -> String {
    let mut sorted = observations.to_vec();
    sorted.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    let mut out = String::new();
    for observation in sorted {
        if out.len() >= max_chars {
            break;
        }
        let mut body = observation.body.replace('\0', "");
        if body.len() > 4_000 {
            body.truncate(4_000);
        }
        out.push_str("\n---\n");
        out.push_str(&format!(
            "source: {}\nagent: {}\nevidence: {}\ntext:\n{}\n",
            observation.source_kind,
            observation.agent.as_deref().unwrap_or("unknown"),
            observation.evidence,
            body
        ));
    }
    if out.len() > max_chars {
        out.truncate(max_chars);
    }
    out
}

fn observation_source_summary(observations: &[ObservationRecord]) -> String {
    let ids = observations
        .iter()
        .take(5)
        .map(|observation| format!("observation:{}", observation.id))
        .collect::<Vec<_>>()
        .join(", ");
    if ids.is_empty() {
        "0 observations".to_string()
    } else {
        format!("{} observations: {}", observations.len(), ids)
    }
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

fn observation_path(project_root: &Path, id: &str) -> PathBuf {
    let safe = id.replace(':', "/").replace(['\\', ' '], "-");
    observations_dir(project_root).join(format!("{safe}.yml"))
}

fn observations_dir(project_root: &Path) -> PathBuf {
    config::kernel_dir(project_root).join("observations")
}

fn count_yml_files(dir: &Path) -> Result<usize> {
    if !dir.exists() {
        return Ok(0);
    }
    let mut count = 0;
    for entry in walkdir::WalkDir::new(dir).follow_links(false) {
        let entry = entry?;
        if entry.file_type().is_file()
            && entry.path().extension().and_then(|value| value.to_str()) == Some("yml")
        {
            count += 1;
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests;
