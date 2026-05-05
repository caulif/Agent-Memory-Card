use super::*;

#[test]
fn skilllet_target_matrix_marks_assigned_agents() {
    let temp = tempfile::tempdir().expect("tempdir");
    add_skilllet(
        temp.path(),
        "project:use-axios",
        "Use Axios",
        "Use Axios for frontend HTTP requests.",
        "preference",
        "project",
        vec!["codex".to_string(), "claude-code".to_string()],
    )
    .expect("add skilllet");

    let matrix = skilllet_target_matrix(temp.path()).expect("matrix");

    assert_eq!(matrix.rows[0].skilllet_id, "project:use-axios");
    assert_eq!(matrix.rows[0].targets.get("codex"), Some(&true));
    assert_eq!(matrix.rows[0].targets.get("claude-code"), Some(&true));
}

#[test]
fn new_procedure_skilllet_defaults_to_skill_activation() {
    let temp = tempfile::tempdir().expect("tempdir");
    add_skilllet(
        temp.path(),
        "project:frontend-workflow",
        "Frontend Workflow",
        "Use Axios for frontend HTTP requests.",
        "procedure",
        "project",
        vec!["claude-code".to_string()],
    )
    .expect("add skilllet");

    let records = load_skilllets(temp.path()).expect("records");

    assert_eq!(records[0].activation, "skill");
}

#[test]
fn add_skilllet_writes_record_and_project_include() {
    let temp = tempfile::tempdir().expect("tempdir");

    add_skilllet(
        temp.path(),
        "project:use-axios",
        "Use Axios",
        "Use Axios for frontend HTTP requests.",
        "preference",
        "project",
        vec!["codex".to_string()],
    )
    .expect("add skilllet");

    let records = load_skilllets(temp.path()).expect("load skilllets");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].id, "project:use-axios");
    assert!(records[0].brief.contains("Use Axios"));
    assert!(records[0].tags.contains(&"frontend".to_string()));
    assert_eq!(records[0].language, "en");

    let project = config::load_or_default_project_config(temp.path()).expect("load project");
    assert_eq!(project.skilllets.include[0].id, "project:use-axios");
    assert_eq!(project.skilllets.include[0].targets, vec!["codex"]);
}

#[test]
fn set_skilllet_targets_updates_project_include() {
    let temp = tempfile::tempdir().expect("tempdir");
    add_skilllet(
        temp.path(),
        "project:use-axios",
        "Use Axios",
        "Use Axios for frontend HTTP requests.",
        "preference",
        "project",
        vec!["codex".to_string()],
    )
    .expect("add skilllet");

    set_skilllet_targets(
        temp.path(),
        "project:use-axios",
        vec!["claude-code".to_string(), "codex".to_string()],
    )
    .expect("set targets");

    let project = config::load_or_default_project_config(temp.path()).expect("project");
    assert_eq!(
        project.skilllets.include[0].targets,
        vec!["claude-code", "codex"]
    );
}

#[test]
fn set_skilllet_targets_allows_empty_inactive_assignment() {
    let temp = tempfile::tempdir().expect("tempdir");
    add_skilllet(
        temp.path(),
        "project:use-axios",
        "Use Axios",
        "Use Axios for frontend HTTP requests.",
        "preference",
        "project",
        vec!["codex".to_string()],
    )
    .expect("add skilllet");

    set_skilllet_targets(temp.path(), "project:use-axios", Vec::new()).expect("clear targets");

    let project = config::load_or_default_project_config(temp.path()).expect("project");
    assert!(project.skilllets.include[0].targets.is_empty());
    let matrix = skilllet_target_matrix(temp.path()).expect("matrix");
    assert_eq!(matrix.rows[0].targets.get("codex"), Some(&false));
    assert_eq!(matrix.rows[0].targets.get("claude-code"), Some(&false));
}

#[test]
fn update_skilllet_updates_body_tags_and_brief_without_changing_created_at() {
    let temp = tempfile::tempdir().expect("tempdir");
    add_skilllet(
        temp.path(),
        "project:editable",
        "Editable",
        "Original body.",
        "preference",
        "project",
        vec!["codex".to_string()],
    )
    .expect("add skilllet");
    let original = skilllet_map(temp.path())
        .expect("skilllets")
        .remove("project:editable")
        .expect("original");

    let updated = update_skilllet(
        temp.path(),
        "project:editable",
        SkillletUpdate {
            body: Some("Updated body.".to_string()),
            brief: Some("Short editor summary.".to_string()),
            tags: Some(vec![
                "testing".to_string(),
                "frontend".to_string(),
                "testing".to_string(),
            ]),
            ..SkillletUpdate::default()
        },
    )
    .expect("update skilllet");

    assert_eq!(updated.body, "Updated body.");
    assert_eq!(updated.brief, "Short editor summary.");
    assert_eq!(updated.tags, vec!["frontend", "testing"]);
    assert_eq!(updated.created_at, original.created_at);
    assert_ne!(updated.updated_at, original.updated_at);

    let persisted = skilllet_map(temp.path())
        .expect("skilllets")
        .remove("project:editable")
        .expect("updated");
    assert_eq!(persisted.body, "Updated body.");
    assert_eq!(persisted.tags, vec!["frontend", "testing"]);
}

#[test]
fn skilllet_editing_rejects_invalid_fields_and_targets() {
    let temp = tempfile::tempdir().expect("tempdir");
    let empty_title = add_skilllet(
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

    let bad_target = add_skilllet(
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

    add_skilllet(
        temp.path(),
        "project:editable",
        "Editable",
        "Original body.",
        "preference",
        "project",
        vec!["codex".to_string()],
    )
    .expect("add skilllet");
    let bad_kind = update_skilllet(
        temp.path(),
        "project:editable",
        SkillletUpdate {
            kind: Some("mystery".to_string()),
            ..SkillletUpdate::default()
        },
    )
    .expect_err("unknown kind should fail");
    assert!(bad_kind.to_string().contains("kind"));

    let bad_assignment =
        set_skilllet_targets(temp.path(), "project:editable", vec!["cursor".to_string()])
            .expect_err("unknown assignment target should fail");
    assert!(bad_assignment.to_string().contains("target agent"));
}

#[test]
fn skilllet_write_paths_reject_malicious_ids() {
    let temp = tempfile::tempdir().expect("tempdir");
    let home = tempfile::tempdir().expect("home");
    add_skilllet(
        temp.path(),
        "project:safe",
        "Safe",
        "Safe body.",
        "preference",
        "project",
        vec!["codex".to_string()],
    )
    .expect("add safe skilllet");

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
            add_skilllet(
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
            update_skilllet(
                temp.path(),
                id,
                SkillletUpdate {
                    body: Some("Bad body.".to_string()),
                    ..SkillletUpdate::default()
                },
            )
            .is_err()
        );
        assert!(promote_skilllet_to_global(temp.path(), home.path(), id).is_err());
        assert!(
            install_global_skilllet_to_project(temp.path(), home.path(), id, Vec::new()).is_err()
        );
        assert!(
            merge_skilllets(
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
fn promote_skilllet_to_global_copies_record_with_source_project() {
    let project = tempfile::tempdir().expect("project");
    let home = tempfile::tempdir().expect("home");
    add_skilllet(
            project.path(),
            "project:ui-background-tasks",
            "UI 后台任务",
            "处理 Tauri UI 的扫描、整理和编译时，把长任务放到后台，前端只刷新状态和进度，避免点击后卡死。",
            "procedure",
            "project",
            vec!["codex".to_string()],
        )
        .expect("add skilllet");

    let promoted =
        promote_skilllet_to_global(project.path(), home.path(), "project:ui-background-tasks")
            .expect("promote");

    assert_eq!(promoted.scope, "global");
    let source_project = fsutil::path_to_slash(project.path());
    assert_eq!(
        promoted.source_project.as_deref(),
        Some(source_project.as_str())
    );
    let global = load_global_skilllets(home.path()).expect("global skilllets");
    assert_eq!(global.len(), 1);
    assert_eq!(global[0].id, "project:ui-background-tasks");
    assert!(global[0].tags.contains(&"ui-design".to_string()));
}

#[test]
fn install_global_skilllet_to_project_copies_record_and_targets() {
    let source = tempfile::tempdir().expect("source");
    let target = tempfile::tempdir().expect("target");
    let home = tempfile::tempdir().expect("home");
    add_skilllet(
            source.path(),
            "project:review-before-sync",
            "同步前先审阅",
            "每次编译到 Claude Code 或 Codex 前，先检查待审草稿和冲突预警，避免把低价值内容写入生成产物。",
            "procedure",
            "project",
            vec!["codex".to_string()],
        )
        .expect("add source skilllet");
    promote_skilllet_to_global(source.path(), home.path(), "project:review-before-sync")
        .expect("promote global");

    let installed = install_global_skilllet_to_project(
        target.path(),
        home.path(),
        "project:review-before-sync",
        vec!["claude-code".to_string()],
    )
    .expect("install global");

    assert_eq!(installed.scope, "project");
    assert!(installed.source_project.is_some());
    let project_records = load_skilllets(target.path()).expect("target skilllets");
    assert_eq!(project_records.len(), 1);
    assert_eq!(project_records[0].id, "project:review-before-sync");
    let project = config::load_or_default_project_config(target.path()).expect("target project");
    assert_eq!(project.skilllets.include[0].targets, vec!["claude-code"]);
}

#[test]
fn merge_skilllets_creates_combined_skilllet() {
    let temp = tempfile::tempdir().expect("tempdir");
    add_skilllet(
        temp.path(),
        "project:use-axios",
        "Use Axios",
        "Use Axios for frontend HTTP requests.",
        "preference",
        "project",
        vec!["codex".to_string()],
    )
    .expect("add axios");
    add_skilllet(
        temp.path(),
        "project:prefer-bun",
        "Prefer Bun",
        "Use Bun for JavaScript package management and scripts.",
        "preference",
        "project",
        vec!["codex".to_string()],
    )
    .expect("add bun");

    merge_skilllets(
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

    let merged = skilllet_map(temp.path())
        .expect("skilllets")
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
        .skilllets
        .include
        .iter()
        .find(|item| item.id == "project:frontend-defaults")
        .expect("merged ref");
    assert_eq!(merged_ref.targets, vec!["claude-code", "codex"]);
}
