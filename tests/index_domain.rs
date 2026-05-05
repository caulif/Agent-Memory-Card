use agent_kernel::{candidate, draft, index, skilllet};

#[test]
fn rebuild_project_index_counts_file_native_records() {
    let temp = tempfile::tempdir().expect("tempdir");
    candidate::add_candidate(temp.path(), new_candidate("project:use-axios")).expect("candidate");
    draft::add_draft(
        temp.path(),
        draft::NewDraft {
            id: "project:prefer-bun".to_string(),
            title: "Prefer Bun".to_string(),
            kind: "preference".to_string(),
            scope: "project".to_string(),
            body: "Use Bun for JavaScript package management and scripts.".to_string(),
            targets: vec!["codex".to_string()],
            evidence: "manual test".to_string(),
            confidence: None,
            reason: None,
            matched_template: None,
            extraction: candidate::ExtractionMetadata::default(),
        },
    )
    .expect("draft");
    skilllet::add_skilllet(
        temp.path(),
        "project:prefer-axios",
        "Prefer Axios",
        "Use Axios for frontend HTTP requests.",
        "preference",
        "project",
        vec!["codex".to_string()],
    )
    .expect("skilllet");

    let report = index::rebuild_project_index(temp.path()).expect("rebuild index");
    let loaded = index::load_project_index(temp.path()).expect("load index");

    assert_eq!(report.candidate_count, 1);
    assert_eq!(loaded.draft_count, 1);
    assert_eq!(loaded.skilllet_count, 1);
}

fn new_candidate(id: &str) -> candidate::NewCandidate {
    candidate::NewCandidate {
        id: id.to_string(),
        title: "Use Axios".to_string(),
        kind: "preference".to_string(),
        scope: "project".to_string(),
        body: "Use Axios for frontend HTTP requests.".to_string(),
        brief: None,
        tags: Vec::new(),
        language: None,
        targets: vec!["codex".to_string()],
        evidence: "manual test".to_string(),
        confidence: None,
        reason: None,
        matched_template: None,
        source_observations: Vec::new(),
        extraction: candidate::ExtractionMetadata::default(),
    }
}
