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
            candidate.title.to_lowercase()
        } else {
            candidate.body.to_lowercase()
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
    if left.kind != right.kind || left.scope != right.scope {
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
