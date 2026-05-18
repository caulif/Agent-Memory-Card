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

/// Memory Card text quality input for local, deterministic checks.
#[derive(Debug, Clone)]
pub struct CardQualityInput<'a> {
    pub title: &'a str,
    pub body: &'a str,
    pub brief: &'a str,
    pub kind: &'a str,
    pub scope: &'a str,
    pub evidence_quotes: Vec<&'a str>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CardQualityReport {
    pub passed: bool,
    pub scores: CardQualityScores,
    pub failures: Vec<CardQualityFailure>,
}

impl CardQualityReport {
    pub fn total_score(&self) -> u32 {
        self.scores.clarity
            + self.scores.actionability
            + self.scores.evidence_grounding
            + self.scores.abstraction_fit
            + self.scores.boundary_quality
            + self.scores.field_consistency
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CardQualityScores {
    pub clarity: u32,
    pub actionability: u32,
    pub evidence_grounding: u32,
    pub abstraction_fit: u32,
    pub boundary_quality: u32,
    pub field_consistency: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum CardQualityFailure {
    WeakEvidence,
    VagueAction,
    MissingBoundary,
    AbstractionTooLow,
    AbstractionTooHigh,
    FieldMismatch,
    BriefDuplicatesBody,
    UnsupportedWhy,
    TemplateSmell,
    PipelineMeta,
}

pub fn quality_report_for_card(input: CardQualityInput<'_>) -> CardQualityReport {
    let title = input.title.trim();
    let body = input.body.trim();
    let brief = input.brief.trim();
    let kind = input.kind.trim();
    let scope = input.scope.trim();
    let evidence_text = input.evidence_quotes.join("\n");
    let mut failures = Vec::new();

    if input.evidence_quotes.is_empty()
        || input
            .evidence_quotes
            .iter()
            .all(|quote| quote.trim().chars().count() < 8)
    {
        failures.push(CardQualityFailure::WeakEvidence);
    }
    if is_vague_action(body) {
        failures.push(CardQualityFailure::VagueAction);
    }
    if !has_boundary_language(body) {
        failures.push(CardQualityFailure::MissingBoundary);
    }
    if looks_like_low_level_detail(body) {
        failures.push(CardQualityFailure::AbstractionTooLow);
    }
    if looks_like_over_abstract(body) {
        failures.push(CardQualityFailure::AbstractionTooHigh);
    }
    if field_mismatch(kind, scope, body) {
        failures.push(CardQualityFailure::FieldMismatch);
    }
    if char_jaccard_for_quality(brief, body) >= 0.62 {
        failures.push(CardQualityFailure::BriefDuplicatesBody);
    }
    if unsupported_by_evidence(body, brief, &evidence_text) {
        failures.push(CardQualityFailure::UnsupportedWhy);
    }
    if has_template_smell(body) {
        failures.push(CardQualityFailure::TemplateSmell);
    }
    if looks_like_pipeline_meta_card(title, body, brief) {
        failures.push(CardQualityFailure::PipelineMeta);
    }
    failures.sort_by_key(|failure| format!("{failure:?}"));
    failures.dedup();

    let scores = CardQualityScores {
        clarity: score_clear_text(title, body),
        actionability: if failures.contains(&CardQualityFailure::VagueAction) {
            2
        } else {
            5
        },
        evidence_grounding: if failures.contains(&CardQualityFailure::UnsupportedWhy) {
            1
        } else if failures.contains(&CardQualityFailure::WeakEvidence) {
            2
        } else {
            5
        },
        abstraction_fit: if failures.contains(&CardQualityFailure::AbstractionTooLow)
            || failures.contains(&CardQualityFailure::AbstractionTooHigh)
        {
            2
        } else {
            5
        },
        boundary_quality: if failures.contains(&CardQualityFailure::MissingBoundary) {
            1
        } else {
            5
        },
        field_consistency: if failures.contains(&CardQualityFailure::FieldMismatch) {
            2
        } else {
            5
        },
    };
    let passed = scores.total() >= 24
        && !failures.iter().any(|failure| {
            matches!(
                failure,
                CardQualityFailure::WeakEvidence
                    | CardQualityFailure::VagueAction
                    | CardQualityFailure::MissingBoundary
                    | CardQualityFailure::FieldMismatch
                    | CardQualityFailure::UnsupportedWhy
                    | CardQualityFailure::PipelineMeta
            )
        });

    CardQualityReport {
        passed,
        scores,
        failures,
    }
}

impl<'a> From<&'a super::crystallize::CrystallizedCard> for CardQualityInput<'a> {
    fn from(card: &'a super::crystallize::CrystallizedCard) -> Self {
        Self {
            title: &card.title,
            body: &card.body,
            brief: &card.brief,
            kind: &card.kind,
            scope: &card.scope,
            evidence_quotes: card
                .evidence_quotes
                .iter()
                .map(|quote| quote.text.as_str())
                .collect(),
        }
    }
}

pub fn quality_report_for_crystallized_card(
    card: &super::crystallize::CrystallizedCard,
) -> CardQualityReport {
    quality_report_for_card(CardQualityInput::from(card))
}

impl CardQualityScores {
    fn total(&self) -> u32 {
        self.clarity
            + self.actionability
            + self.evidence_grounding
            + self.abstraction_fit
            + self.boundary_quality
            + self.field_consistency
    }
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

fn is_vague_action(body: &str) -> bool {
    let lower = body.to_lowercase();
    let vague_markers = [
        "提高质量",
        "提高生成质量",
        "更好",
        "优化效果",
        "做好",
        "处理好",
        "注意",
        "合理",
        "改进",
        "improve quality",
        "make it better",
    ];
    contains_any(&lower, &vague_markers)
        && !contains_any(
            &lower,
            &[
                "先", "必须", "不要", "不得", "只", "记录", "区分", "校验", "回退", "拒绝",
                "rewrite", "validate",
            ],
        )
}

fn has_boundary_language(body: &str) -> bool {
    let lower = body.to_lowercase();
    contains_any(
        &lower,
        &[
            "不写入",
            "不进入",
            "不能",
            "不要",
            "不得",
            "只用于",
            "仅用于",
            "例外",
            "边界",
            "除非",
            "回退",
            "禁止",
            "not ",
            "only ",
            "unless",
            "fallback",
        ],
    )
}

fn looks_like_low_level_detail(body: &str) -> bool {
    let lower = body.to_lowercase();
    contains_any(
        &lower,
        &[
            "storage_key",
            "thresholds:",
            "enum",
            "0/10/50/200",
            "按钮颜色",
            "字号",
            "端口号",
            "字段名",
            "css",
        ],
    )
}

fn looks_like_over_abstract(body: &str) -> bool {
    let lower = body.to_lowercase();
    contains_any(
        &lower,
        &[
            "始终保持高质量",
            "追求卓越",
            "持续优化一切",
            "用心做好",
            "be excellent",
            "always improve everything",
        ],
    )
}

fn field_mismatch(kind: &str, scope: &str, body: &str) -> bool {
    let lower = body.to_lowercase();
    let kind_mismatch = match kind {
        "constraint" => contains_any(&lower, &["偏好", "更喜欢", "倾向", "prefer"]),
        "preference" => contains_any(
            &lower,
            &["必须", "禁止", "不得", "不能", "must", "forbidden"],
        ),
        "procedure" => !contains_any(
            &lower,
            &[
                "先", "再", "步骤", "流程", "运行", "校验", "记录", "区分", "when", "then",
            ],
        ),
        _ => true,
    };
    let scope_mismatch =
        scope == "project" && contains_any(&lower, &["所有项目", "跨项目", "global", "通用"]);
    kind_mismatch || scope_mismatch || !matches!(scope, "global" | "project")
}

fn unsupported_by_evidence(body: &str, brief: &str, evidence: &str) -> bool {
    if evidence.trim().is_empty() {
        return true;
    }
    let claim = normalize_quality_text(&format!("{body}\n{brief}"));
    let evidence = normalize_quality_text(evidence);
    let risky_claims = [
        "上传到github",
        "写入git",
        "共享完整语料",
        "进入生产链路",
        "runtime参考",
    ];
    risky_claims.iter().any(|claim_marker| {
        claim.contains(claim_marker)
            && !has_negated_boundary(&claim, claim_marker)
            && !evidence.contains(claim_marker)
    })
}

fn has_negated_boundary(text: &str, marker: &str) -> bool {
    ["不", "禁止", "不得", "不能", "不要", "只用于", "仅用于"]
        .iter()
        .any(|prefix| text.contains(&format!("{prefix}{marker}")))
}

fn has_template_smell(body: &str) -> bool {
    let normalized = body.trim();
    (normalized.starts_with("当") || normalized.starts_with("在"))
        && normalized.contains("；目标是")
        && normalized.chars().count() < 65
}

fn looks_like_pipeline_meta_card(title: &str, body: &str, brief: &str) -> bool {
    let lower = format!("{title}\n{body}\n{brief}").to_lowercase();
    let generation_surface = [
        "prompt",
        "render",
        "rewrite",
        "llm",
        "模型",
        "卡片",
        "最终卡片",
        "成品卡片",
        "改写器",
        "memory card",
        "candidate",
        "候选",
        "生成",
        "提炼",
        "筛选",
    ]
    .iter()
    .filter(|marker| lower.contains(**marker))
    .count()
        >= 2;
    let eval_surface = [
        "抽样",
        "最终卡片",
        "分数",
        "指标",
        "人工看",
        "人工审阅",
        "质量",
        "评估",
        "golden",
        "eval",
        "score",
        "metric",
        "sample",
    ]
    .iter()
    .filter(|marker| lower.contains(**marker))
    .count()
        >= 2;

    generation_surface && eval_surface
}

fn score_clear_text(title: &str, body: &str) -> u32 {
    if title.chars().count() < 8 || body.chars().count() < 40 {
        2
    } else if looks_like_over_abstract(body) {
        3
    } else {
        5
    }
}

fn char_jaccard_for_quality(a: &str, b: &str) -> f32 {
    let normalize = |s: &str| -> std::collections::BTreeSet<char> {
        s.chars()
            .filter(|c| c.is_alphanumeric() || ('\u{4E00}'..='\u{9FFF}').contains(c))
            .flat_map(|c| c.to_lowercase())
            .collect()
    };
    let set_a = normalize(a);
    let set_b = normalize(b);
    if set_a.is_empty() && set_b.is_empty() {
        return 1.0;
    }
    if set_a.is_empty() || set_b.is_empty() {
        return 0.0;
    }
    let intersection = set_a.intersection(&set_b).count();
    let union = set_a.union(&set_b).count();
    intersection as f32 / union as f32
}

fn normalize_quality_text(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric() || ('\u{4E00}'..='\u{9FFF}').contains(c))
        .flat_map(|c| c.to_lowercase())
        .collect()
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

    #[test]
    fn card_quality_report_accepts_grounded_actionable_card() {
        let report = quality_report_for_card(CardQualityInput {
            title: "生成卡片前先确认隐私边界",
            body: "在优化 Memory Card 生成链路时，先区分合成测试、脱敏样例和本地私有评估；真实对话只用于本地聚合评估，不写入 Git 或 GitHub，避免把私人数据变成运行时参考。",
            brief: "用于保护真实对话只留在本地评估中。",
            kind: "constraint",
            scope: "global",
            evidence_quotes: vec![
                "真实对话数据只用于本地效果评估，不进入 Git/GitHub，也不进入生产链路。",
                "可提交测试集只能使用合成或人工脱敏样例，不能包含私人对话原文。",
            ],
        });

        assert!(report.passed, "{report:#?}");
        assert!(report.total_score() >= 24, "{report:#?}");
        assert!(report.failures.is_empty(), "{report:#?}");
    }

    #[test]
    fn card_quality_report_flags_missing_boundary_and_template_smell() {
        let report = quality_report_for_card(CardQualityInput {
            title: "提高记忆卡片质量",
            body: "当优化记忆卡片时，提高生成质量；目标是更好。",
            brief: "用于更好。",
            kind: "procedure",
            scope: "global",
            evidence_quotes: vec!["生成器需要把空泛候选改成可执行、有边界的 Memory Card。"],
        });

        assert!(!report.passed);
        assert!(
            report
                .failures
                .contains(&CardQualityFailure::MissingBoundary)
        );
        assert!(report.failures.contains(&CardQualityFailure::VagueAction));
        assert!(report.failures.contains(&CardQualityFailure::TemplateSmell));
    }

    #[test]
    fn card_quality_report_rejects_generation_quality_acceptance_chatter() {
        let report = quality_report_for_card(CardQualityInput {
            title: "抽样查看最终卡片质量",
            body: "修改卡片改写器时，必须抽样审阅最终卡片，不要只看质量分。",
            brief: "用于避免只看指标。",
            kind: "procedure",
            scope: "project",
            evidence_quotes: vec!["修改卡片改写器时，必须抽样审阅最终卡片，不要只看质量分。"],
        });

        assert!(!report.passed);
        assert!(report.failures.contains(&CardQualityFailure::PipelineMeta));
    }

    #[test]
    fn card_quality_report_flags_unsupported_why_and_field_mismatch() {
        let report = quality_report_for_card(CardQualityInput {
            title: "真实数据不能进入仓库",
            body: "在处理真实对话评估时，必须把原文上传到 GitHub 方便回归；目的是让团队共享完整语料。",
            brief: "用于共享完整真实语料。",
            kind: "preference",
            scope: "project",
            evidence_quotes: vec!["真实对话数据只用于本地效果评估，不进入 Git/GitHub。"],
        });

        assert!(!report.passed);
        assert!(
            report
                .failures
                .contains(&CardQualityFailure::UnsupportedWhy)
        );
        assert!(report.failures.contains(&CardQualityFailure::FieldMismatch));
    }

    #[test]
    fn card_quality_report_can_score_crystallized_card() {
        let card = super::super::crystallize::CrystallizedCard {
            cluster_id: "c-quality".to_string(),
            recurrence: 2,
            title: "生成卡片前先确认隐私边界".to_string(),
            body: "在优化 Memory Card 生成链路时，先区分合成测试、脱敏样例和本地私有评估；真实对话只用于本地聚合评估，不写入 Git 或 GitHub，避免把私人数据变成运行时参考。"
                .to_string(),
            brief: "用于保护真实对话只留在本地评估中。".to_string(),
            kind: "constraint".to_string(),
            scope: "global".to_string(),
            activation: "always-on".to_string(),
            tags: Vec::new(),
            language: "zh-CN".to_string(),
            evidence_quotes: vec![super::super::induce::EvidenceQuote {
                observation_id: "synthetic-privacy-1".to_string(),
                text: "真实对话数据只用于本地效果评估，不进入 Git/GitHub，也不进入生产链路。"
                    .to_string(),
            }],
            temporal_status: "stable".to_string(),
            confidence: 0.9,
        };

        let report = quality_report_for_crystallized_card(&card);

        assert!(report.passed, "{report:#?}");
    }
}
