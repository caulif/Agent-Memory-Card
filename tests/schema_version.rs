use agent_kernel::candidate::ExtractionMetadata;
use agent_kernel::draft::{self, NewDraft};
use agent_kernel::memory_card;

#[test]
fn draft_and_memory_card_records_default_to_current_schema_version() {
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
    memory_card::add_memory_card(
        temp.path(),
        "project:use-axios",
        "Use Axios",
        "Use Axios for frontend HTTP requests.",
        "preference",
        "project",
        vec!["codex".to_string()],
    )
    .expect("memory card");

    let drafts = draft::load_drafts(temp.path()).expect("drafts");
    let memory_cards = memory_card::load_memory_cards(temp.path()).expect("memory cards");

    assert_eq!(drafts[0].schema_version, 1);
    assert_eq!(memory_cards[0].schema_version, 1);
}
