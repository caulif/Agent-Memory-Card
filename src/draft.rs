use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::candidate::ExtractionMetadata;
use crate::config;
use crate::fsutil;
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

pub fn approve_draft(project_root: &Path, id: &str) -> Result<()> {
    let root = fsutil::normalize_project_root(project_root)?;
    let path = draft_path(&root, id)?;
    if !path.exists() {
        return Err(anyhow!("draft `{id}` does not exist"));
    }
    let draft: DraftRecord = serde_yaml::from_str(&fs::read_to_string(&path)?)?;
    // 检查同名 skilllet 是否已存在；若 body 语义相似则直接删除草稿（视为已批准）
    let existing_skilllets = skilllet::load_skilllets(&root)?;
    if let Some(existing_skilllet) = existing_skilllets
        .iter()
        .find(|skilllet| skilllet.id == draft.id)
    {
        if textutil::jaccard_similarity(&draft.body, &existing_skilllet.body) >= 0.75 {
            fs::remove_file(path)?;
            return Ok(());
        }
        return Err(anyhow!(
            "skilllet id conflict for `{}` (different body); review or merge the existing Skilllet before approving this Draft",
            draft.id
        ));
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

    let body = sources
        .iter()
        .map(|skilllet| format!("## {}\n\n{}", skilllet.title, skilllet.body))
        .collect::<Vec<_>>()
        .join("\n\n");
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
                "Created as a reviewable fusion candidate from {} skilllets.",
                sources.len()
            )),
            matched_template: Some("manual:skilllet-fusion".to_string()),
            extraction: ExtractionMetadata::default(),
        },
    )?;

    // 融合后删除源 skilllet，只保留融合结果
    for source_id in &source_ids {
        skilllet::delete_skilllet(&root, source_id)?;
    }

    load_drafts(&root)?
        .into_iter()
        .find(|draft| draft.id == id)
        .ok_or_else(|| anyhow!("fused draft `{id}` was not written"))
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
mod tests {
    use super::*;

    #[test]
    fn approves_draft_into_skilllet() {
        let temp = tempfile::tempdir().expect("tempdir");
        add_draft(
            temp.path(),
            NewDraft {
                id: "project:prefer-bun".to_string(),
                title: "Prefer Bun".to_string(),
                body: "Use Bun for JavaScript package management and scripts.".to_string(),
                kind: "preference".to_string(),
                scope: "project".to_string(),
                targets: vec!["codex".to_string()],
                evidence: "manual test".to_string(),
                confidence: None,
                reason: None,
                matched_template: None,
                extraction: ExtractionMetadata::default(),
            },
        )
        .expect("add draft");

        approve_draft(temp.path(), "project:prefer-bun").expect("approve");

        assert!(load_drafts(temp.path()).expect("drafts").is_empty());
        let skilllets = skilllet::load_skilllets(temp.path()).expect("skilllets");
        assert_eq!(skilllets[0].id, "project:prefer-bun");
    }

    #[test]
    fn approve_draft_rejects_existing_skilllet_id_conflict() {
        let temp = tempfile::tempdir().expect("tempdir");
        skilllet::add_skilllet(
            temp.path(),
            "project:prefer-bun",
            "Prefer Bun",
            "Use Bun for JavaScript package management.",
            "preference",
            "project",
            vec!["codex".to_string()],
        )
        .expect("skilllet");
        add_draft(
            temp.path(),
            NewDraft {
                id: "project:prefer-bun".to_string(),
                title: "Prefer Bun Updated".to_string(),
                body: "Use Bun for all JavaScript scripts.".to_string(),
                kind: "preference".to_string(),
                scope: "project".to_string(),
                targets: vec!["codex".to_string()],
                evidence: "desktop test".to_string(),
                confidence: Some(0.9),
                reason: Some("Repeated correction".to_string()),
                matched_template: None,
                extraction: ExtractionMetadata::default(),
            },
        )
        .expect("draft");

        let err = approve_draft(temp.path(), "project:prefer-bun")
            .expect_err("approval should require conflict review");

        assert!(err.to_string().contains("conflict"));
        assert_eq!(load_drafts(temp.path()).expect("drafts").len(), 1);
        let skilllets = skilllet::load_skilllets(temp.path()).expect("skilllets");
        assert_eq!(
            skilllets[0].body,
            "Use Bun for JavaScript package management."
        );
    }

    #[test]
    fn update_draft_rejects_empty_body_unknown_kind_and_unknown_target() {
        let temp = tempfile::tempdir().expect("tempdir");
        add_draft(
            temp.path(),
            NewDraft {
                id: "project:editable".to_string(),
                title: "Editable".to_string(),
                body: "Use Vitest for frontend tests.".to_string(),
                kind: "preference".to_string(),
                scope: "project".to_string(),
                targets: vec!["codex".to_string()],
                evidence: "desktop test".to_string(),
                confidence: Some(0.9),
                reason: Some("Repeated correction".to_string()),
                matched_template: None,
                extraction: ExtractionMetadata::default(),
            },
        )
        .expect("draft");

        let empty_body = update_draft(
            temp.path(),
            "project:editable",
            DraftUpdate {
                body: Some("   ".to_string()),
                ..DraftUpdate::default()
            },
        )
        .expect_err("empty body should fail");
        assert!(empty_body.to_string().contains("body"));

        let unknown_kind = update_draft(
            temp.path(),
            "project:editable",
            DraftUpdate {
                kind: Some("mystery".to_string()),
                ..DraftUpdate::default()
            },
        )
        .expect_err("unknown kind should fail");
        assert!(unknown_kind.to_string().contains("kind"));

        let unknown_target = update_draft(
            temp.path(),
            "project:editable",
            DraftUpdate {
                targets: Some(vec!["cursor".to_string()]),
                ..DraftUpdate::default()
            },
        )
        .expect_err("unknown target should fail");
        assert!(unknown_target.to_string().contains("target agent"));
    }

    #[test]
    fn draft_explainability_fields_roundtrip() {
        let temp = tempfile::tempdir().expect("tempdir");
        add_draft(
            temp.path(),
            NewDraft {
                id: "project:prefer-bun".to_string(),
                title: "Prefer Bun".to_string(),
                body: "Use Bun for JavaScript package management and scripts.".to_string(),
                kind: "preference".to_string(),
                scope: "project".to_string(),
                targets: vec!["codex".to_string()],
                evidence: "manual test".to_string(),
                confidence: Some(0.92),
                reason: Some("Matched project preference template".to_string()),
                matched_template: Some("project:prefer-bun".to_string()),
                extraction: ExtractionMetadata::default(),
            },
        )
        .expect("add draft");

        let drafts = load_drafts(temp.path()).expect("drafts");

        assert_eq!(drafts[0].confidence, Some(0.92));
        assert_eq!(
            drafts[0].reason.as_deref(),
            Some("Matched project preference template")
        );
        assert_eq!(
            drafts[0].matched_template.as_deref(),
            Some("project:prefer-bun")
        );
    }

    #[test]
    fn draft_records_include_plain_brief_language_and_tags() {
        let temp = tempfile::tempdir().expect("tempdir");
        add_draft(
            temp.path(),
            NewDraft {
                id: "project:ui-background-tasks".to_string(),
                title: "UI 后台任务".to_string(),
                body: "处理 Tauri UI 的扫描、整理和编译时，把长任务放到后台，前端只刷新状态和进度，避免点击后卡死。"
                    .to_string(),
                kind: "procedure".to_string(),
                scope: "project".to_string(),
                targets: vec!["codex".to_string()],
                evidence: "manual test".to_string(),
                confidence: Some(0.9),
                reason: None,
                matched_template: None,
                extraction: ExtractionMetadata::default(),
            },
        )
        .expect("add draft");

        let drafts = load_drafts(temp.path()).expect("drafts");

        assert_eq!(drafts[0].language, "zh");
        assert!(drafts[0].brief.contains("这条草稿记录了"));
        assert!(!drafts[0].brief.contains("以后遇到类似场景"));
        assert!(drafts[0].brief.chars().count() < drafts[0].body.chars().count() + 40);
        assert!(drafts[0].tags.contains(&"ui-design".to_string()));
        assert!(drafts[0].tags.contains(&"tauri".to_string()));
        assert!(drafts[0].tags.contains(&"performance".to_string()));
    }

    #[test]
    fn updates_draft_reviewable_fields_without_losing_explainability() {
        let temp = tempfile::tempdir().expect("tempdir");
        add_draft(
            temp.path(),
            NewDraft {
                id: "project:prefer-bun".to_string(),
                title: "Prefer Bun".to_string(),
                body: "Use Bun for JavaScript package management and scripts.".to_string(),
                kind: "preference".to_string(),
                scope: "project".to_string(),
                targets: vec!["codex".to_string()],
                evidence: "manual test".to_string(),
                confidence: Some(0.92),
                reason: Some("Matched project preference template".to_string()),
                matched_template: Some("built-in:Prefer Bun".to_string()),
                extraction: ExtractionMetadata::default(),
            },
        )
        .expect("add draft");

        let updated = update_draft(
            temp.path(),
            "project:prefer-bun",
            DraftUpdate {
                title: Some("Prefer Bun Runtime".to_string()),
                body: Some(
                    "Use Bun for package management, scripts, and JS runtime tasks.".to_string(),
                ),
                targets: Some(vec!["claude-code".to_string(), "codex".to_string()]),
                ..Default::default()
            },
        )
        .expect("update draft");

        assert_eq!(updated.title, "Prefer Bun Runtime");
        assert_eq!(
            updated.body,
            "Use Bun for package management, scripts, and JS runtime tasks."
        );
        assert_eq!(updated.targets, vec!["claude-code", "codex"]);
        assert_eq!(updated.confidence, Some(0.92));
        assert_eq!(
            updated.matched_template.as_deref(),
            Some("built-in:Prefer Bun")
        );

        let drafts = load_drafts(temp.path()).expect("drafts");
        assert_eq!(drafts[0].title, "Prefer Bun Runtime");
    }

    #[test]
    fn updates_draft_editor_fields_and_allows_empty_targets() {
        let temp = tempfile::tempdir().expect("tempdir");
        add_draft(
            temp.path(),
            NewDraft {
                id: "project:prefer-bun".to_string(),
                title: "Prefer Bun".to_string(),
                body: "Use Bun for JavaScript package management and scripts.".to_string(),
                kind: "preference".to_string(),
                scope: "project".to_string(),
                targets: vec!["codex".to_string()],
                evidence: "manual test".to_string(),
                confidence: Some(0.92),
                reason: Some("Matched project preference template".to_string()),
                matched_template: Some("built-in:Prefer Bun".to_string()),
                extraction: ExtractionMetadata::default(),
            },
        )
        .expect("add draft");

        let updated = update_draft(
            temp.path(),
            "project:prefer-bun",
            DraftUpdate {
                brief: Some("以后 JS 项目默认用 Bun 管理依赖和脚本。".to_string()),
                tags: Some(vec![
                    "workflow".to_string(),
                    "frontend".to_string(),
                    "frontend".to_string(),
                ]),
                language: Some("zh".to_string()),
                targets: Some(Vec::new()),
                ..Default::default()
            },
        )
        .expect("update draft editor fields");

        assert_eq!(updated.brief, "以后 JS 项目默认用 Bun 管理依赖和脚本。");
        assert_eq!(updated.tags, vec!["frontend", "workflow"]);
        assert_eq!(updated.language, "zh");
        assert!(updated.targets.is_empty());
    }

    #[test]
    fn rejects_draft_ids_that_escape_kernel_directory() {
        let temp = tempfile::tempdir().expect("tempdir");

        let err = add_draft(
            temp.path(),
            NewDraft {
                id: "../escape".to_string(),
                title: "Escape".to_string(),
                body: "This must not write outside the drafts directory.".to_string(),
                kind: "preference".to_string(),
                scope: "project".to_string(),
                targets: vec!["codex".to_string()],
                evidence: "manual test".to_string(),
                confidence: None,
                reason: None,
                matched_template: None,
                extraction: ExtractionMetadata::default(),
            },
        )
        .expect_err("path traversal id should fail");

        assert!(err.to_string().contains("invalid draft id"));
        assert!(!config::kernel_dir(temp.path()).join("escape.yml").exists());
    }

    #[test]
    fn merges_drafts_into_new_reviewable_draft_without_removing_sources() {
        let temp = tempfile::tempdir().expect("tempdir");
        add_draft(
            temp.path(),
            NewDraft {
                id: "project:use-axios".to_string(),
                title: "Use Axios".to_string(),
                body: "Use Axios for frontend HTTP requests.".to_string(),
                kind: "preference".to_string(),
                scope: "project".to_string(),
                targets: vec!["codex".to_string()],
                evidence: "observation:a".to_string(),
                confidence: Some(0.92),
                reason: Some("Matched HTTP client preference".to_string()),
                matched_template: Some("built-in:Use Axios".to_string()),
                extraction: ExtractionMetadata::default(),
            },
        )
        .expect("add axios draft");
        add_draft(
            temp.path(),
            NewDraft {
                id: "project:prefer-bun".to_string(),
                title: "Prefer Bun".to_string(),
                body: "Use Bun for package management and scripts.".to_string(),
                kind: "preference".to_string(),
                scope: "project".to_string(),
                targets: vec!["claude-code".to_string()],
                evidence: "observation:b".to_string(),
                confidence: Some(0.84),
                reason: Some("Matched package manager preference".to_string()),
                matched_template: Some("built-in:Prefer Bun".to_string()),
                extraction: ExtractionMetadata::default(),
            },
        )
        .expect("add bun draft");

        let merged = merge_drafts(
            temp.path(),
            "project:frontend-defaults",
            "Frontend Defaults",
            vec![
                "project:use-axios".to_string(),
                "project:prefer-bun".to_string(),
            ],
            vec!["codex".to_string(), "claude-code".to_string()],
        )
        .expect("merge drafts");

        assert_eq!(merged.id, "project:frontend-defaults");
        assert_eq!(merged.title, "Frontend Defaults");
        assert_eq!(merged.kind, "procedure");
        assert_eq!(merged.scope, "project");
        assert_eq!(merged.targets, vec!["claude-code", "codex"]);
        assert!(merged.body.contains("## Use Axios"));
        assert!(merged.body.contains("Use Bun for package management"));
        assert!(merged.evidence.contains("Merged Drafts"));
        assert!(merged.evidence.contains("project:use-axios"));
        assert_eq!(merged.confidence, Some(0.84));
        assert!(
            merged
                .reason
                .as_deref()
                .unwrap()
                .contains("Merged 2 drafts")
        );

        let drafts = load_drafts(temp.path()).expect("drafts");
        // 合并后源草稿被删除，只保留合并结果
        assert_eq!(drafts.len(), 1);
        assert!(!drafts.iter().any(|draft| draft.id == "project:use-axios"));
        assert!(!drafts.iter().any(|draft| draft.id == "project:prefer-bun"));
        assert!(
            drafts
                .iter()
                .any(|draft| draft.id == "project:frontend-defaults")
        );
    }

    #[test]
    fn merge_requires_at_least_two_source_drafts() {
        let temp = tempfile::tempdir().expect("tempdir");
        add_draft(
            temp.path(),
            NewDraft {
                id: "project:solo".to_string(),
                title: "Solo".to_string(),
                body: "One draft is not a merge.".to_string(),
                kind: "preference".to_string(),
                scope: "project".to_string(),
                targets: vec!["codex".to_string()],
                evidence: "manual test".to_string(),
                confidence: None,
                reason: None,
                matched_template: None,
                extraction: ExtractionMetadata::default(),
            },
        )
        .expect("add solo");

        let err = merge_drafts(
            temp.path(),
            "project:solo-merged",
            "Solo Merged",
            vec!["project:solo".to_string()],
            vec!["codex".to_string()],
        )
        .expect_err("single source should fail");

        assert!(err.to_string().contains("at least two source drafts"));
        assert_eq!(load_drafts(temp.path()).expect("drafts").len(), 1);
    }

    #[test]
    fn fuses_skilllets_into_reviewable_draft() {
        let temp = tempfile::tempdir().expect("tempdir");
        skilllet::add_skilllet(
            temp.path(),
            "project:ui-background-tasks",
            "UI Background Tasks",
            "Run long UI scans in background tasks and refresh progress separately.",
            "procedure",
            "project",
            vec!["codex".to_string()],
        )
        .expect("add ui");
        skilllet::add_skilllet(
            temp.path(),
            "project:tauri-responsive",
            "Tauri Responsive Shell",
            "Keep Tauri commands short from the frontend and report status incrementally.",
            "procedure",
            "project",
            vec!["claude-code".to_string()],
        )
        .expect("add tauri");

        let draft = fuse_skilllets_to_draft(
            temp.path(),
            "project:responsive-desktop-workflow",
            "Responsive Desktop Workflow",
            vec![
                "project:ui-background-tasks".to_string(),
                "project:tauri-responsive".to_string(),
            ],
            vec!["codex".to_string(), "claude-code".to_string()],
        )
        .expect("fuse");

        assert_eq!(draft.kind, "procedure");
        assert_eq!(
            draft.matched_template.as_deref(),
            Some("manual:skilllet-fusion")
        );
        assert!(draft.evidence.contains("Fused Skilllets"));
        assert!(draft.body.contains("UI Background Tasks"));
        assert!(draft.body.contains("Tauri Responsive Shell"));
        assert!(draft.brief.contains("Responsive Desktop Workflow"));
        assert!(draft.tags.contains(&"tauri".to_string()));
    }
}
