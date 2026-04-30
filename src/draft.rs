use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::config;
use crate::fsutil;
use crate::skilllet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftRecord {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub scope: String,
    pub body: String,
    pub targets: Vec<String>,
    pub evidence: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct NewDraft {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub scope: String,
    pub body: String,
    pub targets: Vec<String>,
    pub evidence: String,
}

pub fn add_draft(project_root: &Path, draft: NewDraft) -> Result<()> {
    let root = fsutil::normalize_project_root(project_root)?;
    config::ensure_kernel_dir(&root)?;
    let now = Utc::now().to_rfc3339();
    let record = DraftRecord {
        id: draft.id.clone(),
        title: draft.title,
        kind: draft.kind,
        scope: draft.scope,
        body: draft.body,
        targets: draft.targets,
        evidence: draft.evidence,
        status: "draft".to_string(),
        created_at: now.clone(),
        updated_at: now,
    };
    let path = draft_path(&root, &draft.id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_yaml::to_string(&record)?)?;
    Ok(())
}

pub fn load_drafts(project_root: &Path) -> Result<Vec<DraftRecord>> {
    let root = fsutil::normalize_project_root(project_root)?;
    let dir = config::kernel_dir(&root).join("drafts");
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut records: Vec<DraftRecord> = Vec::new();
    for entry in walkdir::WalkDir::new(dir).follow_links(false) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.path().extension().and_then(|value| value.to_str()) != Some("yml") {
            continue;
        }
        records.push(serde_yaml::from_str(&fs::read_to_string(entry.path())?)?);
    }
    records.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(records)
}

pub fn approve_draft(project_root: &Path, id: &str) -> Result<()> {
    let root = fsutil::normalize_project_root(project_root)?;
    let path = draft_path(&root, id);
    if !path.exists() {
        return Err(anyhow!("draft `{id}` does not exist"));
    }
    let draft: DraftRecord = serde_yaml::from_str(&fs::read_to_string(&path)?)?;
    skilllet::add_skilllet(
        &root,
        &draft.id,
        &draft.title,
        &draft.body,
        &draft.kind,
        &draft.scope,
        draft.targets,
    )?;
    fs::remove_file(path)?;
    Ok(())
}

fn draft_path(project_root: &Path, id: &str) -> PathBuf {
    let safe = id.replace(':', "/").replace(['\\', ' '], "-");
    config::kernel_dir(project_root)
        .join("drafts")
        .join(format!("{safe}.yml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approves_draft_into_skilllet() {
        let temp = tempfile::tempdir().expect("tempdir");
        add_draft(
            temp.path(),
            NewDraft {
                id: "project:prefer-pnpm".to_string(),
                title: "Prefer pnpm".to_string(),
                body: "Use pnpm for package management.".to_string(),
                kind: "preference".to_string(),
                scope: "project".to_string(),
                targets: vec!["codex".to_string()],
                evidence: "manual test".to_string(),
            },
        )
        .expect("add draft");

        approve_draft(temp.path(), "project:prefer-pnpm").expect("approve");

        assert!(load_drafts(temp.path()).expect("drafts").is_empty());
        let skilllets = skilllet::load_skilllets(temp.path()).expect("skilllets");
        assert_eq!(skilllets[0].id, "project:prefer-pnpm");
    }
}
