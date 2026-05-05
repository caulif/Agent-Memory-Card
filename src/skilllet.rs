use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::candidate::ExtractionMetadata;
use crate::config::{self, SkillletRef};
use crate::fsutil;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillletRecord {
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
    #[serde(default = "default_activation")]
    pub activation: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger_description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_project: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extraction: Option<ExtractionMetadata>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approved_from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
    #[serde(default)]
    pub merge_history: Vec<MergeEvent>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeEvent {
    pub source_id: String,
    pub merged_at: String,
    pub action: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillletUpdate {
    pub title: Option<String>,
    pub body: Option<String>,
    pub brief: Option<String>,
    pub tags: Option<Vec<String>>,
    pub language: Option<String>,
    pub kind: Option<String>,
    pub scope: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillletTargetMatrix {
    pub agents: Vec<String>,
    pub rows: Vec<SkillletTargetMatrixRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillletTargetMatrixRow {
    pub skilllet_id: String,
    pub title: String,
    pub targets: std::collections::BTreeMap<String, bool>,
}

impl SkillletTargetMatrix {
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("Agent-Kernel Skilllet Target Matrix\n\n");
        if self.rows.is_empty() {
            out.push_str("No skilllets found.\n");
            return out;
        }
        out.push_str(&format!("Skilllet | {}\n", self.agents.join(" | ")));
        out.push_str(&format!(
            "{}\n",
            std::iter::repeat_n("---", self.agents.len() + 1)
                .collect::<Vec<_>>()
                .join(" | ")
        ));
        for row in &self.rows {
            let cells = self
                .agents
                .iter()
                .map(|agent| {
                    if row.targets.get(agent).copied().unwrap_or(false) {
                        "yes"
                    } else {
                        "no"
                    }
                })
                .collect::<Vec<_>>();
            out.push_str(&format!("{} | {}\n", row.skilllet_id, cells.join(" | ")));
        }
        out
    }
}

pub fn add_skilllet(
    project_root: &Path,
    id: &str,
    title: &str,
    body: &str,
    kind: &str,
    scope: &str,
    targets: Vec<String>,
) -> Result<()> {
    add_skilllet_with_provenance(
        project_root,
        id,
        title,
        body,
        kind,
        scope,
        targets,
        None,
        None,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn add_skilllet_with_provenance(
    project_root: &Path,
    id: &str,
    title: &str,
    body: &str,
    kind: &str,
    scope: &str,
    targets: Vec<String>,
    extraction: Option<ExtractionMetadata>,
    approved_from: Option<String>,
    evidence: Option<String>,
) -> Result<()> {
    let root = fsutil::normalize_project_root(project_root)?;
    validate_skilllet_fields(&root, title, body, kind, scope, &targets)?;
    config::ensure_kernel_dir(&root)?;
    let now = Utc::now().to_rfc3339();
    let path = skilllet_path(&root, id)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let existing = if path.exists() {
        fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_yaml::from_str::<SkillletRecord>(&text).ok())
    } else {
        None
    };
    let metadata = infer_skilllet_metadata(title, body, kind);

    let record = SkillletRecord {
        schema_version: existing
            .as_ref()
            .map(|record| record.schema_version)
            .unwrap_or_else(current_schema_version),
        id: id.to_string(),
        title: title.to_string(),
        kind: kind.to_string(),
        scope: scope.to_string(),
        body: body.to_string(),
        brief: metadata.brief,
        tags: metadata.tags,
        language: metadata.language,
        activation: existing
            .as_ref()
            .map(|record| record.activation.clone())
            .unwrap_or_else(|| infer_activation(kind).to_string()),
        trigger_description: existing
            .as_ref()
            .and_then(|record| record.trigger_description.clone()),
        source_project: existing
            .as_ref()
            .and_then(|record| record.source_project.clone()),
        extraction: extraction.or_else(|| {
            existing
                .as_ref()
                .and_then(|record| record.extraction.clone())
        }),
        approved_from: approved_from.or_else(|| {
            existing
                .as_ref()
                .and_then(|record| record.approved_from.clone())
        }),
        evidence: evidence.or_else(|| existing.as_ref().and_then(|record| record.evidence.clone())),
        merge_history: existing
            .as_ref()
            .map(|record| record.merge_history.clone())
            .unwrap_or_default(),
        created_at: existing
            .as_ref()
            .map(|record| record.created_at.clone())
            .unwrap_or_else(|| now.clone()),
        updated_at: now,
    };
    fs::write(&path, serde_yaml::to_string(&record)?)
        .with_context(|| format!("write {}", path.display()))?;

    let mut project = config::load_or_default_project_config(&root)?;
    if let Some(existing) = project
        .skilllets
        .include
        .iter_mut()
        .find(|item| item.id == id)
    {
        existing.targets = targets;
        existing.scope = Some(scope.to_string());
    } else {
        project.skilllets.include.push(SkillletRef {
            id: id.to_string(),
            targets,
            scope: Some(scope.to_string()),
        });
    }
    config::save_project_config(&root, &project)?;

    Ok(())
}

pub fn update_skilllet(
    project_root: &Path,
    id: &str,
    update: SkillletUpdate,
) -> Result<SkillletRecord> {
    let root = fsutil::normalize_project_root(project_root)?;
    let path = skilllet_path(&root, id)?;
    if !path.exists() {
        return Err(anyhow!("skilllet `{id}` does not exist"));
    }

    let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let mut record: SkillletRecord = serde_yaml::from_str(&text)
        .with_context(|| format!("parse skilllet {}", path.display()))?;

    if let Some(title) = update.title {
        record.title = title;
    }
    if let Some(body) = update.body {
        record.body = body;
    }
    if let Some(brief) = update.brief {
        record.brief = brief;
    }
    if let Some(tags) = update.tags {
        record.tags = normalize_tags(tags);
    }
    if let Some(language) = update.language {
        record.language = language;
    }
    if let Some(kind) = update.kind {
        record.kind = kind;
    }
    if let Some(scope) = update.scope {
        record.scope = scope;
    }
    if record.activation == "model-decision" {
        record.activation = infer_activation(&record.kind).to_string();
    }
    validate_skilllet_fields(
        &root,
        &record.title,
        &record.body,
        &record.kind,
        &record.scope,
        &Vec::new(),
    )?;
    record.updated_at = Utc::now().to_rfc3339();

    fs::write(&path, serde_yaml::to_string(&record)?)
        .with_context(|| format!("write {}", path.display()))?;
    Ok(record)
}

pub fn load_skilllets(project_root: &Path) -> Result<Vec<SkillletRecord>> {
    let root = fsutil::normalize_project_root(project_root)?;
    let dir = config::kernel_dir(&root).join("skilllets");
    load_skilllets_from_dir(&dir)
}

pub fn load_global_skilllets(home: &Path) -> Result<Vec<SkillletRecord>> {
    load_skilllets_from_dir(&home.join(".agent-kernel").join("skilllets"))
}

pub fn count_global_skilllets(home: &Path) -> Result<usize> {
    count_skilllets_in_dir(&home.join(".agent-kernel").join("skilllets"))
}

fn load_skilllets_from_dir(dir: &Path) -> Result<Vec<SkillletRecord>> {
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
        records.push(serde_yaml::from_str::<SkillletRecord>(&text)?);
    }
    records.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(records)
}

fn count_skilllets_in_dir(dir: &Path) -> Result<usize> {
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

pub fn set_skilllet_targets(project_root: &Path, id: &str, targets: Vec<String>) -> Result<()> {
    validate_skilllet_id(id)?;
    let root = fsutil::normalize_project_root(project_root)?;
    validate_targets(&root, &targets)?;
    let skilllets = load_skilllets(&root)?;
    let Some(record) = skilllets.iter().find(|record| record.id == id) else {
        return Err(anyhow!("skilllet `{id}` does not exist"));
    };
    let mut project = config::load_or_default_project_config(&root)?;
    if let Some(existing) = project
        .skilllets
        .include
        .iter_mut()
        .find(|item| item.id == id)
    {
        existing.targets = targets;
        existing.scope = Some(record.scope.clone());
    } else {
        project.skilllets.include.push(SkillletRef {
            id: id.to_string(),
            targets,
            scope: Some(record.scope.clone()),
        });
    }
    config::save_project_config(&root, &project)
}

pub fn promote_skilllet_to_global(
    project_root: &Path,
    home: &Path,
    id: &str,
) -> Result<SkillletRecord> {
    validate_skilllet_id(id)?;
    let root = fsutil::normalize_project_root(project_root)?;
    let Some(mut record) = skilllet_map(&root)?.remove(id) else {
        return Err(anyhow!("skilllet `{id}` does not exist"));
    };
    record.scope = "global".to_string();
    record.source_project = Some(fsutil::path_to_slash(&root));
    record.updated_at = Utc::now().to_rfc3339();

    let path = global_skilllet_path(home, id)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, serde_yaml::to_string(&record)?)
        .with_context(|| format!("write {}", path.display()))?;
    Ok(record)
}

pub fn install_global_skilllet_to_project(
    project_root: &Path,
    home: &Path,
    id: &str,
    targets: Vec<String>,
) -> Result<SkillletRecord> {
    validate_skilllet_id(id)?;
    let root = fsutil::normalize_project_root(project_root)?;
    let Some(mut record) = load_global_skilllets(home)?
        .into_iter()
        .find(|record| record.id == id)
    else {
        return Err(anyhow!("global skilllet `{id}` does not exist"));
    };

    config::ensure_kernel_dir(&root)?;
    record.scope = "project".to_string();
    record.updated_at = Utc::now().to_rfc3339();

    let path = skilllet_path(&root, id)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, serde_yaml::to_string(&record)?)
        .with_context(|| format!("write {}", path.display()))?;

    let mut project = config::load_or_default_project_config(&root)?;
    if let Some(existing) = project
        .skilllets
        .include
        .iter_mut()
        .find(|item| item.id == id)
    {
        existing.targets = targets;
        existing.scope = Some("project".to_string());
    } else {
        project.skilllets.include.push(SkillletRef {
            id: id.to_string(),
            targets,
            scope: Some("project".to_string()),
        });
    }
    config::save_project_config(&root, &project)?;

    Ok(record)
}

pub fn merge_skilllets(
    project_root: &Path,
    id: &str,
    title: &str,
    source_ids: Vec<String>,
    targets: Vec<String>,
) -> Result<()> {
    validate_skilllet_id(id)?;
    let root = fsutil::normalize_project_root(project_root)?;
    let skilllets = skilllet_map(&root)?;
    let mut body = String::new();
    let mut missing = Vec::new();

    for source_id in &source_ids {
        let Some(record) = skilllets.get(source_id) else {
            missing.push(source_id.clone());
            continue;
        };
        body.push_str(&format!("## {}\n\n{}\n\n", record.title, record.body));
    }

    if !missing.is_empty() {
        return Err(anyhow!("missing source skilllets: {}", missing.join(", ")));
    }
    if source_ids.is_empty() {
        return Err(anyhow!("at least one source skilllet is required"));
    }

    add_skilllet(
        &root,
        id,
        title,
        body.trim(),
        "procedure",
        "project",
        targets,
    )?;

    // 合并后删除源 skilllet，只保留合并结果
    for source_id in &source_ids {
        let path = skilllet_path(&root, source_id)?;
        if path.exists() {
            fs::remove_file(&path)?;
        }
    }

    Ok(())
}

/// 删除指定 skilllet 文件
pub fn delete_skilllet(project_root: &Path, id: &str) -> Result<()> {
    let root = fsutil::normalize_project_root(project_root)?;
    let path = skilllet_path(&root, id)?;
    if path.exists() {
        fs::remove_file(&path)?;
    }
    Ok(())
}

pub fn skilllet_map(
    project_root: &Path,
) -> Result<std::collections::BTreeMap<String, SkillletRecord>> {
    Ok(load_skilllets(project_root)?
        .into_iter()
        .map(|record| (record.id.clone(), record))
        .collect())
}

pub fn skilllet_target_matrix(project_root: &Path) -> Result<SkillletTargetMatrix> {
    let root = fsutil::normalize_project_root(project_root)?;
    let project = config::load_or_default_project_config(&root)?;
    let skilllets = load_skilllets(&root)?;
    let agents = project.agents.keys().cloned().collect::<Vec<_>>();
    let refs = project
        .skilllets
        .include
        .iter()
        .map(|item| (item.id.clone(), item.targets.clone()))
        .collect::<std::collections::BTreeMap<_, _>>();

    let rows = skilllets
        .into_iter()
        .map(|record| {
            let explicit_targets = refs.get(&record.id);
            let targets = agents
                .iter()
                .map(|agent| {
                    let assigned = if let Some(explicit_targets) = explicit_targets {
                        explicit_targets.iter().any(|target| target == agent)
                    } else {
                        project
                            .agents
                            .get(agent)
                            .map(|agent| agent.enabled)
                            .unwrap_or(false)
                    };
                    (agent.clone(), assigned)
                })
                .collect();
            SkillletTargetMatrixRow {
                skilllet_id: record.id,
                title: record.title,
                targets,
            }
        })
        .collect();

    Ok(SkillletTargetMatrix { agents, rows })
}

#[cfg(test)]
mod tests;

const MAX_SKILLLET_ID_LEN: usize = 128;

fn skilllet_path(project_root: &Path, id: &str) -> Result<PathBuf> {
    skilllet_file_path(&config::kernel_dir(project_root).join("skilllets"), id)
}

fn global_skilllet_path(home: &Path, id: &str) -> Result<PathBuf> {
    skilllet_file_path(&home.join(".agent-kernel").join("skilllets"), id)
}

fn skilllet_file_path(dir: &Path, id: &str) -> Result<PathBuf> {
    validate_skilllet_id(id)?;
    let safe = id.replace(':', "/").replace(['\\', ' '], "-");
    let path = dir.join(format!("{safe}.yml"));
    ensure_skilllet_path_stays_in_dir(dir, &path)?;
    Ok(path)
}

fn validate_skilllet_id(id: &str) -> Result<()> {
    if id.trim().is_empty() {
        return Err(anyhow!("skilllet id must not be empty"));
    }
    if id.len() > MAX_SKILLLET_ID_LEN {
        return Err(anyhow!(
            "skilllet id must be at most {MAX_SKILLLET_ID_LEN} bytes"
        ));
    }
    if id.contains("..") {
        return Err(anyhow!("skilllet id must not contain `..`"));
    }
    if id.contains('/') || id.contains('\\') {
        return Err(anyhow!("skilllet id must not contain path separators"));
    }
    let bytes = id.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return Err(anyhow!("skilllet id must not look like an absolute path"));
    }
    if Path::new(id).is_absolute() {
        return Err(anyhow!("skilllet id must not be an absolute path"));
    }
    Ok(())
}

fn validate_skilllet_fields(
    project_root: &Path,
    title: &str,
    body: &str,
    kind: &str,
    scope: &str,
    targets: &[String],
) -> Result<()> {
    validate_non_empty("skilllet title", title)?;
    validate_non_empty("skilllet body", body)?;
    validate_kind(kind)?;
    validate_scope(scope)?;
    validate_targets(project_root, targets)
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
        "template",
        "workflow",
        "convention",
        "correction",
        "anti-pattern",
    ];
    if !KINDS.contains(&kind) {
        return Err(anyhow!("unknown skilllet kind `{kind}`"));
    }
    Ok(())
}

fn validate_scope(scope: &str) -> Result<()> {
    const SCOPES: &[&str] = &["project", "global", "agent", "directory", "agent-specific"];
    if !SCOPES.contains(&scope) {
        return Err(anyhow!("unknown skilllet scope `{scope}`"));
    }
    Ok(())
}

fn validate_targets(project_root: &Path, targets: &[String]) -> Result<()> {
    let project = config::load_or_default_project_config(project_root)?;
    for target in targets {
        if !project.agents.contains_key(target) {
            return Err(anyhow!("unknown skilllet target agent `{target}`"));
        }
    }
    Ok(())
}

fn ensure_skilllet_path_stays_in_dir(dir: &Path, path: &Path) -> Result<()> {
    path.strip_prefix(dir)
        .map(|_| ())
        .with_context(|| format!("skilllet path escaped {}", dir.display()))
}

fn normalize_tags(mut tags: Vec<String>) -> Vec<String> {
    tags.sort();
    tags.dedup();
    tags
}

#[derive(Debug, Clone)]
struct SkillletMetadata {
    brief: String,
    tags: Vec<String>,
    language: String,
}

fn infer_skilllet_metadata(title: &str, body: &str, kind: &str) -> SkillletMetadata {
    let language = infer_language(title, body);
    let tags = infer_tags(title, body, kind);
    let brief = if language == "zh" {
        format!(
            "以后遇到类似任务，可以复用“{}”：{}",
            title,
            concise_zh_summary(body)
        )
    } else {
        format!(
            "Use \"{}\" for similar future work: {}",
            title,
            concise_en_summary(body)
        )
    };
    SkillletMetadata {
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
    let mappings: [(&str, &[&str]); 11] = [
        ("ui-design", &["ui", "界面", "设计", "视觉", "按钮", "组件"]),
        (
            "frontend",
            &["frontend", "前端", "react", "typescript", "axios"],
        ),
        ("backend", &["backend", "后端", "api", "server"]),
        ("rust", &["rust", "cargo", "clippy"]),
        ("tauri", &["tauri", "desktop", "桌面"]),
        (
            "testing",
            &["test", "testing", "vitest", "playwright", "测试"],
        ),
        (
            "performance",
            &["performance", "性能", "卡顿", "卡死", "responsive"],
        ),
        (
            "agent-handoff",
            &["claude", "codex", "agent", "智能体", "交接"],
        ),
        ("code-style", &["style", "lint", "格式", "代码风格"]),
        (
            "workflow",
            &["workflow", "流程", "步骤", "procedure", "checklist"],
        ),
        ("safety", &["safety", "安全", "secret", "redact", "禁止"]),
    ];
    let mut tags = mappings
        .iter()
        .filter(|(_, markers)| markers.iter().any(|marker| lower.contains(marker)))
        .map(|(tag, _)| (*tag).to_string())
        .collect::<Vec<_>>();
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

fn default_activation() -> String {
    "model-decision".to_string()
}

pub fn infer_activation(kind: &str) -> &'static str {
    match kind {
        "preference" | "constraint" => "always-on",
        "procedure" | "template" | "workflow" => "skill",
        _ => "model-decision",
    }
}

fn current_schema_version() -> u32 {
    1
}
