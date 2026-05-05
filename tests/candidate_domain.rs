use agent_kernel::candidate::{
    CandidateStatus, ExtractionMetadata, NewCandidate, add_candidate,
    approve_candidate_to_skilllet, hide_candidate, list_visible_candidates, load_candidates,
    reject_candidate,
};
use agent_kernel::{draft, skilllet};

fn new_candidate(id: &str) -> NewCandidate {
    NewCandidate {
        id: id.to_string(),
        title: "Prefer Bun".to_string(),
        kind: "preference".to_string(),
        scope: "project".to_string(),
        body: "Use Bun for JavaScript package management and scripts.".to_string(),
        brief: None,
        tags: Vec::new(),
        language: None,
        targets: vec!["codex".to_string()],
        evidence: "observation:session-1".to_string(),
        confidence: Some(0.92),
        reason: Some("Repeated package manager preference.".to_string()),
        matched_template: Some("built-in:prefer-bun".to_string()),
        source_observations: vec!["obs:session-1".to_string()],
        extraction: ExtractionMetadata::default(),
    }
}

#[test]
fn candidate_records_round_trip_under_project_candidates_dir() {
    let temp = tempfile::tempdir().expect("tempdir");

    add_candidate(temp.path(), new_candidate("project:prefer-bun")).expect("add candidate");
    let candidates = load_candidates(temp.path()).expect("load candidates");

    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].id, "project:prefer-bun");
    assert_eq!(candidates[0].status, CandidateStatus::Candidate);
    assert_eq!(candidates[0].schema_version, 1);
    assert_eq!(candidates[0].confidence, Some(0.92));
    assert_eq!(candidates[0].source_observations, vec!["obs:session-1"]);
}

#[test]
fn visible_candidates_exclude_hidden_rejected_and_promoted_records() {
    let temp = tempfile::tempdir().expect("tempdir");
    add_candidate(temp.path(), new_candidate("project:visible")).expect("visible");
    add_candidate(temp.path(), new_candidate("project:hidden")).expect("hidden");
    add_candidate(temp.path(), new_candidate("project:rejected")).expect("rejected");

    hide_candidate(temp.path(), "project:hidden").expect("hide");
    reject_candidate(
        temp.path(),
        "project:rejected",
        Some("Too specific to the current task.".to_string()),
    )
    .expect("reject");

    let visible = list_visible_candidates(temp.path()).expect("visible candidates");

    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].id, "project:visible");
}

#[test]
fn approve_candidate_creates_skilllet_and_marks_candidate_promoted() {
    let temp = tempfile::tempdir().expect("tempdir");
    add_candidate(temp.path(), new_candidate("project:prefer-bun")).expect("add candidate");

    let skilllet_record =
        approve_candidate_to_skilllet(temp.path(), "project:prefer-bun").expect("approve");
    let candidates = load_candidates(temp.path()).expect("load candidates");
    let skilllets = skilllet::load_skilllets(temp.path()).expect("load skilllets");
    let drafts = draft::load_drafts(temp.path()).expect("load drafts");

    assert_eq!(skilllet_record.id, "project:prefer-bun");
    assert_eq!(skilllets.len(), 1);
    assert!(skilllets[0].brief.contains("JavaScript 包管理"));
    assert!(skilllets[0].tags.contains(&"bun".to_string()));
    assert!(drafts.is_empty());
    assert_eq!(candidates[0].status, CandidateStatus::Promoted);
}
