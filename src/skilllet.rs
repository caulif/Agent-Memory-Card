use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::config::{self, SkillletRef};
use crate::fsutil;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillletRecord {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub scope: String,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
}

pub fn add_skilllet(
    project_root: &Path,
    id: &str,
    title: &str,
    body: &str,
    kind: &str,
    scope: &str,
    targets: Vec<String>,
) -> Result<()> {
    let root = fsutil::normalize_project_root(project_root)?;
    config::ensure_kernel_dir(&root)?;
    let now = Utc::now().to_rfc3339();
    let path = skilllet_path(&root, id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let existing = if path.exists() {
        fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_yaml::from_str::<SkillletRecord>(&text).ok())
    } else {
        None
    };

    let record = SkillletRecord {
        id: id.to_string(),
        title: title.to_string(),
        kind: kind.to_string(),
        scope: scope.to_string(),
        body: body.to_string(),
        created_at: existing
            .as_ref()
            .map(|record| record.created_at.clone())
            .unwrap_or_else(|| now.clone()),
        updated_at: now,
    };
    fs::write(&path, serde_yaml::to_string(&record)?)
        .with_context(|| format!("write {}", path.display()))?;

    let mut project = config::load_or_default_project_config(&root)?;
    if let Some(existing) = project
        .skilllets
        .include
        .iter_mut()
        .find(|item| item.id == id)
    {
        existing.targets = targets;
        existing.scope = Some(scope.to_string());
    } else {
        project.skilllets.include.push(SkillletRef {
            id: id.to_string(),
            targets,
            scope: Some(scope.to_string()),
        });
    }
    config::save_project_config(&root, &project)?;

    Ok(())
}

pub fn load_skilllets(project_root: &Path) -> Result<Vec<SkillletRecord>> {
    let root = fsutil::normalize_project_root(project_root)?;
    let dir = config::kernel_dir(&root).join("skilllets");
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut records = Vec::new();
    for entry in walkdir::WalkDir::new(dir).follow_links(false) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.path().extension().and_then(|value| value.to_str()) != Some("yml") {
            continue;
        }
        let text = fs::read_to_string(entry.path())?;
        records.push(serde_yaml::from_str::<SkillletRecord>(&text)?);
    }
    records.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(records)
}

pub fn skilllet_map(
    project_root: &Path,
) -> Result<std::collections::BTreeMap<String, SkillletRecord>> {
    Ok(load_skilllets(project_root)?
        .into_iter()
        .map(|record| (record.id.clone(), record))
        .collect())
}

fn skilllet_path(project_root: &Path, id: &str) -> PathBuf {
    let safe = id.replace(':', "/").replace(['\\', ' '], "-");
    config::kernel_dir(project_root)
        .join("skilllets")
        .join(format!("{safe}.yml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_skilllet_writes_record_and_project_include() {
        let temp = tempfile::tempdir().expect("tempdir");

        add_skilllet(
            temp.path(),
            "project:use-axios",
            "Use Axios",
            "Use Axios for frontend HTTP requests.",
            "preference",
            "project",
            vec!["codex".to_string()],
        )
        .expect("add skilllet");

        let records = load_skilllets(temp.path()).expect("load skilllets");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].id, "project:use-axios");

        let project = config::load_or_default_project_config(temp.path()).expect("load project");
        assert_eq!(project.skilllets.include[0].id, "project:use-axios");
        assert_eq!(project.skilllets.include[0].targets, vec!["codex"]);
    }
}
