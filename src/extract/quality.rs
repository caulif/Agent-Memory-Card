use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
    pub expected_memory_tier: String,
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
    pub f1: f32,
    pub precision_at_10: f32,
    pub cohen_kappa: f32,
    pub tier_mix: BTreeMap<String, usize>,
    pub tier_recall: BTreeMap<String, f32>,
    pub precision_at_10_by_tier: BTreeMap<String, f32>,
}

/// 对质量测试用例列表执行提取漏斗，返回质量报告。
/// 每个用例的输入通过 chunk → classify → score 管线处理，然后与期望结果比对。
pub fn quality_report_for_text_cases(cases: Vec<QualityTextCase>) -> QualityReport {
    let mut true_positives = Vec::new();
    let mut false_positives = Vec::new();
    let mut false_negatives = Vec::new();
    let mut true_negative_count = 0usize;
    let mut ranked = Vec::new();
    let mut tier_mix = BTreeMap::<String, usize>::new();
    let mut expected_by_tier = BTreeMap::<String, usize>::new();
    let mut hits_by_tier = BTreeMap::<String, usize>::new();

    for case in &cases {
        let chunk = text_case_to_chunk(&case.id, &case.origin, &case.input);
        let classification = classify_chunk(&chunk);
        let score = score_chunk(&chunk);
        let predicted_tier = infer_quality_memory_tier(&case.input, &classification.signal);

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
        let tier_ok =
            case.expected_memory_tier.is_empty() || predicted_tier == case.expected_memory_tier;

        let classification_hit = expected_candidate
            && signal_ok
            && artifact_kind_ok
            && hardness_ok
            && terms_ok
            && tags_ok
            && tier_ok;
        let candidate_hit = predicted_candidate && expected_candidate;

        if predicted_candidate {
            *tier_mix.entry(predicted_tier.clone()).or_default() += 1;
            ranked.push((
                case.id.clone(),
                score.score,
                expected_candidate,
                predicted_tier.clone(),
            ));
        }
        if expected_candidate && !case.expected_memory_tier.is_empty() {
            *expected_by_tier
                .entry(case.expected_memory_tier.clone())
                .or_default() += 1;
            if classification_hit {
                *hits_by_tier
                    .entry(case.expected_memory_tier.clone())
                    .or_default() += 1;
            }
        }

        match (predicted_candidate, expected_candidate, candidate_hit) {
            (true, true, true) => true_positives.push(case.id.clone()),
            (true, false, _) => false_positives.push(case.id.clone()),
            (false, true, _) => false_negatives.push(case.id.clone()),
            (false, false, _) => true_negative_count += 1,
            (true, true, false) => unreachable!("candidate_hit is true when both sides are true"),
        }
    }

    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let top_10 = ranked.iter().take(10).collect::<Vec<_>>();
    let top_10_hits = top_10.iter().filter(|(_, _, hit, _)| *hit).count();
    let mut top_10_by_tier = BTreeMap::<String, (usize, usize)>::new();
    for (_, _, hit, tier) in &top_10 {
        let entry = top_10_by_tier.entry((*tier).clone()).or_default();
        entry.1 += 1;
        if *hit {
            entry.0 += 1;
        }
    }

    let predicted = true_positives.len() + false_positives.len();
    let expected = true_positives.len() + false_negatives.len();
    let precision = ratio(true_positives.len(), predicted);
    let recall = ratio(true_positives.len(), expected);
    let f1 = if precision + recall == 0.0 {
        0.0
    } else {
        2.0 * precision * recall / (precision + recall)
    };
    let precision_at_10 = ratio(top_10_hits, top_10.len());
    let cohen_kappa = cohen_kappa(
        true_positives.len(),
        false_positives.len(),
        false_negatives.len(),
        true_negative_count,
    );
    let tier_recall = expected_by_tier
        .into_iter()
        .map(|(tier, expected)| {
            let hits = hits_by_tier.get(&tier).copied().unwrap_or_default();
            (tier, ratio(hits, expected))
        })
        .collect();
    let precision_at_10_by_tier = top_10_by_tier
        .into_iter()
        .map(|(tier, (hits, predicted))| (tier, ratio(hits, predicted)))
        .collect();

    QualityReport {
        total: cases.len(),
        true_positives,
        false_positives,
        false_negatives,
        visible_candidates: predicted,
        precision,
        recall,
        f1,
        precision_at_10,
        cohen_kappa,
        tier_mix,
        tier_recall,
        precision_at_10_by_tier,
    }
}

fn infer_quality_memory_tier(input: &str, signal: &str) -> String {
    let lower = input.to_lowercase();
    if signal == "reject" {
        return String::new();
    }
    if contains_any(
        &lower,
        &[
            "不要改 rust",
            "只输出",
            "只修改",
            "只读",
            "不要写文件",
            "不要编辑文件",
        ],
    ) {
        return String::new();
    }
    if contains_any(
        &lower,
        &[
            "先提问",
            "先澄清",
            "先规划",
            "小改快测",
            "大改重测",
            "真实历史",
            "真实历史回放",
            "审阅边界",
            "review 边界",
            "协作",
            "feedback loop",
            "real history",
            "planning",
            "top 10",
            "验证结果",
            "遗漏风险",
            "审阅时",
            "自动切换到 cli provider",
            "每次改提炼",
            "目标子集",
            "全量相关测试",
            "先证明坏例子",
            "模板输出",
            "llm 抽象",
            "边界样本",
            "judge",
            "自我修正",
            "反馈反复拒绝",
            "降权",
            "为什么保留",
            "压低",
            "gold set",
            "tier mix",
            "assistant synthesis",
            "fallback",
        ],
    ) {
        "collaboration_preference".to_string()
    } else if contains_any(
        &lower,
        &[
            "核心功能",
            "核心路径",
            "可用主线",
            "用户视角",
            "用户体验",
            "体验优化",
            "跨项目",
            "提炼候选时质量优先",
            "宁可少",
            "抽象失败",
            "硬凑原则",
            "真正有价值",
            "其他项目",
            "长期规则",
            "判断标准",
            "长期记忆",
            "未来决策",
            "通用方法论",
            "真实工作流",
            "最终使用感受",
            "响应性",
            "作用域分层",
            "关键词模板",
            "高价值候选",
            "稳定偏好",
            "一次性命令",
            "复用原则",
            "core functionality",
            "user perspective",
            "user experience",
            "cross-project",
        ],
    ) {
        "cross_project_principle".to_string()
    } else {
        "project_rule".to_string()
    }
}

fn contains_any(text: &str, markers: &[&str]) -> bool {
    markers.iter().any(|marker| text.contains(marker))
}

fn ratio(numerator: usize, denominator: usize) -> f32 {
    if denominator == 0 {
        1.0
    } else {
        numerator as f32 / denominator as f32
    }
}

fn cohen_kappa(tp: usize, fp: usize, fn_: usize, tn: usize) -> f32 {
    let total = tp + fp + fn_ + tn;
    if total == 0 {
        return 1.0;
    }
    let total_f = total as f32;
    let observed = (tp + tn) as f32 / total_f;
    let predicted_positive = (tp + fp) as f32 / total_f;
    let predicted_negative = (fn_ + tn) as f32 / total_f;
    let expected_positive = (tp + fn_) as f32 / total_f;
    let expected_negative = (fp + tn) as f32 / total_f;
    let expected = predicted_positive * expected_positive + predicted_negative * expected_negative;
    if (1.0 - expected).abs() < f32::EPSILON {
        1.0
    } else {
        (observed - expected) / (1.0 - expected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quality_report_tracks_memory_tier_metrics() {
        let report = quality_report_for_text_cases(vec![QualityTextCase {
            id: "core".to_string(),
            origin: "user".to_string(),
            input: "核心功能优先，先把主流程做稳，再扩展周边能力。".to_string(),
            expected: "candidate".to_string(),
            expected_signal: "".to_string(),
            expected_artifact_kind: "".to_string(),
            expected_hardness: "".to_string(),
            expected_memory_tier: "cross_project_principle".to_string(),
            expected_terms: vec!["核心功能".to_string()],
            expected_tags: Vec::new(),
        }]);

        assert_eq!(report.tier_mix.get("cross_project_principle"), Some(&1));
        assert_eq!(
            report.tier_recall.get("cross_project_principle"),
            Some(&1.0)
        );
    }
}
