use super::{
    MemoryCardMatch, SkillMatch, cap_score, empty_as, mentions_skill, mentions_update_intent,
};
use crate::candidate::CandidateRecord;
use crate::config::SkillRecord;
use crate::memory_card::MemoryCardRecord;

pub(super) fn memory_card_event_summary(matches: &[MemoryCardMatch]) -> String {
    let Some(top) = matches.first() else {
        return "Compared 0 Memory Card match(es); no local or global card looked related."
            .to_string();
    };
    format!(
        "Compared {} Memory Card match(es); top `{}` is `{}` with score {:.2}. {} {}",
        matches.len(),
        top.id,
        top.merge_hint,
        top.score,
        top.overlap_summary,
        top.gap_summary
    )
}

pub(super) fn skill_event_summary(matches: &[SkillMatch]) -> String {
    let Some(top) = matches.first() else {
        return "Checked 0 Skill match(es); no Skill target or reference looked related."
            .to_string();
    };
    format!(
        "Checked {} Skill match(es); top `{}` is `{}`. {} {}",
        matches.len(),
        top.id,
        top.target_role,
        top.coverage_summary,
        top.gap_summary
    )
}

pub(super) fn memory_overlap_summary(
    card: &MemoryCardRecord,
    scope: &str,
    score: f32,
    duplicate: bool,
) -> String {
    let relation = if duplicate {
        "strongly overlaps"
    } else {
        "partially overlaps"
    };
    format!(
        "Memory Card `{}` ({scope}) {relation} via title/body/tags comparison (score {:.2}).",
        card.id,
        cap_score(score)
    )
}

pub(super) fn memory_gap_summary(
    candidate: &CandidateRecord,
    card: &MemoryCardRecord,
    scope: &str,
    duplicate: bool,
) -> String {
    if scope != "project" {
        return "Global Memory Cards are comparison references, not direct merge targets."
            .to_string();
    }
    if duplicate && mentions_update_intent(candidate) {
        return format!(
            "Candidate appears to update `{}`; merge should preserve any new trigger, boundary, or acceptance rule.",
            card.id
        );
    }
    if duplicate {
        return format!(
            "`{}` already covers the candidate closely; a new card would add clutter unless new evidence changes the boundary.",
            card.id
        );
    }
    format!(
        "`{}` is related but not enough to prove coverage; use it to avoid repeating existing wording.",
        card.id
    )
}

pub(super) fn memory_merge_hint(
    candidate: &CandidateRecord,
    scope: &str,
    duplicate: bool,
) -> String {
    if scope != "project" {
        return "reference_only".to_string();
    }
    if duplicate && mentions_update_intent(candidate) {
        "merge_candidate".to_string()
    } else if duplicate {
        "already_covered_candidate".to_string()
    } else {
        "related_reference".to_string()
    }
}

pub(super) fn skill_coverage_summary(skill: &SkillRecord, score: f32) -> String {
    format!(
        "Skill `{}` currently covers: {} (match {:.2}).",
        skill.name,
        empty_as(&skill.description, "no description"),
        cap_score(score)
    )
}

pub(super) fn skill_gap_summary(
    candidate: &CandidateRecord,
    skill: &SkillRecord,
    project_level: bool,
) -> String {
    if !project_level {
        return format!(
            "`{}` is a global/reference Skill, so it can inform wording but should not receive mounted project Memory Cards by default.",
            skill.name
        );
    }
    if mentions_update_intent(candidate) || mentions_skill(candidate) {
        return format!(
            "Project Skill `{}` can receive this Memory Card if the proposal adds a missing trigger, instruction, boundary, or acceptance check.",
            skill.name
        );
    }
    format!(
        "Project Skill `{}` is related, but the candidate still needs a concrete Skill gap before direct mounting.",
        skill.name
    )
}
