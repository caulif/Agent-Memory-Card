use super::Candidate;
use crate::textutil;

pub(super) fn dedupe_candidates(candidates: Vec<Candidate>) -> Vec<Candidate> {
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for candidate in candidates {
        let key = if matches!(
            candidate.matched_template.as_deref(),
            Some("project-improvement" | "high-value-prompt")
        ) {
            format!(
                "{}:{}",
                candidate.memory_tier.as_str(),
                candidate.title.to_lowercase()
            )
        } else {
            format!(
                "{}:{}",
                candidate.memory_tier.as_str(),
                candidate.body.to_lowercase()
            )
        };
        if !seen.insert(key) {
            continue;
        }

        if let Some(existing_index) = out
            .iter()
            .position(|existing| are_similar_candidates(existing, &candidate))
        {
            if candidate_rank(&candidate) > candidate_rank(&out[existing_index]) {
                out[existing_index] = candidate;
            }
        } else {
            out.push(candidate);
        }
    }
    out
}

fn are_similar_candidates(left: &Candidate, right: &Candidate) -> bool {
    if left.kind != right.kind {
        return false;
    }
    if same_semantic_domain(left, right) {
        return true;
    }
    if left.scope != right.scope {
        return false;
    }
    textutil::jaccard_similarity(&left.body, &right.body) >= 0.5
        || textutil::jaccard_similarity(&left.title, &right.title) >= 0.8
}

fn candidate_rank(candidate: &Candidate) -> (u8, u8, u16) {
    let confidence = (candidate.confidence.unwrap_or(0.0) * 1000.0).round() as u16;
    let template = u8::from(candidate.matched_template.is_some());
    let reason = u8::from(candidate.reason.is_some());
    (template, reason, confidence)
}

fn same_semantic_domain(left: &Candidate, right: &Candidate) -> bool {
    let left_domain = semantic_domain(left);
    !left_domain.is_empty() && left_domain == semantic_domain(right)
}

fn semantic_domain(candidate: &Candidate) -> &'static str {
    let lower = format!("{}\n{}", candidate.title, candidate.body).to_lowercase();
    if contains_any(&lower, &["真实历史", "real history", "静态样例"]) {
        "real-history"
    } else if contains_any(
        &lower,
        &["review", "merge", "审阅边界", "人工审阅", "固化规则"],
    ) {
        "review-boundary"
    } else if contains_any(&lower, &["候选质量", "候选数量"]) {
        "candidate-quality"
    } else if contains_any(&lower, &["持续自我修正", "自我修正", "真实反馈"]) {
        "self-correction"
    } else {
        ""
    }
}

fn contains_any(text: &str, markers: &[&str]) -> bool {
    markers.iter().any(|marker| text.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::candidate::MemoryTier;

    fn candidate(
        title: &str,
        body: &str,
        kind: &str,
        scope: &str,
        tier: MemoryTier,
        confidence: f32,
    ) -> Candidate {
        Candidate {
            title: title.to_string(),
            body: body.to_string(),
            kind: kind.to_string(),
            scope: scope.to_string(),
            memory_tier: tier,
            abstraction_of: None,
            abstracted_from: None,
            evidence: body.to_string(),
            confidence: Some(confidence),
            reason: None,
            matched_template: Some("principle-signal".to_string()),
        }
    }

    #[test]
    fn merges_same_real_history_domain_across_tiers() {
        let candidates = dedupe_candidates(vec![
            candidate(
                "评估提炼优先用真实历史会话做回归验证",
                "评估提炼质量时优先用真实历史会话做回归验证",
                "procedure",
                "global",
                MemoryTier::CrossProjectPrinciple,
                0.80,
            ),
            candidate(
                "再次强调用真实历史会话做回归检验",
                "再次强调用真实历史会话做回归检验",
                "procedure",
                "global",
                MemoryTier::CollaborationPreference,
                0.78,
            ),
            candidate(
                "改提炼逻辑前必须用真实历史跑一遍",
                "改提炼逻辑前必须用真实历史跑一遍，别只用静态样例",
                "procedure",
                "project",
                MemoryTier::ProjectRule,
                0.74,
            ),
        ]);

        assert_eq!(candidates.len(), 1, "{candidates:#?}");
        assert!(candidates[0].body.contains("真实历史"));
    }

    #[test]
    fn keeps_distinct_semantic_domains() {
        let candidates = dedupe_candidates(vec![
            candidate(
                "评估提炼优先用真实历史会话做回归验证",
                "评估提炼质量时优先用真实历史会话做回归验证",
                "procedure",
                "global",
                MemoryTier::CrossProjectPrinciple,
                0.80,
            ),
            candidate(
                "先 review 再 merge 保留人工审阅边界",
                "先 review 再 merge，保留人工审阅边界，不要让 AI 直接固化规则。",
                "procedure",
                "global",
                MemoryTier::CollaborationPreference,
                0.78,
            ),
        ]);

        assert_eq!(candidates.len(), 2, "{candidates:#?}");
    }
}
