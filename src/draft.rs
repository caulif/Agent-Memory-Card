use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use crate::candidate::ExtractionMetadata;
use crate::config;
use crate::fsutil;
use crate::provider::{self, ProviderJsonSchema, ProviderRequest};
use crate::skilllet;
use crate::textutil;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftRecord {
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
    pub targets: Vec<String>,
    pub evidence: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_template: Option<String>,
    #[serde(default)]
    pub extraction: ExtractionMetadata,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct NewDraft {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub scope: String,
    pub body: String,
    pub targets: Vec<String>,
    pub evidence: String,
    pub confidence: Option<f32>,
    pub reason: Option<String>,
    pub matched_template: Option<String>,
    pub extraction: ExtractionMetadata,
}

#[derive(Debug, Clone, Default)]
pub struct DraftUpdate {
    pub title: Option<String>,
    pub body: Option<String>,
    pub brief: Option<String>,
    pub tags: Option<Vec<String>>,
    pub language: Option<String>,
    pub kind: Option<String>,
    pub scope: Option<String>,
    pub targets: Option<Vec<String>>,
}

pub fn add_draft(project_root: &Path, draft: NewDraft) -> Result<()> {
    let root = fsutil::normalize_project_root(project_root)?;
    config::ensure_kernel_dir(&root)?;
    let now = Utc::now().to_rfc3339();
    let metadata = infer_draft_metadata(&draft.title, &draft.body, &draft.kind);
    let record = DraftRecord {
        schema_version: current_schema_version(),
        id: draft.id.clone(),
        title: draft.title,
        kind: draft.kind,
        scope: draft.scope,
        body: draft.body,
        brief: metadata.brief,
        tags: metadata.tags,
        language: metadata.language,
        targets: draft.targets,
        evidence: draft.evidence,
        confidence: draft.confidence,
        reason: draft.reason,
        matched_template: draft.matched_template,
        extraction: draft.extraction,
        status: "draft".to_string(),
        created_at: now.clone(),
        updated_at: now,
    };
    let path = draft_path(&root, &draft.id)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_yaml::to_string(&record)?)?;
    Ok(())
}

#[derive(Debug, Clone)]
struct DraftMetadata {
    brief: String,
    tags: Vec<String>,
    language: String,
}

fn infer_draft_metadata(title: &str, body: &str, kind: &str) -> DraftMetadata {
    let language = infer_language(title, body);
    let tags = infer_tags(title, body, kind);
    let brief = if language == "zh" {
        format!("这条草稿记录了“{}”：{}", title, concise_zh_summary(body))
    } else {
        format!("这条草稿记录了 \"{}\"：{}", title, concise_en_summary(body))
    };
    DraftMetadata {
        brief,
        tags,
        language,
    }
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

fn infer_tags(title: &str, body: &str, kind: &str) -> Vec<String> {
    let lower = format!("{title}\n{body}\n{kind}").to_lowercase();
    let mut tags = Vec::new();
    let mappings = [
        (
            "ui-design",
            ["ui", "界面", "设计", "视觉", "按钮", "组件"].as_slice(),
        ),
        (
            "frontend",
            ["frontend", "前端", "react", "typescript", "axios"].as_slice(),
        ),
        ("backend", ["backend", "后端", "api", "server"].as_slice()),
        ("rust", ["rust", "cargo", "clippy"].as_slice()),
        ("tauri", ["tauri", "desktop", "桌面"].as_slice()),
        (
            "testing",
            ["test", "testing", "vitest", "playwright", "测试"].as_slice(),
        ),
        (
            "performance",
            ["performance", "性能", "卡顿", "卡死", "responsive"].as_slice(),
        ),
        (
            "agent-handoff",
            ["claude", "codex", "agent", "智能体", "交接"].as_slice(),
        ),
        (
            "code-style",
            ["style", "lint", "格式", "代码风格"].as_slice(),
        ),
        (
            "workflow",
            ["workflow", "流程", "步骤", "procedure", "checklist"].as_slice(),
        ),
        (
            "safety",
            ["safety", "安全", "secret", "redact", "禁止"].as_slice(),
        ),
    ];
    for (tag, markers) in mappings {
        if markers.iter().any(|marker| lower.contains(marker)) {
            tags.push(tag.to_string());
        }
    }
    if tags.is_empty() {
        tags.push(kind.to_string());
    }
    tags.sort();
    tags.dedup();
    tags
}

fn concise_zh_summary(body: &str) -> String {
    body.trim()
        .chars()
        .take(88)
        .collect::<String>()
        .trim()
        .trim_end_matches(['。', '.', ';'])
        .to_string()
}

fn concise_en_summary(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.len() <= 120 {
        return trimmed.trim_end_matches(['.', ';']).to_string();
    }
    trimmed.chars().take(120).collect::<String>()
}

fn default_language() -> String {
    "zh".to_string()
}

fn current_schema_version() -> u32 {
    1
}

pub fn load_drafts(project_root: &Path) -> Result<Vec<DraftRecord>> {
    let root = fsutil::normalize_project_root(project_root)?;
    let dir = config::kernel_dir(&root).join("drafts");
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut records: Vec<DraftRecord> = Vec::new();
    for entry in walkdir::WalkDir::new(dir).follow_links(false) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.path().extension().and_then(|value| value.to_str()) != Some("yml") {
            continue;
        }
        records.push(serde_yaml::from_str(&fs::read_to_string(entry.path())?)?);
    }
    records.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(records)
}

pub fn load_reviewable_drafts(project_root: &Path) -> Result<Vec<DraftRecord>> {
    let root = fsutil::normalize_project_root(project_root)?;
    let existing = ExistingSkillletIndex::load(&root)?;
    Ok(load_drafts(&root)?
        .into_iter()
        .filter(|draft| !existing.represents_draft(draft))
        .collect())
}

pub fn approve_draft(project_root: &Path, id: &str) -> Result<()> {
    let root = fsutil::normalize_project_root(project_root)?;
    let path = draft_path(&root, id)?;
    if !path.exists() {
        return Err(anyhow!("draft `{id}` does not exist"));
    }
    let draft: DraftRecord = serde_yaml::from_str(&fs::read_to_string(&path)?)?;
    // 检查同名 skilllet 是否已存在；若是同一概念的更新，则吸收到现有 Skilllet。
    let existing_skilllets = skilllet::load_skilllets(&root)?;
    if let Some(existing_skilllet) = existing_skilllets
        .iter()
        .find(|skilllet| skilllet.id == draft.id)
    {
        if textutil::jaccard_similarity(&draft.body, &existing_skilllet.body) >= 0.75 {
            fs::remove_file(path)?;
            return Ok(());
        }
        if skilllet::review_update_matches_existing(existing_skilllet, &draft.title, &draft.body) {
            skilllet::update_skilllet_from_review(
                &root,
                &draft.id,
                draft.title.clone(),
                draft.body.clone(),
                draft.brief.clone(),
                draft.tags.clone(),
                draft.language.clone(),
                draft.kind.clone(),
                draft.scope.clone(),
            )?;
            fs::remove_file(path)?;
            return Ok(());
        }
        return Err(anyhow!(
            "skilllet id conflict for `{}` (different body); review or merge the existing Skilllet before approving this Draft",
            draft.id
        ));
    }
    let existing_index = ExistingSkillletIndex::from_records(&root, &existing_skilllets)?;
    if existing_index.represents_draft(&draft) {
        fs::remove_file(path)?;
        return Ok(());
    }
    skilllet::add_skilllet_with_provenance(
        &root,
        &draft.id,
        &draft.title,
        &draft.body,
        &draft.kind,
        &draft.scope,
        draft.targets,
        Some(draft.extraction.clone()),
        Some(draft.id.clone()),
        Some(draft.evidence.clone()),
    )?;
    fs::remove_file(path)?;
    Ok(())
}

#[derive(Debug, Default)]
struct ExistingSkillletIndex {
    ids: BTreeSet<String>,
    id_slugs: BTreeSet<String>,
    title_slugs: BTreeSet<String>,
    bodies: Vec<String>,
}

impl ExistingSkillletIndex {
    fn load(project_root: &Path) -> Result<Self> {
        let skilllets = skilllet::load_skilllets(project_root)?;
        Self::from_records(project_root, &skilllets)
    }

    fn from_records(project_root: &Path, skilllets: &[skilllet::SkillletRecord]) -> Result<Self> {
        let mut index = Self::default();
        for skilllet in skilllets {
            index.add_id(&skilllet.id);
            index.title_slugs.insert(review_title_slug(&skilllet.title));
            index.bodies.push(skilllet.body.clone());
        }

        let project = config::load_or_default_project_config(project_root)?;
        for included in project.skilllets.include {
            index.add_id(&included.id);
        }

        Ok(index)
    }

    fn add_id(&mut self, id: &str) {
        self.ids.insert(id.to_string());
        self.id_slugs.insert(review_id_slug(id));
        for variant in id_scope_variants(id) {
            self.ids.insert(variant);
        }
    }

    fn represents_draft(&self, draft: &DraftRecord) -> bool {
        if self.ids.contains(&draft.id) {
            return true;
        }
        if id_scope_variants(&draft.id)
            .iter()
            .any(|variant| self.ids.contains(variant))
        {
            return true;
        }
        let id_slug = review_id_slug(&draft.id);
        if !id_slug.is_empty() && self.id_slugs.contains(&id_slug) {
            return true;
        }
        let title_slug = review_title_slug(&draft.title);
        if !title_slug.is_empty()
            && (self.title_slugs.contains(&title_slug) || self.id_slugs.contains(&title_slug))
        {
            return true;
        }
        self.bodies
            .iter()
            .any(|body| body_matches_existing(body, &draft.body))
    }
}

fn id_scope_variants(id: &str) -> Vec<String> {
    let Some((scope, rest)) = id.split_once(':') else {
        return Vec::new();
    };
    match scope {
        "project" => vec![format!("global:{rest}")],
        "global" => vec![format!("project:{rest}")],
        _ => Vec::new(),
    }
}

fn review_id_slug(id: &str) -> String {
    let raw = id
        .split_once(':')
        .map(|(_, rest)| rest)
        .unwrap_or(id)
        .trim();
    review_slug(raw)
}

fn review_title_slug(title: &str) -> String {
    review_slug(title)
}

fn review_slug(value: &str) -> String {
    let mut text = value.trim();
    for suffix in [" Fusion", " fusion", "-fusion", " 融合", "融合"] {
        if let Some(stripped) = text.strip_suffix(suffix) {
            text = stripped.trim();
            break;
        }
    }
    textutil::slug(text)
}

fn body_matches_existing(existing: &str, draft: &str) -> bool {
    if textutil::jaccard_similarity(existing, draft) >= 0.72 {
        return true;
    }
    let left = compact_text(existing);
    let right = compact_text(draft);
    left.chars().count() >= 24
        && right.chars().count() >= 24
        && (left.contains(&right) || right.contains(&left))
}

fn compact_text(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || ('\u{4e00}'..='\u{9fff}').contains(ch))
        .flat_map(char::to_lowercase)
        .collect()
}

pub fn reject_draft(project_root: &Path, id: &str) -> Result<()> {
    let root = fsutil::normalize_project_root(project_root)?;
    let path = draft_path(&root, id)?;
    if !path.exists() {
        return Err(anyhow!("draft `{id}` does not exist"));
    }
    fs::remove_file(path)?;
    Ok(())
}

pub fn update_draft(project_root: &Path, id: &str, update: DraftUpdate) -> Result<DraftRecord> {
    let root = fsutil::normalize_project_root(project_root)?;
    let path = draft_path(&root, id)?;
    if !path.exists() {
        return Err(anyhow!("draft `{id}` does not exist"));
    }
    let mut draft: DraftRecord = serde_yaml::from_str(&fs::read_to_string(&path)?)?;
    if let Some(title) = update.title {
        draft.title = title;
    }
    if let Some(body) = update.body {
        draft.body = body;
    }
    if let Some(brief) = update.brief {
        draft.brief = brief;
    }
    if let Some(tags) = update.tags {
        draft.tags = textutil::normalize_string_list(tags);
    }
    if let Some(language) = update.language {
        draft.language = language;
    }
    if let Some(kind) = update.kind {
        draft.kind = kind;
    }
    if let Some(scope) = update.scope {
        draft.scope = scope;
    }
    if let Some(targets) = update.targets {
        draft.targets = textutil::normalize_string_list(targets);
    }
    validate_draft_fields(&root, &draft)?;
    draft.updated_at = Utc::now().to_rfc3339();
    fs::write(path, serde_yaml::to_string(&draft)?)?;
    Ok(draft)
}

fn validate_draft_fields(project_root: &Path, draft: &DraftRecord) -> Result<()> {
    validate_non_empty("draft title", &draft.title)?;
    validate_non_empty("draft body", &draft.body)?;
    validate_kind(&draft.kind)?;
    validate_scope(&draft.scope)?;
    validate_targets(project_root, &draft.targets)
}

fn validate_non_empty(label: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(anyhow!("{label} must not be empty"));
    }
    Ok(())
}

fn validate_kind(kind: &str) -> Result<()> {
    const KINDS: &[&str] = &[
        "rule",
        "skilllet",
        "observation",
        "package",
        "preference",
        "constraint",
        "procedure",
        "convention",
        "correction",
        "anti-pattern",
    ];
    if !KINDS.contains(&kind) {
        return Err(anyhow!("unknown draft kind `{kind}`"));
    }
    Ok(())
}

fn validate_scope(scope: &str) -> Result<()> {
    const SCOPES: &[&str] = &["project", "global", "agent", "directory", "agent-specific"];
    if !SCOPES.contains(&scope) {
        return Err(anyhow!("unknown draft scope `{scope}`"));
    }
    Ok(())
}

fn validate_targets(project_root: &Path, targets: &[String]) -> Result<()> {
    let project = config::load_or_default_project_config(project_root)?;
    for target in targets {
        if !project.agents.contains_key(target) {
            return Err(anyhow!("unknown draft target agent `{target}`"));
        }
    }
    Ok(())
}

pub fn merge_drafts(
    project_root: &Path,
    id: &str,
    title: &str,
    source_ids: Vec<String>,
    targets: Vec<String>,
) -> Result<DraftRecord> {
    if source_ids.len() < 2 {
        return Err(anyhow!("at least two source drafts are required"));
    }

    let root = fsutil::normalize_project_root(project_root)?;
    let drafts = load_drafts(&root)?
        .into_iter()
        .map(|draft| (draft.id.clone(), draft))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut missing = Vec::new();
    let mut sources = Vec::new();
    for source_id in &source_ids {
        let Some(draft) = drafts.get(source_id) else {
            missing.push(source_id.clone());
            continue;
        };
        sources.push(draft.clone());
    }
    if !missing.is_empty() {
        return Err(anyhow!("missing source drafts: {}", missing.join(", ")));
    }

    let body = sources
        .iter()
        .map(|draft| format!("## {}\n\n{}", draft.title, draft.body))
        .collect::<Vec<_>>()
        .join("\n\n");
    let evidence = format!("Merged Drafts: {}", source_ids.join(", "));
    let reason = Some(format!(
        "Merged {} drafts into one review candidate.",
        sources.len()
    ));
    let confidence = sources
        .iter()
        .map(|draft| draft.confidence.unwrap_or(0.0))
        .min_by(f32::total_cmp);
    let mut targets = targets;
    targets.sort();
    targets.dedup();

    add_draft(
        &root,
        NewDraft {
            id: id.to_string(),
            title: title.to_string(),
            body,
            kind: "procedure".to_string(),
            scope: "project".to_string(),
            targets,
            evidence,
            confidence,
            reason,
            matched_template: Some("manual:draft-merge".to_string()),
            extraction: ExtractionMetadata::default(),
        },
    )?;

    // 合并后删除源草稿，只保留合并结果
    for source_id in &source_ids {
        let path = draft_path(&root, source_id)?;
        if path.exists() {
            fs::remove_file(&path)?;
        }
    }

    load_drafts(&root)?
        .into_iter()
        .find(|draft| draft.id == id)
        .ok_or_else(|| anyhow!("merged draft `{id}` was not written"))
}

pub fn fuse_skilllets_to_draft(
    project_root: &Path,
    id: &str,
    title: &str,
    source_ids: Vec<String>,
    targets: Vec<String>,
) -> Result<DraftRecord> {
    if source_ids.len() < 2 {
        return Err(anyhow!("at least two source skilllets are required"));
    }

    let root = fsutil::normalize_project_root(project_root)?;
    let skilllets = skilllet::skilllet_map(&root)?;
    let mut missing = Vec::new();
    let mut sources = Vec::new();
    for source_id in &source_ids {
        let Some(skilllet) = skilllets.get(source_id) else {
            missing.push(source_id.clone());
            continue;
        };
        sources.push(skilllet.clone());
    }
    if !missing.is_empty() {
        return Err(anyhow!("missing source skilllets: {}", missing.join(", ")));
    }

    let body = fuse_skilllet_body_with_provider(&root, title, &sources)
        .unwrap_or_else(|| fuse_skilllet_body_deterministic(title, &sources));
    let evidence = format!("Fused Skilllets: {}", source_ids.join(", "));
    let confidence = Some(0.8);
    let mut targets = targets;
    targets.sort();
    targets.dedup();

    add_draft(
        &root,
        NewDraft {
            id: id.to_string(),
            title: title.to_string(),
            body,
            kind: "procedure".to_string(),
            scope: "project".to_string(),
            targets,
            evidence,
            confidence,
            reason: Some(format!(
                "Synthesized a reviewable fusion candidate from {} skilllets.",
                sources.len()
            )),
            matched_template: Some("manual:skilllet-fusion".to_string()),
            extraction: ExtractionMetadata::default(),
        },
    )?;

    load_drafts(&root)?
        .into_iter()
        .find(|draft| draft.id == id)
        .ok_or_else(|| anyhow!("fused draft `{id}` was not written"))
}

#[derive(Debug, Deserialize)]
struct FuseSkillletResponse {
    body: String,
}

fn fuse_skilllet_body_with_provider(
    project_root: &Path,
    title: &str,
    sources: &[skilllet::SkillletRecord],
) -> Option<String> {
    let cfg = provider::load_or_default_provider_config(project_root).ok()?;
    let request = build_fuse_skilllet_prompt(title, sources);
    let output =
        provider::call_provider_for_role(&cfg, provider::ProviderRole::Refine, &request, 2048)
            .ok()?;
    let parsed: FuseSkillletResponse = serde_json::from_str(output.trim()).ok()?;
    let body = parsed.body.trim().to_string();
    is_valid_fused_body(&body).then_some(body)
}

fn build_fuse_skilllet_prompt(
    title: &str,
    sources: &[skilllet::SkillletRecord],
) -> ProviderRequest {
    let skilllets = sources
        .iter()
        .map(|source| {
            serde_json::json!({
                "id": source.id,
                "title": source.title,
                "kind": source.kind,
                "scope": source.scope,
                "body": source.body,
                "brief": source.brief,
                "tags": source.tags,
            })
        })
        .collect::<Vec<_>>();
    ProviderRequest {
        system_prompt: r#"你把多个 Skilllet 融合成一个新的高质量 Draft Skilllet。

要求：
- 输出一条新的综合规则，不要简单拼接源 Skilllet 标题或 Markdown 小节。
- 保留每个源 Skilllet 的核心触发条件、动作和边界。
- 删除重复内容，冲突处写成需要 review 的边界。
- 中文输入优先输出中文；英文输入可输出英文。
- 只返回 JSON。"#
            .to_string(),
        user_prompt: serde_json::json!({
            "new_title": title,
            "source_skilllets": skilllets,
        })
        .to_string(),
        json_schema: Some(ProviderJsonSchema {
            name: "FuseSkilllets".to_string(),
            strict: true,
            schema: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "body": { "type": "string" }
                },
                "required": ["body"]
            }),
        }),
    }
}

fn fuse_skilllet_body_deterministic(title: &str, sources: &[skilllet::SkillletRecord]) -> String {
    let bodies = sources
        .iter()
        .map(|skilllet| skilllet.body.trim().trim_end_matches(['.', '。']))
        .filter(|body| !body.is_empty())
        .collect::<Vec<_>>();
    format!(
        "当需要执行“{}”相关流程时，综合遵循这些 Skilllet：{}。目标是把多个片段压缩成一条可审阅、可执行的新规则，而不是保留多个重复草稿。",
        title.trim(),
        bodies.join("；")
    )
}

fn is_valid_fused_body(body: &str) -> bool {
    let lower = body.to_lowercase();
    body.chars().count() >= 24
        && body.chars().count() <= 1000
        && !body.lines().any(|line| line.trim_start().starts_with('#'))
        && !lower.contains("source skilllet")
        && !lower.contains("源 skilllet")
}

/// 删除指定草稿文件
pub fn delete_draft(project_root: &Path, id: &str) -> Result<()> {
    let root = fsutil::normalize_project_root(project_root)?;
    let path = draft_path(&root, id)?;
    if path.exists() {
        fs::remove_file(&path)?;
    }
    Ok(())
}

fn draft_path(project_root: &Path, id: &str) -> Result<PathBuf> {
    validate_draft_id(id)?;
    let safe = id.replace(':', "/").replace(['\\', ' '], "-");
    Ok(config::kernel_dir(project_root)
        .join("drafts")
        .join(format!("{safe}.yml")))
}

fn validate_draft_id(id: &str) -> Result<()> {
    let trimmed = id.trim();
    if trimmed.is_empty()
        || trimmed.len() > 160
        || trimmed.contains("..")
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains('\0')
        || Path::new(trimmed).is_absolute()
    {
        return Err(anyhow!("invalid draft id `{id}`"));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
