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
        report.precision_at_10 >= 0.80,
        "precision_at_10 too low: {report:#?}"
    );
    assert!(
        report.false_positives.is_empty(),
        "noise entered candidate set: {report:#?}"
    );
    assert!(
        report.visible_candidates <= 10,
        "default extraction should stay small: {report:#?}"
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
        Some("你又忘了跑测试。以后涉及 Rust 提取逻辑时，先加质量 fixture，再跑 cargo test。".to_string()),
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
