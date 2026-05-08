use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::candidate::ExtractionMetadata;
use crate::config::{self, MemoryCardRef};
use crate::fsutil;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCardRecord {
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
pub struct MemoryCardUpdate {
    pub title: Option<String>,
    pub body: Option<String>,
    pub brief: Option<String>,
    pub tags: Option<Vec<String>>,
    pub language: Option<String>,
    pub kind: Option<String>,
    pub scope: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCardTargetMatrix {
    pub agents: Vec<String>,
    pub rows: Vec<MemoryCardTargetMatrixRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCardTargetMatrixRow {
    pub memory_card_id: String,
    pub title: String,
    pub scope: String,
    pub targets: std::collections::BTreeMap<String, bool>,
}

impl MemoryCardTargetMatrix {
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("Agent Memory Kernel Memory Card Target Matrix\n\n");
        if self.rows.is_empty() {
            out.push_str("No Memory Cards found.\n");
            return out;
        }
        out.push_str(&format!("Memory Card | {}\n", self.agents.join(" | ")));
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
            out.push_str(&format!("{} | {}\n", row.memory_card_id, cells.join(" | ")));
        }
        out
    }
}

pub fn add_memory_card(
    project_root: &Path,
    id: &str,
    title: &str,
    body: &str,
    kind: &str,
    scope: &str,
    targets: Vec<String>,
) -> Result<()> {
    add_memory_card_with_provenance(
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
pub fn add_memory_card_with_provenance(
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
    validate_memory_card_fields(&root, title, body, kind, scope, &targets)?;
    config::ensure_kernel_dir(&root)?;
    let now = Utc::now().to_rfc3339();
    let path = memory_card_path(&root, id)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let existing = if path.exists() {
        fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_yaml::from_str::<MemoryCardRecord>(&text).ok())
    } else {
        None
    };
    let metadata = infer_memory_card_metadata(title, body, kind);

    let record = MemoryCardRecord {
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
        .memory_cards
        .include
        .iter_mut()
        .find(|item| item.id == id)
    {
        existing.targets = targets;
        existing.scope = Some(scope.to_string());
    } else {
        project.memory_cards.include.push(MemoryCardRef {
            id: id.to_string(),
            targets,
            scope: Some(scope.to_string()),
        });
    }
    config::save_project_config(&root, &project)?;

    Ok(())
}

pub fn update_memory_card(
    project_root: &Path,
    id: &str,
    update: MemoryCardUpdate,
) -> Result<MemoryCardRecord> {
    let root = fsutil::normalize_project_root(project_root)?;
    let path = memory_card_path(&root, id)?;
    if !path.exists() {
        return Err(anyhow!("Memory Card `{id}` does not exist"));
    }

    let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let mut record: MemoryCardRecord = serde_yaml::from_str(&text)
        .with_context(|| format!("parse Memory Card {}", path.display()))?;

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
    validate_memory_card_fields(
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

pub fn review_update_matches_existing(
    existing: &MemoryCardRecord,
    incoming_title: &str,
    incoming_body: &str,
) -> bool {
    existing.title.trim() == incoming_title.trim()
        || crate::textutil::jaccard_similarity(&existing.body, incoming_body) >= 0.45
}

#[allow(clippy::too_many_arguments)]
pub fn update_memory_card_from_review(
    project_root: &Path,
    id: &str,
    title: String,
    body: String,
    brief: String,
    tags: Vec<String>,
    language: String,
    kind: String,
    scope: String,
) -> Result<MemoryCardRecord> {
    update_memory_card(
        project_root,
        id,
        MemoryCardUpdate {
            title: Some(title),
            body: Some(body),
            brief: Some(brief),
            tags: Some(tags),
            language: Some(language),
            kind: Some(kind),
            scope: Some(scope),
        },
    )
}

pub fn load_memory_cards(project_root: &Path) -> Result<Vec<MemoryCardRecord>> {
    let root = fsutil::normalize_project_root(project_root)?;
    let dir = config::kernel_dir(&root).join("memory-cards");
    load_memory_cards_from_dir(&dir)
}

pub fn load_global_memory_cards(home: &Path) -> Result<Vec<MemoryCardRecord>> {
    load_memory_cards_from_dir(&home.join(".agent-kernel").join("memory-cards"))
}

pub fn count_global_memory_cards(home: &Path) -> Result<usize> {
    count_memory_cards_in_dir(&home.join(".agent-kernel").join("memory-cards"))
}

fn load_memory_cards_from_dir(dir: &Path) -> Result<Vec<MemoryCardRecord>> {
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
        records.push(
            serde_yaml::from_str::<MemoryCardRecord>(&text)
                .with_context(|| format!("parse Memory Card {}", entry.path().display()))?,
        );
    }
    records.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(records)
}

fn count_memory_cards_in_dir(dir: &Path) -> Result<usize> {
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

pub fn set_memory_card_targets(project_root: &Path, id: &str, targets: Vec<String>) -> Result<()> {
    validate_memory_card_id(id)?;
    let root = fsutil::normalize_project_root(project_root)?;
    validate_targets(&root, &targets)?;
    let memory_cards = load_memory_cards(&root)?;
    let Some(record) = memory_cards.iter().find(|record| record.id == id) else {
        return Err(anyhow!("Memory Card `{id}` does not exist"));
    };
    let mut project = config::load_or_default_project_config(&root)?;
    if let Some(existing) = project
        .memory_cards
        .include
        .iter_mut()
        .find(|item| item.id == id)
    {
        existing.targets = targets;
        existing.scope = Some(record.scope.clone());
    } else {
        project.memory_cards.include.push(MemoryCardRef {
            id: id.to_string(),
            targets,
            scope: Some(record.scope.clone()),
        });
    }
    config::save_project_config(&root, &project)
}

pub fn clear_memory_card_targets(project_root: &Path, agent: Option<String>) -> Result<usize> {
    let root = fsutil::normalize_project_root(project_root)?;
    let mut project = config::load_or_default_project_config(&root)?;
    if let Some(agent) = agent.as_ref() {
        validate_targets(&root, std::slice::from_ref(agent))?;
    }
    let memory_cards = load_memory_cards(&root)?;
    let default_targets = enabled_agent_targets(&project);
    let mut changed = 0usize;

    for record in memory_cards {
        let existing_index = project
            .memory_cards
            .include
            .iter()
            .position(|item| item.id == record.id);
        let current_targets = existing_index
            .and_then(|index| project.memory_cards.include.get(index))
            .map(|item| item.targets.clone())
            .unwrap_or_else(|| default_targets.clone());
        let next_targets = match agent.as_ref() {
            Some(agent) => current_targets
                .into_iter()
                .filter(|target| target != agent)
                .collect::<Vec<_>>(),
            None => Vec::new(),
        };
        if existing_index
            .and_then(|index| project.memory_cards.include.get(index))
            .map(|item| item.targets.as_slice() == next_targets.as_slice())
            .unwrap_or(false)
        {
            continue;
        }
        changed += 1;
        if let Some(index) = existing_index {
            if let Some(existing) = project.memory_cards.include.get_mut(index) {
                existing.targets = next_targets;
                existing.scope = Some(record.scope);
            }
        } else {
            project.memory_cards.include.push(MemoryCardRef {
                id: record.id,
                targets: next_targets,
                scope: Some(record.scope),
            });
        }
    }
    dedupe_memory_card_refs(&mut project.memory_cards.include);
    config::save_project_config(&root, &project)?;
    Ok(changed)
}

pub fn promote_memory_card_to_global(
    project_root: &Path,
    home: &Path,
    id: &str,
) -> Result<MemoryCardRecord> {
    validate_memory_card_id(id)?;
    let root = fsutil::normalize_project_root(project_root)?;
    let mut project = config::load_or_default_project_config(&root)?;
    let Some(mut record) = memory_card_map(&root)?.remove(id) else {
        return Err(anyhow!("Memory Card `{id}` does not exist"));
    };
    let original_id = record.id.clone();
    let promoted_id = global_memory_card_id(&record)?;
    record.scope = "global".to_string();
    record.id = promoted_id.clone();
    record.source_project = Some(fsutil::path_to_slash(&root));
    record.updated_at = Utc::now().to_rfc3339();

    let global_path = global_memory_card_path(home, &promoted_id)?;
    if let Some(parent) = global_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&global_path, serde_yaml::to_string(&record)?)
        .with_context(|| format!("write {}", global_path.display()))?;

    let project_global_path = memory_card_path(&root, &promoted_id)?;
    if let Some(parent) = project_global_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&project_global_path, serde_yaml::to_string(&record)?)
        .with_context(|| format!("write {}", project_global_path.display()))?;

    let mut preserved_targets = None;
    if let Some(existing) = project
        .memory_cards
        .include
        .iter_mut()
        .find(|item| item.id == original_id || item.id == promoted_id)
    {
        if existing.id == original_id {
            preserved_targets = Some(existing.targets.clone());
        }
        existing.id = promoted_id.clone();
        existing.scope = Some("global".to_string());
    } else {
        let targets = enabled_agent_targets(&project);
        preserved_targets = Some(targets.clone());
        project.memory_cards.include.push(MemoryCardRef {
            id: promoted_id.clone(),
            targets,
            scope: Some("global".to_string()),
        });
    }
    if let Some(targets) = preserved_targets {
        if let Some(existing) = project
            .memory_cards
            .include
            .iter_mut()
            .find(|item| item.id == promoted_id)
        {
            existing.targets = targets;
        }
    }
    dedupe_memory_card_refs(&mut project.memory_cards.include);
    config::save_project_config(&root, &project)?;

    if original_id != promoted_id {
        let old_path = memory_card_path(&root, &original_id)?;
        if old_path.exists() {
            fs::remove_file(old_path)?;
        }
    }
    Ok(record)
}

pub fn install_global_memory_card_to_project(
    project_root: &Path,
    home: &Path,
    id: &str,
    targets: Vec<String>,
) -> Result<MemoryCardRecord> {
    validate_memory_card_id(id)?;
    let root = fsutil::normalize_project_root(project_root)?;
    let Some(mut record) = load_global_memory_cards(home)?
        .into_iter()
        .find(|record| record.id == id)
    else {
        return Err(anyhow!("global Memory Card `{id}` does not exist"));
    };

    config::ensure_kernel_dir(&root)?;
    record.scope = "global".to_string();
    record.updated_at = Utc::now().to_rfc3339();

    let path = memory_card_path(&root, id)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, serde_yaml::to_string(&record)?)
        .with_context(|| format!("write {}", path.display()))?;

    let mut project = config::load_or_default_project_config(&root)?;
    if let Some(existing) = project
        .memory_cards
        .include
        .iter_mut()
        .find(|item| item.id == id)
    {
        existing.targets = targets;
        existing.scope = Some("global".to_string());
    } else {
        project.memory_cards.include.push(MemoryCardRef {
            id: id.to_string(),
            targets,
            scope: Some("global".to_string()),
        });
    }
    config::save_project_config(&root, &project)?;

    Ok(record)
}

pub fn merge_memory_cards(
    project_root: &Path,
    id: &str,
    title: &str,
    source_ids: Vec<String>,
    targets: Vec<String>,
) -> Result<()> {
    validate_memory_card_id(id)?;
    let root = fsutil::normalize_project_root(project_root)?;
    let memory_cards = memory_card_map(&root)?;
    let mut body = String::new();
    let mut missing = Vec::new();

    for source_id in &source_ids {
        let Some(record) = memory_cards.get(source_id) else {
            missing.push(source_id.clone());
            continue;
        };
        body.push_str(&format!("## {}\n\n{}\n\n", record.title, record.body));
    }

    if !missing.is_empty() {
        return Err(anyhow!(
            "missing source Memory Cards: {}",
            missing.join(", ")
        ));
    }
    if source_ids.is_empty() {
        return Err(anyhow!("at least one source Memory Card is required"));
    }

    add_memory_card(
        &root,
        id,
        title,
        body.trim(),
        "procedure",
        "project",
        targets,
    )?;

    // 合并后删除源 Memory Card，只保留合并结果
    for source_id in &source_ids {
        let path = memory_card_path(&root, source_id)?;
        if path.exists() {
            fs::remove_file(&path)?;
        }
    }

    Ok(())
}

/// 删除指定 Memory Card 文件
pub fn delete_memory_card(project_root: &Path, id: &str) -> Result<()> {
    let root = fsutil::normalize_project_root(project_root)?;
    let path = memory_card_path(&root, id)?;
    if path.exists() {
        fs::remove_file(&path)?;
    }
    let mut project = config::load_or_default_project_config(&root)?;
    project.memory_cards.include.retain(|item| item.id != id);
    for supplement in &mut project.skills.supplements {
        supplement
            .memory_cards
            .retain(|memory_card_id| memory_card_id != id);
    }
    project
        .skills
        .supplements
        .retain(|supplement| !supplement.memory_cards.is_empty());
    config::save_project_config(&root, &project)
}

pub fn memory_card_map(
    project_root: &Path,
) -> Result<std::collections::BTreeMap<String, MemoryCardRecord>> {
    Ok(load_memory_cards(project_root)?
        .into_iter()
        .map(|record| (record.id.clone(), record))
        .collect())
}

pub fn memory_card_target_matrix(project_root: &Path) -> Result<MemoryCardTargetMatrix> {
    let root = fsutil::normalize_project_root(project_root)?;
    let project = config::load_or_default_project_config(&root)?;
    let memory_cards = load_memory_cards(&root)?;
    let agents = project.agents.keys().cloned().collect::<Vec<_>>();
    let refs = project
        .memory_cards
        .include
        .iter()
        .map(|item| (item.id.clone(), item.targets.clone()))
        .collect::<std::collections::BTreeMap<_, _>>();

    let rows = memory_cards
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
            MemoryCardTargetMatrixRow {
                memory_card_id: record.id,
                title: record.title,
                scope: record.scope,
                targets,
            }
        })
        .collect();

    Ok(MemoryCardTargetMatrix { agents, rows })
}

fn global_memory_card_id(record: &MemoryCardRecord) -> Result<String> {
    if record.id.starts_with("global:") {
        return Ok(record.id.clone());
    }
    let raw_slug = record
        .id
        .split_once(':')
        .map(|(_, slug)| slug)
        .filter(|slug| !slug.trim().is_empty())
        .unwrap_or(record.title.trim());
    let mut id = format!("global:{}", raw_slug.trim());
    while id.len() > MAX_MEMORY_CARD_ID_LEN {
        let Some((idx, _)) = id.char_indices().next_back() else {
            break;
        };
        id.truncate(idx);
    }
    validate_memory_card_id(&id)?;
    Ok(id)
}

fn enabled_agent_targets(project: &config::ProjectConfig) -> Vec<String> {
    project
        .agents
        .iter()
        .filter(|(_, agent)| agent.enabled)
        .map(|(name, _)| name.clone())
        .collect()
}

fn dedupe_memory_card_refs(refs: &mut Vec<MemoryCardRef>) {
    let mut seen = std::collections::BTreeSet::new();
    refs.retain(|item| seen.insert(item.id.clone()));
}

const MAX_MEMORY_CARD_ID_LEN: usize = 128;

fn memory_card_path(project_root: &Path, id: &str) -> Result<PathBuf> {
    memory_card_file_path(&config::kernel_dir(project_root).join("memory-cards"), id)
}

fn global_memory_card_path(home: &Path, id: &str) -> Result<PathBuf> {
    memory_card_file_path(&home.join(".agent-kernel").join("memory-cards"), id)
}

fn memory_card_file_path(dir: &Path, id: &str) -> Result<PathBuf> {
    validate_memory_card_id(id)?;
    let safe = id.replace(':', "/").replace(['\\', ' '], "-");
    let path = dir.join(format!("{safe}.yml"));
    ensure_memory_card_path_stays_in_dir(dir, &path)?;
    Ok(path)
}

fn validate_memory_card_id(id: &str) -> Result<()> {
    if id.trim().is_empty() {
        return Err(anyhow!("Memory Card id must not be empty"));
    }
    if id.len() > MAX_MEMORY_CARD_ID_LEN {
        return Err(anyhow!(
            "Memory Card id must be at most {MAX_MEMORY_CARD_ID_LEN} bytes"
        ));
    }
    if id.contains("..") {
        return Err(anyhow!("Memory Card id must not contain `..`"));
    }
    if id.contains('/') || id.contains('\\') {
        return Err(anyhow!("Memory Card id must not contain path separators"));
    }
    let bytes = id.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return Err(anyhow!(
            "Memory Card id must not look like an absolute path"
        ));
    }
    if Path::new(id).is_absolute() {
        return Err(anyhow!("Memory Card id must not be an absolute path"));
    }
    Ok(())
}

fn validate_memory_card_fields(
    project_root: &Path,
    title: &str,
    body: &str,
    kind: &str,
    scope: &str,
    targets: &[String],
) -> Result<()> {
    validate_non_empty("Memory Card title", title)?;
    validate_non_empty("Memory Card body", body)?;
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
        "memory-card",
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
        return Err(anyhow!("unknown Memory Card kind `{kind}`"));
    }
    Ok(())
}

fn validate_scope(scope: &str) -> Result<()> {
    const SCOPES: &[&str] = &["project", "global", "agent", "directory", "agent-specific"];
    if !SCOPES.contains(&scope) {
        return Err(anyhow!("unknown Memory Card scope `{scope}`"));
    }
    Ok(())
}

fn validate_targets(project_root: &Path, targets: &[String]) -> Result<()> {
    let project = config::load_or_default_project_config(project_root)?;
    for target in targets {
        if !project.agents.contains_key(target) {
            return Err(anyhow!("unknown Memory Card target agent `{target}`"));
        }
    }
    Ok(())
}

fn ensure_memory_card_path_stays_in_dir(dir: &Path, path: &Path) -> Result<()> {
    path.strip_prefix(dir)
        .map(|_| ())
        .with_context(|| format!("Memory Card path escaped {}", dir.display()))
}

fn normalize_tags(mut tags: Vec<String>) -> Vec<String> {
    tags.sort();
    tags.dedup();
    tags
}

#[derive(Debug, Clone)]
struct MemoryCardMetadata {
    brief: String,
    tags: Vec<String>,
    language: String,
}

fn infer_memory_card_metadata(title: &str, body: &str, kind: &str) -> MemoryCardMetadata {
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
    MemoryCardMetadata {
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
