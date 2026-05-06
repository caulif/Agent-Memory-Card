use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Serialize;
use serde_json::{Map, Value, json};

use crate::fsutil;

const AGENT_KERNEL_HOOK_MARKER: &str = "agent-kernel observe evolve";
const AGENT_KERNEL_MANAGED_MARKER_KEY: &str = "agentKernelManaged";

#[derive(Debug, Clone, Copy)]
pub enum ClaudeHookEvent {
    Stop,
    PreCompact,
    SessionEnd,
}

impl ClaudeHookEvent {
    pub fn as_str(self) -> &'static str {
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

#[derive(Debug, Clone)]
pub struct ClaudeCommandHook {
    pub event: ClaudeHookEvent,
    pub matcher: Option<String>,
    pub command: String,
}

pub fn install_claude_project_hooks(
    project_root: &Path,
    events: Vec<ClaudeHookEvent>,
    targets: Vec<String>,
) -> Result<HookInstallReport> {
    let hooks = events
        .into_iter()
        .map(|event| ClaudeCommandHook {
            event,
            matcher: None,
            command: evolve_command(&targets),
        })
        .collect::<Vec<_>>();
    write_claude_project_command_hooks(project_root, &hooks, false)
}

pub fn plan_claude_project_hooks(
    project_root: &Path,
    events: Vec<ClaudeHookEvent>,
    targets: Vec<String>,
) -> Result<HookInstallReport> {
    let hooks = events
        .into_iter()
        .map(|event| ClaudeCommandHook {
            event,
            matcher: None,
            command: evolve_command(&targets),
        })
        .collect::<Vec<_>>();
    write_claude_project_command_hooks(project_root, &hooks, true)
}

pub fn write_claude_project_command_hooks(
    project_root: &Path,
    command_hooks: &[ClaudeCommandHook],
    dry_run: bool,
) -> Result<HookInstallReport> {
    let root = fsutil::normalize_project_root(project_root)?;
    let path = claude_settings_local_path(&root);
    let (settings, installed_events) = render_command_hook_settings(&path, command_hooks)?;
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
    let path = claude_settings_local_path(&root);
    let mut settings = load_settings(&path)?;
    let mut removed_handlers = 0usize;

    if let Some(Value::Object(hooks)) = settings.get_mut("hooks") {
        for value in hooks.values_mut() {
            let Value::Array(groups) = value else {
                continue;
            };
            let before = groups.len();
            let removed_in_groups = groups
                .iter()
                .filter(|group| is_agent_kernel_group(group))
                .map(group_handler_count)
                .sum::<usize>();
            groups.retain(|group| !is_agent_kernel_group(group));
            removed_handlers += removed_in_groups;
            if before == groups.len() {
                continue;
            }
        }
        hooks.retain(|_, value| value.as_array().is_some_and(|groups| !groups.is_empty()));
    }

    write_settings(&path, &settings)?;
    Ok(HookUninstallReport {
        path,
        removed_handlers,
    })
}

pub fn render_claude_project_hook_settings(
    project_root: &Path,
    command_hooks: &[ClaudeCommandHook],
) -> Result<(PathBuf, String, Vec<String>)> {
    let root = fsutil::normalize_project_root(project_root)?;
    let path = claude_settings_local_path(&root);
    let (settings, installed_events) = render_command_hook_settings(&path, command_hooks)?;
    Ok((
        path,
        serde_json::to_string_pretty(&settings)?,
        installed_events,
    ))
}

pub fn claude_settings_local_path(project_root: &Path) -> PathBuf {
    project_root.join(".claude").join("settings.local.json")
}

fn render_command_hook_settings(
    path: &Path,
    command_hooks: &[ClaudeCommandHook],
) -> Result<(Map<String, Value>, Vec<String>)> {
    let mut settings = load_settings(path)?;
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
    let installed_events = install_command_hooks(hooks, command_hooks);
    Ok((settings, installed_events))
}

fn install_command_hooks(
    hooks: &mut Map<String, Value>,
    command_hooks: &[ClaudeCommandHook],
) -> Vec<String> {
    let mut installed_events = Vec::new();
    let mut hooks_by_event = Vec::<(String, Vec<&ClaudeCommandHook>)>::new();

    for hook in command_hooks {
        let event_name = hook.event.as_str().to_string();
        if let Some((_, event_hooks)) = hooks_by_event
            .iter_mut()
            .find(|(name, _)| *name == event_name)
        {
            event_hooks.push(hook);
        } else {
            hooks_by_event.push((event_name, vec![hook]));
        }
    }

    for (event_name, event_hooks) in hooks_by_event {
        let groups = hooks
            .entry(event_name.clone())
            .or_insert_with(|| Value::Array(Vec::new()));
        if let Value::Array(groups) = groups {
            remove_agent_kernel_groups(groups);
            for hook in event_hooks {
                let mut group = Map::new();
                group.insert(
                    AGENT_KERNEL_MANAGED_MARKER_KEY.to_string(),
                    Value::Bool(true),
                );
                if let Some(matcher) = hook.matcher.as_deref()
                    && !matcher.trim().is_empty()
                {
                    group.insert("matcher".to_string(), Value::String(matcher.to_string()));
                }
                group.insert(
                    "hooks".to_string(),
                    Value::Array(vec![json!({
                        "type": "command",
                        "command": hook.command
                    })]),
                );
                groups.push(Value::Object(group));
            }
        }
        installed_events.push(event_name);
    }

    installed_events
}

fn evolve_command(targets: &[String]) -> String {
    let mut command = "agent-kernel observe evolve --project \"$CLAUDE_PROJECT_DIR\"".to_string();
    for target in targets {
        command.push_str(" --target ");
        command.push_str(target);
    }
    command
}

fn remove_agent_kernel_groups(groups: &mut Vec<Value>) {
    groups.retain(|group| !is_agent_kernel_group(group));
}

fn is_agent_kernel_group(group: &Value) -> bool {
    group
        .get(AGENT_KERNEL_MANAGED_MARKER_KEY)
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || group
            .get("hooks")
            .and_then(Value::as_array)
            .is_some_and(|handlers| handlers.iter().any(is_agent_kernel_handler))
}

fn group_handler_count(group: &Value) -> usize {
    group
        .get("hooks")
        .and_then(Value::as_array)
        .map(|handlers| handlers.len())
        .unwrap_or(0)
}

fn is_agent_kernel_handler(handler: &Value) -> bool {
    handler
        .get("command")
        .and_then(Value::as_str)
        .is_some_and(|command| command.contains(AGENT_KERNEL_HOOK_MARKER))
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
