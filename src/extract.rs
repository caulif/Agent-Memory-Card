use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use serde::Deserialize;

use crate::candidate;
use crate::config;
use crate::draft::{self, NewDraft};
use crate::fsutil;
use crate::provider;
use crate::skilllet;
use crate::textutil;

pub mod chunk;
pub mod classify;
mod dedupe;
pub(crate) mod embedding;
pub mod gate;
mod llm;
pub mod quality;
pub mod scoring;
mod signals;

pub use quality::{QualityReport, QualityTextCase, quality_report_for_text_cases};

use dedupe::dedupe_candidates;
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
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PreferenceTemplatePreview {
    pub title: String,
    pub body: String,
    pub source: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PreferenceRegistryValidationReport {
    pub errors: usize,
    pub warnings: usize,
    pub messages: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PreferenceTestReport {
    pub matches: Vec<PreferenceTestMatch>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PreferenceTestMatch {
    pub draft_id: String,
    pub title: String,
    pub body: String,
    pub source: String,
    pub required: Vec<String>,
    pub context: Vec<String>,
}

impl PreferenceTestReport {
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("Agent-Kernel preference test\n\n");
        if self.matches.is_empty() {
            out.push_str("No preference templates matched.\n");
            return out;
        }
        out.push_str("Matches:\n");
        for item in &self.matches {
            out.push_str(&format!(
                "- {} [{}] {}: {}\n",
                item.draft_id, item.source, item.title, item.body
            ));
            out.push_str(&format!("  required: {}\n", item.required.join(", ")));
            out.push_str(&format!("  context: {}\n", item.context.join(", ")));
        }
        out
    }
}

impl PreferenceRegistryValidationReport {
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("Agent-Kernel preference registry validation\n\n");
        if self.messages.is_empty() {
            out.push_str("No issues found.\n");
        } else {
            for message in &self.messages {
                out.push_str(&format!("- {message}\n"));
            }
        }
        out.push_str(&format!(
            "\nSummary: {} errors, {} warnings\n",
            self.errors, self.warnings
        ));
        out
    }
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

pub fn preference_templates(project_root: &Path) -> Result<Vec<PreferenceTemplatePreview>> {
    Ok(load_known_preferences(project_root)?
        .into_iter()
        .map(|preference| PreferenceTemplatePreview {
            title: preference.title,
            body: preference.body,
            source: preference.source,
        })
        .collect())
}

pub fn init_preference_registry(project_root: &Path) -> Result<bool> {
    let root = fsutil::normalize_project_root(project_root)?;
    config::ensure_kernel_dir(&root)?;
    let path = preference_registry_path(&root);
    if path.exists() {
        return Ok(false);
    }
    fs::write(path, DEFAULT_PREFERENCE_REGISTRY)?;
    Ok(true)
}

pub fn validate_preference_registry(
    project_root: &Path,
) -> Result<PreferenceRegistryValidationReport> {
    let root = fsutil::normalize_project_root(project_root)?;
    let path = preference_registry_path(&root);
    if !path.exists() {
        return Ok(PreferenceRegistryValidationReport {
            errors: 0,
            warnings: 1,
            messages: vec![
                "warning: .agent-kernel/preference-registry.yml does not exist".to_string(),
            ],
        });
    }

    let text = fs::read_to_string(&path)?;
    let registry: ProjectPreferenceRegistry = serde_yaml::from_str(&text)?;
    let mut report = PreferenceRegistryValidationReport {
        errors: 0,
        warnings: 0,
        messages: Vec::new(),
    };
    let mut seen_titles = BTreeSet::new();
    for (index, preference) in registry.preferences.iter().enumerate() {
        let number = index + 1;
        let title = preference.title.trim();
        if title.is_empty() {
            report.errors += 1;
            report
                .messages
                .push(format!("error: preference #{number} has an empty title"));
        } else if !seen_titles.insert(title.to_lowercase()) {
            report.errors += 1;
            report.messages.push(format!(
                "error: preference #{number} has duplicate title `{title}`"
            ));
        }
        if preference.body.trim().is_empty() {
            report.errors += 1;
            report
                .messages
                .push(format!("error: preference #{number} has an empty body"));
        }
        if preference.required.is_empty() {
            report.errors += 1;
            report.messages.push(format!(
                "error: preference #{number} has no required markers"
            ));
        }
        if preference.context.is_empty() {
            report.warnings += 1;
            report.messages.push(format!(
                "warning: preference #{number} has no context markers and may match too broadly"
            ));
        }
    }
    Ok(report)
}

pub fn test_preference_text(project_root: &Path, text: &str) -> Result<PreferenceTestReport> {
    let preferences = load_known_preferences(project_root)?;
    let mut matches = Vec::new();
    for sentence in split_sentences(text) {
        if !looks_like_rule(sentence) {
            continue;
        }
        let lower = sentence.to_lowercase();
        for preference in &preferences {
            if let Some(reason) = preference.match_reason(&lower) {
                let candidate = Candidate {
                    title: preference.title.clone(),
                    body: preference.body.clone(),
                    kind: "preference".to_string(),
                    scope: "project".to_string(),
                    evidence: sentence.to_string(),
                    confidence: Some(0.92),
                    reason: Some(format_match_reason(&reason)),
                    matched_template: Some(format!("{}:{}", preference.source, preference.title)),
                };
                matches.push(PreferenceTestMatch {
                    draft_id: draft_id(&candidate),
                    title: preference.title.clone(),
                    body: preference.body.clone(),
                    source: preference.source.clone(),
                    required: reason.required,
                    context: reason.context,
                });
                break;
            }
        }
    }
    Ok(PreferenceTestReport { matches })
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
    let candidates = extract_candidates_with_preferences(&redacted_input, &preferences);

    // 对已有 skilllet 做语义比对。重复内容保留为合并建议，而不是静默丢弃。
    let existing_skilllets = skilllet::load_skilllets(project_root)?;
    let deduper = embedding::SemanticDeduper::new(0.75, 0.65);
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
        .collect();

    // 应用质量评分门控：将每个候选转为 EvidenceChunk（使用原始证据文本），评分，仅保留 Candidate 级别
    let mut scored_candidates: Vec<(
        Candidate,
        scoring::ExtractionScore,
        candidate::ExtractionAction,
    )> = Vec::new();
    for (candidate, action) in candidates {
        let chunk = chunk::EvidenceChunk {
            id: candidate.title.clone(),
            text: candidate.evidence.clone(),
            origin: chunk::ChunkOrigin::User,
            source_kind: source.to_string(),
            source_observations: Vec::new(),
        };
        let score = scoring::score_chunk(&chunk);
        if score.disposition == scoring::ExtractionDisposition::Candidate {
            scored_candidates.push((candidate, score, action));
        }
    }
    // 按 confidence 降序排序，截断到 10 条
    scored_candidates.sort_by(|(a, _, _), (b, _, _)| {
        b.confidence
            .unwrap_or(0.0)
            .partial_cmp(&a.confidence.unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored_candidates.truncate(10);

    let previews = scored_candidates
        .iter()
        .map(|(candidate, _score, action)| {
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
            }
        })
        .collect::<Vec<_>>();
    if dry_run {
        return Ok(ExtractReport {
            created: Vec::new(),
            skipped: Vec::new(),
            candidates: previews,
            dry_run,
            provider: "local".to_string(),
            redacted,
        });
    }

    let mut created = Vec::new();
    let mut skipped = Vec::new();
    for (candidate, score, action) in scored_candidates {
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
    let mut scored_items: Vec<(&embedding::LlmKnowledgeItem, scoring::ExtractionScore)> =
        Vec::new();
    for item in &final_items {
        let chunk = chunk::EvidenceChunk {
            id: item.title.clone(),
            text: item.body.clone(),
            origin: chunk::ChunkOrigin::Assistant,
            source_kind: source.to_string(),
            source_observations: Vec::new(),
        };
        let score = scoring::score_chunk(&chunk);
        if score.disposition == scoring::ExtractionDisposition::Candidate {
            scored_items.push((item, score));
        }
    }
    // 按 confidence 降序排序，截断到 10 条
    scored_items.sort_by(|(a, _), (b, _)| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored_items.truncate(10);

    // 生成预览
    let previews = scored_items
        .iter()
        .map(|(item, score)| {
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
            }
        })
        .collect::<Vec<_>>();

    if dry_run {
        return Ok(ExtractReport {
            created: Vec::new(),
            skipped: Vec::new(),
            candidates: previews,
            dry_run,
            provider: provider_name.to_string(),
            redacted,
        });
    }

    // 写入 Candidate（附带提取元数据）
    let mut created = Vec::new();
    let mut skipped = Vec::new();
    for (item, score) in scored_items {
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
    let candidates = extract_high_value_candidates_with_preferences(
        &redacted_input,
        &preferences,
        max_candidates,
    );

    // 对已有 skilllet 做语义比对。重复内容保留为合并建议，而不是静默丢弃。
    let existing_skilllets = skilllet::load_skilllets(project_root)?;
    let deduper = embedding::SemanticDeduper::new(0.75, 0.65);
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
        .collect();

    let previews = candidates
        .iter()
        .map(|(candidate, action)| {
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
            }
        })
        .collect::<Vec<_>>();
    if dry_run {
        return Ok(ExtractReport {
            created: Vec::new(),
            skipped: Vec::new(),
            candidates: previews,
            dry_run,
            provider: "local".to_string(),
            redacted,
        });
    }

    let mut created = Vec::new();
    let mut skipped = Vec::new();
    for (candidate, action) in candidates {
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
    for sentence in split_sentences(input) {
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

    for sentence in split_sentences(input) {
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
        weak_candidates.entry(key).or_insert_with(|| Candidate {
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
        });
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

fn high_value_prompt_candidate(sentence: &str) -> Option<Candidate> {
    if !looks_like_high_value_prompt_signal(sentence) {
        return None;
    }

    let body = normalize_high_value_prompt_body(sentence);
    if body.len() < 28 || body.len() > 360 {
        return None;
    }

    Some(Candidate {
        title: title_from_high_value_prompt(&body),
        body,
        kind: "procedure".to_string(),
        scope: infer_scope(sentence).to_string(),
        evidence: sentence.to_string(),
        confidence: Some(0.84),
        reason: Some(
            "Matched high-value prompt pattern: reusable agent handoff or project-improving workflow."
                .to_string(),
        ),
        matched_template: Some("high-value-prompt".to_string()),
    })
}

fn scored_signal_candidate(sentence: &str, origin: chunk::ChunkOrigin) -> Option<Candidate> {
    let chunk = chunk::EvidenceChunk {
        id: textutil::slug(sentence),
        text: sentence.trim().to_string(),
        origin,
        source_kind: "local-text".to_string(),
        source_observations: Vec::new(),
    };
    let score = scoring::score_chunk(&chunk);
    if score.disposition != scoring::ExtractionDisposition::Candidate {
        return None;
    }

    let body = normalize_body(sentence);
    if body.len() < 12 || body.len() > 400 {
        return None;
    }

    Some(Candidate {
        title: title_from_scored_signal(&body, &score.matched_signal),
        body,
        kind: kind_from_scored_signal(&score.matched_signal).to_string(),
        scope: infer_scope(sentence).to_string(),
        evidence: sentence.to_string(),
        confidence: Some((score.score / 5.0).clamp(0.7, 0.94)),
        reason: Some(score.reason),
        matched_template: Some(format!("score:{}", score.matched_signal)),
    })
}

fn kind_from_scored_signal(signal: &str) -> &'static str {
    match signal {
        "constraint" => "constraint",
        "procedure" | "correction" | "decision" | "ai_project_improvement" => "procedure",
        _ => "preference",
    }
}

fn title_from_scored_signal(body: &str, signal: &str) -> String {
    if signal == "ai_project_improvement" {
        "Review AI-Origin Project Improvement".to_string()
    } else if signal == "constraint" && body.contains("AGENTS.md") {
        "Protect AGENTS.md Drift Review".to_string()
    } else {
        title_from_body(body)
    }
}

fn normalize_known_preference(
    sentence: &str,
    preferences: &[KnownPreference],
) -> Option<Candidate> {
    let lower = sentence.to_lowercase();
    preferences.iter().find_map(|preference| {
        preference.match_reason(&lower).map(|reason| Candidate {
            title: preference.title.clone(),
            body: preference.body.clone(),
            kind: "preference".to_string(),
            scope: "project".to_string(),
            evidence: sentence.to_string(),
            confidence: Some(0.92),
            reason: Some(format_match_reason(&reason)),
            matched_template: Some(format!("{}:{}", preference.source, preference.title)),
        })
    })
}

struct KnownPreference {
    title: String,
    body: String,
    source: String,
    required: Vec<String>,
    context: Vec<String>,
}

impl KnownPreference {
    fn match_reason(&self, lower: &str) -> Option<KnownPreferenceMatchReason> {
        let required = self
            .required
            .iter()
            .filter(|marker| lower.contains(marker.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        if required.len() != self.required.len() {
            return None;
        }
        let context = self
            .context
            .iter()
            .filter(|marker| lower.contains(marker.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        if !self.context.is_empty() && context.is_empty() {
            return None;
        }
        Some(KnownPreferenceMatchReason { required, context })
    }
}

struct KnownPreferenceMatchReason {
    required: Vec<String>,
    context: Vec<String>,
}

fn format_match_reason(reason: &KnownPreferenceMatchReason) -> String {
    let required = if reason.required.is_empty() {
        "none".to_string()
    } else {
        reason.required.join(", ")
    };
    let context = if reason.context.is_empty() {
        "none".to_string()
    } else {
        reason.context.join(", ")
    };
    format!("Matched preference template; required: {required}; context: {context}")
}

#[derive(Debug, Default, Deserialize)]
struct ProjectPreferenceRegistry {
    #[serde(default)]
    preferences: Vec<ProjectPreference>,
}

#[derive(Debug, Deserialize)]
struct ProjectPreference {
    #[serde(default)]
    title: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    required: Vec<String>,
    #[serde(default)]
    context: Vec<String>,
}

fn load_known_preferences(project_root: &Path) -> Result<Vec<KnownPreference>> {
    let mut preferences = load_project_preferences(project_root)?;
    preferences.extend(built_in_preferences());
    Ok(preferences)
}

fn load_project_preferences(project_root: &Path) -> Result<Vec<KnownPreference>> {
    let root = fsutil::normalize_project_root(project_root)?;
    let path = preference_registry_path(&root);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(&path)?;
    let registry: ProjectPreferenceRegistry = serde_yaml::from_str(&text)?;
    Ok(registry
        .preferences
        .into_iter()
        .filter(|preference| {
            !preference.title.trim().is_empty() && !preference.body.trim().is_empty()
        })
        .map(|preference| KnownPreference {
            title: preference.title,
            body: preference.body,
            source: "project".to_string(),
            required: preference
                .required
                .into_iter()
                .map(|marker| marker.to_lowercase())
                .collect(),
            context: preference
                .context
                .into_iter()
                .map(|marker| marker.to_lowercase())
                .collect(),
        })
        .collect())
}

fn preference_registry_path(project_root: &Path) -> PathBuf {
    config::kernel_dir(project_root).join("preference-registry.yml")
}

const DEFAULT_PREFERENCE_REGISTRY: &str = r#"preferences:
  - title: Use Playwright
    body: Use Playwright for browser automation tests.
    required:
      - playwright
    context:
      - cypress
      - browser automation
      - 浏览器自动化
"#;

fn built_in_preferences() -> Vec<KnownPreference> {
    vec![
        KnownPreference {
            title: "Prefer Bun".to_string(),
            body: "Use Bun for JavaScript package management and scripts.".to_string(),
            source: "built-in".to_string(),
            required: strings(&["bun"]),
            context: strings(&[
                "npm",
                "pnpm",
                "yarn",
                "bun run",
                "bunx",
                "包管理",
                "package manager",
                "package management",
                "javascript package",
                "js 脚本",
            ]),
        },
        KnownPreference {
            title: "Use Axios".to_string(),
            body: "Use Axios for frontend HTTP requests.".to_string(),
            source: "built-in".to_string(),
            required: strings(&["axios"]),
            context: strings(&[
                "fetch",
                "http",
                "request",
                "requests",
                "api",
                "前端请求",
                "请求",
                "接口",
            ]),
        },
        KnownPreference {
            title: "Use Vitest".to_string(),
            body: "Use Vitest for frontend unit tests.".to_string(),
            source: "built-in".to_string(),
            required: strings(&["vitest"]),
            context: strings(&[
                "jest",
                "unit test",
                "unit tests",
                "frontend test",
                "frontend tests",
                "单元测试",
                "测试",
            ]),
        },
    ]
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

fn draft_id(candidate: &Candidate) -> String {
    let slug = textutil::slug(&candidate.title);
    format!("project:{slug}")
}

/// 从证据块和评分结果构建提取元数据，用于记录溯源信息。
fn extraction_metadata_for_chunk(
    chunk: &chunk::EvidenceChunk,
    score: &scoring::ExtractionScore,
    similar_record: Option<String>,
) -> candidate::ExtractionMetadata {
    let classification = classify::classify_chunk(chunk);
    let route = classification.artifact_kind.clone();
    candidate::ExtractionMetadata {
        origin: chunk.origin.as_str().to_string(),
        matched_signal: score.matched_signal.clone(),
        reason: score.reason.clone(),
        source_observations: chunk.source_observations.clone(),
        score_breakdown: score.breakdown.clone(),
        similar_record,
        tags: classification.tags.clone(),
        classification: Some(classification),
        suggested_action: Some(candidate::ExtractionAction::new_candidate_for_route(&route)),
    }
}

// ============================================================
// 旧版正则提取函数（从 signals.rs 移入，供 local engine 使用）
// ============================================================

fn looks_like_rule(sentence: &str) -> bool {
    let lower = sentence.to_lowercase();
    let markers = [
        "以后",
        "必须",
        "不要",
        "禁止",
        "统一",
        "默认",
        "优先",
        "改用",
        "不再",
        "always",
        "never",
        "must",
        "prefer",
        "use ",
        "don't",
        "do not",
        "default to",
    ];
    markers.iter().any(|marker| lower.contains(marker))
}

fn looks_like_skilllet_signal(sentence: &str) -> bool {
    let lower = sentence.to_lowercase();
    if signals::is_low_value_task_sentence(&lower) {
        return false;
    }
    if signals::looks_like_unresolved_user_request(&lower)
        && !signals::has_strong_memory_marker(&lower)
    {
        return false;
    }

    let durable_markers = [
        "以后",
        "所有项目",
        "每次",
        "总是",
        "默认",
        "统一",
        "优先",
        "禁止",
        "不要",
        "必须",
        "记住",
        "always",
        "never",
        "prefer",
        "by default",
        "default to",
        "for every",
        "for all",
        "must",
        "do not",
    ];
    let domain_markers = [
        "agent",
        "skill",
        "skilllet",
        "claude",
        "codex",
        "cursor",
        "bun",
        "cargo",
        "rust",
        "typescript",
        "react",
        "tauri",
        "test",
        "ui",
        "api",
        "http",
        "git",
        "mcp",
        "智能体",
        "技能",
        "规则",
        "项目",
        "测试",
        "前端",
        "后端",
        "界面",
        "代理",
    ];

    durable_markers.iter().any(|marker| lower.contains(marker))
        && domain_markers.iter().any(|marker| lower.contains(marker))
}

fn looks_like_high_value_prompt_signal(sentence: &str) -> bool {
    let lower = sentence.to_lowercase();
    if signals::is_low_value_task_sentence(&lower) {
        return false;
    }
    if signals::looks_like_unresolved_user_request(&lower)
        && !signals::has_explicit_memory_marker(&lower)
    {
        return false;
    }

    let prompt_markers = [
        "提示",
        "prompt",
        "可以先",
        "先让",
        "再由",
        "让 claude",
        "由 codex",
        "交给 claude",
        "codex 做",
        "输出",
        "清单",
        "步骤",
        "检查",
        "复用",
        "模式",
        "减少返工",
        "显著减少",
        "提高质量",
        "降低理解成本",
        "减少误解",
        "handoff",
        "checklist",
        "playbook",
        "workflow",
    ];
    let domain_markers = [
        "claude",
        "codex",
        "agent",
        "skilllet",
        "skill",
        "ui",
        "组件",
        "状态流",
        "按钮",
        "rust",
        "tauri",
        "react",
        "测试",
        "集成测试",
        "架构",
        "前端",
        "后端",
        "重构",
        "agent",
        "frontend",
        "backend",
        "architecture",
        "test",
        "refactor",
    ];
    let outcome_markers = [
        "减少返工",
        "显著减少",
        "提高质量",
        "降低理解成本",
        "减少误解",
        "更稳定",
        "更清晰",
        "可复用",
        "避免遗漏",
        "avoid rework",
        "reduce rework",
        "improve",
    ];

    prompt_markers
        .iter()
        .filter(|marker| lower.contains(**marker))
        .count()
        >= 2
        && domain_markers.iter().any(|marker| lower.contains(marker))
        && outcome_markers.iter().any(|marker| lower.contains(marker))
}

fn normalize_body(sentence: &str) -> String {
    let mut body = sentence.trim().to_string();
    let replacements = [
        ("我再说最后一次，", ""),
        ("我再说最后一次", ""),
        ("以后", ""),
        ("请", ""),
        ("记住：", ""),
        ("记住:", ""),
    ];
    for (from, to) in replacements {
        body = body.replace(from, to);
    }
    body = body.trim_matches(['，', ',', ' ']).trim().to_string();
    if body.ends_with(';') {
        body.pop();
    }
    body
}

fn normalize_project_improvement_body(sentence: &str) -> String {
    let mut body = normalize_body(sentence);
    let replacements = [
        ("这次", ""),
        ("最终做法是", "Reusable approach:"),
        ("根因是", "Root cause:"),
        ("原因是", "Root cause:"),
    ];
    for (from, to) in replacements {
        body = body.replace(from, to);
    }
    body.trim_matches(['，', ',', ' ']).trim().to_string()
}

fn normalize_high_value_prompt_body(sentence: &str) -> String {
    let body = normalize_body(sentence);
    let replacements = [
        ("这个提示能", "This prompt can "),
        ("这个提示可以", "This prompt can "),
        ("可以先", "Start by "),
        ("再由", "then hand off to "),
    ];
    let mut normalized = body;
    for (from, to) in replacements {
        normalized = normalized.replace(from, to);
    }
    normalized.trim_matches(['，', ',', ' ']).trim().to_string()
}

fn title_from_body(body: &str) -> String {
    let words = body.split_whitespace().collect::<Vec<_>>();
    if words.len() >= 3 {
        return words.iter().take(6).copied().collect::<Vec<_>>().join(" ");
    }
    body.chars().take(24).collect()
}

fn title_from_project_improvement(body: &str) -> String {
    let lower = body.to_lowercase();
    if lower.contains("卡死") || lower.contains("卡顿") || lower.contains("freeze") {
        "Keep UI Responsive During Long Tasks".to_string()
    } else if lower.contains("增量") || lower.contains("incremental") {
        "Use Incremental Local Processing".to_string()
    } else if lower.contains("跨平台") || lower.contains("windows") || lower.contains("mac") {
        "Handle Cross-Platform Runtime Differences".to_string()
    } else if lower.contains("测试") || lower.contains("test") {
        "Preserve Regression Tests For Fixes".to_string()
    } else {
        title_from_body(body)
    }
}

fn title_from_high_value_prompt(body: &str) -> String {
    let lower = body.to_lowercase();
    if lower.contains("claude") && lower.contains("codex") && lower.contains("ui") {
        "Use Agent Handoff Prompts For UI Refactors".to_string()
    } else if lower.contains("checklist") || lower.contains("清单") {
        "Use Checklist Prompts For Complex Agent Tasks".to_string()
    } else if lower.contains("架构") || lower.contains("architecture") {
        "Use Architecture Prompts Before Implementation".to_string()
    } else {
        title_from_body(body)
    }
}

fn classify_kind(sentence: &str) -> &'static str {
    let lower = sentence.to_lowercase();
    if lower.contains("不要")
        || lower.contains("禁止")
        || lower.contains("never")
        || lower.contains("do not")
        || lower.contains("don't")
    {
        "constraint"
    } else {
        "preference"
    }
}

fn infer_scope(sentence: &str) -> &'static str {
    let lower = sentence.to_lowercase();
    if lower.contains("所有项目") || lower.contains("全局") || lower.contains("for all") {
        "global"
    } else if lower.contains("claude") || lower.contains("codex") || lower.contains("agent") {
        "agent"
    } else {
        "project"
    }
}

#[cfg(test)]
mod tests;
