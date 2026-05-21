use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

use super::{
    MemoryCardMatch, ObservationSnippet, SkillMatch, SynthesisContextPack, SynthesisEvent,
    SynthesisReview, SynthesisToolName, WorkflowFailureInsight, metrics, propose,
    search_memory_cards, search_observations, search_skills, skill_eval, stop_reason_for,
    writing_guide_summary,
};
use crate::candidate::CandidateRecord;
use crate::config;
use crate::fsutil;
use crate::memory_card;
use crate::observation::{self, ObservationRecord};
use crate::provider::{self, ProviderJsonSchema, ProviderRequest, ProviderRole};

const MAX_PROVIDER_TOOL_CALLS: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct ProviderToolCall {
    pub tool: String,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct ProviderToolPlan {
    #[serde(default)]
    pub tool_calls: Vec<ProviderToolCall>,
}

#[derive(Debug, Default)]
struct LoopState {
    observations: Vec<ObservationSnippet>,
    related_observations: Vec<ObservationSnippet>,
    workflow_failures: Vec<WorkflowFailureInsight>,
    memory_cards: Vec<MemoryCardMatch>,
    skills: Vec<SkillMatch>,
    writing_guide_summary: String,
    events: Vec<SynthesisEvent>,
    executed: BTreeSet<String>,
}

struct ToolEnvironment {
    observations: Vec<ObservationRecord>,
    project_cards: Vec<memory_card::MemoryCardRecord>,
    global_cards: Vec<memory_card::MemoryCardRecord>,
    skills: Vec<config::SkillRecord>,
    query: String,
}

pub(super) fn run_with_provider(
    root: &Path,
    candidate: &CandidateRecord,
    approvable_kind: &str,
) -> Result<SynthesisReview> {
    let provider_path = provider::provider_config_path(root)?;
    if !provider_path.exists() {
        return Err(anyhow!("provider loop disabled without providers.yml"));
    }
    let cfg = provider::load_or_default_provider_config(root)?;
    let env = load_environment(root, candidate, approvable_kind)?;
    let request = build_plan_request(candidate, approvable_kind);
    let output = provider::call_provider_for_role(&cfg, ProviderRole::Refine, &request, 1024)
        .context("provider synthesis tool plan")?;
    let plan = parse_tool_plan(&output)?;
    run_with_tool_plan(root, candidate, approvable_kind, env, plan)
}

fn run_with_tool_plan(
    _root: &Path,
    candidate: &CandidateRecord,
    approvable_kind: &str,
    env: ToolEnvironment,
    plan: ProviderToolPlan,
) -> Result<SynthesisReview> {
    let mut state = LoopState::default();
    for call in normalized_plan(plan, candidate) {
        execute_tool(candidate, &env, &mut state, &call)?;
    }
    if state.writing_guide_summary.is_empty() {
        execute_tool(
            candidate,
            &env,
            &mut state,
            &ProviderToolCall {
                tool: SynthesisToolName::ReadWritingGuide.as_str().to_string(),
                reason: "safety hook: writing guide is required before proposal".to_string(),
            },
        )?;
    }
    let proposal = propose(
        candidate,
        approvable_kind,
        &state.memory_cards,
        &state.skills,
    );
    append_post_proposal_events(&mut state, &proposal);
    let stop_reason = stop_reason_for(
        &proposal,
        &state.observations,
        &state.related_observations,
        &state.workflow_failures,
    );
    let metrics = metrics::metrics_for(&proposal, &stop_reason);
    Ok(SynthesisReview {
        session_id: super::session_id(candidate, &env.query),
        candidate_id: candidate.id.clone(),
        stop_reason,
        context: SynthesisContextPack {
            observations: state.observations,
            related_observations: state.related_observations,
            workflow_failures: state.workflow_failures,
            memory_cards: state.memory_cards,
            skills: state.skills,
            writing_guide_summary: state.writing_guide_summary,
        },
        proposal,
        events: state.events,
        metrics,
    })
}

fn load_environment(
    root: &Path,
    candidate: &CandidateRecord,
    approvable_kind: &str,
) -> Result<ToolEnvironment> {
    let observations = observation::load_observations(root)?;
    let project_cards = memory_card::load_memory_cards(root)?;
    let global_cards = fsutil::home_dir()
        .map(|home| memory_card::load_global_memory_cards(&home))
        .transpose()?
        .unwrap_or_default();
    let skills = config::load_skill_index(root)?.skills;
    Ok(ToolEnvironment {
        observations,
        project_cards,
        global_cards,
        skills,
        query: super::candidate_query(candidate, approvable_kind),
    })
}

fn normalized_plan(
    mut plan: ProviderToolPlan,
    candidate: &CandidateRecord,
) -> Vec<ProviderToolCall> {
    plan.tool_calls.truncate(MAX_PROVIDER_TOOL_CALLS);
    ensure_tool(&mut plan.tool_calls, SynthesisToolName::SearchObservations);
    ensure_tool(&mut plan.tool_calls, SynthesisToolName::SearchMemoryCards);
    if super::mentions_skill(candidate) {
        ensure_tool(&mut plan.tool_calls, SynthesisToolName::SearchSkills);
    }
    plan.tool_calls
}

fn ensure_tool(calls: &mut Vec<ProviderToolCall>, tool: SynthesisToolName) {
    if calls.iter().any(|call| call.tool == tool.as_str()) {
        return;
    }
    calls.push(ProviderToolCall {
        tool: tool.as_str().to_string(),
        reason: "safety hook".to_string(),
    });
}

fn execute_tool(
    candidate: &CandidateRecord,
    env: &ToolEnvironment,
    state: &mut LoopState,
    call: &ProviderToolCall,
) -> Result<()> {
    let tool = parse_tool_name(&call.tool)?;
    let key = tool.as_str().to_string();
    if state.executed.contains(&key) {
        return Ok(());
    }
    match tool {
        SynthesisToolName::SearchObservations => {
            state.observations = search_observations(candidate, &env.observations, &env.query);
            let item_ids = state
                .observations
                .iter()
                .map(|snippet| snippet.id.clone())
                .collect();
            push_event(
                state,
                tool,
                observation_summary(state.observations.len()),
                item_ids,
            );
        }
        SynthesisToolName::SearchGlobalHistory | SynthesisToolName::SummarizeWorkflowFailures => {
            return Err(anyhow!(
                "`{}` is disabled; global Observation retrieval is not part of the current synthesis path",
                tool.as_str()
            ));
        }
        SynthesisToolName::SearchMemoryCards => {
            state.memory_cards =
                search_memory_cards(&env.project_cards, &env.global_cards, candidate, &env.query);
            state.events.push(SynthesisEvent {
                tool,
                summary: super::explain::memory_card_event_summary(&state.memory_cards),
                item_ids: state
                    .memory_cards
                    .iter()
                    .map(|matched| matched.id.clone())
                    .collect(),
            });
        }
        SynthesisToolName::SearchSkills => {
            state.skills = search_skills(&env.skills, candidate, &env.query);
            state.events.push(SynthesisEvent {
                tool,
                summary: super::explain::skill_event_summary(&state.skills),
                item_ids: state
                    .skills
                    .iter()
                    .map(|matched| matched.id.clone())
                    .collect(),
            });
        }
        SynthesisToolName::ReadWritingGuide => {
            state.writing_guide_summary = writing_guide_summary();
            state.events.push(SynthesisEvent {
                tool,
                summary: "Provider requested the built-in Memory Card writing guide.".to_string(),
                item_ids: Vec::new(),
            });
        }
        SynthesisToolName::FindMemoryDuplicates
        | SynthesisToolName::CompareWithSkill
        | SynthesisToolName::EvaluateSkillUsefulness => {
            return Err(anyhow!(
                "`{}` is a post-proposal tool and cannot be called before proposal construction",
                tool.as_str()
            ));
        }
    }
    state.executed.insert(key);
    Ok(())
}

fn append_post_proposal_events(state: &mut LoopState, proposal: &super::SynthesisProposal) {
    if let Some(target) = proposal.merge_target_id.as_ref() {
        state.events.push(SynthesisEvent {
            tool: SynthesisToolName::FindMemoryDuplicates,
            summary: format!(
                "Recommended merge because existing Memory Card `{target}` overlaps the candidate."
            ),
            item_ids: vec![target.clone()],
        });
    }
    if proposal.action == "already_covered"
        && let Some(target) = proposal.target_context.target_id.as_ref()
    {
        state.events.push(SynthesisEvent {
            tool: SynthesisToolName::FindMemoryDuplicates,
            summary: format!(
                "Recommended no new card because Memory Card `{target}` already covers the candidate."
            ),
            item_ids: vec![target.clone()],
        });
    }
    if let Some(target) = proposal.target_skill_id.as_ref() {
        state.events.push(SynthesisEvent {
            tool: SynthesisToolName::CompareWithSkill,
            summary: if proposal.action == "skill_targeted_card" {
                format!(
                    "Recommended Skill-targeted Memory Card because project Skill `{target}` has a concrete gap."
                )
            } else {
                format!(
                    "Checked project Skill `{target}`, but the proposal still needs a stronger counterfactual value delta."
                )
            },
            item_ids: vec![target.clone()],
        });
    }
    if let Some(evaluation) = proposal.skill_usefulness.as_ref() {
        state.events.push(SynthesisEvent {
            tool: SynthesisToolName::EvaluateSkillUsefulness,
            summary: skill_eval::event_summary(evaluation),
            item_ids: vec![evaluation.target_skill_id.clone()],
        });
    }
}

fn observation_summary(count: usize) -> String {
    format!("Provider requested direct evidence; read {count} related observation snippet(s).")
}

fn push_event(
    state: &mut LoopState,
    tool: SynthesisToolName,
    summary: String,
    item_ids: Vec<String>,
) {
    state.events.push(SynthesisEvent {
        tool,
        summary,
        item_ids,
    });
}

fn parse_tool_plan(output: &str) -> Result<ProviderToolPlan> {
    serde_json::from_str(output.trim()).context("parse provider synthesis tool plan")
}

fn parse_tool_name(value: &str) -> Result<SynthesisToolName> {
    match value {
        "search_observations" => Ok(SynthesisToolName::SearchObservations),
        "search_global_history" => Ok(SynthesisToolName::SearchGlobalHistory),
        "search_memory_cards" => Ok(SynthesisToolName::SearchMemoryCards),
        "search_skills" => Ok(SynthesisToolName::SearchSkills),
        "read_writing_guide" => Ok(SynthesisToolName::ReadWritingGuide),
        "find_memory_duplicates" => Ok(SynthesisToolName::FindMemoryDuplicates),
        "compare_with_skill" => Ok(SynthesisToolName::CompareWithSkill),
        "summarize_workflow_failures" => Ok(SynthesisToolName::SummarizeWorkflowFailures),
        "evaluate_skill_usefulness" => Ok(SynthesisToolName::EvaluateSkillUsefulness),
        other => Err(anyhow!("unknown or non-read-only synthesis tool `{other}`")),
    }
}

fn build_plan_request(candidate: &CandidateRecord, approvable_kind: &str) -> ProviderRequest {
    ProviderRequest {
        system_prompt: "You plan read-only tools for Agent Memory Kernel synthesis. Return JSON only. Do not write files, mutate configs, or include private chain-of-thought.".to_string(),
        user_prompt: serde_json::json!({
            "candidate": {
                "id": candidate.id,
                "title": candidate.title,
                "kind": candidate.kind,
                "normalized_kind": approvable_kind,
                "scope": candidate.scope,
                "body": candidate.body,
                "brief": candidate.brief,
                "tags": candidate.tags,
                "evidence": candidate.evidence,
            },
            "allowed_tools": [
                "search_observations",
                "search_memory_cards",
                "search_skills",
                "read_writing_guide"
            ],
            "instruction": "Pick only the read-only tools needed to decide whether this should become a Memory Card, merge, already-covered decision, Skill-targeted card, workflow card, or needs_human."
        }).to_string(),
        json_schema: Some(ProviderJsonSchema {
            name: "SynthesisToolPlan".to_string(),
            strict: true,
            schema: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "tool_calls": {
                        "type": "array",
                        "maxItems": MAX_PROVIDER_TOOL_CALLS,
                        "items": {
                            "type": "object",
                            "additionalProperties": false,
                            "properties": {
                                "tool": {
                                    "type": "string",
                                    "enum": [
                                        "search_observations",
                                        "search_memory_cards",
                                        "search_skills",
                                        "read_writing_guide"
                                    ]
                                },
                                "reason": { "type": "string" }
                            },
                            "required": ["tool", "reason"]
                        }
                    }
                },
                "required": ["tool_calls"]
            }),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::candidate::{CandidateStatus, ExtractionMetadata};
    use crate::config::{SkillIndex, SkillRecord, save_skill_index};
    use crate::extract::lifecycle::MemoryCardOperation;

    #[test]
    fn provider_tool_plan_can_produce_skill_gap_review() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        save_skill_index(
            root,
            &SkillIndex {
                generated_at: "now".to_string(),
                skills: vec![SkillRecord {
                    id: "project:ux-quality".to_string(),
                    name: "agent-kernel-ux-quality-pass".to_string(),
                    description: "Use when checking product UX quality.".to_string(),
                    source_path: ".agents/skills/agent-kernel-ux-quality-pass".to_string(),
                    source_kind: "project".to_string(),
                    source_hash: "hash".to_string(),
                    warnings: Vec::new(),
                }],
            },
        )
        .expect("skill index");
        observation::import_observation_text(
            root,
            &root.join("session.jsonl"),
            "manual-note",
            Some("codex"),
            "用户反馈 agent-kernel-ux-quality-pass 应该做真实 UX 试用，而不是只看只读字典。",
        )
        .expect("observation");
        let candidate = candidate(
            "provider-skill-gap",
            "补强 agent-kernel-ux-quality-pass",
            "补强 agent-kernel-ux-quality-pass skill：做真实 UX 试用，而不是只看只读字典。",
        );
        let env = load_environment(root, &candidate, "procedure").expect("environment");
        let review = run_with_tool_plan(
            root,
            &candidate,
            "procedure",
            env,
            ProviderToolPlan {
                tool_calls: vec![
                    call("search_observations"),
                    call("search_skills"),
                    call("read_writing_guide"),
                ],
            },
        )
        .expect("review");

        assert_eq!(review.proposal.action, "skill_targeted_card");
        assert!(review.metrics.counterfactual_pass);
        assert!(
            review
                .events
                .iter()
                .any(|event| event.tool == SynthesisToolName::EvaluateSkillUsefulness)
        );
    }

    #[test]
    fn provider_tool_plan_rejects_write_or_unknown_tools() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        let candidate = candidate("unsafe-tool", "危险工具", "尝试写文件。");
        let env = load_environment(root, &candidate, "procedure").expect("environment");
        let result = run_with_tool_plan(
            root,
            &candidate,
            "procedure",
            env,
            ProviderToolPlan {
                tool_calls: vec![call("write_memory_card")],
            },
        );

        assert!(result.is_err());
        assert!(
            result
                .expect_err("must fail")
                .to_string()
                .contains("unknown or non-read-only")
        );
    }

    #[test]
    fn provider_tool_plan_rejects_paused_global_observation_tools() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        let candidate = candidate(
            "paused-tool",
            "全局检索",
            "不要默认做全局 Observation 检索。",
        );
        let env = load_environment(root, &candidate, "procedure").expect("environment");
        let result = run_with_tool_plan(
            root,
            &candidate,
            "procedure",
            env,
            ProviderToolPlan {
                tool_calls: vec![call("search_global_history")],
            },
        );

        assert!(result.is_err());
        assert!(
            result
                .expect_err("must fail")
                .to_string()
                .contains("global Observation retrieval")
        );
    }

    fn call(tool: &str) -> ProviderToolCall {
        ProviderToolCall {
            tool: tool.to_string(),
            reason: "test".to_string(),
        }
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
            created_at: "2026-05-21T00:00:00Z".to_string(),
            updated_at: "2026-05-21T00:00:00Z".to_string(),
        }
    }
}
