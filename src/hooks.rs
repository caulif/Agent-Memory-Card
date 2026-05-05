use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Serialize;
use serde_json::{Map, Value, json};

use crate::fsutil;

const AGENT_KERNEL_HOOK_MARKER: &str = "agent-kernel observe evolve";

#[derive(Debug, Clone, Copy)]
pub enum ClaudeHookEvent {
    Stop,
    PreCompact,
    SessionEnd,
}

impl ClaudeHookEvent {
    fn as_str(self) -> &'static str {
        match self {
            ClaudeHookEvent::Stop => "Stop",
            ClaudeHookEvent::PreCompact => "PreCompact",
            ClaudeHookEvent::SessionEnd => "SessionEnd",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct HookInstallReport {
    pub path: PathBuf,
    pub installed_events: Vec<String>,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct HookUninstallReport {
    pub path: PathBuf,
    pub removed_handlers: usize,
}

pub fn install_claude_project_hooks(
    project_root: &Path,
    events: Vec<ClaudeHookEvent>,
    targets: Vec<String>,
) -> Result<HookInstallReport> {
    install_claude_project_hooks_with_options(project_root, events, targets, false)
}

pub fn plan_claude_project_hooks(
    project_root: &Path,
    events: Vec<ClaudeHookEvent>,
    targets: Vec<String>,
) -> Result<HookInstallReport> {
    install_claude_project_hooks_with_options(project_root, events, targets, true)
}

fn install_claude_project_hooks_with_options(
    project_root: &Path,
    events: Vec<ClaudeHookEvent>,
    targets: Vec<String>,
    dry_run: bool,
) -> Result<HookInstallReport> {
    let root = fsutil::normalize_project_root(project_root)?;
    let path = settings_local_path(&root);
    let mut settings = load_settings(&path)?;
    {
        let hooks = settings
            .entry("hooks".to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        if !hooks.is_object() {
            *hooks = Value::Object(Map::new());
        }
    }
    let hooks = settings
        .get_mut("hooks")
        .and_then(Value::as_object_mut)
        .expect("hooks object");
    let installed_events = install_into_hooks(hooks, events, targets);
    if !dry_run {
        write_settings(&path, &settings)?;
    }
    Ok(HookInstallReport {
        path,
        installed_events,
        dry_run,
    })
}

pub fn uninstall_claude_project_hooks(project_root: &Path) -> Result<HookUninstallReport> {
    let root = fsutil::normalize_project_root(project_root)?;
    let path = settings_local_path(&root);
    let mut settings = load_settings(&path)?;
    let mut removed_handlers = 0usize;

    if let Some(Value::Object(hooks)) = settings.get_mut("hooks") {
        for value in hooks.values_mut() {
            let Value::Array(groups) = value else {
                continue;
            };
            for group in &mut *groups {
                let Value::Object(group) = group else {
                    continue;
                };
                let Some(Value::Array(handlers)) = group.get_mut("hooks") else {
                    continue;
                };
                let before = handlers.len();
                handlers.retain(|handler| !is_agent_kernel_handler(handler));
                removed_handlers += before - handlers.len();
            }
            groups.retain(|group| {
                group
                    .get("hooks")
                    .and_then(Value::as_array)
                    .is_some_and(|handlers| !handlers.is_empty())
            });
        }
        hooks.retain(|_, value| value.as_array().is_some_and(|groups| !groups.is_empty()));
    }

    write_settings(&path, &settings)?;
    Ok(HookUninstallReport {
        path,
        removed_handlers,
    })
}

fn install_into_hooks(
    hooks: &mut Map<String, Value>,
    events: Vec<ClaudeHookEvent>,
    targets: Vec<String>,
) -> Vec<String> {
    let command = evolve_command(targets);
    events
        .into_iter()
        .map(|event| {
            let event_name = event.as_str().to_string();
            let groups = hooks
                .entry(event_name.clone())
                .or_insert_with(|| Value::Array(Vec::new()));
            if let Value::Array(groups) = groups {
                remove_agent_kernel_groups(groups);
                groups.push(json!({
                    "hooks": [{
                        "type": "command",
                        "command": command
                    }]
                }));
            }
            event_name
        })
        .collect::<Vec<_>>()
}

fn evolve_command(targets: Vec<String>) -> String {
    let mut command = "agent-kernel observe evolve --project \"$CLAUDE_PROJECT_DIR\"".to_string();
    for target in targets {
        command.push_str(" --target ");
        command.push_str(&target);
    }
    command
}

fn remove_agent_kernel_groups(groups: &mut Vec<Value>) {
    groups.retain(|group| {
        !group
            .get("hooks")
            .and_then(Value::as_array)
            .is_some_and(|handlers| handlers.iter().any(is_agent_kernel_handler))
    });
}

fn is_agent_kernel_handler(handler: &Value) -> bool {
    handler
        .get("command")
        .and_then(Value::as_str)
        .is_some_and(|command| command.contains(AGENT_KERNEL_HOOK_MARKER))
}

fn settings_local_path(project_root: &Path) -> PathBuf {
    project_root.join(".claude").join("settings.local.json")
}

fn load_settings(path: &Path) -> Result<Map<String, Value>> {
    if !path.exists() {
        return Ok(Map::new());
    }
    let value: Value = serde_json::from_str(&fs::read_to_string(path)?)?;
    Ok(value.as_object().cloned().unwrap_or_default())
}

fn write_settings(path: &Path, settings: &Map<String, Value>) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(settings)?)?;
    Ok(())
}
