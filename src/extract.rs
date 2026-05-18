use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};

use crate::candidate;
use crate::candidate::MemoryTier;
use crate::draft::{self, NewDraft};
use crate::memory_card;
use crate::provider;
use crate::textutil;

mod r#abstract;
mod atomic;
mod candidate_factory;
pub mod chunk;
pub mod classify;
pub mod cluster;
pub mod crystallize;
mod dedupe;
pub(crate) mod embedding;
mod feedback_gate;
pub mod gate;
pub mod induce;
pub mod lifecycle;
mod llm;
mod llm_pipeline_impl;
pub(crate) mod memory_gate;
pub mod pipeline;
mod preference;
pub mod quality;
mod quality_gate;
mod ranking;
mod recurrence;
mod refine;
mod report;
pub mod scoring;
mod shared_impl;
mod signals;
pub mod strip;
pub mod truncate;

pub use preference::{
    PreferenceRegistryValidationReport, PreferenceTemplatePreview, PreferenceTestMatch,
    PreferenceTestReport, init_preference_registry, preference_templates, test_preference_text,
    validate_preference_registry,
};
pub use quality::{QualityReport, QualityTextCase, quality_report_for_text_cases};

use atomic::split_atomic_sentences;
use candidate_factory::{
    atomic_exception_candidate, classify_kind, draft_id, extraction_metadata_for_chunk,
    high_value_prompt_candidate, infer_scope, looks_like_memory_card_signal, looks_like_rule,
    normalize_body, normalize_project_improvement_body, principle_candidates,
    scored_signal_candidate, self_verification_candidate, title_from_body,
    title_from_project_improvement,
};
use dedupe::dedupe_candidates;
use llm_pipeline_impl::extract_llm_text_to_drafts;
use preference::{KnownPreference, load_known_preferences, normalize_known_preference};
use quality_gate::{QualityDisposition, evaluate_candidate_quality, quality_skip_message};
use shared_impl::*;
use signals::{has_explicit_memory_marker, looks_like_project_improvement_signal, split_sentences};

#[derive(Debug)]
pub struct ExtractReport {
    pub created: Vec<String>,
    pub skipped: Vec<String>,
    pub candidates: Vec<ExtractCandidatePreview>,
    pub dry_run: bool,
    pub provider: String,
    pub redacted: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ExtractCandidatePreview {
    pub id: String,
    pub title: String,
    pub body: String,
    pub kind: String,
    pub scope: String,
    pub memory_tier: MemoryTier,
    pub abstraction_of: Option<String>,
    pub abstracted_from: Option<String>,
    pub evidence: String,
    pub confidence: Option<f32>,
    pub reason: Option<String>,
    pub matched_template: Option<String>,
    pub classification: Option<classify::KnowledgeClassification>,
    pub tags: Vec<String>,
    pub suggested_action: Option<candidate::ExtractionAction>,
    pub operation: lifecycle::MemoryCardOperation,
    pub quality_flags: Vec<String>,
}

#[derive(Debug, Clone)]
pub(super) struct Candidate {
    title: String,
    body: String,
    kind: String,
    scope: String,
    memory_tier: MemoryTier,
    abstraction_of: Option<String>,
    abstracted_from: Option<String>,
    evidence: String,
    confidence: Option<f32>,
    reason: Option<String>,
    matched_template: Option<String>,
}

pub fn extract_to_drafts(
    project_root: &Path,
    text: Option<String>,
    file: Option<PathBuf>,
    targets: Vec<String>,
    provider_name: Option<String>,
    dry_run: bool,
) -> Result<ExtractReport> {
    let (input, source) = match (text, file) {
        (Some(text), None) => (text, "inline text".to_string()),
        (None, Some(path)) => (
            fs::read_to_string(&path)?,
            format!("file:{}", path.to_string_lossy()),
        ),
        _ => return Err(anyhow!("provide exactly one of --text or --file")),
    };

    extract_text_to_drafts(
        project_root,
        &input,
        targets,
        &source,
        provider_name,
        dry_run,
    )
}

pub fn extract_text_to_drafts(
    project_root: &Path,
    input: &str,
    targets: Vec<String>,
    source: &str,
    provider_name: Option<String>,
    dry_run: bool,
) -> Result<ExtractReport> {
    let provider_name = provider_name.unwrap_or_else(|| {
        provider::load_or_default_provider_config(project_root)
            .map(|cfg| cfg.extraction_provider)
            .unwrap_or_else(|_| "local".to_string())
    });

    if provider_name == "local" {
        return extract_local_text_to_drafts(project_root, input, targets, source, dry_run, false);
    }

    extract_llm_text_to_drafts(
        project_root,
        input,
        targets,
        source,
        &provider_name,
        dry_run,
        None,
    )
}

fn extract_local_text_to_drafts(
    project_root: &Path,
    input: &str,
    targets: Vec<String>,
    source: &str,
    dry_run: bool,
    allow_provider_refine: bool,
) -> Result<ExtractReport> {
    let provider_cfg = provider::load_or_default_provider_config(project_root)?;
    let redacted_input = if provider_cfg.privacy.redact_secrets {
        provider::redact_secrets(input)
    } else {
        input.to_string()
    };
    let redacted = redacted_input != input;
    let preferences = load_known_preferences(project_root)?;
    let mut candidates = extract_candidates_with_preferences(
        &redacted_input,
        &preferences,
        provider_cfg.fallback_methodology_templates,
    );
    recurrence::apply_recurrence_boost(project_root, &mut candidates)?;

    let existing_memory_cards = memory_card::load_memory_cards(project_root)?;
    let deduper = embedding::SemanticDeduper::new(0.75, 0.65);
    let mut skipped = Vec::new();
    let candidates: Vec<(Candidate, candidate::ExtractionAction)> = candidates
        .into_iter()
        .map(|candidate| {
            let scope_memory_cards = scoped_memory_cards(&existing_memory_cards, &candidate.scope);
            let action = match deduper.dedup_against_existing(&candidate.body, &scope_memory_cards)
            {
                embedding::DedupResult::Duplicate {
                    similar_id,
                    similarity,
                } => candidate::ExtractionAction::merge_into_existing(similar_id, similarity),
                embedding::DedupResult::Unique => candidate::ExtractionAction::new_candidate(),
            };
            (candidate, action)
        })
        .filter_map(|(mut candidate, action)| {
            let feedback = feedback_gate::apply_candidate_feedback(
                project_root,
                &draft_id(&candidate),
                &mut candidate,
            );
            if let Ok(Some(message)) = feedback {
                skipped.push(message);
                return None;
            } else if let Err(error) = feedback {
                skipped.push(format!("feedback: {error}"));
            }
            let decision = evaluate_candidate_quality(&candidate, &action);
            if decision.disposition == QualityDisposition::Skip {
                skipped.push(quality_skip_message(&draft_id(&candidate), &decision));
                None
            } else {
                let memory_decision = memory_gate::evaluate_memory_candidate(
                    &candidate.title,
                    &candidate.body,
                    &candidate.evidence,
                    &candidate.kind,
                    &candidate.scope,
                );
                if memory_decision.disposition == memory_gate::MemoryGateDisposition::Reject {
                    skipped.push(format!(
                        "{}: memory-gate ({})",
                        draft_id(&candidate),
                        memory_decision.flags.join(",")
                    ));
                    return None;
                }
                let action = if memory_decision.disposition
                    == memory_gate::MemoryGateDisposition::ReviewOnly
                {
                    action.with_route(memory_decision.route)
                } else {
                    action
                };
                Some((candidate, action))
            }
        })
        .collect();

    let mut scored_candidates: Vec<(
        Candidate,
        scoring::ExtractionScore,
        candidate::ExtractionAction,
        quality_gate::QualityGateDecision,
    )> = Vec::new();
    for (candidate, action) in candidates {
        let decision = evaluate_candidate_quality(&candidate, &action);
        let chunk = chunk::EvidenceChunk {
            id: candidate.title.clone(),
            text: candidate.evidence.clone(),
            origin: chunk::ChunkOrigin::User,
            source_kind: source.to_string(),
            source_observations: Vec::new(),
        };
        let score = scoring::score_chunk(&chunk);
        if score.disposition == scoring::ExtractionDisposition::Candidate
            || is_priority_template(&candidate)
        {
            scored_candidates.push((candidate, score, action, decision));
        }
    }
    let selected = ranking::select_balanced_candidates(
        &scored_candidates,
        10,
        |(candidate, _, _, _)| candidate_selection_score(candidate),
        |(candidate, _, _, _)| ranking::candidate_cluster_key(candidate),
        |(candidate, _, _, _)| candidate.memory_tier.clone(),
    );
    let mut slots = scored_candidates
        .into_iter()
        .map(Some)
        .collect::<Vec<Option<_>>>();
    let scored_candidates = selected
        .into_iter()
        .filter_map(|index| slots.get_mut(index).and_then(Option::take))
        .collect::<Vec<_>>();
    let scored_candidates = if should_refine_final_memory(source) {
        let originals = scored_candidates
            .iter()
            .map(|(candidate, _, _, _)| candidate.clone())
            .collect::<Vec<_>>();
        let (refined, refine_messages) =
            refine::refine_candidates(project_root, &originals, allow_provider_refine);
        skipped.extend(refine_messages);
        scored_candidates
            .into_iter()
            .zip(refined)
            .filter_map(|((_, score, action, _), refined)| {
                let candidate = refined?;
                let decision = evaluate_candidate_quality(&candidate, &action);
                if decision.disposition == QualityDisposition::Skip {
                    skipped.push(quality_skip_message(&draft_id(&candidate), &decision));
                    return None;
                }
                let memory_decision = memory_gate::evaluate_memory_candidate(
                    &candidate.title,
                    &candidate.body,
                    &candidate.evidence,
                    &candidate.kind,
                    &candidate.scope,
                );
                if memory_decision.disposition == memory_gate::MemoryGateDisposition::Reject {
                    skipped.push(format!(
                        "{}: memory-gate ({})",
                        draft_id(&candidate),
                        memory_decision.flags.join(",")
                    ));
                    return None;
                }
                Some((candidate, score, action, decision))
            })
            .collect::<Vec<_>>()
    } else {
        scored_candidates
    };

    let previews = scored_candidates
        .iter()
        .map(|(candidate, _score, action, decision)| {
            let chunk = chunk::EvidenceChunk {
                id: candidate.title.clone(),
                text: candidate.evidence.clone(),
                origin: chunk::ChunkOrigin::User,
                source_kind: source.to_string(),
                source_observations: Vec::new(),
            };
            let classification = classification_for_candidate(candidate, &chunk);
            let routed_action = route_action_for_classification(action, &classification);
            ExtractCandidatePreview {
                id: draft_id(candidate),
                title: candidate.title.clone(),
                body: candidate.body.clone(),
                kind: candidate.kind.clone(),
                scope: candidate.scope.clone(),
                memory_tier: candidate.memory_tier.clone(),
                abstraction_of: candidate.abstraction_of.clone(),
                abstracted_from: candidate.abstracted_from.clone(),
                evidence: format!("{source}: {}", candidate.evidence),
                confidence: candidate.confidence,
                reason: candidate.reason.clone(),
                matched_template: candidate.matched_template.clone(),
                classification: Some(classification.clone()),
                tags: classification.tags.clone(),
                suggested_action: Some(routed_action),
                operation: decision.operation.clone(),
                quality_flags: decision.flags.clone(),
            }
        })
        .collect::<Vec<_>>();
    if dry_run {
        return Ok(ExtractReport {
            created: Vec::new(),
            skipped,
            candidates: previews,
            dry_run,
            provider: "local".to_string(),
            redacted,
        });
    }

    let recurrence_candidates = scored_candidates
        .iter()
        .map(|(candidate, _, _, _)| candidate.clone())
        .collect::<Vec<_>>();
    recurrence::record_candidate_recurrence(project_root, &recurrence_candidates, source)?;
    let mut created = Vec::new();
    for (candidate, score, action, _decision) in scored_candidates {
        let id = draft_id(&candidate);
        let chunk = chunk::EvidenceChunk {
            id: candidate.title.clone(),
            text: candidate.evidence.clone(),
            origin: chunk::ChunkOrigin::User,
            source_kind: source.to_string(),
            source_observations: Vec::new(),
        };
        let mut extraction =
            extraction_metadata_for_chunk(&chunk, &score, action.record_id.clone());
        let route = extraction
            .classification
            .as_ref()
            .map(|classification| classification.artifact_kind.as_str())
            .unwrap_or("review_only");
        extraction.suggested_action = Some(route_action(action, route));
        let (memory_tier, value_scores) = build_memory_tier_metadata(&candidate);
        extraction.memory_tier = memory_tier;
        extraction.value_scores = value_scores;
        extraction.abstraction_of = candidate.abstraction_of.clone();
        extraction.abstracted_from = candidate.abstracted_from.clone();
        let result = draft::add_draft(
            project_root,
            NewDraft {
                id: id.clone(),
                title: candidate.title,
                kind: candidate.kind,
                scope: candidate.scope,
                body: candidate.body,
                targets: targets.clone(),
                evidence: format!("{source}: {}", candidate.evidence),
                confidence: candidate.confidence,
                reason: candidate.reason,
                matched_template: candidate.matched_template,
                extraction,
            },
        );
        match result {
            Ok(()) => created.push(id),
            Err(error) => skipped.push(format!("{id}: {error}")),
        }
    }
    Ok(ExtractReport {
        created,
        skipped,
        candidates: previews,
        dry_run,
        provider: "local".to_string(),
        redacted,
    })
}

pub fn extract_high_value_text_to_drafts(
    project_root: &Path,
    input: &str,
    targets: Vec<String>,
    source: &str,
    provider_name: Option<String>,
    dry_run: bool,
    max_candidates: usize,
) -> Result<ExtractReport> {
    let provider_name = provider_name.unwrap_or_else(|| {
        provider::load_or_default_provider_config(project_root)
            .map(|cfg| cfg.extraction_provider)
            .unwrap_or_else(|_| "local".to_string())
    });

    if provider_name == "local" {
        return extract_local_high_value_text_to_drafts(
            project_root,
            input,
            targets,
            source,
            dry_run,
            max_candidates,
            false,
        );
    }

    extract_llm_text_to_drafts(
        project_root,
        input,
        targets,
        source,
        &provider_name,
        dry_run,
        Some(max_candidates),
    )
}

fn extract_local_high_value_text_to_drafts(
    project_root: &Path,
    input: &str,
    targets: Vec<String>,
    source: &str,
    dry_run: bool,
    max_candidates: usize,
    allow_provider_refine: bool,
) -> Result<ExtractReport> {
    let provider_cfg = provider::load_or_default_provider_config(project_root)?;
    let redacted_input = if provider_cfg.privacy.redact_secrets {
        provider::redact_secrets(input)
    } else {
        input.to_string()
    };
    let redacted = redacted_input != input;
    let preferences = load_known_preferences(project_root)?;
    let mut candidates = extract_high_value_candidates_with_preferences(
        &redacted_input,
        &preferences,
        max_candidates,
        provider_cfg.fallback_methodology_templates,
    );
    recurrence::apply_recurrence_boost(project_root, &mut candidates)?;

    let existing_memory_cards = memory_card::load_memory_cards(project_root)?;
    let deduper = embedding::SemanticDeduper::new(0.75, 0.65);
    let mut skipped = Vec::new();
    let candidates: Vec<(
        Candidate,
        candidate::ExtractionAction,
        quality_gate::QualityGateDecision,
    )> = candidates
        .into_iter()
        .map(|candidate| {
            let scope_memory_cards = scoped_memory_cards(&existing_memory_cards, &candidate.scope);
            let action = match deduper.dedup_against_existing(&candidate.body, &scope_memory_cards)
            {
                embedding::DedupResult::Duplicate {
                    similar_id,
                    similarity,
                } => candidate::ExtractionAction::merge_into_existing(similar_id, similarity),
                embedding::DedupResult::Unique => candidate::ExtractionAction::new_candidate(),
            };
            (candidate, action)
        })
        .filter_map(|(mut candidate, action)| {
            let feedback = feedback_gate::apply_candidate_feedback(
                project_root,
                &draft_id(&candidate),
                &mut candidate,
            );
            if let Ok(Some(message)) = feedback {
                skipped.push(message);
                return None;
            } else if let Err(error) = feedback {
                skipped.push(format!("feedback: {error}"));
            }
            let decision = evaluate_candidate_quality(&candidate, &action);
            if decision.disposition == QualityDisposition::Skip {
                skipped.push(quality_skip_message(&draft_id(&candidate), &decision));
                None
            } else {
                let memory_decision = memory_gate::evaluate_memory_candidate(
                    &candidate.title,
                    &candidate.body,
                    &candidate.evidence,
                    &candidate.kind,
                    &candidate.scope,
                );
                if memory_decision.disposition == memory_gate::MemoryGateDisposition::Reject {
                    skipped.push(format!(
                        "{}: memory-gate ({})",
                        draft_id(&candidate),
                        memory_decision.flags.join(",")
                    ));
                    return None;
                }
                let action = if memory_decision.disposition
                    == memory_gate::MemoryGateDisposition::ReviewOnly
                {
                    action.with_route(memory_decision.route)
                } else {
                    action
                };
                Some((candidate, action, decision))
            }
        })
        .collect();

    let candidates = if should_refine_final_memory(source) {
        let originals = candidates
            .iter()
            .map(|(candidate, _, _)| candidate.clone())
            .collect::<Vec<_>>();
        let (refined, refine_messages) =
            refine::refine_candidates(project_root, &originals, allow_provider_refine);
        skipped.extend(refine_messages);
        candidates
            .into_iter()
            .zip(refined)
            .filter_map(|((_, action, _), refined)| {
                let candidate = refined?;
                let decision = evaluate_candidate_quality(&candidate, &action);
                if decision.disposition == QualityDisposition::Skip {
                    skipped.push(quality_skip_message(&draft_id(&candidate), &decision));
                    return None;
                }
                let memory_decision = memory_gate::evaluate_memory_candidate(
                    &candidate.title,
                    &candidate.body,
                    &candidate.evidence,
                    &candidate.kind,
                    &candidate.scope,
                );
                if memory_decision.disposition == memory_gate::MemoryGateDisposition::Reject {
                    skipped.push(format!(
                        "{}: memory-gate ({})",
                        draft_id(&candidate),
                        memory_decision.flags.join(",")
                    ));
                    return None;
                }
                Some((candidate, action, decision))
            })
            .collect::<Vec<_>>()
    } else {
        candidates
    };

    let previews = candidates
        .iter()
        .map(|(candidate, action, decision)| {
            let chunk = chunk::EvidenceChunk {
                id: candidate.title.clone(),
                text: candidate.evidence.clone(),
                origin: chunk::ChunkOrigin::User,
                source_kind: source.to_string(),
                source_observations: Vec::new(),
            };
            let classification = classification_for_candidate(candidate, &chunk);
            let routed_action = route_action_for_classification(action, &classification);
            ExtractCandidatePreview {
                id: draft_id(candidate),
                title: candidate.title.clone(),
                body: candidate.body.clone(),
                kind: candidate.kind.clone(),
                scope: candidate.scope.clone(),
                memory_tier: candidate.memory_tier.clone(),
                abstraction_of: candidate.abstraction_of.clone(),
                abstracted_from: candidate.abstracted_from.clone(),
                evidence: format!("{source}: {}", candidate.evidence),
                confidence: candidate.confidence,
                reason: candidate.reason.clone(),
                matched_template: candidate.matched_template.clone(),
                classification: Some(classification.clone()),
                tags: classification.tags.clone(),
                suggested_action: Some(routed_action),
                operation: decision.operation.clone(),
                quality_flags: decision.flags.clone(),
            }
        })
        .collect::<Vec<_>>();
    if dry_run {
        return Ok(ExtractReport {
            created: Vec::new(),
            skipped,
            candidates: previews,
            dry_run,
            provider: "local".to_string(),
            redacted,
        });
    }

    let recurrence_candidates = candidates
        .iter()
        .map(|(candidate, _, _)| candidate.clone())
        .collect::<Vec<_>>();
    recurrence::record_candidate_recurrence(project_root, &recurrence_candidates, source)?;
    let mut created = Vec::new();
    for (candidate, action, _decision) in candidates {
        let id = draft_id(&candidate);
        let chunk = chunk::EvidenceChunk {
            id: candidate.title.clone(),
            text: candidate.evidence.clone(),
            origin: chunk::ChunkOrigin::User,
            source_kind: source.to_string(),
            source_observations: Vec::new(),
        };
        let score = scoring::score_chunk(&chunk);
        let mut extraction =
            extraction_metadata_for_chunk(&chunk, &score, action.record_id.clone());
        let route = extraction
            .classification
            .as_ref()
            .map(|classification| classification.artifact_kind.as_str())
            .unwrap_or("review_only");
        extraction.suggested_action = Some(route_action(action, route));
        let (memory_tier, value_scores) = build_memory_tier_metadata(&candidate);
        extraction.memory_tier = memory_tier;
        extraction.value_scores = value_scores;
        extraction.abstraction_of = candidate.abstraction_of.clone();
        extraction.abstracted_from = candidate.abstracted_from.clone();
        let result = draft::add_draft(
            project_root,
            NewDraft {
                id: id.clone(),
                title: candidate.title,
                kind: candidate.kind,
                scope: candidate.scope,
                body: candidate.body,
                targets: targets.clone(),
                evidence: format!("{source}: {}", candidate.evidence),
                confidence: candidate.confidence,
                reason: candidate.reason,
                matched_template: candidate.matched_template,
                extraction,
            },
        );
        match result {
            Ok(()) => created.push(id),
            Err(error) => skipped.push(format!("{id}: {error}")),
        }
    }
    Ok(ExtractReport {
        created,
        skipped,
        candidates: previews,
        dry_run,
        provider: "local".to_string(),
        redacted,
    })
}

fn route_action_for_classification(
    action: &candidate::ExtractionAction,
    classification: &classify::KnowledgeClassification,
) -> candidate::ExtractionAction {
    route_action(action.clone(), &classification.artifact_kind)
}

fn should_refine_final_memory(source: &str) -> bool {
    let lower = source.to_lowercase();
    lower.contains("observation synthesis")
        || lower.contains("methodology prefilter")
        || lower.contains("gold methodology")
        || lower.contains("chunk ")
}

fn route_action(action: candidate::ExtractionAction, route: &str) -> candidate::ExtractionAction {
    if action.route == "review_only" {
        action.with_route("review_only")
    } else {
        action.with_route(route)
    }
}

fn extract_candidates_with_preferences(
    input: &str,
    preferences: &[KnownPreference],
    fallback_methodology_templates: bool,
) -> Vec<Candidate> {
    let mut candidates = Vec::new();
    for raw_sentence in split_sentences(input) {
        for sentence in split_atomic_sentences(raw_sentence) {
            let sentence = sentence.as_str();
            if let Some(candidate) = self_verification_candidate(sentence) {
                candidates.push(candidate);
                continue;
            }
            if fallback_methodology_templates {
                let methodology_candidates = methodology_pair_candidates(sentence);
                if !methodology_candidates.is_empty() {
                    candidates.extend(methodology_candidates);
                    continue;
                }
            }
            let principle_signal_candidates = principle_candidates(sentence);
            if !principle_signal_candidates.is_empty() {
                candidates.extend(principle_signal_candidates);
                continue;
            }
            if let Some(candidate) = high_value_prompt_candidate(sentence) {
                candidates.push(candidate);
                continue;
            }

            if looks_like_project_improvement_signal(sentence) {
                let body = normalize_project_improvement_body(sentence);
                if body.len() >= 28 && body.len() <= 360 {
                    candidates.push(Candidate {
                    title: title_from_project_improvement(&body),
                    body,
                    kind: "procedure".to_string(),
                    scope: infer_scope(sentence).to_string(),
                    memory_tier: MemoryTier::ProjectRule,
                    abstraction_of: None,
                    abstracted_from: None,
                    evidence: sentence.to_string(),
                    confidence: Some(0.82),
                    reason: Some(
                        "Matched high-value project improvement record: reusable fix, decision, or execution pattern."
                            .to_string(),
                    ),
                    matched_template: Some("project-improvement".to_string()),
                });
                }
                continue;
            }

            if let Some(candidate) = normalize_known_preference(sentence, preferences) {
                candidates.push(candidate);
                continue;
            }
            if let Some(candidate) = atomic_exception_candidate(sentence) {
                candidates.push(candidate);
                continue;
            }
            if let Some(candidate) = scored_signal_candidate(sentence, chunk::ChunkOrigin::User) {
                candidates.push(candidate);
                continue;
            }
            if !looks_like_rule(sentence) {
                continue;
            }
            let body = normalize_body(sentence);
            if body.len() < 12 {
                continue;
            }
            let title = title_from_body(&body);
            candidates.push(Candidate {
                title,
                body,
                kind: classify_kind(sentence).to_string(),
                scope: "project".to_string(),
                memory_tier: MemoryTier::ProjectRule,
                abstraction_of: None,
                abstracted_from: None,
                evidence: sentence.to_string(),
                confidence: Some(0.62),
                reason: Some("Matched local rule-like sentence heuristic.".to_string()),
                matched_template: None,
            });
        }
    }
    dedupe_candidates(candidates)
}

fn extract_high_value_candidates_with_preferences(
    input: &str,
    preferences: &[KnownPreference],
    max_candidates: usize,
    fallback_methodology_templates: bool,
) -> Vec<Candidate> {
    let mut candidates = Vec::new();
    let mut weak_counts = std::collections::BTreeMap::<String, usize>::new();
    let mut weak_candidates = std::collections::BTreeMap::<String, Candidate>::new();

    for raw_sentence in split_sentences(input) {
        for sentence in split_atomic_sentences(raw_sentence) {
            let sentence = sentence.as_str();
            if let Some(candidate) = self_verification_candidate(sentence) {
                candidates.push(candidate);
                continue;
            }
            if fallback_methodology_templates {
                let methodology_candidates = methodology_pair_candidates(sentence);
                if !methodology_candidates.is_empty() {
                    candidates.extend(methodology_candidates);
                    continue;
                }
            }
            let principle_signal_candidates = principle_candidates(sentence);
            if !principle_signal_candidates.is_empty() {
                candidates.extend(principle_signal_candidates);
                continue;
            }
            if let Some(candidate) = high_value_prompt_candidate(sentence) {
                candidates.push(candidate);
                continue;
            }

            if looks_like_project_improvement_signal(sentence) {
                let body = normalize_project_improvement_body(sentence);
                if body.len() >= 28 && body.len() <= 360 {
                    candidates.push(Candidate {
                    title: title_from_project_improvement(&body),
                    body,
                    kind: "procedure".to_string(),
                    scope: infer_scope(sentence).to_string(),
                    memory_tier: MemoryTier::ProjectRule,
                    abstraction_of: None,
                    abstracted_from: None,
                    evidence: sentence.to_string(),
                    confidence: Some(0.82),
                    reason: Some(
                        "Matched high-value project improvement record: reusable fix, decision, or execution pattern."
                            .to_string(),
                    ),
                    matched_template: Some("project-improvement".to_string()),
                });
                }
                continue;
            }

            if let Some(candidate) = normalize_known_preference(sentence, preferences) {
                candidates.push(candidate);
                continue;
            }
            if let Some(candidate) = atomic_exception_candidate(sentence) {
                candidates.push(candidate);
                continue;
            }
            if let Some(candidate) = scored_signal_candidate(sentence, chunk::ChunkOrigin::User) {
                candidates.push(candidate);
                continue;
            }
            if !looks_like_rule(sentence) {
                continue;
            }
            if !looks_like_memory_card_signal(sentence) {
                continue;
            }
            let body = normalize_body(sentence);
            if body.len() < 18 || body.len() > 220 {
                continue;
            }
            let key = body.to_lowercase();
            *weak_counts.entry(key.clone()).or_default() += 1;
            weak_candidates.entry(key).or_insert_with(|| {
                Candidate {
            title: title_from_body(&body),
            body,
            kind: classify_kind(sentence).to_string(),
            scope: infer_scope(sentence).to_string(),
            memory_tier: MemoryTier::ProjectRule,
            abstraction_of: None,
            abstracted_from: None,
            evidence: sentence.to_string(),
            confidence: Some(0.78),
            reason: Some(
                "Matched high-value MemoryCard signal: durable preference, constraint, or workflow."
                    .to_string(),
            ),
            matched_template: None,
        }
            });
        }
    }

    for (key, count) in weak_counts {
        let Some(candidate) = weak_candidates.remove(&key) else {
            continue;
        };
        if count >= 2 || has_explicit_memory_marker(&candidate.evidence) {
            candidates.push(candidate);
        }
    }

    let deduped = dedupe_candidates(candidates);
    let selected = ranking::select_balanced_candidates(
        &deduped,
        max_candidates,
        candidate_selection_score,
        ranking::candidate_cluster_key,
        |candidate| candidate.memory_tier.clone(),
    );
    selected
        .into_iter()
        .filter_map(|index| deduped.get(index).cloned())
        .collect()
}
