use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::config;
use crate::draft::{self, DraftUpdate, NewDraft};
use crate::extract::classify::KnowledgeClassification;
use crate::fsutil;
use crate::skilllet::{self, SkillletRecord, SkillletUpdate};
use crate::textutil;

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
        extraction: candidate.extraction,
        status: CandidateStatus::Candidate,
        rejected_reason: None,
        created_at: now.clone(),
        updated_at: now,
    };
    let path = candidate_path(&root, &candidate.id)?;
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

pub fn hide_candidate(project_root: &Path, id: &str) -> Result<CandidateRecord> {
    update_candidate_status(project_root, id, CandidateStatus::Hidden, None)
}

pub fn reject_candidate(
    project_root: &Path,
    id: &str,
    rejected_reason: Option<String>,
) -> Result<CandidateRecord> {
    update_candidate_status(project_root, id, CandidateStatus::Rejected, rejected_reason)
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
    Ok(draft)
}

pub fn approve_candidate_to_skilllet(project_root: &Path, id: &str) -> Result<SkillletRecord> {
    let root = fsutil::normalize_project_root(project_root)?;
    let mut candidate = load_candidate(&root, id)?;
    if candidate.status != CandidateStatus::Candidate {
        return Err(anyhow!(
            "candidate `{id}` cannot be approved from status {:?}",
            candidate.status
        ));
    }
    // 检查同名 skilllet 是否已存在；若 body 语义相似则直接标记候选为已提升
    if let Some(existing_skilllet) = skilllet::load_skilllets(&root)?
        .into_iter()
        .find(|skilllet| skilllet.id == candidate.id)
    {
        if textutil::jaccard_similarity(&candidate.body, &existing_skilllet.body) >= 0.75 {
            candidate.status = CandidateStatus::Promoted;
            candidate.updated_at = Utc::now().to_rfc3339();
            save_candidate(&root, &candidate)?;
            return Ok(existing_skilllet);
        }
        return Err(anyhow!(
            "skilllet id conflict for `{}` (different body); review or merge the existing Skilllet before approving this system suggestion",
            candidate.id
        ));
    }

    skilllet::add_skilllet(
        &root,
        &candidate.id,
        &candidate.title,
        &candidate.body,
        &candidate.kind,
        &candidate.scope,
        candidate.targets.clone(),
    )?;
    let skilllet = skilllet::update_skilllet(
        &root,
        &candidate.id,
        SkillletUpdate {
            brief: Some(candidate.brief.clone()),
            tags: Some(candidate.tags.clone()),
            language: Some(candidate.language.clone()),
            ..SkillletUpdate::default()
        },
    )?;
    candidate.status = CandidateStatus::Promoted;
    candidate.updated_at = Utc::now().to_rfc3339();
    save_candidate(&root, &candidate)?;
    Ok(skilllet)
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
    config::kernel_dir(project_root)
        .join("candidates")
        .join("project")
}

fn candidate_path(project_root: &Path, id: &str) -> Result<PathBuf> {
    let safe = id.replace(['/', '\\', ':'], "-");
    Ok(candidates_dir(project_root).join(format!("{safe}.yml")))
}

fn enrich_record_defaults(candidate: &mut CandidateRecord) {
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
            ["javascript", "typescript", "node", "bun", "npm", "vite"].as_slice(),
        ),
        ("bun", ["bun"].as_slice()),
        (
            "package-manager",
            ["package manager", "包管理", "scripts", "package management"].as_slice(),
        ),
        (
            "tooling",
            ["tooling", "scripts", "cli", "bun", "cargo"].as_slice(),
        ),
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
    if tags.iter().any(|tag| tag == "axios" || tag == "bun") {
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
    if lower.contains("bun") {
        return "JavaScript 包管理和脚本执行优先使用 Bun。".to_string();
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
        approve_candidate_to_skilllet(temp.path(), "promote").expect("promoted");

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
    fn approving_candidate_creates_skilllet_and_preserves_metadata() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut candidate = new_candidate("project:prefer-bun", 0.92, Some("prefer-tool"));
        candidate.title = "Prefer Bun".to_string();
        candidate.body = "Use Bun for JavaScript package management and scripts.".to_string();
        add_candidate(temp.path(), candidate).expect("candidate");

        let skilllet =
            approve_candidate_to_skilllet(temp.path(), "project:prefer-bun").expect("skilllet");
        let visible = list_visible_candidates(temp.path()).expect("visible candidates");

        assert_eq!(skilllet.id, "project:prefer-bun");
        assert!(skilllet.brief.contains("JavaScript 包管理"));
        assert!(skilllet.tags.contains(&"bun".to_string()));
        assert!(visible.is_empty());
        assert!(draft::load_drafts(temp.path()).expect("drafts").is_empty());
    }
}
