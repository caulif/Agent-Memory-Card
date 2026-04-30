use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use chrono::Utc;

use serde::{Deserialize, Serialize};

use crate::config::{self, ArtifactState, MirrorState, ProjectLock, SkillRecord};
use crate::fsutil;
use crate::skilllet::{self, SkillletRecord};

#[derive(Debug, serde::Serialize)]
pub struct BuildReport {
    preview: bool,
    actions: Vec<String>,
    warnings: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct StatusReport {
    rows: Vec<StatusRow>,
    warnings: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
struct StatusRow {
    skill: String,
    agent: String,
    target: String,
    status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MirrorMarker {
    generated_by: String,
    mode: String,
    source: String,
    skill: String,
    agent: String,
    mirrored_at: String,
    source_hash: String,
}

impl BuildReport {
    pub fn render(&self) -> String {
        let mut out = String::new();
        if self.preview {
            out.push_str("Agent-Kernel build preview\n\n");
        } else {
            out.push_str("Agent-Kernel build complete\n\n");
        }

        if self.actions.is_empty() {
            out.push_str("No actions.\n");
        } else {
            for action in &self.actions {
                out.push_str("- ");
                out.push_str(action);
                out.push('\n');
            }
        }

        if !self.warnings.is_empty() {
            out.push_str("\nWarnings:\n");
            for warning in &self.warnings {
                out.push_str("- ");
                out.push_str(warning);
                out.push('\n');
            }
        }
        out
    }
}

impl StatusReport {
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("Agent-Kernel status\n\n");
        if self.rows.is_empty() {
            out.push_str("No mirrors declared.\n");
        } else {
            for row in &self.rows {
                out.push_str(&format!(
                    "- {} -> {}: {} ({})\n",
                    row.skill, row.agent, row.status, row.target
                ));
            }
        }
        if !self.warnings.is_empty() {
            out.push_str("\nWarnings:\n");
            for warning in &self.warnings {
                out.push_str(&format!("- {warning}\n"));
            }
        }
        out
    }
}

pub fn build_project(project_root: &Path, preview: bool) -> Result<BuildReport> {
    let root = fsutil::normalize_project_root(project_root)?;
    let config = config::load_or_default_project_config(&root)?;
    let index = config::load_skill_index(&root)?;
    let skilllets = skilllet::skilllet_map(&root)?;
    let skill_by_id = index
        .skills
        .iter()
        .map(|skill| (skill.id.clone(), skill.clone()))
        .collect::<BTreeMap<_, _>>();

    let mut actions = Vec::new();
    let mut warnings = Vec::new();
    let mut lock = ProjectLock {
        generated_at: Utc::now().to_rfc3339(),
        mirrors: Vec::new(),
        artifacts: Vec::new(),
    };

    for mirror in &config.skills.mirrors {
        let Some(skill) = skill_by_id.get(&mirror.reference) else {
            warnings.push(format!(
                "mirror references unknown skill `{}`",
                mirror.reference
            ));
            continue;
        };

        for target in &mirror.targets {
            let Some(agent) = config.agents.get(target) else {
                warnings.push(format!("mirror target `{target}` is not configured"));
                continue;
            };
            if !agent.enabled {
                warnings.push(format!("mirror target `{target}` is disabled"));
                continue;
            }
            let Some(skills_dir) = &agent.exports.skills_dir else {
                warnings.push(format!(
                    "mirror target `{target}` does not have a skills_dir export"
                ));
                continue;
            };

            let target_dir = root.join(skills_dir).join(safe_skill_dir_name(skill));
            actions.push(format!(
                "{} skill `{}` -> {}",
                if preview { "Would mirror" } else { "Mirrored" },
                skill.id,
                fsutil::path_to_slash(&target_dir)
            ));
            if !preview {
                fsutil::copy_dir_all(Path::new(&skill.source_path), &target_dir)?;
                let marker = serde_yaml::to_string(&serde_json::json!({
                    "generated_by": "agent-kernel",
                    "mode": "mirror",
                    "source": skill.source_path,
                    "skill": skill.id,
                    "agent": target,
                    "mirrored_at": Utc::now().to_rfc3339(),
                    "source_hash": skill.source_hash,
                }))?;
                fs::write(target_dir.join(".agent-kernel-mirror.yml"), marker)?;
                let target_hash =
                    fsutil::sha256_dir_excluding(&target_dir, &[".agent-kernel-mirror.yml"])?;
                lock.mirrors.push(MirrorState {
                    source: skill.source_path.clone(),
                    target: fsutil::path_to_slash(&target_dir),
                    agent: target.clone(),
                    source_hash: skill.source_hash.clone(),
                    target_hash,
                    status: "synced".to_string(),
                });
            }
        }
    }

    for (agent_name, agent) in &config.agents {
        if !agent.enabled {
            continue;
        }
        if let Some(instructions) = &agent.exports.instructions {
            let path = root.join(instructions);
            let content = render_instructions(
                agent_name,
                &config.skills.mirrors,
                &config.skilllets.include,
                &skilllets,
            );
            actions.push(format!(
                "{} {}",
                if preview { "Would write" } else { "Wrote" },
                fsutil::path_to_slash(&path)
            ));
            if !preview {
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&path, &content).with_context(|| format!("write {}", path.display()))?;
                lock.artifacts.push(ArtifactState {
                    path: fsutil::path_to_slash(&path),
                    hash: fsutil::sha256_text(&content),
                    kind: format!("{agent_name}:instructions"),
                });
            }
        }
    }

    if !preview {
        config::save_lock(&root, &lock)?;
    }

    Ok(BuildReport {
        preview,
        actions,
        warnings,
    })
}

fn render_instructions(
    agent_name: &str,
    mirrors: &[config::MirrorDecl],
    skilllet_refs: &[config::SkillletRef],
    skilllets: &BTreeMap<String, SkillletRecord>,
) -> String {
    let mut out = String::new();
    out.push_str("<!-- Generated by Agent-Kernel. Do not edit directly. Run `agent-kernel import` to ingest manual changes. -->\n\n");
    out.push_str("# Agent Kernel Instructions\n\n");
    out.push_str("This file is a build artifact for the current project.\n\n");
    out.push_str("## Enabled Skills\n\n");

    let enabled = mirrors
        .iter()
        .filter(|mirror| mirror.targets.iter().any(|target| target == agent_name))
        .collect::<Vec<_>>();

    if enabled.is_empty() {
        out.push_str("- No mirrored skills are declared for this agent yet.\n");
    } else {
        for mirror in enabled {
            out.push_str(&format!("- `{}`\n", mirror.reference));
        }
    }

    out.push_str("\n## Enabled Skilllets\n\n");
    let mut rendered = 0;
    for item in skilllet_refs {
        if !item.targets.is_empty() && !item.targets.iter().any(|target| target == agent_name) {
            continue;
        }
        if let Some(record) = skilllets.get(&item.id) {
            out.push_str(&format!("### {}\n\n{}\n\n", record.title, record.body));
            rendered += 1;
        }
    }
    if rendered == 0 {
        out.push_str("- No skilllets are declared for this agent yet.\n");
    }

    out
}

fn safe_skill_dir_name(skill: &SkillRecord) -> String {
    skill
        .id
        .split(':')
        .next_back()
        .unwrap_or(&skill.name)
        .replace(['/', '\\', ':'], "-")
}

pub fn preview_as_json(project_root: &Path) -> Result<serde_json::Value> {
    let report = build_project(project_root, true)?;
    Ok(serde_json::json!({
        "preview": true,
        "text": report.render(),
        "actions": report.actions,
        "warnings": report.warnings,
    }))
}

pub fn status_project(project_root: &Path) -> Result<StatusReport> {
    let root = fsutil::normalize_project_root(project_root)?;
    let config = config::load_or_default_project_config(&root)?;
    let index = config::load_skill_index(&root)?;
    let skill_by_id = index
        .skills
        .iter()
        .map(|skill| (skill.id.clone(), skill.clone()))
        .collect::<BTreeMap<_, _>>();

    let mut rows = Vec::new();
    let mut warnings = Vec::new();

    for mirror in &config.skills.mirrors {
        let Some(skill) = skill_by_id.get(&mirror.reference) else {
            warnings.push(format!("unknown skill `{}`", mirror.reference));
            continue;
        };
        for target in &mirror.targets {
            let Some(agent) = config.agents.get(target) else {
                warnings.push(format!("unknown agent `{target}`"));
                continue;
            };
            let Some(skills_dir) = &agent.exports.skills_dir else {
                warnings.push(format!("agent `{target}` has no skills_dir"));
                continue;
            };
            let target_dir = root.join(skills_dir).join(safe_skill_dir_name(skill));
            let status = if !target_dir.exists() {
                "missing".to_string()
            } else {
                let target_hash =
                    fsutil::sha256_dir_excluding(&target_dir, &[".agent-kernel-mirror.yml"])?;
                let current_source_hash = fsutil::sha256_dir(Path::new(&skill.source_path))?;
                let marker = read_marker(&target_dir)?;
                if target_hash == current_source_hash {
                    "synced".to_string()
                } else if let Some(marker) = marker {
                    let source_changed = marker.source_hash != current_source_hash;
                    let target_changed = marker.source_hash != target_hash;
                    match (source_changed, target_changed) {
                        (true, false) => "source updated".to_string(),
                        (false, true) => "target drifted".to_string(),
                        (true, true) => "source updated + target drifted".to_string(),
                        (false, false) => "synced".to_string(),
                    }
                } else {
                    "unmanaged target exists".to_string()
                }
            };
            rows.push(StatusRow {
                skill: mirror.reference.clone(),
                agent: target.clone(),
                target: fsutil::path_to_slash(&target_dir),
                status,
            });
        }
    }

    Ok(StatusReport { rows, warnings })
}

pub fn mirror(project_root: &Path, skill_id: &str, agent: &str) -> Result<()> {
    let root = fsutil::normalize_project_root(project_root)?;
    let config = config::load_or_default_project_config(&root)?;
    let agent_config = config
        .agents
        .get(agent)
        .ok_or_else(|| anyhow!("unknown agent `{agent}`"))?;
    if agent_config.exports.skills_dir.is_none() {
        return Err(anyhow!("agent `{agent}` does not support mirrored skills"));
    }
    config::add_mirror(&root, skill_id, agent)
}

pub fn sync_project(project_root: &Path) -> Result<BuildReport> {
    build_project(project_root, false)
}

fn read_marker(target_dir: &Path) -> Result<Option<MirrorMarker>> {
    let path = target_dir.join(".agent-kernel-mirror.yml");
    if !path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(path)?;
    Ok(Some(serde_yaml::from_str(&text)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_skill(path: &Path, body: &str) {
        fs::create_dir_all(path).expect("create skill dir");
        fs::write(
            path.join("SKILL.md"),
            format!("---\nname: demo\ndescription: Use when testing mirror status behavior\n---\n{body}\n"),
        )
        .expect("write skill");
    }

    #[test]
    fn status_distinguishes_source_updated_and_target_drifted() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        let source = root.join("source-skill");
        write_skill(&source, "# Demo");

        let mut project = config::default_project_config(root);
        project.skills.mirrors.push(config::MirrorDecl {
            reference: "local:demo".to_string(),
            targets: vec!["codex".to_string()],
        });
        config::save_project_config(root, &project).expect("save project");
        config::save_skill_index(
            root,
            &config::SkillIndex {
                generated_at: "test".to_string(),
                skills: vec![SkillRecord {
                    id: "local:demo".to_string(),
                    name: "demo".to_string(),
                    description: "Use when testing mirror status behavior".to_string(),
                    source_path: fsutil::path_to_slash(&source),
                    source_kind: "referenced".to_string(),
                    source_hash: fsutil::sha256_dir(&source).expect("source hash"),
                    warnings: Vec::new(),
                }],
            },
        )
        .expect("save index");

        sync_project(root).expect("sync");
        let synced = status_project(root).expect("status");
        assert_eq!(synced.rows[0].status, "synced");

        write_skill(&source, "# Demo\nUpdated");
        let source_updated = status_project(root).expect("status");
        assert_eq!(source_updated.rows[0].status, "source updated");

        let target = root.join(".agents").join("skills").join("demo");
        fs::write(target.join("LOCAL.md"), "local edit").expect("target edit");
        let both = status_project(root).expect("status");
        assert_eq!(both.rows[0].status, "source updated + target drifted");
    }

    #[test]
    fn render_instructions_includes_targeted_skilllets() {
        let mut skilllets = BTreeMap::new();
        skilllets.insert(
            "project:use-axios".to_string(),
            SkillletRecord {
                id: "project:use-axios".to_string(),
                title: "Use Axios".to_string(),
                kind: "preference".to_string(),
                scope: "project".to_string(),
                body: "Use Axios for frontend requests.".to_string(),
                created_at: "now".to_string(),
                updated_at: "now".to_string(),
            },
        );
        let refs = vec![config::SkillletRef {
            id: "project:use-axios".to_string(),
            targets: vec!["codex".to_string()],
            scope: Some("project".to_string()),
        }];

        let codex = render_instructions("codex", &[], &refs, &skilllets);
        let claude = render_instructions("claude-code", &[], &refs, &skilllets);

        assert!(codex.contains("Use Axios for frontend requests."));
        assert!(!claude.contains("Use Axios for frontend requests."));
    }
}
