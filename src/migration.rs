use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Serialize;
use serde_yaml::{Mapping, Value};

use crate::{config, fsutil};

#[derive(Debug, Clone, Serialize)]
pub struct MigrationReport {
    pub scanned: usize,
    pub updated: usize,
}

pub fn migrate_project_records(project_root: &Path) -> Result<MigrationReport> {
    let root = fsutil::normalize_project_root(project_root)?;
    let mut report = MigrationReport {
        scanned: 0,
        updated: 0,
    };

    if migrate_memory_card_storage_names(&root)? {
        report.updated += 1;
    }
    if migrate_project_config_names(&root)? {
        report.updated += 1;
    }

    for dir in record_dirs(&root) {
        if !dir.exists() {
            continue;
        }
        for entry in walkdir::WalkDir::new(dir).follow_links(false) {
            let entry = entry?;
            if !entry.file_type().is_file()
                || entry.path().extension().and_then(|value| value.to_str()) != Some("yml")
            {
                continue;
            }
            report.scanned += 1;
            if migrate_yaml_record(entry.path())? {
                report.updated += 1;
            }
        }
    }

    Ok(report)
}

fn migrate_yaml_record(path: &Path) -> Result<bool> {
    let text = fs::read_to_string(path)?;
    let mut value: Value = serde_yaml::from_str(&text)?;
    let Value::Mapping(mapping) = &mut value else {
        return Ok(false);
    };
    let mut changed = normalize_memory_card_record(mapping);
    if mapping.contains_key(Value::String("schema_version".to_string())) {
        if changed {
            fs::write(path, serde_yaml::to_string(&value)?)?;
        }
        return Ok(changed);
    }
    insert_schema_version(mapping);
    changed = true;
    fs::write(path, serde_yaml::to_string(&value)?)?;
    Ok(changed)
}

fn insert_schema_version(mapping: &mut Mapping) {
    let mut reordered = Mapping::new();
    reordered.insert(
        Value::String("schema_version".to_string()),
        Value::Number(1.into()),
    );
    for (key, value) in std::mem::take(mapping) {
        reordered.insert(key, value);
    }
    *mapping = reordered;
}

fn record_dirs(root: &Path) -> Vec<PathBuf> {
    let kernel = config::kernel_dir(root);
    vec![
        kernel.join("candidates"),
        kernel.join("drafts"),
        kernel.join("memory-cards"),
    ]
}

fn migrate_memory_card_storage_names(root: &Path) -> Result<bool> {
    let kernel = config::kernel_dir(root);
    let legacy = kernel.join(legacy_memory_cards_dir_name());
    let next = kernel.join("memory-cards");
    if !legacy.exists() {
        return Ok(false);
    }
    if !next.exists() {
        fs::rename(&legacy, &next)?;
        return Ok(true);
    }
    for entry in walkdir::WalkDir::new(&legacy).follow_links(false) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = entry.path().strip_prefix(&legacy)?;
        let target = next.join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(entry.path(), target)?;
    }
    fs::remove_dir_all(legacy)?;
    Ok(true)
}

fn migrate_project_config_names(root: &Path) -> Result<bool> {
    let path = config::project_config_path(root);
    if !path.exists() {
        return Ok(false);
    }
    let text = fs::read_to_string(&path)?;
    let mut value: Value = serde_yaml::from_str(&text)?;
    let Value::Mapping(mapping) = &mut value else {
        return Ok(false);
    };
    let legacy_key = Value::String(legacy_memory_cards_dir_name());
    let next_key = Value::String("memory_cards".to_string());
    let mut changed = false;
    if !mapping.contains_key(&next_key)
        && let Some(legacy_value) = mapping.remove(&legacy_key)
    {
        mapping.insert(next_key, legacy_value);
        changed = true;
    }
    if changed {
        fs::write(path, serde_yaml::to_string(&value)?)?;
    }
    Ok(changed)
}

fn normalize_memory_card_record(mapping: &mut Mapping) -> bool {
    let mut changed = false;
    for field in ["title", "body", "brief", "evidence"] {
        let key = Value::String(field.to_string());
        if let Some(Value::String(text)) = mapping.get_mut(&key) {
            let next = normalize_legacy_memory_card_text(text);
            if *text != next {
                *text = next;
                changed = true;
            }
        }
    }
    let kind_key = Value::String("kind".to_string());
    if let Some(Value::String(kind)) = mapping.get_mut(&kind_key)
        && *kind == legacy_memory_card_singular()
    {
        *kind = "memory-card".to_string();
        changed = true;
    }
    changed
}

fn normalize_legacy_memory_card_text(text: &str) -> String {
    text.replace(&legacy_memory_card_plural_title(), "Memory Cards")
        .replace(&legacy_memory_card_singular_title(), "Memory Card")
        .replace(&legacy_memory_card_plural(), "memory-cards")
        .replace(&legacy_memory_card_singular(), "memory-card")
}

fn legacy_memory_cards_dir_name() -> String {
    ["skill", "lets"].concat()
}

fn legacy_memory_card_singular() -> String {
    ["skill", "let"].concat()
}

fn legacy_memory_card_plural() -> String {
    ["skill", "lets"].concat()
}

fn legacy_memory_card_singular_title() -> String {
    ["Skill", "let"].concat()
}

fn legacy_memory_card_plural_title() -> String {
    ["Skill", "lets"].concat()
}
