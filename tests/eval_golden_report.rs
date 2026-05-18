use agent_kernel::eval::{
    append_seen_memory_signatures, run_golden_set_eval, run_golden_set_eval_with_induce_provider,
    score_pipeline_report,
};
use agent_kernel::extract::crystallize::CrystallizedCard;
use agent_kernel::extract::induce::EvidenceQuote;
use agent_kernel::extract::pipeline::PipelineReport;
use agent_kernel::observation::ObservationRecord;
use agent_kernel::provider::ProviderRequest;
use anyhow::Result;

#[test]
fn golden_set_eval_reports_quality_metrics() {
    let report = run_golden_set_eval(std::path::Path::new(".")).expect("golden eval");

    assert!(report.positive_total >= 17);
    assert!(report.negative_total >= 28);
    assert!(report.positive_recall_percent >= 80.0);
    assert!(report.negative_precision_percent >= 85.0);
    assert_eq!(report.one_off_false_positive_count, 0);
    assert_eq!(report.one_off_false_positive_percent, 0.0);
    assert!(report.duplicate_cluster_risk_count <= report.positive_total);
    assert!(report.evidence_valid_count <= report.positive_total);
    assert!(report.evidence_valid_percent >= 70.0);
    assert_eq!(report.provider_evidence_valid_count, None);
    assert_eq!(report.provider_evidence_valid_percent, None);
    assert_eq!(report.positive_cases.len(), report.positive_total);
    assert_eq!(report.negative_cases.len(), report.negative_total);
    assert!(report.render_markdown().contains("Golden Set Eval"));
    assert!(report.render_markdown().contains("positive recall"));
    assert!(report.render_markdown().contains("negative precision"));
    assert!(report.render_markdown().contains("one-off false positive"));
    assert!(report.render_markdown().contains("duplicate cluster risk"));
    assert!(report.render_markdown().contains("evidence validity"));
}

#[test]
fn eval_cli_can_run_golden_set_report() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_agent-kernel"))
        .arg("eval")
        .arg("--golden-set")
        .arg("--project")
        .arg(".")
        .output()
        .expect("run eval --golden-set");

    assert!(
        output.status.success(),
        "eval --golden-set failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Golden Set Eval"));
    assert!(stdout.contains("positive recall"));
    assert!(stdout.contains("negative precision"));
    assert!(stdout.contains("one-off false positive"));
    assert!(stdout.contains("duplicate cluster risk"));
    assert!(stdout.contains("evidence validity"));
}

struct EchoProvider;

impl agent_kernel::extract::induce::InduceProvider for EchoProvider {
    fn call(&self, request: &ProviderRequest, _: usize) -> Result<String> {
        let cluster_id = parse_prompt_field(&request.user_prompt, "cluster_id").unwrap_or("golden");
        let recurrence = parse_prompt_field(&request.user_prompt, "recurrence")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(1);
        if recurrence < 2 {
            return Ok(r#"{"reject": true, "reason": "singleton"}"#.to_string());
        }
        let (observation_id, quote) =
            parse_first_prompt_message(&request.user_prompt).expect("prompt evidence");
        Ok(format!(
            r#"{{
  "cluster_id": "{}",
  "recurrence": {},
  "title": "校验Provider证据回溯",
  "when": "provider 根据重复观察归纳 Memory Card 时",
  "what": "只保留能回到原始 observation 的 evidence quote",
  "why": "让 provider-induced evidence 可以被人工审查和回放验证",
  "kind": "procedure",
  "scope": "global",
  "evidence_quotes": [{{"observation_id": "{}", "text": "{}"}}],
  "temporal_status": "stable",
  "confidence": 0.9
}}"#,
            json_escape(cluster_id),
            recurrence,
            json_escape(&observation_id),
            json_escape(&quote)
        ))
    }
}

struct HallucinatingProvider;

impl agent_kernel::extract::induce::InduceProvider for HallucinatingProvider {
    fn call(&self, _: &ProviderRequest, _: usize) -> Result<String> {
        Ok(r#"{
  "title": "Bad provider evidence",
  "when": "always",
  "what": "use invented evidence",
  "why": "bad fixture",
  "kind": "procedure",
  "scope": "global",
  "evidence_quotes": [{"observation_id": "missing", "text": "not in source"}],
  "temporal_status": "stable",
  "confidence": 0.9
}"#
        .to_string())
    }
}

#[test]
fn golden_set_eval_can_measure_provider_induced_evidence_validity() {
    let report = run_golden_set_eval_with_induce_provider(std::path::Path::new("."), &EchoProvider)
        .expect("provider golden eval");

    assert!(
        report.provider_evidence_valid_count.unwrap_or(0) >= 14,
        "provider evidence count should cover most positives: {:#?}",
        report.provider_evidence_valid_count
    );
    assert!(
        report.provider_evidence_valid_percent.unwrap_or(0.0) >= 80.0,
        "provider evidence percent should be useful: {:#?}",
        report.provider_evidence_valid_percent
    );
    assert!(
        report
            .render_markdown()
            .contains("provider evidence validity")
    );
    assert!(
        report
            .positive_cases
            .iter()
            .any(|case| case.provider_evidence_valid == Some(true))
    );
}

#[test]
fn golden_set_eval_catches_provider_evidence_hallucination() {
    let report =
        run_golden_set_eval_with_induce_provider(std::path::Path::new("."), &HallucinatingProvider)
            .expect("provider golden eval");

    assert_eq!(report.provider_evidence_valid_count, Some(0));
    assert_eq!(report.provider_evidence_valid_percent, Some(0.0));
    assert!(
        report
            .positive_cases
            .iter()
            .all(|case| case.provider_evidence_valid == Some(false))
    );
}

#[test]
fn score_pipeline_report_flags_duplicate_and_missing_evidence() {
    let observations = vec![ObservationRecord {
        id: "obs:1".to_string(),
        source_kind: "test".to_string(),
        source_path: "test.jsonl".to_string(),
        agent: Some("codex".to_string()),
        body: "评估提炼质量时必须用真实历史会话回归验证。".to_string(),
        evidence: "test".to_string(),
        redacted: false,
        created_at: "2026-05-01T00:00:00+00:00".to_string(),
    }];
    let report = PipelineReport {
        observations_in: 1,
        stripped_kept: 1,
        truncated_count: 1,
        truncated_long_messages: 0,
        clusters_in: 1,
        induce_accepted: 2,
        induce_rejected: 0,
        induce_failed: 1,
        induce_calls: 1,
        induce_clusters_selected: 1,
        crystallize_accepted: 2,
        crystallize_rejected: 0,
        cards: vec![
            test_card(
                "c1",
                vec![EvidenceQuote {
                    observation_id: "obs:1".to_string(),
                    text: "真实历史会话回归验证".to_string(),
                }],
            ),
            test_card("c2", Vec::new()),
        ],
        crystallize_rejects: Vec::new(),
        failures_preview: vec!["provider malformed JSON".to_string()],
        layer_timings_ms: Default::default(),
        pipeline_version: 1,
    };

    let qa = score_pipeline_report(&report, &observations).expect("qa");

    assert_eq!(qa.cards_total, 2);
    assert_eq!(qa.provider_failures, 1);
    assert_eq!(qa.evidence_valid, 1);
    assert_eq!(qa.evidence_missing, 1);
    assert!(qa.score < 90);
    assert!(
        qa.issues
            .iter()
            .any(|issue| issue.contains("missing evidence"))
    );
}

#[test]
fn score_pipeline_report_accepts_signal_fragment_evidence_ids() {
    let observations = vec![ObservationRecord {
        id: "obs:base".to_string(),
        source_kind: "test".to_string(),
        source_path: "test.jsonl".to_string(),
        agent: Some("codex".to_string()),
        body: "重视真实历史回归，不迷信静态样例。".to_string(),
        evidence: "test".to_string(),
        redacted: false,
        created_at: "2026-05-01T00:00:00+00:00".to_string(),
    }];
    let report = PipelineReport {
        observations_in: 1,
        stripped_kept: 1,
        truncated_count: 1,
        truncated_long_messages: 0,
        clusters_in: 1,
        induce_accepted: 1,
        induce_rejected: 0,
        induce_failed: 0,
        induce_calls: 1,
        induce_clusters_selected: 1,
        crystallize_accepted: 1,
        crystallize_rejected: 0,
        cards: vec![test_card(
            "c1",
            vec![EvidenceQuote {
                observation_id: "obs:base#signal1".to_string(),
                text: "真实历史回归".to_string(),
            }],
        )],
        crystallize_rejects: Vec::new(),
        failures_preview: Vec::new(),
        layer_timings_ms: Default::default(),
        pipeline_version: 1,
    };

    let qa = score_pipeline_report(&report, &observations).expect("qa");

    assert_eq!(qa.evidence_valid, 1);
    assert_eq!(qa.evidence_invalid, 0);
}

#[test]
fn score_pipeline_report_flags_expected_project_workflow_recall_miss() {
    let observations = vec![
        ObservationRecord {
            id: "obs:dao:1".to_string(),
            source_kind: "test".to_string(),
            source_path: "dao.jsonl".to_string(),
            agent: Some("codex".to_string()),
            body: "DaoFocus 的任务实现要遵循 TDD、Bun 测试和 review 后再 merge 的项目工作流。".to_string(),
            evidence: "test".to_string(),
            redacted: false,
            created_at: "2026-05-01T00:00:00+00:00".to_string(),
        },
        ObservationRecord {
            id: "obs:dao:2".to_string(),
            source_kind: "test".to_string(),
            source_path: "dao.jsonl".to_string(),
            agent: Some("codex".to_string()),
            body: "再次强调 DaoFocus 开发要保留审查、测试、验证和协作边界，而不是沉淀灵石境界这类产品细节。".to_string(),
            evidence: "test".to_string(),
            redacted: false,
            created_at: "2026-05-01T00:01:00+00:00".to_string(),
        },
    ];
    let report = PipelineReport {
        observations_in: 2,
        stripped_kept: 2,
        truncated_count: 2,
        truncated_long_messages: 0,
        clusters_in: 2,
        induce_accepted: 0,
        induce_rejected: 2,
        induce_failed: 0,
        induce_calls: 1,
        induce_clusters_selected: 2,
        crystallize_accepted: 0,
        crystallize_rejected: 0,
        cards: Vec::new(),
        crystallize_rejects: Vec::new(),
        failures_preview: Vec::new(),
        layer_timings_ms: Default::default(),
        pipeline_version: 1,
    };

    let qa = score_pipeline_report(&report, &observations).expect("qa");

    assert!(
        qa.score < 85,
        "zero project-workflow recall should block review readiness"
    );
    assert!(
        qa.issues
            .iter()
            .any(|issue| issue.contains("expected project_workflow recall")),
        "issues should explain the missing project-workflow lane: {:#?}",
        qa.issues
    );
}

#[test]
fn seen_memory_signatures_are_written_as_jsonl() {
    let temp = tempfile::tempdir().expect("tempdir");
    let report = PipelineReport {
        observations_in: 0,
        stripped_kept: 0,
        truncated_count: 0,
        truncated_long_messages: 0,
        clusters_in: 0,
        induce_accepted: 1,
        induce_rejected: 0,
        induce_failed: 0,
        induce_calls: 1,
        induce_clusters_selected: 1,
        crystallize_accepted: 1,
        crystallize_rejected: 0,
        cards: vec![test_card(
            "c1",
            vec![EvidenceQuote {
                observation_id: "obs:1".to_string(),
                text: "真实历史会话回归验证".to_string(),
            }],
        )],
        crystallize_rejects: Vec::new(),
        failures_preview: Vec::new(),
        layer_timings_ms: Default::default(),
        pipeline_version: 1,
    };

    let written = append_seen_memory_signatures(temp.path(), &report, "reviewed").expect("seen");
    let path = temp
        .path()
        .join(".agent-kernel")
        .join("seen-memory-signatures.jsonl");
    let text = std::fs::read_to_string(path).expect("seen jsonl");

    assert_eq!(written, 1);
    assert!(text.contains("\"outcome\":\"reviewed\""));
    assert!(text.contains("\"source_observation_ids\":[\"obs:1\"]"));
}

fn test_card(cluster_id: &str, evidence_quotes: Vec<EvidenceQuote>) -> CrystallizedCard {
    CrystallizedCard {
        cluster_id: cluster_id.to_string(),
        recurrence: 2,
        title: "优先真实历史回归".to_string(),
        body: "当评估提炼质量时，必须用真实历史会话回归验证；目标是让规则接受真实数据检验。"
            .to_string(),
        brief: "用于真实历史回归验证提炼质量".to_string(),
        kind: "procedure".to_string(),
        scope: "global".to_string(),
        activation: "always-on".to_string(),
        tags: vec!["eval".to_string()],
        language: "zh-CN".to_string(),
        evidence_quotes,
        temporal_status: "stable".to_string(),
        confidence: 0.9,
    }
}

fn parse_prompt_field<'a>(prompt: &'a str, field: &str) -> Option<&'a str> {
    let prefix = format!("{field}: ");
    prompt
        .lines()
        .find_map(|line| line.strip_prefix(&prefix).map(str::trim))
}

fn parse_first_prompt_message(prompt: &str) -> Option<(String, String)> {
    let mut lines = prompt.lines();
    while let Some(line) = lines.next() {
        if !line.starts_with("--- [") || !line.contains(" obs=") {
            continue;
        }
        let observation_id = line
            .split(" obs=")
            .nth(1)?
            .split(" created=")
            .next()?
            .trim()
            .to_string();
        let mut body = String::new();
        for body_line in lines.by_ref() {
            if body_line.starts_with("--- [") {
                break;
            }
            if !body.is_empty() {
                body.push('\n');
            }
            body.push_str(body_line);
        }
        let quote = body.trim().chars().take(80).collect::<String>();
        if observation_id.is_empty() || quote.is_empty() {
            return None;
        }
        return Some((observation_id, quote));
    }
    None
}

fn json_escape(value: &str) -> String {
    let quoted = serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string());
    quoted
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or("")
        .to_string()
}
