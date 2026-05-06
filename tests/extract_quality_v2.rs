use agent_kernel::extract;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct FixtureFile {
    cases: Vec<FixtureCase>,
}

#[derive(Debug, Deserialize)]
struct FixtureCase {
    id: String,
    origin: String,
    input: String,
    expected: String,
    expected_signal: String,
    expected_artifact_kind: String,
    expected_hardness: String,
    expected_terms: Vec<String>,
    expected_tags: Vec<String>,
}

#[test]
fn high_quality_extraction_v2_meets_precision_gate() {
    let fixture: FixtureFile =
        serde_yaml::from_str(include_str!("fixtures/extract_quality_v2.yml"))
            .expect("parse extract quality fixtures");

    let report = extract::quality_report_for_text_cases(
        fixture
            .cases
            .iter()
            .map(|case| extract::QualityTextCase {
                id: case.id.clone(),
                origin: case.origin.clone(),
                input: case.input.clone(),
                expected: case.expected.clone(),
                expected_signal: case.expected_signal.clone(),
                expected_artifact_kind: case.expected_artifact_kind.clone(),
                expected_hardness: case.expected_hardness.clone(),
                expected_terms: case.expected_terms.clone(),
                expected_tags: case.expected_tags.clone(),
            })
            .collect(),
    );

    assert!(
        report.precision_at_10 >= 0.90,
        "precision_at_10 too low: {report:#?}"
    );
    assert!(
        report.false_positives.is_empty(),
        "noise entered candidate set: {report:#?}"
    );
    assert!(
        report.visible_candidates <= 12,
        "expanded corpus should still produce a compact positive set: {report:#?}"
    );
}

#[test]
fn local_extract_keeps_dotted_artifact_names_in_candidate_body() {
    let temp = tempfile::tempdir().expect("tempdir");

    let report = extract::extract_to_drafts(
        temp.path(),
        Some("不要直接覆盖 AGENTS.md；发现漂移时先导入成 Draft，让我确认后再同步。".to_string()),
        None,
        vec!["codex".to_string()],
        Some("local".to_string()),
        true,
    )
    .expect("extract");

    assert_eq!(report.candidates.len(), 1);
    assert!(report.candidates[0].body.contains("AGENTS.md"));
    assert!(report.candidates[0].body.contains("Draft"));
}

#[test]
fn local_extract_persists_classification_metadata_on_draft() {
    let temp = tempfile::tempdir().expect("tempdir");

    extract::extract_to_drafts(
        temp.path(),
        Some("不要直接覆盖 AGENTS.md；发现漂移时先导入成 Draft，让我确认后再同步。".to_string()),
        None,
        vec!["codex".to_string()],
        Some("local".to_string()),
        false,
    )
    .expect("extract");

    let drafts = agent_kernel::draft::load_drafts(temp.path()).expect("drafts");
    let classification = drafts[0]
        .extraction
        .classification
        .as_ref()
        .expect("classification metadata");

    assert_eq!(classification.signal, "constraint");
    assert_eq!(classification.artifact_kind, "always_on_rule");
    assert_eq!(classification.hardness, "high");
    assert!(
        drafts[0]
            .extraction
            .tags
            .contains(&"domain:governance".to_string())
    );
    let evidence_span = drafts[0]
        .extraction
        .evidence_span
        .as_ref()
        .expect("structured evidence span");
    assert_eq!(evidence_span.role, "user");
    assert!(evidence_span.quote.contains("AGENTS.md"));
    assert!(!drafts[0].body.contains("observation:"));
}

#[test]
fn dry_run_axios_preference_candidate() {
    let temp = tempfile::tempdir().expect("tempdir");

    let report = extract::extract_to_drafts(
        temp.path(),
        Some("以后这个项目的前端 HTTP 请求统一用 Axios，不要再写裸 fetch。".to_string()),
        None,
        vec!["codex".to_string()],
        Some("local".to_string()),
        true,
    )
    .expect("extract");

    assert!(
        !report.candidates.is_empty(),
        "Axios HTTP preference should produce a candidate"
    );
    let preview = &report.candidates[0];
    let classification = preview.classification.as_ref().expect("classification");
    assert_eq!(
        classification.signal, "preference",
        "expected preference signal"
    );
    assert_eq!(
        classification.hardness, "low",
        "expected low hardness for tool preference"
    );
    assert!(
        preview.tags.contains(&"target:agents-md".to_string()),
        "always_on_rule artifact should have target:agents-md tag; got {:#?}",
        preview.tags
    );
}

#[test]
fn dry_run_accepted_ai_improvement_candidate() {
    let temp = tempfile::tempdir().expect("tempdir");

    let report = extract::extract_to_drafts(
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

    assert!(
        !report.candidates.is_empty(),
        "accepted AI project improvement text should produce a candidate"
    );
    let preview = &report.candidates[0];
    let classification = preview.classification.as_ref().expect("classification");
    assert_eq!(
        classification.signal, "ai_project_improvement",
        "expected ai_project_improvement signal"
    );
    assert_eq!(
        classification.artifact_kind, "review_only",
        "expected review_only artifact"
    );
    assert!(
        preview.tags.contains(&"evidence:accepted-ai".to_string()),
        "should have evidence:accepted-ai tag; got {:#?}",
        preview.tags
    );
}

#[test]
fn dry_run_rejects_standalone_complaint_and_keeps_actionable_validation() {
    let temp = tempfile::tempdir().expect("tempdir");

    let report = extract::extract_to_drafts(
        temp.path(),
        Some(
            "你又忘了跑测试。以后涉及 Rust 提取逻辑时，先加质量 fixture，再跑 cargo test。"
                .to_string(),
        ),
        None,
        vec!["codex".to_string()],
        Some("local".to_string()),
        true,
    )
    .expect("extract");

    assert_eq!(
        report.candidates.len(),
        1,
        "standalone complaint should not become its own Candidate: {:#?}",
        report.candidates
    );
    assert!(
        !report.candidates[0].body.contains("你又忘了跑测试"),
        "Candidate should keep the actionable validation rule, not the complaint"
    );
    let classification = report.candidates[0]
        .classification
        .as_ref()
        .expect("classification");
    assert_eq!(classification.signal, "validation");
    assert_eq!(classification.hardness, "high");
}

#[test]
fn dry_run_previews_include_classification_and_tags() {
    let temp = tempfile::tempdir().expect("tempdir");

    let report = extract::extract_to_drafts(
        temp.path(),
        Some(
            "以后这个项目都用 Bun 管理 JavaScript 依赖和脚本，不要再建议 npm install。".to_string(),
        ),
        None,
        vec!["codex".to_string()],
        Some("local".to_string()),
        true,
    )
    .expect("extract");

    let preview = &report.candidates[0];
    let classification = preview.classification.as_ref().expect("classification");
    assert_eq!(classification.signal, "preference");
    assert_eq!(classification.hardness, "low");
    assert!(preview.tags.contains(&"shape:preference".to_string()));
    assert!(
        report
            .render()
            .contains("classification: signal=preference")
    );
}

#[test]
fn dry_run_duplicate_existing_skilllet_is_suppressed_as_noop() {
    let temp = tempfile::tempdir().expect("tempdir");
    agent_kernel::skilllet::add_skilllet(
        temp.path(),
        "project:prefer-bun",
        "Prefer Bun",
        "Use Bun for JavaScript package management and scripts.",
        "preference",
        "project",
        vec!["codex".to_string()],
    )
    .expect("seed skilllet");

    let report = extract::extract_to_drafts(
        temp.path(),
        Some(
            "以后这个项目都用 Bun 管理 JavaScript 依赖和脚本，不要再建议 npm install。".to_string(),
        ),
        None,
        vec!["codex".to_string()],
        Some("local".to_string()),
        true,
    )
    .expect("extract");

    assert!(
        report.candidates.is_empty(),
        "existing equivalent Skilllets should not reappear as candidate noise"
    );
    assert!(
        report
            .skipped
            .iter()
            .any(|item| item.contains("duplicate-existing")),
        "duplicate suppression should be visible in skipped reasons: {:#?}",
        report.skipped
    );
}

#[test]
fn dry_run_filters_internal_evidence_leaks_from_candidate_text() {
    let temp = tempfile::tempdir().expect("tempdir");

    let report = extract::extract_to_drafts(
        temp.path(),
        Some(
            "evidence: 81 observations: observation:obs:claude-code:sha256:015e2, observation:obs:claude-code:sha256:053a9 - Do NOT mention teammate proposals."
                .to_string(),
        ),
        None,
        vec!["codex".to_string()],
        Some("local".to_string()),
        true,
    )
    .expect("extract");

    assert!(
        report.candidates.is_empty(),
        "internal observation ids and evidence metadata must not enter candidate inbox: {:#?}",
        report.candidates
    );
    assert!(
        report
            .skipped
            .iter()
            .any(|item| item.contains("internal-leak")),
        "quality gate should explain the internal leak rejection: {:#?}",
        report.skipped
    );
}

#[test]
fn dry_run_filters_meta_discussion_questions_about_the_pipeline() {
    let temp = tempfile::tempdir().expect("tempdir");

    let report = extract::extract_to_drafts(
        temp.path(),
        Some(
            "是否已经把 Skilllet 编译为 hook（例如把 git commit 前必须 cargo clippy 编译成 PreToolUse hook）？"
                .to_string(),
        ),
        None,
        vec!["codex".to_string()],
        Some("local".to_string()),
        true,
    )
    .expect("extract");

    assert!(
        report.candidates.is_empty(),
        "questions about implementation planning should not become Skilllet candidates"
    );
    assert!(
        report
            .skipped
            .iter()
            .any(|item| item.contains("meta-discussion")),
        "quality gate should explain the meta-discussion rejection: {:#?}",
        report.skipped
    );
}

#[test]
fn dry_run_hides_exact_duplicate_existing_skilllets_instead_of_showing_new_candidates() {
    let temp = tempfile::tempdir().expect("tempdir");
    agent_kernel::skilllet::add_skilllet(
        temp.path(),
        "project:prefer-bun",
        "Prefer Bun",
        "Use Bun for JavaScript package management and scripts.",
        "preference",
        "project",
        vec!["codex".to_string()],
    )
    .expect("seed skilllet");

    let report = extract::extract_to_drafts(
        temp.path(),
        Some(
            "以后这个项目都用 Bun 管理 JavaScript 依赖和脚本，不要再建议 npm install。".to_string(),
        ),
        None,
        vec!["codex".to_string()],
        Some("local".to_string()),
        true,
    )
    .expect("extract");

    assert!(
        report.candidates.is_empty(),
        "near-exact duplicates should be treated as NOOP/evidence merge, not shown as new drafts"
    );
    assert!(
        report
            .skipped
            .iter()
            .any(|item| item.contains("duplicate-existing")),
        "duplicate suppression should be visible in skipped reasons: {:#?}",
        report.skipped
    );
}

#[test]
fn dry_run_splits_preference_and_exception_into_atomic_candidates() {
    let temp = tempfile::tempdir().expect("tempdir");

    let report = extract::extract_to_drafts(
        temp.path(),
        Some(
            "以后 HTTP 请求默认用 Axios，但上传大文件保留 fetch，因为需要 ReadableStream streaming。"
                .to_string(),
        ),
        None,
        vec!["codex".to_string()],
        Some("local".to_string()),
        true,
    )
    .expect("extract");

    assert!(
        report.candidates.len() >= 2,
        "preference plus exception should become separate atomic candidates: {:#?}",
        report.candidates
    );
    assert!(
        report
            .candidates
            .iter()
            .any(|candidate| candidate.body.contains("Axios"))
    );
    assert!(
        report
            .candidates
            .iter()
            .any(|candidate| candidate.body.contains("fetch")
                && candidate.body.contains("ReadableStream")),
        "exception should be preserved as its own candidate: {:#?}",
        report.candidates
    );
}

#[test]
fn repeated_non_dry_extractions_boost_recurring_candidate_confidence() {
    let temp = tempfile::tempdir().expect("tempdir");
    let text = "以后所有 Rust 改动必须运行 cargo clippy。";

    for source in ["session-1", "session-2", "session-3"] {
        extract::extract_to_drafts(
            temp.path(),
            Some(text.to_string()),
            None,
            vec!["codex".to_string()],
            Some("local".to_string()),
            false,
        )
        .unwrap_or_else(|error| panic!("extract {source}: {error}"));
    }

    let report = extract::extract_to_drafts(
        temp.path(),
        Some(text.to_string()),
        None,
        vec!["codex".to_string()],
        Some("local".to_string()),
        true,
    )
    .expect("extract dry run");

    let candidate = report
        .candidates
        .iter()
        .find(|candidate| candidate.body.contains("cargo clippy"))
        .expect("recurring cargo clippy candidate");
    assert!(
        candidate.confidence.unwrap_or(0.0) >= 0.9,
        "recurring candidate should be boosted: {candidate:#?}"
    );
    assert!(
        candidate
            .reason
            .as_deref()
            .unwrap_or_default()
            .contains("Recurring across 3 observations"),
        "reason should explain recurrence: {candidate:#?}"
    );
}

#[test]
fn future_value_gate_rejects_soft_personality_without_operational_trigger() {
    let chunk = agent_kernel::extract::chunk::text_case_to_chunk(
        "soft-personality-noise",
        "assistant",
        "以后保持热情积极、有自己的品味，让用户感觉更舒服。",
    );

    let decision = agent_kernel::extract::gate::future_value_gate(&chunk);

    assert_eq!(decision.disposition, "reject");
    assert_eq!(
        decision.reason,
        "soft-personality-without-operational-trigger"
    );
}

#[test]
fn dry_run_previews_include_route_and_compile_decision() {
    let temp = tempfile::tempdir().expect("tempdir");

    let report = extract::extract_to_drafts(
        temp.path(),
        Some("以后这个项目的前端 HTTP 请求统一用 Axios，不要再写裸 fetch。".to_string()),
        None,
        vec!["codex".to_string()],
        Some("local".to_string()),
        true,
    )
    .expect("extract");

    let action = report.candidates[0]
        .suggested_action
        .as_ref()
        .expect("suggested action");

    assert_eq!(action.action, "new_candidate");
    assert_eq!(action.route, "always_on_rule");
    assert_eq!(action.compile_enabled, Some(true));
    assert!(
        action
            .rationale
            .as_deref()
            .unwrap_or_default()
            .contains("AGENTS.md")
    );
}
