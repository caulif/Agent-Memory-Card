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
pub use quality::{
    CardQualityFailure, CardQualityInput, CardQualityReport, CardQualityScores, QualityReport,
    QualityTextCase, quality_report_for_card, quality_report_for_crystallized_card,
    quality_report_for_text_cases,
};

use atomic::split_atomic_sentences;
use candidate_factory::{
    atomic_exception_candidate, classify_kind, delivery_acceptance_candidate, draft_id,
    existing_flow_planning_candidate, extraction_metadata_for_chunk, failure_flow_candidates,
    global_flow_candidates, high_value_prompt_candidate, infer_scope,
    local_only_golden_set_candidate, looks_like_memory_card_signal, looks_like_rule,
    normalize_body, normalize_project_improvement_body,
    parallel_agent_github_coordination_candidate, planning_deduplication_candidate,
    principle_candidates, project_startup_collaboration_candidate, scored_signal_candidate,
    self_verification_candidate, speed_validation_cadence_candidate, title_from_body,
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
    candidates.extend(failure_flow_candidates(input));
    candidates.extend(global_flow_candidates(input));
    if is_structured_flow_material(input) {
        return dedupe_candidates(candidates);
    }
    if should_try_whole_input_candidate(input)
        && let Some(candidate) = project_startup_collaboration_candidate(input)
    {
        candidates.push(candidate);
    }
    for raw_sentence in split_sentences(input) {
        for sentence in split_atomic_sentences(raw_sentence) {
            let sentence = sentence.as_str();
            if let Some(candidate) = project_startup_collaboration_candidate(sentence) {
                candidates.push(candidate);
                continue;
            }
            if let Some(candidate) = speed_validation_cadence_candidate(sentence) {
                candidates.push(candidate);
                continue;
            }
            if let Some(candidate) = existing_flow_planning_candidate(sentence) {
                candidates.push(candidate);
                continue;
            }
            if let Some(candidate) = delivery_acceptance_candidate(sentence) {
                candidates.push(candidate);
                continue;
            }
            if let Some(candidate) = planning_deduplication_candidate(sentence) {
                candidates.push(candidate);
                continue;
            }
            if let Some(candidate) = parallel_agent_github_coordination_candidate(sentence) {
                candidates.push(candidate);
                continue;
            }
            if let Some(candidate) = local_only_golden_set_candidate(sentence) {
                candidates.push(candidate);
                continue;
            }
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
    candidates.extend(failure_flow_candidates(input));
    candidates.extend(global_flow_candidates(input));
    if is_structured_flow_material(input) {
        let deduped = dedupe_candidates(candidates);
        let selected = ranking::select_balanced_candidates(
            &deduped,
            max_candidates,
            candidate_selection_score,
            ranking::candidate_cluster_key,
            |candidate| candidate.memory_tier.clone(),
        );
        return selected
            .into_iter()
            .filter_map(|index| deduped.get(index).cloned())
            .collect();
    }
    if should_try_whole_input_candidate(input)
        && let Some(candidate) = project_startup_collaboration_candidate(input)
    {
        candidates.push(candidate);
    }
    let mut weak_counts = std::collections::BTreeMap::<String, usize>::new();
    let mut weak_candidates = std::collections::BTreeMap::<String, Candidate>::new();

    for raw_sentence in split_sentences(input) {
        for sentence in split_atomic_sentences(raw_sentence) {
            let sentence = sentence.as_str();
            if let Some(candidate) = project_startup_collaboration_candidate(sentence) {
                candidates.push(candidate);
                continue;
            }
            if let Some(candidate) = speed_validation_cadence_candidate(sentence) {
                candidates.push(candidate);
                continue;
            }
            if let Some(candidate) = existing_flow_planning_candidate(sentence) {
                candidates.push(candidate);
                continue;
            }
            if let Some(candidate) = delivery_acceptance_candidate(sentence) {
                candidates.push(candidate);
                continue;
            }
            if let Some(candidate) = planning_deduplication_candidate(sentence) {
                candidates.push(candidate);
                continue;
            }
            if let Some(candidate) = parallel_agent_github_coordination_candidate(sentence) {
                candidates.push(candidate);
                continue;
            }
            if let Some(candidate) = local_only_golden_set_candidate(sentence) {
                candidates.push(candidate);
                continue;
            }
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

fn should_try_whole_input_candidate(input: &str) -> bool {
    input.lines().filter(|line| !line.trim().is_empty()).count() <= 2 && input.len() <= 700
}

fn is_structured_flow_material(input: &str) -> bool {
    input.contains("ConversationFlowSummary:") || input.contains("FailureFlowSummary:")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn high_value_extraction_rejects_one_off_copy_release_and_generic_ux_requests() {
        let temp = tempfile::tempdir().expect("tempdir");
        let report = extract_high_value_text_to_drafts(
                temp.path(),
                "请给这个开源项目写一篇社区宣传帖，不要太长，去除 AI 味道，不要写得太生硬，有人的味道和情绪表达一点。\nREADME 里删掉当前范围、技术栈和路线提醒，重新组织内容，然后打 v0.1.0 release。\n全面分析一下现有项目的功能上还有什么不足或者可以优化用户体验的地方，告诉我。",
                vec!["codex".to_string()],
                "observation synthesis, chunk 1/1",
                Some("local".to_string()),
                true,
                8,
            )
            .expect("extract");

        assert!(
            report.candidates.is_empty(),
            "one-off copywriting, release, README, and generic UX analysis requests should not become MemoryCard drafts: {:?}",
            report.candidates
        );
    }

    #[test]
    fn high_value_extraction_rejects_isolated_aesthetic_fragments() {
        let temp = tempfile::tempdir().expect("tempdir");
        let report = extract_high_value_text_to_drafts(
            temp.path(),
            "保留例外：不要太丑，极简也可以很美。",
            vec!["codex".to_string()],
            "observation synthesis, chunk 1/1",
            Some("local".to_string()),
            true,
            8,
        )
        .expect("extract");

        assert!(
            report.candidates.is_empty(),
            "isolated aesthetic fragments without an operational trigger should not become MemoryCard drafts: {:?}",
            report.candidates
        );
    }

    #[test]
    fn high_value_extraction_keeps_project_startup_visual_collaboration_preference() {
        let temp = tempfile::tempdir().expect("tempdir");
        let report = extract_high_value_text_to_drafts(
            temp.path(),
            "有些内容可能用浏览器里的可视化更容易讨论，比如弹窗、标注、设置页布局和架构图。我可以边聊边做轻量 mockup、对比图或流程图给你看。用户同意使用这个方式。",
            vec!["codex".to_string()],
            "observation synthesis, chunk 1/1",
            Some("local".to_string()),
            true,
            8,
        )
        .expect("extract");

        assert!(
            report.candidates.iter().any(|candidate| {
                candidate.memory_tier == MemoryTier::CollaborationPreference
                    && candidate.body.contains("可视化")
                    && candidate.body.contains("mockup")
                    && candidate.body.contains("流程图")
            }),
            "visual project-startup collaboration preference should survive as an actionable rule: {:?}; skipped={:?}",
            report.candidates,
            report.skipped
        );
    }

    #[test]
    fn high_value_extraction_keeps_project_startup_reference_research_preference() {
        let temp = tempfile::tempdir().expect("tempdir");
        let report = extract_high_value_text_to_drafts(
            temp.path(),
            "项目启动规划时，如果有可以借鉴的开源项目、同类产品或者任何相关内容，也可以先参考借鉴，再规划方案。",
            vec!["codex".to_string()],
            "observation synthesis, chunk 1/1",
            Some("local".to_string()),
            true,
            8,
        )
        .expect("extract");

        assert!(
            report.candidates.iter().any(|candidate| {
                candidate.memory_tier == MemoryTier::CollaborationPreference
                    && candidate.body.contains("开源项目")
                    && candidate.body.contains("同类产品")
                    && candidate.body.contains("规划")
            }),
            "reference-research planning preference should not be treated as one-off open-source publishing work: {:?}; skipped={:?}",
            report.candidates,
            report.skipped
        );
    }

    #[test]
    fn high_value_extraction_rewrites_speed_validation_cadence_preference() {
        let temp = tempfile::tempdir().expect("tempdir");
        let report = extract_high_value_text_to_drafts(
            temp.path(),
            "我确认，同时我希望迅速开发，不要过多检查/测试浪费时间，必要时测试/检查就行，然后最后全部完成前进行一次总的测试/检查就行。",
            vec!["codex".to_string()],
            "observation synthesis, chunk 1/1",
            Some("local".to_string()),
            true,
            8,
        )
        .expect("extract");

        assert!(
            report.candidates.iter().any(|candidate| {
                candidate.memory_tier == MemoryTier::CollaborationPreference
                    && candidate.body.contains("小改")
                    && candidate.body.contains("必要检查")
                    && candidate.body.contains("完整测试")
                    && !candidate.body.contains("浪费时间")
            }),
            "speed/validation cadence should become a durable workflow preference, not a raw complaint: {:?}; skipped={:?}",
            report.candidates,
            report.skipped
        );
    }

    #[test]
    fn high_value_extraction_keeps_stage_shaped_workflow_preferences() {
        let temp = tempfile::tempdir().expect("tempdir");
        let report = extract_high_value_text_to_drafts(
            temp.path(),
            "项目启动阶段：遇到大型或模糊任务，先调研可借鉴的开源项目和同类产品，明确定位和核心功能，再制定计划。\n开发中段：改提炼、Memory Card 或 UI 流程前，先理解现有项目和既有流程，再提出改动方案。\n交付阶段：绿色测试只是证据之一，最终还要从用户实际感知和需求覆盖判断质量。\n规划阶段：使用 GitHub 或计划文档时，注意已经规划了的内容就不要重复规划了。\n评估阶段：Golden Set 只用于本地测试，不要把我自己的对话数据上传 Git，链路里面不应该用真实数据。",
            vec!["codex".to_string()],
            "observation synthesis, chunk 1/1",
            Some("local".to_string()),
            true,
            10,
        )
        .expect("extract");

        assert!(
            report.candidates.iter().any(|candidate| {
                candidate.body.contains("开源项目") && candidate.body.contains("同类产品")
            }),
            "startup research preference should be extracted: {:?}; skipped={:?}",
            report.candidates,
            report.skipped
        );
        assert!(
            report.candidates.iter().any(|candidate| {
                candidate.body.contains("先理解现有项目") && candidate.body.contains("既有流程")
            }),
            "mid-development existing-flow preference should be extracted: {:?}; skipped={:?}",
            report.candidates,
            report.skipped
        );
        assert!(
            report.candidates.iter().any(|candidate| {
                candidate.body.contains("绿色测试") && candidate.body.contains("用户实际感知")
            }),
            "delivery acceptance preference should be extracted: {:?}; skipped={:?}",
            report.candidates,
            report.skipped
        );
        assert!(
            report.candidates.iter().any(|candidate| {
                candidate.body.contains("避免重复规划") && candidate.body.contains("已有规划")
            }),
            "planning deduplication preference should be normalized: {:?}; skipped={:?}",
            report.candidates,
            report.skipped
        );
        assert!(
            report.candidates.iter().any(|candidate| {
                candidate.body.contains("Golden Set")
                    && candidate.body.contains("本地效果评估")
                    && candidate.body.contains("不要把真实对话数据")
            }),
            "local-only Golden Set privacy boundary should be normalized: {:?}; skipped={:?}",
            report.candidates,
            report.skipped
        );
    }

    #[test]
    fn high_value_extraction_normalizes_parallel_agent_github_coordination() {
        let temp = tempfile::tempdir().expect("tempdir");
        let report = extract_high_value_text_to_drafts(
            temp.path(),
            "我现在有另一个agent的github上进行修改，注意不要重复，此外你也可以用github，但是注意已经规划了的内容就不要重复规划了。",
            vec!["codex".to_string()],
            "observation synthesis, chunk 1/1",
            Some("local".to_string()),
            true,
            8,
        )
        .expect("extract");

        assert!(
            report.candidates.iter().any(|candidate| {
                candidate.body.contains("多 agent")
                    && candidate.body.contains("Issue")
                    && candidate.body.contains("避免重复规划")
            }),
            "parallel GitHub agent coordination should be normalized: {:?}; skipped={:?}",
            report.candidates,
            report.skipped
        );
        assert!(
            report
                .candidates
                .iter()
                .all(|candidate| !candidate.body.contains("我现在有另一个agent")),
            "raw user fragment should not be emitted as the card body: {:?}",
            report.candidates
        );
    }

    #[test]
    fn high_value_extraction_rejects_goal_execution_and_phase_choice_noise() {
        let temp = tempfile::tempdir().expect("tempdir");
        let report = extract_high_value_text_to_drafts(
            temp.path(),
            "/goal 使用Subagent-Driven模式，实现现有计划的所有功能，需要时可以搜索现有开源项目进行学习和借鉴其成熟思路和架构，完成M0之后进行简单测试，同时给出命令，让我自己运行测试一下。\n我同意，优先做 A/B 的基础体验，再用 C 做增强。\n/goal 接下来用这个spec_driven_develop这个skills结合github进行规划和实现，直到完成。\n真实历史 dry-run top 10 至少 3 条是合理候选。",
            vec!["codex".to_string()],
            "observation synthesis, chunk 1/1",
            Some("local".to_string()),
            true,
            8,
        )
        .expect("extract");

        assert!(
            report.candidates.is_empty(),
            "one-off /goal execution directives and local phase choices should not become global cards: {:?}; skipped={:?}",
            report.candidates,
            report.skipped
        );
    }

    #[test]
    fn high_value_extraction_rejects_code_analysis_noise() {
        let temp = tempfile::tempdir().expect("tempdir");
        let report = extract_high_value_text_to_drafts(
            temp.path(),
            "**噪声过滤细致** — `extract.rs` 中的信号检测函数（`looks_like_rule`、`looks_like_skilllet_signal`）层层递进，有效地把一次性请求和持久规则区分开。",
            vec!["codex".to_string()],
            "observation synthesis, chunk 1/1",
            Some("local".to_string()),
            true,
            8,
        )
        .expect("extract");

        assert!(
            report.candidates.is_empty(),
            "code analysis praise should not become a project-improvement card: {report:#?}"
        );
    }

    #[test]
    fn high_value_extraction_rejects_code_analysis_validation_signal() {
        let temp = tempfile::tempdir().expect("tempdir");
        let report = extract_high_value_text_to_drafts(
            temp.path(),
            "**噪声过滤细致** — `extract.rs` 中的信号检测函数（`looks_like_rule`、`looks_like_skilllet_signal`、`is_low_value_task_sentence`、`looks_like_unresolved_user_request`）层层递进，有效地把\"继续优化UI\"这种一次性求和\"所有Rust项目必须运行cargo test\"这种持久规则区分开",
            vec!["codex".to_string()],
            "observation synthesis, chunk 1/1",
            Some("local".to_string()),
            true,
            8,
        )
        .expect("extract");

        assert!(
            report.candidates.is_empty(),
            "code analysis validation signal should be filtered: {report:#?}"
        );
    }

    #[test]
    fn high_value_extraction_rejects_extraction_taxonomy_artifacts() {
        let temp = tempfile::tempdir().expect("tempdir");
        let report = extract_high_value_text_to_drafts(
            temp.path(),
            "- 优先保留稳定偏好、流程、约束、质量标准、回归方法、审阅边界",
            vec!["codex".to_string()],
            "observation synthesis, chunk 1/1",
            Some("local".to_string()),
            true,
            8,
        )
        .expect("extract");

        assert!(
            report.candidates.is_empty(),
            "extraction taxonomy bullet should be filtered: {report:#?}"
        );
    }

    #[test]
    fn high_value_extraction_surfaces_failure_flow_pipeline_break_rule() {
        let temp = tempfile::tempdir().expect("tempdir");
        let input = "FailureFlowSummary:\n- signal:pipeline_break obs:a text:LLM induction JSON 被截断，解析失败，所以没有进入最终 crystallize 卡片阶段。\n- signal:final_quality_correction obs:b text:不能只看指标，要自己看最终卡片质量。\n";

        let report = extract_high_value_text_to_drafts(
            temp.path(),
            input,
            vec!["codex".to_string()],
            "failure flow prefilter",
            Some("local".to_string()),
            true,
            8,
        )
        .expect("extract");

        assert!(
            report
                .candidates
                .iter()
                .any(|candidate| candidate.matched_template.as_deref()
                    == Some("failure-flow:pipeline-break")),
            "{report:#?}"
        );
        assert!(
            report
                .candidates
                .iter()
                .any(|candidate| candidate.body.contains("先定位链路断点")),
            "{report:#?}"
        );
    }

    #[test]
    fn high_value_extraction_surfaces_failure_flow_boundary_rules() {
        let temp = tempfile::tempdir().expect("tempdir");
        let input = "FailureFlowSummary:\n- signal:false_positive_noise obs:a text:代码分析和抽取 taxonomy 不应该计入长期记忆。\n- signal:privacy_boundary obs:b text:真实历史 dry-run 只用于本地评估，不进入 Git 或 Golden Set。\n- signal:scope_boundary obs:c text:项目记忆和全局记忆要分层，避免局部流程污染全局记忆。\n";

        let report = extract_high_value_text_to_drafts(
            temp.path(),
            input,
            vec!["codex".to_string()],
            "failure flow prefilter",
            Some("local".to_string()),
            true,
            8,
        )
        .expect("extract");

        for template in [
            "failure-flow:false-positive-abstraction",
            "failure-flow:local-only-regression",
            "failure-flow:scope-boundary",
        ] {
            assert!(
                report
                    .candidates
                    .iter()
                    .any(|candidate| candidate.matched_template.as_deref() == Some(template)),
                "{template}: {report:#?}"
            );
        }
    }

    #[test]
    fn high_value_extraction_canonicalizes_design_stage_shorthand() {
        let temp = tempfile::tempdir().expect("tempdir");
        let report = extract_high_value_text_to_drafts(
            temp.path(),
            "设计阶段：先提问、先澄清目标、先规划、从用户视角看。",
            vec!["codex".to_string()],
            "test",
            Some("local".to_string()),
            true,
            8,
        )
        .expect("extract");

        assert!(
            report.candidates.iter().any(|candidate| {
                candidate.scope == "global"
                    && candidate.body
                        == "设计阶段先提问、先澄清目标、先规划，并从用户视角检查方案。"
            }),
            "{report:#?}"
        );
    }

    #[test]
    fn high_value_extraction_surfaces_transferable_failure_lessons() {
        let temp = tempfile::tempdir().expect("tempdir");
        let input = "FailureFlowSummary:\n- signal:refactor_correction obs:a text:我有几次重构和纠偏，这些都可以吸取经验，形成高质量工作流里的模块。\n- signal:transferable_workflow obs:b text:项目特殊偏好也可以提取或修改为全局偏好，迁移到其他项目或放进自己的工作流。\n- signal:review_iterate_loop obs:c text:测试时必须看真实数据生成了什么记忆卡片，结合目标审核，分析问题和解决方案，修改优化直到符合预期。\n";

        let report = extract_high_value_text_to_drafts(
            temp.path(),
            input,
            vec!["codex".to_string()],
            "failure flow prefilter",
            Some("local".to_string()),
            true,
            8,
        )
        .expect("extract");

        for template in [
            "failure-flow:refactor-lessons",
            "failure-flow:transferable-workflow",
            "failure-flow:review-iterate-loop",
        ] {
            assert!(
                report
                    .candidates
                    .iter()
                    .any(|candidate| candidate.matched_template.as_deref() == Some(template)),
                "{template}: {report:#?}"
            );
        }
    }
}
