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
