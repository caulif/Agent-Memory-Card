use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use chrono::Utc;

use crate::config::{self, MemoryCardRef};
use crate::fsutil;

use super::{
    MAX_MEMORY_CARD_ID_LEN, MemoryCardRecord, dedupe_memory_card_refs, enabled_agent_targets,
    global_memory_card_path, load_global_memory_cards, memory_card_map, memory_card_path,
    validate_memory_card_id,
};

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
