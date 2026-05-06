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
    pub normalized: String,
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
        let Some(signal) = find_recurring_signal(&index, candidate) else {
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
        let normalized = normalized_candidate_text(candidate);
        if let Some(signal) = find_recurring_signal_index(&index, candidate)
            .and_then(|signal_index| index.signals.get_mut(signal_index))
        {
            signal.last_seen = now.clone();
            if signal.normalized.trim().is_empty() {
                signal.normalized = normalized;
            }
            signal.occurrences.push(RecurringOccurrence {
                source: source.to_string(),
                seen_at: now.clone(),
            });
        } else {
            index.signals.push(RecurringSignal {
                signature,
                normalized,
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
    let normalized = normalized_candidate_text(candidate);
    format!(
        "sig:{}",
        textutil::slug(&fsutil::sha256_text(&normalized)[..16])
    )
}

fn find_recurring_signal<'a>(
    index: &'a RecurrenceIndex,
    candidate: &Candidate,
) -> Option<&'a RecurringSignal> {
    find_recurring_signal_index(index, candidate).and_then(|i| index.signals.get(i))
}

fn find_recurring_signal_index(index: &RecurrenceIndex, candidate: &Candidate) -> Option<usize> {
    let signature = candidate_signature(candidate);
    let normalized = normalized_candidate_text(candidate);
    index.signals.iter().position(|signal| {
        signal.signature == signature
            || (!signal.normalized.trim().is_empty()
                && textutil::jaccard_similarity(&normalized, &signal.normalized) >= 0.50)
    })
}

fn normalized_candidate_text(candidate: &Candidate) -> String {
    let lower = format!("{} {}", candidate.kind, candidate.body.to_lowercase());
    let replacements = [
        ("javascript", " js "),
        ("typescript", " ts "),
        ("package management", " package-manager "),
        ("package manager", " package-manager "),
        ("包管理", " package-manager "),
        ("依赖和脚本", " dependencies scripts "),
        ("依赖", " dependencies "),
        ("脚本", " scripts "),
        ("统一走", " use "),
        ("统一用", " use "),
        ("默认用", " use "),
        ("使用", " use "),
        ("采用", " use "),
        ("不要再建议", " avoid "),
        ("不要", " avoid "),
        ("禁止", " avoid "),
        ("必须", " must "),
        ("以后", " "),
        ("这个项目", " "),
        ("the project", " "),
        ("this project", " "),
        ("for", " "),
        ("and", " "),
        ("the", " "),
    ];
    let mut normalized = lower;
    for (from, to) in replacements {
        normalized = normalized.replace(from, to);
    }
    textutil::tokenize(&normalized)
        .into_iter()
        .collect::<Vec<_>>()
        .join(" ")
}
