use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::config::{self, ProjectLock};
use crate::fsutil;
use crate::memory_card::MemoryCardRecord;

use super::{ExpectedArtifact, expected_artifacts};

const DIFF_PREVIEW_LIMIT: usize = 20;
const DIFF_LINES_LIMIT: usize = 240;

#[derive(Debug, Clone, Serialize)]
pub struct ArtifactPreviewRow {
    pub agent: String,
    pub kind: String,
    pub path: String,
    pub status: String,
    pub current_hash: Option<String>,
    pub expected_hash: String,
    pub diff_preview: Vec<String>,
    pub diff_lines: Vec<String>,
    pub diff_truncated: bool,
}

pub fn artifact_preview_rows_from(
    root: &Path,
    config: &config::ProjectConfig,
    memory_cards: &BTreeMap<String, MemoryCardRecord>,
    previous_lock: &ProjectLock,
) -> Result<Vec<ArtifactPreviewRow>> {
    let expected = expected_artifacts(root, config, memory_cards)?;
    expected
        .into_iter()
        .map(|artifact| artifact_preview_row(artifact, previous_lock))
        .collect()
}

fn artifact_preview_row(
    artifact: ExpectedArtifact,
    previous_lock: &ProjectLock,
) -> Result<ArtifactPreviewRow> {
    let path = fsutil::path_to_slash(&artifact.path);
    let expected_hash = fsutil::sha256_text(&artifact.expected_content);
    let previous_hash = previous_lock
        .artifacts
        .iter()
        .find(|state| state.path == path)
        .map(|state| state.hash.as_str());

    let (status, current_hash, current_content) = if artifact.path.exists() {
        let current = fs::read_to_string(&artifact.path)
            .with_context(|| format!("read artifact {}", artifact.path.display()))?;
        let current_hash = fsutil::sha256_text(&current);
        let status = if current_hash == expected_hash {
            "unchanged"
        } else if previous_hash.is_some_and(|hash| hash != current_hash) {
            "drifted"
        } else {
            "update"
        };
        (status.to_string(), Some(current_hash), Some(current))
    } else {
        ("create".to_string(), None, None)
    };

    let (diff_lines, diff_truncated) = diff_lines(
        current_content.as_deref(),
        &artifact.expected_content,
        DIFF_LINES_LIMIT,
    );

    Ok(ArtifactPreviewRow {
        agent: artifact.agent,
        kind: artifact.kind,
        path,
        status,
        current_hash,
        expected_hash,
        diff_preview: diff_lines
            .iter()
            .take(DIFF_PREVIEW_LIMIT)
            .cloned()
            .collect(),
        diff_lines,
        diff_truncated,
    })
}

fn diff_lines(current: Option<&str>, expected: &str, limit: usize) -> (Vec<String>, bool) {
    let mut lines = Vec::new();
    let mut truncated = false;
    match current {
        None => {
            for line in expected.lines() {
                lines.push(format!("+ {line}"));
                if lines.len() >= limit {
                    truncated = true;
                    break;
                }
            }
        }
        Some(current) if current == expected => {}
        Some(current) => {
            let current_lines = current.lines().collect::<Vec<_>>();
            let expected_lines = expected.lines().collect::<Vec<_>>();
            let max_len = current_lines.len().max(expected_lines.len());
            for index in 0..max_len {
                let before = current_lines.get(index).copied();
                let after = expected_lines.get(index).copied();
                if before == after {
                    continue;
                }
                if let Some(before) = before {
                    lines.push(format!("- {before}"));
                    if lines.len() >= limit {
                        truncated = true;
                        break;
                    }
                }
                if let Some(after) = after {
                    lines.push(format!("+ {after}"));
                    if lines.len() >= limit {
                        truncated = true;
                        break;
                    }
                }
            }
        }
    }
    (lines, truncated)
}
