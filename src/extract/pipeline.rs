//! Pipeline 收口：STRIP → TRUNCATE → CLUSTER → INDUCE → CRYSTALLIZE。

use std::time::Instant;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::extract::cluster::{ClusterOptions, MessageCluster, cluster_messages_with};
use crate::extract::crystallize::{
    CrystallizationOutcome, CrystallizedCard, crystallize_candidates,
};
use crate::extract::induce::{
    DefaultInduceProvider, InduceProvider, InducedCandidate, InductionOutcome,
    induce_batch_with_provider, induce_with_provider,
};
use crate::extract::strip::strip_observations;
use crate::extract::truncate::truncate_messages;
use crate::observation::ObservationRecord;
use crate::provider::ProviderConfig;

pub const STAGE_STRIP: &str = "strip";
pub const STAGE_TRUNCATE: &str = "truncate";
pub const STAGE_CLUSTER: &str = "cluster";
pub const STAGE_INDUCE: &str = "induce";
pub const STAGE_CRYSTALLIZE: &str = "crystallize";

/// 流水线选项：组合各层配置 + 是否跳过 INDUCE。
pub struct PipelineOptions {
    pub cluster: ClusterOptions,
    /// LLM 归纳策略：默认先选 Top-K 高信号 evidence，再一次批量综合。
    pub induce: InduceMode,
    /// 不传 provider 即跳过 INDUCE / CRYSTALLIZE，只到 Layer 3
    pub skip_induce: bool,
}

impl Default for PipelineOptions {
    fn default() -> Self {
        Self {
            cluster: ClusterOptions::default(),
            induce: InduceMode::default(),
            skip_induce: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InduceMode {
    /// 兼容旧路径：每个 cluster 一次 LLM。
    PerCluster,
    /// 快速主路径：只选 Top-K 高信号 cluster，一次 LLM 综合多张卡。
    BatchTopK {
        evidence_top_k: usize,
        max_cards: usize,
    },
}

impl Default for InduceMode {
    fn default() -> Self {
        Self::BatchTopK {
            evidence_top_k: 8,
            max_cards: 5,
        }
    }
}

/// 流水线最终报告。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineReport {
    pub observations_in: usize,
    pub stripped_kept: usize,
    pub truncated_count: usize,
    pub truncated_long_messages: usize,
    pub clusters_in: usize,
    pub induce_accepted: usize,
    pub induce_rejected: usize,
    pub induce_failed: usize,
    #[serde(default)]
    pub induce_calls: usize,
    #[serde(default)]
    pub induce_clusters_selected: usize,
    pub crystallize_accepted: usize,
    pub crystallize_rejected: usize,
    pub cards: Vec<CrystallizedCard>,
    #[serde(default)]
    pub crystallize_rejects: Vec<CrystallizeReject>,
    #[serde(default)]
    pub stage_metrics: Vec<PipelineStageMetric>,
    /// 失败原因的串简表（前若干条），便于 debug
    pub failures_preview: Vec<String>,
    /// 各层耗时（毫秒）。键：strip / truncate / cluster / induce / crystallize
    #[serde(default)]
    pub layer_timings_ms: std::collections::BTreeMap<String, u64>,
    /// 流水线版本号，写入卡的 extraction.pipeline_version
    pub pipeline_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrystallizeReject {
    pub cluster_id: String,
    pub candidate: InducedCandidate,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PipelineStageMetric {
    pub stage: String,
    pub input_count: usize,
    pub output_count: usize,
    pub accepted_count: usize,
    pub rejected_count: usize,
    pub failed_count: usize,
    pub duration_ms: u64,
}

#[derive(Debug, Clone)]
pub struct StripStageInput<'a> {
    pub observations: &'a [ObservationRecord],
}

#[derive(Debug, Clone)]
pub struct StripStageOutput {
    pub messages: Vec<crate::extract::strip::StrippedMessage>,
}

#[derive(Debug, Clone)]
pub struct TruncateStageInput {
    pub messages: Vec<crate::extract::strip::StrippedMessage>,
}

#[derive(Debug, Clone)]
pub struct TruncateStageOutput {
    pub messages: Vec<crate::extract::truncate::TruncatedMessage>,
    pub long_truncated: usize,
}

#[derive(Debug, Clone)]
pub struct ClusterStageInput {
    pub messages: Vec<crate::extract::truncate::TruncatedMessage>,
    pub options: ClusterOptions,
}

#[derive(Debug, Clone)]
pub struct ClusterStageOutput {
    pub clusters: Vec<MessageCluster>,
}

#[derive(Debug, Clone)]
pub struct InduceStageInput {
    pub clusters: Vec<MessageCluster>,
    pub mode: InduceMode,
    pub skip_induce: bool,
}

#[derive(Debug, Clone)]
pub struct InduceStageOutput {
    pub candidates: Vec<InducedCandidate>,
    pub rejected: usize,
    pub failed: usize,
    pub calls: usize,
    pub clusters_selected: usize,
    pub failures_preview: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CrystallizeStageInput {
    pub candidates: Vec<InducedCandidate>,
}

#[derive(Debug, Clone)]
pub struct CrystallizeStageOutput {
    pub cards: Vec<CrystallizedCard>,
    pub rejects: Vec<CrystallizeReject>,
    pub failures_preview: Vec<String>,
}

impl PipelineReport {
    /// 人类可读的单页摘要（不含 cards 详情）。
    pub fn render_summary(&self) -> String {
        let mut out = String::new();
        out.push_str("Pipeline report\n");
        out.push_str(&format!(
            "- observations_in:        {}\n",
            self.observations_in
        ));
        out.push_str(&format!(
            "- Layer 1 STRIP kept:     {} ({:.0}% retained)\n",
            self.stripped_kept,
            percent(self.stripped_kept, self.observations_in)
        ));
        out.push_str(&format!(
            "- Layer 2 TRUNCATE:       {} messages ({} long-truncated)\n",
            self.truncated_count, self.truncated_long_messages
        ));
        out.push_str(&format!(
            "- Layer 3 CLUSTER:        {} clusters\n",
            self.clusters_in
        ));
        out.push_str(&format!(
            "- Layer 4 INDUCE:         {} accepted / {} rejected / {} failed\n",
            self.induce_accepted, self.induce_rejected, self.induce_failed
        ));
        out.push_str(&format!(
            "                           {} LLM call(s), {} / {} clusters selected\n",
            self.induce_calls, self.induce_clusters_selected, self.clusters_in
        ));
        out.push_str(&format!(
            "- Layer 5 CRYSTALLIZE:    {} accepted / {} rejected\n",
            self.crystallize_accepted, self.crystallize_rejected
        ));
        if !self.failures_preview.is_empty() {
            out.push_str("\nFailure preview (first 10):\n");
            for line in &self.failures_preview {
                out.push_str(&format!("  - {line}\n"));
            }
        }
        if !self.cards.is_empty() {
            out.push_str("\nAccepted cards:\n");
            for card in &self.cards {
                out.push_str(&format!(
                    "  - [{}] {} (recurrence={}, confidence={:.2})\n",
                    card.kind, card.title, card.recurrence, card.confidence
                ));
            }
        }
        out
    }
}

fn percent(part: usize, whole: usize) -> f32 {
    if whole == 0 {
        0.0
    } else {
        (part as f32) * 100.0 / (whole as f32)
    }
}

/// 主入口（默认 provider）。
pub fn run_pipeline(
    observations: &[ObservationRecord],
    cfg: &ProviderConfig,
    options: &PipelineOptions,
) -> Result<PipelineReport> {
    let provider = DefaultInduceProvider { cfg };
    run_pipeline_with(observations, &provider, options)
}

/// 自定义 provider 入口（测试 / mock 用）。
pub fn run_pipeline_with(
    observations: &[ObservationRecord],
    provider: &dyn InduceProvider,
    options: &PipelineOptions,
) -> Result<PipelineReport> {
    let mut timings = std::collections::BTreeMap::<String, u64>::new();
    let mut stage_metrics = Vec::<PipelineStageMetric>::new();

    let t = Instant::now();
    let strip_output = run_strip_stage(StripStageInput { observations });
    let strip_ms = t.elapsed().as_millis() as u64;
    timings.insert(STAGE_STRIP.to_string(), strip_ms);
    stage_metrics.push(PipelineStageMetric {
        stage: STAGE_STRIP.to_string(),
        input_count: observations.len(),
        output_count: strip_output.messages.len(),
        accepted_count: strip_output.messages.len(),
        rejected_count: observations
            .len()
            .saturating_sub(strip_output.messages.len()),
        failed_count: 0,
        duration_ms: strip_ms,
    });

    let t = Instant::now();
    let truncate_output = run_truncate_stage(TruncateStageInput {
        messages: strip_output.messages,
    });
    let truncate_ms = t.elapsed().as_millis() as u64;
    timings.insert(STAGE_TRUNCATE.to_string(), truncate_ms);
    stage_metrics.push(PipelineStageMetric {
        stage: STAGE_TRUNCATE.to_string(),
        input_count: truncate_output.messages.len(),
        output_count: truncate_output.messages.len(),
        accepted_count: truncate_output.messages.len(),
        rejected_count: 0,
        failed_count: 0,
        duration_ms: truncate_ms,
    });

    let t = Instant::now();
    let truncate_count = truncate_output.messages.len();
    let truncated_long = truncate_output.long_truncated;
    let cluster_output = run_cluster_stage(ClusterStageInput {
        messages: truncate_output.messages,
        options: options.cluster.clone(),
    })?;
    let cluster_ms = t.elapsed().as_millis() as u64;
    timings.insert(STAGE_CLUSTER.to_string(), cluster_ms);
    stage_metrics.push(PipelineStageMetric {
        stage: STAGE_CLUSTER.to_string(),
        input_count: truncate_count,
        output_count: cluster_output.clusters.len(),
        accepted_count: cluster_output.clusters.len(),
        rejected_count: 0,
        failed_count: 0,
        duration_ms: cluster_ms,
    });

    let t = Instant::now();
    let clusters_in = cluster_output.clusters.len();
    let induce_output = run_induce_stage(
        provider,
        InduceStageInput {
            clusters: cluster_output.clusters,
            mode: options.induce,
            skip_induce: options.skip_induce,
        },
    );
    let induce_ms = t.elapsed().as_millis() as u64;
    timings.insert(STAGE_INDUCE.to_string(), induce_ms);
    stage_metrics.push(PipelineStageMetric {
        stage: STAGE_INDUCE.to_string(),
        input_count: clusters_in,
        output_count: induce_output.candidates.len(),
        accepted_count: induce_output.candidates.len(),
        rejected_count: induce_output.rejected,
        failed_count: induce_output.failed,
        duration_ms: induce_ms,
    });

    let t = Instant::now();
    let induce_accepted = induce_output.candidates.len();
    let induce_rejected = induce_output.rejected;
    let induce_failed = induce_output.failed;
    let induce_calls = induce_output.calls;
    let induce_clusters_selected = induce_output.clusters_selected;
    let mut failures_preview = induce_output.failures_preview;
    let crystallize_output = run_crystallize_stage(CrystallizeStageInput {
        candidates: induce_output.candidates,
    });
    let crystallize_ms = t.elapsed().as_millis() as u64;
    timings.insert(STAGE_CRYSTALLIZE.to_string(), crystallize_ms);
    failures_preview.extend(
        crystallize_output
            .failures_preview
            .iter()
            .take(10usize.saturating_sub(failures_preview.len()))
            .cloned(),
    );
    stage_metrics.push(PipelineStageMetric {
        stage: STAGE_CRYSTALLIZE.to_string(),
        input_count: induce_accepted,
        output_count: crystallize_output.cards.len(),
        accepted_count: crystallize_output.cards.len(),
        rejected_count: crystallize_output.rejects.len(),
        failed_count: 0,
        duration_ms: crystallize_ms,
    });

    Ok(PipelineReport {
        observations_in: observations.len(),
        stripped_kept: stage_metrics[0].output_count,
        truncated_count: truncate_count,
        truncated_long_messages: truncated_long,
        clusters_in,
        induce_accepted,
        induce_rejected,
        induce_failed,
        induce_calls,
        induce_clusters_selected,
        crystallize_accepted: crystallize_output.cards.len(),
        crystallize_rejected: crystallize_output.rejects.len(),
        cards: crystallize_output.cards,
        crystallize_rejects: crystallize_output.rejects,
        stage_metrics,
        failures_preview,
        layer_timings_ms: timings,
        pipeline_version: PIPELINE_VERSION,
    })
}

/// 当前流水线版本号；任何会改变卡产出的 layer 修改都应 +1。
pub const PIPELINE_VERSION: u32 = 1;

pub fn run_strip_stage(input: StripStageInput<'_>) -> StripStageOutput {
    StripStageOutput {
        messages: strip_observations(input.observations),
    }
}

pub fn run_truncate_stage(input: TruncateStageInput) -> TruncateStageOutput {
    let messages = truncate_messages(&input.messages);
    let long_truncated = messages
        .iter()
        .filter(|message| message.was_truncated)
        .count();
    TruncateStageOutput {
        messages,
        long_truncated,
    }
}

pub fn run_cluster_stage(input: ClusterStageInput) -> Result<ClusterStageOutput> {
    Ok(ClusterStageOutput {
        clusters: cluster_messages_with(&input.messages, &input.options)?,
    })
}

pub fn run_induce_stage(
    provider: &dyn InduceProvider,
    input: InduceStageInput,
) -> InduceStageOutput {
    let (outcomes, calls, clusters_selected): (Vec<InductionOutcome>, usize, usize) = if input
        .skip_induce
    {
        (Vec::new(), 0, 0)
    } else {
        match input.mode {
            InduceMode::PerCluster => (
                induce_with_provider(provider, &input.clusters),
                input.clusters.len(),
                input.clusters.len(),
            ),
            InduceMode::BatchTopK {
                evidence_top_k,
                max_cards,
            } => {
                let selected = select_induce_clusters(&input.clusters, evidence_top_k);
                let selected_len = selected.len();
                let (outcomes, calls) = if selected.is_empty() {
                    (Vec::new(), 0)
                } else {
                    let batch_outcomes = induce_batch_with_provider(provider, &selected, max_cards);
                    if is_batch_failure(&batch_outcomes) {
                        let mut outcomes = batch_outcomes;
                        outcomes.extend(induce_with_provider(provider, &selected));
                        (outcomes, 1 + selected_len)
                    } else {
                        (batch_outcomes, 1)
                    }
                };
                (outcomes, calls, selected_len)
            }
        }
    };
    let mut candidates = Vec::new();
    let mut rejected = 0usize;
    let mut failed = 0usize;
    let mut failures_preview = Vec::new();
    for outcome in outcomes {
        match outcome {
            InductionOutcome::Accepted(candidate) => candidates.push(candidate),
            InductionOutcome::Rejected { cluster_id, reason } => {
                rejected += 1;
                if failures_preview.len() < 5 {
                    failures_preview.push(format!("induce reject {cluster_id}: {reason}"));
                }
            }
            InductionOutcome::Failed { cluster_id, error } => {
                failed += 1;
                if failures_preview.len() < 5 {
                    failures_preview.push(format!("induce fail {cluster_id}: {error}"));
                }
            }
        }
    }
    InduceStageOutput {
        candidates,
        rejected,
        failed,
        calls,
        clusters_selected,
        failures_preview,
    }
}

pub fn run_crystallize_stage(input: CrystallizeStageInput) -> CrystallizeStageOutput {
    let outcomes = crystallize_candidates(&input.candidates);
    let mut cards = Vec::new();
    let mut rejects = Vec::new();
    let mut failures_preview = Vec::new();
    for outcome in outcomes {
        match outcome {
            CrystallizationOutcome::Accepted(card) => cards.push(card),
            CrystallizationOutcome::Rejected {
                cluster_id,
                candidate,
                reasons,
            } => {
                rejects.push(CrystallizeReject {
                    cluster_id: cluster_id.clone(),
                    candidate,
                    reasons: reasons.clone(),
                });
                if failures_preview.len() < 10 {
                    failures_preview.push(format!(
                        "crystallize reject {cluster_id}: {}",
                        reasons.join("; ")
                    ));
                }
            }
        }
    }
    CrystallizeStageOutput {
        cards,
        rejects,
        failures_preview,
    }
}

fn select_induce_clusters(
    clusters: &[MessageCluster],
    evidence_top_k: usize,
) -> Vec<MessageCluster> {
    if evidence_top_k == 0 {
        return Vec::new();
    }
    let mut scored = clusters
        .iter()
        .map(|cluster| {
            (
                cluster_lane(cluster),
                cluster_signal_score(cluster),
                cluster.clone(),
            )
        })
        .collect::<Vec<_>>();
    scored.sort_by(|left, right| {
        right
            .1
            .cmp(&left.1)
            .then_with(|| right.2.recurrence.cmp(&left.2.recurrence))
            .then_with(|| left.2.cluster_id.cmp(&right.2.cluster_id))
    });
    let mut selected = Vec::<MessageCluster>::new();
    let mut selected_ids = std::collections::BTreeSet::<String>::new();
    for lane in [
        ExtractionLane::ProjectWorkflow,
        ExtractionLane::MemoryGovernance,
        ExtractionLane::EngineeringMethod,
        ExtractionLane::CollaborationPreference,
        ExtractionLane::TemporaryTask,
    ] {
        if selected.len() >= evidence_top_k {
            break;
        }
        if let Some((_, _, cluster)) = scored.iter().find(|(cluster_lane, score, cluster)| {
            *cluster_lane == lane && *score > 0 && !selected_ids.contains(&cluster.cluster_id)
        }) {
            selected.push(cluster.clone());
            selected_ids.insert(cluster.cluster_id.clone());
        }
    }
    if selected.len() >= evidence_top_k {
        return selected;
    }
    scored
        .into_iter()
        .filter(|(_, score, cluster)| *score > 0 && !selected_ids.contains(&cluster.cluster_id))
        .take(evidence_top_k - selected.len())
        .for_each(|(_, _, cluster)| selected.push(cluster));
    selected
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ExtractionLane {
    ProjectWorkflow,
    EngineeringMethod,
    CollaborationPreference,
    MemoryGovernance,
    TemporaryTask,
    General,
}

fn cluster_lane(cluster: &MessageCluster) -> ExtractionLane {
    let combined = cluster
        .messages
        .iter()
        .map(|message| message.body.as_str())
        .collect::<Vec<_>>()
        .join("\n")
        .to_lowercase();
    if contains_any(
        &combined,
        &[
            "daofocus",
            "tdd",
            "bun",
            "review",
            "审查",
            "测试",
            "验证",
            "合并",
            "merge",
            "工作流",
            "项目约定",
            "协作边界",
        ],
    ) {
        return ExtractionLane::ProjectWorkflow;
    }
    if contains_any(
        &combined,
        &[
            "memory card",
            "golden set",
            "evidence",
            "observation",
            "提炼",
            "证据",
            "候选",
            "记忆",
            "drift",
        ],
    ) {
        return ExtractionLane::MemoryGovernance;
    }
    if contains_any(
        &combined,
        &[
            "这次",
            "这轮",
            "当前",
            "临时",
            "今天",
            "先不要",
            "先别",
            "本次",
            "temporary",
        ],
    ) {
        return ExtractionLane::TemporaryTask;
    }
    if contains_any(
        &combined,
        &[
            "不要问",
            "自主",
            "简体中文",
            "语气",
            "回复",
            "交流",
            "沟通",
            "偏好",
            "preference",
        ],
    ) {
        return ExtractionLane::CollaborationPreference;
    }
    if methodology_signal_score(&combined) > 0 {
        return ExtractionLane::EngineeringMethod;
    }
    ExtractionLane::General
}

fn contains_any(text: &str, markers: &[&str]) -> bool {
    markers.iter().any(|marker| text.contains(marker))
}

fn project_workflow_signal_score(text: &str) -> usize {
    let lower = text.to_lowercase();
    [
        "daofocus",
        "tdd",
        "bun",
        "review",
        "审查",
        "测试",
        "验证",
        "合并",
        "merge",
        "工作流",
        "项目约定",
        "协作边界",
    ]
    .iter()
    .filter(|marker| lower.contains(**marker))
    .count()
}

fn memory_governance_signal_score(text: &str) -> usize {
    let lower = text.to_lowercase();
    [
        "memory card",
        "golden set",
        "evidence",
        "observation",
        "提炼",
        "证据",
        "候选",
        "记忆",
        "drift",
        "lineage",
    ]
    .iter()
    .filter(|marker| lower.contains(**marker))
    .count()
}

fn is_batch_failure(outcomes: &[InductionOutcome]) -> bool {
    matches!(
        outcomes,
        [InductionOutcome::Failed { cluster_id, .. }] if cluster_id == "batch"
    )
}

fn cluster_signal_score(cluster: &MessageCluster) -> usize {
    let mut score = cluster.recurrence.saturating_sub(1);
    let mut has_high_signal = cluster.recurrence > 1;
    for message in &cluster.messages {
        if message.observation_id.contains("#signal") {
            score += 50;
            has_high_signal = true;
        }
        let methodology = methodology_signal_score(&message.body);
        if methodology > 0 {
            has_high_signal = true;
        }
        score += methodology * 10;
        if message.body.contains("候选质量") || message.body.contains("质量优先") {
            score += 6;
        }
        let project_workflow = project_workflow_signal_score(&message.body);
        if project_workflow > 0 {
            has_high_signal = true;
        }
        score += project_workflow * 12;
        let memory_governance = memory_governance_signal_score(&message.body);
        if memory_governance > 0 {
            has_high_signal = true;
        }
        score += memory_governance * 8;
        score += durable_signal_score(&message.body);
    }
    if has_high_signal { score } else { 0 }
}

fn methodology_signal_score(text: &str) -> usize {
    let lower = text.to_lowercase();
    [
        "真实历史",
        "真实结果",
        "真实输入",
        "静态样例",
        "持续修正",
        "自我修正",
        "候选质量",
        "质量优先",
        "少而精",
        "少而准",
        "审阅边界",
        "人工审阅",
        "先 review",
        "先规划",
        "先澄清",
        "先提问",
        "小改快测",
        "大改重测",
        "大改详测",
        "dry-run",
        "dry run",
        "推理引擎",
        "检查有没有问题",
        "质量高不高",
        "real history",
        "human review",
        "review boundary",
    ]
    .iter()
    .filter(|marker| lower.contains(**marker))
    .count()
}

fn durable_signal_score(text: &str) -> usize {
    let lower = text.to_lowercase();
    [
        "以后", "每次", "默认", "统一", "必须", "不要", "优先", "always", "prefer", "must",
        "default",
    ]
    .iter()
    .filter(|marker| lower.contains(**marker))
    .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::ProviderRequest;

    fn obs(id: &str, body: &str) -> ObservationRecord {
        ObservationRecord {
            id: id.to_string(),
            source_kind: "claude-code-session".to_string(),
            source_path: "test.jsonl".to_string(),
            agent: Some("claude-code".to_string()),
            body: body.to_string(),
            evidence: "test".to_string(),
            redacted: false,
            created_at: "2026-05-01T00:00:00+00:00".to_string(),
        }
    }

    /// Stub provider: returns one happy-path induce response per call.
    struct HappyStub;
    impl InduceProvider for HappyStub {
        fn call(&self, _: &ProviderRequest, _: usize) -> Result<String> {
            Ok(r#"{
                "title": "评估提炼优先用真实历史回归",
                "when": "评估提炼质量或修改提炼逻辑时",
                "what": "用真实历史会话做回归验证而不是只依赖静态样例",
                "why": "让提炼规则在真实数据上接受检验",
                "kind": "procedure",
                "scope": "global",
                "evidence_quotes": [
                    {"observation_id": "o1", "text": "用真实历史"}
                ],
                "temporal_status": "stable",
                "confidence": 0.85
            }"#
            .to_string())
        }
    }

    #[test]
    fn end_to_end_with_happy_stub_produces_card() {
        let observations = vec![
            obs(
                "o1",
                "评估提炼质量时用真实历史会话做回归验证而不是只依赖静态样例",
            ),
            obs(
                "o2",
                "评估提炼时也别忘了用真实历史会话做回归验证而不是只依赖静态样例",
            ),
        ];
        let options = PipelineOptions {
            cluster: ClusterOptions {
                force_jaccard: true,
                ..ClusterOptions::default()
            },
            induce: InduceMode::PerCluster,
            skip_induce: false,
        };
        let report = run_pipeline_with(&observations, &HappyStub, &options).expect("ok");
        assert_eq!(report.observations_in, 2);
        assert!(report.stripped_kept >= 1);
        assert!(report.crystallize_accepted >= 1);
        assert!(!report.cards.is_empty());
        let card = &report.cards[0];
        assert_eq!(card.kind, "procedure");
        assert!(card.body.starts_with("当"));
        assert!(card.brief.starts_with("用于"));
    }

    #[test]
    fn skip_induce_returns_no_cards_but_runs_layers_1_to_3() {
        let observations = vec![
            obs(
                "o1",
                "评估提炼质量时用真实历史会话做回归验证不要只用静态样例",
            ),
            obs(
                "o2",
                "评估提炼时优先用真实历史会话做回归验证不要只用静态样例",
            ),
        ];
        let options = PipelineOptions {
            cluster: ClusterOptions {
                force_jaccard: true,
                ..ClusterOptions::default()
            },
            induce: InduceMode::PerCluster,
            skip_induce: true,
        };
        let report = run_pipeline_with(&observations, &HappyStub, &options).expect("ok");
        assert_eq!(report.observations_in, 2);
        assert!(report.stripped_kept >= 1);
        assert_eq!(report.induce_accepted, 0);
        assert_eq!(report.crystallize_accepted, 0);
        assert_eq!(
            report
                .stage_metrics
                .iter()
                .map(|metric| metric.stage.as_str())
                .collect::<Vec<_>>(),
            vec![
                STAGE_STRIP,
                STAGE_TRUNCATE,
                STAGE_CLUSTER,
                STAGE_INDUCE,
                STAGE_CRYSTALLIZE
            ]
        );
        assert_eq!(report.stage_metrics[0].input_count, 2);
        assert_eq!(report.stage_metrics[0].output_count, report.stripped_kept);
        assert_eq!(report.stage_metrics[3].failed_count, report.induce_failed);
    }

    #[test]
    fn induce_failure_does_not_block_other_clusters() {
        // 两条 obs，第一次调用失败，第二次成功
        struct CountStub {
            count: std::sync::Mutex<usize>,
        }
        impl InduceProvider for CountStub {
            fn call(&self, _: &ProviderRequest, _: usize) -> Result<String> {
                let mut c = self.count.lock().unwrap();
                *c += 1;
                if *c == 1 {
                    Err(anyhow::anyhow!("simulated network down"))
                } else {
                    Ok(r#"{"reject": true, "reason": "second one rejected"}"#.to_string())
                }
            }
        }
        let observations = vec![
            obs("o1", "正常长一点的中文表达内容防止被 STRIP 拦"),
            obs("o2", "另一条正常长度的中文表达内容防止被 STRIP 拦"),
        ];
        // force jaccard + 调宽阈值，确保 2 条形成 2 簇
        let options = PipelineOptions {
            cluster: ClusterOptions {
                force_jaccard: true,
                jaccard_long_threshold: 0.95, // 几乎不归簇
                jaccard_short_threshold: 0.95,
                keep_singletons: true,
                ..ClusterOptions::default()
            },
            induce: InduceMode::PerCluster,
            skip_induce: false,
        };
        let stub = CountStub {
            count: std::sync::Mutex::new(0),
        };
        let report = run_pipeline_with(&observations, &stub, &options).expect("ok");
        assert!(report.induce_failed >= 1 || report.induce_rejected >= 1);
        // 没有 crystallize accepted（一个失败一个 reject）
        assert_eq!(report.crystallize_accepted, 0);
    }

    #[test]
    fn batch_top_k_induce_calls_provider_once_for_selected_evidence() {
        struct CaptureStub {
            calls: std::sync::Mutex<Vec<String>>,
        }
        impl InduceProvider for CaptureStub {
            fn call(&self, request: &ProviderRequest, _: usize) -> Result<String> {
                self.calls.lock().unwrap().push(request.user_prompt.clone());
                Ok(r#"[{
                    "title": "优先保证候选质量",
                    "when": "生成候选、规则或记忆卡时",
                    "what": "优先保证候选质量，而不是追求数量",
                    "why": "让候选长期稳定、可回溯、可执行",
                    "kind": "preference",
                    "scope": "global",
                    "evidence_quotes": [
                        {"observation_id": "o-signal", "text": "候选质量优先"}
                    ],
                    "temporal_status": "stable",
                    "confidence": 0.9
                }]"#
                .to_string())
            }
        }
        let observations = vec![
            obs(
                "o-signal",
                "重视真实历史回归，不迷信静态样例。\n候选质量优先于数量。",
            ),
            obs(
                "o-noise",
                "请只读这些文件并输出风险，不要改代码。这是一次性任务边界。",
            ),
            obs("o-noise-2", "发布版本只在需要时生成 release。"),
        ];
        let options = PipelineOptions {
            cluster: ClusterOptions {
                force_jaccard: true,
                jaccard_long_threshold: 0.99,
                jaccard_short_threshold: 0.99,
                keep_singletons: true,
                ..ClusterOptions::default()
            },
            induce: InduceMode::BatchTopK {
                evidence_top_k: 1,
                max_cards: 1,
            },
            skip_induce: false,
        };
        let stub = CaptureStub {
            calls: std::sync::Mutex::new(Vec::new()),
        };

        let report = run_pipeline_with(&observations, &stub, &options).expect("ok");

        let calls = stub.calls.lock().unwrap();
        assert_eq!(calls.len(), 1, "batch mode should call provider once");
        assert!(calls[0].contains("候选质量优先"), "{}", calls[0]);
        assert!(!calls[0].contains("一次性任务边界"), "{}", calls[0]);
        assert_eq!(report.induce_calls, 1);
        assert_eq!(report.induce_clusters_selected, 1);
        assert_eq!(report.crystallize_accepted, 1);
    }

    #[test]
    fn batch_top_k_reserves_budget_for_project_workflow_lane() {
        struct CaptureOnlyStub {
            calls: std::sync::Mutex<Vec<String>>,
        }
        impl InduceProvider for CaptureOnlyStub {
            fn call(&self, request: &ProviderRequest, _: usize) -> Result<String> {
                self.calls.lock().unwrap().push(request.user_prompt.clone());
                Ok("[]".to_string())
            }
        }
        let observations = vec![
            obs(
                "o-method-1",
                "评估提炼质量时必须用真实历史会话回归验证，不要只依赖静态样例。",
            ),
            obs(
                "o-method-2",
                "候选质量优先于数量，生成 Memory Card 前要先保证证据和审阅边界。",
            ),
            obs(
                "o-project-workflow",
                "DaoFocus 项目约定：TDD、Bun 测试、review、merge 前验证是长期工作流，不能被泛泛工程方法论挤掉。",
            ),
        ];
        let options = PipelineOptions {
            cluster: ClusterOptions {
                force_jaccard: true,
                jaccard_long_threshold: 0.99,
                jaccard_short_threshold: 0.99,
                keep_singletons: true,
                ..ClusterOptions::default()
            },
            induce: InduceMode::BatchTopK {
                evidence_top_k: 2,
                max_cards: 3,
            },
            skip_induce: false,
        };
        let stub = CaptureOnlyStub {
            calls: std::sync::Mutex::new(Vec::new()),
        };

        let report = run_pipeline_with(&observations, &stub, &options).expect("ok");

        let calls = stub.calls.lock().unwrap();
        assert_eq!(report.induce_clusters_selected, 2);
        assert_eq!(calls.len(), 1);
        assert!(
            calls[0].contains("真实历史会话回归验证") || calls[0].contains("候选质量优先于数量"),
            "{}",
            calls[0]
        );
        assert!(
            calls[0].contains("DaoFocus") && calls[0].contains("TDD"),
            "project-workflow lane should keep project workflow in the selected evidence budget:\n{}",
            calls[0]
        );
        assert!(
            calls[0].contains("project workflow rules"),
            "batch induce prompt should tell providers to keep project workflow rules:\n{}",
            calls[0]
        );
    }

    #[test]
    fn batch_top_k_falls_back_to_per_cluster_when_batch_fails() {
        struct FallbackStub {
            calls: std::sync::Mutex<usize>,
        }
        impl InduceProvider for FallbackStub {
            fn call(&self, _: &ProviderRequest, _: usize) -> Result<String> {
                let mut calls = self.calls.lock().unwrap();
                *calls += 1;
                if *calls == 1 {
                    Err(anyhow::anyhow!("batch provider response had no text"))
                } else {
                    Ok(r#"{"reject": true, "reason": "not durable enough"}"#.to_string())
                }
            }
        }
        let observations = vec![
            obs("o1", "候选质量优先，真实历史回归必须覆盖这类提炼规则。"),
            obs("o2", "人工审阅边界必须保留，不要自动批准高影响记忆。"),
        ];
        let options = PipelineOptions {
            cluster: ClusterOptions {
                force_jaccard: true,
                jaccard_long_threshold: 0.99,
                jaccard_short_threshold: 0.99,
                keep_singletons: true,
                ..ClusterOptions::default()
            },
            induce: InduceMode::BatchTopK {
                evidence_top_k: 2,
                max_cards: 3,
            },
            skip_induce: false,
        };
        let stub = FallbackStub {
            calls: std::sync::Mutex::new(0),
        };

        let report = run_pipeline_with(&observations, &stub, &options).expect("ok");

        assert_eq!(report.induce_calls, 3);
        assert_eq!(report.induce_clusters_selected, 2);
        assert_eq!(report.induce_failed, 1);
        assert_eq!(report.induce_rejected, 2);
        assert!(
            report
                .failures_preview
                .iter()
                .any(|line| line.contains("batch provider response had no text")),
            "batch failure should remain visible after fallback: {:?}",
            report.failures_preview
        );
    }

    #[test]
    fn batch_top_k_with_no_signal_skips_provider() {
        struct PanicStub;
        impl InduceProvider for PanicStub {
            fn call(&self, _: &ProviderRequest, _: usize) -> Result<String> {
                panic!("provider should not be called when no high-signal evidence is selected")
            }
        }
        let observations = vec![obs(
            "o-noise",
            "只读任务：阅读这些文件，不要改代码。输出当前流程概要。",
        )];
        let options = PipelineOptions {
            cluster: ClusterOptions {
                force_jaccard: true,
                keep_singletons: true,
                ..ClusterOptions::default()
            },
            induce: InduceMode::BatchTopK {
                evidence_top_k: 5,
                max_cards: 3,
            },
            skip_induce: false,
        };

        let report = run_pipeline_with(&observations, &PanicStub, &options).expect("ok");

        assert_eq!(report.induce_calls, 0);
        assert_eq!(report.induce_clusters_selected, 0);
        assert_eq!(report.crystallize_accepted, 0);
    }

    #[test]
    fn crystallize_rejects_keep_candidate_details_for_review() {
        struct OverlapStub;
        impl InduceProvider for OverlapStub {
            fn call(&self, _: &ProviderRequest, _: usize) -> Result<String> {
                Ok(r#"{
                    "title": "代码变更后重复质量审查",
                    "when": "完成代码变更后",
                    "what": "执行代码质量审查并在修复后再次审查",
                    "why": "执行代码质量审查并在修复后再次审查",
                    "kind": "procedure",
                    "scope": "global",
                    "evidence_quotes": [
                        {"observation_id": "o-review", "text": "执行代码质量审查"}
                    ],
                    "temporal_status": "stable",
                    "confidence": 0.9
                }"#
                .to_string())
            }
        }
        let observations = vec![obs(
            "o-review",
            "完成代码变更后要执行代码质量审查，并在修复后再次审查。",
        )];
        let options = PipelineOptions {
            cluster: ClusterOptions {
                force_jaccard: true,
                keep_singletons: true,
                ..ClusterOptions::default()
            },
            induce: InduceMode::PerCluster,
            skip_induce: false,
        };

        let report = run_pipeline_with(&observations, &OverlapStub, &options).expect("ok");

        assert_eq!(report.crystallize_accepted, 0);
        assert_eq!(report.crystallize_rejected, 1);
        assert_eq!(report.crystallize_rejects.len(), 1);
        let rejected = &report.crystallize_rejects[0];
        assert!(
            rejected.cluster_id.starts_with("c_"),
            "{}",
            rejected.cluster_id
        );
        assert_eq!(rejected.candidate.title, "代码变更后重复质量审查");
        assert!(
            rejected
                .reasons
                .iter()
                .any(|reason| reason.contains("brief-body overlap")),
            "{:#?}",
            rejected.reasons
        );
    }
}
