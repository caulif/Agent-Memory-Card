use serde::{Deserialize, Serialize};

use super::chunk::text_case_to_chunk;
use super::classify::classify_chunk;
use super::scoring::{ExtractionDisposition, score_chunk};

/// 质量测试用例：输入文本及其期望的提取结果。
#[derive(Debug, Clone)]
pub struct QualityTextCase {
    pub id: String,
    pub origin: String,
    pub input: String,
    pub expected: String,
    pub expected_signal: String,
    pub expected_artifact_kind: String,
    pub expected_hardness: String,
    pub expected_terms: Vec<String>,
    pub expected_tags: Vec<String>,
}

/// 质量报告：统计真阳性、假阳性、假阴性以及各项指标。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityReport {
    pub total: usize,
    pub true_positives: Vec<String>,
    pub false_positives: Vec<String>,
    pub false_negatives: Vec<String>,
    pub visible_candidates: usize,
    pub precision: f32,
    pub recall: f32,
    pub precision_at_10: f32,
}

/// 对质量测试用例列表执行提取漏斗，返回质量报告。
/// 每个用例的输入通过 chunk → classify → score 管线处理，然后与期望结果比对。
pub fn quality_report_for_text_cases(cases: Vec<QualityTextCase>) -> QualityReport {
    let mut true_positives = Vec::new();
    let mut false_positives = Vec::new();
    let mut false_negatives = Vec::new();
    let mut ranked = Vec::new();

    for case in &cases {
        let chunk = text_case_to_chunk(&case.id, &case.origin, &case.input);
        let classification = classify_chunk(&chunk);
        let score = score_chunk(&chunk);

        let predicted_candidate = score.disposition == ExtractionDisposition::Candidate;
        let expected_candidate = case.expected == "candidate";

        // 分类字段匹配：信号类型、制品类型、硬度、术语、标签
        let signal_ok =
            case.expected_signal.is_empty() || classification.signal == case.expected_signal;
        let artifact_kind_ok = case.expected_artifact_kind.is_empty()
            || classification.artifact_kind == case.expected_artifact_kind;
        let hardness_ok =
            case.expected_hardness.is_empty() || classification.hardness == case.expected_hardness;
        let terms_ok = case
            .expected_terms
            .iter()
            .all(|term| case.input.contains(term));
        let tags_ok = case
            .expected_tags
            .iter()
            .all(|tag| classification.tags.contains(tag));

        let hit = expected_candidate
            && signal_ok
            && artifact_kind_ok
            && hardness_ok
            && terms_ok
            && tags_ok;

        if predicted_candidate {
            ranked.push((case.id.clone(), score.score, hit));
        }

        match (predicted_candidate, hit) {
            (true, true) => true_positives.push(case.id.clone()),
            (true, false) => false_positives.push(case.id.clone()),
            (false, true) => false_negatives.push(case.id.clone()),
            (false, false) => {}
        }
    }

    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let top_10 = ranked.iter().take(10).collect::<Vec<_>>();
    let top_10_hits = top_10.iter().filter(|(_, _, hit)| *hit).count();

    let predicted = true_positives.len() + false_positives.len();
    let expected = true_positives.len() + false_negatives.len();
    let precision = ratio(true_positives.len(), predicted);
    let recall = ratio(true_positives.len(), expected);
    let precision_at_10 = ratio(top_10_hits, top_10.len());

    QualityReport {
        total: cases.len(),
        true_positives,
        false_positives,
        false_negatives,
        visible_candidates: predicted,
        precision,
        recall,
        precision_at_10,
    }
}

fn ratio(numerator: usize, denominator: usize) -> f32 {
    if denominator == 0 {
        1.0
    } else {
        numerator as f32 / denominator as f32
    }
}
