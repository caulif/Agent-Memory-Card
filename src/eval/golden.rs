use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::extract::cluster::ClusterOptions;
use crate::extract::pipeline::{InduceMode, PipelineOptions, run_pipeline_with};
use crate::extract::{CardQualityFailure, quality_report_for_crystallized_card};
use crate::observation::ObservationRecord;
use crate::provider::{ProviderConfig, ProviderRequest};

use super::apply_eval_timeouts;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldenSetEvalReport {
    pub positive_total: usize,
    pub positive_hits: usize,
    pub positive_recall_percent: f32,
    pub negative_total: usize,
    pub negative_rejected: usize,
    pub negative_precision_percent: f32,
    pub one_off_false_positive_count: usize,
    pub one_off_false_positive_percent: f32,
    pub duplicate_cluster_risk_count: usize,
    pub duplicate_cluster_risk_percent: f32,
    pub evidence_valid_count: usize,
    pub evidence_valid_percent: f32,
    pub card_quality_passed_count: usize,
    pub card_quality_passed_percent: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_evidence_valid_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_evidence_valid_percent: Option<f32>,
    pub positive_cases: Vec<GoldenCaseEval>,
    pub negative_cases: Vec<GoldenCaseEval>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldenSetEvalRunRecord {
    pub report: GoldenSetEvalReport,
    pub provider: String,
    pub pipeline_version: u32,
    pub timestamp: String,
}

impl GoldenSetEvalReport {
    pub fn render_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str("# Golden Set Eval\n\n");
        out.push_str(&format!(
            "- positive recall: {:.0}% ({}/{})\n",
            self.positive_recall_percent, self.positive_hits, self.positive_total
        ));
        out.push_str(&format!(
            "- negative precision: {:.0}% ({}/{})\n",
            self.negative_precision_percent, self.negative_rejected, self.negative_total
        ));
        out.push_str(&format!(
            "- one-off false positive: {:.0}% ({}/{})\n",
            self.one_off_false_positive_percent,
            self.one_off_false_positive_count,
            self.negative_total
        ));
        out.push_str(&format!(
            "- duplicate cluster risk: {:.0}% ({}/{})\n",
            self.duplicate_cluster_risk_percent,
            self.duplicate_cluster_risk_count,
            self.positive_total
        ));
        out.push_str(&format!(
            "- evidence validity: {:.0}% ({}/{})\n",
            self.evidence_valid_percent, self.evidence_valid_count, self.positive_total
        ));
        out.push_str(&format!(
            "- card quality baseline: {:.0}% ({}/{})\n",
            self.card_quality_passed_percent, self.card_quality_passed_count, self.positive_total
        ));
        if let (Some(count), Some(percent)) = (
            self.provider_evidence_valid_count,
            self.provider_evidence_valid_percent,
        ) {
            out.push_str(&format!(
                "- provider evidence validity: {:.0}% ({}/{})\n",
                percent, count, self.positive_total
            ));
        }
        out.push_str("\n## Positive Cases\n\n");
        for case in &self.positive_cases {
            let provider_evidence = case
                .provider_evidence_valid
                .map(|valid| {
                    if valid {
                        ", provider=valid"
                    } else {
                        ", provider=weak"
                    }
                })
                .unwrap_or("");
            let card_quality = case
                .card_quality_passed
                .map(|passed| {
                    if passed {
                        ", card_quality=pass".to_string()
                    } else {
                        format!(
                            ", card_quality=fail({})",
                            format_failures(&case.card_quality_failures)
                        )
                    }
                })
                .unwrap_or_default();
            out.push_str(&format!(
                "- `{}`: stripped={}, clusters={}, evidence={}{}{} -> {}\n",
                case.id,
                case.stripped_count,
                case.cluster_count,
                if case.evidence_valid { "valid" } else { "weak" },
                provider_evidence,
                card_quality,
                if case.passed { "hit" } else { "miss" }
            ));
        }
        out.push_str("\n## Negative Cases\n\n");
        for case in &self.negative_cases {
            out.push_str(&format!(
                "- `{}`: stripped={}, clusters={} -> {}\n",
                case.id,
                case.stripped_count,
                case.cluster_count,
                if case.passed { "rejected" } else { "leaked" }
            ));
        }
        out
    }
}

impl GoldenSetEvalRunRecord {
    pub fn new(report: GoldenSetEvalReport, provider: String) -> Self {
        Self {
            report,
            provider,
            pipeline_version: 1,
            timestamp: Utc::now().to_rfc3339(),
        }
    }
}

fn format_failures(failures: &[CardQualityFailure]) -> String {
    if failures.is_empty() {
        return "none".to_string();
    }
    failures
        .iter()
        .map(|failure| format!("{failure:?}"))
        .collect::<Vec<_>>()
        .join(",")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldenCaseEval {
    pub id: String,
    pub stripped_count: usize,
    pub cluster_count: usize,
    pub evidence_valid: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card_quality_passed: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub card_quality_failures: Vec<CardQualityFailure>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_evidence_valid: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_evidence_failure_category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_failure_fixture: Option<GoldenFailureFixtureProposal>,
    pub passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldenFailureFixtureProposal {
    pub id: String,
    pub description: String,
    pub failure_category: String,
    pub user_messages: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct GoldenSet {
    positives: Vec<GoldenCase>,
    negatives: Vec<GoldenCase>,
}

#[derive(Debug, Deserialize)]
struct GoldenCase {
    id: String,
    user_messages: Vec<String>,
}

struct NoopInduceProvider;

impl crate::extract::induce::InduceProvider for NoopInduceProvider {
    fn call(&self, _: &ProviderRequest, _: usize) -> Result<String> {
        Ok(
            r#"{"reject": true, "reason": "golden eval only runs deterministic layers"}"#
                .to_string(),
        )
    }
}

struct EchoEvidenceInduceProvider;

impl crate::extract::induce::InduceProvider for EchoEvidenceInduceProvider {
    fn call(&self, request: &ProviderRequest, _: usize) -> Result<String> {
        let cluster_id = parse_prompt_field(&request.user_prompt, "cluster_id").unwrap_or("golden");
        let recurrence = parse_prompt_field(&request.user_prompt, "recurrence")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(1);
        if recurrence < 2 {
            return Ok(
                r#"{"reject": true, "reason": "singleton evidence is not durable enough"}"#
                    .to_string(),
            );
        }
        let (observation_id, quote) = parse_first_prompt_message(&request.user_prompt)
            .with_context(|| "parse golden induce prompt evidence")?;
        Ok(format!(
            r#"{{
  "cluster_id": "{}",
  "recurrence": {},
  "title": "黄金集证据回溯验证",
  "when": "从历史会话提炼长期记忆",
  "what": "必须引用原始 observation 中的字面证据",
  "why": "确保最终 Memory Card 可被审查和回溯",
  "boundary": "只用于验证证据回溯，不代表最终自然语言质量",
  "kind": "constraint",
  "scope": "global",
  "memory_tier": "cross_project_principle",
  "abstraction_level": "good",
  "support_level": "strong",
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

pub fn run_golden_set_eval(workspace_root: &Path) -> Result<GoldenSetEvalReport> {
    run_golden_set_eval_internal(workspace_root, None)
}

pub fn write_latest_golden_set_eval_run(
    workspace_root: &Path,
    run: &GoldenSetEvalRunRecord,
) -> Result<()> {
    let root = crate::fsutil::normalize_project_root(workspace_root)?;
    let dir = crate::config::kernel_dir(&root).join("evals");
    fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    let path = dir.join("latest-golden.json");
    let raw = serde_json::to_string_pretty(run)?;
    fs::write(&path, raw).with_context(|| format!("write {}", path.display()))
}

pub fn write_provider_evidence_failure_fixtures(
    workspace_root: &Path,
    report: &GoldenSetEvalReport,
) -> Result<usize> {
    let fixtures = report
        .positive_cases
        .iter()
        .filter_map(|case| case.provider_failure_fixture.clone())
        .collect::<Vec<_>>();
    if fixtures.is_empty() {
        return Ok(0);
    }
    let root = crate::fsutil::normalize_project_root(workspace_root)?;
    let dir = crate::config::kernel_dir(&root).join("evals");
    fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    let path = dir.join("provider-evidence-failure-fixtures.yml");
    let raw = serde_yaml::to_string(&fixtures)?;
    fs::write(&path, raw).with_context(|| format!("write {}", path.display()))?;
    Ok(fixtures.len())
}

pub fn load_latest_golden_set_eval_run(
    workspace_root: &Path,
) -> Result<Option<GoldenSetEvalRunRecord>> {
    let root = crate::fsutil::normalize_project_root(workspace_root)?;
    let path = crate::config::kernel_dir(&root)
        .join("evals")
        .join("latest-golden.json");
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&raw)
        .with_context(|| format!("parse {}", path.display()))
        .map(Some)
}

pub fn run_golden_set_eval_with_provider(
    workspace_root: &Path,
    provider_override: Option<&str>,
    timeout_secs: u64,
) -> Result<GoldenSetEvalReport> {
    let mut cfg = crate::provider::load_or_default_provider_config(workspace_root)
        .unwrap_or_else(|_| ProviderConfig::default());
    if let Some(provider) = provider_override {
        cfg.extraction_provider = provider.to_string();
        cfg.role_providers.extract = Some(provider.to_string());
    }
    apply_eval_timeouts(&mut cfg, timeout_secs);
    let provider = crate::extract::induce::DefaultInduceProvider { cfg: &cfg };
    run_golden_set_eval_internal(workspace_root, Some(&provider))
}

pub fn run_golden_set_eval_with_induce_provider(
    workspace_root: &Path,
    provider: &dyn crate::extract::induce::InduceProvider,
) -> Result<GoldenSetEvalReport> {
    run_golden_set_eval_internal(workspace_root, Some(provider))
}

fn run_golden_set_eval_internal(
    workspace_root: &Path,
    provider: Option<&dyn crate::extract::induce::InduceProvider>,
) -> Result<GoldenSetEvalReport> {
    let golden = load_golden_set(workspace_root)?;
    let mut positive_cases = Vec::new();
    let mut negative_cases = Vec::new();

    for case in &golden.positives {
        let eval = eval_golden_case(case, true, provider)?;
        positive_cases.push(eval);
    }
    for case in &golden.negatives {
        let eval = eval_golden_case(case, false, None)?;
        negative_cases.push(eval);
    }

    let positive_hits = positive_cases.iter().filter(|case| case.passed).count();
    let negative_rejected = negative_cases.iter().filter(|case| case.passed).count();
    let positive_total = positive_cases.len();
    let negative_total = negative_cases.len();
    let one_off_false_positive_count = negative_total.saturating_sub(negative_rejected);
    let duplicate_cluster_risk_count = positive_cases
        .iter()
        .filter(|case| case.cluster_count > 1)
        .count();
    let evidence_valid_count = positive_cases
        .iter()
        .filter(|case| case.evidence_valid)
        .count();
    let card_quality_passed_count = positive_cases
        .iter()
        .filter(|case| case.card_quality_passed == Some(true))
        .count();
    let provider_evidence_valid_count = provider.map(|_| {
        positive_cases
            .iter()
            .filter(|case| case.provider_evidence_valid == Some(true))
            .count()
    });

    Ok(GoldenSetEvalReport {
        positive_total,
        positive_hits,
        positive_recall_percent: percent(positive_hits, positive_total),
        negative_total,
        negative_rejected,
        negative_precision_percent: percent(negative_rejected, negative_total),
        one_off_false_positive_count,
        one_off_false_positive_percent: percent(one_off_false_positive_count, negative_total),
        duplicate_cluster_risk_count,
        duplicate_cluster_risk_percent: percent(duplicate_cluster_risk_count, positive_total),
        evidence_valid_count,
        evidence_valid_percent: percent(evidence_valid_count, positive_total),
        card_quality_passed_count,
        card_quality_passed_percent: percent(card_quality_passed_count, positive_total),
        provider_evidence_valid_count,
        provider_evidence_valid_percent: provider_evidence_valid_count
            .map(|count| percent(count, positive_total)),
        positive_cases,
        negative_cases,
    })
}

fn load_golden_set(workspace_root: &Path) -> Result<GoldenSet> {
    let path = workspace_root
        .join("tests")
        .join("golden")
        .join("golden_set.yml");
    let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    serde_yaml::from_str(&text).with_context(|| format!("parse {}", path.display()))
}

fn eval_golden_case(
    case: &GoldenCase,
    positive: bool,
    provider: Option<&dyn crate::extract::induce::InduceProvider>,
) -> Result<GoldenCaseEval> {
    let observations = case
        .user_messages
        .iter()
        .enumerate()
        .map(|(idx, body)| ObservationRecord {
            id: format!("obs:golden:{}:{idx}", case.id),
            source_kind: "claude-code-session".to_string(),
            source_path: format!("tests/golden/{}.jsonl", case.id),
            agent: Some("claude-code".to_string()),
            body: body.clone(),
            evidence: "golden".to_string(),
            redacted: false,
            created_at: format!("2026-05-01T00:{idx:02}:00+00:00"),
        })
        .collect::<Vec<_>>();
    let cluster_options = ClusterOptions {
        force_jaccard: true,
        keep_singletons: true,
        ..ClusterOptions::default()
    };
    let options = PipelineOptions {
        cluster: cluster_options.clone(),
        induce: Default::default(),
        skip_induce: true,
    };
    let report = run_pipeline_with(&observations, &NoopInduceProvider, &options)?;
    let evidence_valid = if positive {
        final_quote_evidence_valid(&observations, cluster_options.clone())?
    } else {
        true
    };
    let provider_evidence_check = if positive {
        match provider {
            Some(provider) => Some(provider_quote_evidence_check(
                &observations,
                case,
                cluster_options.clone(),
                provider,
            )?),
            None => None,
        }
    } else {
        None
    };
    let card_quality_reports = if positive {
        card_quality_reports_for_case(&observations, cluster_options.clone(), provider)?
    } else {
        Vec::new()
    };
    let card_quality_passed = if positive {
        Some(
            !card_quality_reports.is_empty()
                && card_quality_reports.iter().all(|report| report.passed),
        )
    } else {
        None
    };
    let card_quality_failures = card_quality_reports
        .iter()
        .flat_map(|report| report.failures.iter().cloned())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let passed = if positive {
        report.stripped_kept >= 2 && report.clusters_in < report.stripped_kept
    } else {
        report.stripped_kept == 0 || report.clusters_in >= report.stripped_kept
    };
    Ok(GoldenCaseEval {
        id: case.id.clone(),
        stripped_count: report.stripped_kept,
        cluster_count: report.clusters_in,
        evidence_valid,
        card_quality_passed,
        card_quality_failures,
        provider_evidence_valid: provider_evidence_check.as_ref().map(|check| check.valid),
        provider_evidence_failure_category: provider_evidence_check
            .as_ref()
            .and_then(|check| check.failure_category.clone()),
        provider_failure_fixture: provider_evidence_check.and_then(|check| check.fixture_proposal),
        passed,
    })
}

fn final_quote_evidence_valid(
    observations: &[ObservationRecord],
    cluster_options: ClusterOptions,
) -> Result<bool> {
    let options = PipelineOptions {
        cluster: cluster_options,
        induce: InduceMode::PerCluster,
        skip_induce: false,
    };
    let report = run_pipeline_with(observations, &EchoEvidenceInduceProvider, &options)?;
    if report.cards.is_empty() {
        return Ok(false);
    }
    Ok(report.cards.iter().all(|card| {
        !card.evidence_quotes.is_empty()
            && card.evidence_quotes.iter().all(|quote| {
                observations.iter().any(|observation| {
                    observation.id == quote.observation_id
                        && !quote.text.trim().is_empty()
                        && observation.body.contains(quote.text.trim())
                })
            })
    }))
}

fn card_quality_reports_for_case(
    observations: &[ObservationRecord],
    cluster_options: ClusterOptions,
    provider: Option<&dyn crate::extract::induce::InduceProvider>,
) -> Result<Vec<crate::extract::CardQualityReport>> {
    let options = PipelineOptions {
        cluster: cluster_options,
        induce: InduceMode::PerCluster,
        skip_induce: false,
    };
    let noop_provider = EchoEvidenceInduceProvider;
    let provider = provider.unwrap_or(&noop_provider);
    let report = run_pipeline_with(observations, provider, &options)?;
    Ok(report
        .cards
        .iter()
        .map(quality_report_for_crystallized_card)
        .collect())
}

struct ProviderEvidenceCheck {
    valid: bool,
    failure_category: Option<String>,
    fixture_proposal: Option<GoldenFailureFixtureProposal>,
}

fn provider_quote_evidence_check(
    observations: &[ObservationRecord],
    case: &GoldenCase,
    cluster_options: ClusterOptions,
    provider: &dyn crate::extract::induce::InduceProvider,
) -> Result<ProviderEvidenceCheck> {
    let options = PipelineOptions {
        cluster: cluster_options,
        induce: InduceMode::PerCluster,
        skip_induce: false,
    };
    let report = run_pipeline_with(observations, provider, &options)?;
    if report.cards.is_empty() {
        let category = provider_failure_category_from_preview(&report.failures_preview)
            .unwrap_or("provider-no-card");
        return Ok(provider_evidence_failure(
            case,
            category,
            "Provider did not produce a Memory Card for a positive golden case.",
        ));
    }
    for card in &report.cards {
        if card.evidence_quotes.is_empty() {
            return Ok(provider_evidence_failure(
                case,
                "provider-missing-evidence",
                "Provider produced a Memory Card without evidence quotes.",
            ));
        }
        for quote in &card.evidence_quotes {
            let grounded = observations.iter().any(|observation| {
                observation.id == quote.observation_id
                    && !quote.text.trim().is_empty()
                    && observation.body.contains(quote.text.trim())
            });
            if !grounded {
                return Ok(provider_evidence_failure(
                    case,
                    "provider-untraceable-evidence",
                    "Provider evidence quote could not be traced to the source observations.",
                ));
            }
        }
    }
    Ok(ProviderEvidenceCheck {
        valid: true,
        failure_category: None,
        fixture_proposal: None,
    })
}

fn provider_failure_category_from_preview(preview: &[String]) -> Option<&'static str> {
    preview.iter().find_map(|line| {
        let lower = line.to_lowercase();
        if lower.contains("evidence")
            && (lower.contains("ground")
                || lower.contains("trace")
                || lower.contains("not in source")
                || lower.contains("observation"))
        {
            Some("provider-untraceable-evidence")
        } else if lower.contains("evidence") {
            Some("provider-missing-evidence")
        } else {
            None
        }
    })
}

fn provider_evidence_failure(
    case: &GoldenCase,
    category: &str,
    description: &str,
) -> ProviderEvidenceCheck {
    ProviderEvidenceCheck {
        valid: false,
        failure_category: Some(category.to_string()),
        fixture_proposal: Some(GoldenFailureFixtureProposal {
            id: format!("provider-evidence-{}", case.id),
            description: description.to_string(),
            failure_category: category.to_string(),
            user_messages: case.user_messages.clone(),
        }),
    }
}

fn percent(part: usize, whole: usize) -> f32 {
    if whole == 0 {
        0.0
    } else {
        (part as f32) * 100.0 / (whole as f32)
    }
}
