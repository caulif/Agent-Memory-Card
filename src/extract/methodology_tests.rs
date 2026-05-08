use super::*;

#[test]
fn high_value_extraction_normalizes_methodology_drafts_into_actionable_rules() {
    let temp = tempfile::tempdir().expect("tempdir");
    let report = extract_high_value_text_to_drafts(
        temp.path(),
        "还要再智能一点，不要固定以后/必须/always/prefer 这类规则词，高价值 prompt 也可以。\n你对用户视角、体验、核心功能优先级的偏好。\n希望结果能支持持续自我修正。\n真实历史回归比样例更重要。\n重视真实历史回归，不迷信静态样例。",
        vec!["codex".to_string()],
        "test",
        Some("local".to_string()),
        true,
        8,
    )
    .expect("extract");

    assert!(
        report.candidates.iter().any(|candidate| {
            candidate.scope == "global"
                && candidate.body.contains("不要只依赖固定规则词")
                && candidate.body.contains("真实高价值表达")
        }),
        "keyword-rule extraction principle should be global and actionable: {:?}",
        report.candidates
    );
    assert!(
        report.candidates.iter().any(|candidate| {
            candidate
                .body
                .contains("开发评估优先关注核心功能、用户视角与体验质量")
        }),
        "meta user-perspective description should be rewritten as an actionable rule: {:?}",
        report.candidates
    );
    assert!(
        report.candidates.iter().any(|candidate| {
            candidate
                .body
                .contains("提炼结果应支持基于真实反馈持续自我修正")
        }),
        "self-correction wording should be concrete: {:?}",
        report.candidates
    );
    assert_eq!(
        report
            .candidates
            .iter()
            .filter(|candidate| candidate.body.contains("真实历史回归"))
            .count(),
        1,
        "real-history variants should collapse to one candidate: {:?}",
        report.candidates
    );
}

#[test]
fn high_value_extraction_rejects_replay_artifact_and_current_state_complaint() {
    let temp = tempfile::tempdir().expect("tempdir");
    let report = extract_high_value_text_to_drafts(
        temp.path(),
        "cross-project-principle\n优化架构，现在的架构功能也实现不了，体验也很不好。\n小改快测，大改重测。",
        vec!["codex".to_string()],
        "test",
        Some("local".to_string()),
        true,
        8,
    )
    .expect("extract");

    assert!(
        report
            .candidates
            .iter()
            .all(|candidate| !candidate.id.contains("cross-project-principl")
                && !candidate.body.contains("现在的架构")),
        "artifact and current-state complaint should not survive: {:?}",
        report.candidates
    );
    assert!(
        report
            .candidates
            .iter()
            .any(|candidate| candidate.body.contains("小改快测")),
        "valid methodology should still survive: {:?}",
        report.candidates
    );
}

#[test]
fn high_value_extraction_cleans_existing_candidate_prefixes_from_review_boundary() {
    let temp = tempfile::tempdir().expect("tempdir");
    let report = extract_high_value_text_to_drafts(
        temp.path(),
        "global:协作阶段：先 review 再 merge，先保留审阅边界，不要让 AI 直接固化规则。",
        vec!["codex".to_string()],
        "test",
        Some("local".to_string()),
        true,
        8,
    )
    .expect("extract");

    assert!(
        report.candidates.iter().any(|candidate| {
            candidate.body == "先 review 再 merge，保留人工审阅边界，不要让 AI 直接固化规则。"
                && !candidate.id.starts_with("global:global-")
        }),
        "review-boundary candidate should be clean: {:?}",
        report.candidates
    );
}

#[test]
fn high_value_extraction_refines_final_candidates_into_agent_memory_rules() {
    let temp = tempfile::tempdir().expect("tempdir");
    let report = extract_high_value_text_to_drafts(
        temp.path(),
        "project:保持使用过程流畅-避免每个操作都触发明显卡顿\n\
global:候选质量优先于候选数量\n\
global:提炼结果应支持基于真实反馈持续自我修正\n\
global:重视真实历史回归-不迷信静态样例\n\
global:先-review-再-merge-保留人工审阅边界-不要让-ai-直接固化规则\n\
global:小改快测-大改重测\n\
global:自己用推理引擎跑一下真实结果-检查有没有问题-质\n\
project:保留例外-do-not-return-more-than-3\n\
project:不该记的内容直接过滤掉就行了-不要展示为啥不改记",
        vec!["codex".to_string()],
        "observation synthesis test",
        Some("local".to_string()),
        true,
        10,
    )
    .expect("extract");

    assert!(
        report.candidates.iter().all(|candidate| {
            !candidate.body.contains("当生成、筛选或展示候选记忆时")
                && !candidate.body.contains("候选质量优先于数量")
        }),
        "candidate-quality pipeline meta should be filtered: {:?}",
        report.candidates
    );
    assert!(
        report.candidates.iter().any(|candidate| {
            candidate.body.contains("真实历史会话做回归验证")
                && candidate.body.contains("不要只依赖静态样例")
        }),
        "real-history rule should be executable memory: {:?}",
        report.candidates
    );
    assert!(
        report.candidates.iter().any(|candidate| {
            candidate
                .body
                .contains("候选规则、MemoryCard 或关键变更准备固化")
                && candidate.memory_tier == crate::candidate::MemoryTier::ProjectRule
                && candidate.scope == "project"
        }),
        "MemoryCard review/merge boundary should remain project-scoped: {:?}",
        report.candidates
    );
    assert!(
        report.candidates.iter().any(|candidate| {
            candidate
                .body
                .contains("当准备提交最终代码、分析结论或复杂任务结果时")
                && candidate.body.contains("dry-run 自检")
        }),
        "self-verification request should be rewritten into durable memory: candidates={:?} skipped={:?}",
        report.candidates,
        report.skipped
    );
    assert!(
        report
            .candidates
            .iter()
            .all(|candidate| !candidate.body.contains("检查有没有问题-质")),
        "truncated raw task phrasing should not survive: {:?}",
        report.candidates
    );
    assert!(
        report.candidates.iter().all(|candidate| {
            !candidate.body.contains("无长期价值或低质量")
                && !candidate.body.contains("只让用户审阅真正值得固化的记忆")
        }),
        "low-quality filtering pipeline meta should be filtered: {:?}",
        report.candidates
    );
    assert!(
        report
            .candidates
            .iter()
            .all(|candidate| !candidate.id.contains("do-not-return-more-than-3")),
        "example fragments should not survive as project IDs: {:?}",
        report.candidates
    );
}

#[test]
fn self_verification_signal_is_candidate_before_refine() {
    let candidate =
        self_verification_candidate("global:自己用推理引擎跑一下真实结果-检查有没有问题-质")
            .expect("self-verification candidate");

    assert_eq!(
        candidate.memory_tier,
        crate::candidate::MemoryTier::CollaborationPreference
    );

    let candidates = extract_high_value_candidates_with_preferences(
        "global:自己用推理引擎跑一下真实结果-检查有没有问题-质",
        &built_in_preferences(),
        10,
        false,
    );
    assert!(
        candidates
            .iter()
            .any(|candidate| candidate.matched_template.as_deref()
                == Some("self-verification-signal")),
        "{candidates:?}"
    );
    let candidate = candidates
        .into_iter()
        .find(|candidate| candidate.matched_template.as_deref() == Some("self-verification-signal"))
        .expect("candidate");
    let action = crate::candidate::ExtractionAction::new_candidate();
    let quality = evaluate_candidate_quality(&candidate, &action);
    let memory = memory_gate::evaluate_memory_candidate(
        &candidate.title,
        &candidate.body,
        &candidate.evidence,
        &candidate.kind,
        &candidate.scope,
    );
    assert_eq!(quality.disposition, QualityDisposition::Keep);
    assert_ne!(
        memory.disposition,
        memory_gate::MemoryGateDisposition::Reject,
        "{memory:?}"
    );

    let batch = extract_high_value_candidates_with_preferences(
        "project:保持使用过程流畅-避免每个操作都触发明显卡顿\n\
global:候选质量优先于候选数量\n\
global:提炼结果应支持基于真实反馈持续自我修正\n\
global:重视真实历史回归-不迷信静态样例\n\
global:先-review-再-merge-保留人工审阅边界-不要让-ai-直接固化规则\n\
global:小改快测-大改重测\n\
global:自己用推理引擎跑一下真实结果-检查有没有问题-质",
        &built_in_preferences(),
        10,
        false,
    );
    assert!(
        batch
            .iter()
            .any(|candidate| candidate.matched_template.as_deref()
                == Some("self-verification-signal")),
        "{batch:?}"
    );
}
