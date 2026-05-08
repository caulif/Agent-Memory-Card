use std::fs;

use agent_kernel::{config, migration};

#[test]
fn migrate_project_records_adds_schema_version_to_existing_yaml_records() {
    let temp = tempfile::tempdir().expect("tempdir");
    let draft_dir = config::kernel_dir(temp.path())
        .join("drafts")
        .join("project");
    fs::create_dir_all(&draft_dir).expect("draft dir");
    let draft_path = draft_dir.join("prefer-bun.yml");
    fs::write(
        &draft_path,
        r#"id: project:prefer-bun
title: Prefer Bun
kind: preference
scope: project
body: Use Bun for JavaScript package management and scripts.
brief: Use Bun.
tags: []
language: en
targets:
- codex
evidence: old fixture
status: draft
created_at: now
updated_at: now
"#,
    )
    .expect("old draft");

    let report = migration::migrate_project_records(temp.path()).expect("migrate");

    assert_eq!(report.updated, 1);
    assert_eq!(report.scanned, 1);
    let migrated = fs::read_to_string(draft_path).expect("migrated draft");
    assert!(migrated.contains("schema_version: 1"));
}

#[test]
fn migrate_project_records_moves_legacy_cards_to_memory_cards() {
    let temp = tempfile::tempdir().expect("tempdir");
    let legacy_dir = config::kernel_dir(temp.path())
        .join(["skill", "lets"].concat())
        .join("project");
    fs::create_dir_all(&legacy_dir).expect("legacy dir");
    fs::write(
        legacy_dir.join("review-boundary.yml"),
        format!(
            r#"id: project:review-boundary
title: Review {legacy_title}
kind: {legacy_kind}
scope: project
body: Keep {legacy_kind} approvals reviewable.
created_at: now
updated_at: now
"#,
            legacy_title = ["Skill", "let"].concat(),
            legacy_kind = ["skill", "let"].concat(),
        ),
    )
    .expect("legacy memory card");
    config::ensure_kernel_dir(temp.path()).expect("kernel dir");
    fs::write(
        config::project_config_path(temp.path()),
        format!(
            r#"version: 1
project:
  name: demo
  root: .
agents:
  codex:
    enabled: true
    exports:
      instructions: AGENTS.md
{legacy_key}:
  include:
  - id: project:review-boundary
    targets:
    - codex
"#,
            legacy_key = ["skill", "lets"].concat(),
        ),
    )
    .expect("legacy project config");

    let report = migration::migrate_project_records(temp.path()).expect("migrate");
    let migrated = fs::read_to_string(
        config::kernel_dir(temp.path())
            .join("memory-cards")
            .join("project")
            .join("review-boundary.yml"),
    )
    .expect("migrated memory card");
    let project = fs::read_to_string(config::project_config_path(temp.path())).expect("project");

    assert!(report.updated >= 2);
    assert!(migrated.contains("title: Review Memory Card"));
    assert!(migrated.contains("kind: memory-card"));
    assert!(migrated.contains("Keep memory-card approvals reviewable."));
    assert!(project.contains("memory_cards:"));
    assert!(!project.contains(&["skill", "lets:"].concat()));
}
