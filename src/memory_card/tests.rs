use super::*;

#[test]
fn memory_card_target_matrix_marks_assigned_agents() {
    let temp = tempfile::tempdir().expect("tempdir");
    add_memory_card(
        temp.path(),
        "project:use-axios",
        "Use Axios",
        "Use Axios for frontend HTTP requests.",
        "preference",
        "project",
        vec!["codex".to_string(), "claude-code".to_string()],
    )
    .expect("add memory_card");

    let matrix = memory_card_target_matrix(temp.path()).expect("matrix");

    assert_eq!(matrix.rows[0].memory_card_id, "project:use-axios");
    assert_eq!(matrix.rows[0].scope, "project");
    assert_eq!(matrix.rows[0].targets.get("codex"), Some(&true));
    assert_eq!(matrix.rows[0].targets.get("claude-code"), Some(&true));
}

#[test]
fn new_procedure_memory_card_defaults_to_skill_activation() {
    let temp = tempfile::tempdir().expect("tempdir");
    add_memory_card(
        temp.path(),
        "project:frontend-workflow",
        "Frontend Workflow",
        "Use Axios for frontend HTTP requests.",
        "procedure",
        "project",
        vec!["claude-code".to_string()],
    )
    .expect("add memory_card");

    let records = load_memory_cards(temp.path()).expect("records");

    assert_eq!(records[0].activation, "skill");
}

#[test]
fn add_memory_card_writes_record_and_project_include() {
    let temp = tempfile::tempdir().expect("tempdir");

    add_memory_card(
        temp.path(),
        "project:use-axios",
        "Use Axios",
        "Use Axios for frontend HTTP requests.",
        "preference",
        "project",
        vec!["codex".to_string()],
    )
    .expect("add memory_card");

    let records = load_memory_cards(temp.path()).expect("load memory_cards");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].id, "project:use-axios");
    assert!(records[0].brief.contains("Use Axios"));
    assert!(records[0].tags.contains(&"frontend".to_string()));
    assert_eq!(records[0].language, "en");

    let project = config::load_or_default_project_config(temp.path()).expect("load project");
    assert_eq!(project.memory_cards.include[0].id, "project:use-axios");
    assert_eq!(project.memory_cards.include[0].targets, vec!["codex"]);
}

#[test]
fn set_memory_card_targets_updates_project_include() {
    let temp = tempfile::tempdir().expect("tempdir");
    add_memory_card(
        temp.path(),
        "project:use-axios",
        "Use Axios",
        "Use Axios for frontend HTTP requests.",
        "preference",
        "project",
        vec!["codex".to_string()],
    )
    .expect("add memory_card");

    set_memory_card_targets(
        temp.path(),
        "project:use-axios",
        vec!["claude-code".to_string(), "codex".to_string()],
    )
    .expect("set targets");

    let project = config::load_or_default_project_config(temp.path()).expect("project");
    assert_eq!(
        project.memory_cards.include[0].targets,
        vec!["claude-code", "codex"]
    );
}

#[test]
fn set_memory_card_targets_allows_empty_inactive_assignment() {
    let temp = tempfile::tempdir().expect("tempdir");
    add_memory_card(
        temp.path(),
        "project:use-axios",
        "Use Axios",
        "Use Axios for frontend HTTP requests.",
        "preference",
        "project",
        vec!["codex".to_string()],
    )
    .expect("add memory_card");

    set_memory_card_targets(temp.path(), "project:use-axios", Vec::new()).expect("clear targets");

    let project = config::load_or_default_project_config(temp.path()).expect("project");
    assert!(project.memory_cards.include[0].targets.is_empty());
    let matrix = memory_card_target_matrix(temp.path()).expect("matrix");
    assert_eq!(matrix.rows[0].targets.get("codex"), Some(&false));
    assert_eq!(matrix.rows[0].targets.get("claude-code"), Some(&false));
}

#[test]
fn delete_memory_card_removes_file_and_project_assignment() {
    let temp = tempfile::tempdir().expect("tempdir");
    add_memory_card(
        temp.path(),
        "project:use-axios",
        "Use Axios",
        "Use Axios for frontend HTTP requests.",
        "preference",
        "project",
        vec!["codex".to_string()],
    )
    .expect("add memory_card");

    delete_memory_card(temp.path(), "project:use-axios").expect("delete memory_card");

    assert!(
        load_memory_cards(temp.path())
            .expect("memory_cards")
            .is_empty()
    );
    let project = config::load_or_default_project_config(temp.path()).expect("project");
    assert!(project.memory_cards.include.is_empty());
    let matrix = memory_card_target_matrix(temp.path()).expect("matrix");
    assert!(matrix.rows.is_empty());
}

#[test]
fn clear_memory_card_targets_removes_one_agent_without_deleting_memory_cards() {
    let temp = tempfile::tempdir().expect("tempdir");
    add_memory_card(
        temp.path(),
        "project:use-axios",
        "Use Axios",
        "Use Axios for frontend HTTP requests.",
        "preference",
        "project",
        vec!["codex".to_string(), "claude-code".to_string()],
    )
    .expect("add axios");
    add_memory_card(
        temp.path(),
        "project:prefer-bun",
        "Prefer Bun",
        "Use Bun for JavaScript package management and scripts.",
        "preference",
        "project",
        vec!["codex".to_string()],
    )
    .expect("add bun");

    let changed =
        clear_memory_card_targets(temp.path(), Some("codex".to_string())).expect("clear codex");

    assert_eq!(changed, 2);
    let records = load_memory_cards(temp.path()).expect("memory_cards");
    assert_eq!(records.len(), 2);
    let project = config::load_or_default_project_config(temp.path()).expect("project");
    let axios = project
        .memory_cards
        .include
        .iter()
        .find(|item| item.id == "project:use-axios")
        .expect("axios ref");
    let bun = project
        .memory_cards
        .include
        .iter()
        .find(|item| item.id == "project:prefer-bun")
        .expect("bun ref");
    assert_eq!(axios.targets, vec!["claude-code"]);
    assert!(bun.targets.is_empty());
}

#[test]
fn clear_memory_card_targets_without_agent_clears_every_assignment() {
    let temp = tempfile::tempdir().expect("tempdir");
    add_memory_card(
        temp.path(),
        "project:use-axios",
        "Use Axios",
        "Use Axios for frontend HTTP requests.",
        "preference",
        "project",
        vec!["codex".to_string(), "claude-code".to_string()],
    )
    .expect("add axios");
    add_memory_card(
        temp.path(),
        "project:prefer-bun",
        "Prefer Bun",
        "Use Bun for JavaScript package management and scripts.",
        "preference",
        "project",
        vec!["codex".to_string()],
    )
    .expect("add bun");

    let changed = clear_memory_card_targets(temp.path(), None).expect("clear all");

    assert_eq!(changed, 2);
    let project = config::load_or_default_project_config(temp.path()).expect("project");
    assert!(
        project
            .memory_cards
            .include
            .iter()
            .all(|item| item.targets.is_empty())
    );
    let matrix = memory_card_target_matrix(temp.path()).expect("matrix");
    assert!(
        matrix
            .rows
            .iter()
            .flat_map(|row| row.targets.values())
            .all(|assigned| !assigned)
    );
}

#[test]
fn update_memory_card_updates_body_tags_and_brief_without_changing_created_at() {
    let temp = tempfile::tempdir().expect("tempdir");
    add_memory_card(
        temp.path(),
        "project:editable",
        "Editable",
        "Original body.",
        "preference",
        "project",
        vec!["codex".to_string()],
    )
    .expect("add memory_card");
    let original = memory_card_map(temp.path())
        .expect("memory_cards")
        .remove("project:editable")
        .expect("original");

    let updated = update_memory_card(
        temp.path(),
        "project:editable",
        MemoryCardUpdate {
            body: Some("Updated body.".to_string()),
            brief: Some("Short editor summary.".to_string()),
            tags: Some(vec![
                "testing".to_string(),
                "frontend".to_string(),
                "testing".to_string(),
            ]),
            ..MemoryCardUpdate::default()
        },
    )
    .expect("update memory_card");

    assert_eq!(updated.body, "Updated body.");
    assert_eq!(updated.brief, "Short editor summary.");
    assert_eq!(updated.tags, vec!["frontend", "testing"]);
    assert_eq!(updated.created_at, original.created_at);
    assert_ne!(updated.updated_at, original.updated_at);

    let persisted = memory_card_map(temp.path())
        .expect("memory_cards")
        .remove("project:editable")
        .expect("updated");
    assert_eq!(persisted.body, "Updated body.");
    assert_eq!(persisted.tags, vec!["frontend", "testing"]);
}

#[test]
fn memory_card_editing_rejects_invalid_fields_and_targets() {
    let temp = tempfile::tempdir().expect("tempdir");
    let empty_title = add_memory_card(
        temp.path(),
        "project:empty",
        " ",
        "Valid body.",
        "preference",
        "project",
        vec!["codex".to_string()],
    )
    .expect_err("empty title should fail");
    assert!(empty_title.to_string().contains("title"));

    let bad_target = add_memory_card(
        temp.path(),
        "project:bad-target",
        "Bad Target",
        "Valid body.",
        "preference",
        "project",
        vec!["cursor".to_string()],
    )
    .expect_err("unknown target should fail");
    assert!(bad_target.to_string().contains("target agent"));

    add_memory_card(
        temp.path(),
        "project:editable",
        "Editable",
        "Original body.",
        "preference",
        "project",
        vec!["codex".to_string()],
    )
    .expect("add memory_card");
    let bad_kind = update_memory_card(
        temp.path(),
        "project:editable",
        MemoryCardUpdate {
            kind: Some("mystery".to_string()),
            ..MemoryCardUpdate::default()
        },
    )
    .expect_err("unknown kind should fail");
    assert!(bad_kind.to_string().contains("kind"));

    let bad_assignment =
        set_memory_card_targets(temp.path(), "project:editable", vec!["cursor".to_string()])
            .expect_err("unknown assignment target should fail");
    assert!(bad_assignment.to_string().contains("target agent"));
}

#[test]
fn memory_card_write_paths_reject_malicious_ids() {
    let temp = tempfile::tempdir().expect("tempdir");
    let home = tempfile::tempdir().expect("home");
    add_memory_card(
        temp.path(),
        "project:safe",
        "Safe",
        "Safe body.",
        "preference",
        "project",
        vec!["codex".to_string()],
    )
    .expect("add safe memory_card");

    for id in [
        "",
        "../escape",
        "project/escape",
        "project\\escape",
        "/absolute",
        "C:\\absolute",
        &"a".repeat(129),
    ] {
        assert!(
            add_memory_card(
                temp.path(),
                id,
                "Bad",
                "Bad body.",
                "preference",
                "project",
                Vec::new(),
            )
            .is_err()
        );
        assert!(
            update_memory_card(
                temp.path(),
                id,
                MemoryCardUpdate {
                    body: Some("Bad body.".to_string()),
                    ..MemoryCardUpdate::default()
                },
            )
            .is_err()
        );
        assert!(promote_memory_card_to_global(temp.path(), home.path(), id).is_err());
        assert!(
            install_global_memory_card_to_project(temp.path(), home.path(), id, Vec::new())
                .is_err()
        );
        assert!(
            merge_memory_cards(
                temp.path(),
                id,
                "Merged",
                vec!["project:safe".to_string()],
                Vec::new(),
            )
            .is_err()
        );
    }

    assert!(!temp.path().join("escape.yml").exists());
    assert!(!home.path().join("absolute.yml").exists());
}

#[test]
fn promote_memory_card_to_global_copies_record_with_source_project() {
    let project = tempfile::tempdir().expect("project");
    let home = tempfile::tempdir().expect("home");
    add_memory_card(
            project.path(),
            "project:ui-background-tasks",
            "UI 后台任务",
            "处理 Tauri UI 的扫描、整理和编译时，把长任务放到后台，前端只刷新状态和进度，避免点击后卡死。",
            "procedure",
            "project",
            vec!["codex".to_string()],
        )
        .expect("add memory_card");

    let promoted =
        promote_memory_card_to_global(project.path(), home.path(), "project:ui-background-tasks")
            .expect("promote");

    assert_eq!(promoted.scope, "global");
    let source_project = fsutil::path_to_slash(project.path());
    assert_eq!(
        promoted.source_project.as_deref(),
        Some(source_project.as_str())
    );
    let global = load_global_memory_cards(home.path()).expect("global memory_cards");
    assert_eq!(global.len(), 1);
    assert_eq!(global[0].id, "global:ui-background-tasks");
    assert!(global[0].tags.contains(&"ui-design".to_string()));
    let project_records = load_memory_cards(project.path()).expect("project memory_cards");
    assert_eq!(project_records.len(), 1);
    assert_eq!(project_records[0].id, "global:ui-background-tasks");
    let project_config =
        config::load_or_default_project_config(project.path()).expect("project config");
    assert_eq!(
        project_config.memory_cards.include[0].id,
        "global:ui-background-tasks"
    );
    assert_eq!(
        project_config.memory_cards.include[0].scope.as_deref(),
        Some("global")
    );
}

#[test]
fn install_global_memory_card_to_project_copies_record_and_targets() {
    let source = tempfile::tempdir().expect("source");
    let target = tempfile::tempdir().expect("target");
    let home = tempfile::tempdir().expect("home");
    add_memory_card(
            source.path(),
            "project:review-before-sync",
            "同步前先审阅",
            "每次编译到 Claude Code 或 Codex 前，先检查待审草稿和冲突预警，避免把低价值内容写入生成产物。",
            "procedure",
            "project",
            vec!["codex".to_string()],
        )
        .expect("add source memory_card");
    promote_memory_card_to_global(source.path(), home.path(), "project:review-before-sync")
        .expect("promote global");

    let installed = install_global_memory_card_to_project(
        target.path(),
        home.path(),
        "global:review-before-sync",
        vec!["claude-code".to_string()],
    )
    .expect("install global");

    assert_eq!(installed.scope, "global");
    assert!(installed.source_project.is_some());
    let project_records = load_memory_cards(target.path()).expect("target memory_cards");
    assert_eq!(project_records.len(), 1);
    assert_eq!(project_records[0].id, "global:review-before-sync");
    let project = config::load_or_default_project_config(target.path()).expect("target project");
    assert_eq!(project.memory_cards.include[0].targets, vec!["claude-code"]);
    assert_eq!(
        project.memory_cards.include[0].scope.as_deref(),
        Some("global")
    );
}

#[test]
fn merge_memory_cards_creates_combined_memory_card() {
    let temp = tempfile::tempdir().expect("tempdir");
    add_memory_card(
        temp.path(),
        "project:use-axios",
        "Use Axios",
        "Use Axios for frontend HTTP requests.",
        "preference",
        "project",
        vec!["codex".to_string()],
    )
    .expect("add axios");
    add_memory_card(
        temp.path(),
        "project:prefer-bun",
        "Prefer Bun",
        "Use Bun for JavaScript package management and scripts.",
        "preference",
        "project",
        vec!["codex".to_string()],
    )
    .expect("add bun");

    merge_memory_cards(
        temp.path(),
        "project:frontend-defaults",
        "Frontend Defaults",
        vec![
            "project:use-axios".to_string(),
            "project:prefer-bun".to_string(),
        ],
        vec!["claude-code".to_string(), "codex".to_string()],
    )
    .expect("merge");

    let merged = memory_card_map(temp.path())
        .expect("memory_cards")
        .remove("project:frontend-defaults")
        .expect("merged");
    assert!(
        merged
            .body
            .contains("Use Axios for frontend HTTP requests.")
    );
    assert!(
        merged
            .body
            .contains("Use Bun for JavaScript package management and scripts.")
    );

    let project = config::load_or_default_project_config(temp.path()).expect("project");
    let merged_ref = project
        .memory_cards
        .include
        .iter()
        .find(|item| item.id == "project:frontend-defaults")
        .expect("merged ref");
    assert_eq!(merged_ref.targets, vec!["claude-code", "codex"]);
}
