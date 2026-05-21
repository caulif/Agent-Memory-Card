use std::collections::BTreeSet;
use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::candidate::{CandidateRecord, SynthesisTraceEntry, TargetContext, ValueDelta};
use crate::config::{self, SkillRecord};
use crate::fsutil;
use crate::memory_card::{self, MemoryCardRecord};
use crate::observation::{self, ObservationRecord};
use crate::textutil;

mod explain;
mod skill_eval;

const WRITING_GUIDE: &str = include_str!("../prompts/memory-card-writing-guide.md");
const MAX_OBSERVATION_SNIPPETS: usize = 5;
const MAX_RELATED_OBSERVATION_SNIPPETS: usize = 4;
const MAX_WORKFLOW_FAILURE_INSIGHTS: usize = 4;
const MAX_MEMORY_CARD_MATCHES: usize = 5;
const MAX_SKILL_MATCHES: usize = 5;
const DUPLICATE_THRESHOLD: f32 = 0.74;
const DIRECT_OBSERVATION_THRESHOLD: f32 = 0.4;
const RELATED_THRESHOLD: f32 = 0.18;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SynthesisStopReason {
    AlreadyCovered,
    MergeTargetFound,
    SkillGapFound,
    WorkflowGapFound,
    NewCardGrounded,
    NeedsHuman,
}

impl SynthesisStopReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AlreadyCovered => "already_covered",
            Self::MergeTargetFound => "merge_target_found",
            Self::SkillGapFound => "skill_gap_found",
            Self::WorkflowGapFound => "workflow_gap_found",
            Self::NewCardGrounded => "new_card_grounded",
            Self::NeedsHuman => "needs_human",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SynthesisToolName {
    SearchObservations,
    SearchGlobalHistory,
    SearchMemoryCards,
    SearchSkills,
    ReadWritingGuide,
    FindMemoryDuplicates,
    CompareWithSkill,
    EvaluateSkillUsefulness,
    SummarizeWorkflowFailures,
}

impl SynthesisToolName {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SearchObservations => "search_observations",
            Self::SearchGlobalHistory => "search_global_history",
            Self::SearchMemoryCards => "search_memory_cards",
            Self::SearchSkills => "search_skills",
            Self::ReadWritingGuide => "read_writing_guide",
            Self::FindMemoryDuplicates => "find_memory_duplicates",
            Self::CompareWithSkill => "compare_with_skill",
            Self::EvaluateSkillUsefulness => "evaluate_skill_usefulness",
            Self::SummarizeWorkflowFailures => "summarize_workflow_failures",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SynthesisEvent {
    pub tool: SynthesisToolName,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub item_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObservationSnippet {
    pub id: String,
    pub source_kind: String,
    pub quote: String,
    pub score: f32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub relation: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowFailureInsight {
    pub kind: String,
    pub observation_id: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryCardMatch {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub scope: String,
    pub score: f32,
    pub duplicate: bool,
    pub overlap_summary: String,
    pub gap_summary: String,
    pub merge_hint: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkillMatch {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source_kind: String,
    pub score: f32,
    pub project_level: bool,
    pub coverage_summary: String,
    pub gap_summary: String,
    pub target_role: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkillUsefulnessEvaluation {
    pub target_skill_id: String,
    pub before_behavior: String,
    pub after_behavior: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub improved_axes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing_axes: Vec<String>,
    pub verdict: String,
    pub score: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SynthesisContextPack {
    #[serde(default)]
    pub observations: Vec<ObservationSnippet>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related_observations: Vec<ObservationSnippet>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub workflow_failures: Vec<WorkflowFailureInsight>,
    #[serde(default)]
    pub memory_cards: Vec<MemoryCardMatch>,
    #[serde(default)]
    pub skills: Vec<SkillMatch>,
    pub writing_guide_summary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SynthesisProposal {
    pub action: String,
    pub card_function: String,
    pub value_claim: String,
    pub value_delta: ValueDelta,
    pub target_context: TargetContext,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merge_target_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_skill_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill_usefulness: Option<SkillUsefulnessEvaluation>,
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SynthesisReviewMetrics {
    pub no_card_decision: bool,
    pub merge_recommended: bool,
    pub approval_candidate: bool,
    pub duplicate_suppressed: bool,
    pub counterfactual_pass: bool,
    pub needs_human: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SynthesisReview {
    pub session_id: String,
    pub candidate_id: String,
    pub stop_reason: SynthesisStopReason,
    pub context: SynthesisContextPack,
    pub proposal: SynthesisProposal,
    pub events: Vec<SynthesisEvent>,
    pub metrics: SynthesisReviewMetrics,
}

pub fn run_memory_card_synthesis(
    project_root: &Path,
    candidate: &CandidateRecord,
    approvable_kind: &str,
) -> Result<SynthesisReview> {
    let root = fsutil::normalize_project_root(project_root)?;
    let query = candidate_query(candidate, approvable_kind);
    let observations = observation::load_observations(&root)?;
    let memory_cards = memory_card::load_memory_cards(&root)?;
    let global_memory_cards = fsutil::home_dir()
        .map(|home| memory_card::load_global_memory_cards(&home))
        .transpose()?
        .unwrap_or_default();
    let skills = config::load_skill_index(&root)?.skills;

    let observation_snippets = search_observations(candidate, &observations, &query);
    let related_observations =
        search_global_history(candidate, &observations, &query, &observation_snippets);
    let history_window = observation_snippets
        .iter()
        .chain(related_observations.iter())
        .collect::<Vec<_>>();
    let workflow_failures = summarize_workflow_failures(&observations, &history_window);
    let memory_matches =
        search_memory_cards(&memory_cards, &global_memory_cards, candidate, &query);
    let skill_matches = search_skills(&skills, candidate, &query);
    let guide_summary = writing_guide_summary();

    let mut events = vec![
        SynthesisEvent {
            tool: SynthesisToolName::SearchObservations,
            summary: format!(
                "Read {} related observation snippet(s) for candidate evidence grounding.",
                observation_snippets.len()
            ),
            item_ids: observation_snippets
                .iter()
                .map(|snippet| snippet.id.clone())
                .collect(),
        },
        SynthesisEvent {
            tool: SynthesisToolName::SearchGlobalHistory,
            summary: format!(
                "Read {} broader history snippet(s) beyond direct evidence for repeated workflow context.",
                related_observations.len()
            ),
            item_ids: related_observations
                .iter()
                .map(|snippet| snippet.id.clone())
                .collect(),
        },
        SynthesisEvent {
            tool: SynthesisToolName::SummarizeWorkflowFailures,
            summary: format!(
                "Summarized {} workflow failure signal(s) from the selected history window.",
                workflow_failures.len()
            ),
            item_ids: workflow_failures
                .iter()
                .map(|failure| failure.observation_id.clone())
                .collect(),
        },
        SynthesisEvent {
            tool: SynthesisToolName::SearchMemoryCards,
            summary: explain::memory_card_event_summary(&memory_matches),
            item_ids: memory_matches
                .iter()
                .map(|matched| matched.id.clone())
                .collect(),
        },
        SynthesisEvent {
            tool: SynthesisToolName::SearchSkills,
            summary: explain::skill_event_summary(&skill_matches),
            item_ids: skill_matches
                .iter()
                .map(|matched| matched.id.clone())
                .collect(),
        },
        SynthesisEvent {
            tool: SynthesisToolName::ReadWritingGuide,
            summary: "Loaded the built-in Memory Card writing guide for style and value checks."
                .to_string(),
            item_ids: Vec::new(),
        },
    ];

    let proposal = propose(candidate, approvable_kind, &memory_matches, &skill_matches);
    if let Some(target) = proposal.merge_target_id.as_ref() {
        events.push(SynthesisEvent {
            tool: SynthesisToolName::FindMemoryDuplicates,
            summary: format!(
                "Recommended merge because existing Memory Card `{target}` overlaps the candidate."
            ),
            item_ids: vec![target.clone()],
        });
    }
    if proposal.action == "already_covered" {
        if let Some(target) = proposal.target_context.target_id.as_ref() {
            events.push(SynthesisEvent {
                tool: SynthesisToolName::FindMemoryDuplicates,
                summary: format!(
                    "Recommended no new card because Memory Card `{target}` already covers the candidate."
                ),
                item_ids: vec![target.clone()],
            });
        }
    }
    if let Some(target) = proposal.target_skill_id.as_ref() {
        let summary = if proposal.action == "skill_targeted_card" {
            format!(
                "Recommended Skill-targeted Memory Card because project Skill `{target}` has a concrete gap."
            )
        } else {
            format!(
                "Checked project Skill `{target}`, but the proposal still needs a stronger counterfactual value delta."
            )
        };
        events.push(SynthesisEvent {
            tool: SynthesisToolName::CompareWithSkill,
            summary,
            item_ids: vec![target.clone()],
        });
    }
    if let Some(evaluation) = proposal.skill_usefulness.as_ref() {
        events.push(SynthesisEvent {
            tool: SynthesisToolName::EvaluateSkillUsefulness,
            summary: skill_eval::event_summary(evaluation),
            item_ids: vec![evaluation.target_skill_id.clone()],
        });
    }

    let stop_reason = stop_reason_for(
        &proposal,
        &observation_snippets,
        &related_observations,
        &workflow_failures,
    );
    let metrics = metrics_for(&proposal, &stop_reason);
    Ok(SynthesisReview {
        session_id: session_id(candidate, &query),
        candidate_id: candidate.id.clone(),
        stop_reason,
        context: SynthesisContextPack {
            observations: observation_snippets,
            related_observations,
            workflow_failures,
            memory_cards: memory_matches,
            skills: skill_matches,
            writing_guide_summary: guide_summary,
        },
        proposal,
        events,
        metrics,
    })
}

pub fn review_to_trace(review: &SynthesisReview) -> Vec<SynthesisTraceEntry> {
    let mut trace = review
        .events
        .iter()
        .map(|event| SynthesisTraceEntry {
            step: event.tool.as_str().to_string(),
            summary: event.summary.clone(),
            item_ids: event.item_ids.clone(),
        })
        .collect::<Vec<_>>();
    trace.push(SynthesisTraceEntry {
        step: "stop".to_string(),
        summary: format!(
            "Stopped with `{}` after producing `{}`.",
            review.stop_reason.as_str(),
            review.proposal.action
        ),
        item_ids: Vec::new(),
    });
    trace
}

fn propose(
    candidate: &CandidateRecord,
    approvable_kind: &str,
    memory_matches: &[MemoryCardMatch],
    skill_matches: &[SkillMatch],
) -> SynthesisProposal {
    let covered_match = memory_matches
        .iter()
        .find(|matched| {
            matched.scope == "project" && matched.duplicate && !mentions_update_intent(candidate)
        })
        .cloned();
    let duplicate_match = candidate
        .extraction
        .suggested_action
        .as_ref()
        .filter(|action| action.action == "merge_into_existing")
        .and_then(|action| {
            action
                .target_record
                .clone()
                .or_else(|| action.record_id.clone())
        })
        .or_else(|| {
            memory_matches
                .iter()
                .find(|matched| matched.duplicate && matched.scope == "project")
                .map(|matched| matched.id.clone())
        });
    let project_skill = skill_matches
        .iter()
        .find(|skill| skill.project_level)
        .cloned();
    let skill_usefulness = skill_eval::evaluate(candidate, project_skill.as_ref());

    let inferred_function = if covered_match.is_some() {
        "library"
    } else if duplicate_match.is_some() {
        "merge"
    } else if project_skill.is_some() || mentions_skill(candidate) {
        "skill_targeted"
    } else if approvable_kind == "workflow" || approvable_kind == "procedure" {
        "workflow"
    } else {
        "library"
    };
    let target_type = if covered_match.is_some() {
        "memory_card"
    } else {
        match inferred_function {
            "merge" => "memory_card",
            "skill_targeted" => "project_skill",
            "workflow" => "workflow",
            _ => "workflow",
        }
    };
    let target_id = covered_match
        .as_ref()
        .map(|matched| matched.id.clone())
        .or_else(|| {
            duplicate_match
                .clone()
                .or_else(|| project_skill.as_ref().map(|skill| skill.id.clone()))
        });
    let value_delta = if let Some(matched) = covered_match.as_ref() {
        ValueDelta {
            existing_behavior: format!(
                "既有 Memory Card `{}` 已覆盖同一触发和行为边界。",
                matched.id
            ),
            missing_part: "未发现比既有卡片更具体的新触发、动作或边界。".to_string(),
            new_behavior:
                "将该候选标记为已覆盖，保持规则库精简；只有出现新的边界时才改为合并更新。"
                    .to_string(),
            why_not_duplicate: format!(
                "继续新增会与「{}」形成重复卡片，降低未来 Skill/agent 选择上下文的清晰度。",
                matched.title
            ),
        }
    } else {
        value_delta_for(
            candidate,
            inferred_function,
            duplicate_match
                .as_deref()
                .or_else(|| covered_match.as_ref().map(|matched| matched.id.as_str())),
            project_skill.as_ref(),
        )
    };
    let value_claim = value_claim_for(
        candidate,
        inferred_function,
        &value_delta,
        project_skill.as_ref(),
        covered_match.as_ref(),
    );
    let action = if covered_match.is_some() {
        "already_covered"
    } else if inferred_function == "skill_targeted"
        && !skill_eval::is_counterfactual_pass(skill_usefulness.as_ref())
    {
        "needs_human"
    } else {
        match inferred_function {
            "merge" => "merge_card",
            "skill_targeted" => "skill_targeted_card",
            "workflow" => "workflow_card",
            _ => "new_card",
        }
    };
    let confidence = confidence_for(candidate, memory_matches, skill_matches, inferred_function);

    SynthesisProposal {
        action: action.to_string(),
        card_function: inferred_function.to_string(),
        value_claim,
        value_delta,
        target_context: TargetContext {
            target_type: target_type.to_string(),
            target_id,
            why_this_target: target_reason(
                covered_match.as_ref(),
                inferred_function,
                duplicate_match.as_deref(),
                project_skill.as_ref(),
            ),
        },
        merge_target_id: duplicate_match,
        target_skill_id: project_skill.map(|skill| skill.id),
        skill_usefulness,
        confidence,
    }
}

fn search_observations(
    candidate: &CandidateRecord,
    observations: &[ObservationRecord],
    query: &str,
) -> Vec<ObservationSnippet> {
    let source_ids = candidate
        .source_observations
        .iter()
        .chain(candidate.extraction.source_observations.iter())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut scored = observations
        .iter()
        .filter_map(|observation| {
            let source_boost = if source_ids.contains(&observation.id) {
                0.65
            } else {
                0.0
            };
            let score = source_boost + lexical_score(query, &observation.body);
            (score >= DIRECT_OBSERVATION_THRESHOLD || source_boost > 0.0).then(|| {
                ObservationSnippet {
                    id: observation.id.clone(),
                    source_kind: observation.source_kind.clone(),
                    quote: snippet(&observation.body, 280),
                    score: cap_score(score),
                    relation: if source_boost > 0.0 {
                        "direct_evidence".to_string()
                    } else {
                        "lexical_evidence".to_string()
                    },
                }
            })
        })
        .collect::<Vec<_>>();
    scored.sort_by(|a, b| b.score.total_cmp(&a.score).then_with(|| a.id.cmp(&b.id)));
    scored.truncate(MAX_OBSERVATION_SNIPPETS);
    if scored.is_empty() && !candidate.evidence.trim().is_empty() {
        scored.push(ObservationSnippet {
            id: format!("candidate:{}", candidate.id),
            source_kind: "candidate_evidence".to_string(),
            quote: snippet(&candidate.evidence, 280),
            score: 0.45,
            relation: "candidate_evidence".to_string(),
        });
    }
    scored
}

fn search_global_history(
    candidate: &CandidateRecord,
    observations: &[ObservationRecord],
    query: &str,
    direct_snippets: &[ObservationSnippet],
) -> Vec<ObservationSnippet> {
    let direct_ids = direct_snippets
        .iter()
        .map(|snippet| snippet.id.as_str())
        .collect::<BTreeSet<_>>();
    let source_ids = candidate
        .source_observations
        .iter()
        .chain(candidate.extraction.source_observations.iter())
        .cloned()
        .collect::<BTreeSet<_>>();
    let source_paths = observations
        .iter()
        .filter(|observation| source_ids.contains(&observation.id))
        .map(|observation| observation.source_path.as_str())
        .collect::<BTreeSet<_>>();
    let mut scored = observations
        .iter()
        .filter(|observation| !direct_ids.contains(observation.id.as_str()))
        .filter_map(|observation| {
            let same_source = source_paths.contains(observation.source_path.as_str());
            let failure_boost = if looks_like_synthesis_relevant_feedback(&observation.body) {
                0.2
            } else {
                0.0
            };
            let source_boost = if same_source { 0.28 } else { 0.0 };
            let score =
                lexical_score(query, &observation.body) * 0.7 + source_boost + failure_boost;
            (score >= RELATED_THRESHOLD).then(|| ObservationSnippet {
                id: observation.id.clone(),
                source_kind: observation.source_kind.clone(),
                quote: snippet(&observation.body, 280),
                score: cap_score(score),
                relation: if same_source {
                    "same_source_context".to_string()
                } else {
                    "related_history".to_string()
                },
            })
        })
        .collect::<Vec<_>>();
    scored.sort_by(|a, b| b.score.total_cmp(&a.score).then_with(|| a.id.cmp(&b.id)));
    scored.truncate(MAX_RELATED_OBSERVATION_SNIPPETS);
    scored
}

fn summarize_workflow_failures(
    observations: &[ObservationRecord],
    snippets: &[&ObservationSnippet],
) -> Vec<WorkflowFailureInsight> {
    let snippet_ids = snippets
        .iter()
        .map(|snippet| snippet.id.as_str())
        .collect::<BTreeSet<_>>();
    let selected = observations
        .iter()
        .filter(|observation| snippet_ids.contains(observation.id.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let summary = observation::failure_flow::summarize_failure_flow(&selected);
    summary
        .signals
        .into_iter()
        .take(MAX_WORKFLOW_FAILURE_INSIGHTS)
        .map(|signal| WorkflowFailureInsight {
            kind: signal.kind.as_str().to_string(),
            observation_id: signal.observation_id,
            summary: signal.snippet,
        })
        .collect()
}

fn search_memory_cards(
    project_cards: &[MemoryCardRecord],
    global_cards: &[MemoryCardRecord],
    candidate: &CandidateRecord,
    query: &str,
) -> Vec<MemoryCardMatch> {
    let mut scored = project_cards
        .iter()
        .map(|card| memory_card_match(card, candidate, query, false))
        .chain(
            global_cards
                .iter()
                .map(|card| memory_card_match(card, candidate, query, true)),
        )
        .filter(|matched| matched.score >= RELATED_THRESHOLD)
        .collect::<Vec<_>>();
    scored.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.scope.cmp(&b.scope))
            .then_with(|| a.id.cmp(&b.id))
    });
    scored.truncate(MAX_MEMORY_CARD_MATCHES);
    scored
}

fn memory_card_match(
    card: &MemoryCardRecord,
    candidate: &CandidateRecord,
    query: &str,
    global: bool,
) -> MemoryCardMatch {
    let text = format!(
        "{} {} {} {}",
        card.title,
        card.body,
        card.brief,
        card.tags.join(" ")
    );
    let score = lexical_score(query, &text);
    let duplicate = score >= DUPLICATE_THRESHOLD;
    let scope = if global {
        "global".to_string()
    } else {
        card.scope.clone()
    };
    MemoryCardMatch {
        id: card.id.clone(),
        title: card.title.clone(),
        kind: card.kind.clone(),
        scope: scope.clone(),
        score: cap_score(score),
        duplicate,
        overlap_summary: explain::memory_overlap_summary(card, &scope, score, duplicate),
        gap_summary: explain::memory_gap_summary(candidate, card, &scope, duplicate),
        merge_hint: explain::memory_merge_hint(candidate, &scope, duplicate),
    }
}

fn search_skills(
    skills: &[SkillRecord],
    candidate: &CandidateRecord,
    query: &str,
) -> Vec<SkillMatch> {
    let mut scored = skills
        .iter()
        .filter_map(|skill| {
            let text = format!("{} {} {}", skill.name, skill.description, skill.source_kind);
            let score = lexical_score(query, &text);
            (score >= RELATED_THRESHOLD || query_mentions_name(query, &skill.name)).then(|| {
                let project_level = skill.source_kind == "project";
                SkillMatch {
                    id: skill.id.clone(),
                    name: skill.name.clone(),
                    description: skill.description.clone(),
                    source_kind: skill.source_kind.clone(),
                    score: cap_score(score),
                    project_level,
                    coverage_summary: explain::skill_coverage_summary(skill, score),
                    gap_summary: explain::skill_gap_summary(candidate, skill, project_level),
                    target_role: if project_level {
                        "project_target".to_string()
                    } else {
                        "global_reference".to_string()
                    },
                }
            })
        })
        .collect::<Vec<_>>();
    scored.sort_by(|a, b| {
        b.project_level
            .cmp(&a.project_level)
            .then_with(|| b.score.total_cmp(&a.score))
            .then_with(|| a.id.cmp(&b.id))
    });
    scored.truncate(MAX_SKILL_MATCHES);
    scored
}

fn value_delta_for(
    candidate: &CandidateRecord,
    card_function: &str,
    duplicate_id: Option<&str>,
    skill: Option<&SkillMatch>,
) -> ValueDelta {
    let missing_part = if let Some(skill) = skill {
        format!(
            "项目级 Skill `{}` 缺少该历史反馈中的触发、操作边界或验收要求。",
            skill.name
        )
    } else if let Some(id) = duplicate_id {
        format!("既有 Memory Card `{id}` 需要吸收这条反馈中的更成熟表达。")
    } else {
        "项目知识库尚未把这条反馈表达成可复用的触发、动作和边界。".to_string()
    };
    let existing_behavior = match card_function {
        "skill_targeted" => skill
            .map(|skill| {
                format!(
                    "当前项目级 Skill `{}` 已提供基础能力：{}",
                    skill.name,
                    empty_as(&skill.description, "未写明详细描述")
                )
            })
            .unwrap_or_else(|| {
                "已有 Skill 注册信息可作为参考，但尚未找到稳定项目级目标。".to_string()
            }),
        "merge" => duplicate_id
            .map(|id| format!("已有 Memory Card `{id}` 覆盖相近主题。"))
            .unwrap_or_else(|| "已有相近 Memory Card 覆盖同一主题。".to_string()),
        "workflow" => "现有项目工作流可被执行，但缺少这条反馈暴露出的明确操作边界。".to_string(),
        _ => "现有项目规则库尚未稳定表达这条可复用偏好或约束。".to_string(),
    };
    let new_behavior = format!(
        "下次处理同类任务时，先应用 Memory Card「{}」中的未来行为规则，并在证据不足时保留人工审阅。",
        candidate.title
    );
    let why_not_duplicate = if duplicate_id.is_some() {
        "该提案应合并更新既有卡片，不作为另一张重复卡片进入规则库。".to_string()
    } else {
        "只有当它补上现有 Skill 或 Memory Card 未覆盖的具体触发、动作或边界时才应批准。".to_string()
    };
    ValueDelta {
        existing_behavior,
        missing_part,
        new_behavior,
        why_not_duplicate,
    }
}

fn value_claim_for(
    candidate: &CandidateRecord,
    card_function: &str,
    value_delta: &ValueDelta,
    skill: Option<&SkillMatch>,
    covered_match: Option<&MemoryCardMatch>,
) -> String {
    if let Some(matched) = covered_match {
        return format!(
            "「{}」已由既有 Memory Card `{}` 覆盖，审阅成功路径是忽略候选而不是新增卡片。",
            candidate.title, matched.id
        );
    }
    match (card_function, skill) {
        ("skill_targeted", Some(skill)) => format!(
            "补强项目级 Skill `{}`，让它下次能处理“{}”这一缺口。",
            skill.name, value_delta.missing_part
        ),
        ("merge", _) => format!(
            "把候选「{}」合并进既有 Memory Card，避免重复卡片同时保留新增边界。",
            candidate.title
        ),
        ("workflow", _) => format!(
            "把「{}」沉淀为工作流 Memory Card，减少下次同类任务的执行偏差。",
            candidate.title
        ),
        _ => format!(
            "把「{}」转为可审阅 Memory Card，让未来 agent 行为有明确触发和边界。",
            candidate.title
        ),
    }
}

fn target_reason(
    covered_match: Option<&MemoryCardMatch>,
    card_function: &str,
    duplicate_id: Option<&str>,
    skill: Option<&SkillMatch>,
) -> String {
    if let Some(matched) = covered_match {
        return format!(
            "Memory Card `{}` 已充分覆盖该候选；保留候选会制造重复规则。",
            matched.id
        );
    }
    match (card_function, duplicate_id, skill) {
        ("merge", Some(id), _) => {
            format!("Memory Card `{id}` 是最高相似的项目级既有规则，应优先合并。")
        }
        ("skill_targeted", _, Some(skill)) => format!(
            "`{}` 是项目级 Skill，可以接收 Memory Card 补强；全局 Skill 仅作参考。",
            skill.name
        ),
        ("workflow", _, _) => {
            "该候选描述的是重复工作流中的操作改进，适合作为 Workflow Memory Card。".to_string()
        }
        _ => "该候选暂未命中可治理项目级 Skill，先作为项目规则库 Memory Card 审阅。".to_string(),
    }
}

fn confidence_for(
    candidate: &CandidateRecord,
    memory_matches: &[MemoryCardMatch],
    skill_matches: &[SkillMatch],
    card_function: &str,
) -> f32 {
    let evidence = if candidate.evidence.trim().is_empty() {
        0.0
    } else {
        0.2
    };
    let candidate_confidence = candidate.confidence.unwrap_or(0.55) * 0.45;
    let match_confidence = match card_function {
        "merge" => memory_matches
            .first()
            .map(|matched| matched.score * 0.3)
            .unwrap_or(0.0),
        "skill_targeted" => skill_matches
            .first()
            .map(|skill| skill.score * 0.25)
            .unwrap_or(0.0),
        _ => 0.12,
    };
    cap_score(0.15 + evidence + candidate_confidence + match_confidence)
}

fn stop_reason_for(
    proposal: &SynthesisProposal,
    observation_snippets: &[ObservationSnippet],
    related_observations: &[ObservationSnippet],
    workflow_failures: &[WorkflowFailureInsight],
) -> SynthesisStopReason {
    let has_local_context = !observation_snippets.is_empty()
        || !related_observations.is_empty()
        || !workflow_failures.is_empty();
    if proposal.confidence < 0.38 || !has_local_context {
        return SynthesisStopReason::NeedsHuman;
    }
    if proposal.action == "needs_human" {
        return SynthesisStopReason::NeedsHuman;
    }
    if proposal.action == "already_covered" {
        return SynthesisStopReason::AlreadyCovered;
    }
    match proposal.card_function.as_str() {
        "merge" => SynthesisStopReason::MergeTargetFound,
        "skill_targeted" => SynthesisStopReason::SkillGapFound,
        "workflow" => SynthesisStopReason::WorkflowGapFound,
        _ => SynthesisStopReason::NewCardGrounded,
    }
}

fn metrics_for(
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

fn candidate_query(candidate: &CandidateRecord, approvable_kind: &str) -> String {
    [
        candidate.title.as_str(),
        candidate.body.as_str(),
        candidate.brief.as_str(),
        candidate.evidence.as_str(),
        approvable_kind,
        &candidate.tags.join(" "),
        &candidate.targets.join(" "),
    ]
    .join(" ")
}

fn lexical_score(query: &str, haystack: &str) -> f32 {
    let jaccard = textutil::jaccard_similarity(query, haystack);
    let q = query.to_lowercase();
    let h = haystack.to_lowercase();
    let contains_bonus = if !q.trim().is_empty() && (h.contains(q.trim()) || q.contains(h.trim())) {
        0.35
    } else {
        0.0
    };
    cap_score(jaccard.max(cjk_char_overlap(&q, &h)) + contains_bonus)
}

fn cjk_char_overlap(query: &str, haystack: &str) -> f32 {
    let query_chars = significant_chars(query);
    let haystack_chars = significant_chars(haystack);
    if query_chars.len() < 8 || haystack_chars.len() < 8 {
        return 0.0;
    }
    let intersection = query_chars.intersection(&haystack_chars).count() as f32;
    let smaller = query_chars.len().min(haystack_chars.len()) as f32;
    if smaller == 0.0 {
        0.0
    } else {
        intersection / smaller
    }
}

fn significant_chars(text: &str) -> BTreeSet<char> {
    text.chars()
        .filter(|ch| ch.is_alphanumeric() || (*ch as u32) > 0x7f)
        .collect()
}

fn query_mentions_name(query: &str, name: &str) -> bool {
    let normalized_query = query.to_lowercase().replace(['-', '_'], " ");
    let normalized_name = name.to_lowercase().replace(['-', '_'], " ");
    !normalized_name.trim().is_empty() && normalized_query.contains(normalized_name.trim())
}

fn mentions_skill(candidate: &CandidateRecord) -> bool {
    let text = format!(
        "{} {} {} {}",
        candidate.title,
        candidate.body,
        candidate.brief,
        candidate.tags.join(" ")
    )
    .to_lowercase();
    text.contains("skill") || text.contains("技能")
}

fn mentions_update_intent(candidate: &CandidateRecord) -> bool {
    let text = format!(
        "{} {} {} {}",
        candidate.title,
        candidate.body,
        candidate.brief,
        candidate.tags.join(" ")
    )
    .to_lowercase();
    [
        "更新", "补充", "补强", "优化", "融合", "合并", "改写", "update", "add", "merge", "refine",
        "improve",
    ]
    .iter()
    .any(|marker| text.contains(marker))
}

fn looks_like_synthesis_relevant_feedback(text: &str) -> bool {
    let lower = text.to_lowercase();
    contains_any(
        &lower,
        &[
            "反馈", "问题", "不足", "缺少", "不要", "不能", "应该", "优化", "改进", "审核", "真实",
            "测试", "failure", "missing", "should", "review", "quality",
        ],
    )
}

fn contains_any(text: &str, markers: &[&str]) -> bool {
    markers.iter().any(|marker| text.contains(marker))
}

fn writing_guide_summary() -> String {
    WRITING_GUIDE
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed.starts_with("- ") || trimmed.starts_with("## ")
        })
        .take(12)
        .collect::<Vec<_>>()
        .join("\n")
}

fn session_id(candidate: &CandidateRecord, query: &str) -> String {
    fsutil::sha256_text(&format!("{}:{query}", candidate.id))
        .trim_start_matches("sha256:")
        .chars()
        .take(16)
        .collect()
}

fn snippet(text: &str, max_chars: usize) -> String {
    let clean = text.split_whitespace().collect::<Vec<_>>().join(" ");
    clean.chars().take(max_chars).collect()
}

fn empty_as(value: &str, fallback: &str) -> String {
    if value.trim().is_empty() {
        fallback.to_string()
    } else {
        value.trim().to_string()
    }
}

fn cap_score(score: f32) -> f32 {
    score.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests;
