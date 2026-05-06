use crate::candidate::ExtractionAction;

use super::Candidate;
use super::lifecycle::SkillletOperation;

#[derive(Debug, Clone)]
pub(crate) struct QualityGateDecision {
    pub operation: SkillletOperation,
    pub disposition: QualityDisposition,
    pub reason: String,
    pub flags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum QualityDisposition {
    Keep,
    Skip,
}

pub(crate) fn evaluate_candidate_quality(
    candidate: &Candidate,
    action: &ExtractionAction,
) -> QualityGateDecision {
    let combined = format!(
        "{}\n{}\n{}\n{}",
        candidate.title,
        candidate.body,
        candidate.evidence,
        candidate.reason.as_deref().unwrap_or_default()
    );
    let lower = combined.to_lowercase();

    if contains_internal_leak(&lower) {
        return skip(
            SkillletOperation::Noop,
            "internal-leak",
            "Candidate contains internal observation/evidence metadata instead of a reusable rule.",
        );
    }

    if looks_like_meta_discussion(&lower) {
        return skip(
            SkillletOperation::Noop,
            "meta-discussion",
            "Candidate is about planning or implementation status, not durable agent behavior.",
        );
    }

    if action.action == "merge_into_existing" && action.similarity.unwrap_or(0.0) >= 0.95 {
        return skip(
            SkillletOperation::Noop,
            "duplicate-existing",
            "Equivalent Skilllet already exists; suppressing duplicate candidate.",
        );
    }

    if action.action == "noop" {
        return skip(
            SkillletOperation::Noop,
            "llm-noop",
            action.reason.as_deref().unwrap_or(
                "LLM update phase marked this candidate as already covered or low value.",
            ),
        );
    }

    if !is_known_high_value_template(candidate) && !looks_self_contained_rule(&candidate.body) {
        return skip(
            SkillletOperation::Noop,
            "not-self-contained-rule",
            "Candidate body is not a self-contained future rule.",
        );
    }

    QualityGateDecision {
        operation: operation_from_action(action),
        disposition: QualityDisposition::Keep,
        reason: "candidate passed quality gate".to_string(),
        flags: Vec::new(),
    }
}

fn is_known_high_value_template(candidate: &Candidate) -> bool {
    matches!(
        candidate.matched_template.as_deref(),
        Some("project-improvement" | "high-value-prompt" | "atomic-exception")
    )
}

pub(crate) fn quality_skip_message(id: &str, decision: &QualityGateDecision) -> String {
    format!("{id}: {} ({})", decision.flags.join(","), decision.reason)
}

fn skip(operation: SkillletOperation, flag: &str, reason: &str) -> QualityGateDecision {
    QualityGateDecision {
        operation,
        disposition: QualityDisposition::Skip,
        reason: reason.to_string(),
        flags: vec![flag.to_string()],
    }
}

fn contains_internal_leak(lower: &str) -> bool {
    let markers = [
        "evidence:",
        "observation:",
        "observations:",
        "sha256",
        "source_observations",
        "score:",
        "matched_signal",
        "merged suggestion",
        "similarity 100",
        "相似度 100",
        "合并建议",
    ];
    markers.iter().any(|marker| lower.contains(marker))
}

fn looks_like_meta_discussion(lower: &str) -> bool {
    let question_like = lower.contains("是否")
        || lower.contains("有没有")
        || lower.contains("should we")
        || lower.contains("do we already")
        || lower.contains("have we");
    let pipeline_terms = [
        "skilllet",
        "candidate",
        "draft",
        "hook",
        "compile",
        "编译",
        "提炼",
        "规划",
        "计划",
        "实现",
    ];
    question_like && pipeline_terms.iter().any(|term| lower.contains(term))
}

fn looks_self_contained_rule(body: &str) -> bool {
    let trimmed = body.trim();
    if trimmed.len() < 10 {
        return false;
    }
    if trimmed.ends_with('?') || trimmed.ends_with('？') {
        return false;
    }
    let lower = trimmed.to_lowercase();
    let rule_markers = [
        "use ",
        "prefer ",
        "always ",
        "never ",
        "must ",
        "do not ",
        "don't ",
        "keep ",
        "avoid ",
        "default to ",
        "默认",
        "统一",
        "优先",
        "必须",
        "不要",
        "禁止",
        "保留",
        "先",
        "走",
        "用",
        "使用",
        "采用",
        "选用",
        "改用",
        "替代",
        "而不是",
        "仅当",
        "只有",
        "如果",
    ];
    if rule_markers.iter().any(|marker| lower.contains(marker)) {
        return true;
    }

    let tool_markers = [
        "ky", "pnpm", "bun", "axios", "ofetch", "biome", "oxlint", "vitest", "tanstack", "turbo",
    ];
    let durable_markers = ["以后", "默认", "统一", "prefer", "use", "must", "always"];
    tool_markers.iter().any(|tool| lower.contains(tool))
        && durable_markers.iter().any(|marker| lower.contains(marker))
}

fn operation_from_action(action: &ExtractionAction) -> SkillletOperation {
    match action.action.as_str() {
        "merge_into_existing" => SkillletOperation::Update,
        "conflict" => SkillletOperation::Conflict,
        "supersede" => SkillletOperation::Supersede,
        "noop" => SkillletOperation::Noop,
        _ => SkillletOperation::Add,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(title: &str, body: &str) -> Candidate {
        Candidate {
            title: title.to_string(),
            body: body.to_string(),
            kind: "preference".to_string(),
            scope: "project".to_string(),
            evidence: body.to_string(),
            confidence: Some(0.9),
            reason: None,
            matched_template: None,
        }
    }

    #[test]
    fn rejects_internal_evidence_leaks() {
        let decision = evaluate_candidate_quality(
            &candidate(
                "evidence: 81 observations",
                "observation:obs:claude-code:sha256:abc - Do NOT mention teammate proposals.",
            ),
            &ExtractionAction::new_candidate(),
        );

        assert_eq!(decision.disposition, QualityDisposition::Skip);
        assert_eq!(decision.flags, vec!["internal-leak"]);
    }

    #[test]
    fn rejects_meta_discussion_questions() {
        let decision = evaluate_candidate_quality(
            &candidate(
                "是否已经把 Skilllet 编译为 hook",
                "是否已经把 Skilllet 编译为 hook，例如 git commit 前运行 cargo clippy？",
            ),
            &ExtractionAction::new_candidate(),
        );

        assert_eq!(decision.disposition, QualityDisposition::Skip);
        assert_eq!(decision.flags, vec!["meta-discussion"]);
    }

    #[test]
    fn suppresses_near_exact_duplicate_existing_skilllets() {
        let decision = evaluate_candidate_quality(
            &candidate(
                "Prefer Bun",
                "Use Bun for JavaScript package management and scripts.",
            ),
            &ExtractionAction::merge_into_existing("project:prefer-bun".to_string(), 0.99),
        );

        assert_eq!(decision.disposition, QualityDisposition::Skip);
        assert_eq!(decision.operation, SkillletOperation::Noop);
        assert_eq!(decision.flags, vec!["duplicate-existing"]);
    }
}
