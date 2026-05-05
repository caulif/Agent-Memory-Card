use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::Result;

use crate::config::{self, ArtifactState, ProjectLock};
use crate::fsutil;
use crate::hooks::{self, ClaudeCommandHook, ClaudeHookEvent};
use crate::skilllet::SkillletRecord;

use super::ensure_generated_artifact_is_safe_to_write;

#[derive(Debug, Default)]
pub(super) struct CompiledHookArtifacts {
    pub(super) actions: Vec<String>,
    pub(super) warnings: Vec<String>,
    pub(super) artifacts: Vec<ArtifactState>,
}

pub(super) fn compile_skilllet_hooks(
    root: &Path,
    agent_name: &str,
    skilllet_refs: &[config::SkillletRef],
    skilllets: &BTreeMap<String, SkillletRecord>,
    preview: bool,
    previous_lock: &ProjectLock,
) -> Result<CompiledHookArtifacts> {
    let mut compiled = CompiledHookArtifacts::default();
    if agent_name != "claude-code" {
        return Ok(compiled);
    }

    let mut command_hooks = Vec::new();
    for item in skilllet_refs {
        if !item.targets.iter().any(|target| target == agent_name) {
            continue;
        }
        let Some(record) = skilllets.get(&item.id) else {
            continue;
        };
        if record.activation != "hook" {
            continue;
        }
        match parse_skilllet_hook(record) {
            Ok(parsed) => command_hooks.extend(parsed),
            Err(warning) => compiled
                .warnings
                .push(format!("hook skilllet `{}` skipped: {warning}", record.id)),
        }
    }

    if command_hooks.is_empty() {
        return Ok(compiled);
    }

    let (path, content, installed_events) =
        hooks::render_claude_project_hook_settings(root, &command_hooks)?;
    compiled.actions.push(format!(
        "{} {} ({})",
        if preview {
            "Would write Claude hooks"
        } else {
            "Wrote Claude hooks"
        },
        fsutil::path_to_slash(&path),
        installed_events.join(", ")
    ));

    if preview {
        return Ok(compiled);
    }

    ensure_generated_artifact_is_safe_to_write(&path, previous_lock)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, &content)?;
    compiled.artifacts.push(ArtifactState {
        path: fsutil::path_to_slash(&path),
        hash: fsutil::sha256_text(&content),
        kind: "claude-code:hooks".to_string(),
    });
    Ok(compiled)
}

pub(super) fn expected_hook_artifact(
    root: &Path,
    agent_name: &str,
    skilllet_refs: &[config::SkillletRef],
    skilllets: &BTreeMap<String, SkillletRecord>,
) -> Result<Option<(std::path::PathBuf, String)>> {
    if agent_name != "claude-code" {
        return Ok(None);
    }
    let mut command_hooks = Vec::new();
    for item in skilllet_refs {
        if !item.targets.iter().any(|target| target == agent_name) {
            continue;
        }
        let Some(record) = skilllets.get(&item.id) else {
            continue;
        };
        if record.activation != "hook" {
            continue;
        }
        if let Ok(parsed) = parse_skilllet_hook(record) {
            command_hooks.extend(parsed);
        }
    }
    if command_hooks.is_empty() {
        return Ok(None);
    }
    let (path, content, _) = hooks::render_claude_project_hook_settings(root, &command_hooks)?;
    Ok(Some((path, content)))
}

fn parse_skilllet_hook(record: &SkillletRecord) -> std::result::Result<Vec<ClaudeCommandHook>, String> {
    let command = record.body.trim();
    if command.is_empty() {
        return Err("body must contain the hook command".to_string());
    }

    let events = record
        .tags
        .iter()
        .filter_map(|tag| parse_event_tag(tag))
        .collect::<Vec<_>>();
    if events.is_empty() {
        return Err(
            "missing event tag; add one of hook:event:stop, hook:event:precompact, hook:event:session-end"
                .to_string(),
        );
    }

    let matcher = record.tags.iter().find_map(|tag| {
        tag.strip_prefix("hook:matcher:")
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    });

    Ok(events
        .into_iter()
        .map(|event| ClaudeCommandHook {
            event,
            matcher: matcher.clone(),
            command: command.to_string(),
        })
        .collect())
}

fn parse_event_tag(tag: &str) -> Option<ClaudeHookEvent> {
    let value = tag.strip_prefix("hook:event:")?;
    match value {
        "stop" => Some(ClaudeHookEvent::Stop),
        "precompact" => Some(ClaudeHookEvent::PreCompact),
        "session-end" => Some(ClaudeHookEvent::SessionEnd),
        _ => None,
    }
}

