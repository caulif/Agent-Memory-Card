use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::{candidate, config, draft, fsutil, skilllet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectIndex {
    pub schema_version: u32,
    pub updated_at: String,
    pub candidate_count: usize,
    pub draft_count: usize,
    pub skilllet_count: usize,
}

pub fn rebuild_project_index(project_root: &Path) -> Result<ProjectIndex> {
    let root = fsutil::normalize_project_root(project_root)?;
    config::ensure_kernel_dir(&root)?;
    let index = ProjectIndex {
        schema_version: 1,
        updated_at: Utc::now().to_rfc3339(),
        candidate_count: candidate::load_candidates(&root)?.len(),
        draft_count: draft::load_drafts(&root)?.len(),
        skilllet_count: skilllet::load_skilllets(&root)?.len(),
    };
    fs::write(index_path(&root), serde_yaml::to_string(&index)?)?;
    Ok(index)
}

pub fn load_project_index(project_root: &Path) -> Result<ProjectIndex> {
    let root = fsutil::normalize_project_root(project_root)?;
    let path = index_path(&root);
    if path.exists() {
        return Ok(serde_yaml::from_str(&fs::read_to_string(path)?)?);
    }
    rebuild_project_index(&root)
}

fn index_path(project_root: &Path) -> PathBuf {
    config::kernel_dir(project_root).join("index.yml")
}
