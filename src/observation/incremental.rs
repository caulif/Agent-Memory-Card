use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::{config, fsutil};

use super::ConversationFile;
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ObservationIndex {
    pub version: u32,
    #[serde(default)]
    pub sources: Vec<ObservationSourceState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationSourceState {
    pub source_path: String,
    pub agent: String,
    pub source_kind: String,
    pub processed_bytes: u64,
    pub source_hash: String,
    #[serde(default)]
    pub modified_at_unix_ms: u128,
    pub updated_at: String,
}

pub(super) fn read_incremental_conversation_text(
    file: &ConversationFile,
    index: &mut ObservationIndex,
) -> Result<Option<String>> {
    let metadata =
        fs::metadata(&file.path).with_context(|| format!("stat {}", file.path.display()))?;
    let size = metadata.len();
    let modified_at_unix_ms = metadata_modified_unix_ms(&metadata);
    let key = fsutil::path_to_slash(&file.path);
    let now = Utc::now().to_rfc3339();

    let existing_position = index
        .sources
        .iter()
        .position(|source| source.source_path == key);
    if let Some(position) = existing_position {
        let source = &index.sources[position];
        if source.processed_bytes == size
            && source.modified_at_unix_ms == modified_at_unix_ms
            && source.modified_at_unix_ms > 0
        {
            return Ok(None);
        }
    }

    let start = existing_position
        .and_then(|position| index.sources.get(position))
        .and_then(|source| (size > source.processed_bytes).then_some(source.processed_bytes))
        .unwrap_or(0);
    let bytes = read_file_from_offset(&file.path, start)?;
    let text = String::from_utf8_lossy(&bytes).to_string();
    let state = ObservationSourceState {
        source_path: key,
        agent: file.agent.clone(),
        source_kind: file.source_kind.clone(),
        processed_bytes: size,
        source_hash: metadata_signature(size, modified_at_unix_ms),
        modified_at_unix_ms,
        updated_at: now,
    };
    if let Some(position) = existing_position {
        index.sources[position] = state;
    } else {
        index.sources.push(state);
    }
    Ok(Some(text))
}

pub(super) fn read_file_from_offset(path: &Path, offset: u64) -> Result<Vec<u8>> {
    let mut file = fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    if offset > 0 {
        file.seek(SeekFrom::Start(offset))
            .with_context(|| format!("seek {}", path.display()))?;
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .with_context(|| format!("read {}", path.display()))?;
    Ok(bytes)
}

pub(super) fn metadata_modified_unix_ms(metadata: &fs::Metadata) -> u128 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

pub(super) fn metadata_signature(size: u64, modified_at_unix_ms: u128) -> String {
    format!("meta:{size}:{modified_at_unix_ms}")
}

pub(super) fn load_observation_index(project_root: &Path) -> Result<ObservationIndex> {
    let path = observation_index_path(project_root);
    if !path.exists() {
        return Ok(ObservationIndex {
            version: 1,
            sources: Vec::new(),
        });
    }
    let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let mut index: ObservationIndex =
        serde_yaml::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
    index.version = index.version.max(1);
    Ok(index)
}

pub(super) fn save_observation_index(project_root: &Path, index: &ObservationIndex) -> Result<()> {
    let path = observation_index_path(project_root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_yaml::to_string(index)?)?;
    Ok(())
}

pub(super) fn observation_index_path(project_root: &Path) -> PathBuf {
    config::kernel_dir(project_root).join("observation-index.yml")
}
