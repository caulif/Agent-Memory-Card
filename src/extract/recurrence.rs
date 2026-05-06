use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::config;
use crate::fsutil;
use crate::textutil;

use super::Candidate;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecurrenceIndex {
    #[serde(default)]
    pub signals: Vec<RecurringSignal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecurringSignal {
    pub signature: String,
    #[serde(default)]
    pub occurrences: Vec<RecurringOccurrence>,
    pub last_seen: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecurringOccurrence {
    pub source: String,
    pub seen_at: String,
}

pub(super) fn apply_recurrence_boost(
    project_root: &Path,
    candidates: &mut [Candidate],
) -> Result<()> {
    let index = load_recurrence_index(project_root)?;
    for candidate in candidates {
        let signature = candidate_signature(candidate);
        let Some(signal) = index
            .signals
            .iter()
            .find(|signal| signal.signature == signature)
        else {
            continue;
        };
        let count = signal.occurrences.len();
        if count < 3 {
            continue;
        }
        let confidence = candidate.confidence.unwrap_or(0.7);
        candidate.confidence = Some((confidence + 0.1).min(0.95));
        let reason = candidate.reason.clone().unwrap_or_default();
        let recurrence = format!("Recurring across {count} observations.");
        candidate.reason = Some(if reason.trim().is_empty() {
            recurrence
        } else if reason.contains("Recurring across") {
            reason
        } else {
            format!("{reason} {recurrence}")
        });
    }
    Ok(())
}

pub(super) fn record_candidate_recurrence(
    project_root: &Path,
    candidates: &[Candidate],
    source: &str,
) -> Result<()> {
    let mut index = load_recurrence_index(project_root)?;
    let now = Utc::now().to_rfc3339();
    for candidate in candidates {
        let signature = candidate_signature(candidate);
        if let Some(signal) = index
            .signals
            .iter_mut()
            .find(|signal| signal.signature == signature)
        {
            signal.last_seen = now.clone();
            signal.occurrences.push(RecurringOccurrence {
                source: source.to_string(),
                seen_at: now.clone(),
            });
        } else {
            index.signals.push(RecurringSignal {
                signature,
                occurrences: vec![RecurringOccurrence {
                    source: source.to_string(),
                    seen_at: now.clone(),
                }],
                last_seen: now.clone(),
            });
        }
    }
    save_recurrence_index(project_root, &index)
}

fn load_recurrence_index(project_root: &Path) -> Result<RecurrenceIndex> {
    let path = recurrence_index_path(project_root);
    if !path.exists() {
        return Ok(RecurrenceIndex::default());
    }
    let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    serde_yaml::from_str(&text).with_context(|| format!("parse {}", path.display()))
}

fn save_recurrence_index(project_root: &Path, index: &RecurrenceIndex) -> Result<()> {
    config::ensure_kernel_dir(project_root)?;
    let path = recurrence_index_path(project_root);
    fs::write(&path, serde_yaml::to_string(index)?)
        .with_context(|| format!("write {}", path.display()))
}

fn recurrence_index_path(project_root: &Path) -> std::path::PathBuf {
    config::kernel_dir(project_root).join("recurrence-index.yml")
}

fn candidate_signature(candidate: &Candidate) -> String {
    let normalized = format!("{} {}", candidate.kind, candidate.body.to_lowercase());
    format!(
        "sig:{}",
        textutil::slug(&fsutil::sha256_text(&normalized)[..16])
    )
}
