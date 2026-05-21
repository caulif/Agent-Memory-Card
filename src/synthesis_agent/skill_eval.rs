use super::{SkillMatch, SkillUsefulnessEvaluation, cap_score, lexical_score};
use crate::candidate::CandidateRecord;

const AXES: [&str; 4] = ["trigger", "instruction", "boundary", "acceptance"];

pub(super) fn evaluate(
    candidate: &CandidateRecord,
    skill: Option<&SkillMatch>,
) -> Option<SkillUsefulnessEvaluation> {
    let skill = skill.filter(|matched| matched.project_level)?;
    let candidate_text = candidate_text(candidate);
    let skill_text = format!("{} {}", skill.name, skill.description);
    let candidate_axes = axes_for(&candidate_text);
    let skill_axes = axes_for(&skill_text);
    let improved_axes = candidate_axes
        .iter()
        .filter(|axis| !skill_axes.contains(axis))
        .cloned()
        .collect::<Vec<_>>();
    let missing_axes = AXES
        .iter()
        .filter(|axis| {
            !candidate_axes
                .iter()
                .any(|candidate_axis| candidate_axis == *axis)
        })
        .map(|axis| (*axis).to_string())
        .collect::<Vec<_>>();
    let restatement_score = lexical_score(&candidate_text, &skill_text);
    let score = skill_usefulness_score(&improved_axes, &missing_axes, restatement_score);
    let verdict = if improved_axes.is_empty() || score < 0.35 {
        "no_meaningful_change"
    } else {
        "counterfactual_pass"
    };
    Some(SkillUsefulnessEvaluation {
        target_skill_id: skill.id.clone(),
        before_behavior: format!(
            "Before: Skill `{}` covers {}.",
            skill.name,
            empty_as(&skill.description, "no explicit behavior")
        ),
        after_behavior: after_behavior(candidate, &improved_axes),
        improved_axes,
        missing_axes,
        verdict: verdict.to_string(),
        score,
    })
}

pub(super) fn is_counterfactual_pass(evaluation: Option<&SkillUsefulnessEvaluation>) -> bool {
    evaluation.is_some_and(|value| value.verdict == "counterfactual_pass" && value.score >= 0.35)
}

pub(super) fn event_summary(evaluation: &SkillUsefulnessEvaluation) -> String {
    if evaluation.verdict == "counterfactual_pass" {
        format!(
            "Counterfactual pass for `{}`: after proposal improves {} (score {:.2}).",
            evaluation.target_skill_id,
            joined_axes(&evaluation.improved_axes),
            evaluation.score
        )
    } else {
        format!(
            "Counterfactual did not prove useful Skill delta for `{}`; missing {} and score {:.2}.",
            evaluation.target_skill_id,
            joined_axes(&evaluation.missing_axes),
            evaluation.score
        )
    }
}

fn skill_usefulness_score(
    improved_axes: &[String],
    missing_axes: &[String],
    restatement_score: f32,
) -> f32 {
    let improvement = improved_axes.len() as f32 * 0.24;
    let completeness = (AXES.len() - missing_axes.len()) as f32 * 0.08;
    let restatement_penalty = if improved_axes.is_empty() && restatement_score >= 0.72 {
        0.28
    } else {
        0.0
    };
    cap_score(0.16 + improvement + completeness - restatement_penalty)
}

fn axes_for(text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    let mut axes = Vec::new();
    if has_any(
        &lower,
        &[
            "use when",
            "when ",
            "trigger",
            "触发",
            "适用场景",
            "场景",
            "当",
            "遇到",
        ],
    ) {
        axes.push("trigger".to_string());
    }
    if has_any(
        &lower,
        &[
            "instruction",
            "instructions",
            "do:",
            "action",
            "动作",
            "执行",
            "步骤",
            "必须",
            "应当",
            "先",
            "做",
            "补强",
            "优化",
            "流程",
        ],
    ) {
        axes.push("instruction".to_string());
    }
    if has_any(
        &lower,
        &[
            "boundary",
            "boundaries",
            "unless",
            "avoid",
            "不要",
            "不能",
            "避免",
            "边界",
            "只在",
            "而不是",
            "不是",
        ],
    ) {
        axes.push("boundary".to_string());
    }
    if has_any(
        &lower,
        &[
            "acceptance",
            "verify",
            "check",
            "test",
            "验收",
            "验证",
            "检查",
            "测试",
            "确认",
            "通过",
        ],
    ) {
        axes.push("acceptance".to_string());
    }
    axes
}

fn candidate_text(candidate: &CandidateRecord) -> String {
    [
        candidate.title.as_str(),
        candidate.body.as_str(),
        candidate.brief.as_str(),
        candidate.evidence.as_str(),
        &candidate.tags.join(" "),
    ]
    .join(" ")
}

fn after_behavior(candidate: &CandidateRecord, improved_axes: &[String]) -> String {
    if improved_axes.is_empty() {
        return format!(
            "After: proposal for `{}` does not add a trigger, instruction, boundary, or acceptance check beyond the target Skill.",
            candidate.title
        );
    }
    format!(
        "After: proposal for `{}` adds {} to the target Skill behavior.",
        candidate.title,
        joined_axes(improved_axes)
    )
}

fn joined_axes(values: &[String]) -> String {
    if values.is_empty() {
        "no axes".to_string()
    } else {
        values.join(", ")
    }
}

fn has_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn empty_as(value: &str, fallback: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}
