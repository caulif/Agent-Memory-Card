use std::collections::BTreeMap;

use super::*;

pub(super) fn candidate_id_from_scope_and_title(scope: &str, title: &str) -> String {
    let prefix = match scope {
        "global" => "global",
        "agent" => "agent",
        _ => "project",
    };
    format!("{prefix}:{}", textutil::slug(title))
}

pub(super) fn candidate_value_scores(candidate: &Candidate) -> BTreeMap<String, f32> {
    candidate_value_scores_with_origin(candidate, None)
}

pub(super) fn candidate_value_scores_with_origin(
    candidate: &Candidate,
    origin: Option<chunk::ChunkOrigin>,
) -> BTreeMap<String, f32> {
    let lower = candidate.body.to_lowercase();
    let mut scores = BTreeMap::new();
    let durability = if lower.contains("always")
        || lower.contains("以后")
        || lower.contains("默认")
        || lower.contains("每次")
        || lower.contains("prefer")
        || lower.contains("must")
        || lower.contains("should")
    {
        0.92
    } else {
        0.72
    };
    let reusability = match candidate.memory_tier {
        MemoryTier::ProjectRule => {
            if lower.contains("all projects")
                || lower.contains("所有项目")
                || lower.contains("用户视角")
                || lower.contains("core functionality")
                || lower.contains("核心功能")
            {
                0.82
            } else {
                0.42
            }
        }
        MemoryTier::CrossProjectPrinciple => 0.94,
        MemoryTier::CollaborationPreference => 0.88,
    };
    let specificity = if candidate.body.len() >= 24 && candidate.body.len() <= 220 {
        0.82
    } else {
        0.58
    };
    let recurrence = if candidate
        .reason
        .as_deref()
        .is_some_and(|reason| reason.contains("Recurring across"))
    {
        0.92
    } else {
        0.40
    };
    let source_trust = match origin {
        Some(chunk::ChunkOrigin::User) => 1.0,
        Some(chunk::ChunkOrigin::Assistant) | Some(chunk::ChunkOrigin::AiSynthesis) => 0.60,
        Some(chunk::ChunkOrigin::Artifact) => 0.70,
        Some(chunk::ChunkOrigin::Unknown) => 0.80,
        None => 1.0,
    };
    let locality = match candidate.memory_tier {
        MemoryTier::ProjectRule => 0.28,
        MemoryTier::CrossProjectPrinciple => 0.94,
        MemoryTier::CollaborationPreference => 0.86,
    };
    scores.insert("durability".to_string(), durability);
    scores.insert("reusability".to_string(), reusability);
    scores.insert("specificity".to_string(), specificity);
    scores.insert("recurrence".to_string(), recurrence);
    scores.insert("source_trust".to_string(), source_trust);
    scores.insert("locality".to_string(), locality);
    scores
}

pub(super) fn candidate_selection_score(candidate: &Candidate) -> f32 {
    let base = candidate.confidence.unwrap_or(0.0);
    let value_scores = candidate_value_scores(candidate);
    base + value_scores.values().sum::<f32>() / value_scores.len() as f32
}

pub(super) fn llm_item_selection_score(item: &embedding::LlmKnowledgeItem) -> f32 {
    let rubric = [
        item.durability_score,
        item.reusability_score,
        item.specificity_score,
        item.source_trust_score,
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    if rubric.is_empty() {
        return candidate_selection_score(&Candidate {
            title: item.title.clone(),
            body: item.body.clone(),
            kind: item.kind.clone(),
            scope: item.scope.clone(),
            memory_tier: item.memory_tier.clone(),
            abstraction_of: item.abstraction_of.clone(),
            abstracted_from: item.abstracted_from.clone(),
            evidence: item.evidence.clone(),
            confidence: Some(item.confidence),
            reason: Some(item.reason.clone()),
            matched_template: Some(item.matched_signal.clone()),
        });
    }

    item.confidence + rubric.iter().sum::<f32>() / rubric.len() as f32
}

pub(super) fn build_memory_tier_metadata(
    candidate: &Candidate,
) -> (MemoryTier, BTreeMap<String, f32>) {
    (
        candidate.memory_tier.clone(),
        candidate_value_scores_with_origin(candidate, Some(chunk::ChunkOrigin::User)),
    )
}

pub(super) fn llm_item_value_scores(item: &embedding::LlmKnowledgeItem) -> BTreeMap<String, f32> {
    let mut scores = candidate_value_scores(&Candidate {
        title: item.title.clone(),
        body: item.body.clone(),
        kind: item.kind.clone(),
        scope: item.scope.clone(),
        memory_tier: item.memory_tier.clone(),
        abstraction_of: item.abstraction_of.clone(),
        abstracted_from: item.abstracted_from.clone(),
        evidence: item.evidence.clone(),
        confidence: Some(item.confidence),
        reason: Some(item.reason.clone()),
        matched_template: Some(item.matched_signal.clone()),
    });
    if let Some(value) = item.durability_score {
        scores.insert("durability".to_string(), value);
    }
    if let Some(value) = item.reusability_score {
        scores.insert("reusability".to_string(), value);
    }
    if let Some(value) = item.specificity_score {
        scores.insert("specificity".to_string(), value);
    }
    if let Some(value) = item.source_trust_score {
        scores.insert("source_trust".to_string(), value);
    }
    scores
}

pub(super) fn is_priority_template(candidate: &Candidate) -> bool {
    matches!(
        candidate.matched_template.as_deref(),
        Some("atomic-exception" | "project-improvement" | "high-value-prompt" | "principle-signal")
    ) || candidate
        .matched_template
        .as_deref()
        .is_some_and(|template| template.starts_with("methodology-"))
}

pub(super) fn scoped_skilllets(
    existing_skilllets: &[skilllet::SkillletRecord],
    scope: &str,
) -> Vec<skilllet::SkillletRecord> {
    existing_skilllets
        .iter()
        .filter(|skilllet| match scope {
            "global" => skilllet.scope == "global",
            "agent" => skilllet.scope == "agent",
            _ => skilllet.scope != "global",
        })
        .cloned()
        .collect()
}

pub(super) fn methodology_pair_candidates(sentence: &str) -> Vec<Candidate> {
    let lower = sentence.to_lowercase();
    let project_body = normalize_body(sentence);
    if project_body.len() < 8 || project_body.len() > 320 {
        return Vec::new();
    }

    let mut abstract_specs = Vec::new();
    if mentions_test_strategy(&lower) {
        abstract_specs.push((
            "Prefer Fast Targeted Tests".to_string(),
            "Prefer fast targeted tests for small changes, and reserve comprehensive test runs for larger or higher-risk changes.".to_string(),
            MemoryTier::CollaborationPreference,
            "Abstracted a durable testing and verification preference from repeated development guidance.".to_string(),
        ));
    }
    if mentions_design_planning(&lower) {
        abstract_specs.push((
            "Plan Before Editing".to_string(),
            "Before implementation, ask clarifying questions, confirm the goal, and present a plan.".to_string(),
            MemoryTier::CollaborationPreference,
            "Abstracted a stable planning and collaboration preference from the user's design guidance.".to_string(),
        ));
    }
    if mentions_user_experience_principle(&lower) {
        abstract_specs.push((
            "Prioritize User Experience".to_string(),
            "Evaluate changes from the user's perspective and keep everyday interactions responsive and polished outside unavoidable model latency.".to_string(),
            MemoryTier::CrossProjectPrinciple,
            "Abstracted a reusable product and UX principle from project-specific feedback.".to_string(),
        ));
    }
    if mentions_core_functionality(&lower) {
        abstract_specs.push((
            "Focus On Core Functionality".to_string(),
            "Focus on the core functionality first, and defer secondary features until the main workflow is solid.".to_string(),
            MemoryTier::CrossProjectPrinciple,
            "Abstracted a reusable product prioritization principle from repeated guidance.".to_string(),
        ));
    }
    if mentions_real_history_validation(&lower) {
        abstract_specs.push((
            "Validate With Real History".to_string(),
            "Validate extraction changes against real conversation history, not only synthetic fixtures or toy examples.".to_string(),
            MemoryTier::CollaborationPreference,
            "Abstracted a durable evaluation preference from the user's real-history testing workflow.".to_string(),
        ));
    }
    if mentions_review_boundary(&lower) {
        abstract_specs.push((
            "Preserve Review Boundaries".to_string(),
            "Keep AI-generated rules reviewable, and preserve explicit human approval boundaries before turning them into durable instructions.".to_string(),
            MemoryTier::CollaborationPreference,
            "Abstracted a stable governance preference from review-boundary guidance.".to_string(),
        ));
    }

    if abstract_specs.is_empty() {
        return Vec::new();
    }

    let project_title = title_from_body(&project_body);
    let project_id = candidate_id_from_scope_and_title("project", &project_title);
    let project_reason =
        "Captured a project-context expression that also maps to a reusable cross-project principle."
            .to_string();

    let mut output = vec![Candidate {
        title: project_title,
        body: project_body,
        kind: classify_kind(sentence).to_string(),
        scope: infer_scope(sentence).to_string(),
        memory_tier: MemoryTier::ProjectRule,
        abstraction_of: None,
        abstracted_from: None,
        evidence: sentence.to_string(),
        confidence: Some(0.83),
        reason: Some(project_reason),
        matched_template: Some("methodology-project-shadow".to_string()),
    }];

    for (index, (abstract_title, abstract_body, tier, abstract_reason)) in
        abstract_specs.into_iter().enumerate()
    {
        let abstract_id = candidate_id_from_scope_and_title("global", &abstract_title);
        if index == 0 {
            output[0].abstraction_of = Some(abstract_id.clone());
        }
        output.push(Candidate {
            title: abstract_title,
            body: abstract_body,
            kind: if tier == MemoryTier::CrossProjectPrinciple {
                "principle".to_string()
            } else {
                "procedure".to_string()
            },
            scope: "global".to_string(),
            memory_tier: tier,
            abstraction_of: None,
            abstracted_from: Some(project_id.clone()),
            evidence: sentence.to_string(),
            confidence: Some(0.9),
            reason: Some(abstract_reason),
            matched_template: Some("fallback-methodology-template".to_string()),
        });
    }

    output
}

pub(super) fn has_methodology_signal(body: &str) -> bool {
    let lower = body.to_lowercase();
    signals::has_principle_signal(&lower)
        || signals::has_planning_heuristic_signal(&lower)
        || signals::has_collaboration_preference_signal(&lower)
        || mentions_design_planning(&lower)
        || mentions_real_history_validation(&lower)
        || mentions_review_boundary(&lower)
        || mentions_test_strategy(&lower)
        || mentions_user_experience_principle(&lower)
        || mentions_core_functionality(&lower)
}

pub(super) fn memory_tier_from_guess(guess: Option<&str>, body: &str) -> MemoryTier {
    match guess.unwrap_or_default() {
        "cross_project_principle" => MemoryTier::CrossProjectPrinciple,
        "collaboration_preference" => MemoryTier::CollaborationPreference,
        "project_rule" => MemoryTier::ProjectRule,
        _ => {
            let lower = body.to_lowercase();
            if mentions_design_planning(&lower)
                || mentions_real_history_validation(&lower)
                || mentions_review_boundary(&lower)
                || mentions_test_strategy(&lower)
            {
                MemoryTier::CollaborationPreference
            } else if mentions_user_experience_principle(&lower)
                || mentions_core_functionality(&lower)
            {
                MemoryTier::CrossProjectPrinciple
            } else {
                MemoryTier::ProjectRule
            }
        }
    }
}

pub(super) fn abstracted_item_from_spec(
    project_item: &embedding::LlmKnowledgeItem,
    spec: r#abstract::AbstractedKnowledge,
) -> embedding::LlmKnowledgeItem {
    let memory_tier = spec.memory_tier.unwrap_or_else(|| {
        if mentions_design_planning(&spec.body.to_lowercase())
            || mentions_real_history_validation(&spec.body.to_lowercase())
            || mentions_test_strategy(&spec.body.to_lowercase())
        {
            MemoryTier::CollaborationPreference
        } else {
            MemoryTier::CrossProjectPrinciple
        }
    });
    let title = title_from_body(&spec.body);
    let project_id = candidate_id_from_scope_and_title(&project_item.scope, &project_item.title);
    embedding::LlmKnowledgeItem {
        title,
        body: spec.body,
        kind: if memory_tier == MemoryTier::CrossProjectPrinciple {
            "principle".to_string()
        } else {
            "procedure".to_string()
        },
        scope: "global".to_string(),
        memory_tier,
        abstraction_of: None,
        abstracted_from: Some(project_id),
        confidence: project_item.confidence.max(0.86),
        evidence: project_item.evidence.clone(),
        reason: spec.reason,
        matched_signal: "llm-abstract-stage".to_string(),
        is_noise: false,
        suggested_action: candidate::ExtractionAction::new_candidate_for_route("review_only"),
        durability_score: Some(0.86),
        reusability_score: Some(0.92),
        specificity_score: Some(0.80),
        source_trust_score: project_item.source_trust_score,
    }
}

pub(super) fn classification_for_candidate(
    candidate: &Candidate,
    chunk: &chunk::EvidenceChunk,
) -> classify::KnowledgeClassification {
    let classification = classify::classify_chunk(chunk);
    if classification.artifact_kind != "reject" {
        return classification;
    }

    match candidate.memory_tier {
        MemoryTier::CrossProjectPrinciple => classify::KnowledgeClassification {
            signal: "principle".to_string(),
            artifact_kind: "always_on_rule".to_string(),
            activation: "always_on".to_string(),
            hardness: "medium".to_string(),
            control: "principle".to_string(),
            rationale: "Tier-aware fallback classification for a reusable cross-project principle."
                .to_string(),
            tags: vec![
                "activation:always-on".to_string(),
                "hardness:medium".to_string(),
                "shape:principle".to_string(),
                "target:agents-md".to_string(),
            ],
        },
        MemoryTier::CollaborationPreference => classify::KnowledgeClassification {
            signal: if candidate.kind == "procedure" {
                "procedure".to_string()
            } else {
                "preference".to_string()
            },
            artifact_kind: "workflow_skill".to_string(),
            activation: "skill".to_string(),
            hardness: "medium".to_string(),
            control: "checklist".to_string(),
            rationale: "Tier-aware fallback classification for a durable collaboration preference."
                .to_string(),
            tags: vec![
                "activation:skill".to_string(),
                "hardness:medium".to_string(),
                if candidate.kind == "procedure" {
                    "shape:procedure".to_string()
                } else {
                    "shape:preference".to_string()
                },
                "target:skill-dir".to_string(),
            ],
        },
        MemoryTier::ProjectRule => classification,
    }
}

fn mentions_test_strategy(lower: &str) -> bool {
    (lower.contains("小修改")
        || lower.contains("small change")
        || lower.contains("targeted test")
        || lower.contains("快测"))
        && (lower.contains("大版本")
            || lower.contains("larger")
            || lower.contains("high-risk")
            || lower.contains("comprehensive")
            || lower.contains("完整回归")
            || lower.contains("重测"))
        && (lower.contains("测试")
            || lower.contains("test")
            || lower.contains("回归")
            || lower.contains("验证"))
}

fn mentions_design_planning(lower: &str) -> bool {
    (lower.contains("先提问")
        || lower.contains("先澄清")
        || lower.contains("先规划")
        || lower.contains("before implementation")
        || lower.contains("ask clarifying questions"))
        && (lower.contains("设计")
            || lower.contains("规划")
            || lower.contains("goal")
            || lower.contains("方案")
            || lower.contains("plan"))
}

fn mentions_user_experience_principle(lower: &str) -> bool {
    lower.contains("用户视角")
        || lower.contains("用户体验")
        || lower.contains("体验优化")
        || lower.contains("responsive")
        || lower.contains("丝滑")
        || lower.contains("卡顿")
}

fn mentions_core_functionality(lower: &str) -> bool {
    lower.contains("核心功能")
        || lower.contains("core functionality")
        || lower.contains("main workflow")
}

fn mentions_real_history_validation(lower: &str) -> bool {
    (lower.contains("真实历史") || lower.contains("real history") || lower.contains("重测"))
        && (lower.contains("测试") || lower.contains("验证") || lower.contains("replay"))
}

fn mentions_review_boundary(lower: &str) -> bool {
    lower.contains("审阅边界")
        || lower.contains("review boundary")
        || lower.contains("不要让 ai")
        || lower.contains("human approval")
        || lower.contains("先 review")
        || lower.contains("candidate/draft")
}
