use crate::candidate::ExtractionAction;

use super::Candidate;
use super::lifecycle::MemoryCardOperation;

#[derive(Debug, Clone)]
pub(crate) struct QualityGateDecision {
    pub operation: MemoryCardOperation,
    pub disposition: QualityDisposition,
    pub reason: String,
    pub flags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum QualityDisposition {
    Keep,
    Skip,
}

pub(crate) fn evaluate_candidate_quality(
    candidate: &Candidate,
    action: &ExtractionAction,
) -> QualityGateDecision {
    let combined = format!(
        "{}\n{}\n{}\n{}",
        candidate.title,
        candidate.body,
        candidate.evidence,
        candidate.reason.as_deref().unwrap_or_default()
    );
    let lower = combined.to_lowercase();
    let candidate_text = format!("{}\n{}", candidate.title, candidate.body).to_lowercase();

    if contains_internal_leak(&candidate_text) {
        return skip(
            MemoryCardOperation::Noop,
            "internal-leak",
            "Candidate contains internal observation/evidence metadata instead of a reusable rule.",
        );
    }

    if looks_like_generated_instruction_artifact(&lower) {
        return skip(
            MemoryCardOperation::Noop,
            "generated-instruction-artifact",
            "Candidate came from generated AGENTS/CLAUDE instructions rather than user-authored memory.",
        );
    }

    if looks_like_generation_quality_acceptance_chatter(&lower) {
        return skip(
            MemoryCardOperation::Noop,
            "memory-pipeline-meta",
            "Candidate is about Memory Card generation/evaluation mechanics, not durable user preference.",
        );
    }

    if looks_like_meta_discussion(&lower) {
        return skip(
            MemoryCardOperation::Noop,
            "meta-discussion",
            "Candidate is about planning or implementation status, not durable agent behavior.",
        );
    }

    if looks_like_temporary_task_constraint(&lower) {
        return skip(
            MemoryCardOperation::Noop,
            "temporary-task-constraint",
            "Candidate reflects a one-off task brief or write-scope boundary, not a durable project rule.",
        );
    }

    if looks_like_package_manager_preference(&lower) {
        return skip(
            MemoryCardOperation::Noop,
            "package-manager-preference",
            "Package manager preferences are intentionally excluded from MemoryCard extraction.",
        );
    }

    if action.action == "merge_into_existing" && action.similarity.unwrap_or(0.0) >= 0.95 {
        return skip(
            MemoryCardOperation::Noop,
            "duplicate-existing",
            "Equivalent MemoryCard already exists; suppressing duplicate candidate.",
        );
    }

    if action.action == "noop" {
        return skip(
            MemoryCardOperation::Noop,
            "llm-noop",
            action.reason.as_deref().unwrap_or(
                "LLM update phase marked this candidate as already covered or low value.",
            ),
        );
    }

    if !is_known_high_value_template(candidate) && !looks_self_contained_rule(&candidate.body) {
        return skip(
            MemoryCardOperation::Noop,
            "not-self-contained-rule",
            "Candidate body is not a self-contained future rule.",
        );
    }

    QualityGateDecision {
        operation: operation_from_action(action),
        disposition: QualityDisposition::Keep,
        reason: "candidate passed quality gate".to_string(),
        flags: Vec::new(),
    }
}

fn is_known_high_value_template(candidate: &Candidate) -> bool {
    matches!(
        candidate.matched_template.as_deref(),
        Some(
            "project-improvement"
                | "high-value-prompt"
                | "atomic-exception"
                | "self-verification-signal"
                | "deterministic-memory-refine"
                | "llm-memory-refine"
        )
    )
}

pub(crate) fn quality_skip_message(id: &str, decision: &QualityGateDecision) -> String {
    format!("{id}: {} ({})", decision.flags.join(","), decision.reason)
}

fn skip(operation: MemoryCardOperation, flag: &str, reason: &str) -> QualityGateDecision {
    QualityGateDecision {
        operation,
        disposition: QualityDisposition::Skip,
        reason: reason.to_string(),
        flags: vec![flag.to_string()],
    }
}

fn contains_internal_leak(lower: &str) -> bool {
    let markers = [
        "evidence:",
        "observation:",
        "observations:",
        "observation synthesis",
        "sha256",
        "source_observations",
        "title:",
        "score:",
        "matched_signal",
        "high-value durable",
        "模板:",
        "merged suggestion",
        "similarity 100",
        "相似度 100",
        "合并建议",
    ];
    markers.iter().any(|marker| lower.contains(marker))
}

fn looks_like_meta_discussion(lower: &str) -> bool {
    if lower.contains("不要只依赖固定规则词") && lower.contains("真实高价值表达") {
        return false;
    }
    if (lower.contains("提交最终代码") || lower.contains("复杂任务结果前"))
        && (lower.contains("自检") || lower.contains("dry-run") || lower.contains("真实输入"))
    {
        return false;
    }

    let question_like = lower.contains("是否")
        || lower.contains("有没有")
        || lower.contains("should we")
        || lower.contains("do we already")
        || lower.contains("have we");
    let system_analysis = (lower.contains("提炼器")
        || lower.contains("当前系统")
        || lower.contains("signal")
        || lower.contains("信号")
        || lower.contains("candidate")
        || lower.contains("候选")
        || lower.contains("候选记忆")
        || lower.contains("记忆")
        || lower.contains("抽取")
        || lower.contains("筛选")
        || lower.contains("classification")
        || lower.contains("classify")
        || lower.contains("prompt"))
        && (lower.contains("识别")
            || lower.contains("命中")
            || lower.contains("更擅长")
            || lower.contains("不擅长")
            || lower.contains("抽象")
            || lower.contains("方法论")
            || lower.contains("当前"));
    let pipeline_terms = [
        "memory_card",
        "candidate",
        "候选",
        "候选记忆",
        "记忆",
        "draft",
        "hook",
        "compile",
        "编译",
        "提炼",
        "抽取",
        "筛选",
        "规划",
        "计划",
        "实现",
    ];
    let candidate_pipeline_rule = (lower.contains("候选记忆")
        || (lower.contains("候选质量") && lower.contains("候选数量"))
        || lower.contains("候选被判定为无长期价值")
        || lower.contains("真正值得固化的记忆")
        || (lower.contains("生成") && lower.contains("筛选") && lower.contains("候选")))
        && (lower.contains("只保留")
            || lower.contains("高置信度")
            || lower.contains("可执行")
            || lower.contains("过滤")
            || lower.contains("质量优先"));
    let generic_planning_principle = (lower.contains("当规划或评估开发工作时")
        || (lower.contains("核心功能链路") && lower.contains("周边功能")))
        && !lower.contains("所有项目")
        && !lower.contains("每次");
    let current_product_feedback = (lower.contains("残留")
        || lower.contains("现有bug")
        || lower.contains("现有 bug")
        || lower.contains("重复这个问题")
        || lower.contains("图片中这几个完全是不对")
        || lower.contains("分配页面")
        || lower.contains("调用claude code")
        || lower.contains("调用 claude code"))
        && (lower.contains("我希望") || lower.contains("想办法") || lower.contains("需要实现"));
    (question_like && pipeline_terms.iter().any(|term| lower.contains(term)))
        || system_analysis
        || candidate_pipeline_rule
        || generic_planning_principle
        || current_product_feedback
}

fn looks_like_generation_quality_acceptance_chatter(lower: &str) -> bool {
    let generation_surface = [
        "prompt",
        "render",
        "rewrite",
        "llm",
        "模型",
        "卡片",
        "最终卡片",
        "成品卡片",
        "改写器",
        "memory card",
        "candidate",
        "候选",
        "生成",
        "提炼",
        "筛选",
    ]
    .iter()
    .filter(|marker| lower.contains(**marker))
    .count()
        >= 2;
    let eval_surface = [
        "抽样",
        "最终卡片",
        "分数",
        "指标",
        "人工看",
        "人工审阅",
        "质量",
        "评估",
        "golden",
        "eval",
        "score",
        "metric",
        "sample",
    ]
    .iter()
    .filter(|marker| lower.contains(**marker))
    .count()
        >= 2;
    let process_instruction = [
        "要",
        "不要",
        "不能只",
        "不应该只",
        "必须",
        "should",
        "must",
        "not only",
        "don't just",
    ]
    .iter()
    .any(|marker| lower.contains(marker));

    generation_surface && eval_surface && process_instruction
}

fn looks_like_generated_instruction_artifact(lower: &str) -> bool {
    let generated_markers = [
        "enabled memory cards",
        "enabled skills",
        "generated by agent-kernel",
        "do not edit directly",
        "included with the developer message",
        "don't need to be re-read",
        "do not need to be re-read",
        "contents of the agents.md file",
        "contents of the claude.md file",
        "prefer spawning the preset",
        "preset's specialty",
        "parallelize tool calls whenever you can",
        "file reads such as `cat`, `rg`, `sed`, `ls`, `git show`, `nl`, and `wc`",
        "the user does not see command execution outputs",
        "you and the user share the same workspace",
        "let principle_topics",
        "principle_named_terms",
        "principle named terms",
        "let planning_topics",
        "fn has_principle_signal",
        "pub(super) fn",
        "memorytier::projectrule",
        "expected_memory_tier",
        "must_be_extracted",
        "must_reject_reason",
        "precision@10",
        "cohen_kappa",
        "gold set",
        "gold recall",
        "src/extract/",
        "整体方案",
        "三大改动",
        "按优先级",
        "验收：",
        "验收标准",
        "质量报告",
        "优先级建议",
        "acceptance criteria",
        "non-dry-run",
        "fixtures",
        "same id",
        "promote fixtures",
        "source ids",
        "artifact drifts",
        "import artifact drifts",
        "大段生成内容漂移",
        "误导成新增",
        "fuse memory_cards",
        "medium/high risk commands",
        "medium high risk commands",
        "backend issued decision tokens",
        "backend-issued decision tokens",
        "do not use those",
        "do-not-open-with-task",
        "do not open with task",
        "src/build.rs",
        "always-on-rule",
        "do not revert others edits",
        "do not revert others' edits",
        "若遇到 429",
        "遇到 429",
        "不要直接停止",
        "需要你拍板",
    ];
    generated_markers
        .iter()
        .any(|marker| lower.contains(marker))
}

fn looks_like_temporary_task_constraint(lower: &str) -> bool {
    if has_durable_scope_marker(lower) {
        return false;
    }

    let path_or_file_scope = lower.contains("src/")
        || lower.contains("app/")
        || lower.contains(".rs")
        || lower.contains(".tsx")
        || lower.contains(".ts")
        || lower.contains("当前项目：")
        || lower.contains("project path:");
    let scope_markers = [
        "写入范围仅限",
        "modify only",
        "edit only",
        "do not modify files outside",
        "不要改前端",
        "不要改 rust",
        "do not touch rust or root",
        "不要改文件",
        "只读审查",
        "read-only review",
        "只修改 ",
        "review src/",
        "inspect the current diff",
        "scope for this run",
        "implement tasks",
        "current failing command",
        "read these files first",
        "return json:",
    ];
    let matched_scope = scope_markers.iter().any(|marker| lower.contains(marker));
    let one_off_negative_scope = [
        "不要改 rust",
        "不要改前端",
        "不要改后端",
        "不要改文件",
        "不要修改文件",
        "不要编辑文件",
        "不要动 rust",
        "不要动前端",
        "只修改",
        "仅修改",
        "只改 ",
        "只输出",
        "输出测试清单",
        "do not touch rust",
        "do not modify",
        "do not edit",
        "modify only",
        "edit only",
        "scope for this run",
    ]
    .iter()
    .any(|marker| lower.contains(marker));
    let review_only_markers = [
        "只读审查",
        "不要改文件",
        "不要修改文件",
        "不要编辑文件",
        "只输出",
        "if no changes are needed",
        "say so with caveats",
        "return json:",
        "do not touch rust or root",
    ];
    let review_only = review_only_markers
        .iter()
        .any(|marker| lower.contains(marker));

    (path_or_file_scope && matched_scope)
        || one_off_negative_scope
        || review_only
        || lower.starts_with("return json:")
        || lower.contains("只输出简洁的改动方案")
        || lower.contains("先整理一下仓库")
        || lower.contains("以本地为准")
        || lower.contains("先规划一个修改方案")
        || lower.contains("制定一个修改计划")
        || lower.contains("拆成可交给 codex 实现")
        || lower.contains("这次不要参考 memory")
        || lower.contains("这次不要参考memory")
        || lower.contains("不要参考 memory")
        || lower.contains("不要参考memory")
        || lower.contains("本轮不要参考")
        || lower.contains("本次不要参考")
}

fn has_durable_scope_marker(lower: &str) -> bool {
    [
        "以后",
        "所有项目",
        "每个项目",
        "每次",
        "长期",
        "默认",
        "统一",
        "总是",
        "always",
        "for every project",
        "for all projects",
        "by default",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn looks_like_package_manager_preference(lower: &str) -> bool {
    let package_tool = ["bun", "npm", "pnpm", "yarn"]
        .iter()
        .any(|marker| lower.contains(marker));
    let package_context = [
        "包管理",
        "依赖",
        "脚本",
        "package manager",
        "package management",
        "javascript package",
        "js package",
        "scripts",
        "bun run",
    ]
    .iter()
    .any(|marker| lower.contains(marker));
    package_tool && package_context
}

fn looks_self_contained_rule(body: &str) -> bool {
    let trimmed = body.trim();
    if trimmed.len() < 10 {
        return false;
    }
    if trimmed.ends_with('?') || trimmed.ends_with('？') {
        return false;
    }
    let lower = trimmed.to_lowercase();
    let rule_markers = [
        "use ",
        "prefer ",
        "always ",
        "never ",
        "must ",
        "do not ",
        "don't ",
        "keep ",
        "avoid ",
        "default to ",
        "focus on ",
        "prioritize ",
        "evaluate ",
        "validate ",
        "reserve ",
        "before implementation",
        "ask clarifying questions",
        "默认",
        "统一",
        "优先",
        "必须",
        "不要",
        "禁止",
        "保留",
        "先",
        "走",
        "关注核心功能",
        "核心功能优先",
        "用户视角",
        "用户体验",
        "体验优先",
        "真实历史",
        "审阅边界",
        "持续自我修正",
        "自我修正",
        "候选质量",
        "小改快测",
        "大改重测",
        "先提问",
        "先澄清",
        "先规划",
        "用",
        "使用",
        "采用",
        "选用",
        "改用",
        "替代",
        "而不是",
        "仅当",
        "只有",
        "如果",
    ];
    if rule_markers.iter().any(|marker| lower.contains(marker)) {
        return true;
    }

    let tool_markers = [
        "ky", "pnpm", "bun", "axios", "ofetch", "biome", "oxlint", "vitest", "tanstack", "turbo",
    ];
    let durable_markers = ["以后", "默认", "统一", "prefer", "use", "must", "always"];
    tool_markers.iter().any(|tool| lower.contains(tool))
        && durable_markers.iter().any(|marker| lower.contains(marker))
}

fn operation_from_action(action: &ExtractionAction) -> MemoryCardOperation {
    match action.action.as_str() {
        "merge_into_existing" => MemoryCardOperation::Update,
        "conflict" => MemoryCardOperation::Conflict,
        "supersede" => MemoryCardOperation::Supersede,
        "noop" => MemoryCardOperation::Noop,
        _ => MemoryCardOperation::Add,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(title: &str, body: &str) -> Candidate {
        Candidate {
            title: title.to_string(),
            body: body.to_string(),
            kind: "preference".to_string(),
            scope: "project".to_string(),
            memory_tier: crate::candidate::MemoryTier::ProjectRule,
            abstraction_of: None,
            abstracted_from: None,
            evidence: body.to_string(),
            confidence: Some(0.9),
            reason: None,
            matched_template: None,
        }
    }

    #[test]
    fn rejects_internal_evidence_leaks() {
        let decision = evaluate_candidate_quality(
            &candidate(
                "evidence: 81 observations",
                "observation:obs:claude-code:sha256:abc - Do NOT mention teammate proposals.",
            ),
            &ExtractionAction::new_candidate(),
        );

        assert_eq!(decision.disposition, QualityDisposition::Skip);
        assert_eq!(decision.flags, vec!["internal-leak"]);
    }

    #[test]
    fn rejects_meta_discussion_questions() {
        let decision = evaluate_candidate_quality(
            &candidate(
                "是否已经把 MemoryCard 编译为 hook",
                "是否已经把 MemoryCard 编译为 hook，例如 git commit 前运行 cargo clippy？",
            ),
            &ExtractionAction::new_candidate(),
        );

        assert_eq!(decision.disposition, QualityDisposition::Skip);
        assert_eq!(decision.flags, vec!["meta-discussion"]);
    }

    #[test]
    fn rejects_generation_quality_acceptance_chatter() {
        let decision = evaluate_candidate_quality(
            &candidate(
                "修改卡片改写器要抽样审阅成品",
                "修改卡片改写器时，必须抽样审阅最终卡片，不要只看质量分。",
            ),
            &ExtractionAction::new_candidate(),
        );

        assert_eq!(decision.disposition, QualityDisposition::Skip);
        assert_eq!(decision.flags, vec!["memory-pipeline-meta"]);
    }

    #[test]
    fn rejects_generated_agent_instruction_artifacts() {
        let decision = evaluate_candidate_quality(
            &candidate(
                "Protect AGENTS.md Drift Review",
                "The contents of the AGENTS.md file at the root of the repo and any directories from the CWD up to the root are included with the developer message and don't need to be re-read",
            ),
            &ExtractionAction::new_candidate(),
        );

        assert_eq!(decision.disposition, QualityDisposition::Skip);
        assert_eq!(decision.flags, vec!["generated-instruction-artifact"]);
    }

    #[test]
    fn rejects_enabled_memory_card_artifact_blocks() {
        let decision = evaluate_candidate_quality(
            &candidate(
                "Use Axios",
                "## Enabled Memory Cards\n\n### Use Axios\n\nUse Axios for frontend HTTP requests",
            ),
            &ExtractionAction::new_candidate(),
        );

        assert_eq!(decision.disposition, QualityDisposition::Skip);
        assert_eq!(decision.flags, vec!["generated-instruction-artifact"]);
    }

    #[test]
    fn rejects_preset_spawn_instruction_artifacts() {
        let decision = evaluate_candidate_quality(
            &candidate(
                "When a task matches a preset's",
                "When a task matches a preset's specialty, prefer spawning the preset over a generic CLI agent",
            ),
            &ExtractionAction::new_candidate(),
        );

        assert_eq!(decision.disposition, QualityDisposition::Skip);
        assert_eq!(decision.flags, vec!["generated-instruction-artifact"]);
    }

    #[test]
    fn rejects_parallelize_tool_calls_instruction_artifacts() {
        let decision = evaluate_candidate_quality(
            &candidate(
                "You parallelize tool calls whenever",
                "You parallelize tool calls whenever you can, especially file reads such as `cat`, `rg`, `sed`, `ls`, `git show`, `nl`, and `wc`",
            ),
            &ExtractionAction::new_candidate(),
        );

        assert_eq!(decision.disposition, QualityDisposition::Skip);
        assert_eq!(decision.flags, vec!["generated-instruction-artifact"]);
    }

    #[test]
    fn rejects_one_off_task_scope_constraints() {
        let decision = evaluate_candidate_quality(
            &candidate(
                "Do not touch Rust or root",
                "Do not touch Rust or root package scripts. Modify only app/src/main.tsx and app/src/ui-helpers.ts in this run.",
            ),
            &ExtractionAction::new_candidate(),
        );

        assert_eq!(decision.disposition, QualityDisposition::Skip);
        assert_eq!(decision.flags, vec!["temporary-task-constraint"]);
    }

    #[test]
    fn rejects_standalone_do_not_touch_rust_scope_boundary() {
        let decision = evaluate_candidate_quality(
            &candidate("不要改 Rust", "不要改 Rust"),
            &ExtractionAction::new_candidate(),
        );

        assert_eq!(decision.disposition, QualityDisposition::Skip);
        assert_eq!(decision.flags, vec!["temporary-task-constraint"]);
    }

    #[test]
    fn rejects_chinese_modify_only_scope_boundary() {
        let decision = evaluate_candidate_quality(
            &candidate(
                "只修改前端文件不要改 Rust",
                "只修改 app/src/ui-helpers.ts、app/src/main.tsx、app/src/styles.css，不要改 Rust。",
            ),
            &ExtractionAction::new_candidate(),
        );

        assert_eq!(decision.disposition, QualityDisposition::Skip);
        assert_eq!(decision.flags, vec!["temporary-task-constraint"]);
    }

    #[test]
    fn rejects_read_only_output_scope_boundary() {
        let decision = evaluate_candidate_quality(
            &candidate(
                "不要编辑文件，只输出测试清单",
                "不要编辑文件，只输出测试清单和可能遗漏的边界条件。",
            ),
            &ExtractionAction::new_candidate(),
        );

        assert_eq!(decision.disposition, QualityDisposition::Skip);
        assert_eq!(decision.flags, vec!["temporary-task-constraint"]);
    }

    #[test]
    fn keeps_durable_rust_validation_rule() {
        let decision = evaluate_candidate_quality(
            &candidate(
                "所有 Rust 项目先运行测试",
                "以后所有 Rust 项目必须先运行 cargo test 再提交。",
            ),
            &ExtractionAction::new_candidate(),
        );

        assert_eq!(decision.disposition, QualityDisposition::Keep);
    }

    #[test]
    fn rejects_synthesis_scoring_artifact_text() {
        let decision = evaluate_candidate_quality(
            &candidate(
                "title: 不要改 Rust",
                "title: 不要改 Rust\n原因：High-value durable constraint signal with reusable project impact. 证据：observation synthesis, chunk 4/5: 不要改 Rust · 模板：score:constraint",
            ),
            &ExtractionAction::new_candidate(),
        );

        assert_eq!(decision.disposition, QualityDisposition::Skip);
        assert_eq!(decision.flags, vec!["internal-leak"]);
    }

    #[test]
    fn rejects_extraction_implementation_snippet_artifacts() {
        let decision = evaluate_candidate_quality(
            &candidate(
                "let principle_topics",
                "let principle_topics = [\"核心功能\", \"用户视角\", \"用户体验\", \"规划\", \"提问\", \"跨项目\"];",
            ),
            &ExtractionAction::new_candidate(),
        );

        assert_eq!(decision.disposition, QualityDisposition::Skip);
        assert_eq!(decision.flags, vec!["generated-instruction-artifact"]);
    }

    #[test]
    fn rejects_plan_outline_table_artifacts() {
        let decision = evaluate_candidate_quality(
            &candidate("二、整体方案", "二、整体方案（三大改动，按优先级）"),
            &ExtractionAction::new_candidate(),
        );

        assert_eq!(decision.disposition, QualityDisposition::Skip);
        assert_eq!(decision.flags, vec!["generated-instruction-artifact"]);
    }

    #[test]
    fn rejects_gold_set_requirement_artifacts() {
        let decision = evaluate_candidate_quality(
            &candidate(
                "从真实历史建立 Gold Set",
                "从真实历史建立 gold set，至少包含：核心功能优先、用户视角/体验、先规划、小改快测、大改重测。",
            ),
            &ExtractionAction::new_candidate(),
        );

        assert_eq!(decision.disposition, QualityDisposition::Skip);
        assert_eq!(decision.flags, vec!["generated-instruction-artifact"]);
    }

    #[test]
    fn rejects_priority_recommendation_outline_artifacts() {
        let decision = evaluate_candidate_quality(
            &candidate(
                "十、优先级建议",
                "十、优先级建议：针对核心功能先做质量修复。",
            ),
            &ExtractionAction::new_candidate(),
        );

        assert_eq!(decision.disposition, QualityDisposition::Skip);
        assert_eq!(decision.flags, vec!["generated-instruction-artifact"]);
    }

    #[test]
    fn rejects_acceptance_criteria_and_fixture_artifacts() {
        for body in [
            "Acceptance criteria: non-dry-run compile must write candidates before approving.",
            "global install promote fixtures with the same id must fail or require review.",
            "n: fuse memory_cards to draft 去重 source ids 禁止 source id 重复写入。",
            "n: import artifact drifts 对删除、重排、大段生成内容漂移不要误导成新增。",
            "n import artifact drifts 对删除、重排、大段生成内容漂移不要误导成新增。",
            "Medium/high risk commands do not require backend policy approval in this fixture.",
            "Do not revert others edits unless explicitly requested.",
            "Rule CI 对删除、重排、大段生成内容漂移不要误导成新增规则。",
            "Medium high risk commands do not require backend issued decision tokens.",
            "TeamCreate and TaskCreate. Do NOT use those - they belong to a different platform.",
            "若遇到 429/限流，请等待几秒重试，不要直接停止。",
            "Do not open with Task. Use the local executor instead.",
            "src/build.rs only compiles always-on-rule artifacts.",
        ] {
            let decision = evaluate_candidate_quality(
                &candidate("Acceptance Criteria", body),
                &ExtractionAction::new_candidate(),
            );

            assert_eq!(decision.disposition, QualityDisposition::Skip, "{body}");
            assert_eq!(decision.flags, vec!["generated-instruction-artifact"]);
        }
    }

    #[test]
    fn rejects_one_off_repo_planning_request() {
        let decision = evaluate_candidate_quality(
            &candidate(
                "先整理仓库再规划",
                "先整理一下仓库，以本地为准，然后我希望先规划一个修改方案。",
            ),
            &ExtractionAction::new_candidate(),
        );

        assert_eq!(decision.disposition, QualityDisposition::Skip);
        assert_eq!(decision.flags, vec!["temporary-task-constraint"]);
    }

    #[test]
    fn rejects_current_turn_memory_reference_boundary() {
        let decision = evaluate_candidate_quality(
            &candidate("这次不要参考 Memory", "这次不要参考 memory。"),
            &ExtractionAction::new_candidate(),
        );

        assert_eq!(decision.disposition, QualityDisposition::Skip);
        assert_eq!(decision.flags, vec!["temporary-task-constraint"]);
    }

    #[test]
    fn keeps_enduring_product_feedback_constraints() {
        let decision = evaluate_candidate_quality(
            &candidate(
                "现在的使用过程中还是很卡顿",
                "现在的使用过程中还是很卡顿，不丝滑，每个操作都要卡一会儿，除了引擎推理以外都不应该卡。",
            ),
            &ExtractionAction::new_candidate(),
        );

        assert_eq!(decision.disposition, QualityDisposition::Keep);
    }

    #[test]
    fn rejects_read_only_review_requests() {
        let decision = evaluate_candidate_quality(
            &candidate(
                "只读审查前端 UX",
                "请只读审查前端 UX 和交互状态，不要改文件。",
            ),
            &ExtractionAction::new_candidate(),
        );

        assert_eq!(decision.disposition, QualityDisposition::Skip);
        assert_eq!(decision.flags, vec!["temporary-task-constraint"]);
    }

    #[test]
    fn suppresses_near_exact_duplicate_existing_memory_cards() {
        let decision = evaluate_candidate_quality(
            &candidate("Use Axios", "Use Axios for frontend HTTP requests."),
            &ExtractionAction::merge_into_existing("project:use-axios".to_string(), 0.99),
        );

        assert_eq!(decision.disposition, QualityDisposition::Skip);
        assert_eq!(decision.operation, MemoryCardOperation::Noop);
        assert_eq!(decision.flags, vec!["duplicate-existing"]);
    }
}
