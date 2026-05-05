use std::fs;

use agent_kernel::hooks;

#[test]
fn install_project_hooks_writes_review_first_claude_settings_local_config() {
    let temp = tempfile::tempdir().expect("tempdir");

    let report = hooks::install_claude_project_hooks(
        temp.path(),
        vec![
            hooks::ClaudeHookEvent::Stop,
            hooks::ClaudeHookEvent::SessionEnd,
        ],
        vec!["codex".to_string(), "claude-code".to_string()],
    )
    .expect("install hooks");

    assert_eq!(report.installed_events, vec!["Stop", "SessionEnd"]);
    assert!(!report.dry_run);
    let settings = fs::read_to_string(temp.path().join(".claude/settings.local.json"))
        .expect("settings local");
    assert!(settings.contains("\"Stop\""));
    assert!(settings.contains("\"SessionEnd\""));
    assert!(settings.contains("agent-kernel observe evolve"));
    assert!(settings.contains("--target codex"));
    assert!(settings.contains("--target claude-code"));
    assert!(!settings.contains("approve"));
    assert!(!settings.contains("sync"));
}

#[test]
fn plan_project_hooks_does_not_write_settings_file() {
    let temp = tempfile::tempdir().expect("tempdir");

    let report = hooks::plan_claude_project_hooks(
        temp.path(),
        vec![hooks::ClaudeHookEvent::Stop],
        vec!["codex".to_string()],
    )
    .expect("plan hooks");

    assert!(report.dry_run);
    assert!(!temp.path().join(".claude/settings.local.json").exists());
}

#[test]
fn uninstall_project_hooks_removes_agent_kernel_handlers_only() {
    let temp = tempfile::tempdir().expect("tempdir");
    hooks::install_claude_project_hooks(
        temp.path(),
        vec![hooks::ClaudeHookEvent::Stop],
        vec!["codex".to_string()],
    )
    .expect("install hooks");

    let report = hooks::uninstall_claude_project_hooks(temp.path()).expect("uninstall hooks");

    assert_eq!(report.removed_handlers, 1);
    let settings = fs::read_to_string(temp.path().join(".claude/settings.local.json"))
        .expect("settings local");
    assert!(!settings.contains("agent-kernel observe evolve"));
}

#[test]
fn install_project_hooks_preserves_existing_settings_keys() {
    let temp = tempfile::tempdir().expect("tempdir");
    let settings_path = temp.path().join(".claude/settings.local.json");
    fs::create_dir_all(settings_path.parent().expect("parent")).expect("settings dir");
    fs::write(
        &settings_path,
        r#"{
  "permissions": {
    "allow": ["Bash(cargo test:*)"]
  }
}"#,
    )
    .expect("existing settings");

    hooks::install_claude_project_hooks(
        temp.path(),
        vec![hooks::ClaudeHookEvent::PreCompact],
        vec!["codex".to_string()],
    )
    .expect("install hooks");

    let settings = fs::read_to_string(settings_path).expect("settings");
    assert!(settings.contains("\"permissions\""));
    assert!(settings.contains("Bash(cargo test:*)"));
    assert!(settings.contains("\"PreCompact\""));
}
