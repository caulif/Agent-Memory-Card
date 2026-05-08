use std::fs;
#[cfg(test)]
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::candidate::{self, NewCandidate};
use crate::config;
use crate::extract;
use crate::fsutil;
use crate::memory_card;
use crate::provider;
use crate::textutil;

mod agent_engine_impl;
mod chunked;
mod conversation;
mod incremental;
mod replay;
mod report_render;
mod sessions_index;

use agent_engine_impl::{
    default_candidate_kind, default_candidate_scope, is_usable_agent_candidate,
    normalize_candidate_kind, normalize_candidate_scope, parse_agent_candidates, run_agent_engine,
};
use conversation::{
    collect_jsonl, collect_single_jsonl, conversation_belongs_to_project,
    normalize_observation_body,
};
#[cfg(test)]
use incremental::{ObservationIndex, ObservationSourceState, metadata_modified_unix_ms};
use incremental::{
    load_observation_index, read_incremental_conversation_text, save_observation_index,
};
pub use replay::{ObservationReplayReport, replay_local_conversations};

const DAILY_CANDIDATE_LIMIT: usize = 8;

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
struct AgentMemoryCardCandidate {
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

    let source = "observation synthesis".to_string();
    let extracted = match engine {
        "local" => chunked::extract_local_chunks_to_report(
            project_root,
            &observations,
            targets.clone(),
            &source,
            DAILY_CANDIDATE_LIMIT,
        )?,
        "llm" => chunked::extract_llm_chunks_to_report(
            project_root,
            &observations,
            targets.clone(),
            &source,
            DAILY_CANDIDATE_LIMIT,
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
                Ok(agent_report) if agent_report_has_enough_recall(&agent_report, 3) => {
                    return Ok(agent_report);
                }
                Ok(agent_report) => {
                    report.engine = format!(
                        "{engine} -> local fallback (agent returned {} candidates)",
                        agent_report.candidates
                    );
                    chunked::extract_local_chunks_to_report(
                        project_root,
                        &observations,
                        targets.clone(),
                        &source,
                        DAILY_CANDIDATE_LIMIT,
                    )?
                }
                Err(error) => {
                    report.engine = format!("{engine} -> local fallback ({})", short_error(&error));
                    chunked::extract_local_chunks_to_report(
                        project_root,
                        &observations,
                        targets.clone(),
                        &source,
                        DAILY_CANDIDATE_LIMIT,
                    )?
                }
            }
        }
        _ => chunked::extract_local_chunks_to_report(
            project_root,
            &observations,
            targets.clone(),
            &source,
            DAILY_CANDIDATE_LIMIT,
        )?,
    };
    let synthesized_candidates = extracted
        .candidates
        .into_iter()
        .filter(|candidate| {
            candidate
                .classification
                .as_ref()
                .is_none_or(|classification| classification.artifact_kind != "reject")
        })
        .collect::<Vec<_>>();
    let mut created_ids = Vec::new();
    let mut skipped_count = extracted.skipped.len();
    if !dry_run {
        for candidate in &synthesized_candidates {
            let source_observations =
                relevant_source_observations(&observations, &candidate.evidence, &candidate.body);
            let extraction = candidate::ExtractionMetadata {
                origin: "user".to_string(),
                matched_signal: candidate
                    .classification
                    .as_ref()
                    .map(|classification| classification.signal.clone())
                    .unwrap_or_default(),
                reason: candidate.reason.clone().unwrap_or_default(),
                source_observations: source_observations.clone(),
                score_breakdown: Default::default(),
                classification: candidate.classification.clone(),
                similar_record: candidate
                    .suggested_action
                    .as_ref()
                    .and_then(|action| action.record_id.clone()),
                tags: candidate.tags.clone(),
                suggested_action: candidate.suggested_action.clone(),
                evidence_span: Some(candidate::EvidenceSpan {
                    role: "user".to_string(),
                    quote: candidate.evidence.clone(),
                    observation_id: source_observations.first().cloned(),
                    turn_id: None,
                    surrounding_context: Vec::new(),
                }),
                memory_tier: candidate.memory_tier.clone(),
                value_scores: Default::default(),
                abstraction_of: candidate.abstraction_of.clone(),
                abstracted_from: candidate.abstracted_from.clone(),
            };
            let result = candidate::add_candidate(
                project_root,
                NewCandidate {
                    id: candidate.id.clone(),
                    title: candidate.title.clone(),
                    kind: candidate.kind.clone(),
                    scope: candidate.scope.clone(),
                    body: candidate.body.clone(),
                    brief: None,
                    tags: candidate.tags.clone(),
                    language: None,
                    targets: targets.clone(),
                    evidence: candidate.evidence.clone(),
                    confidence: candidate.confidence,
                    reason: candidate.reason.clone(),
                    matched_template: candidate.matched_template.clone(),
                    source_observations,
                    extraction,
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

fn relevant_source_observations(
    observations: &[ObservationRecord],
    evidence: &str,
    body: &str,
) -> Vec<String> {
    let evidence_snippet = evidence
        .split_once(": ")
        .map(|(_, snippet)| snippet)
        .unwrap_or(evidence)
        .trim();
    let body = body.trim();
    let matched = observations
        .iter()
        .filter(|observation| {
            let text = observation.body.trim();
            (!evidence_snippet.is_empty()
                && (text.contains(evidence_snippet) || evidence_snippet.contains(text)))
                || (!body.is_empty() && (text.contains(body) || body.contains(text)))
        })
        .map(|observation| observation.id.clone())
        .collect::<Vec<_>>();
    if matched.is_empty() {
        observations
            .iter()
            .take(1)
            .map(|observation| observation.id.clone())
            .collect()
    } else {
        matched
    }
}

fn prefilter_agent_synthesis_material(
    project_root: &Path,
    observations: &[ObservationRecord],
    targets: Vec<String>,
    source: &str,
) -> Result<(extract::ExtractReport, String)> {
    let raw_material = chunked::prefiltered_synthesis_material(observations, 24_000);
    let prefiltered = extract::extract_high_value_text_to_drafts(
        project_root,
        &raw_material,
        targets,
        source,
        Some("local".to_string()),
        true,
        DAILY_CANDIDATE_LIMIT,
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
        truncate_utf8_boundary(&mut out, max_chars);
    }
    out
}

fn truncate_utf8_boundary(text: &mut String, max_len: usize) {
    if text.len() <= max_len {
        return;
    }
    let mut new_len = max_len;
    while new_len > 0 && !text.is_char_boundary(new_len) {
        new_len -= 1;
    }
    text.truncate(new_len);
}

fn short_error(error: &anyhow::Error) -> String {
    let mut text = error.to_string().replace('\n', " ");
    if text.len() > 160 {
        truncate_utf8_boundary(&mut text, 160);
    }
    text
}

fn agent_report_has_enough_recall(report: &ObservationSynthesisReport, minimum: usize) -> bool {
    report.candidates >= minimum
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
    let mut candidates = filter_agent_candidates_through_local_gate(
        project_root,
        parse_agent_candidates(&output)?,
        source,
    )?;
    candidates.sort_by(|(a, _, _), (b, _, _)| {
        b.confidence
            .unwrap_or(0.0)
            .total_cmp(&a.confidence.unwrap_or(0.0))
            .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
    });
    candidates.truncate(5);

    let mut report = ObservationSynthesisReport {
        engine: engine.to_string(),
        created: 0,
        skipped: 0,
        candidates: candidates.len(),
        drafts: Vec::new(),
        candidate_drafts: Vec::new(),
        dry_run,
    };

    for (candidate, score, action) in candidates {
        if !is_usable_agent_candidate(&candidate) {
            report.skipped += 1;
            continue;
        }
        let scope = normalize_candidate_scope(&candidate.scope);
        let id = format!("{}:{}", scope, textutil::slug(&candidate.title));
        report.candidate_drafts.push(id.clone());
        if dry_run {
            continue;
        }
        let chunk = agent_candidate_chunk(&candidate, source);
        let classification = extract::classify::classify_chunk(&chunk);
        let routed_action = action.with_route(&classification.artifact_kind);
        let extraction = candidate::ExtractionMetadata {
            origin: chunk.origin.as_str().to_string(),
            matched_signal: score.matched_signal.clone(),
            reason: score.reason.clone(),
            source_observations: Vec::new(),
            score_breakdown: score.breakdown.clone(),
            classification: Some(classification.clone()),
            similar_record: routed_action.record_id.clone(),
            tags: classification.tags,
            suggested_action: Some(routed_action),
            evidence_span: Some(candidate::EvidenceSpan {
                role: chunk.origin.as_str().to_string(),
                quote: candidate.body.clone(),
                observation_id: None,
                turn_id: None,
                surrounding_context: Vec::new(),
            }),
            memory_tier: if scope == "global" {
                candidate::MemoryTier::CrossProjectPrinciple
            } else {
                candidate::MemoryTier::ProjectRule
            },
            value_scores: Default::default(),
            abstraction_of: None,
            abstracted_from: None,
        };
        candidate::add_candidate(
            project_root,
            NewCandidate {
                id: id.clone(),
                title: candidate.title,
                kind: normalize_candidate_kind(&candidate.kind),
                scope,
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
                extraction,
            },
        )?;
        report.created += 1;
        report.drafts.push(id);
    }

    Ok(report)
}

fn filter_agent_candidates_through_local_gate(
    project_root: &Path,
    candidates: Vec<AgentMemoryCardCandidate>,
    source: &str,
) -> Result<
    Vec<(
        AgentMemoryCardCandidate,
        extract::scoring::ExtractionScore,
        candidate::ExtractionAction,
    )>,
> {
    let existing_memory_cards = memory_card::load_memory_cards(project_root)?;
    let deduper = extract::embedding::SemanticDeduper::new(0.75, 0.65);
    let mut retained = Vec::new();

    for candidate in candidates {
        if !is_usable_agent_candidate(&candidate) {
            continue;
        }
        if looks_like_generated_enabled_memory_card_candidate(&candidate) {
            continue;
        }
        let chunk = agent_candidate_chunk(&candidate, source);
        let score = extract::scoring::score_chunk(&chunk);
        if score.disposition != extract::scoring::ExtractionDisposition::Candidate {
            continue;
        }
        let scope_memory_cards = existing_memory_cards
            .iter()
            .filter(|record| match candidate.scope.as_str() {
                "global" => record.scope == "global",
                "agent" => record.scope == "agent",
                _ => record.scope != "global",
            })
            .cloned()
            .collect::<Vec<_>>();
        let action = match deduper.dedup_against_existing(&candidate.body, &scope_memory_cards) {
            extract::embedding::DedupResult::Duplicate {
                similar_id,
                similarity,
            } => candidate::ExtractionAction::merge_into_existing(similar_id, similarity),
            extract::embedding::DedupResult::Unique => candidate::ExtractionAction::new_candidate(),
        };
        let classification = extract::classify::classify_chunk(&chunk);
        retained.push((
            candidate,
            score,
            action.with_route(&classification.artifact_kind),
        ));
    }

    Ok(retained)
}

fn looks_like_generated_enabled_memory_card_candidate(
    candidate: &AgentMemoryCardCandidate,
) -> bool {
    let lower = format!(
        "{}\n{}\n{}\n{}",
        candidate.title,
        candidate.body,
        candidate.brief.as_deref().unwrap_or_default(),
        candidate.reason.as_deref().unwrap_or_default()
    )
    .to_lowercase();
    [
        "enabled memory_cards",
        "generated by agent-kernel",
        "use bun for javascript package management",
        "use axios for frontend http requests",
        "delegate ui polish",
        "invoke claude code first as the ui optimization agent",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn agent_candidate_chunk(
    candidate: &AgentMemoryCardCandidate,
    source: &str,
) -> extract::chunk::EvidenceChunk {
    extract::chunk::EvidenceChunk {
        id: candidate.title.clone(),
        text: format!("{}\n{}", candidate.title, candidate.body),
        origin: extract::chunk::ChunkOrigin::AiSynthesis,
        source_kind: source.to_string(),
        source_observations: Vec::new(),
    }
}

fn agent_synthesis_prompt(material: &str) -> String {
    format!(
        r#"You are helping build Agent Memory Kernel, a local MemoryCard evolution engine.

Extract only durable, high-value agent skills from the local conversation material.

Definition:
- A Skill is an agent capability package: triggerable, reusable, procedural, and useful across future tasks.
- A MemoryCard is lighter: one stable preference, constraint, convention, workflow, correction, project improvement, root-cause learning, architecture decision, or supplement that can be compiled into Claude Code / Codex instructions or attached to a Skill.
- Keep only items that would still improve future work after the current bug or feature request is finished.
- Prefer a balanced set: project rules, cross-project principles, and collaboration preferences.
- Use scope="global" for cross-project principles and durable collaboration preferences; use scope="project" only when the rule depends on this project.
- Keep durable product quality constraints when they state a reusable acceptance bar, for example "outside model reasoning, interactions should not feel stuck or janky".

Reject:
- one-off requests like "continue", "fix this", "optimize UI", "how do I start"
- stack traces, terminal output, base instructions, system/developer prompts
- vague project brainstorming, PRD sections, fixture/gold-set requirements, or priority outlines without a durable future behavior
- unresolved product requests or bug reports such as "add a progress window", "UI is ugly", "tell me why it is stuck"; do not reject a product quality constraint when it includes a durable standard
- secrets, credentials, personal sensitive content
- raw error logs unless they include the reusable cause and fix

Return only a JSON array, no markdown. Max 12 items.
Each item: title, body, brief, tags, language, kind, scope, confidence, reason.
Allowed kind: preference, constraint, procedure, convention, correction, anti-pattern.
Allowed scope: global, project, agent.
Brief must be a concise Simplified Chinese explanation of what the candidate is for.
Tags must be compact and content-specific, for example axios, bun, frontend, http, js, structured-data, parser, 通用范式, agent-behavior, tool-use, meta-instruction.
Use confidence >= 0.78 only. Body must be concise, general, imperative, and reusable.

Recall targets:
- cross-project: core functionality first, user perspective/experience, planning before edits, real-history validation.
- collaboration: small-change fast tests / large-change broad tests, preserve human review boundaries, prefer candidate quality over quantity.
- product quality constraint: keep stable acceptance bars such as non-reasoning UI operations should remain smooth.

Material:
{material}"#
    )
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
