use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow};

use crate::config::{self, ArtifactState, ProjectLock};
use crate::fsutil;
use crate::memory_card::MemoryCardRecord;

use super::{ExpectedArtifact, ensure_generated_artifact_is_safe_to_write};

#[derive(Debug, Default)]
pub(super) struct CompiledMemoryCards {
    pub(super) actions: Vec<String>,
    pub(super) warnings: Vec<String>,
    pub(super) artifacts: Vec<ArtifactState>,
}

struct AgentSkillCompileContext<'a> {
    root: &'a Path,
    agent_name: &'a str,
    skills_dir: &'a str,
    preview: bool,
    previous_lock: &'a ProjectLock,
}

pub(super) fn compile_memory_cards_as_agent_skills(
    root: &Path,
    agent_name: &str,
    skills_dir: &str,
    memory_card_refs: &[config::MemoryCardRef],
    memory_cards: &BTreeMap<String, MemoryCardRecord>,
    preview: bool,
    previous_lock: &ProjectLock,
) -> Result<CompiledMemoryCards> {
    let mut compiled = CompiledMemoryCards::default();
    for item in memory_card_refs {
        if !item.targets.iter().any(|target| target == agent_name) {
            continue;
        }
        let Some(record) = memory_cards.get(&item.id) else {
            continue;
        };
        if !memory_card_compiles_to_agent_skill(record) {
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
    let context = AgentSkillCompileContext {
        root,
        agent_name,
        skills_dir,
        preview,
        previous_lock,
    };
    compile_kernel_memory_card_agent_skill(
        &context,
        memory_card_refs,
        memory_cards,
        &mut compiled,
    )?;
    Ok(compiled)
}

pub(super) fn expected_agent_skill_artifacts(
    root: &Path,
    agent_name: &str,
    skills_dir: &str,
    memory_card_refs: &[config::MemoryCardRef],
    memory_cards: &BTreeMap<String, MemoryCardRecord>,
) -> Vec<ExpectedArtifact> {
    let mut artifacts = Vec::new();
    for item in memory_card_refs {
        if !item.targets.iter().any(|target| target == agent_name) {
            continue;
        }
        let Some(record) = memory_cards.get(&item.id) else {
            continue;
        };
        if !memory_card_compiles_to_agent_skill(record) {
            continue;
        }

        let skill_name = agent_skill_name(record);
        let skill_dir = root.join(skills_dir).join(&skill_name);
        artifacts.push(ExpectedArtifact {
            agent: agent_name.to_string(),
            path: skill_dir.join("SKILL.md"),
            expected_content: render_agent_skill(record, &skill_name),
            kind: format!("{agent_name}:agent-skill"),
        });
        artifacts.push(ExpectedArtifact {
            agent: agent_name.to_string(),
            path: skill_dir
                .join("references")
                .join(format!("{}.md", safe_file_stem(&record.id))),
            expected_content: render_agent_skill_reference(record),
            kind: format!("{agent_name}:agent-skill-reference"),
        });
    }

    let skill_dir = root.join(skills_dir).join("agent-kernel-memory-card");
    artifacts.push(ExpectedArtifact {
        agent: agent_name.to_string(),
        path: skill_dir.join("SKILL.md"),
        expected_content: render_kernel_memory_card_skill(),
        kind: format!("{agent_name}:agent-kernel-memory-card-skill"),
    });
    artifacts.push(ExpectedArtifact {
        agent: agent_name.to_string(),
        path: skill_dir.join("references").join("memory-cards.md"),
        expected_content: render_kernel_memory_card_reference(
            agent_name,
            memory_card_refs,
            memory_cards,
        ),
        kind: format!("{agent_name}:agent-kernel-memory-card-reference"),
    });

    artifacts
}

fn compile_kernel_memory_card_agent_skill(
    context: &AgentSkillCompileContext<'_>,
    memory_card_refs: &[config::MemoryCardRef],
    memory_cards: &BTreeMap<String, MemoryCardRecord>,
    compiled: &mut CompiledMemoryCards,
) -> Result<()> {
    let skill_dir = context
        .root
        .join(context.skills_dir)
        .join("agent-kernel-memory-card");
    let skill_path = skill_dir.join("SKILL.md");
    let references_dir = skill_dir.join("references");
    let reference_path = references_dir.join("memory-cards.md");
    let skill_content = render_kernel_memory_card_skill();
    let reference_content =
        render_kernel_memory_card_reference(context.agent_name, memory_card_refs, memory_cards);

    compiled.actions.push(format!(
        "{} Agent Memory Kernel Memory Card Agent Skill -> {}",
        if context.preview {
            "Would compile"
        } else {
            "Compiled"
        },
        fsutil::path_to_slash(&skill_dir)
    ));
    if context.preview {
        return Ok(());
    }

    ensure_generated_artifact_is_safe_to_write(&skill_path, context.previous_lock)?;
    ensure_generated_artifact_is_safe_to_write(&reference_path, context.previous_lock)?;
    fs::create_dir_all(&references_dir)?;
    fs::write(&skill_path, &skill_content)
        .with_context(|| format!("write {}", skill_path.display()))?;
    fs::write(&reference_path, &reference_content)
        .with_context(|| format!("write {}", reference_path.display()))?;
    compiled.artifacts.push(ArtifactState {
        path: fsutil::path_to_slash(&skill_path),
        hash: fsutil::sha256_text(&skill_content),
        kind: format!("{}:agent-kernel-memory-card-skill", context.agent_name),
    });
    compiled.artifacts.push(ArtifactState {
        path: fsutil::path_to_slash(&reference_path),
        hash: fsutil::sha256_text(&reference_content),
        kind: format!("{}:agent-kernel-memory-card-reference", context.agent_name),
    });
    Ok(())
}

pub(super) fn memory_card_compiles_to_agent_skill(record: &MemoryCardRecord) -> bool {
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

fn render_agent_skill(record: &MemoryCardRecord, skill_name: &str) -> String {
    let description = agent_skill_description(record);
    let reference_file = format!("{}.md", safe_file_stem(&record.id));
    format!(
        "---\nname: {skill_name}\ndescription: {description}\n---\n\n# {}\n\nThis skill was compiled from Agent Memory Kernel Memory Cards.\n\n## Rules\n\n- See `references/{}` for the full rule.\n",
        record.title, reference_file
    )
}

fn render_agent_skill_reference(record: &MemoryCardRecord) -> String {
    format!(
        "# {}\n\n{}\n\n## Metadata\n\n- Memory Card: `{}`\n- Kind: `{}`\n- Scope: `{}`\n",
        record.title, record.body, record.id, record.kind, record.scope
    )
}

fn render_kernel_memory_card_skill() -> String {
    r#"---
name: agent-kernel-memory-card
description: Use when an agent needs to list, approve, edit, assign, merge, or sync Agent Memory Kernel Memory Cards without opening the UI.
---

# Agent Memory Kernel Memory Card

This skill was compiled from Agent Memory Kernel Memory Cards.

## Workflow

- Read `references/memory-cards.md` first to see the current generated library summary.
- Prefer MCP tools when available: `list_memory_cards`, `update_memory_card`, `assign_memory_card`, `merge_memory_cards`, and `sync_project`.
- When using the CLI, call `agent-kernel memory-card ...` subcommands for Memory Card operations and `agent-kernel draft approve ...` to approve a Draft into a Memory Card.
- Mutating operations must use the `agent-managed` Kernel policy explicitly and rely on Agent Memory Kernel audit logging; do not edit `.agent-kernel/memory-cards` files by hand.
- Keep UI and agent operations synchronized by treating `.agent-kernel/memory-cards` plus `.agent-kernel/project.yml` as the single source of truth, then run `sync_project` or `agent-kernel project sync` after assignment changes.
"#
    .to_string()
}

fn render_kernel_memory_card_reference(
    agent_name: &str,
    memory_card_refs: &[config::MemoryCardRef],
    memory_cards: &BTreeMap<String, MemoryCardRecord>,
) -> String {
    let mut out = String::new();
    out.push_str("# Current Memory Cards\n\n");
    out.push_str(
        "Generated from `.agent-kernel/memory-cards` and `.agent-kernel/project.yml`.\n\n",
    );
    let mut rendered = 0;
    for item in memory_card_refs {
        let assigned = item.targets.iter().any(|target| target == agent_name);
        let Some(record) = memory_cards.get(&item.id) else {
            continue;
        };
        rendered += 1;
        out.push_str(&format!(
            "- `{}` [{}] {} - {}\n",
            record.id,
            if assigned { "assigned" } else { "available" },
            record.title,
            record.kind
        ));
    }
    if rendered == 0 {
        out.push_str("- No Memory Cards are declared yet.\n");
    }
    out
}

fn agent_skill_description(record: &MemoryCardRecord) -> String {
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

fn agent_skill_name(record: &MemoryCardRecord) -> String {
    let title_slug = kebab_case(&record.title);
    if title_slug.is_empty() {
        safe_file_stem(&record.id)
    } else {
        title_slug
    }
}

fn safe_file_stem(value: &str) -> String {
    let stem = kebab_case(value);
    if stem.is_empty() || (!value.is_ascii() && stem.len() <= 16) {
        format!("memory-card-{}", short_stable_hash(value))
    } else {
        stem
    }
}

fn short_stable_hash(value: &str) -> String {
    fsutil::sha256_text(value)
        .trim_start_matches("sha256:")
        .chars()
        .take(12)
        .collect()
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
