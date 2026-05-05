use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow};

use crate::config::{self, ArtifactState, ProjectLock};
use crate::fsutil;
use crate::skilllet::SkillletRecord;

use super::ensure_generated_artifact_is_safe_to_write;

#[derive(Debug, Default)]
pub(super) struct CompiledSkilllets {
    pub(super) actions: Vec<String>,
    pub(super) warnings: Vec<String>,
    pub(super) artifacts: Vec<ArtifactState>,
}

pub(super) fn compile_skilllets_as_agent_skills(
    root: &Path,
    agent_name: &str,
    skills_dir: &str,
    skilllet_refs: &[config::SkillletRef],
    skilllets: &BTreeMap<String, SkillletRecord>,
    preview: bool,
    previous_lock: &ProjectLock,
) -> Result<CompiledSkilllets> {
    let mut compiled = CompiledSkilllets::default();
    for item in skilllet_refs {
        if !item.targets.iter().any(|target| target == agent_name) {
            continue;
        }
        let Some(record) = skilllets.get(&item.id) else {
            continue;
        };
        if !skilllet_compiles_to_agent_skill(record) {
            continue;
        }

        let skill_name = agent_skill_name(record);
        let skill_dir = root.join(skills_dir).join(&skill_name);
        let skill_path = skill_dir.join("SKILL.md");
        let references_dir = skill_dir.join("references");
        let reference_path = references_dir.join(format!("{}.md", safe_file_stem(&record.id)));
        let skill_content = render_agent_skill(record, &skill_name);
        let reference_content = render_agent_skill_reference(record);

        compiled.actions.push(format!(
            "{} Agent Skill `{}` -> {}",
            if preview { "Would compile" } else { "Compiled" },
            record.id,
            fsutil::path_to_slash(&skill_dir)
        ));
        let errors = validate_agent_skill_content(&skill_name, &skill_content);
        if !errors.is_empty() {
            compiled.warnings.extend(errors);
            if !preview {
                return Err(anyhow!(
                    "compiled Agent Skill `{skill_name}` is invalid; run preview for details"
                ));
            }
        }
        if preview {
            continue;
        }

        ensure_generated_artifact_is_safe_to_write(&skill_path, previous_lock)?;
        ensure_generated_artifact_is_safe_to_write(&reference_path, previous_lock)?;
        fs::create_dir_all(&references_dir)?;
        fs::write(&skill_path, &skill_content)
            .with_context(|| format!("write {}", skill_path.display()))?;
        fs::write(&reference_path, &reference_content)
            .with_context(|| format!("write {}", reference_path.display()))?;
        compiled.artifacts.push(ArtifactState {
            path: fsutil::path_to_slash(&skill_path),
            hash: fsutil::sha256_text(&skill_content),
            kind: format!("{agent_name}:agent-skill"),
        });
        compiled.artifacts.push(ArtifactState {
            path: fsutil::path_to_slash(&reference_path),
            hash: fsutil::sha256_text(&reference_content),
            kind: format!("{agent_name}:agent-skill-reference"),
        });
    }
    Ok(compiled)
}

pub(super) fn skilllet_compiles_to_agent_skill(record: &SkillletRecord) -> bool {
    if record.activation == "skill" {
        return true;
    }
    if record.activation != "model-decision" && record.activation != "always-on" {
        return false;
    }
    if matches!(record.kind.as_str(), "procedure" | "template" | "workflow") {
        return true;
    }
    record
        .extraction
        .as_ref()
        .and_then(|metadata| metadata.suggested_action.as_ref())
        .is_some_and(|action| action.route == "workflow_skill")
}

fn render_agent_skill(record: &SkillletRecord, skill_name: &str) -> String {
    let description = agent_skill_description(record);
    let reference_file = format!("{}.md", safe_file_stem(&record.id));
    format!(
        "---\nname: {skill_name}\ndescription: {description}\n---\n\n# {}\n\nThis skill was compiled from Agent-Kernel Skilllets.\n\n## Rules\n\n- See `references/{}` for the full rule.\n",
        record.title, reference_file
    )
}

fn render_agent_skill_reference(record: &SkillletRecord) -> String {
    format!(
        "# {}\n\n{}\n\n## Metadata\n\n- Skilllet: `{}`\n- Kind: `{}`\n- Scope: `{}`\n",
        record.title, record.body, record.id, record.kind, record.scope
    )
}

fn agent_skill_description(record: &SkillletRecord) -> String {
    let brief = if let Some(trigger) = record.trigger_description.as_deref()
        && !trigger.trim().is_empty()
    {
        trigger.trim()
    } else if record.brief.trim().is_empty() {
        record.title.trim()
    } else {
        record.brief.trim()
    };
    let mut description = if brief.to_lowercase().starts_with("use when") {
        brief.to_string()
    } else {
        format!("Use when {}", brief.trim_end_matches('.').to_lowercase())
    };
    if description.len() > 1024 {
        description.truncate(1024);
    }
    description
}

fn agent_skill_name(record: &SkillletRecord) -> String {
    let title_slug = kebab_case(&record.title);
    if title_slug.is_empty() {
        safe_file_stem(&record.id)
    } else {
        title_slug
    }
}

fn safe_file_stem(value: &str) -> String {
    let stem = kebab_case(value);
    if stem.is_empty() {
        "skilllet".to_string()
    } else {
        stem
    }
}

fn kebab_case(value: &str) -> String {
    let mut out = String::new();
    let mut last_was_dash = false;
    for ch in value.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_lowercase() || ch.is_ascii_digit() {
            out.push(ch);
            last_was_dash = false;
        } else if !last_was_dash && !out.is_empty() {
            out.push('-');
            last_was_dash = true;
        }
        if out.len() >= 64 {
            break;
        }
    }
    out.trim_matches('-').to_string()
}

fn validate_agent_skill_content(skill_name: &str, content: &str) -> Vec<String> {
    let mut warnings = Vec::new();
    if !is_valid_agent_skill_name(skill_name) {
        warnings.push(format!(
            "Agent Skill name `{skill_name}` must be kebab-case"
        ));
    }
    if !content.contains("\ndescription: ") {
        warnings.push(format!("Agent Skill `{skill_name}` is missing description"));
    }
    warnings
}

fn is_valid_agent_skill_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
        && !value.starts_with('-')
        && !value.ends_with('-')
        && !value.contains("--")
}
