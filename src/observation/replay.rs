use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::fsutil;

use super::conversation::{ConversationProjectMatch, conversation_project_match};
use super::{
    ObservationImportReport, discover_local_conversation_files, import_observation_text,
    synthesize_observations_to_drafts_with_engine,
};

#[derive(Debug, Clone, Serialize)]
pub struct ObservationReplayReport {
    pub engine: String,
    pub discovered_sources: usize,
    pub matched_sources: usize,
    pub excluded_other_project: usize,
    pub excluded_unknown_project: usize,
    pub imported: usize,
    pub import_skipped: usize,
    pub drafts_created: usize,
    pub draft_candidates: usize,
    pub synthesis_skipped: usize,
    pub drafts: Vec<String>,
    pub candidate_drafts: Vec<String>,
    pub dry_run: bool,
}

pub fn replay_local_conversations(
    project_root: &Path,
    home: &Path,
    targets: Vec<String>,
    dry_run: bool,
    engine: &str,
    include_unknown_project: bool,
) -> Result<ObservationReplayReport> {
    let files = discover_local_conversation_files(home)?;
    let root = fsutil::normalize_project_root(project_root)?;
    let mut matched = Vec::new();
    let mut excluded_other_project = 0usize;
    let mut excluded_unknown_project = 0usize;

    for file in files.iter() {
        match conversation_project_match(file, &root, include_unknown_project) {
            ConversationProjectMatch::Match => matched.push(file.clone()),
            ConversationProjectMatch::UnknownProject => excluded_unknown_project += 1,
            ConversationProjectMatch::OtherProject => excluded_other_project += 1,
        }
    }

    let replay_temp;
    let replay_root = if dry_run {
        replay_temp = Some(tempfile::tempdir().context("create replay temp project")?);
        replay_temp.as_ref().expect("temp").path().to_path_buf()
    } else {
        root.clone()
    };

    let mut imported = ObservationImportReport {
        created: 0,
        skipped: 0,
        observations: Vec::new(),
    };
    for file in &matched {
        let text = fs::read_to_string(&file.path)
            .with_context(|| format!("read {}", file.path.display()))?;
        let report = import_observation_text(
            &replay_root,
            &file.path,
            &file.source_kind,
            Some(&file.agent),
            &text,
        )?;
        imported.created += report.created;
        imported.skipped += report.skipped;
        imported.observations.extend(report.observations);
    }

    let synthesized =
        synthesize_observations_to_drafts_with_engine(&replay_root, targets, dry_run, engine)?;

    Ok(ObservationReplayReport {
        engine: synthesized.engine,
        discovered_sources: files.len(),
        matched_sources: matched.len(),
        excluded_other_project,
        excluded_unknown_project,
        imported: imported.created,
        import_skipped: imported.skipped,
        drafts_created: synthesized.created,
        draft_candidates: synthesized.candidates,
        synthesis_skipped: synthesized.skipped,
        drafts: synthesized.drafts,
        candidate_drafts: synthesized.candidate_drafts,
        dry_run,
    })
}
