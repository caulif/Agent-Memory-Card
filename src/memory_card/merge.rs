use std::fs;
use std::path::Path;

use anyhow::{Result, anyhow};

use crate::fsutil;

use super::{add_memory_card, memory_card_map, memory_card_path, validate_memory_card_id};

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

    for source_id in &source_ids {
        let path = memory_card_path(&root, source_id)?;
        if path.exists() {
            fs::remove_file(&path)?;
        }
    }

    Ok(())
}
