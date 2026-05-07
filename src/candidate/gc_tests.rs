use super::*;

fn candidate(id: &str, title: &str, body: &str, evidence: &str) -> NewCandidate {
    NewCandidate {
        id: id.to_string(),
        title: title.to_string(),
        kind: "constraint".to_string(),
        scope: "project".to_string(),
        body: body.to_string(),
        brief: None,
        tags: Vec::new(),
        language: None,
        targets: vec!["codex".to_string()],
        evidence: evidence.to_string(),
        confidence: Some(0.9),
        reason: Some("test".to_string()),
        matched_template: Some("test".to_string()),
        source_observations: Vec::new(),
        extraction: ExtractionMetadata::default(),
    }
}

#[test]
fn gc_candidates_hides_existing_one_off_boundaries() {
    let temp = tempfile::tempdir().expect("tempdir");
    add_candidate(
        temp.path(),
        candidate(
            "project:prefer-bun",
            "Prefer Bun",
            "Use Bun for JavaScript package management and scripts.",
            "以后 JS 包管理统一使用 Bun。",
        ),
    )
    .expect("durable");
    add_candidate(
        temp.path(),
        candidate(
            "project:do-not-touch-rust",
            "不要改 Rust",
            "不要改 Rust",
            "不要改 Rust。",
        ),
    )
    .expect("one-off");

    let report = gc_candidates(temp.path()).expect("gc");
    let visible = list_visible_candidates(temp.path()).expect("visible");

    assert_eq!(report.hidden, vec!["project:do-not-touch-rust"]);
    assert_eq!(
        visible
            .iter()
            .map(|candidate| candidate.id.as_str())
            .collect::<Vec<_>>(),
        vec!["project:prefer-bun"]
    );
}
