use super::*;

#[test]
fn eval_pipeline_options_keep_singletons_for_real_recall() {
    let options = pipeline_options_for_eval();
    assert!(
        options.cluster.keep_singletons,
        "eval must not discard singleton clusters; real reference cards often come from one high-signal observation"
    );
    assert!(
        matches!(
            options.induce,
            crate::extract::pipeline::InduceMode::BatchTopK { .. }
        ),
        "eval should use batch Top-K induce so real-project validation stays fast"
    );
}

#[test]
fn summary_averages_pairwise_scores() {
    let reports = vec![
        report_with_judge("one", "A", 0.25, 0.75),
        report_with_judge("two", "B", 0.55, 0.65),
    ];

    let summary = summarize_reports(&reports);

    assert!((summary.pipeline_score_avg - 0.40).abs() < 1e-6);
    assert!((summary.baseline_score_avg - 0.70).abs() < 1e-6);
}

#[test]
fn pairwise_validation_rejects_unknown_winner_and_out_of_range_scores() {
    let bad_winner = PairwiseJudgeResult {
        winner: "pipeline".to_string(),
        score_a: 0.5,
        score_b: 0.5,
        notes: Vec::new(),
    };
    assert!(validate_pairwise_result(&bad_winner).is_err());

    let bad_score = PairwiseJudgeResult {
        winner: "A".to_string(),
        score_a: 1.5,
        score_b: 0.5,
        notes: Vec::new(),
    };
    assert!(validate_pairwise_result(&bad_score).is_err());
}

#[test]
fn eval_timeout_override_can_increase_cli_timeout() {
    let mut cfg = ProviderConfig::default();
    apply_eval_timeouts(&mut cfg, 300);
    match cfg.providers.get("codex-cli").expect("codex provider") {
        Provider::CodexCli { timeout_secs, .. } => assert_eq!(*timeout_secs, 300),
        other => panic!("expected codex provider, got {other:?}"),
    }
}

fn report_with_judge(name: &str, winner: &str, score_a: f32, score_b: f32) -> ProjectEvalReport {
    ProjectEvalReport {
        name: name.to_string(),
        path: PathBuf::from(name),
        observation_count: 0,
        reference_cards: Vec::new(),
        pipeline_cards: Vec::new(),
        baseline_cards: Vec::new(),
        pairwise_judge: Some(PairwiseJudgeResult {
            winner: winner.to_string(),
            score_a,
            score_b,
            notes: Vec::new(),
        }),
        errors: Vec::new(),
        output_dir: PathBuf::from(name),
    }
}
