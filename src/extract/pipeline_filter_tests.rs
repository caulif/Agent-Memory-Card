use super::*;

#[test]
fn filters_current_ui_feedback_before_candidate_creation() {
    let temp = tempfile::tempdir().expect("tempdir");
    let report = extract_high_value_text_to_drafts(
        temp.path(),
        "有很多重复的，我不知道是之前的残留还是现有bug，我希望你想办法解决重复这个问题，分配页面我也希望可以实现卡片拖动的效果。",
        vec!["codex".to_string()],
        "observation synthesis, chunk 1/1",
        Some("local".to_string()),
        true,
        8,
    )
    .expect("extract");

    assert!(
        report.candidates.is_empty(),
        "current task feedback should not become durable memory: {:?}",
        report.candidates
    );
}

#[test]
fn filters_memory_pipeline_rules_after_refine() {
    let temp = tempfile::tempdir().expect("tempdir");
    let report = extract_high_value_text_to_drafts(
        temp.path(),
        "当生成、筛选或展示候选记忆时，只保留高置信度且可执行的少量候选；目标是让候选质量优先于数量。",
        vec!["codex".to_string()],
        "observation synthesis, chunk 1/1",
        Some("local".to_string()),
        true,
        8,
    )
    .expect("extract");

    assert!(
        report
            .candidates
            .iter()
            .all(|candidate| !candidate.body.contains("候选记忆")
                && !candidate.body.contains("候选质量")),
        "memory pipeline meta rules should be filtered after refinement: {:?}",
        report.candidates
    );
}

#[test]
fn filters_generic_planning_principle_after_refine() {
    let temp = tempfile::tempdir().expect("tempdir");
    let report = extract_high_value_text_to_drafts(
        temp.path(),
        "当规划或评估开发工作时，先确保核心功能链路和用户体验稳定；再考虑周边功能。",
        vec!["codex".to_string()],
        "observation synthesis, chunk 1/1",
        Some("local".to_string()),
        true,
        8,
    )
    .expect("extract");

    assert!(
        report.candidates.iter().all(|candidate| {
            !candidate.body.contains("当规划或评估开发工作时")
                && !candidate.body.contains("核心功能链路")
        }),
        "generic planning principle should not stay in review queue: {:?}",
        report.candidates
    );
}
