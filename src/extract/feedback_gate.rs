use std::path::Path;

use anyhow::Result;

use crate::feedback;

use super::{Candidate, embedding};

pub(super) fn apply_candidate_feedback(
    project_root: &Path,
    candidate_id: &str,
    candidate: &mut Candidate,
) -> Result<Option<String>> {
    let Some(penalty) =
        feedback::feedback_penalty_for_body(project_root, "candidate", &candidate.body)?
    else {
        return Ok(None);
    };
    let reason = format!(
        "feedback-rejected: {} repeated rejections ({})",
        penalty.rejections, penalty.reason
    );
    if penalty.rejections >= 3 {
        return Ok(Some(format!("{candidate_id}: {reason}")));
    }
    let confidence = candidate.confidence.unwrap_or(0.7);
    candidate.confidence = Some((confidence + penalty.confidence_delta).clamp(0.0, 1.0));
    candidate.reason = Some(match candidate.reason.take() {
        Some(existing) => format!("{existing}; {reason}"),
        None => reason,
    });
    Ok(None)
}

pub(super) fn skip_body_from_feedback(
    project_root: &Path,
    candidate_id: &str,
    body: &str,
) -> Result<Option<String>> {
    let Some(penalty) = feedback::feedback_penalty_for_body(project_root, "candidate", body)?
    else {
        return Ok(None);
    };
    if penalty.rejections >= 3 {
        return Ok(Some(format!(
            "{candidate_id}: feedback-rejected: {} repeated rejections ({})",
            penalty.rejections, penalty.reason
        )));
    }
    Ok(None)
}

pub(super) fn apply_llm_item_feedback(
    project_root: &Path,
    candidate_id: &str,
    item: &mut embedding::LlmKnowledgeItem,
) -> Result<Option<String>> {
    let Some(penalty) = feedback::feedback_penalty_for_candidate_body(
        project_root,
        "candidate",
        &item.scope,
        &item.memory_tier,
        &item.body,
    )?
    else {
        return Ok(None);
    };
    let reason = format!(
        "feedback-rejected: {} repeated rejections ({})",
        penalty.rejections, penalty.reason
    );
    if penalty.rejections >= 3 {
        return Ok(Some(format!("{candidate_id}: {reason}")));
    }
    item.confidence = (item.confidence + penalty.confidence_delta).clamp(0.0, 1.0);
    item.reason = format!("{}; {reason}", item.reason);
    Ok(None)
}
