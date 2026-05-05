use super::*;

fn candidate_with_body(title: &str, body: &str, confidence: f32) -> Candidate {
    Candidate {
        title: title.to_string(),
        body: body.to_string(),
        kind: "preference".to_string(),
        scope: "project".to_string(),
        evidence: body.to_string(),
        confidence: Some(confidence),
        reason: None,
        matched_template: None,
    }
}

#[test]
fn dedupe_candidates_merges_near_duplicate_preferences() {
    let candidates = dedupe_candidates(vec![
        candidate_with_body("Use Axios", "Use Axios for frontend HTTP requests.", 0.82),
        candidate_with_body(
            "Prefer Axios",
            "Prefer Axios for frontend API requests.",
            0.78,
        ),
    ]);

    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].title, "Use Axios");
}

#[test]
fn dedupe_candidates_keeps_distinct_tool_preferences() {
    let candidates = dedupe_candidates(vec![
        candidate_with_body("Use Axios", "Use Axios for frontend HTTP requests.", 0.82),
        candidate_with_body(
            "Prefer Bun",
            "Use Bun for JavaScript package management and scripts.",
            0.92,
        ),
    ]);

    assert_eq!(candidates.len(), 2);
}

#[test]
fn extracts_chinese_rule_candidate() {
    let candidates = extract_candidates("以后前端请求统一使用 Axios，不要再用 Fetch。");

    assert_eq!(candidates.len(), 1);
    assert_eq!(draft_id(&candidates[0]), "project:use-axios");
    assert_eq!(candidates[0].title, "Use Axios");
    assert_eq!(candidates[0].body, "Use Axios for frontend HTTP requests.");
}

#[test]
fn extract_command_creates_draft() {
    let temp = tempfile::tempdir().expect("tempdir");

    let report = extract_to_drafts(
        temp.path(),
        Some("Always use Bun for JavaScript package management and scripts.".to_string()),
        None,
        vec!["codex".to_string()],
        Some("local".to_string()),
        false,
    )
    .expect("extract");

    assert_eq!(report.created.len(), 1);
    let drafts = draft::load_drafts(temp.path()).expect("drafts");
    assert_eq!(drafts[0].targets, vec!["codex"]);
}

#[test]
fn extracted_preference_draft_includes_explainability() {
    let temp = tempfile::tempdir().expect("tempdir");

    extract_to_drafts(
        temp.path(),
        Some("以后前端请求统一使用 Axios，不要再用 Fetch。".to_string()),
        None,
        vec!["codex".to_string()],
        Some("local".to_string()),
        false,
    )
    .expect("extract");

    let drafts = draft::load_drafts(temp.path()).expect("drafts");

    assert_eq!(
        drafts[0].matched_template.as_deref(),
        Some("built-in:Use Axios")
    );
    assert_eq!(drafts[0].confidence, Some(0.92));
    assert!(
        drafts[0]
            .reason
            .as_deref()
            .unwrap_or_default()
            .contains("required: axios")
    );
}

#[test]
fn dry_run_previews_preference_explainability() {
    let temp = tempfile::tempdir().expect("tempdir");

    let report = extract_to_drafts(
        temp.path(),
        Some("以后前端请求统一使用 Axios，不要再用 Fetch。".to_string()),
        None,
        vec!["codex".to_string()],
        Some("local".to_string()),
        true,
    )
    .expect("extract");

    assert_eq!(
        report.candidates[0].matched_template.as_deref(),
        Some("built-in:Use Axios")
    );
    assert_eq!(report.candidates[0].confidence, Some(0.92));
    assert!(
        report
            .render()
            .contains("Matched preference template; required: axios")
    );
}

#[test]
fn dry_run_does_not_create_drafts() {
    let temp = tempfile::tempdir().expect("tempdir");

    let report = extract_to_drafts(
        temp.path(),
        Some("Always use Bun for JavaScript package management and scripts.".to_string()),
        None,
        vec!["codex".to_string()],
        Some("local".to_string()),
        true,
    )
    .expect("extract");

    assert_eq!(report.candidates.len(), 1);
    assert!(draft::load_drafts(temp.path()).expect("drafts").is_empty());
}

#[test]
fn dry_run_keeps_governance_constraint_candidates() {
    let temp = tempfile::tempdir().expect("tempdir");

    let report = extract_to_drafts(
        temp.path(),
        Some("不要直接覆盖 AGENTS.md；发现漂移时先导入成 Draft，让我确认后再同步。".to_string()),
        None,
        vec!["codex".to_string()],
        Some("local".to_string()),
        true,
    )
    .expect("extract");

    assert_eq!(report.candidates.len(), 1);
    assert_eq!(report.candidates[0].kind, "constraint");
    assert!(
        report.candidates[0]
            .matched_template
            .as_deref()
            .unwrap_or_default()
            .contains("constraint")
    );
}

#[test]
fn dry_run_keeps_accepted_ai_project_improvements() {
    let temp = tempfile::tempdir().expect("tempdir");

    let report = extract_to_drafts(
            temp.path(),
            Some(
                "基于用户确认，项目应该把 AI 生成的高质量项目改善也送入 Candidate/Draft，而不是直接写 Skilllet；这能保留审阅边界。"
                    .to_string(),
            ),
            None,
            vec!["codex".to_string()],
            Some("local".to_string()),
            true,
        )
        .expect("extract");

    assert_eq!(report.candidates.len(), 1);
    assert_eq!(report.candidates[0].kind, "procedure");
    assert!(
        report.candidates[0]
            .matched_template
            .as_deref()
            .unwrap_or_default()
            .contains("ai_project_improvement")
    );
}

#[test]
fn normalizes_bun_package_manager_preference() {
    let temp = tempfile::tempdir().expect("tempdir");

    let report = extract_to_drafts(
        temp.path(),
        Some("以后把 npm 改为 Bun，所有 JS 脚本都用 bun run。".to_string()),
        None,
        vec!["codex".to_string()],
        Some("local".to_string()),
        true,
    )
    .expect("extract");

    assert_eq!(report.candidates.len(), 1);
    assert_eq!(report.candidates[0].id, "project:prefer-bun");
    assert_eq!(report.candidates[0].title, "Prefer Bun");
    assert_eq!(
        report.candidates[0].body,
        "Use Bun for JavaScript package management and scripts."
    );
}

#[test]
fn normalizes_vitest_unit_test_preference() {
    let temp = tempfile::tempdir().expect("tempdir");

    let report = extract_to_drafts(
        temp.path(),
        Some("以后前端单元测试默认使用 Vitest，不要再写 Jest 配置。".to_string()),
        None,
        vec!["codex".to_string()],
        Some("local".to_string()),
        true,
    )
    .expect("extract");

    assert_eq!(report.candidates.len(), 1);
    assert_eq!(report.candidates[0].id, "project:use-vitest");
    assert_eq!(report.candidates[0].title, "Use Vitest");
    assert_eq!(
        report.candidates[0].body,
        "Use Vitest for frontend unit tests."
    );
}

#[test]
fn uses_project_preference_registry() {
    let temp = tempfile::tempdir().expect("tempdir");
    let registry_dir = temp.path().join(".agent-kernel");
    fs::create_dir_all(&registry_dir).expect("registry dir");
    fs::write(
        registry_dir.join("preference-registry.yml"),
        r#"preferences:
  - title: Use Playwright
    body: Use Playwright for browser automation tests.
    required:
      - playwright
    context:
      - cypress
      - browser automation
      - 浏览器自动化
"#,
    )
    .expect("write registry");

    let report = extract_to_drafts(
        temp.path(),
        Some("以后浏览器自动化测试统一使用 Playwright，不要再用 Cypress。".to_string()),
        None,
        vec!["codex".to_string()],
        Some("local".to_string()),
        true,
    )
    .expect("extract");

    assert_eq!(report.candidates.len(), 1);
    assert_eq!(report.candidates[0].id, "project:use-playwright");
    assert_eq!(report.candidates[0].title, "Use Playwright");
    assert_eq!(
        report.candidates[0].body,
        "Use Playwright for browser automation tests."
    );
}

#[test]
fn project_preference_registry_overrides_built_ins() {
    let temp = tempfile::tempdir().expect("tempdir");
    let registry_dir = temp.path().join(".agent-kernel");
    fs::create_dir_all(&registry_dir).expect("registry dir");
    fs::write(
        registry_dir.join("preference-registry.yml"),
        r#"preferences:
  - title: Use Bun Runtime
    body: Use Bun for package management, scripts, and JavaScript runtime tasks.
    required:
      - bun
    context:
      - npm
      - package management
"#,
    )
    .expect("write registry");

    let report = extract_to_drafts(
        temp.path(),
        Some("Always use Bun instead of npm for package management.".to_string()),
        None,
        vec!["codex".to_string()],
        Some("local".to_string()),
        true,
    )
    .expect("extract");

    assert_eq!(report.candidates.len(), 1);
    assert_eq!(report.candidates[0].id, "project:use-bun-runtime");
    assert_eq!(report.candidates[0].title, "Use Bun Runtime");
    assert_eq!(
        report.candidates[0].body,
        "Use Bun for package management, scripts, and JavaScript runtime tasks."
    );
}

#[test]
fn preference_templates_list_project_before_built_ins() {
    let temp = tempfile::tempdir().expect("tempdir");
    let registry_dir = temp.path().join(".agent-kernel");
    fs::create_dir_all(&registry_dir).expect("registry dir");
    fs::write(
        registry_dir.join("preference-registry.yml"),
        r#"preferences:
  - title: Use Playwright
    body: Use Playwright for browser automation tests.
    required:
      - playwright
    context:
      - cypress
"#,
    )
    .expect("write registry");

    let templates = preference_templates(temp.path()).expect("templates");

    assert_eq!(templates[0].source, "project");
    assert_eq!(templates[0].title, "Use Playwright");
    assert!(
        templates
            .iter()
            .any(|template| template.source == "built-in")
    );
}

#[test]
fn init_preference_registry_writes_example() {
    let temp = tempfile::tempdir().expect("tempdir");

    let created = init_preference_registry(temp.path()).expect("init registry");
    let path = config::kernel_dir(temp.path()).join("preference-registry.yml");

    assert!(created);
    assert!(path.exists());
    let text = fs::read_to_string(path).expect("registry");
    assert!(text.contains("Use Playwright"));
    assert!(text.contains("preferences:"));
}

#[test]
fn validate_preference_registry_reports_errors_and_warnings() {
    let temp = tempfile::tempdir().expect("tempdir");
    let registry_dir = temp.path().join(".agent-kernel");
    fs::create_dir_all(&registry_dir).expect("registry dir");
    fs::write(
        registry_dir.join("preference-registry.yml"),
        r#"preferences:
  - title: Use Playwright
    body: Use Playwright for browser automation tests.
    required: []
    context: []
  - title: Use Playwright
    body: Duplicate title.
    required:
      - playwright
    context:
      - cypress
  - title: ""
    body: Missing title.
    required:
      - vitest
"#,
    )
    .expect("write registry");

    let report = validate_preference_registry(temp.path()).expect("validate");

    assert_eq!(report.errors, 3);
    assert_eq!(report.warnings, 2);
    assert!(
        report
            .messages
            .iter()
            .any(|message| message.contains("duplicate title"))
    );
}

#[test]
fn test_preference_text_reports_match_reason() {
    let temp = tempfile::tempdir().expect("tempdir");
    let registry_dir = temp.path().join(".agent-kernel");
    fs::create_dir_all(&registry_dir).expect("registry dir");
    fs::write(
        registry_dir.join("preference-registry.yml"),
        r#"preferences:
  - title: Use Playwright
    body: Use Playwright for browser automation tests.
    required:
      - playwright
    context:
      - cypress
"#,
    )
    .expect("write registry");

    let report = test_preference_text(
        temp.path(),
        "以后浏览器自动化测试统一使用 Playwright，不要再用 Cypress。",
    )
    .expect("test preference");

    assert_eq!(report.matches.len(), 1);
    assert_eq!(report.matches[0].draft_id, "project:use-playwright");
    assert_eq!(report.matches[0].source, "project");
    assert_eq!(report.matches[0].required, vec!["playwright"]);
    assert_eq!(report.matches[0].context, vec!["cypress"]);
}

#[test]
fn extraction_redacts_secret_evidence() {
    let temp = tempfile::tempdir().expect("tempdir");

    extract_to_drafts(
        temp.path(),
        Some(
            "Always use Bun for JavaScript package management; token=supersecret123456789."
                .to_string(),
        ),
        None,
        vec!["codex".to_string()],
        Some("local".to_string()),
        false,
    )
    .expect("extract");

    let drafts = draft::load_drafts(temp.path()).expect("drafts");
    assert!(drafts[0].evidence.contains("[REDACTED]"));
    assert!(!drafts[0].evidence.contains("supersecret123456789"));
}

#[test]
fn high_value_extraction_filters_task_chatter() {
    let temp = tempfile::tempdir().expect("tempdir");
    let report = extract_high_value_text_to_drafts(
            temp.path(),
            "继续优化 UI。\n现在帮我修复按钮。\n以后所有前端请求统一使用 Axios，不要再用 Fetch。\nAlways run cargo clippy before pushing Rust changes.",
            vec!["codex".to_string(), "claude-code".to_string()],
            "test",
            Some("local".to_string()),
            true,
            8,
        )
        .expect("extract");

    assert_eq!(report.candidates.len(), 2);
    assert!(
        report
            .candidates
            .iter()
            .any(|item| item.id == "project:use-axios")
    );
    assert!(
        report
            .candidates
            .iter()
            .all(|item| !item.body.contains("继续优化"))
    );
}

#[test]
fn high_value_extraction_rejects_ui_bug_report_and_one_off_planning_requests() {
    let temp = tempfile::tempdir().expect("tempdir");
    let report = extract_high_value_text_to_drafts(
            temp.path(),
            "现在的草稿质量太低了，请修改对应规则。\n读取本地会话现在卡在了72%，告诉我为什么。\n我需要知道他后台在干什么，可以放一个小窗口展示在干什么。\n草稿的中文简介部分应该是介绍这个草稿的内容是什么，而不是现在这样。",
            vec!["codex".to_string()],
            "test",
            Some("local".to_string()),
            true,
            8,
        )
        .expect("extract");

    assert!(
        report.candidates.is_empty(),
        "one-off UI feedback and bug reports should not become Skilllet drafts: {:?}",
        report.candidates
    );
}

#[test]
fn high_value_extraction_rejects_unresolved_feature_requests_with_outcome_words() {
    let temp = tempfile::tempdir().expect("tempdir");
    let report = extract_high_value_text_to_drafts(
            temp.path(),
            "我希望 UI 界面加一个小窗口展示后台引擎在干什么，避免用户不知道进度。\n应该读取 Claude 和 Codex 的所有项目，并默认使用 Claude Code 整理。",
            vec!["codex".to_string()],
            "test",
            Some("local".to_string()),
            true,
            8,
        )
        .expect("extract");

    assert!(
        report.candidates.is_empty(),
        "unresolved product requests should not be stored as durable Skilllets: {:?}",
        report.candidates
    );
}

#[test]
fn high_value_extraction_keeps_only_reusable_project_improvement_lessons() {
    let temp = tempfile::tempdir().expect("tempdir");
    let report = extract_high_value_text_to_drafts(
            temp.path(),
            "这次把后台整理拆成发现文件、导入增量、整理材料、调用引擎、解析候选、写入草稿等阶段，最终做法是让任务状态暴露阶段日志，避免长任务只显示一个假进度。",
            vec!["codex".to_string()],
            "test",
            Some("local".to_string()),
            true,
            8,
        )
        .expect("extract");

    assert_eq!(report.candidates.len(), 1);
    assert!(report.candidates[0].body.contains("阶段"));
    assert!(report.candidates[0].body.contains("长任务"));
}

#[test]
fn high_value_extraction_limits_candidate_count() {
    let temp = tempfile::tempdir().expect("tempdir");
    let report = extract_high_value_text_to_drafts(
            temp.path(),
            "以后所有 Rust 项目必须运行 cargo test。\n以后所有 React 项目默认使用 TypeScript。\n以后所有 UI 改动必须检查中文显示。",
            vec!["codex".to_string()],
            "test",
            Some("local".to_string()),
            true,
            2,
        )
        .expect("extract");

    assert_eq!(report.candidates.len(), 2);
}

#[test]
fn high_value_extraction_keeps_project_improvement_records() {
    let temp = tempfile::tempdir().expect("tempdir");
    let text = "这次修复了 Tauri UI 点击后卡死的问题，根因是按钮动作在前端串行等待长时间 Rust 命令。最终做法是把扫描和整理放到后台任务，前端只刷新状态，界面保持可交互。";
    assert!(looks_like_project_improvement_signal(
        "这次修复了 Tauri UI 点击后卡死的问题，根因是按钮动作在前端串行等待长时间 Rust 命令"
    ));
    let improvement_body = normalize_project_improvement_body(
        "这次修复了 Tauri UI 点击后卡死的问题，根因是按钮动作在前端串行等待长时间 Rust 命令",
    );
    assert!(improvement_body.len() >= 28 && improvement_body.len() <= 360);
    let local_candidates =
        extract_high_value_candidates_with_preferences(text, &built_in_preferences(), 8);
    assert!(!local_candidates.is_empty());
    let report = extract_high_value_text_to_drafts(
        temp.path(),
        text,
        vec!["codex".to_string(), "claude-code".to_string()],
        "test",
        Some("local".to_string()),
        true,
        8,
    )
    .expect("extract");

    assert!(!report.candidates.is_empty());
    assert!(
        report
            .candidates
            .iter()
            .all(|candidate| candidate.kind == "procedure")
    );
    assert!(
        report
            .candidates
            .iter()
            .any(|candidate| candidate.body.contains("后台任务"))
    );
    assert!(report.candidates.iter().any(|candidate| {
        candidate
            .reason
            .as_deref()
            .unwrap_or_default()
            .contains("project improvement")
    }));
}

#[test]
fn high_value_extraction_keeps_prompt_patterns_without_rule_words() {
    let temp = tempfile::tempdir().expect("tempdir");
    let report = extract_high_value_text_to_drafts(
            temp.path(),
            "处理大型 UI 重构时，可以先让 Claude Code 输出组件边界、状态流和可交互按钮清单，再由 Codex 做集成测试和 Rust 后端检查，这个提示能显著减少返工。",
            vec!["codex".to_string(), "claude-code".to_string()],
            "test",
            Some("local".to_string()),
            true,
            8,
        )
        .expect("extract");

    assert!(!report.candidates.is_empty());
    assert!(
        report
            .candidates
            .iter()
            .any(|candidate| candidate.matched_template.as_deref() == Some("high-value-prompt"))
    );
}
