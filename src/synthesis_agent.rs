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

const WRITING_GUIDE: &str = include_str!("../prompts/memory-card-writing-guide.md");
const MAX_OBSERVATION_SNIPPETS: usize = 5;
const MAX_MEMORY_CARD_MATCHES: usize = 5;
const MAX_SKILL_MATCHES: usize = 5;
const DUPLICATE_THRESHOLD: f32 = 0.74;
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
    SearchMemoryCards,
    SearchSkills,
    ReadWritingGuide,
    FindMemoryDuplicates,
    CompareWithSkill,
}

impl SynthesisToolName {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SearchObservations => "search_observations",
            Self::SearchMemoryCards => "search_memory_cards",
            Self::SearchSkills => "search_skills",
            Self::ReadWritingGuide => "read_writing_guide",
            Self::FindMemoryDuplicates => "find_memory_duplicates",
            Self::CompareWithSkill => "compare_with_skill",
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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryCardMatch {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub scope: String,
    pub score: f32,
    pub duplicate: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkillMatch {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source_kind: String,
    pub score: f32,
    pub project_level: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SynthesisContextPack {
    #[serde(default)]
    pub observations: Vec<ObservationSnippet>,
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
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SynthesisReview {
    pub session_id: String,
    pub candidate_id: String,
    pub stop_reason: SynthesisStopReason,
    pub context: SynthesisContextPack,
    pub proposal: SynthesisProposal,
    pub events: Vec<SynthesisEvent>,
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
    let memory_matches = search_memory_cards(&memory_cards, &global_memory_cards, &query);
    let skill_matches = search_skills(&skills, &query);
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
            tool: SynthesisToolName::SearchMemoryCards,
            summary: format!(
                "Compared {} Memory Card match(es), including project cards before global references.",
                memory_matches.len()
            ),
            item_ids: memory_matches
                .iter()
                .map(|matched| matched.id.clone())
                .collect(),
        },
        SynthesisEvent {
            tool: SynthesisToolName::SearchSkills,
            summary: format!(
                "Checked {} Skill match(es); global Skills are reference-only targets.",
                skill_matches.len()
            ),
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
    if let Some(target) = proposal.target_skill_id.as_ref() {
        events.push(SynthesisEvent {
            tool: SynthesisToolName::CompareWithSkill,
            summary: format!(
                "Recommended Skill-targeted Memory Card because project Skill `{target}` has a concrete gap."
            ),
            item_ids: vec![target.clone()],
        });
    }

    let stop_reason = stop_reason_for(&proposal, &observation_snippets);
    Ok(SynthesisReview {
        session_id: session_id(candidate, &query),
        candidate_id: candidate.id.clone(),
        stop_reason,
        context: SynthesisContextPack {
            observations: observation_snippets,
            memory_cards: memory_matches,
            skills: skill_matches,
            writing_guide_summary: guide_summary,
        },
        proposal,
        events,
    })
}

pub fn review_to_trace(review: &SynthesisReview) -> Vec<SynthesisTraceEntry> {
    let mut trace = review
        .events
        .iter()
        .map(|event| SynthesisTraceEntry {
            step: event.tool.as_str().to_string(),
            summary: event.summary.clone(),
        })
        .collect::<Vec<_>>();
    trace.push(SynthesisTraceEntry {
        step: "stop".to_string(),
        summary: format!(
            "Stopped with `{}` after producing `{}`.",
            review.stop_reason.as_str(),
            review.proposal.action
        ),
    });
    trace
}

fn propose(
    candidate: &CandidateRecord,
    approvable_kind: &str,
    memory_matches: &[MemoryCardMatch],
    skill_matches: &[SkillMatch],
) -> SynthesisProposal {
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

    let inferred_function = if duplicate_match.is_some() {
        "merge"
    } else if project_skill.is_some() || mentions_skill(candidate) {
        "skill_targeted"
    } else if approvable_kind == "workflow" || approvable_kind == "procedure" {
        "workflow"
    } else {
        "library"
    };
    let target_type = match inferred_function {
        "merge" => "memory_card",
        "skill_targeted" => "project_skill",
        "workflow" => "workflow",
        _ => "workflow",
    };
    let target_id = duplicate_match
        .clone()
        .or_else(|| project_skill.as_ref().map(|skill| skill.id.clone()));
    let value_delta = value_delta_for(
        candidate,
        inferred_function,
        duplicate_match.as_deref(),
        project_skill.as_ref(),
    );
    let value_claim = value_claim_for(
        candidate,
        inferred_function,
        &value_delta,
        project_skill.as_ref(),
    );
    let action = match inferred_function {
        "merge" => "merge_card",
        "skill_targeted" => "skill_targeted_card",
        "workflow" => "workflow_card",
        _ => "new_card",
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
                inferred_function,
                duplicate_match.as_deref(),
                project_skill.as_ref(),
            ),
        },
        merge_target_id: duplicate_match,
        target_skill_id: project_skill.map(|skill| skill.id),
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
            (score >= RELATED_THRESHOLD || source_boost > 0.0).then(|| ObservationSnippet {
                id: observation.id.clone(),
                source_kind: observation.source_kind.clone(),
                quote: snippet(&observation.body, 280),
                score: cap_score(score),
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
        });
    }
    scored
}

fn search_memory_cards(
    project_cards: &[MemoryCardRecord],
    global_cards: &[MemoryCardRecord],
    query: &str,
) -> Vec<MemoryCardMatch> {
    let mut scored = project_cards
        .iter()
        .map(|card| memory_card_match(card, query, false))
        .chain(
            global_cards
                .iter()
                .map(|card| memory_card_match(card, query, true)),
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

fn memory_card_match(card: &MemoryCardRecord, query: &str, global: bool) -> MemoryCardMatch {
    let text = format!(
        "{} {} {} {}",
        card.title,
        card.body,
        card.brief,
        card.tags.join(" ")
    );
    let score = lexical_score(query, &text);
    MemoryCardMatch {
        id: card.id.clone(),
        title: card.title.clone(),
        kind: card.kind.clone(),
        scope: if global {
            "global".to_string()
        } else {
            card.scope.clone()
        },
        score: cap_score(score),
        duplicate: score >= DUPLICATE_THRESHOLD,
    }
}

fn search_skills(skills: &[SkillRecord], query: &str) -> Vec<SkillMatch> {
    let mut scored = skills
        .iter()
        .filter_map(|skill| {
            let text = format!("{} {} {}", skill.name, skill.description, skill.source_kind);
            let score = lexical_score(query, &text);
            (score >= RELATED_THRESHOLD || query_mentions_name(query, &skill.name)).then(|| {
                SkillMatch {
                    id: skill.id.clone(),
                    name: skill.name.clone(),
                    description: skill.description.clone(),
                    source_kind: skill.source_kind.clone(),
                    score: cap_score(score),
                    project_level: skill.source_kind == "project",
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
) -> String {
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
    card_function: &str,
    duplicate_id: Option<&str>,
    skill: Option<&SkillMatch>,
) -> String {
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
) -> SynthesisStopReason {
    if proposal.confidence < 0.38 || observation_snippets.is_empty() {
        return SynthesisStopReason::NeedsHuman;
    }
    match proposal.card_function.as_str() {
        "merge" => SynthesisStopReason::MergeTargetFound,
        "skill_targeted" => SynthesisStopReason::SkillGapFound,
        "workflow" => SynthesisStopReason::WorkflowGapFound,
        _ => SynthesisStopReason::NewCardGrounded,
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
mod tests {
    use super::*;
    use crate::candidate::{CandidateStatus, ExtractionMetadata};
    use crate::config::{SkillIndex, SkillRecord, save_skill_index};
    use crate::extract::lifecycle::MemoryCardOperation;
    use crate::memory_card;
    use crate::observation;

    #[test]
    fn synthesis_targets_project_skill_and_keeps_global_reference_only() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        save_skill_index(
            root,
            &SkillIndex {
                generated_at: "now".to_string(),
                skills: vec![
                    SkillRecord {
                        id: "project:ux-quality".to_string(),
                        name: "agent-kernel-ux-quality-pass".to_string(),
                        description: "Use when checking product UX quality.".to_string(),
                        source_path: ".agents/skills/agent-kernel-ux-quality-pass".to_string(),
                        source_kind: "project".to_string(),
                        source_hash: "hash".to_string(),
                        warnings: Vec::new(),
                    },
                    SkillRecord {
                        id: "local:skill-creator".to_string(),
                        name: "skill-creator".to_string(),
                        description: "Create reusable skills.".to_string(),
                        source_path: "C:/Users/example/.codex/skills/skill-creator".to_string(),
                        source_kind: "referenced".to_string(),
                        source_hash: "hash".to_string(),
                        warnings: Vec::new(),
                    },
                ],
            },
        )
        .expect("skill index");
        observation::import_observation_text(
            root,
            &root.join("session.jsonl"),
            "codex-session",
            Some("codex"),
            "用户反馈 Skills 页面应该帮助 agent-kernel-ux-quality-pass 做真实 UX 试用，而不是只展示只读字典。",
        )
        .expect("observation");
        let candidate = candidate(
            "skill-card",
            "Skills 应支持 UX 试用闭环",
            "补强 agent-kernel-ux-quality-pass skill 的真实试用流程",
        );

        let review = run_memory_card_synthesis(root, &candidate, "procedure").expect("review");

        assert_eq!(review.proposal.card_function, "skill_targeted");
        assert_eq!(
            review.proposal.target_context.target_id.as_deref(),
            Some("project:ux-quality")
        );
        assert_eq!(review.stop_reason, SynthesisStopReason::SkillGapFound);
    }

    #[test]
    fn synthesis_recommends_merge_for_near_duplicate_project_card() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        memory_card::add_memory_card(
            root,
            "memory-card-existing",
            "用真实试用验证 UX",
            "触发：完成页面重构后。\n\n动作：按真实用户路径试用并记录问题。\n\n边界：不要只凭编译通过判断完成。",
            "procedure",
            "project",
            vec![],
        )
        .expect("memory card");
        observation::import_observation_text(
            root,
            &root.join("session.jsonl"),
            "codex-session",
            Some("codex"),
            "页面重构后要按真实用户路径试用并记录问题，不要只看编译。",
        )
        .expect("observation");
        let candidate = candidate(
            "memory-card-new",
            "用真实试用验证 UX",
            "页面重构后要按真实用户路径试用并记录问题，不要只看编译。",
        );

        let review = run_memory_card_synthesis(root, &candidate, "procedure").expect("review");

        assert_eq!(review.proposal.card_function, "merge");
        assert_eq!(
            review.proposal.merge_target_id.as_deref(),
            Some("memory-card-existing")
        );
        assert_eq!(review.stop_reason, SynthesisStopReason::MergeTargetFound);
    }

    #[test]
    fn trace_is_compact_and_reviewable() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        observation::import_observation_text(
            root,
            &root.join("session.jsonl"),
            "codex-session",
            Some("codex"),
            "以后生成 Memory Card 时要说明价值增量和边界。",
        )
        .expect("observation");
        let candidate = candidate(
            "memory-card-value",
            "说明 Memory Card 价值增量",
            "生成 Memory Card 时要说明价值增量和边界。",
        );
        let review = run_memory_card_synthesis(root, &candidate, "procedure").expect("review");
        let trace = review_to_trace(&review);

        assert!(
            trace
                .iter()
                .any(|entry| entry.step == "search_observations")
        );
        assert!(trace.iter().any(|entry| entry.step == "stop"));
        assert!(
            trace
                .iter()
                .all(|entry| !entry.summary.to_lowercase().contains("chain-of-thought"))
        );
    }

    fn candidate(id: &str, title: &str, body: &str) -> CandidateRecord {
        CandidateRecord {
            schema_version: 1,
            id: id.to_string(),
            title: title.to_string(),
            kind: "procedure".to_string(),
            scope: "project".to_string(),
            body: body.to_string(),
            brief: body.to_string(),
            tags: vec!["workflow".to_string()],
            language: "zh".to_string(),
            targets: Vec::new(),
            evidence: body.to_string(),
            confidence: Some(0.82),
            reason: Some("test".to_string()),
            matched_template: None,
            source_observations: Vec::new(),
            extraction: ExtractionMetadata::default(),
            operation: MemoryCardOperation::Add,
            duplicate_of: None,
            conflict_with: Vec::new(),
            quality_flags: Vec::new(),
            status: CandidateStatus::Candidate,
            rejected_reason: None,
            created_at: "2026-05-20T00:00:00Z".to_string(),
            updated_at: "2026-05-20T00:00:00Z".to_string(),
        }
    }
}
