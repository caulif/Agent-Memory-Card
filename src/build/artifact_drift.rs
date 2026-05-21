use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::candidate::ExtractionMetadata;
use crate::config::{self, ProjectLock};
use crate::draft;
use crate::fsutil;
use crate::memory_card;

use super::{ExpectedArtifact, expected_artifacts, upsert_artifact_lock};

#[derive(Debug, serde::Serialize)]
pub struct ArtifactImportReport {
    pub created: usize,
    pub skipped: usize,
    pub drafts: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct ArtifactDriftResolutionReport {
    pub resolved: usize,
    pub skipped: usize,
    pub paths: Vec<String>,
}

impl ArtifactImportReport {
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("Agent Memory Kernel artifact import\n\n");
        out.push_str(&format!("Created drafts: {}\n", self.created));
        out.push_str(&format!("Skipped artifacts: {}\n", self.skipped));
        if !self.drafts.is_empty() {
            out.push_str("\nDrafts:\n");
            for draft in &self.drafts {
                out.push_str(&format!("- {draft}\n"));
            }
        }
        out
    }
}

pub fn import_artifact_drifts(project_root: &Path) -> Result<ArtifactImportReport> {
    import_artifact_drifts_matching(project_root, None)
}

pub fn import_artifact_drift_path(
    project_root: &Path,
    artifact_path: &str,
) -> Result<ArtifactImportReport> {
    import_artifact_drifts_matching(project_root, Some(artifact_path))
}

fn import_artifact_drifts_matching(
    project_root: &Path,
    only_artifact_path: Option<&str>,
) -> Result<ArtifactImportReport> {
    let root = fsutil::normalize_project_root(project_root)?;
    let config = config::load_or_default_project_config(&root)?;
    let memory_cards = memory_card::memory_card_map(&root)?;
    let expected = expected_artifacts(&root, &config, &memory_cards)?;
    let mut lock = config::load_lock(&root)?;
    let mut lock_changed = false;

    let mut created = 0;
    let mut skipped = 0;
    let mut drafts = Vec::new();

    for artifact in expected {
        if !artifact_path_matches(&root, &artifact, only_artifact_path) {
            skipped += 1;
            continue;
        }
        if !artifact.path.exists() {
            skipped += 1;
            continue;
        }
        let actual = fs::read_to_string(&artifact.path)
            .with_context(|| format!("read artifact {}", artifact.path.display()))?;
        if fsutil::sha256_text(&actual) == fsutil::sha256_text(&artifact.expected_content) {
            upsert_artifact_lock(
                &mut lock,
                &artifact,
                fsutil::sha256_text(&artifact.expected_content),
            );
            lock_changed = true;
            skipped += 1;
            continue;
        }

        let extracted = extract_added_lines(&artifact.expected_content, &actual);
        let body = if extracted.trim().is_empty() {
            actual.trim().to_string()
        } else {
            extracted
        };
        if body.trim().is_empty() {
            skipped += 1;
            continue;
        }

        let id = format!(
            "artifact:{}:{}",
            artifact.agent,
            &fsutil::sha256_text(&format!("{}:{body}", artifact.path.display()))[..12]
        );
        draft::add_draft(
            &root,
            draft::NewDraft {
                id: id.clone(),
                title: format!("Manual artifact changes for {}", artifact.agent),
                kind: "preference".to_string(),
                scope: "project".to_string(),
                body,
                targets: vec![artifact.agent.clone()],
                evidence: format!("Imported from {}", fsutil::path_to_slash(&artifact.path)),
                confidence: None,
                reason: None,
                matched_template: None,
                extraction: ExtractionMetadata::default(),
            },
        )?;
        let current_hash = fsutil::sha256_text(&actual);
        upsert_artifact_lock(&mut lock, &artifact, current_hash);
        lock_changed = true;
        drafts.push(id);
        created += 1;
    }

    if lock_changed {
        config::save_lock(&root, &lock)?;
    }

    Ok(ArtifactImportReport {
        created,
        skipped,
        drafts,
    })
}

pub fn keep_artifact_drifts(project_root: &Path) -> Result<ArtifactDriftResolutionReport> {
    resolve_artifact_drifts(project_root, ArtifactDriftResolution::KeepCurrent, None)
}

pub fn discard_artifact_drifts(project_root: &Path) -> Result<ArtifactDriftResolutionReport> {
    resolve_artifact_drifts(project_root, ArtifactDriftResolution::DiscardCurrent, None)
}

pub fn keep_artifact_drift_path(
    project_root: &Path,
    artifact_path: &str,
) -> Result<ArtifactDriftResolutionReport> {
    resolve_artifact_drifts(
        project_root,
        ArtifactDriftResolution::KeepCurrent,
        Some(artifact_path),
    )
}

pub fn discard_artifact_drift_path(
    project_root: &Path,
    artifact_path: &str,
) -> Result<ArtifactDriftResolutionReport> {
    resolve_artifact_drifts(
        project_root,
        ArtifactDriftResolution::DiscardCurrent,
        Some(artifact_path),
    )
}

enum ArtifactDriftResolution {
    KeepCurrent,
    DiscardCurrent,
}

fn resolve_artifact_drifts(
    project_root: &Path,
    resolution: ArtifactDriftResolution,
    only_artifact_path: Option<&str>,
) -> Result<ArtifactDriftResolutionReport> {
    let root = fsutil::normalize_project_root(project_root)?;
    let config = config::load_or_default_project_config(&root)?;
    let memory_cards = memory_card::memory_card_map(&root)?;
    let expected = expected_artifacts(&root, &config, &memory_cards)?;
    let mut lock = config::load_lock(&root)?;
    let mut resolved = 0;
    let mut skipped = 0;
    let mut paths = Vec::new();

    for artifact in expected {
        if !artifact_path_matches(&root, &artifact, only_artifact_path) {
            skipped += 1;
            continue;
        }
        if !artifact.path.exists() {
            skipped += 1;
            continue;
        }

        let actual = fs::read_to_string(&artifact.path)
            .with_context(|| format!("read artifact {}", artifact.path.display()))?;
        let actual_hash = fsutil::sha256_text(&actual);
        let expected_hash = fsutil::sha256_text(&artifact.expected_content);
        if actual_hash == expected_hash {
            upsert_artifact_lock(&mut lock, &artifact, expected_hash);
            skipped += 1;
            continue;
        }
        if !is_drifted_artifact(&lock, &artifact, &actual_hash) {
            skipped += 1;
            continue;
        }

        match resolution {
            ArtifactDriftResolution::KeepCurrent => {
                upsert_artifact_lock(&mut lock, &artifact, actual_hash);
            }
            ArtifactDriftResolution::DiscardCurrent => {
                if let Some(parent) = artifact.path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&artifact.path, &artifact.expected_content)
                    .with_context(|| format!("write artifact {}", artifact.path.display()))?;
                upsert_artifact_lock(&mut lock, &artifact, expected_hash);
            }
        }
        resolved += 1;
        paths.push(artifact_report_path(&root, &artifact));
    }

    if resolved > 0 {
        config::save_lock(&root, &lock)?;
    }

    Ok(ArtifactDriftResolutionReport {
        resolved,
        skipped,
        paths,
    })
}

fn artifact_report_path(root: &Path, artifact: &ExpectedArtifact) -> String {
    artifact
        .path
        .strip_prefix(root)
        .map(fsutil::path_to_slash)
        .unwrap_or_else(|_| fsutil::path_to_slash(&artifact.path))
}

fn artifact_path_matches(
    root: &Path,
    artifact: &ExpectedArtifact,
    only_artifact_path: Option<&str>,
) -> bool {
    let Some(needle) = only_artifact_path else {
        return true;
    };
    let normalized_needle = needle
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string();
    let artifact_label = fsutil::path_to_slash(&artifact.path);
    let relative_label = artifact
        .path
        .strip_prefix(root)
        .map(fsutil::path_to_slash)
        .unwrap_or_else(|_| artifact_label.clone());
    artifact_label == normalized_needle
        || relative_label == normalized_needle
        || artifact_label.ends_with(&format!("/{normalized_needle}"))
}

fn is_drifted_artifact(
    lock: &ProjectLock,
    artifact: &ExpectedArtifact,
    current_hash: &str,
) -> bool {
    let path_label = fsutil::path_to_slash(&artifact.path);
    lock.artifacts
        .iter()
        .find(|previous| previous.path == path_label)
        .is_none_or(|previous| previous.hash != current_hash)
}

fn extract_added_lines(expected: &str, actual: &str) -> String {
    let mut expected_counts = BTreeMap::<&str, usize>::new();
    for line in expected.lines() {
        *expected_counts.entry(line).or_default() += 1;
    }

    let mut added = Vec::new();
    for line in actual.lines() {
        if let Some(count) = expected_counts.get_mut(line)
            && *count > 0
        {
            *count -= 1;
            continue;
        }
        added.push(line);
    }
    added.join("\n")
}
