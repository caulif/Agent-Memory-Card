use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillletTargetMatrix {
    pub agents: Vec<String>,
    pub rows: Vec<SkillletTargetMatrixRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillletTargetMatrixRow {
    pub skilllet_id: String,
    pub title: String,
    pub targets: std::collections::BTreeMap<String, bool>,
}

impl SkillletTargetMatrix {
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("Agent-Kernel Skilllet Target Matrix\n\n");
        if self.rows.is_empty() {
            out.push_str("No skilllets found.\n");
            return out;
        }
        out.push_str(&format!("Skilllet | {}\n", self.agents.join(" | ")));
        out.push_str(&format!(
            "{}\n",
            std::iter::repeat_n("---", self.agents.len() + 1)
                .collect::<Vec<_>>()
                .join(" | ")
        ));
        for row in &self.rows {
            let cells = self
                .agents
                .iter()
                .map(|agent| {
                    if row.targets.get(agent).copied().unwrap_or(false) {
                        "yes"
                    } else {
                        "no"
                    }
                })
                .collect::<Vec<_>>();
            out.push_str(&format!("{} | {}\n", row.skilllet_id, cells.join(" | ")));
        }
        out
    }
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

pub fn set_skilllet_targets(project_root: &Path, id: &str, targets: Vec<String>) -> Result<()> {
    let root = fsutil::normalize_project_root(project_root)?;
    let skilllets = load_skilllets(&root)?;
    let Some(record) = skilllets.iter().find(|record| record.id == id) else {
        return Err(anyhow!("skilllet `{id}` does not exist"));
    };
    let mut project = config::load_or_default_project_config(&root)?;
    if let Some(existing) = project
        .skilllets
        .include
        .iter_mut()
        .find(|item| item.id == id)
    {
        existing.targets = targets;
        existing.scope = Some(record.scope.clone());
    } else {
        project.skilllets.include.push(SkillletRef {
            id: id.to_string(),
            targets,
            scope: Some(record.scope.clone()),
        });
    }
    config::save_project_config(&root, &project)
}

pub fn merge_skilllets(
    project_root: &Path,
    id: &str,
    title: &str,
    source_ids: Vec<String>,
    targets: Vec<String>,
) -> Result<()> {
    let root = fsutil::normalize_project_root(project_root)?;
    let skilllets = skilllet_map(&root)?;
    let mut body = String::new();
    let mut missing = Vec::new();

    for source_id in &source_ids {
        let Some(record) = skilllets.get(source_id) else {
            missing.push(source_id.clone());
            continue;
        };
        body.push_str(&format!("## {}\n\n{}\n\n", record.title, record.body));
    }

    if !missing.is_empty() {
        return Err(anyhow!("missing source skilllets: {}", missing.join(", ")));
    }
    if source_ids.is_empty() {
        return Err(anyhow!("at least one source skilllet is required"));
    }

    add_skilllet(
        &root,
        id,
        title,
        body.trim(),
        "procedure",
        "project",
        targets,
    )
}

pub fn skilllet_map(
    project_root: &Path,
) -> Result<std::collections::BTreeMap<String, SkillletRecord>> {
    Ok(load_skilllets(project_root)?
        .into_iter()
        .map(|record| (record.id.clone(), record))
        .collect())
}

pub fn skilllet_target_matrix(project_root: &Path) -> Result<SkillletTargetMatrix> {
    let root = fsutil::normalize_project_root(project_root)?;
    let project = config::load_or_default_project_config(&root)?;
    let skilllets = load_skilllets(&root)?;
    let agents = project.agents.keys().cloned().collect::<Vec<_>>();
    let refs = project
        .skilllets
        .include
        .iter()
        .map(|item| (item.id.clone(), item.targets.clone()))
        .collect::<std::collections::BTreeMap<_, _>>();

    let rows = skilllets
        .into_iter()
        .map(|record| {
            let explicit_targets = refs.get(&record.id).cloned().unwrap_or_default();
            let targets = agents
                .iter()
                .map(|agent| {
                    let assigned = if explicit_targets.is_empty() {
                        project
                            .agents
                            .get(agent)
                            .map(|agent| agent.enabled)
                            .unwrap_or(false)
                    } else {
                        explicit_targets.iter().any(|target| target == agent)
                    };
                    (agent.clone(), assigned)
                })
                .collect();
            SkillletTargetMatrixRow {
                skilllet_id: record.id,
                title: record.title,
                targets,
            }
        })
        .collect();

    Ok(SkillletTargetMatrix { agents, rows })
}

#[cfg(test)]
mod matrix_tests {
    use super::*;

    #[test]
    fn skilllet_target_matrix_marks_assigned_agents() {
        let temp = tempfile::tempdir().expect("tempdir");
        add_skilllet(
            temp.path(),
            "project:use-axios",
            "Use Axios",
            "Use Axios for frontend HTTP requests.",
            "preference",
            "project",
            vec!["codex".to_string(), "claude-code".to_string()],
        )
        .expect("add skilllet");

        let matrix = skilllet_target_matrix(temp.path()).expect("matrix");

        assert_eq!(matrix.rows[0].skilllet_id, "project:use-axios");
        assert_eq!(matrix.rows[0].targets.get("codex"), Some(&true));
        assert_eq!(matrix.rows[0].targets.get("claude-code"), Some(&true));
    }
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

    #[test]
    fn set_skilllet_targets_updates_project_include() {
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

        set_skilllet_targets(
            temp.path(),
            "project:use-axios",
            vec!["claude-code".to_string(), "codex".to_string()],
        )
        .expect("set targets");

        let project = config::load_or_default_project_config(temp.path()).expect("project");
        assert_eq!(
            project.skilllets.include[0].targets,
            vec!["claude-code", "codex"]
        );
    }

    #[test]
    fn merge_skilllets_creates_combined_skilllet() {
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
        .expect("add axios");
        add_skilllet(
            temp.path(),
            "project:prefer-pnpm",
            "Prefer pnpm",
            "Use pnpm for package management.",
            "preference",
            "project",
            vec!["codex".to_string()],
        )
        .expect("add pnpm");

        merge_skilllets(
            temp.path(),
            "project:frontend-defaults",
            "Frontend Defaults",
            vec![
                "project:use-axios".to_string(),
                "project:prefer-pnpm".to_string(),
            ],
            vec!["claude-code".to_string(), "codex".to_string()],
        )
        .expect("merge");

        let merged = skilllet_map(temp.path())
            .expect("skilllets")
            .remove("project:frontend-defaults")
            .expect("merged");
        assert!(
            merged
                .body
                .contains("Use Axios for frontend HTTP requests.")
        );
        assert!(merged.body.contains("Use pnpm for package management."));

        let project = config::load_or_default_project_config(temp.path()).expect("project");
        let merged_ref = project
            .skilllets
            .include
            .iter()
            .find(|item| item.id == "project:frontend-defaults")
            .expect("merged ref");
        assert_eq!(merged_ref.targets, vec!["claude-code", "codex"]);
    }
}
