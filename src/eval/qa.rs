use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::extract::pipeline::PipelineReport;
use crate::observation::ObservationRecord;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunQAReport {
    pub score: u8,
    pub cards_total: usize,
    pub evidence_valid: usize,
    pub evidence_missing: usize,
    pub evidence_invalid: usize,
    pub provider_failures: usize,
    pub duplicate_signatures: usize,
    pub scope_risks: usize,
    pub lifecycle_risks: usize,
    pub missing_artifact_impact: usize,
    pub expected_lane_misses: usize,
    pub issues: Vec<String>,
}

impl RunQAReport {
    pub fn render_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str("# Extraction Run QA\n\n");
        out.push_str(&format!("- score: {}/100\n", self.score));
        out.push_str(&format!("- cards: {}\n", self.cards_total));
        out.push_str(&format!(
            "- evidence: {} valid / {} missing / {} invalid\n",
            self.evidence_valid, self.evidence_missing, self.evidence_invalid
        ));
        out.push_str(&format!(
            "- provider failures: {}\n",
            self.provider_failures
        ));
        out.push_str(&format!(
            "- duplicate signatures: {}\n",
            self.duplicate_signatures
        ));
        out.push_str(&format!("- scope risks: {}\n", self.scope_risks));
        out.push_str(&format!("- lifecycle risks: {}\n", self.lifecycle_risks));
        out.push_str(&format!(
            "- missing artifact impact: {}\n",
            self.missing_artifact_impact
        ));
        out.push_str(&format!(
            "- expected lane misses: {}\n",
            self.expected_lane_misses
        ));
        if !self.issues.is_empty() {
            out.push_str("\n## Issues\n\n");
            for issue in &self.issues {
                out.push_str(&format!("- {issue}\n"));
            }
        }
        out
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SeenMemorySignature {
    timestamp: String,
    body_hash: String,
    evidence_hash: String,
    source_observation_ids: Vec<String>,
    title: String,
    cluster_id: String,
    outcome: String,
}

pub fn score_pipeline_report(
    report: &PipelineReport,
    observations: &[ObservationRecord],
) -> Result<RunQAReport> {
    let mut evidence_valid = 0usize;
    let mut evidence_missing = 0usize;
    let mut evidence_invalid = 0usize;
    let mut duplicate_signatures = 0usize;
    let mut scope_risks = 0usize;
    let mut lifecycle_risks = 0usize;
    let mut missing_artifact_impact = 0usize;
    let mut expected_lane_misses = 0usize;
    let mut seen = std::collections::BTreeSet::<String>::new();
    let mut issues = Vec::<String>::new();

    for card in &report.cards {
        if card.evidence_quotes.is_empty() {
            evidence_missing += 1;
            issues.push(format!("{}: missing evidence quotes", card.cluster_id));
        } else if card.evidence_quotes.iter().all(|quote| {
            observations.iter().any(|observation| {
                evidence_id_matches(&observation.id, &quote.observation_id)
                    && !quote.text.trim().is_empty()
                    && normalized_contains(&observation.body, quote.text.trim())
            })
        }) {
            evidence_valid += 1;
        } else {
            evidence_invalid += 1;
            issues.push(format!("{}: invalid evidence quote", card.cluster_id));
        }

        let signature = memory_signature(&card.body, &card.evidence_quotes);
        if !seen.insert(signature) {
            duplicate_signatures += 1;
            issues.push(format!("{}: duplicate memory signature", card.cluster_id));
        }
        if card.scope == "global" && looks_project_specific(&card.body) {
            scope_risks += 1;
            issues.push(format!(
                "{}: project-specific body has global scope",
                card.cluster_id
            ));
        }
        if looks_temporary(&card.body) && card.activation == "always-on" {
            lifecycle_risks += 1;
            issues.push(format!("{}: temporary rule is always-on", card.cluster_id));
        }
        if card.scope == "project" && card.tags.is_empty() {
            missing_artifact_impact += 1;
            issues.push(format!("{}: missing artifact impact hint", card.cluster_id));
        }
    }

    for expected_lane in expected_lanes(observations) {
        if !report
            .cards
            .iter()
            .any(|card| card_matches_expected_lane(card, expected_lane))
        {
            expected_lane_misses += 1;
            issues.push(format!(
                "expected {} recall but no accepted card covered that lane",
                expected_lane.as_str()
            ));
        }
    }

    let provider_failures = report.induce_failed;
    let mut penalty = 0usize;
    penalty += evidence_missing * 18;
    penalty += evidence_invalid * 24;
    penalty += duplicate_signatures * 12;
    penalty += provider_failures * 8;
    penalty += scope_risks * 8;
    penalty += lifecycle_risks * 12;
    penalty += missing_artifact_impact * 4;
    penalty += expected_lane_misses * 24;
    let score = 100usize.saturating_sub(penalty).min(100) as u8;

    Ok(RunQAReport {
        score,
        cards_total: report.cards.len(),
        evidence_valid,
        evidence_missing,
        evidence_invalid,
        provider_failures,
        duplicate_signatures,
        scope_risks,
        lifecycle_risks,
        missing_artifact_impact,
        expected_lane_misses,
        issues,
    })
}

pub fn append_seen_memory_signatures(
    project_root: &Path,
    report: &PipelineReport,
    outcome: &str,
) -> Result<usize> {
    let root = crate::fsutil::normalize_project_root(project_root)?;
    let dir = crate::config::kernel_dir(&root);
    fs::create_dir_all(&dir)?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("seen-memory-signatures.jsonl"))?;
    let mut written = 0usize;
    for card in &report.cards {
        let source_observation_ids = card
            .evidence_quotes
            .iter()
            .map(|quote| quote.observation_id.clone())
            .collect::<Vec<_>>();
        let entry = SeenMemorySignature {
            timestamp: Utc::now().to_rfc3339(),
            body_hash: sha256_short(&card.body),
            evidence_hash: sha256_short(
                &card
                    .evidence_quotes
                    .iter()
                    .map(|quote| format!("{}:{}", quote.observation_id, quote.text))
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            source_observation_ids,
            title: card.title.clone(),
            cluster_id: card.cluster_id.clone(),
            outcome: outcome.to_string(),
        };
        writeln!(file, "{}", serde_json::to_string(&entry)?)?;
        written += 1;
    }
    Ok(written)
}

fn memory_signature(
    body: &str,
    evidence_quotes: &[crate::extract::induce::EvidenceQuote],
) -> String {
    let evidence = evidence_quotes
        .iter()
        .map(|quote| format!("{}:{}", quote.observation_id, quote.text))
        .collect::<Vec<_>>()
        .join("\n");
    sha256_short(&format!(
        "{}\n---\n{}",
        normalize_signature_text(body),
        evidence
    ))
}

fn sha256_short(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    hasher
        .finalize()
        .iter()
        .take(12)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn normalize_signature_text(text: &str) -> String {
    text.chars()
        .filter(|ch| ch.is_alphanumeric() || ('\u{4e00}'..='\u{9fff}').contains(ch))
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

fn looks_project_specific(text: &str) -> bool {
    let lower = text.to_lowercase();
    ["daofocus", "灵石", "境界", "修炼", "当前项目"]
        .iter()
        .any(|marker| lower.contains(marker))
}

fn looks_temporary(text: &str) -> bool {
    ["这次", "这轮", "临时", "当前任务", "本次", "今天"]
        .iter()
        .any(|marker| text.contains(marker))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpectedLane {
    ProjectWorkflow,
    MemoryGovernance,
    EngineeringMethod,
}

impl ExpectedLane {
    fn as_str(self) -> &'static str {
        match self {
            Self::ProjectWorkflow => "project_workflow",
            Self::MemoryGovernance => "memory_governance",
            Self::EngineeringMethod => "engineering_method",
        }
    }
}

fn expected_lanes(observations: &[ObservationRecord]) -> Vec<ExpectedLane> {
    let mut project_workflow = 0usize;
    let mut memory = 0usize;
    let mut engineering = 0usize;
    for observation in observations {
        let text = observation.body.to_lowercase();
        project_workflow += count_markers(
            &text,
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
        );
        memory += count_markers(
            &text,
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
                "lineage",
            ],
        );
        engineering += count_markers(
            &text,
            &[
                "真实历史",
                "真实结果",
                "静态样例",
                "候选质量",
                "审阅边界",
                "人工审阅",
                "测试",
                "验证",
                "review",
                "build",
                "cargo",
                "bun",
            ],
        );
    }

    let mut lanes = Vec::new();
    if project_workflow >= 5 {
        lanes.push(ExpectedLane::ProjectWorkflow);
    }
    if memory >= 5 {
        lanes.push(ExpectedLane::MemoryGovernance);
    }
    if engineering >= 5 {
        lanes.push(ExpectedLane::EngineeringMethod);
    }
    lanes
}

fn card_matches_expected_lane(
    card: &crate::extract::crystallize::CrystallizedCard,
    lane: ExpectedLane,
) -> bool {
    let text = format!(
        "{}\n{}\n{}\n{}\n{}",
        card.title,
        card.body,
        card.brief,
        card.tags.join("\n"),
        card.evidence_quotes
            .iter()
            .map(|quote| quote.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    )
    .to_lowercase();
    match lane {
        ExpectedLane::ProjectWorkflow => {
            count_markers(
                &text,
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
            ) >= 1
        }
        ExpectedLane::MemoryGovernance => {
            count_markers(
                &text,
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
                    "lineage",
                ],
            ) >= 1
        }
        ExpectedLane::EngineeringMethod => {
            count_markers(
                &text,
                &[
                    "真实历史",
                    "真实结果",
                    "静态样例",
                    "候选质量",
                    "审阅边界",
                    "人工审阅",
                    "测试",
                    "验证",
                    "review",
                    "build",
                    "cargo",
                    "bun",
                ],
            ) >= 1
        }
    }
}

fn count_markers(text: &str, markers: &[&str]) -> usize {
    markers
        .iter()
        .filter(|marker| text.contains(**marker))
        .count()
}

fn evidence_id_matches(observation_id: &str, quote_id: &str) -> bool {
    observation_id == quote_id
        || quote_id
            .strip_prefix(observation_id)
            .is_some_and(|rest| rest.starts_with('#'))
}

fn normalized_contains(haystack: &str, needle: &str) -> bool {
    let normalize = |value: &str| -> String {
        value
            .chars()
            .filter(|ch| ch.is_alphanumeric() || ('\u{4e00}'..='\u{9fff}').contains(ch))
            .flat_map(char::to_lowercase)
            .collect()
    };
    let haystack = normalize(haystack);
    let needle = normalize(needle);
    !needle.is_empty() && haystack.contains(&needle)
}
