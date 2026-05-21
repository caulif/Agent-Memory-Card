use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::config;
use crate::draft::{self, DraftUpdate, NewDraft};
use crate::extract::classify::KnowledgeClassification;
use crate::extract::lifecycle::MemoryCardOperation;
use crate::extract::memory_gate::{self, MemoryGateDisposition};
use crate::feedback;
use crate::fsutil;
use crate::memory_card::{self, MemoryCardRecord, MemoryCardUpdate};
use crate::provider::{self, ProviderJsonSchema, ProviderRequest};
use crate::synthesis_agent::{self, SkillUsefulnessEvaluation, SynthesisReview};
use crate::textutil;

mod action;
mod evidence;

pub use evidence::{
    EvidenceBundle, EvidenceContextRecord, EvidenceQuoteRecord, EvidenceValidity, SourceTrust,
};

/// 提取元数据：记录提取过程中的溯源信息，用于解释为什么生成这条记录。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExtractionMetadata {
    #[serde(default)]
    pub origin: String,
    #[serde(default)]
    pub matched_signal: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub source_observations: Vec<String>,
    #[serde(default)]
    pub score_breakdown: BTreeMap<String, f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classification: Option<KnowledgeClassification>,
    #[serde(default)]
    pub similar_record: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested_action: Option<ExtractionAction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_span: Option<EvidenceSpan>,
    #[serde(default)]
    pub memory_tier: MemoryTier,
    #[serde(default)]
    pub value_scores: BTreeMap<String, f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abstraction_of: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abstracted_from: Option<String>,
    /// 五层流水线版本（仅当 origin=pipeline-v2 时设置）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pipeline_version: Option<u32>,
    /// 每层执行 trace，按时间序追加（strip/truncate/cluster/induce/crystallize）
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layer_trace: Vec<LayerTraceEntry>,
    /// 若本卡在某层被拒绝，标注哪一层；接受落盘时为 None
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rejected_at: Option<String>,
    /// 统一证据契约：让 Candidate / Draft / MemoryCard 使用同一种来源、quote 与可信度结构。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_bundle: Option<EvidenceBundle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card_function: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_claim: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_delta: Option<ValueDelta>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_context: Option<TargetContext>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill_usefulness: Option<SkillUsefulnessEvaluation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub synthesis_trace: Vec<SynthesisTraceEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub synthesis_action: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub synthesis_stop_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ValueDelta {
    #[serde(default)]
    pub existing_behavior: String,
    #[serde(default)]
    pub missing_part: String,
    #[serde(default)]
    pub new_behavior: String,
    #[serde(default)]
    pub why_not_duplicate: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TargetContext {
    #[serde(default)]
    pub target_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_id: Option<String>,
    #[serde(default)]
    pub why_this_target: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SynthesisTraceEntry {
    pub step: String,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub item_ids: Vec<String>,
}

/// 单层执行记录：层名 + 耗时 + 关键计数（如 cluster_size、kept_ratio）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LayerTraceEntry {
    pub layer: String,
    pub ms: u64,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub info: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceSpan {
    pub role: String,
    pub quote: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default)]
    pub surrounding_context: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtractionAction {
    pub action: String,
    #[serde(default = "default_extraction_route")]
    pub route: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_record: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compile_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub similarity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MemoryTier {
    #[default]
    ProjectRule,
    CrossProjectPrinciple,
    CollaborationPreference,
}

impl MemoryTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            MemoryTier::ProjectRule => "project_rule",
            MemoryTier::CrossProjectPrinciple => "cross_project_principle",
            MemoryTier::CollaborationPreference => "collaboration_preference",
        }
    }
}

fn default_extraction_route() -> String {
    "always_on_rule".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CandidateStatus {
    Candidate,
    Hidden,
    Rejected,
    Promoted,
}

fn default_candidate_status() -> CandidateStatus {
    CandidateStatus::Candidate
}

fn current_schema_version() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateRecord {
    #[serde(default = "current_schema_version")]
    pub schema_version: u32,
    pub id: String,
    pub title: String,
    pub kind: String,
    pub scope: String,
    pub body: String,
    #[serde(default)]
    pub brief: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default)]
    pub targets: Vec<String>,
    pub evidence: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_template: Option<String>,
    #[serde(default)]
    pub source_observations: Vec<String>,
    #[serde(default)]
    pub extraction: ExtractionMetadata,
    #[serde(default)]
    pub operation: MemoryCardOperation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duplicate_of: Option<String>,
    #[serde(default)]
    pub conflict_with: Vec<String>,
    #[serde(default)]
    pub quality_flags: Vec<String>,
    #[serde(default = "default_candidate_status")]
    pub status: CandidateStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rejected_reason: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default)]
pub struct CandidateUpdate {
    pub title: Option<String>,
    pub body: Option<String>,
    pub brief: Option<String>,
    pub tags: Option<Vec<String>>,
    pub language: Option<String>,
    pub kind: Option<String>,
    pub scope: Option<String>,
    pub targets: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct NewCandidate {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub scope: String,
    pub body: String,
    pub brief: Option<String>,
    pub tags: Vec<String>,
    pub language: Option<String>,
    pub targets: Vec<String>,
    pub evidence: String,
    pub confidence: Option<f32>,
    pub reason: Option<String>,
    pub matched_template: Option<String>,
    pub source_observations: Vec<String>,
    pub extraction: ExtractionMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CandidateGcReport {
    pub evaluated: usize,
    pub hidden: Vec<String>,
}

pub fn add_candidate(project_root: &Path, candidate: NewCandidate) -> Result<()> {
    let root = fsutil::normalize_project_root(project_root)?;
    config::ensure_kernel_dir(&root)?;
    let now = Utc::now().to_rfc3339();
    let metadata = infer_candidate_metadata(
        &candidate.title,
        &candidate.body,
        &candidate.kind,
        candidate.matched_template.as_deref(),
        candidate.reason.as_deref(),
    );
    let record = CandidateRecord {
        schema_version: current_schema_version(),
        id: candidate.id.clone(),
        title: candidate.title,
        kind: candidate.kind,
        scope: candidate.scope,
        body: candidate.body,
        brief: candidate
            .brief
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(metadata.brief),
        tags: {
            let mut normalized_tags = textutil::normalize_string_list(candidate.tags);
            normalized_tags.extend(textutil::normalize_string_list(
                candidate.extraction.tags.clone(),
            ));
            normalized_tags.sort();
            normalized_tags.dedup();
            if normalized_tags.is_empty() {
                metadata.tags
            } else {
                normalized_tags
            }
        },
        language: candidate
            .language
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(metadata.language),
        targets: textutil::normalize_string_list(candidate.targets),
        evidence: candidate.evidence,
        confidence: candidate.confidence,
        reason: candidate.reason,
        matched_template: candidate.matched_template,
        source_observations: textutil::normalize_string_list(candidate.source_observations),
        operation: operation_from_extraction(&candidate.extraction),
        duplicate_of: candidate
            .extraction
            .suggested_action
            .as_ref()
            .and_then(|action| action.target_record.clone()),
        conflict_with: Vec::new(),
        quality_flags: Vec::new(),
        extraction: candidate.extraction,
        status: CandidateStatus::Candidate,
        rejected_reason: None,
        created_at: now.clone(),
        updated_at: now,
    };
    let path = candidate_path_for_scope(&root, &record.scope, &candidate.id)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_yaml::to_string(&record)?)?;
    Ok(())
}

pub fn load_candidates(project_root: &Path) -> Result<Vec<CandidateRecord>> {
    let root = fsutil::normalize_project_root(project_root)?;
    let dir = candidates_dir(&root);
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
        let mut record: CandidateRecord = serde_yaml::from_str(&fs::read_to_string(entry.path())?)?;
        enrich_record_defaults(&mut record);
        records.push(record);
    }
    records.sort_by(|a: &CandidateRecord, b: &CandidateRecord| {
        b.confidence
            .unwrap_or(0.0)
            .total_cmp(&a.confidence.unwrap_or(0.0))
            .then_with(|| {
                b.matched_template
                    .is_some()
                    .cmp(&a.matched_template.is_some())
            })
            .then_with(|| b.updated_at.cmp(&a.updated_at))
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(records)
}

pub fn list_visible_candidates(project_root: &Path) -> Result<Vec<CandidateRecord>> {
    Ok(load_candidates(project_root)?
        .into_iter()
        .filter(|candidate| candidate.status == CandidateStatus::Candidate)
        .collect())
}

pub fn list_visible_candidates_with_synthesis_preview(
    project_root: &Path,
) -> Result<Vec<CandidateRecord>> {
    let root = fsutil::normalize_project_root(project_root)?;
    Ok(list_visible_candidates(&root)?
        .into_iter()
        .map(|candidate| enrich_candidate_with_synthesis_preview(&root, candidate))
        .collect())
}

pub fn enrich_candidate_with_synthesis_preview(
    project_root: &Path,
    mut candidate: CandidateRecord,
) -> CandidateRecord {
    let approvable_kind = normalize_approvable_candidate_kind(&candidate.kind);
    let has_review_metadata = candidate.extraction.value_delta.is_some()
        && candidate.extraction.value_claim.is_some()
        && !candidate.extraction.synthesis_trace.is_empty();
    let review = if has_review_metadata {
        None
    } else {
        synthesis_agent::run_memory_card_synthesis(project_root, &candidate, &approvable_kind).ok()
    };
    if let Some(review) = review.as_ref() {
        apply_synthesis_review_to_extraction(&mut candidate.extraction, review);
    } else if let Some(card_function) = candidate.extraction.card_function.as_deref() {
        if candidate.extraction.synthesis_action.is_none() {
            candidate.extraction.synthesis_action =
                Some(action_from_card_function(card_function).to_string());
        }
        if candidate.extraction.synthesis_stop_reason.is_none() {
            candidate.extraction.synthesis_stop_reason =
                Some(stop_reason_from_card_function(card_function).to_string());
        }
    }
    if !is_structured_memory_body(&candidate.body) || !is_mature_existing_brief(&candidate.brief) {
        let preview = mature_candidate_deterministic(&candidate, &approvable_kind, review.as_ref());
        candidate.title = preview.title;
        candidate.body = preview.body;
        candidate.brief = preview.brief;
        candidate.kind = preview.kind;
        candidate.scope = preview.scope;
        candidate.tags = preview.tags;
        candidate.language = preview.language;
    }
    candidate
}

pub fn gc_candidates(project_root: &Path) -> Result<CandidateGcReport> {
    let root = fsutil::normalize_project_root(project_root)?;
    let mut report = CandidateGcReport::default();
    for mut candidate in list_visible_candidates(&root)? {
        report.evaluated += 1;
        let decision = memory_gate::evaluate_memory_candidate(
            &candidate.title,
            &candidate.body,
            &candidate.evidence,
            &candidate.kind,
            &candidate.scope,
        );
        if decision.disposition != MemoryGateDisposition::Reject {
            continue;
        }
        candidate.status = CandidateStatus::Hidden;
        candidate.rejected_reason = Some("memory-gc".to_string());
        candidate.updated_at = Utc::now().to_rfc3339();
        save_candidate(&root, &candidate)?;
        feedback::record_feedback(
            &root,
            "candidate",
            &candidate.id,
            "rejected",
            &candidate.body,
            Some("memory-gc".to_string()),
        )?;
        report.hidden.push(candidate.id);
    }
    Ok(report)
}

pub fn hide_candidate(project_root: &Path, id: &str) -> Result<CandidateRecord> {
    update_candidate_status(project_root, id, CandidateStatus::Hidden, None)
}

pub fn reject_candidate(
    project_root: &Path,
    id: &str,
    rejected_reason: Option<String>,
) -> Result<CandidateRecord> {
    let candidate =
        update_candidate_status(project_root, id, CandidateStatus::Rejected, rejected_reason)?;
    feedback::record_feedback(
        project_root,
        "candidate",
        &candidate.id,
        "rejected",
        &candidate.body,
        candidate.rejected_reason.clone(),
    )?;
    Ok(candidate)
}

pub fn update_candidate(
    project_root: &Path,
    id: &str,
    input: CandidateUpdate,
) -> Result<CandidateRecord> {
    let root = fsutil::normalize_project_root(project_root)?;
    let old_path = candidate_path(&root, id)?;
    let mut candidate = load_candidate(&root, id)?;
    if let Some(title) = input.title {
        candidate.title = title;
    }
    if let Some(body) = input.body {
        candidate.body = body;
    }
    if let Some(brief) = input.brief {
        candidate.brief = brief;
    }
    if let Some(tags) = input.tags {
        candidate.tags = textutil::normalize_string_list(tags);
    }
    if let Some(language) = input.language {
        candidate.language = language;
    }
    if let Some(kind) = input.kind {
        candidate.kind = kind;
    }
    if let Some(scope) = input.scope {
        candidate.scope = scope;
    }
    if let Some(targets) = input.targets {
        candidate.targets = textutil::normalize_string_list(targets);
    }
    candidate.updated_at = Utc::now().to_rfc3339();
    save_candidate(&root, &candidate)?;
    let new_path = candidate_path(&root, &candidate.id)?;
    if old_path != new_path && old_path.exists() {
        fs::remove_file(old_path)?;
    }
    Ok(candidate)
}

pub fn promote_candidate_to_draft(project_root: &Path, id: &str) -> Result<draft::DraftRecord> {
    let root = fsutil::normalize_project_root(project_root)?;
    let mut candidate = load_candidate(&root, id)?;
    if candidate.status != CandidateStatus::Candidate {
        return Err(anyhow!(
            "candidate `{id}` cannot be promoted from status {:?}",
            candidate.status
        ));
    }
    draft::add_draft(
        &root,
        NewDraft {
            id: candidate.id.clone(),
            title: candidate.title.clone(),
            kind: candidate.kind.clone(),
            scope: candidate.scope.clone(),
            body: candidate.body.clone(),
            targets: candidate.targets.clone(),
            evidence: candidate.evidence.clone(),
            confidence: candidate.confidence,
            reason: candidate.reason.clone(),
            matched_template: candidate.matched_template.clone(),
            extraction: candidate.extraction.clone(),
        },
    )?;
    let draft = draft::update_draft(
        &root,
        &candidate.id,
        DraftUpdate {
            brief: Some(candidate.brief.clone()),
            tags: Some(candidate.tags.clone()),
            language: Some(candidate.language.clone()),
            ..DraftUpdate::default()
        },
    )?;
    candidate.status = CandidateStatus::Promoted;
    candidate.updated_at = Utc::now().to_rfc3339();
    save_candidate(&root, &candidate)?;
    feedback::record_feedback(
        &root,
        "candidate",
        &candidate.id,
        "promoted-to-draft",
        &candidate.body,
        None,
    )?;
    Ok(draft)
}

pub fn approve_candidate_to_memory_card(project_root: &Path, id: &str) -> Result<MemoryCardRecord> {
    let root = fsutil::normalize_project_root(project_root)?;
    let mut candidate = load_candidate(&root, id)?;
    let approvable_kind = normalize_approvable_candidate_kind(&candidate.kind);
    if candidate.status != CandidateStatus::Candidate {
        return Err(anyhow!(
            "candidate `{id}` cannot be approved from status {:?}",
            candidate.status
        ));
    }
    let matured = mature_candidate_for_memory_card(&root, &candidate, &approvable_kind);
    let matured_extraction =
        extraction_with_synthesis_metadata(candidate.extraction.clone(), &matured);
    if let Some(target_id) = matured_extraction
        .suggested_action
        .as_ref()
        .filter(|action| action.action == "merge_into_existing")
        .and_then(|action| {
            action
                .target_record
                .clone()
                .or_else(|| action.record_id.clone())
        })
    {
        if let Some(existing_memory_card) = memory_card::load_memory_cards(&root)?
            .into_iter()
            .find(|memory_card| memory_card.id == target_id)
        {
            memory_card::update_memory_card_from_review(
                &root,
                &target_id,
                if matured.title.trim().is_empty() {
                    existing_memory_card.title.clone()
                } else {
                    matured.title.clone()
                },
                matured.body.clone(),
                matured.brief.clone(),
                matured.tags.clone(),
                matured.language.clone(),
                matured.kind.clone(),
                existing_memory_card.scope.clone(),
            )?;
            let updated = memory_card::update_memory_card_extraction(
                &root,
                &target_id,
                Some(matured_extraction.clone()),
            )?;
            candidate.status = CandidateStatus::Promoted;
            candidate.updated_at = Utc::now().to_rfc3339();
            save_candidate(&root, &candidate)?;
            feedback::record_feedback(
                &root,
                "candidate",
                &candidate.id,
                "approved-existing-memory_card-merge",
                &candidate.body,
                None,
            )?;
            return Ok(updated);
        }
    }
    // 检查同名 memory_card 是否已存在；若是同一概念的更新，则吸收到现有 MemoryCard。
    if let Some(existing_memory_card) = memory_card::load_memory_cards(&root)?
        .into_iter()
        .find(|memory_card| memory_card.id == candidate.id)
    {
        if textutil::jaccard_similarity(&candidate.body, &existing_memory_card.body) >= 0.75 {
            candidate.status = CandidateStatus::Promoted;
            candidate.updated_at = Utc::now().to_rfc3339();
            save_candidate(&root, &candidate)?;
            feedback::record_feedback(
                &root,
                "candidate",
                &candidate.id,
                "approved-existing-memory_card",
                &candidate.body,
                None,
            )?;
            return Ok(existing_memory_card);
        }
        if memory_card::review_update_matches_existing(
            &existing_memory_card,
            &matured.title,
            &matured.body,
        ) {
            memory_card::update_memory_card_from_review(
                &root,
                &candidate.id,
                matured.title.clone(),
                matured.body.clone(),
                matured.brief.clone(),
                matured.tags.clone(),
                matured.language.clone(),
                matured.kind.clone(),
                matured.scope.clone(),
            )?;
            let updated = memory_card::update_memory_card_extraction(
                &root,
                &candidate.id,
                Some(matured_extraction.clone()),
            )?;
            candidate.status = CandidateStatus::Promoted;
            candidate.updated_at = Utc::now().to_rfc3339();
            save_candidate(&root, &candidate)?;
            feedback::record_feedback(
                &root,
                "candidate",
                &candidate.id,
                "approved-existing-memory_card-update",
                &candidate.body,
                None,
            )?;
            return Ok(updated);
        }
        return Err(anyhow!(
            "memory_card id conflict for `{}` (different body); review or merge the existing MemoryCard before approving this system suggestion",
            candidate.id
        ));
    }

    memory_card::add_memory_card_with_provenance(
        &root,
        &candidate.id,
        &matured.title,
        &matured.body,
        &matured.kind,
        &matured.scope,
        candidate.targets.clone(),
        Some(matured_extraction),
        Some(candidate.id.clone()),
        Some(candidate.evidence.clone()),
    )?;
    let memory_card = memory_card::update_memory_card(
        &root,
        &candidate.id,
        MemoryCardUpdate {
            brief: Some(matured.brief.clone()),
            tags: Some(matured.tags.clone()),
            language: Some(matured.language.clone()),
            ..MemoryCardUpdate::default()
        },
    )?;
    candidate.status = CandidateStatus::Promoted;
    candidate.updated_at = Utc::now().to_rfc3339();
    save_candidate(&root, &candidate)?;
    feedback::record_feedback(
        &root,
        "candidate",
        &candidate.id,
        "approved",
        &candidate.body,
        None,
    )?;
    Ok(memory_card)
}

#[derive(Debug, Clone)]
struct MatureMemoryCardText {
    title: String,
    body: String,
    brief: String,
    kind: String,
    scope: String,
    tags: Vec<String>,
    language: String,
    card_function: String,
    value_claim: String,
    value_delta: ValueDelta,
    target_context: TargetContext,
    skill_usefulness: Option<SkillUsefulnessEvaluation>,
    synthesis_trace: Vec<SynthesisTraceEntry>,
}

#[derive(Debug, serde::Deserialize)]
struct MatureMemoryCardResponse {
    title: String,
    body: String,
    brief: String,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    card_function: Option<String>,
    #[serde(default)]
    value_claim: Option<String>,
    #[serde(default)]
    value_delta: Option<ValueDelta>,
    #[serde(default)]
    target_context: Option<TargetContext>,
}

fn mature_candidate_for_memory_card(
    project_root: &Path,
    candidate: &CandidateRecord,
    approvable_kind: &str,
) -> MatureMemoryCardText {
    let synthesis =
        synthesis_agent::run_memory_card_synthesis(project_root, candidate, approvable_kind).ok();
    mature_candidate_with_provider(project_root, candidate, approvable_kind, synthesis.as_ref())
        .unwrap_or_else(|| {
            mature_candidate_deterministic(candidate, approvable_kind, synthesis.as_ref())
        })
}

fn mature_candidate_with_provider(
    project_root: &Path,
    candidate: &CandidateRecord,
    approvable_kind: &str,
    synthesis: Option<&SynthesisReview>,
) -> Option<MatureMemoryCardText> {
    let provider_path = provider::provider_config_path(project_root).ok()?;
    if !provider_path.exists() {
        return None;
    }
    let cfg = provider::load_or_default_provider_config(project_root).ok()?;
    let request = ProviderRequest {
        system_prompt: r#"你是 Agent Memory Card 的最终改写审核器。

目标：把候选口语片段改写成可长期复用的 Memory Card，而不是复述聊天原文。

输出要求：
- 先判断这张 Memory Card 能让现有 workflow 或项目级 Skill 下次做得更好；没有增量价值时，在 value_delta 中说明已覆盖或边界。
- title 是短规范名，不要保留“/goal”“用户说”“这条候选”等聊天痕迹。
- body 必须采用 When / Do / Boundary 三段结构；中文内容可写成“触发 / 动作 / 边界”。
- Do 必须是未来 agent 可执行的行为规则。
- Boundary 必须写清适用范围、人工确认点或不要越界的内容。
- brief 用“用于...”概括这张卡的用途，不要复读 title/body。
- card_function 只能是 library、skill_targeted、workflow、merge 之一；如果候选激活 skill 或目标是 Skill，优先 skill_targeted。
- value_claim 用一句话说明它要防止的未来失败、补上的 Skill/workflow 缺口或带来的行为改进。
- value_delta 必须说明 existing_behavior、missing_part、new_behavior、why_not_duplicate。
- target_context 说明目标是 project_skill、workflow、memory_card 或 global_reference，以及为什么放在这里。
- 合并重复或相似语义时，保留更成熟、更可执行的表述。
- 不要编造证据中没有的工具、路径、指标或团队规则。
- 只返回 JSON。"#
            .to_string(),
        user_prompt: serde_json::json!({
            "candidate": {
                "id": candidate.id,
                "title": candidate.title,
                "kind": candidate.kind,
                "normalized_kind": approvable_kind,
                "scope": candidate.scope,
                "body": candidate.body,
                "brief": candidate.brief,
                "tags": candidate.tags,
                "evidence": candidate.evidence,
                "reason": candidate.reason,
            },
            "synthesis_context": synthesis,
        })
        .to_string(),
        json_schema: Some(ProviderJsonSchema {
            name: "MatureMemoryCardReview".to_string(),
            strict: true,
            schema: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "title": { "type": "string" },
                    "body": { "type": "string" },
                    "brief": { "type": "string" },
                    "kind": { "type": ["string", "null"], "enum": ["preference", "constraint", "procedure", "convention", "correction", "anti-pattern", "rule", "template", "workflow", null] },
                    "scope": { "type": ["string", "null"], "enum": ["project", "global", "agent", "directory", "agent-specific", null] },
                    "tags": { "type": "array", "items": { "type": "string" } },
                    "card_function": { "type": ["string", "null"], "enum": ["library", "skill_targeted", "workflow", "merge", null] },
                    "value_claim": { "type": ["string", "null"] },
                    "value_delta": {
                        "type": ["object", "null"],
                        "additionalProperties": false,
                        "properties": {
                            "existing_behavior": { "type": "string" },
                            "missing_part": { "type": "string" },
                            "new_behavior": { "type": "string" },
                            "why_not_duplicate": { "type": "string" }
                        },
                        "required": ["existing_behavior", "missing_part", "new_behavior", "why_not_duplicate"]
                    },
                    "target_context": {
                        "type": ["object", "null"],
                        "additionalProperties": false,
                        "properties": {
                            "target_type": { "type": "string", "enum": ["project_skill", "workflow", "memory_card", "global_reference"] },
                            "target_id": { "type": ["string", "null"] },
                            "why_this_target": { "type": "string" }
                        },
                        "required": ["target_type", "target_id", "why_this_target"]
                    }
                },
                "required": ["title", "body", "brief", "kind", "scope", "tags", "card_function", "value_claim", "value_delta", "target_context"]
            }),
        }),
    };
    let output =
        provider::call_provider_for_role(&cfg, provider::ProviderRole::Refine, &request, 2048)
            .ok()?;
    let parsed: MatureMemoryCardResponse = serde_json::from_str(output.trim()).ok()?;
    let body = parsed.body.trim().to_string();
    let title = parsed.title.trim().to_string();
    let brief = parsed.brief.trim().to_string();
    if title.chars().count() < 4 || body.chars().count() < 32 || brief.chars().count() < 8 {
        return None;
    }
    if body.contains("/goal") || body.contains("这条候选建议沉淀") {
        return None;
    }
    let kind = parsed
        .kind
        .as_deref()
        .map(normalize_approvable_candidate_kind)
        .unwrap_or_else(|| approvable_kind.to_string());
    let scope = parsed
        .scope
        .filter(|scope| is_valid_candidate_scope(scope))
        .unwrap_or_else(|| candidate.scope.clone());
    let tags = mature_tags(parsed.tags, candidate, &kind);
    let card_function = normalize_card_function(
        parsed.card_function.as_deref(),
        candidate,
        approvable_kind,
        synthesis,
    );
    let value_delta = normalize_value_delta(
        parsed.value_delta,
        candidate,
        &body,
        &card_function,
        synthesis,
    );
    let value_claim = parsed
        .value_claim
        .filter(|claim| claim.trim().chars().count() >= 8)
        .unwrap_or_else(|| {
            synthesize_value_claim(candidate, &value_delta, &card_function, synthesis)
        });
    let target_context =
        normalize_target_context(parsed.target_context, candidate, &card_function, synthesis);
    let skill_usefulness = synthesis.and_then(|review| review.proposal.skill_usefulness.clone());
    Some(MatureMemoryCardText {
        title,
        body,
        brief,
        kind,
        scope,
        tags,
        language: infer_language(&candidate.title, &candidate.body),
        card_function,
        value_claim,
        value_delta,
        target_context,
        skill_usefulness,
        synthesis_trace: synthesis_trace_for_candidate(candidate, true, synthesis),
    })
}

fn mature_candidate_deterministic(
    candidate: &CandidateRecord,
    approvable_kind: &str,
    synthesis: Option<&SynthesisReview>,
) -> MatureMemoryCardText {
    let language = infer_language(&candidate.title, &candidate.body);
    let title = mature_title(&candidate.title, &candidate.body);
    let body = if is_structured_memory_body(&candidate.body) {
        candidate.body.trim().to_string()
    } else if language == "zh" {
        render_structured_zh_body(candidate)
    } else {
        render_structured_en_body(candidate)
    };
    let brief = if is_mature_existing_brief(&candidate.brief) {
        candidate.brief.trim().to_string()
    } else {
        mature_brief(&title, &body, &language)
    };
    let card_function = normalize_card_function(None, candidate, approvable_kind, synthesis);
    let value_delta = normalize_value_delta(None, candidate, &body, &card_function, synthesis);
    let value_claim = synthesize_value_claim(candidate, &value_delta, &card_function, synthesis);
    let target_context = normalize_target_context(None, candidate, &card_function, synthesis);
    let skill_usefulness = synthesis.and_then(|review| review.proposal.skill_usefulness.clone());
    MatureMemoryCardText {
        title,
        body,
        brief,
        kind: approvable_kind.to_string(),
        scope: if is_valid_candidate_scope(&candidate.scope) {
            candidate.scope.clone()
        } else {
            "project".to_string()
        },
        tags: mature_tags(candidate.tags.clone(), candidate, approvable_kind),
        language,
        card_function,
        value_claim,
        value_delta,
        target_context,
        skill_usefulness,
        synthesis_trace: synthesis_trace_for_candidate(candidate, false, synthesis),
    }
}

fn extraction_with_synthesis_metadata(
    mut extraction: ExtractionMetadata,
    matured: &MatureMemoryCardText,
) -> ExtractionMetadata {
    extraction.card_function = Some(matured.card_function.clone());
    extraction.value_claim = Some(matured.value_claim.clone());
    extraction.value_delta = Some(matured.value_delta.clone());
    extraction.target_context = Some(matured.target_context.clone());
    extraction.skill_usefulness = matured.skill_usefulness.clone();
    extraction.synthesis_trace = matured.synthesis_trace.clone();
    extraction.synthesis_action = extraction
        .synthesis_action
        .clone()
        .or_else(|| Some(action_from_card_function(&matured.card_function).to_string()));
    extraction.synthesis_stop_reason = extraction
        .synthesis_stop_reason
        .clone()
        .or_else(|| Some(stop_reason_from_card_function(&matured.card_function).to_string()));
    if let Some(action) = extraction.suggested_action.as_mut() {
        action.rationale = Some(matured.value_claim.clone());
    } else if matured.card_function == "merge"
        && matured.target_context.target_type == "memory_card"
        && matured.target_context.target_id.is_some()
    {
        extraction.suggested_action = Some(ExtractionAction {
            action: "merge_into_existing".to_string(),
            route: "memory_card".to_string(),
            target_record: matured.target_context.target_id.clone(),
            compile_enabled: None,
            record_id: matured.target_context.target_id.clone(),
            similarity: None,
            reason: Some(matured.target_context.why_this_target.clone()),
            rationale: Some(matured.value_claim.clone()),
        });
    }
    extraction
}

fn apply_synthesis_review_to_extraction(
    extraction: &mut ExtractionMetadata,
    review: &SynthesisReview,
) {
    extraction.card_function = Some(review.proposal.card_function.clone());
    extraction.value_claim = Some(review.proposal.value_claim.clone());
    extraction.value_delta = Some(review.proposal.value_delta.clone());
    extraction.target_context = Some(review.proposal.target_context.clone());
    extraction.skill_usefulness = review.proposal.skill_usefulness.clone();
    extraction.synthesis_trace = synthesis_agent::review_to_trace(review);
    extraction.synthesis_action = Some(review.proposal.action.clone());
    extraction.synthesis_stop_reason = Some(review.stop_reason.as_str().to_string());
    if review.proposal.card_function == "merge"
        && review.proposal.merge_target_id.is_some()
        && extraction.suggested_action.is_none()
    {
        extraction.suggested_action = Some(ExtractionAction {
            action: "merge_into_existing".to_string(),
            route: "memory_card".to_string(),
            target_record: review.proposal.merge_target_id.clone(),
            compile_enabled: None,
            record_id: review.proposal.merge_target_id.clone(),
            similarity: Some(review.proposal.confidence),
            reason: Some(review.proposal.target_context.why_this_target.clone()),
            rationale: Some(review.proposal.value_claim.clone()),
        });
    } else if review.proposal.action == "already_covered" && extraction.suggested_action.is_none() {
        extraction.suggested_action = Some(ExtractionAction {
            action: "already_covered".to_string(),
            route: "memory_card".to_string(),
            target_record: review.proposal.target_context.target_id.clone(),
            compile_enabled: None,
            record_id: review.proposal.target_context.target_id.clone(),
            similarity: Some(review.proposal.confidence),
            reason: Some(review.proposal.target_context.why_this_target.clone()),
            rationale: Some(review.proposal.value_claim.clone()),
        });
    }
}

fn action_from_card_function(card_function: &str) -> &'static str {
    match card_function {
        "merge" => "merge_card",
        "skill_targeted" => "skill_targeted_card",
        "workflow" => "workflow_card",
        _ => "new_card",
    }
}

fn stop_reason_from_card_function(card_function: &str) -> &'static str {
    match card_function {
        "merge" => "merge_target_found",
        "skill_targeted" => "skill_gap_found",
        "workflow" => "workflow_gap_found",
        _ => "new_card_grounded",
    }
}

fn normalize_card_function(
    provider_value: Option<&str>,
    candidate: &CandidateRecord,
    approvable_kind: &str,
    synthesis: Option<&SynthesisReview>,
) -> String {
    let normalized = provider_value
        .map(str::trim)
        .filter(|value| matches!(*value, "library" | "skill_targeted" | "workflow" | "merge"));
    if let Some(value) = normalized {
        return value.to_string();
    }
    if let Some(review) = synthesis
        && matches!(
            review.proposal.card_function.as_str(),
            "library" | "skill_targeted" | "workflow" | "merge"
        )
    {
        return review.proposal.card_function.clone();
    }
    if candidate
        .extraction
        .suggested_action
        .as_ref()
        .is_some_and(|action| action.action == "merge_into_existing")
    {
        return "merge".to_string();
    }
    let text = format!(
        "{} {} {} {}",
        candidate.title,
        candidate.body,
        candidate.tags.join(" "),
        approvable_kind
    )
    .to_lowercase();
    if candidate
        .extraction
        .classification
        .as_ref()
        .is_some_and(|classification| classification.activation == "skill")
        || candidate
            .targets
            .iter()
            .any(|target| target.contains("skill"))
        || text.contains("skill")
        || text.contains("技能")
    {
        "skill_targeted".to_string()
    } else if matches!(approvable_kind, "workflow" | "procedure" | "template")
        || text.contains("workflow")
        || text.contains("工作流")
    {
        "workflow".to_string()
    } else {
        "library".to_string()
    }
}

fn normalize_value_delta(
    provider_value: Option<ValueDelta>,
    candidate: &CandidateRecord,
    body: &str,
    card_function: &str,
    synthesis: Option<&SynthesisReview>,
) -> ValueDelta {
    if let Some(delta) = provider_value
        && valid_value_delta(&delta)
    {
        return delta;
    }
    if let Some(review) = synthesis
        && valid_value_delta(&review.proposal.value_delta)
    {
        return review.proposal.value_delta.clone();
    }
    let existing_behavior = match card_function {
        "skill_targeted" => {
            "现有 Skill 或流程已有基础职责，但缺少这条历史反馈中的具体触发、边界或验收要求。"
        }
        "merge" => "已有相近 Memory Card 覆盖同一主题，需要更新为更成熟、更可执行的表达。",
        "workflow" => "现有项目工作流可被执行，但历史反馈显示仍需要更明确的操作约束。",
        _ => "现有项目规则库尚未稳定表达这条可复用偏好或约束。",
    };
    let missing_part = extract_missing_part(candidate, body);
    let new_behavior = extract_new_behavior(body);
    let why_not_duplicate = if card_function == "merge" {
        "该建议被标记为合并更新，价值在于改写或补强既有 Memory Card，而不是新增重复卡。"
    } else {
        "该卡必须提供比现有规则更具体的触发、动作或边界；若审阅时发现已完全覆盖，应选择合并或忽略。"
    };
    ValueDelta {
        existing_behavior: existing_behavior.to_string(),
        missing_part,
        new_behavior,
        why_not_duplicate: why_not_duplicate.to_string(),
    }
}

fn valid_value_delta(delta: &ValueDelta) -> bool {
    [
        &delta.existing_behavior,
        &delta.missing_part,
        &delta.new_behavior,
        &delta.why_not_duplicate,
    ]
    .iter()
    .all(|value| value.trim().chars().count() >= 8)
}

fn synthesize_value_claim(
    candidate: &CandidateRecord,
    value_delta: &ValueDelta,
    card_function: &str,
    synthesis: Option<&SynthesisReview>,
) -> String {
    if let Some(review) = synthesis
        && review.proposal.value_claim.trim().chars().count() >= 8
    {
        return review.proposal.value_claim.clone();
    }
    let target = match card_function {
        "skill_targeted" => "目标 Skill",
        "workflow" => "项目工作流",
        "merge" => "既有 Memory Card",
        _ => "项目规则库",
    };
    let missing = value_delta
        .missing_part
        .trim()
        .trim_end_matches(['。', '.', ';', '；']);
    if infer_language(&candidate.title, &candidate.body) == "zh" {
        format!("补强{target}中“{missing}”这一缺口，避免下次重复出现同类执行偏差。")
    } else {
        format!("Improves {target} by adding the missing behavior: {missing}.")
    }
}

fn normalize_target_context(
    provider_value: Option<TargetContext>,
    candidate: &CandidateRecord,
    card_function: &str,
    synthesis: Option<&SynthesisReview>,
) -> TargetContext {
    if let Some(context) = provider_value
        && !context.target_type.trim().is_empty()
        && !context.why_this_target.trim().is_empty()
    {
        return context;
    }
    if let Some(review) = synthesis
        && !review.proposal.target_context.target_type.trim().is_empty()
        && !review
            .proposal
            .target_context
            .why_this_target
            .trim()
            .is_empty()
    {
        return review.proposal.target_context.clone();
    }
    let target_type = match card_function {
        "skill_targeted" => "project_skill",
        "merge" => "memory_card",
        "workflow" => "workflow",
        _ => "workflow",
    };
    let target_id = if card_function == "merge" {
        candidate
            .extraction
            .suggested_action
            .as_ref()
            .and_then(|action| {
                action
                    .target_record
                    .clone()
                    .or_else(|| action.record_id.clone())
            })
    } else {
        candidate.targets.first().cloned()
    };
    let why_this_target = match card_function {
        "skill_targeted" => "这张 Memory Card 的价值在于补强项目级 Skill 的触发、指令或边界。",
        "merge" => "该候选与既有 Memory Card 主题重叠，应作为合并更新审阅。",
        "workflow" => "该候选描述的是重复项目工作流中的可执行改进。",
        _ => "该候选适合作为项目规则库中的独立 Memory Card 审阅。",
    };
    TargetContext {
        target_type: target_type.to_string(),
        target_id,
        why_this_target: why_this_target.to_string(),
    }
}

fn synthesis_trace_for_candidate(
    candidate: &CandidateRecord,
    provider_used: bool,
    synthesis: Option<&SynthesisReview>,
) -> Vec<SynthesisTraceEntry> {
    let mut trace = vec![
        SynthesisTraceEntry {
            step: "filter".to_string(),
            summary: "Kept because the candidate passed durable-memory review and reached the user Review Inbox.".to_string(),
            item_ids: Vec::new(),
        },
        SynthesisTraceEntry {
            step: "value_delta".to_string(),
            summary: "Compared against the candidate route and merge hints to require a concrete future behavior delta.".to_string(),
            item_ids: Vec::new(),
        },
    ];
    if candidate
        .extraction
        .suggested_action
        .as_ref()
        .is_some_and(|action| action.action == "merge_into_existing")
    {
        trace.push(SynthesisTraceEntry {
            step: "duplicate_check".to_string(),
            summary: "Existing Memory Card overlap found; approval should update the existing card instead of adding clutter.".to_string(),
            item_ids: Vec::new(),
        });
    }
    trace.push(SynthesisTraceEntry {
        step: "rewrite".to_string(),
        summary: if provider_used {
            "Provider rewrite produced a value-directed Memory Card proposal.".to_string()
        } else {
            "Deterministic fallback rendered a structured Memory Card proposal with value metadata."
                .to_string()
        },
        item_ids: Vec::new(),
    });
    if let Some(review) = synthesis {
        trace.extend(synthesis_agent::review_to_trace(review));
    }
    trace
}

fn extract_missing_part(candidate: &CandidateRecord, body: &str) -> String {
    if let Some(action) = candidate.extraction.suggested_action.as_ref()
        && action.action == "merge_into_existing"
    {
        return "既有卡片需要吸收新的触发、动作或边界表达".to_string();
    }
    body.lines()
        .find(|line| line.contains("边界") || line.to_lowercase().starts_with("boundary"))
        .map(|line| {
            line.replace("边界：", "")
                .replace("Boundary:", "")
                .trim()
                .chars()
                .take(80)
                .collect::<String>()
        })
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "缺少可复用的触发、动作和边界说明".to_string())
}

fn extract_new_behavior(body: &str) -> String {
    body.lines()
        .find(|line| line.contains("动作") || line.to_lowercase().starts_with("do:"))
        .map(|line| {
            line.replace("动作：", "")
                .replace("Do:", "")
                .trim()
                .chars()
                .take(96)
                .collect::<String>()
        })
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "下次遇到同类任务时先按卡片规则执行，并保留人工审阅边界。".to_string())
}

fn is_structured_memory_body(body: &str) -> bool {
    let lower = body.to_lowercase();
    (lower.contains("when") && lower.contains("do") && lower.contains("boundary"))
        || (body.contains("触发") && body.contains("动作") && body.contains("边界"))
}

fn render_structured_zh_body(candidate: &CandidateRecord) -> String {
    let source = clean_chatty_text(&candidate.body);
    let (trigger, action, boundary) = split_rule_parts(&source);
    format!(
        "触发：{}\n\n动作：{}\n\n边界：{}",
        trigger, action, boundary
    )
}

fn render_structured_en_body(candidate: &CandidateRecord) -> String {
    let source = clean_chatty_text(&candidate.body);
    let (trigger, action, boundary) = split_rule_parts(&source);
    format!(
        "When: {}\n\nDo: {}\n\nBoundary: {}",
        trigger, action, boundary
    )
}

fn split_rule_parts(source: &str) -> (String, String, String) {
    let normalized = source
        .trim()
        .trim_matches(['“', '”', '"', '\'', '。', '.'])
        .to_string();
    let boundary_markers = [
        "；边界是",
        "；目标是",
        "；目的是",
        "；原因是",
        "; boundary is",
        "; goal is",
    ];
    let mut core = normalized.as_str();
    let mut boundary =
        "仅在该规则与当前项目的长期工作方式一致、且证据充分时应用；涉及高影响取舍时保留人工确认。"
            .to_string();
    for marker in boundary_markers {
        if let Some((head, tail)) = normalized.split_once(marker) {
            core = head;
            boundary = tail.trim().trim_end_matches(['。', '.']).to_string();
            break;
        }
    }
    let (trigger, action) = core
        .split_once('，')
        .or_else(|| core.split_once(','))
        .map(|(when, what)| (when.trim(), what.trim()))
        .unwrap_or(("处理相关任务时", core.trim()));
    (
        ensure_when_clause(trigger),
        ensure_action_clause(action),
        boundary,
    )
}

fn ensure_when_clause(value: &str) -> String {
    let trimmed = value.trim().trim_end_matches('时');
    if trimmed.is_empty() {
        return "处理相关任务时".to_string();
    }
    if trimmed.starts_with("当") || trimmed.starts_with("在") {
        format!("{trimmed}时")
    } else if trimmed.to_lowercase().starts_with("when ") {
        trimmed.to_string()
    } else {
        format!("当{trimmed}时")
    }
}

fn ensure_action_clause(value: &str) -> String {
    let trimmed = value.trim().trim_end_matches(['。', '.', ';', '；']);
    if trimmed.is_empty() {
        "先提炼可执行规则，再进行实现或同步".to_string()
    } else {
        trimmed.to_string()
    }
}

fn clean_chatty_text(value: &str) -> String {
    value
        .replace("/goal", "")
        .replace("不要问我了，", "")
        .replace("这条候选建议沉淀了", "")
        .replace(['“', '”'], "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn mature_title(title: &str, body: &str) -> String {
    let cleaned = clean_chatty_text(title);
    let source = if cleaned.trim().is_empty() {
        clean_chatty_text(body)
    } else {
        cleaned
    };
    let title = source
        .split(['：', ':', '；', ';', '。', '.', '，', ','])
        .next()
        .unwrap_or(source.as_str())
        .trim()
        .trim_start_matches("当")
        .trim_start_matches("在")
        .trim_end_matches("时")
        .to_string();
    title.chars().take(30).collect::<String>()
}

fn mature_brief(title: &str, body: &str, language: &str) -> String {
    let summary = body
        .lines()
        .find(|line| line.contains("动作") || line.to_lowercase().starts_with("do:"))
        .unwrap_or(body)
        .replace("动作：", "")
        .replace("Do:", "")
        .trim()
        .chars()
        .take(54)
        .collect::<String>()
        .trim()
        .trim_end_matches(['。', '.', ';', '；'])
        .to_string();
    if language == "zh" {
        format!("用于在“{title}”场景下执行一致、可审阅的处理方式：{summary}。")
    } else {
        format!("Use this for consistent, reviewable handling of {title}: {summary}.")
    }
}

fn is_mature_existing_brief(brief: &str) -> bool {
    let trimmed = brief.trim();
    !trimmed.is_empty()
        && trimmed.chars().count() >= 8
        && !trimmed.contains("这条候选建议沉淀")
        && !trimmed.contains("以后遇到类似任务，可以复用")
        && !trimmed.contains("/goal")
}

fn mature_tags(
    mut tags: Vec<String>,
    candidate: &CandidateRecord,
    approvable_kind: &str,
) -> Vec<String> {
    tags.retain(|tag| !tag.trim().is_empty());
    if tags.is_empty() {
        tags = infer_candidate_tags(
            &format!(
                "{}\n{}\n{}",
                candidate.title, candidate.body, approvable_kind
            )
            .to_lowercase(),
            approvable_kind,
        );
    }
    if !tags.iter().any(|tag| tag == approvable_kind) {
        tags.push(approvable_kind.to_string());
    }
    tags.sort();
    tags.dedup();
    tags.truncate(8);
    tags
}

fn is_valid_candidate_scope(scope: &str) -> bool {
    matches!(
        scope,
        "project" | "global" | "agent" | "directory" | "agent-specific"
    )
}

fn normalize_approvable_candidate_kind(kind: &str) -> String {
    match kind {
        "preference" | "constraint" | "procedure" | "convention" | "correction"
        | "anti-pattern" | "rule" | "memory_card" | "observation" | "package" => kind.to_string(),
        "principle" | "decision" | "supplement" => "procedure".to_string(),
        _ => "procedure".to_string(),
    }
}

fn update_candidate_status(
    project_root: &Path,
    id: &str,
    status: CandidateStatus,
    rejected_reason: Option<String>,
) -> Result<CandidateRecord> {
    let root = fsutil::normalize_project_root(project_root)?;
    let mut candidate = load_candidate(&root, id)?;
    candidate.status = status;
    candidate.rejected_reason = rejected_reason;
    candidate.updated_at = Utc::now().to_rfc3339();
    save_candidate(&root, &candidate)?;
    Ok(candidate)
}

fn load_candidate(project_root: &Path, id: &str) -> Result<CandidateRecord> {
    let path = candidate_path(project_root, id)?;
    if !path.exists() {
        return Err(anyhow!("candidate `{id}` does not exist"));
    }
    let mut candidate: CandidateRecord = serde_yaml::from_str(&fs::read_to_string(path)?)?;
    enrich_record_defaults(&mut candidate);
    Ok(candidate)
}

fn save_candidate(project_root: &Path, candidate: &CandidateRecord) -> Result<()> {
    let path = candidate_path(project_root, &candidate.id)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_yaml::to_string(candidate)?)?;
    Ok(())
}

fn candidates_dir(project_root: &Path) -> PathBuf {
    config::kernel_dir(project_root).join("candidates")
}

fn candidate_scope_dir(project_root: &Path, scope: &str) -> PathBuf {
    let normalized = match scope {
        "global" => "global",
        "agent" => "agent",
        _ => "project",
    };
    candidates_dir(project_root).join(normalized)
}

fn candidate_path(project_root: &Path, id: &str) -> Result<PathBuf> {
    for scope in ["project", "global", "agent"] {
        let path = candidate_path_for_scope(project_root, scope, id)?;
        if path.exists() {
            return Ok(path);
        }
    }
    candidate_path_for_scope(project_root, "project", id)
}

fn candidate_path_for_scope(project_root: &Path, scope: &str, id: &str) -> Result<PathBuf> {
    let safe = id.replace(['/', '\\', ':'], "-");
    Ok(candidate_scope_dir(project_root, scope).join(format!("{safe}.yml")))
}

fn enrich_record_defaults(candidate: &mut CandidateRecord) {
    if candidate.operation == MemoryCardOperation::Add {
        candidate.operation = operation_from_extraction(&candidate.extraction);
    }
    if candidate.duplicate_of.is_none() {
        candidate.duplicate_of = candidate
            .extraction
            .suggested_action
            .as_ref()
            .and_then(|action| action.target_record.clone());
    }
    if !candidate.brief.trim().is_empty() && !candidate.tags.is_empty() {
        return;
    }
    let metadata = infer_candidate_metadata(
        &candidate.title,
        &candidate.body,
        &candidate.kind,
        candidate.matched_template.as_deref(),
        candidate.reason.as_deref(),
    );
    if candidate.brief.trim().is_empty() {
        candidate.brief = metadata.brief;
    }
    if candidate.tags.is_empty() {
        candidate.tags = metadata.tags;
    }
    if candidate.language.trim().is_empty() {
        candidate.language = metadata.language;
    }
}

fn operation_from_extraction(extraction: &ExtractionMetadata) -> MemoryCardOperation {
    let Some(action) = extraction.suggested_action.as_ref() else {
        return MemoryCardOperation::Add;
    };
    match action.action.as_str() {
        "merge_into_existing" => MemoryCardOperation::Update,
        "supersede" => MemoryCardOperation::Supersede,
        "conflict" => MemoryCardOperation::Conflict,
        "noop" => MemoryCardOperation::Noop,
        _ => MemoryCardOperation::Add,
    }
}

#[derive(Debug, Clone)]
struct CandidateMetadata {
    brief: String,
    tags: Vec<String>,
    language: String,
}

fn infer_candidate_metadata(
    title: &str,
    body: &str,
    kind: &str,
    matched_template: Option<&str>,
    reason: Option<&str>,
) -> CandidateMetadata {
    let lower = format!(
        "{title}\n{body}\n{kind}\n{}\n{}",
        matched_template.unwrap_or(""),
        reason.unwrap_or("")
    )
    .to_lowercase();
    let tags = infer_candidate_tags(&lower, kind);
    CandidateMetadata {
        brief: infer_chinese_brief(title, body, &lower),
        tags,
        language: infer_language(title, body),
    }
}

fn infer_candidate_tags(lower: &str, kind: &str) -> Vec<String> {
    let mut tags = Vec::new();
    let mappings = [
        (
            "frontend",
            ["frontend", "front-end", "前端", "react", "vite"].as_slice(),
        ),
        (
            "http",
            ["http", "request", "请求", "api client", "axios"].as_slice(),
        ),
        ("axios", ["axios"].as_slice()),
        (
            "js",
            ["javascript", "typescript", "node", "vite"].as_slice(),
        ),
        ("tooling", ["tooling", "cli", "cargo"].as_slice()),
        (
            "structured-data",
            [
                "structured data",
                "structured api",
                "结构化",
                "json",
                "yaml",
                "toml",
            ]
            .as_slice(),
        ),
        (
            "parser",
            ["parser", "parse", "解析器", "structured api"].as_slice(),
        ),
        (
            "robustness",
            ["robust", "脆弱", "string manipulation", "字符串"].as_slice(),
        ),
        (
            "通用范式",
            ["paradigm", "pattern", "structured api", "跨语言"].as_slice(),
        ),
        (
            "agent-behavior",
            ["agent", "智能体", "codex", "claude", "tool call"].as_slice(),
        ),
        (
            "tool-use",
            ["tool", "invoke", "parallel", "并行", "工具"].as_slice(),
        ),
        ("parallelism", ["parallel", "并行"].as_slice()),
        (
            "efficiency",
            ["efficiency", "性能", "效率", "faster"].as_slice(),
        ),
        (
            "autonomy",
            ["proactive", "主动", "autonomy", "judgment"].as_slice(),
        ),
        (
            "decision-making",
            ["judgment", "取舍", "decision", "判断"].as_slice(),
        ),
        (
            "meta-instruction",
            ["instruction", "prompt", "元指令", "developer"].as_slice(),
        ),
        (
            "architecture",
            ["architecture", "架构", "boundary", "service"].as_slice(),
        ),
        (
            "performance",
            ["performance", "卡顿", "startup", "启动", "slow"].as_slice(),
        ),
        (
            "testing",
            ["test", "testing", "测试", "clippy", "vitest"].as_slice(),
        ),
        ("safety", ["safety", "安全", "policy", "secret"].as_slice()),
    ];
    for (tag, markers) in mappings {
        if markers.iter().any(|marker| lower.contains(marker)) {
            tags.push(tag.to_string());
        }
    }
    if tags.iter().any(|tag| tag == "axios") {
        tags.push("生态偏好".to_string());
    }
    if tags.is_empty() {
        tags.push(kind.to_string());
    }
    tags.sort();
    tags.dedup();
    tags.truncate(6);
    tags
}

fn infer_chinese_brief(title: &str, body: &str, lower: &str) -> String {
    if lower.contains("axios") {
        return "前端 HTTP 请求优先使用 Axios，统一请求库和调用风格。".to_string();
    }
    if lower.contains("structured api")
        || lower.contains("structured data")
        || lower.contains("string manipulation")
        || lower.contains("parser")
    {
        return "处理结构化数据时优先使用解析器或结构化 API，避免脆弱字符串拼接。".to_string();
    }
    if lower.contains("parallel") || lower.contains("并行") {
        return "独立的读取、搜索、检查类工具调用应尽量并行，提高智能体执行效率。".to_string();
    }
    if lower.contains("proactive") || lower.contains("judgment") || lower.contains("主动") {
        return "智能体执行任务时应主动判断和提出取舍，而不是只被动执行指令。".to_string();
    }
    format!(
        "这条候选建议沉淀了“{}”：{}",
        title.trim(),
        concise_summary(body)
    )
}

fn infer_language(title: &str, body: &str) -> String {
    let combined = format!("{title}\n{body}");
    if combined
        .chars()
        .any(|ch| ('\u{4e00}'..='\u{9fff}').contains(&ch))
    {
        "zh".to_string()
    } else {
        "en".to_string()
    }
}

fn concise_summary(body: &str) -> String {
    let trimmed = body.trim();
    let summary = trimmed.chars().take(72).collect::<String>();
    summary
        .trim()
        .trim_end_matches(['。', '.', ';'])
        .to_string()
}

fn default_language() -> String {
    "zh".to_string()
}

#[cfg(test)]
mod tests;
