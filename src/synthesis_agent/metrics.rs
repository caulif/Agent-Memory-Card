use super::{SynthesisProposal, SynthesisReviewMetrics, SynthesisStopReason, skill_eval};

pub(super) fn metrics_for(
    proposal: &SynthesisProposal,
    stop_reason: &SynthesisStopReason,
) -> SynthesisReviewMetrics {
    let no_card_decision = matches!(proposal.action.as_str(), "already_covered" | "ignore")
        || *stop_reason == SynthesisStopReason::NeedsHuman;
    SynthesisReviewMetrics {
        no_card_decision,
        merge_recommended: proposal.action == "merge_card",
        approval_candidate: matches!(
            proposal.action.as_str(),
            "new_card" | "workflow_card" | "skill_targeted_card" | "merge_card"
        ),
        duplicate_suppressed: proposal.action == "already_covered",
        counterfactual_pass: skill_eval::is_counterfactual_pass(proposal.skill_usefulness.as_ref()),
        needs_human: *stop_reason == SynthesisStopReason::NeedsHuman,
    }
}
