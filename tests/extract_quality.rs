use std::collections::BTreeSet;

use agent_kernel::extract;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Fixture {
    cases: Vec<FixtureCase>,
}

#[derive(Debug, Deserialize)]
struct FixtureCase {
    name: String,
    text: String,
    expected_ids: Vec<String>,
}

#[test]
fn local_extraction_meets_minimum_quality_fixture_thresholds() {
    let fixture: Fixture =
        serde_yaml::from_str(include_str!("fixtures/extract_quality.yml")).expect("fixture yaml");
    let temp = tempfile::tempdir().expect("tempdir");

    let mut expected_total = 0usize;
    let mut actual_total = 0usize;
    let mut true_positive = 0usize;
    let mut failures = Vec::new();

    for case in fixture.cases {
        let report = extract::extract_text_to_drafts(
            temp.path(),
            &case.text,
            vec!["codex".to_string()],
            &case.name,
            Some("local".to_string()),
            true,
        )
        .expect("extract dry run");

        let expected = case.expected_ids.into_iter().collect::<BTreeSet<_>>();
        let actual = report
            .candidates
            .into_iter()
            .map(|candidate| candidate.id)
            .collect::<BTreeSet<_>>();

        expected_total += expected.len();
        actual_total += actual.len();
        true_positive += expected.intersection(&actual).count();

        let missing = expected.difference(&actual).cloned().collect::<Vec<_>>();
        let unexpected = actual.difference(&expected).cloned().collect::<Vec<_>>();
        if !missing.is_empty() || !unexpected.is_empty() {
            failures.push(format!(
                "{} missing={:?} unexpected={:?}",
                case.name, missing, unexpected
            ));
        }
    }

    let precision = if actual_total == 0 {
        1.0
    } else {
        true_positive as f32 / actual_total as f32
    };
    let recall = if expected_total == 0 {
        1.0
    } else {
        true_positive as f32 / expected_total as f32
    };

    assert!(
        precision >= 0.8 && recall >= 0.8,
        "precision={precision:.2} recall={recall:.2} failures={failures:#?}"
    );
}
