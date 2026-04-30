use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use chrono::Utc;

use crate::config::{self, ArtifactState, MirrorState, ProjectLock, SkillRecord};
use crate::fsutil;

#[derive(Debug)]
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
            let content = render_instructions(agent_name, &config.skills.mirrors);
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

fn render_instructions(agent_name: &str, mirrors: &[config::MirrorDecl]) -> String {
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
        "text": report.render()
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
                if target_hash == skill.source_hash {
                    "synced".to_string()
                } else if target_dir.join(".agent-kernel-mirror.yml").exists() {
                    "target drifted".to_string()
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
