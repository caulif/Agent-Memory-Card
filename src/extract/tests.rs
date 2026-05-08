use super::*;
use crate::config;

fn candidate_with_body(title: &str, body: &str, confidence: f32) -> Candidate {
    Candidate {
        title: title.to_string(),
        body: body.to_string(),
        kind: "preference".to_string(),
        scope: "project".to_string(),
        memory_tier: crate::candidate::MemoryTier::ProjectRule,
        abstraction_of: None,
        abstracted_from: None,
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
fn package_manager_preferences_are_not_extracted_as_memory_cards() {
    let temp = tempfile::tempdir().expect("tempdir");
    let report = extract_high_value_text_to_drafts(
        temp.path(),
        "以后 JavaScript 包管理统一使用 Bun，不要用 npm 或 pnpm。",
        vec!["codex".to_string()],
        "test",
        Some("local".to_string()),
        true,
        8,
    )
    .expect("extract");

    assert!(
        report.candidates.iter().all(|candidate| {
            let text =
                format!("{}\n{}\n{}", candidate.title, candidate.body, candidate.id).to_lowercase();
            !text.contains("bun") && !text.contains("package") && !text.contains("包管理")
        }),
        "package manager preferences should not become MemoryCard candidates: {:?}",
        report.candidates
    );
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
fn principle_candidate_preserves_user_wording_instead_of_template_body() {
    let temp = tempfile::tempdir().expect("tempdir");
    let report = extract_to_drafts(
        temp.path(),
        Some(
            "我希望得到的是对其他项目也同样重要的语句，比如设计的时候先提问、先澄清目标、先规划。"
                .to_string(),
        ),
        None,
        vec!["codex".to_string()],
        Some("local".to_string()),
        true,
    )
    .expect("extract");

    let candidate = report
        .candidates
        .iter()
        .find(|candidate| {
            candidate.scope == "global"
                && candidate.memory_tier == crate::candidate::MemoryTier::CollaborationPreference
        })
        .expect("global collaboration preference");

    assert!(candidate.body.contains("对其他项目也同样重要"));
    assert!(candidate.body.contains("先提问"));
    assert!(!candidate.body.contains("ask clarifying questions"));
    assert_ne!(
        candidate.matched_template.as_deref(),
        Some("methodology-abstraction")
    );
}

#[test]
fn principle_request_language_is_not_filtered_when_it_expresses_cross_project_value() {
    let temp = tempfile::tempdir().expect("tempdir");
    let report = extract_high_value_text_to_drafts(
        temp.path(),
        "我希望得到的是对其他项目也同样重要的语句，比如设计的时候先提问、先澄清目标、先规划。",
        vec!["codex".to_string()],
        "test",
        Some("local".to_string()),
        true,
        8,
    )
    .expect("extract");

    assert!(
        report.candidates.iter().any(|candidate| {
            candidate.scope == "global"
                && candidate.memory_tier == crate::candidate::MemoryTier::CollaborationPreference
        }),
        "cross-project planning guidance should survive request-language filtering: {:?}",
        report.candidates
    );
}

#[test]
fn methodology_preferences_generate_first_class_non_project_candidates() {
    let temp = tempfile::tempdir().expect("tempdir");
    let report = extract_high_value_text_to_drafts(
        temp.path(),
        "开发期间我更在意核心功能和用户视角，设计的时候先提问、先澄清目标、先规划，小修改快测，大改再做完整验证。",
        vec!["codex".to_string()],
        "test",
        Some("local".to_string()),
        true,
        10,
    )
    .expect("extract");

    assert!(
        report.candidates.iter().any(|candidate| {
            candidate.memory_tier == crate::candidate::MemoryTier::CrossProjectPrinciple
                && candidate.scope == "global"
                && candidate.kind == "procedure"
        }),
        "should emit one approvable non-project methodology candidate: {:?}",
        report.candidates
    );
}

#[test]
fn repeated_feedback_rejections_suppress_candidate_generation() {
    let temp = tempfile::tempdir().expect("tempdir");
    for item_id in ["one", "two", "three"] {
        crate::feedback::record_feedback(
            temp.path(),
            "candidate",
            item_id,
            "rejected",
            "小修改快测，大改再做完整回归测试",
            Some("too generic".to_string()),
        )
        .expect("record feedback");
    }

    let report = extract_high_value_text_to_drafts(
        temp.path(),
        "小修改快测，大改再做完整回归测试。",
        vec!["codex".to_string()],
        "test",
        Some("local".to_string()),
        true,
        8,
    )
    .expect("extract");

    assert!(
        report
            .skipped
            .iter()
            .any(|item| item.contains("feedback-rejected")),
        "repeated feedback should suppress the matching candidate: {:?}",
        report.skipped
    );
}

#[test]
fn source_trust_uses_chunk_origin_metadata_not_body_text() {
    let mut candidate = candidate_with_body(
        "Assistant Mention",
        "Prefer explicit assistant handoff only after the user confirms the workflow.",
        0.8,
    );
    candidate.memory_tier = crate::candidate::MemoryTier::CollaborationPreference;

    let user_scores = candidate_value_scores_with_origin(
        &candidate,
        Some(crate::extract::chunk::ChunkOrigin::User),
    );
    let assistant_scores = candidate_value_scores_with_origin(
        &candidate,
        Some(crate::extract::chunk::ChunkOrigin::Assistant),
    );

    assert_eq!(user_scores.get("source_trust"), Some(&1.0));
    assert_eq!(assistant_scores.get("source_trust"), Some(&0.60));
}

#[test]
fn fallback_methodology_templates_link_dual_output_metadata_when_enabled() {
    let temp = tempfile::tempdir().expect("tempdir");
    let kernel_dir = config::kernel_dir(temp.path());
    fs::create_dir_all(&kernel_dir).expect("kernel dir");
    fs::write(
        kernel_dir.join("providers.yml"),
        r#"default: local
extraction_provider: local
fallback_methodology_templates: true
role_providers: {}
max_candidates_per_batch: 20
min_confidence: 0.7
providers:
  local:
    type: local-heuristic
privacy:
  upload_policy: ask
  redact_secrets: true
  include_code_context: false
  store_prompts_locally: true
"#,
    )
    .expect("write provider config");

    let report = extract_high_value_text_to_drafts(
        temp.path(),
        "开发期间关注核心功能，也要从用户视角持续优化体验。",
        vec!["codex".to_string()],
        "test",
        Some("local".to_string()),
        true,
        10,
    )
    .expect("extract");

    let project = report
        .candidates
        .iter()
        .find(|candidate| candidate.scope == "project")
        .expect("project candidate");
    let global = report
        .candidates
        .iter()
        .find(|candidate| candidate.scope == "global")
        .expect("global candidate");

    assert_eq!(project.abstraction_of.as_deref(), Some(global.id.as_str()));
    assert_eq!(global.abstracted_from.as_deref(), Some(project.id.as_str()));
    assert_eq!(
        global.matched_template.as_deref(),
        Some("fallback-methodology-template")
    );
}

#[test]
fn extract_command_creates_draft() {
    let temp = tempfile::tempdir().expect("tempdir");

    let report = extract_to_drafts(
        temp.path(),
        Some("以后前端请求统一使用 Axios，不要再用 Fetch。".to_string()),
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
        Some("以后前端请求统一使用 Axios，不要再用 Fetch。".to_string()),
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
                "基于用户确认，项目应该把 AI 生成的高质量项目改善也送入 Candidate/Draft，而不是直接写 MemoryCard；这能保留审阅边界。"
                    .to_string(),
            ),
            None,
            vec!["codex".to_string()],
            Some("local".to_string()),
            true,
        )
        .expect("extract");

    assert_eq!(report.candidates.len(), 1);
    assert!(report.candidates.iter().any(|candidate| {
        candidate.scope == "global"
            && candidate.memory_tier == crate::candidate::MemoryTier::CollaborationPreference
            && candidate.body.contains("审阅边界")
    }));
}

#[test]
fn rejects_bun_package_manager_preference() {
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

    assert!(report.candidates.is_empty());
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
  - title: Use Axios Client
    body: Use Axios with the project request wrapper for frontend HTTP requests.
    required:
      - axios
    context:
      - fetch
      - frontend http
"#,
    )
    .expect("write registry");

    let report = extract_to_drafts(
        temp.path(),
        Some("以后前端 HTTP 请求统一使用 Axios，不要再直接用 fetch。".to_string()),
        None,
        vec!["codex".to_string()],
        Some("local".to_string()),
        true,
    )
    .expect("extract");

    assert_eq!(report.candidates.len(), 1);
    assert_eq!(report.candidates[0].id, "project:use-axios-client");
    assert_eq!(report.candidates[0].title, "Use Axios Client");
    assert_eq!(
        report.candidates[0].body,
        "Use Axios with the project request wrapper for frontend HTTP requests."
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
        Some("以后前端请求统一使用 Axios，不要再用 Fetch；token=supersecret123456789.".to_string()),
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
        "one-off UI feedback and bug reports should not become MemoryCard drafts: {:?}",
        report.candidates
    );
}

#[test]
fn high_value_extraction_filters_one_off_memory_boundaries() {
    let temp = tempfile::tempdir().expect("tempdir");
    let report = extract_high_value_text_to_drafts(
        temp.path(),
        "不要改 Rust。\n只修改 app/src/main.tsx，不要改 src-tauri。\n当前 PR 还有两项没做。\n这个函数现在在 app.py。",
        vec!["codex".to_string()],
        "test",
        Some("local".to_string()),
        true,
        8,
    )
    .expect("extract");

    assert!(
        report.candidates.is_empty(),
        "one-off task state and rebuildable file/path facts should be silently filtered: {:?}",
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
        "unresolved product requests should not be stored as durable MemoryCards: {:?}",
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
fn high_value_extraction_canonicalizes_shorthand_methodology_lists() {
    let temp = tempfile::tempdir().expect("tempdir");
    let report = extract_high_value_text_to_drafts(
        temp.path(),
        "设计阶段：先提问、先澄清目标、先规划、从用户视角看。\n小改快测，大改重测。",
        vec!["codex".to_string()],
        "test",
        Some("local".to_string()),
        true,
        8,
    )
    .expect("extract");

    assert!(
        report.candidates.iter().any(|candidate| {
            candidate.scope == "global"
                && candidate.memory_tier == crate::candidate::MemoryTier::CollaborationPreference
                && candidate.body.contains("先提问")
                && candidate.body.contains("先规划")
        }),
        "planning shorthand should preserve the user's methodology wording: {:?}",
        report.candidates
    );
    assert!(
        report.candidates.iter().any(|candidate| {
            candidate.scope == "global"
                && candidate.memory_tier == crate::candidate::MemoryTier::CollaborationPreference
                && candidate.body.contains("小改快测")
        }),
        "test-strategy shorthand should preserve the user's methodology wording: {:?}",
        report.candidates
    );
}

#[test]
fn high_value_extraction_keeps_history_quality_and_review_boundary_preferences() {
    let temp = tempfile::tempdir().expect("tempdir");
    let report = extract_high_value_text_to_drafts(
        temp.path(),
        "重视真实历史回归，不迷信静态样例。\n更关心候选质量，而不是候选数量。\n希望结果能支持持续自我修正。\n协作阶段：先 review 再 merge、先保留审阅边界、不要让 AI 直接固化规则。",
        vec!["codex".to_string()],
        "test",
        Some("local".to_string()),
        true,
        8,
    )
    .expect("extract");

    assert!(
        report
            .candidates
            .iter()
            .any(|candidate| candidate.body.contains("真实历史回归")),
        "real-history validation should be retained: {:?}",
        report.candidates
    );
    assert!(
        report
            .candidates
            .iter()
            .all(|candidate| !candidate.body.contains("候选质量")),
        "candidate quality pipeline meta should be filtered: {:?}",
        report.candidates
    );
    assert!(
        report
            .candidates
            .iter()
            .any(|candidate| candidate.body.contains("审阅边界")),
        "review boundary preference should be retained: {:?}",
        report.candidates
    );
    assert!(
        report
            .candidates
            .iter()
            .any(|candidate| candidate.body.contains("持续自我修正")),
        "self-correction preference should be retained: {:?}",
        report.candidates
    );
}

#[test]
fn high_value_extraction_filters_artifacts_before_balanced_selection() {
    let temp = tempfile::tempdir().expect("tempdir");
    let noisy_prefix = (0..20)
        .map(|index| {
            format!("Acceptance criteria {index}: non-dry-run fixture source ids must stay stable.")
        })
        .collect::<Vec<_>>()
        .join("\n");
    let report = extract_high_value_text_to_drafts(
        temp.path(),
        &format!(
            "{noisy_prefix}\n重视真实历史回归，不迷信静态样例。\n更关心候选质量，而不是候选数量。"
        ),
        vec!["codex".to_string()],
        "test",
        Some("local".to_string()),
        true,
        4,
    )
    .expect("extract");

    assert!(
        report
            .candidates
            .iter()
            .any(|candidate| candidate.body.contains("真实历史回归")),
        "quality-skipped artifacts should not occupy selection slots: {:?}",
        report.candidates
    );
    assert!(
        report.candidates.iter().all(|candidate| !candidate
            .body
            .to_lowercase()
            .contains("acceptance criteria")),
        "artifact candidates should be filtered before ranking: {:?}",
        report.candidates
    );
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
        extract_high_value_candidates_with_preferences(text, &built_in_preferences(), 8, false);
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
