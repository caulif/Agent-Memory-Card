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
        "manual-note",
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
    assert_eq!(review.proposal.action, "skill_targeted_card");
    assert_eq!(
        review.proposal.target_context.target_id.as_deref(),
        Some("project:ux-quality")
    );
    let usefulness = review
        .proposal
        .skill_usefulness
        .as_ref()
        .expect("skill usefulness");
    assert_eq!(usefulness.verdict, "counterfactual_pass");
    assert!(
        usefulness
            .improved_axes
            .contains(&"instruction".to_string())
    );
    assert!(review.metrics.counterfactual_pass);
    assert!(review.metrics.approval_candidate);
    assert_eq!(review.stop_reason, SynthesisStopReason::SkillGapFound);
}

#[test]
fn synthesis_requires_counterfactual_skill_improvement() {
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
        "agent-kernel-ux-quality-pass: Use when checking product UX quality.",
    )
    .expect("observation");
    let candidate = candidate(
        "skill-restatement",
        "agent-kernel-ux-quality-pass",
        "Use when checking product UX quality.",
    );

    let review = run_memory_card_synthesis(root, &candidate, "procedure").expect("review");

    assert_eq!(review.proposal.card_function, "skill_targeted");
    assert_eq!(review.proposal.action, "needs_human");
    assert_eq!(review.stop_reason, SynthesisStopReason::NeedsHuman);
    let usefulness = review
        .proposal
        .skill_usefulness
        .as_ref()
        .expect("skill usefulness");
    assert_eq!(usefulness.verdict, "no_meaningful_change");
    assert!(usefulness.improved_axes.is_empty());
    assert!(!review.metrics.counterfactual_pass);
    assert!(review.metrics.needs_human);
    assert!(review.metrics.no_card_decision);
    assert!(
        review
            .events
            .iter()
            .any(|event| event.tool == SynthesisToolName::EvaluateSkillUsefulness)
    );
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
        "manual-note",
        Some("codex"),
        "页面重构后要按真实用户路径试用并记录问题，不要只看编译。",
    )
    .expect("observation");
    let candidate = candidate(
        "memory-card-new",
        "补充用真实试用验证 UX",
        "补充既有规则：页面重构后要按真实用户路径试用并记录问题，不要只看编译。",
    );

    let review = run_memory_card_synthesis(root, &candidate, "procedure").expect("review");

    assert_eq!(review.proposal.card_function, "merge");
    let matched_card = review
        .context
        .memory_cards
        .iter()
        .find(|matched| matched.id == "memory-card-existing")
        .expect("matched memory card");
    assert_eq!(matched_card.merge_hint, "merge_candidate");
    assert!(matched_card.overlap_summary.contains("strongly overlaps"));
    assert!(matched_card.gap_summary.contains("new trigger"));
    assert_eq!(
        review.proposal.merge_target_id.as_deref(),
        Some("memory-card-existing")
    );
    assert_eq!(review.stop_reason, SynthesisStopReason::MergeTargetFound);
}

#[test]
fn synthesis_marks_exact_existing_card_as_already_covered() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    memory_card::add_memory_card(
        root,
        "memory-card-existing",
        "真实试用验证 UX",
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
        "完成页面重构后，按真实用户路径试用并记录问题，不要只凭编译通过判断完成。",
    )
    .expect("observation");
    let candidate = candidate(
        "memory-card-covered",
        "真实试用验证 UX",
        "完成页面重构后，按真实用户路径试用并记录问题，不要只凭编译通过判断完成。",
    );

    let review = run_memory_card_synthesis(root, &candidate, "procedure").expect("review");

    assert_eq!(review.proposal.action, "already_covered");
    assert!(review.metrics.no_card_decision);
    assert!(review.metrics.duplicate_suppressed);
    assert!(
        review
            .context
            .memory_cards
            .iter()
            .any(|matched| matched.merge_hint == "already_covered_candidate")
    );
    assert_eq!(
        review.proposal.target_context.target_id.as_deref(),
        Some("memory-card-existing")
    );
    assert_eq!(review.stop_reason, SynthesisStopReason::AlreadyCovered);
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

#[test]
fn synthesis_reads_same_source_history_beyond_direct_evidence() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    let session = root.join("session.txt");
    let imported = observation::import_observation_text(
        root,
        &session,
        "manual-note",
        Some("codex"),
        "用户指出 MemoryCards 页面布局看不完整，需要优化卡片阅读区域。",
    )
    .expect("observation");
    observation::import_observation_text(
        root,
        &session,
        "manual-note",
        Some("codex"),
        "随后又反馈 Skills 页面应区分项目级和全局 Skills，避免错误挂载。",
    )
    .expect("observation");
    let mut candidate = candidate(
        "same-source-context",
        "优化 MemoryCards 阅读布局",
        "MemoryCards 页面布局看不完整，需要优化阅读区域。",
    );
    candidate.source_observations = imported.observations;

    let review = run_memory_card_synthesis(root, &candidate, "procedure").expect("review");

    assert!(
        review
            .context
            .related_observations
            .iter()
            .any(|snippet| snippet.relation == "same_source_context"),
        "{:#?}",
        review.context.related_observations
    );
    assert!(
        review
            .events
            .iter()
            .any(|event| event.tool == SynthesisToolName::SearchGlobalHistory)
    );
}

#[test]
fn synthesis_summarizes_repeated_workflow_failures_from_history() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    observation::import_observation_text(
        root,
        &root.join("history-a.txt"),
        "manual-note",
        Some("codex"),
        "测试的时候必须看看真实数据上生成了什么记忆卡片，然后结合目标审核，分析问题和解决方案，修改优化直到符合预期。",
    )
    .expect("observation");
    observation::import_observation_text(
        root,
        &root.join("history-b.txt"),
        "manual-note",
        Some("codex"),
        "不能只看指标，要自己看最终卡片质量。",
    )
    .expect("observation");
    let candidate = candidate(
        "workflow-failure-summary",
        "用真实历史复核 Memory Card 质量",
        "生成 Memory Card 后要结合真实历史和目标审核，持续修改优化直到符合预期。",
    );

    let review = run_memory_card_synthesis(root, &candidate, "procedure").expect("review");

    assert!(
        review
            .context
            .workflow_failures
            .iter()
            .any(|failure| failure.kind == "review_iterate_loop"),
        "{:#?}",
        review.context.workflow_failures
    );
    assert!(
        review
            .events
            .iter()
            .any(|event| event.tool == SynthesisToolName::SummarizeWorkflowFailures)
    );
}

#[test]
fn synthesis_explains_project_skill_target_and_global_reference_roles() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    save_skill_index(
        root,
        &SkillIndex {
            generated_at: "now".to_string(),
            skills: vec![
                SkillRecord {
                    id: "project:skill-creator".to_string(),
                    name: "skill-creator".to_string(),
                    description: "Create project Skills with clear Use When and Boundaries."
                        .to_string(),
                    source_path: ".agents/skills/skill-creator".to_string(),
                    source_kind: "project".to_string(),
                    source_hash: "hash".to_string(),
                    warnings: Vec::new(),
                },
                SkillRecord {
                    id: "global:skill-creator".to_string(),
                    name: "skill-creator".to_string(),
                    description: "Create reusable global Skills.".to_string(),
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
        &root.join("skill-feedback.txt"),
        "manual-note",
        Some("codex"),
        "用户反馈要补强 skill-creator，让它生成 Memory Card 时写清 Use When、Instructions 和 Boundaries。",
    )
    .expect("observation");
    let candidate = candidate(
        "skill-explain",
        "补强 skill-creator 的 Memory Card 写法",
        "优化 skill-creator skill：生成 Memory Card 时必须写清 Use When、Instructions 和 Boundaries。",
    );

    let review = run_memory_card_synthesis(root, &candidate, "procedure").expect("review");

    let project_skill = review
        .context
        .skills
        .iter()
        .find(|skill| skill.id == "project:skill-creator")
        .expect("project skill");
    assert_eq!(project_skill.target_role, "project_target");
    assert!(project_skill.coverage_summary.contains("Use When"));
    assert!(project_skill.gap_summary.contains("missing trigger"));

    let global_skill = review
        .context
        .skills
        .iter()
        .find(|skill| skill.id == "global:skill-creator")
        .expect("global skill");
    assert_eq!(global_skill.target_role, "global_reference");
    assert!(global_skill.gap_summary.contains("inform wording"));
}

#[test]
fn synthesis_needs_human_when_no_local_context_exists() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    let mut candidate = candidate("no-context", "模糊偏好", "请以后注意这个事情。");
    candidate.evidence.clear();
    candidate.confidence = Some(0.9);

    let review = run_memory_card_synthesis(root, &candidate, "procedure").expect("review");

    assert_eq!(review.stop_reason, SynthesisStopReason::NeedsHuman);
    assert!(review.context.observations.is_empty());
    assert!(review.context.related_observations.is_empty());
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
