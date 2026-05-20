use super::*;

fn new_candidate(id: &str, confidence: f32, matched_template: Option<&str>) -> NewCandidate {
    NewCandidate {
        id: id.to_string(),
        title: id.to_string(),
        kind: "preference".to_string(),
        scope: "project".to_string(),
        body: "Use stable project preferences.".to_string(),
        brief: None,
        tags: Vec::new(),
        language: None,
        targets: vec!["codex".to_string()],
        evidence: "test".to_string(),
        confidence: Some(confidence),
        reason: Some("test".to_string()),
        matched_template: matched_template.map(str::to_string),
        source_observations: vec![format!("obs:{id}")],
        extraction: ExtractionMetadata::default(),
    }
}

#[test]
fn visible_candidates_exclude_hidden_rejected_and_promoted_records() {
    let temp = tempfile::tempdir().expect("tempdir");
    add_candidate(
        temp.path(),
        new_candidate("keep", 0.95, Some("prefer-tool")),
    )
    .expect("keep");
    add_candidate(temp.path(), new_candidate("hide", 0.9, None)).expect("hide");
    add_candidate(temp.path(), new_candidate("reject", 0.88, None)).expect("reject");
    add_candidate(temp.path(), new_candidate("promote", 0.86, None)).expect("promote");

    hide_candidate(temp.path(), "hide").expect("hidden");
    reject_candidate(temp.path(), "reject", Some("not useful".to_string())).expect("rejected");
    approve_candidate_to_memory_card(temp.path(), "promote").expect("promoted");

    let visible = list_visible_candidates(temp.path()).expect("visible");

    assert_eq!(
        visible
            .iter()
            .map(|candidate| candidate.id.as_str())
            .collect::<Vec<_>>(),
        vec!["keep"]
    );
}

#[test]
fn candidate_review_actions_record_feedback() {
    let temp = tempfile::tempdir().expect("tempdir");
    add_candidate(temp.path(), new_candidate("reject", 0.88, None)).expect("reject");
    add_candidate(temp.path(), new_candidate("approve", 0.9, None)).expect("approve");

    reject_candidate(temp.path(), "reject", Some("too generic".to_string()))
        .expect("reject candidate");
    approve_candidate_to_memory_card(temp.path(), "approve").expect("approve candidate");

    let events = feedback::load_feedback(temp.path()).expect("feedback");

    assert_eq!(events.len(), 2);
    assert!(events.iter().any(|event| event.decision == "rejected"));
    assert!(events.iter().any(|event| event.decision == "approved"));
    assert!(
        events
            .iter()
            .any(|event| event.reason.as_deref() == Some("too generic"))
    );
}

#[test]
fn candidates_sort_by_confidence_template_and_update_time() {
    let temp = tempfile::tempdir().expect("tempdir");
    add_candidate(temp.path(), new_candidate("plain", 0.9, None)).expect("plain");
    add_candidate(
        temp.path(),
        new_candidate("templated", 0.9, Some("prefer-tool")),
    )
    .expect("templated");
    add_candidate(temp.path(), new_candidate("low", 0.4, None)).expect("low");

    let candidates = load_candidates(temp.path()).expect("candidates");

    assert_eq!(candidates[0].id, "templated");
    assert_eq!(candidates[1].id, "plain");
    assert_eq!(candidates[2].id, "low");
}

#[test]
fn candidates_get_chinese_brief_and_specific_tags() {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut axios = new_candidate("axios", 0.92, Some("prefer-tool"));
    axios.title = "Use Axios".to_string();
    axios.body = "Use Axios for frontend HTTP requests.".to_string();
    add_candidate(temp.path(), axios).expect("axios");

    let mut structured = new_candidate("structured", 0.9, Some("coding-pattern"));
    structured.title = "Prefer structured APIs over string manipulation".to_string();
    structured.body =
        "Use parsers or structured APIs instead of ad hoc string manipulation.".to_string();
    add_candidate(temp.path(), structured).expect("structured");

    let candidates = load_candidates(temp.path()).expect("candidates");
    let axios = candidates
        .iter()
        .find(|candidate| candidate.id == "axios")
        .expect("axios candidate");
    assert!(axios.brief.contains("前端 HTTP 请求优先使用 Axios"));
    assert!(axios.tags.contains(&"axios".to_string()));
    assert!(axios.tags.contains(&"frontend".to_string()));
    assert!(axios.tags.contains(&"生态偏好".to_string()));

    let structured = candidates
        .iter()
        .find(|candidate| candidate.id == "structured")
        .expect("structured candidate");
    assert!(structured.brief.contains("结构化数据"));
    assert!(structured.tags.contains(&"structured-data".to_string()));
    assert!(structured.tags.contains(&"parser".to_string()));
}

#[test]
fn approving_candidate_creates_memory_card_and_preserves_metadata() {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut candidate = new_candidate("project:use-axios", 0.92, Some("prefer-tool"));
    candidate.title = "Use Axios".to_string();
    candidate.body = "Use Axios for frontend HTTP requests.".to_string();
    add_candidate(temp.path(), candidate).expect("candidate");

    let memory_card =
        approve_candidate_to_memory_card(temp.path(), "project:use-axios").expect("memory_card");
    let visible = list_visible_candidates(temp.path()).expect("visible candidates");

    assert_eq!(memory_card.id, "project:use-axios");
    assert!(memory_card.brief.contains("前端 HTTP 请求优先使用 Axios"));
    assert!(memory_card.tags.contains(&"axios".to_string()));
    assert!(visible.is_empty());
    assert!(draft::load_drafts(temp.path()).expect("drafts").is_empty());
}

#[test]
fn approving_chatty_candidate_matures_body_before_persisting() {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut candidate = new_candidate("project:rough-goal", 0.91, Some("local"));
    candidate.title = "/goal 全自动执行，不要问我了，自主搜索下载".to_string();
    candidate.body = "/goal 全自动执行，不要问我了，自主搜索下载好用的工具，自己决策和实现，最后给我成品就行了。".to_string();
    candidate.brief = Some("这条候选建议沉淀了“/goal 全自动执行，不要问我了”：/goal 全自动执行，不要问我了，自主搜索下载好用的工具".to_string());
    candidate.kind = "constraint".to_string();
    add_candidate(temp.path(), candidate).expect("candidate");

    let memory_card =
        approve_candidate_to_memory_card(temp.path(), "project:rough-goal").expect("memory_card");

    assert!(memory_card.body.contains("触发："));
    assert!(memory_card.body.contains("动作："));
    assert!(memory_card.body.contains("边界："));
    assert!(!memory_card.body.contains("/goal"));
    assert!(!memory_card.brief.contains("这条候选建议沉淀"));
}

#[test]
fn approving_candidate_persists_value_directed_synthesis_metadata() {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut candidate = new_candidate("project:skill-quality-gap", 0.94, Some("high-value-prompt"));
    candidate.title = "Skills need targeted improvements".to_string();
    candidate.body = "When a Memory Card can improve a project Skill, show the existing Skill gap and the future behavior change before approval.".to_string();
    candidate.kind = "procedure".to_string();
    candidate.tags = vec!["skill".to_string(), "review".to_string()];
    candidate.extraction.suggested_action =
        Some(ExtractionAction::new_candidate_for_route("workflow_skill"));
    add_candidate(temp.path(), candidate).expect("candidate");

    let memory_card = approve_candidate_to_memory_card(temp.path(), "project:skill-quality-gap")
        .expect("memory_card");

    let extraction = memory_card.extraction.expect("synthesis metadata");
    assert_eq!(extraction.card_function.as_deref(), Some("skill_targeted"));
    assert!(
        extraction
            .value_claim
            .as_deref()
            .unwrap_or_default()
            .contains("Skill")
    );
    let delta = extraction.value_delta.expect("value delta");
    assert!(!delta.existing_behavior.trim().is_empty());
    assert!(!delta.missing_part.trim().is_empty());
    assert!(!delta.new_behavior.trim().is_empty());
    assert!(!delta.why_not_duplicate.trim().is_empty());
    assert_eq!(
        extraction
            .target_context
            .as_ref()
            .map(|target| target.target_type.as_str()),
        Some("project_skill")
    );
    assert!(
        extraction
            .synthesis_trace
            .iter()
            .any(|entry| entry.step == "value_delta")
    );
}

#[test]
fn visible_candidate_inbox_includes_synthesis_preview_before_approval() {
    let temp = tempfile::tempdir().expect("tempdir");
    crate::observation::import_observation_text(
        temp.path(),
        &temp.path().join("session.jsonl"),
        "codex-session",
        Some("codex"),
        "用户要求 Memory Card 审阅前就展示价值增量、目标上下文和可审阅 trace。",
    )
    .expect("observation");
    let mut candidate = new_candidate("project:review-before-approval", 0.91, Some("review"));
    candidate.title = "Review Value Before Approval".to_string();
    candidate.body =
        "Memory Card review should show value delta and trace before approval.".to_string();
    candidate.kind = "procedure".to_string();
    add_candidate(temp.path(), candidate).expect("candidate");

    let visible =
        list_visible_candidates_with_synthesis_preview(temp.path()).expect("visible candidates");
    let extraction = &visible[0].extraction;

    assert!(extraction.value_claim.is_some());
    assert!(extraction.value_delta.is_some());
    assert!(extraction.target_context.is_some());
    assert!(
        extraction
            .synthesis_trace
            .iter()
            .any(|entry| entry.step == "search_observations")
    );
}

#[test]
fn visible_candidate_inbox_matures_existing_value_metadata_preview() {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut candidate = new_candidate("project:chatty-existing-metadata", 0.91, Some("review"));
    candidate.title = "用户说以后别只看编译".to_string();
    candidate.body = "以后完成 UI 修改以后不要只告诉我编译通过，还要自己试用一下。".to_string();
    candidate.extraction.card_function = Some("workflow".to_string());
    candidate.extraction.value_claim = Some("用于要求 UI 修改后完成真实试用。".to_string());
    candidate.extraction.value_delta = Some(ValueDelta {
        existing_behavior: "已有流程会运行编译检查。".to_string(),
        missing_part: "缺少真实用户路径试用。".to_string(),
        new_behavior: "完成 UI 修改后按用户路径试用并记录问题。".to_string(),
        why_not_duplicate: "该卡补上验收边界，不重复编译规则。".to_string(),
    });
    candidate.extraction.target_context = Some(TargetContext {
        target_type: "workflow".to_string(),
        target_id: None,
        why_this_target: "这是 UI 验收工作流边界。".to_string(),
    });
    candidate.extraction.synthesis_trace = vec![SynthesisTraceEntry {
        step: "value_delta".to_string(),
        summary: "existing metadata".to_string(),
    }];
    add_candidate(temp.path(), candidate).expect("candidate");

    let visible =
        list_visible_candidates_with_synthesis_preview(temp.path()).expect("visible candidates");
    let candidate = &visible[0];

    assert!(candidate.body.contains("触发"));
    assert!(candidate.body.contains("动作"));
    assert!(candidate.body.contains("边界"));
    assert_eq!(
        candidate.extraction.synthesis_action.as_deref(),
        Some("workflow_card")
    );
    assert_eq!(
        candidate.extraction.synthesis_stop_reason.as_deref(),
        Some("workflow_gap_found")
    );
}

#[test]
fn approving_merge_candidate_updates_existing_memory_card() {
    let temp = tempfile::tempdir().expect("tempdir");
    memory_card::add_memory_card(
        temp.path(),
        "project:frontend-flow",
        "Frontend Flow",
        "Run frontend checks before sync.",
        "procedure",
        "project",
        vec!["codex".to_string()],
    )
    .expect("existing memory card");

    let mut candidate = new_candidate(
        "project:frontend-flow-update",
        0.93,
        Some("llm-memory-refine"),
    );
    candidate.title = "Frontend Flow Update".to_string();
    candidate.body = "When changing frontend workflows, run focused UI tests before syncing generated agent artifacts; keep broad tests for larger changes.".to_string();
    candidate.kind = "procedure".to_string();
    candidate.extraction.suggested_action = Some(ExtractionAction::merge_into_existing(
        "project:frontend-flow".to_string(),
        0.91,
    ));
    add_candidate(temp.path(), candidate).expect("candidate");

    let updated = approve_candidate_to_memory_card(temp.path(), "project:frontend-flow-update")
        .expect("updated memory card");
    let visible = list_visible_candidates(temp.path()).expect("visible candidates");
    let cards = memory_card::load_memory_cards(temp.path()).expect("memory cards");

    assert_eq!(updated.id, "project:frontend-flow");
    assert!(updated.body.contains("focused UI tests"));
    assert_eq!(
        updated
            .extraction
            .as_ref()
            .and_then(|metadata| metadata.card_function.as_deref()),
        Some("merge")
    );
    assert!(
        updated
            .extraction
            .as_ref()
            .and_then(|metadata| metadata.value_delta.as_ref())
            .is_some_and(|delta| delta.why_not_duplicate.contains("合并"))
    );
    assert_eq!(cards.len(), 1);
    assert!(visible.is_empty());
}

#[test]
fn approving_legacy_principle_candidate_normalizes_kind() {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut candidate = new_candidate("global:legacy-principle", 0.92, Some("principle-signal"));
    candidate.title = "Legacy Principle".to_string();
    candidate.body = "When planning durable changes, verify the main workflow before polishing secondary details.".to_string();
    candidate.kind = "principle".to_string();
    candidate.scope = "global".to_string();
    add_candidate(temp.path(), candidate).expect("candidate");

    let memory_card = approve_candidate_to_memory_card(temp.path(), "global:legacy-principle")
        .expect("memory_card");

    assert_eq!(memory_card.kind, "procedure");
    assert_eq!(memory_card.scope, "global");
}
