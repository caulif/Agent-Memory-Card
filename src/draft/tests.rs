use super::*;

#[test]
fn approves_draft_into_skilllet() {
    let temp = tempfile::tempdir().expect("tempdir");
    add_draft(
        temp.path(),
        NewDraft {
            id: "project:prefer-bun".to_string(),
            title: "Prefer Bun".to_string(),
            body: "Use Bun for JavaScript package management and scripts.".to_string(),
            kind: "preference".to_string(),
            scope: "project".to_string(),
            targets: vec!["codex".to_string()],
            evidence: "manual test".to_string(),
            confidence: None,
            reason: None,
            matched_template: None,
            extraction: ExtractionMetadata::default(),
        },
    )
    .expect("add draft");

    approve_draft(temp.path(), "project:prefer-bun").expect("approve");

    assert!(load_drafts(temp.path()).expect("drafts").is_empty());
    let skilllets = skilllet::load_skilllets(temp.path()).expect("skilllets");
    assert_eq!(skilllets[0].id, "project:prefer-bun");
}

#[test]
fn approve_draft_rejects_existing_skilllet_id_conflict() {
    let temp = tempfile::tempdir().expect("tempdir");
    skilllet::add_skilllet(
        temp.path(),
        "project:prefer-bun",
        "Prefer Bun",
        "Use Bun for JavaScript package management.",
        "preference",
        "project",
        vec!["codex".to_string()],
    )
    .expect("skilllet");
    add_draft(
        temp.path(),
        NewDraft {
            id: "project:prefer-bun".to_string(),
            title: "Prefer Bun Updated".to_string(),
            body: "Use Bun for all JavaScript scripts.".to_string(),
            kind: "preference".to_string(),
            scope: "project".to_string(),
            targets: vec!["codex".to_string()],
            evidence: "desktop test".to_string(),
            confidence: Some(0.9),
            reason: Some("Repeated correction".to_string()),
            matched_template: None,
            extraction: ExtractionMetadata::default(),
        },
    )
    .expect("draft");

    let err = approve_draft(temp.path(), "project:prefer-bun")
        .expect_err("approval should require conflict review");

    assert!(err.to_string().contains("conflict"));
    assert_eq!(load_drafts(temp.path()).expect("drafts").len(), 1);
    let skilllets = skilllet::load_skilllets(temp.path()).expect("skilllets");
    assert_eq!(
        skilllets[0].body,
        "Use Bun for JavaScript package management."
    );
}

#[test]
fn update_draft_rejects_empty_body_unknown_kind_and_unknown_target() {
    let temp = tempfile::tempdir().expect("tempdir");
    add_draft(
        temp.path(),
        NewDraft {
            id: "project:editable".to_string(),
            title: "Editable".to_string(),
            body: "Use Vitest for frontend tests.".to_string(),
            kind: "preference".to_string(),
            scope: "project".to_string(),
            targets: vec!["codex".to_string()],
            evidence: "desktop test".to_string(),
            confidence: Some(0.9),
            reason: Some("Repeated correction".to_string()),
            matched_template: None,
            extraction: ExtractionMetadata::default(),
        },
    )
    .expect("draft");

    let empty_body = update_draft(
        temp.path(),
        "project:editable",
        DraftUpdate {
            body: Some("   ".to_string()),
            ..DraftUpdate::default()
        },
    )
    .expect_err("empty body should fail");
    assert!(empty_body.to_string().contains("body"));

    let unknown_kind = update_draft(
        temp.path(),
        "project:editable",
        DraftUpdate {
            kind: Some("mystery".to_string()),
            ..DraftUpdate::default()
        },
    )
    .expect_err("unknown kind should fail");
    assert!(unknown_kind.to_string().contains("kind"));

    let unknown_target = update_draft(
        temp.path(),
        "project:editable",
        DraftUpdate {
            targets: Some(vec!["cursor".to_string()]),
            ..DraftUpdate::default()
        },
    )
    .expect_err("unknown target should fail");
    assert!(unknown_target.to_string().contains("target agent"));
}

#[test]
fn draft_explainability_fields_roundtrip() {
    let temp = tempfile::tempdir().expect("tempdir");
    add_draft(
        temp.path(),
        NewDraft {
            id: "project:prefer-bun".to_string(),
            title: "Prefer Bun".to_string(),
            body: "Use Bun for JavaScript package management and scripts.".to_string(),
            kind: "preference".to_string(),
            scope: "project".to_string(),
            targets: vec!["codex".to_string()],
            evidence: "manual test".to_string(),
            confidence: Some(0.92),
            reason: Some("Matched project preference template".to_string()),
            matched_template: Some("project:prefer-bun".to_string()),
            extraction: ExtractionMetadata::default(),
        },
    )
    .expect("add draft");

    let drafts = load_drafts(temp.path()).expect("drafts");

    assert_eq!(drafts[0].confidence, Some(0.92));
    assert_eq!(
        drafts[0].reason.as_deref(),
        Some("Matched project preference template")
    );
    assert_eq!(
        drafts[0].matched_template.as_deref(),
        Some("project:prefer-bun")
    );
}

#[test]
fn draft_records_include_plain_brief_language_and_tags() {
    let temp = tempfile::tempdir().expect("tempdir");
    add_draft(
            temp.path(),
            NewDraft {
                id: "project:ui-background-tasks".to_string(),
                title: "UI 后台任务".to_string(),
                body: "处理 Tauri UI 的扫描、整理和编译时，把长任务放到后台，前端只刷新状态和进度，避免点击后卡死。"
                    .to_string(),
                kind: "procedure".to_string(),
                scope: "project".to_string(),
                targets: vec!["codex".to_string()],
                evidence: "manual test".to_string(),
                confidence: Some(0.9),
                reason: None,
                matched_template: None,
                extraction: ExtractionMetadata::default(),
            },
        )
        .expect("add draft");

    let drafts = load_drafts(temp.path()).expect("drafts");

    assert_eq!(drafts[0].language, "zh");
    assert!(drafts[0].brief.contains("这条草稿记录了"));
    assert!(!drafts[0].brief.contains("以后遇到类似场景"));
    assert!(drafts[0].brief.chars().count() < drafts[0].body.chars().count() + 40);
    assert!(drafts[0].tags.contains(&"ui-design".to_string()));
    assert!(drafts[0].tags.contains(&"tauri".to_string()));
    assert!(drafts[0].tags.contains(&"performance".to_string()));
}

#[test]
fn updates_draft_reviewable_fields_without_losing_explainability() {
    let temp = tempfile::tempdir().expect("tempdir");
    add_draft(
        temp.path(),
        NewDraft {
            id: "project:prefer-bun".to_string(),
            title: "Prefer Bun".to_string(),
            body: "Use Bun for JavaScript package management and scripts.".to_string(),
            kind: "preference".to_string(),
            scope: "project".to_string(),
            targets: vec!["codex".to_string()],
            evidence: "manual test".to_string(),
            confidence: Some(0.92),
            reason: Some("Matched project preference template".to_string()),
            matched_template: Some("built-in:Prefer Bun".to_string()),
            extraction: ExtractionMetadata::default(),
        },
    )
    .expect("add draft");

    let updated = update_draft(
        temp.path(),
        "project:prefer-bun",
        DraftUpdate {
            title: Some("Prefer Bun Runtime".to_string()),
            body: Some(
                "Use Bun for package management, scripts, and JS runtime tasks.".to_string(),
            ),
            targets: Some(vec!["claude-code".to_string(), "codex".to_string()]),
            ..Default::default()
        },
    )
    .expect("update draft");

    assert_eq!(updated.title, "Prefer Bun Runtime");
    assert_eq!(
        updated.body,
        "Use Bun for package management, scripts, and JS runtime tasks."
    );
    assert_eq!(updated.targets, vec!["claude-code", "codex"]);
    assert_eq!(updated.confidence, Some(0.92));
    assert_eq!(
        updated.matched_template.as_deref(),
        Some("built-in:Prefer Bun")
    );

    let drafts = load_drafts(temp.path()).expect("drafts");
    assert_eq!(drafts[0].title, "Prefer Bun Runtime");
}

#[test]
fn updates_draft_editor_fields_and_allows_empty_targets() {
    let temp = tempfile::tempdir().expect("tempdir");
    add_draft(
        temp.path(),
        NewDraft {
            id: "project:prefer-bun".to_string(),
            title: "Prefer Bun".to_string(),
            body: "Use Bun for JavaScript package management and scripts.".to_string(),
            kind: "preference".to_string(),
            scope: "project".to_string(),
            targets: vec!["codex".to_string()],
            evidence: "manual test".to_string(),
            confidence: Some(0.92),
            reason: Some("Matched project preference template".to_string()),
            matched_template: Some("built-in:Prefer Bun".to_string()),
            extraction: ExtractionMetadata::default(),
        },
    )
    .expect("add draft");

    let updated = update_draft(
        temp.path(),
        "project:prefer-bun",
        DraftUpdate {
            brief: Some("以后 JS 项目默认用 Bun 管理依赖和脚本。".to_string()),
            tags: Some(vec![
                "workflow".to_string(),
                "frontend".to_string(),
                "frontend".to_string(),
            ]),
            language: Some("zh".to_string()),
            targets: Some(Vec::new()),
            ..Default::default()
        },
    )
    .expect("update draft editor fields");

    assert_eq!(updated.brief, "以后 JS 项目默认用 Bun 管理依赖和脚本。");
    assert_eq!(updated.tags, vec!["frontend", "workflow"]);
    assert_eq!(updated.language, "zh");
    assert!(updated.targets.is_empty());
}

#[test]
fn rejects_draft_ids_that_escape_kernel_directory() {
    let temp = tempfile::tempdir().expect("tempdir");

    let err = add_draft(
        temp.path(),
        NewDraft {
            id: "../escape".to_string(),
            title: "Escape".to_string(),
            body: "This must not write outside the drafts directory.".to_string(),
            kind: "preference".to_string(),
            scope: "project".to_string(),
            targets: vec!["codex".to_string()],
            evidence: "manual test".to_string(),
            confidence: None,
            reason: None,
            matched_template: None,
            extraction: ExtractionMetadata::default(),
        },
    )
    .expect_err("path traversal id should fail");

    assert!(err.to_string().contains("invalid draft id"));
    assert!(!config::kernel_dir(temp.path()).join("escape.yml").exists());
}

#[test]
fn merges_drafts_into_new_reviewable_draft_without_removing_sources() {
    let temp = tempfile::tempdir().expect("tempdir");
    add_draft(
        temp.path(),
        NewDraft {
            id: "project:use-axios".to_string(),
            title: "Use Axios".to_string(),
            body: "Use Axios for frontend HTTP requests.".to_string(),
            kind: "preference".to_string(),
            scope: "project".to_string(),
            targets: vec!["codex".to_string()],
            evidence: "observation:a".to_string(),
            confidence: Some(0.92),
            reason: Some("Matched HTTP client preference".to_string()),
            matched_template: Some("built-in:Use Axios".to_string()),
            extraction: ExtractionMetadata::default(),
        },
    )
    .expect("add axios draft");
    add_draft(
        temp.path(),
        NewDraft {
            id: "project:prefer-bun".to_string(),
            title: "Prefer Bun".to_string(),
            body: "Use Bun for package management and scripts.".to_string(),
            kind: "preference".to_string(),
            scope: "project".to_string(),
            targets: vec!["claude-code".to_string()],
            evidence: "observation:b".to_string(),
            confidence: Some(0.84),
            reason: Some("Matched package manager preference".to_string()),
            matched_template: Some("built-in:Prefer Bun".to_string()),
            extraction: ExtractionMetadata::default(),
        },
    )
    .expect("add bun draft");

    let merged = merge_drafts(
        temp.path(),
        "project:frontend-defaults",
        "Frontend Defaults",
        vec![
            "project:use-axios".to_string(),
            "project:prefer-bun".to_string(),
        ],
        vec!["codex".to_string(), "claude-code".to_string()],
    )
    .expect("merge drafts");

    assert_eq!(merged.id, "project:frontend-defaults");
    assert_eq!(merged.title, "Frontend Defaults");
    assert_eq!(merged.kind, "procedure");
    assert_eq!(merged.scope, "project");
    assert_eq!(merged.targets, vec!["claude-code", "codex"]);
    assert!(merged.body.contains("## Use Axios"));
    assert!(merged.body.contains("Use Bun for package management"));
    assert!(merged.evidence.contains("Merged Drafts"));
    assert!(merged.evidence.contains("project:use-axios"));
    assert_eq!(merged.confidence, Some(0.84));
    assert!(
        merged
            .reason
            .as_deref()
            .unwrap()
            .contains("Merged 2 drafts")
    );

    let drafts = load_drafts(temp.path()).expect("drafts");
    // 合并后源草稿被删除，只保留合并结果
    assert_eq!(drafts.len(), 1);
    assert!(!drafts.iter().any(|draft| draft.id == "project:use-axios"));
    assert!(!drafts.iter().any(|draft| draft.id == "project:prefer-bun"));
    assert!(
        drafts
            .iter()
            .any(|draft| draft.id == "project:frontend-defaults")
    );
}

#[test]
fn merge_requires_at_least_two_source_drafts() {
    let temp = tempfile::tempdir().expect("tempdir");
    add_draft(
        temp.path(),
        NewDraft {
            id: "project:solo".to_string(),
            title: "Solo".to_string(),
            body: "One draft is not a merge.".to_string(),
            kind: "preference".to_string(),
            scope: "project".to_string(),
            targets: vec!["codex".to_string()],
            evidence: "manual test".to_string(),
            confidence: None,
            reason: None,
            matched_template: None,
            extraction: ExtractionMetadata::default(),
        },
    )
    .expect("add solo");

    let err = merge_drafts(
        temp.path(),
        "project:solo-merged",
        "Solo Merged",
        vec!["project:solo".to_string()],
        vec!["codex".to_string()],
    )
    .expect_err("single source should fail");

    assert!(err.to_string().contains("at least two source drafts"));
    assert_eq!(load_drafts(temp.path()).expect("drafts").len(), 1);
}

#[test]
fn fuses_skilllets_into_reviewable_draft() {
    let temp = tempfile::tempdir().expect("tempdir");
    skilllet::add_skilllet(
        temp.path(),
        "project:ui-background-tasks",
        "UI Background Tasks",
        "Run long UI scans in background tasks and refresh progress separately.",
        "procedure",
        "project",
        vec!["codex".to_string()],
    )
    .expect("add ui");
    skilllet::add_skilllet(
        temp.path(),
        "project:tauri-responsive",
        "Tauri Responsive Shell",
        "Keep Tauri commands short from the frontend and report status incrementally.",
        "procedure",
        "project",
        vec!["claude-code".to_string()],
    )
    .expect("add tauri");

    let draft = fuse_skilllets_to_draft(
        temp.path(),
        "project:responsive-desktop-workflow",
        "Responsive Desktop Workflow",
        vec![
            "project:ui-background-tasks".to_string(),
            "project:tauri-responsive".to_string(),
        ],
        vec!["codex".to_string(), "claude-code".to_string()],
    )
    .expect("fuse");

    assert_eq!(draft.kind, "procedure");
    assert_eq!(
        draft.matched_template.as_deref(),
        Some("manual:skilllet-fusion")
    );
    assert!(draft.evidence.contains("Fused Skilllets"));
    assert!(draft.body.contains("UI Background Tasks"));
    assert!(draft.body.contains("Tauri Responsive Shell"));
    assert!(draft.brief.contains("Responsive Desktop Workflow"));
    assert!(draft.tags.contains(&"tauri".to_string()));
}
