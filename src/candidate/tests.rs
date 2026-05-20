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
