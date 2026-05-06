use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};

use crate::candidate;
use crate::draft::{self, NewDraft};
use crate::provider;
use crate::skilllet;
use crate::textutil;

mod atomic;
mod candidate_factory;
pub mod chunk;
pub mod classify;
mod dedupe;
pub(crate) mod embedding;
pub mod gate;
pub mod lifecycle;
mod llm;
mod preference;
pub mod quality;
mod quality_gate;
mod recurrence;
pub mod scoring;
mod signals;

pub use preference::{
    PreferenceRegistryValidationReport, PreferenceTemplatePreview, PreferenceTestMatch,
    PreferenceTestReport, init_preference_registry, preference_templates, test_preference_text,
    validate_preference_registry,
};
pub use quality::{QualityReport, QualityTextCase, quality_report_for_text_cases};

use atomic::split_atomic_sentences;
use candidate_factory::{
    atomic_exception_candidate, classify_kind, draft_id, extraction_metadata_for_chunk,
    high_value_prompt_candidate, infer_scope, looks_like_rule, looks_like_skilllet_signal,
    normalize_body, normalize_project_improvement_body, scored_signal_candidate, title_from_body,
    title_from_project_improvement,
};
use dedupe::dedupe_candidates;
#[cfg(test)]
use preference::built_in_preferences;
use preference::{KnownPreference, load_known_preferences, normalize_known_preference};
use quality_gate::{QualityDisposition, evaluate_candidate_quality, quality_skip_message};
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

impl ExtractReport {
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("Agent-Kernel extract report\n\n");
        out.push_str(&format!("Provider: {}\n\n", self.provider));
        if self.redacted {
            out.push_str("Secrets: redacted\n\n");
        }
        if self.created.is_empty() {
            if self.dry_run && !self.candidates.is_empty() {
                out.push_str("Draft candidates:\n");
                for candidate in &self.candidates {
                    out.push_str(&format!("- {}: {}\n", candidate.id, candidate.body));
                    if let Some(confidence) = candidate.confidence {
                        out.push_str(&format!("  confidence: {:.0}%\n", confidence * 100.0));
                    }
                    if let Some(template) = candidate.matched_template.as_deref() {
                        out.push_str(&format!("  matched_template: {template}\n"));
                    }
                    if let Some(reason) = candidate.reason.as_deref() {
                        out.push_str(&format!("  reason: {reason}\n"));
                    }
                    if let Some(classification) = candidate.classification.as_ref() {
                        out.push_str(&format!(
                            "  classification: signal={}, artifact={}, hardness={}, activation={}\n",
                            classification.signal,
                            classification.artifact_kind,
                            classification.hardness,
                            classification.activation,
                        ));
                    }
                    if !candidate.tags.is_empty() {
                        out.push_str(&format!("  tags: {}\n", candidate.tags.join(", ")));
                    }
                }
            } else {
                out.push_str("No drafts created.\n");
            }
        } else {
            out.push_str("Drafts created:\n");
            for id in &self.created {
                out.push_str(&format!("- {id}\n"));
            }
        }
        if !self.skipped.is_empty() {
            out.push_str("\nSkipped:\n");
            for item in &self.skipped {
                out.push_str(&format!("- {item}\n"));
            }
        }
        out
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ExtractCandidatePreview {
    pub id: String,
    pub title: String,
    pub body: String,
    pub kind: String,
    pub scope: String,
    pub evidence: String,
    pub confidence: Option<f32>,
    pub reason: Option<String>,
    pub matched_template: Option<String>,
    pub classification: Option<classify::KnowledgeClassification>,
    pub tags: Vec<String>,
    pub suggested_action: Option<candidate::ExtractionAction>,
    pub operation: lifecycle::SkillletOperation,
    pub quality_flags: Vec<String>,
}

#[derive(Debug, Clone)]
pub(super) struct Candidate {
    title: String,
    body: String,
    kind: String,
    scope: String,
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
            .map(|cfg| cfg.default)
            .unwrap_or_else(|_| "local".to_string())
    });

    if provider_name == "local" {
        return extract_local_text_to_drafts(project_root, input, targets, source, dry_run);
    }

    // LLM 引擎路径
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

/// 本地正则提取引擎（保持向后兼容）
fn extract_local_text_to_drafts(
    project_root: &Path,
    input: &str,
    targets: Vec<String>,
    source: &str,
    dry_run: bool,
) -> Result<ExtractReport> {
    let provider_cfg = provider::load_or_default_provider_config(project_root)?;
    let redacted_input = if provider_cfg.privacy.redact_secrets {
        provider::redact_secrets(input)
    } else {
        input.to_string()
    };
    let redacted = redacted_input != input;
    let preferences = load_known_preferences(project_root)?;
    let mut candidates = extract_candidates_with_preferences(&redacted_input, &preferences);
    recurrence::apply_recurrence_boost(project_root, &mut candidates)?;

    // 对已有 skilllet 做语义比对。重复内容保留为合并建议，而不是静默丢弃。
    let existing_skilllets = skilllet::load_skilllets(project_root)?;
    let deduper = embedding::SemanticDeduper::new(0.75, 0.65);
    let mut skipped = Vec::new();
    let candidates: Vec<(Candidate, candidate::ExtractionAction)> = candidates
        .into_iter()
        .map(|candidate| {
            let action = match deduper.dedup_against_existing(&candidate.body, &existing_skilllets)
            {
                embedding::DedupResult::Duplicate {
                    similar_id,
                    similarity,
                } => candidate::ExtractionAction::merge_into_existing(similar_id, similarity),
                embedding::DedupResult::Unique => candidate::ExtractionAction::new_candidate(),
            };
            (candidate, action)
        })
        .filter_map(|(candidate, action)| {
            let decision = evaluate_candidate_quality(&candidate, &action);
            if decision.disposition == QualityDisposition::Skip {
                skipped.push(quality_skip_message(&draft_id(&candidate), &decision));
                None
            } else {
                Some((candidate, action))
            }
        })
        .collect();

    // 应用质量评分门控：将每个候选转为 EvidenceChunk（使用原始证据文本），评分，仅保留 Candidate 级别
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
            || candidate.matched_template.as_deref() == Some("atomic-exception")
        {
            scored_candidates.push((candidate, score, action, decision));
        }
    }
    // 按 confidence 降序排序，截断到 10 条
    scored_candidates.sort_by(|(a, _, _, _), (b, _, _, _)| {
        b.confidence
            .unwrap_or(0.0)
            .partial_cmp(&a.confidence.unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored_candidates.truncate(10);

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
            let classification = classify::classify_chunk(&chunk);
            let routed_action = action.clone().with_route(&classification.artifact_kind);
            ExtractCandidatePreview {
                id: draft_id(candidate),
                title: candidate.title.clone(),
                body: candidate.body.clone(),
                kind: candidate.kind.clone(),
                scope: candidate.scope.clone(),
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
        extraction.suggested_action = Some(action.with_route(route));
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

/// LLM 驱动提取引擎
fn extract_llm_text_to_drafts(
    project_root: &Path,
    input: &str,
    targets: Vec<String>,
    source: &str,
    provider_name: &str,
    dry_run: bool,
    max_candidates: Option<usize>,
) -> Result<ExtractReport> {
    let provider_cfg = provider::load_or_default_provider_config(project_root)?;
    let redacted_input = if provider_cfg.privacy.redact_secrets {
        provider::redact_secrets(input)
    } else {
        input.to_string()
    };
    let redacted = redacted_input != input;

    // 第一层+第二层：过滤噪音，检测候选段落
    let paragraphs = signals::split_into_paragraphs(&redacted_input);
    let candidate_paragraphs = signals::detect_candidate_paragraphs(&paragraphs);

    if candidate_paragraphs.is_empty() {
        return Ok(ExtractReport {
            created: Vec::new(),
            skipped: vec!["No candidate paragraphs detected.".to_string()],
            candidates: Vec::new(),
            dry_run,
            provider: provider_name.to_string(),
            redacted,
        });
    }

    // 第三层：LLM 提取（分批）
    let max_per_batch = provider_cfg.max_candidates_per_batch;
    let min_confidence = provider_cfg.min_confidence;
    let mut all_knowledge = Vec::new();

    for batch in candidate_paragraphs.chunks(max_per_batch) {
        match llm::run_llm_extraction(project_root, batch, 2048) {
            Ok(items) => all_knowledge.extend(items),
            Err(e) => {
                // LLM 提取失败时跳过该批次
                return Ok(ExtractReport {
                    created: Vec::new(),
                    skipped: vec![format!("LLM extraction failed: {e}")],
                    candidates: Vec::new(),
                    dry_run,
                    provider: provider_name.to_string(),
                    redacted,
                });
            }
        }
    }

    // 筛选可用知识
    let usable = llm::filter_usable_knowledge(all_knowledge, min_confidence);

    // 第四层：语义去重
    let skilllets = skilllet::load_skilllets(project_root)?;
    let deduper = embedding::SemanticDeduper::new(0.75, 0.65);
    let mut deduped_items: Vec<embedding::LlmKnowledgeItem> = Vec::new();
    for item in usable {
        if item.is_noise {
            continue;
        }
        let suggested_action = match deduper.dedup_against_existing(&item.body, &skilllets) {
            embedding::DedupResult::Duplicate {
                similar_id,
                similarity,
            } => candidate::ExtractionAction::merge_into_existing(similar_id, similarity),
            embedding::DedupResult::Unique => candidate::ExtractionAction::new_candidate(),
        };
        deduped_items.push(embedding::LlmKnowledgeItem {
            title: item.title.clone(),
            body: item.body.clone(),
            kind: llm::knowledge_kind_to_str(&item.kind).to_string(),
            scope: "project".to_string(),
            confidence: item.confidence,
            evidence: format!("{source}: LLM extraction"),
            reason: item.rationale.clone(),
            matched_signal: format!("{:?}", item.kind),
            is_noise: item.is_noise,
            suggested_action,
        });
    }

    // 同批次去重
    let retained = deduper.dedup_within_batch(&mut deduped_items);

    // 按 confidence 排序
    let mut final_items: Vec<&embedding::LlmKnowledgeItem> =
        retained.iter().map(|&i| &deduped_items[i]).collect();
    final_items.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // 应用 max_candidates 限制
    if let Some(limit) = max_candidates {
        final_items.truncate(limit);
    }

    // 应用质量评分门控：将 LLM 输出转为 EvidenceChunk（assistant 来源），评分，仅保留 Candidate 级别
    let mut skipped = Vec::new();
    let mut scored_items: Vec<(
        &embedding::LlmKnowledgeItem,
        scoring::ExtractionScore,
        quality_gate::QualityGateDecision,
    )> = Vec::new();
    for item in &final_items {
        let candidate = Candidate {
            title: item.title.clone(),
            body: item.body.clone(),
            kind: item.kind.clone(),
            scope: item.scope.clone(),
            evidence: item.evidence.clone(),
            confidence: Some(item.confidence),
            reason: Some(item.reason.clone()),
            matched_template: Some(item.matched_signal.clone()),
        };
        let decision = evaluate_candidate_quality(&candidate, &item.suggested_action);
        if decision.disposition == QualityDisposition::Skip {
            skipped.push(quality_skip_message(
                &format!("project:{}", textutil::slug(&item.title)),
                &decision,
            ));
            continue;
        }
        let chunk = chunk::EvidenceChunk {
            id: item.title.clone(),
            text: item.body.clone(),
            origin: chunk::ChunkOrigin::Assistant,
            source_kind: source.to_string(),
            source_observations: Vec::new(),
        };
        let score = scoring::score_chunk(&chunk);
        if score.disposition == scoring::ExtractionDisposition::Candidate {
            scored_items.push((item, score, decision));
        }
    }
    // 按 confidence 降序排序，截断到 10 条
    scored_items.sort_by(|(a, _, _), (b, _, _)| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored_items.truncate(10);

    // 生成预览
    let previews = scored_items
        .iter()
        .map(|(item, score, decision)| {
            let chunk = chunk::EvidenceChunk {
                id: item.title.clone(),
                text: item.body.clone(),
                origin: chunk::ChunkOrigin::Assistant,
                source_kind: source.to_string(),
                source_observations: Vec::new(),
            };
            let classification = classify::classify_chunk(&chunk);
            let routed_action = item
                .suggested_action
                .clone()
                .with_route(&classification.artifact_kind);
            ExtractCandidatePreview {
                id: format!("project:{}", textutil::slug(&item.title)),
                title: item.title.clone(),
                body: item.body.clone(),
                kind: item.kind.clone(),
                scope: item.scope.clone(),
                evidence: item.evidence.clone(),
                confidence: Some(item.confidence),
                reason: Some(score.reason.clone()),
                matched_template: Some(item.matched_signal.clone()),
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
            provider: provider_name.to_string(),
            redacted,
        });
    }

    // 写入 Candidate（附带提取元数据）
    let mut created = Vec::new();
    for (item, score, _decision) in scored_items {
        let id = format!("project:{}", textutil::slug(&item.title));
        let chunk = chunk::EvidenceChunk {
            id: item.title.clone(),
            text: item.body.clone(),
            origin: chunk::ChunkOrigin::Assistant,
            source_kind: source.to_string(),
            source_observations: Vec::new(),
        };
        let mut extraction =
            extraction_metadata_for_chunk(&chunk, &score, item.suggested_action.record_id.clone());
        let route = extraction
            .classification
            .as_ref()
            .map(|classification| classification.artifact_kind.as_str())
            .unwrap_or("review_only");
        extraction.suggested_action = Some(item.suggested_action.clone().with_route(route));
        let result = candidate::add_candidate(
            project_root,
            candidate::NewCandidate {
                id: id.clone(),
                title: item.title.clone(),
                kind: item.kind.clone(),
                scope: item.scope.clone(),
                body: item.body.clone(),
                brief: None,
                tags: Vec::new(),
                language: None,
                targets: targets.clone(),
                evidence: item.evidence.clone(),
                confidence: Some(item.confidence),
                reason: Some(item.reason.clone()),
                matched_template: Some(item.matched_signal.clone()),
                source_observations: Vec::new(),
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
        provider: provider_name.to_string(),
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
        );
    }

    // LLM 路径：复用 extract_llm_text_to_drafts，加上 max_candidates 限制
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

/// 本地正则高价值提取引擎（保持向后兼容）
fn extract_local_high_value_text_to_drafts(
    project_root: &Path,
    input: &str,
    targets: Vec<String>,
    source: &str,
    dry_run: bool,
    max_candidates: usize,
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
    );
    recurrence::apply_recurrence_boost(project_root, &mut candidates)?;

    // 对已有 skilllet 做语义比对。重复内容保留为合并建议，而不是静默丢弃。
    let existing_skilllets = skilllet::load_skilllets(project_root)?;
    let deduper = embedding::SemanticDeduper::new(0.75, 0.65);
    let mut skipped = Vec::new();
    let candidates: Vec<(
        Candidate,
        candidate::ExtractionAction,
        quality_gate::QualityGateDecision,
    )> = candidates
        .into_iter()
        .map(|candidate| {
            let action = match deduper.dedup_against_existing(&candidate.body, &existing_skilllets)
            {
                embedding::DedupResult::Duplicate {
                    similar_id,
                    similarity,
                } => candidate::ExtractionAction::merge_into_existing(similar_id, similarity),
                embedding::DedupResult::Unique => candidate::ExtractionAction::new_candidate(),
            };
            (candidate, action)
        })
        .filter_map(|(candidate, action)| {
            let decision = evaluate_candidate_quality(&candidate, &action);
            if decision.disposition == QualityDisposition::Skip {
                skipped.push(quality_skip_message(&draft_id(&candidate), &decision));
                None
            } else {
                Some((candidate, action, decision))
            }
        })
        .collect();

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
            let classification = classify::classify_chunk(&chunk);
            let routed_action = action.clone().with_route(&classification.artifact_kind);
            ExtractCandidatePreview {
                id: draft_id(candidate),
                title: candidate.title.clone(),
                body: candidate.body.clone(),
                kind: candidate.kind.clone(),
                scope: candidate.scope.clone(),
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
        extraction.suggested_action = Some(action.with_route(route));
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

#[cfg(test)]
fn extract_candidates(input: &str) -> Vec<Candidate> {
    let preferences = built_in_preferences();
    extract_candidates_with_preferences(input, &preferences)
}

fn extract_candidates_with_preferences(
    input: &str,
    preferences: &[KnownPreference],
) -> Vec<Candidate> {
    let mut candidates = Vec::new();
    for raw_sentence in split_sentences(input) {
        for sentence in split_atomic_sentences(raw_sentence) {
            let sentence = sentence.as_str();
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
) -> Vec<Candidate> {
    let mut candidates = Vec::new();
    let mut weak_counts = std::collections::BTreeMap::<String, usize>::new();
    let mut weak_candidates = std::collections::BTreeMap::<String, Candidate>::new();

    for raw_sentence in split_sentences(input) {
        for sentence in split_atomic_sentences(raw_sentence) {
            let sentence = sentence.as_str();
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
            if !looks_like_skilllet_signal(sentence) {
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
            evidence: sentence.to_string(),
            confidence: Some(0.78),
            reason: Some(
                "Matched high-value Skilllet signal: durable preference, constraint, or workflow."
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

    let mut deduped = dedupe_candidates(candidates);
    deduped.sort_by(|a, b| {
        let left = a.confidence.unwrap_or(0.0);
        let right = b.confidence.unwrap_or(0.0);
        right
            .total_cmp(&left)
            .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
    });
    deduped.truncate(max_candidates);
    deduped
}

#[cfg(test)]
mod tests;
