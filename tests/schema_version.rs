use agent_kernel::candidate::ExtractionMetadata;
use agent_kernel::draft::{self, NewDraft};
use agent_kernel::skilllet;

#[test]
fn draft_and_skilllet_records_default_to_current_schema_version() {
    let temp = tempfile::tempdir().expect("tempdir");

    draft::add_draft(
        temp.path(),
        NewDraft {
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
            extraction: ExtractionMetadata::default(),
        },
    )
    .expect("draft");
    skilllet::add_skilllet(
        temp.path(),
        "project:use-axios",
        "Use Axios",
        "Use Axios for frontend HTTP requests.",
        "preference",
        "project",
        vec!["codex".to_string()],
    )
    .expect("skilllet");

    let drafts = draft::load_drafts(temp.path()).expect("drafts");
    let skilllets = skilllet::load_skilllets(temp.path()).expect("skilllets");

    assert_eq!(drafts[0].schema_version, 1);
    assert_eq!(skilllets[0].schema_version, 1);
}
