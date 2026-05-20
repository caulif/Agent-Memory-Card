use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use chrono::Utc;

use crate::fsutil;

use crate::candidate::ExtractionMetadata;

use super::{
    MemoryCardRecord, MemoryCardUpdate, activation, infer_activation, memory_card_path,
    normalize_tags, validate_memory_card_fields,
};

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
        record.activation = record
            .extraction
            .as_ref()
            .and_then(|metadata| metadata.classification.as_ref())
            .map(|classification| {
                activation::activation_label_from_classification(&classification.activation)
            })
            .unwrap_or_else(|| infer_activation(&record.kind).to_string());
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

pub fn update_memory_card_extraction(
    project_root: &Path,
    id: &str,
    extraction: Option<ExtractionMetadata>,
) -> Result<MemoryCardRecord> {
    let root = fsutil::normalize_project_root(project_root)?;
    let path = memory_card_path(&root, id)?;
    if !path.exists() {
        return Err(anyhow!("Memory Card `{id}` does not exist"));
    }

    let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let mut record: MemoryCardRecord = serde_yaml::from_str(&text)
        .with_context(|| format!("parse Memory Card {}", path.display()))?;
    record.extraction = extraction;
    record.updated_at = Utc::now().to_rfc3339();
    fs::write(&path, serde_yaml::to_string(&record)?)
        .with_context(|| format!("write {}", path.display()))?;
    Ok(record)
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
