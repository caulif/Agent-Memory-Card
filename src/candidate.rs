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
use crate::textutil;

mod action;

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
            &candidate.title,
            &candidate.body,
        ) {
            let updated = memory_card::update_memory_card_from_review(
                &root,
                &candidate.id,
                candidate.title.clone(),
                candidate.body.clone(),
                candidate.brief.clone(),
                candidate.tags.clone(),
                candidate.language.clone(),
                approvable_kind.clone(),
                candidate.scope.clone(),
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
        &candidate.title,
        &candidate.body,
        &approvable_kind,
        &candidate.scope,
        candidate.targets.clone(),
        Some(candidate.extraction.clone()),
        Some(candidate.id.clone()),
        Some(candidate.evidence.clone()),
    )?;
    let memory_card = memory_card::update_memory_card(
        &root,
        &candidate.id,
        MemoryCardUpdate {
            brief: Some(candidate.brief.clone()),
            tags: Some(candidate.tags.clone()),
            language: Some(candidate.language.clone()),
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
mod tests {
    use super::*;

    fn new_candidate(id: &str, confidence: f32, matched_template: Option<&str>) -> NewCandidate {
        NewCandidate {
            id: id.to_string(),
            title: id.to_string(),
            kind: "preference".to_string(),
            scope: "project".to_string(),
            body: "Use stable project preferences.".to_string(),
            brief: None,
            tags: Vec::new(),
            language: None,
            targets: vec!["codex".to_string()],
            evidence: "test".to_string(),
            confidence: Some(confidence),
            reason: Some("test".to_string()),
            matched_template: matched_template.map(str::to_string),
            source_observations: vec![format!("obs:{id}")],
            extraction: ExtractionMetadata::default(),
        }
    }

    #[test]
    fn visible_candidates_exclude_hidden_rejected_and_promoted_records() {
        let temp = tempfile::tempdir().expect("tempdir");
        add_candidate(
            temp.path(),
            new_candidate("keep", 0.95, Some("prefer-tool")),
        )
        .expect("keep");
        add_candidate(temp.path(), new_candidate("hide", 0.9, None)).expect("hide");
        add_candidate(temp.path(), new_candidate("reject", 0.88, None)).expect("reject");
        add_candidate(temp.path(), new_candidate("promote", 0.86, None)).expect("promote");

        hide_candidate(temp.path(), "hide").expect("hidden");
        reject_candidate(temp.path(), "reject", Some("not useful".to_string())).expect("rejected");
        approve_candidate_to_memory_card(temp.path(), "promote").expect("promoted");

        let visible = list_visible_candidates(temp.path()).expect("visible");

        assert_eq!(
            visible
                .iter()
                .map(|candidate| candidate.id.as_str())
                .collect::<Vec<_>>(),
            vec!["keep"]
        );
    }

    #[test]
    fn candidate_review_actions_record_feedback() {
        let temp = tempfile::tempdir().expect("tempdir");
        add_candidate(temp.path(), new_candidate("reject", 0.88, None)).expect("reject");
        add_candidate(temp.path(), new_candidate("approve", 0.9, None)).expect("approve");

        reject_candidate(temp.path(), "reject", Some("too generic".to_string()))
            .expect("reject candidate");
        approve_candidate_to_memory_card(temp.path(), "approve").expect("approve candidate");

        let events = feedback::load_feedback(temp.path()).expect("feedback");

        assert_eq!(events.len(), 2);
        assert!(events.iter().any(|event| event.decision == "rejected"));
        assert!(events.iter().any(|event| event.decision == "approved"));
        assert!(
            events
                .iter()
                .any(|event| event.reason.as_deref() == Some("too generic"))
        );
    }

    #[test]
    fn candidates_sort_by_confidence_template_and_update_time() {
        let temp = tempfile::tempdir().expect("tempdir");
        add_candidate(temp.path(), new_candidate("plain", 0.9, None)).expect("plain");
        add_candidate(
            temp.path(),
            new_candidate("templated", 0.9, Some("prefer-tool")),
        )
        .expect("templated");
        add_candidate(temp.path(), new_candidate("low", 0.4, None)).expect("low");

        let candidates = load_candidates(temp.path()).expect("candidates");

        assert_eq!(candidates[0].id, "templated");
        assert_eq!(candidates[1].id, "plain");
        assert_eq!(candidates[2].id, "low");
    }

    #[test]
    fn candidates_get_chinese_brief_and_specific_tags() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut axios = new_candidate("axios", 0.92, Some("prefer-tool"));
        axios.title = "Use Axios".to_string();
        axios.body = "Use Axios for frontend HTTP requests.".to_string();
        add_candidate(temp.path(), axios).expect("axios");

        let mut structured = new_candidate("structured", 0.9, Some("coding-pattern"));
        structured.title = "Prefer structured APIs over string manipulation".to_string();
        structured.body =
            "Use parsers or structured APIs instead of ad hoc string manipulation.".to_string();
        add_candidate(temp.path(), structured).expect("structured");

        let candidates = load_candidates(temp.path()).expect("candidates");
        let axios = candidates
            .iter()
            .find(|candidate| candidate.id == "axios")
            .expect("axios candidate");
        assert!(axios.brief.contains("前端 HTTP 请求优先使用 Axios"));
        assert!(axios.tags.contains(&"axios".to_string()));
        assert!(axios.tags.contains(&"frontend".to_string()));
        assert!(axios.tags.contains(&"生态偏好".to_string()));

        let structured = candidates
            .iter()
            .find(|candidate| candidate.id == "structured")
            .expect("structured candidate");
        assert!(structured.brief.contains("结构化数据"));
        assert!(structured.tags.contains(&"structured-data".to_string()));
        assert!(structured.tags.contains(&"parser".to_string()));
    }

    #[test]
    fn approving_candidate_creates_memory_card_and_preserves_metadata() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut candidate = new_candidate("project:use-axios", 0.92, Some("prefer-tool"));
        candidate.title = "Use Axios".to_string();
        candidate.body = "Use Axios for frontend HTTP requests.".to_string();
        add_candidate(temp.path(), candidate).expect("candidate");

        let memory_card = approve_candidate_to_memory_card(temp.path(), "project:use-axios")
            .expect("memory_card");
        let visible = list_visible_candidates(temp.path()).expect("visible candidates");

        assert_eq!(memory_card.id, "project:use-axios");
        assert!(memory_card.brief.contains("前端 HTTP 请求优先使用 Axios"));
        assert!(memory_card.tags.contains(&"axios".to_string()));
        assert!(visible.is_empty());
        assert!(draft::load_drafts(temp.path()).expect("drafts").is_empty());
    }

    #[test]
    fn approving_legacy_principle_candidate_normalizes_kind() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut candidate =
            new_candidate("global:legacy-principle", 0.92, Some("principle-signal"));
        candidate.title = "Legacy Principle".to_string();
        candidate.body = "When planning durable changes, verify the main workflow before polishing secondary details.".to_string();
        candidate.kind = "principle".to_string();
        candidate.scope = "global".to_string();
        add_candidate(temp.path(), candidate).expect("candidate");

        let memory_card = approve_candidate_to_memory_card(temp.path(), "global:legacy-principle")
            .expect("memory_card");

        assert_eq!(memory_card.kind, "procedure");
        assert_eq!(memory_card.scope, "global");
    }
}
