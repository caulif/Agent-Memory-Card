use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::extract;
use crate::extract::cluster::ClusterOptions;
use crate::extract::pipeline::{PipelineOptions, PipelineReport, run_pipeline};
use crate::observation::{ObservationRecord, load_observations};
use crate::project_registry;
use crate::provider::{
    Provider, ProviderConfig, ProviderRequest, ProviderRole, call_provider_for_role,
};

mod golden;
mod qa;
pub use golden::{
    GoldenCaseEval, GoldenSetEvalReport, run_golden_set_eval,
    run_golden_set_eval_with_induce_provider, run_golden_set_eval_with_provider,
};
pub use qa::{RunQAReport, append_seen_memory_signatures, score_pipeline_report};

const REFERENCE_SYNTHESIS_PROMPT: &str = include_str!("../prompts/reference_synthesis.md");
const REFERENCE_QUALITY_PROMPT: &str = include_str!("../prompts/reference_quality_judge.md");
const PAIRWISE_JUDGE_PROMPT: &str = include_str!("../prompts/pairwise_card_judge.md");
const EVAL_MATERIAL_MAX_CHARS: usize = 8_000;
const EVAL_CLI_TIMEOUT_SECS: u64 = 20;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceCard {
    pub title: String,
    pub body: String,
    pub brief: String,
    pub kind: String,
    pub scope: String,
    #[serde(default)]
    pub source_observations: Vec<String>,
    #[serde(default)]
    pub evidence_quotes: Vec<String>,
    #[serde(default)]
    pub confidence: Option<f32>,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalCard {
    pub title: String,
    pub body: String,
    pub brief: String,
    pub kind: String,
    pub scope: String,
    pub evidence: String,
    #[serde(default)]
    pub confidence: Option<f32>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairwiseJudgeResult {
    pub winner: String,
    pub score_a: f32,
    pub score_b: f32,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEvalReport {
    pub name: String,
    pub path: PathBuf,
    pub observation_count: usize,
    pub reference_cards: Vec<ReferenceCard>,
    pub pipeline_cards: Vec<EvalCard>,
    pub baseline_cards: Vec<EvalCard>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pairwise_judge: Option<PairwiseJudgeResult>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
    pub output_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EvalSummary {
    pub project_count: usize,
    pub reference_cards: usize,
    pub pipeline_cards: usize,
    pub baseline_cards: usize,
    pub pipeline_wins: usize,
    pub baseline_wins: usize,
    pub ties: usize,
    pub judged_projects: usize,
    pub pipeline_score_avg: f32,
    pub baseline_score_avg: f32,
    pub errors: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealProjectEvalReport {
    pub run_dir: PathBuf,
    pub projects: Vec<ProjectEvalReport>,
    pub summary: EvalSummary,
}

pub trait EvalProvider {
    fn call(
        &self,
        role: ProviderRole,
        request: &ProviderRequest,
        max_tokens: usize,
    ) -> Result<String>;
}

pub struct DefaultEvalProvider<'a> {
    pub cfg: &'a ProviderConfig,
}

impl EvalProvider for DefaultEvalProvider<'_> {
    fn call(
        &self,
        role: ProviderRole,
        request: &ProviderRequest,
        max_tokens: usize,
    ) -> Result<String> {
        call_provider_for_role(self.cfg, role, request, max_tokens)
    }
}

pub fn format_pairwise_judge_prompt(
    reference_cards: &[ReferenceCard],
    set_a: &[EvalCard],
    set_b: &[EvalCard],
) -> String {
    let reference_json =
        serde_json::to_string_pretty(reference_cards).unwrap_or_else(|_| "[]".to_string());
    let a_json =
        serde_json::to_string_pretty(&blind_eval_cards(set_a)).unwrap_or_else(|_| "[]".to_string());
    let b_json =
        serde_json::to_string_pretty(&blind_eval_cards(set_b)).unwrap_or_else(|_| "[]".to_string());

    format!(
        "{PAIRWISE_JUDGE_PROMPT}\n\nReference cards:\n{reference_json}\n\nSet A:\n{a_json}\n\nSet B:\n{b_json}\n"
    )
}

#[derive(Debug, Clone, Serialize)]
struct BlindEvalCard<'a> {
    title: &'a str,
    body: &'a str,
    brief: &'a str,
    kind: &'a str,
    scope: &'a str,
    evidence: &'a str,
    confidence: Option<f32>,
}

fn blind_eval_cards(cards: &[EvalCard]) -> Vec<BlindEvalCard<'_>> {
    cards
        .iter()
        .map(|card| BlindEvalCard {
            title: &card.title,
            body: &card.body,
            brief: &card.brief,
            kind: &card.kind,
            scope: &card.scope,
            evidence: &card.evidence,
            confidence: card.confidence,
        })
        .collect()
}

pub fn run_real_project_eval(
    workspace_root: &Path,
    project_paths: Vec<PathBuf>,
    max_projects: usize,
    json_only: bool,
) -> Result<RealProjectEvalReport> {
    run_real_project_eval_for_paths(workspace_root, project_paths, max_projects, json_only)
}

pub fn run_real_project_eval_with_provider(
    workspace_root: &Path,
    project_paths: Vec<PathBuf>,
    max_projects: usize,
    json_only: bool,
    provider_override: Option<String>,
    timeout_secs: u64,
) -> Result<RealProjectEvalReport> {
    run_real_project_eval_internal(
        workspace_root,
        project_paths,
        max_projects,
        json_only,
        provider_override.as_deref(),
        timeout_secs,
    )
}

pub fn run_real_project_eval_for_paths(
    workspace_root: &Path,
    project_paths: Vec<PathBuf>,
    max_projects: usize,
    json_only: bool,
) -> Result<RealProjectEvalReport> {
    run_real_project_eval_internal(
        workspace_root,
        project_paths,
        max_projects,
        json_only,
        None,
        EVAL_CLI_TIMEOUT_SECS,
    )
}

fn run_real_project_eval_internal(
    workspace_root: &Path,
    project_paths: Vec<PathBuf>,
    max_projects: usize,
    json_only: bool,
    provider_override: Option<&str>,
    timeout_secs: u64,
) -> Result<RealProjectEvalReport> {
    let projects = resolve_eval_projects(workspace_root, project_paths, max_projects)?;
    let run_dir = create_run_dir(workspace_root)?;
    let mut reports = Vec::new();

    for project_path in projects {
        reports.push(run_one_project_eval(
            workspace_root,
            &project_path,
            &run_dir,
            json_only,
            provider_override,
            timeout_secs,
        )?);
    }

    let summary = summarize_reports(&reports);
    write_run_summary(&run_dir, &reports, &summary)?;

    Ok(RealProjectEvalReport {
        run_dir,
        projects: reports,
        summary,
    })
}

fn resolve_eval_projects(
    workspace_root: &Path,
    explicit_paths: Vec<PathBuf>,
    max_projects: usize,
) -> Result<Vec<PathBuf>> {
    let max_projects = max_projects.max(1);
    if !explicit_paths.is_empty() {
        let mut seen = std::collections::BTreeSet::new();
        let mut projects = Vec::new();
        for path in explicit_paths {
            let normalized = crate::fsutil::normalize_project_root(&path)?;
            let key = normalized.to_string_lossy().to_string();
            if seen.insert(key) {
                projects.push(normalized);
            }
            if projects.len() >= max_projects {
                break;
            }
        }
        return Ok(projects);
    }

    let home = crate::fsutil::home_dir().unwrap_or_else(|| workspace_root.to_path_buf());
    let registry = project_registry::load_registry_with_agent_projects(&home)?;
    let mut scored: Vec<(usize, PathBuf)> = Vec::new();
    for project in registry.projects {
        let path = PathBuf::from(&project.path);
        if !path.exists() {
            continue;
        }
        let observation_count = load_observations(&path).map(|obs| obs.len()).unwrap_or(0);
        if observation_count == 0 {
            continue;
        }
        scored.push((observation_count, path));
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    Ok(scored
        .into_iter()
        .take(max_projects)
        .map(|(_, path)| path)
        .collect())
}

fn run_one_project_eval(
    workspace_root: &Path,
    project_path: &Path,
    run_dir: &Path,
    json_only: bool,
    provider_override: Option<&str>,
    timeout_secs: u64,
) -> Result<ProjectEvalReport> {
    let mut cfg = crate::provider::load_or_default_provider_config(project_path)
        .unwrap_or_else(|_| ProviderConfig::default());
    if let Some(provider) = provider_override {
        cfg.extraction_provider = provider.to_string();
        cfg.role_providers.extract = Some(provider.to_string());
        cfg.role_providers.judge = Some(provider.to_string());
    }
    apply_eval_timeouts(&mut cfg, timeout_secs);
    let provider = DefaultEvalProvider { cfg: &cfg };
    let observations = load_observations(project_path)
        .with_context(|| format!("load observations for {}", project_path.display()))?;
    let project_name = project_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("project")
        .to_string();
    let output_dir = run_dir.join(sanitize_name(&project_name));
    fs::create_dir_all(&output_dir)?;

    let mut errors = Vec::new();
    let material = render_eval_material(&observations);

    let reference_candidates = match synthesize_reference_cards(&provider, &material, &observations)
    {
        Ok(cards) => cards,
        Err(error) => {
            errors.push(format!("reference synthesis: {error}"));
            Vec::new()
        }
    };
    write_json(
        &output_dir.join("reference-candidates.json"),
        &reference_candidates,
    )?;

    let reference_cards = if reference_candidates.is_empty() {
        Vec::new()
    } else {
        match judge_reference_cards(&provider, &material, &reference_candidates) {
            Ok(cards) => cards,
            Err(error) => {
                errors.push(format!("reference judge: {error}"));
                Vec::new()
            }
        }
    };
    write_json(
        &output_dir.join("reference-approved.json"),
        &reference_cards,
    )?;

    let pipeline_cards = if reference_cards.is_empty() {
        errors.push("pipeline skipped: no approved reference cards to compare against".to_string());
        Vec::new()
    } else {
        match run_pipeline_for_eval(project_path, &cfg) {
            Ok(report) => {
                write_json(&output_dir.join("pipeline.json"), &report)?;
                report
                    .cards
                    .into_iter()
                    .map(eval_card_from_pipeline)
                    .collect()
            }
            Err(error) => {
                errors.push(format!("pipeline: {error}"));
                Vec::new()
            }
        }
    };

    let baseline_cards = match run_baseline_eval(project_path, &material, &cfg) {
        Ok(cards) => {
            write_json(&output_dir.join("baseline.json"), &cards)?;
            cards
        }
        Err(error) => {
            errors.push(format!("baseline: {error}"));
            Vec::new()
        }
    };

    let pairwise_judge = if !reference_cards.is_empty()
        && (!pipeline_cards.is_empty() || !baseline_cards.is_empty())
    {
        match judge_pairwise(
            &provider,
            &reference_cards,
            &pipeline_cards,
            &baseline_cards,
        ) {
            Ok(result) => {
                write_json(&output_dir.join("pairwise.json"), &result)?;
                Some(result)
            }
            Err(error) => {
                errors.push(format!("pairwise judge: {error}"));
                None
            }
        }
    } else {
        None
    };

    write_summary_page(
        &output_dir,
        workspace_root,
        project_path,
        observations.len(),
        &reference_candidates,
        &reference_cards,
        &pipeline_cards,
        &baseline_cards,
        pairwise_judge.as_ref(),
        &errors,
        json_only,
    )?;

    Ok(ProjectEvalReport {
        name: project_name,
        path: project_path.to_path_buf(),
        observation_count: observations.len(),
        reference_cards,
        pipeline_cards,
        baseline_cards,
        pairwise_judge,
        errors,
        output_dir,
    })
}

fn run_pipeline_for_eval(project_path: &Path, cfg: &ProviderConfig) -> Result<PipelineReport> {
    let observations = load_observations(project_path)?;
    let options = pipeline_options_for_eval();
    run_pipeline(&observations, cfg, &options)
}

fn pipeline_options_for_eval() -> PipelineOptions {
    PipelineOptions {
        cluster: ClusterOptions {
            keep_singletons: true,
            ..ClusterOptions::default()
        },
        induce: Default::default(),
        skip_induce: false,
    }
}

fn run_baseline_eval(
    project_path: &Path,
    material: &str,
    _cfg: &ProviderConfig,
) -> Result<Vec<EvalCard>> {
    let report = extract::extract_text_to_drafts(
        project_path,
        material,
        Vec::new(),
        "eval-reference-material",
        Some("local".to_string()),
        true,
    )?;
    Ok(report
        .candidates
        .into_iter()
        .map(|candidate| EvalCard {
            title: candidate.title,
            body: candidate.body.clone(),
            brief: candidate.body.chars().take(60).collect(),
            kind: candidate.kind,
            scope: candidate.scope,
            evidence: candidate.evidence,
            confidence: candidate.confidence,
            source: "baseline".to_string(),
        })
        .collect())
}

fn synthesize_reference_cards(
    provider: &dyn EvalProvider,
    material: &str,
    observations: &[ObservationRecord],
) -> Result<Vec<ReferenceCard>> {
    let request = ProviderRequest {
        system_prompt: REFERENCE_SYNTHESIS_PROMPT.to_string(),
        user_prompt: material.to_string(),
        json_schema: None,
    };
    let raw = provider.call(ProviderRole::Extract, &request, 1800)?;
    let candidates = parse_cards_array::<ReferenceCard>(&raw)?;
    Ok(candidates
        .into_iter()
        .map(|mut card| {
            if card.source_observations.is_empty() && !card.evidence_quotes.is_empty() {
                card.source_observations =
                    backfill_source_observations(&card.evidence_quotes, observations);
            }
            card
        })
        .collect())
}

fn judge_reference_cards(
    provider: &dyn EvalProvider,
    material: &str,
    cards: &[ReferenceCard],
) -> Result<Vec<ReferenceCard>> {
    let request = ProviderRequest {
        system_prompt: REFERENCE_QUALITY_PROMPT.to_string(),
        user_prompt: format!(
            "Material:\n{}\n\nCandidates:\n{}",
            material,
            serde_json::to_string_pretty(cards)?
        ),
        json_schema: None,
    };
    let raw = provider.call(ProviderRole::Judge, &request, 1200)?;
    parse_cards_array::<ReferenceCard>(&raw)
}

fn judge_pairwise(
    provider: &dyn EvalProvider,
    reference_cards: &[ReferenceCard],
    set_a: &[EvalCard],
    set_b: &[EvalCard],
) -> Result<PairwiseJudgeResult> {
    let request = ProviderRequest {
        system_prompt: PAIRWISE_JUDGE_PROMPT.to_string(),
        user_prompt: format_pairwise_judge_prompt(reference_cards, set_a, set_b),
        json_schema: None,
    };
    let raw = provider.call(ProviderRole::Judge, &request, 1200)?;
    let result = parse_object::<PairwiseJudgeResult>(&raw)?;
    validate_pairwise_result(&result)?;
    Ok(result)
}

fn validate_pairwise_result(result: &PairwiseJudgeResult) -> Result<()> {
    if !matches!(result.winner.as_str(), "A" | "B" | "tie") {
        return Err(anyhow!("pairwise winner must be A, B, or tie"));
    }
    if !(0.0..=1.0).contains(&result.score_a) || !(0.0..=1.0).contains(&result.score_b) {
        return Err(anyhow!("pairwise scores must be in 0.0..=1.0"));
    }
    Ok(())
}

fn eval_card_from_pipeline(card: crate::extract::crystallize::CrystallizedCard) -> EvalCard {
    EvalCard {
        title: card.title,
        body: card.body,
        brief: card.brief,
        kind: card.kind,
        scope: card.scope,
        evidence: card
            .evidence_quotes
            .into_iter()
            .map(|quote| format!("{}: {}", quote.observation_id, quote.text))
            .collect::<Vec<_>>()
            .join("\n"),
        confidence: Some(card.confidence),
        source: "pipeline".to_string(),
    }
}

fn parse_cards_array<T>(raw: &str) -> Result<Vec<T>>
where
    T: serde::de::DeserializeOwned,
{
    let trimmed = strip_json_fence(raw);
    if let Ok(items) = serde_json::from_str::<Vec<T>>(trimmed) {
        return Ok(items);
    }
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
        if let Some(cards) = value.get("cards") {
            return Ok(serde_json::from_value(cards.clone())?);
        }
    }
    let Some(start) = trimmed.find('[') else {
        return Err(anyhow!("response did not contain a JSON array"));
    };
    let Some(end) = trimmed.rfind(']') else {
        return Err(anyhow!("response did not contain a complete JSON array"));
    };
    Ok(serde_json::from_str(&trimmed[start..=end])?)
}

fn parse_object<T>(raw: &str) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
    let trimmed = strip_json_fence(raw);
    if let Ok(value) = serde_json::from_str::<T>(trimmed) {
        return Ok(value);
    }
    let Some(start) = trimmed.find('{') else {
        return Err(anyhow!("response did not contain a JSON object"));
    };
    let Some(end) = trimmed.rfind('}') else {
        return Err(anyhow!("response did not contain a complete JSON object"));
    };
    Ok(serde_json::from_str(&trimmed[start..=end])?)
}

fn strip_json_fence(raw: &str) -> &str {
    let trimmed = raw.trim();
    if let Some(start) = trimmed.find("```json") {
        let body = &trimmed[start + 7..];
        if let Some(end) = body.rfind("```") {
            return body[..end].trim();
        }
    }
    if let Some(start) = trimmed.find("```") {
        let body = &trimmed[start + 3..];
        if let Some(end) = body.rfind("```") {
            return body[..end].trim();
        }
    }
    trimmed
}

fn render_observation_material(observations: &[ObservationRecord]) -> String {
    let mut out = String::new();
    for observation in observations {
        if out.chars().count() >= EVAL_MATERIAL_MAX_CHARS {
            break;
        }
        out.push_str(&format!(
            "--- obs:{} ---\nsource: {}\nagent: {}\nevidence: {}\ntext:\n{}\n\n",
            observation.id,
            observation.source_kind,
            observation.agent.as_deref().unwrap_or("unknown"),
            observation.evidence,
            truncate_text(&observation.body, 1200)
        ));
        if out.chars().count() > EVAL_MATERIAL_MAX_CHARS {
            out = out.chars().take(EVAL_MATERIAL_MAX_CHARS).collect();
            out.push_str("\n[... eval material truncated ...]\n");
            break;
        }
    }
    out
}

fn render_eval_material(observations: &[ObservationRecord]) -> String {
    let high_signal = crate::observation::chunked::prefiltered_synthesis_material(
        observations,
        EVAL_MATERIAL_MAX_CHARS,
    );
    if high_signal.trim().is_empty() {
        render_observation_material(observations)
    } else {
        high_signal
    }
}

fn truncate_text(text: &str, max_chars: usize) -> String {
    let count = text.chars().count();
    if count <= max_chars {
        return text.to_string();
    }
    let head: String = text.chars().take(max_chars / 2).collect();
    let tail: String = text.chars().skip(count - max_chars / 2).collect();
    format!(
        "{head}\n[... omitted {} chars ...]\n{tail}",
        count - max_chars
    )
}

fn backfill_source_observations(
    quotes: &[String],
    observations: &[ObservationRecord],
) -> Vec<String> {
    let mut ids = Vec::new();
    for quote in quotes {
        let needle = quote.trim();
        if needle.is_empty() {
            continue;
        }
        for observation in observations {
            if observation.body.contains(needle) && !ids.contains(&observation.id) {
                ids.push(observation.id.clone());
            }
        }
    }
    ids
}

fn create_run_dir(workspace_root: &Path) -> Result<PathBuf> {
    let dir = workspace_root
        .join("docs")
        .join("runs")
        .join(format!("eval-{}", Utc::now().format("%Y%m%dT%H%M%S")));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    fs::write(path, serde_json::to_string_pretty(value)?)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn write_summary_page(
    output_dir: &Path,
    workspace_root: &Path,
    project_path: &Path,
    observation_count: usize,
    reference_candidates: &[ReferenceCard],
    reference_cards: &[ReferenceCard],
    pipeline_cards: &[EvalCard],
    baseline_cards: &[EvalCard],
    pairwise_judge: Option<&PairwiseJudgeResult>,
    errors: &[String],
    json_only: bool,
) -> Result<()> {
    let mut text = String::new();
    text.push_str("# Real Project Eval\n\n");
    text.push_str(&format!("Workspace: {}\n\n", workspace_root.display()));
    text.push_str(&format!("Project: {}\n\n", project_path.display()));
    text.push_str(&format!("Observations: {}\n\n", observation_count));
    text.push_str(&format!(
        "Reference candidates: {}\n",
        reference_candidates.len()
    ));
    text.push_str(&format!(
        "Approved reference cards: {}\n",
        reference_cards.len()
    ));
    text.push_str(&format!("Pipeline cards: {}\n", pipeline_cards.len()));
    text.push_str(&format!("Baseline cards: {}\n", baseline_cards.len()));
    if let Some(judge) = pairwise_judge {
        text.push_str(&format!(
            "Pairwise winner: {} (A {:.2}, B {:.2})\n",
            judge.winner, judge.score_a, judge.score_b
        ));
    }
    if !errors.is_empty() {
        text.push_str("\nErrors:\n");
        for error in errors {
            text.push_str(&format!("- {}\n", error));
        }
    }
    if json_only {
        text.push_str("\nJSON-only mode was used for stdout; artifacts are still written.\n");
    }
    fs::write(output_dir.join("summary.md"), text)?;
    Ok(())
}

fn write_run_summary(
    run_dir: &Path,
    reports: &[ProjectEvalReport],
    summary: &EvalSummary,
) -> Result<()> {
    let mut text = String::new();
    text.push_str("# Eval Summary\n\n");
    text.push_str(&format!("Projects: {}\n", summary.project_count));
    text.push_str(&format!("Reference cards: {}\n", summary.reference_cards));
    text.push_str(&format!("Pipeline cards: {}\n", summary.pipeline_cards));
    text.push_str(&format!("Baseline cards: {}\n", summary.baseline_cards));
    text.push_str(&format!("Pipeline wins: {}\n", summary.pipeline_wins));
    text.push_str(&format!("Baseline wins: {}\n", summary.baseline_wins));
    text.push_str(&format!("Ties: {}\n", summary.ties));
    text.push_str(&format!("Judged projects: {}\n", summary.judged_projects));
    text.push_str(&format!(
        "Pipeline score avg: {:.2}\n",
        summary.pipeline_score_avg
    ));
    text.push_str(&format!(
        "Baseline score avg: {:.2}\n",
        summary.baseline_score_avg
    ));
    text.push_str(&format!("Errors: {}\n\n", summary.errors));
    for report in reports {
        text.push_str(&format!(
            "- {}: ref {} / pipeline {} / baseline {} / errors {}\n",
            report.name,
            report.reference_cards.len(),
            report.pipeline_cards.len(),
            report.baseline_cards.len(),
            report.errors.len()
        ));
    }
    fs::write(run_dir.join("summary.md"), text)?;
    fs::write(
        run_dir.join("report.json"),
        serde_json::to_string_pretty(&RealProjectEvalReport {
            run_dir: run_dir.to_path_buf(),
            projects: reports.to_vec(),
            summary: summary.clone(),
        })?,
    )?;
    Ok(())
}

fn summarize_reports(reports: &[ProjectEvalReport]) -> EvalSummary {
    let mut summary = EvalSummary {
        project_count: reports.len(),
        ..EvalSummary::default()
    };
    let mut pipeline_score_sum = 0.0f32;
    let mut baseline_score_sum = 0.0f32;
    for report in reports {
        summary.reference_cards += report.reference_cards.len();
        summary.pipeline_cards += report.pipeline_cards.len();
        summary.baseline_cards += report.baseline_cards.len();
        summary.errors += report.errors.len();
        if let Some(judge) = &report.pairwise_judge {
            summary.judged_projects += 1;
            pipeline_score_sum += judge.score_a;
            baseline_score_sum += judge.score_b;
            match judge.winner.as_str() {
                "A" => summary.pipeline_wins += 1,
                "B" => summary.baseline_wins += 1,
                _ => summary.ties += 1,
            }
        }
    }
    if summary.judged_projects > 0 {
        let judged = summary.judged_projects as f32;
        summary.pipeline_score_avg = pipeline_score_sum / judged;
        summary.baseline_score_avg = baseline_score_sum / judged;
    }
    summary
}

fn sanitize_name(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "project".to_string()
    } else {
        out
    }
}

fn apply_eval_timeouts(cfg: &mut ProviderConfig, requested_timeout_secs: u64) {
    let requested_timeout_secs = requested_timeout_secs.max(1);
    for provider in cfg.providers.values_mut() {
        match provider {
            Provider::ClaudeCli { timeout_secs, .. } | Provider::CodexCli { timeout_secs, .. } => {
                *timeout_secs = requested_timeout_secs;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests;
