use crate::candidate::{self, EvidenceSpan};
use crate::textutil;

use super::{Candidate, chunk, classify, scoring, signals};

pub(super) fn failure_flow_candidates(input: &str) -> Vec<Candidate> {
    if !input.contains("FailureFlowSummary:") {
        return Vec::new();
    }
    let lower = input.to_lowercase();
    let mut candidates = Vec::new();

    if lower.contains("signal:pipeline_break") {
        candidates.push(failure_flow_candidate(
            "提取异常先定位链路断点",
            "当记忆提取结果异常少、质量差或没有进入最终卡片阶段时，先定位链路断点，包括候选召回、LLM induction、JSON 解析、质量门控和 crystallize，再决定是否调整 prompt。",
            input,
            "pipeline-break",
            0.87,
        ));
    }

    if lower.contains("signal:final_quality_correction") {
        candidates.push(failure_flow_candidate(
            "优化生成质量要检查最终卡片",
            "优化生成链路时，除了看测试和指标，还要抽样阅读最终候选或 Memory Card，确认内容清楚、可触发、有边界并覆盖用户意图。",
            input,
            "final-quality-review",
            0.86,
        ));
    }

    if lower.contains("signal:false_positive_noise") {
        candidates.push(failure_flow_candidate(
            "实现细节不能直接固化为记忆",
            "提炼记忆卡片时，不要把代码分析、实现细节、抽取 taxonomy 或单次执行说明直接固化为长期记忆；只有抽象成未来可触发的用户偏好或工作流时才保留。",
            input,
            "false-positive-abstraction",
            0.84,
        ));
    }

    if lower.contains("signal:privacy_boundary") {
        candidates.push(failure_flow_candidate(
            "真实历史失败样本转合成回归",
            "真实历史 dry-run 暴露漏召、误召或低质量卡片时，不提交真实对话数据，而是把失败模式转成合成或匿名回归测试。",
            input,
            "local-only-regression",
            0.88,
        ));
    }

    if lower.contains("signal:scope_boundary") {
        candidates.push(failure_flow_candidate(
            "区分项目记忆和全局记忆",
            "固化记忆卡片时，区分只适用于当前项目的流程规则和可跨项目复用的协作偏好，避免把局部实现流程污染到全局记忆。",
            input,
            "scope-boundary",
            0.84,
        ));
    }

    if lower.contains("signal:refactor_correction") {
        candidates.push(failure_flow_candidate(
            "从重构和纠偏中沉淀流程模块",
            "完成重构、架构调整或多轮纠偏后，复盘哪些做法可迁移为未来工作流模块；优先固化触发条件、操作步骤、验证方式和不适用边界。",
            input,
            "refactor-lessons",
            0.86,
        ));
    }

    if lower.contains("signal:transferable_workflow") {
        candidates.push(failure_flow_candidate(
            "把项目偏好抽象成可迁移工作流",
            "审阅项目特殊偏好时，判断它背后的通用协作原则或工程流程；能迁移到其他项目的，改写成全局偏好或工作流模块，而不是保留项目实现细节。",
            input,
            "transferable-workflow",
            0.87,
        ));
    }

    if lower.contains("signal:review_iterate_loop") {
        candidates.push(failure_flow_candidate(
            "真实输出审核后继续迭代",
            "优化提取或生成质量时，必须在真实数据 dry-run 后阅读生成的候选卡片，对照目标审核，分析问题和解决方案，再修改测试与实现直到结果符合预期。",
            input,
            "review-iterate-loop",
            0.88,
        ));
    }

    candidates
}

fn failure_flow_candidate(
    title: &str,
    body: &str,
    evidence: &str,
    template: &str,
    confidence: f32,
) -> Candidate {
    Candidate {
        title: title.to_string(),
        body: body.to_string(),
        kind: "procedure".to_string(),
        scope: "global".to_string(),
        memory_tier: crate::candidate::MemoryTier::CollaborationPreference,
        abstraction_of: None,
        abstracted_from: None,
        evidence: evidence.to_string(),
        confidence: Some(confidence),
        reason: Some("Inferred from user corrections and extraction failure signals.".to_string()),
        matched_template: Some(format!("failure-flow:{template}")),
    }
}

pub(super) fn global_flow_candidates(input: &str) -> Vec<Candidate> {
    if !input.contains("ConversationFlowSummary:") {
        return Vec::new();
    }
    let lower = input.to_lowercase();
    let mut candidates = Vec::new();

    if contains_any(&lower, &["stage:startup", "accepted_offer"])
        && contains_any(&lower, &["开源项目", "同类产品", "参考", "借鉴"])
    {
        candidates.push(global_flow_candidate(
            "项目规划前参考外部样例",
            "项目启动或规划新功能时，先检查已有上下文，并参考可借鉴的开源项目、同类产品或相关资料，再制定方案。",
            input,
            "reference-research",
            0.88,
        ));
    }

    if contains_any(&lower, &["stage:startup", "accepted_offer"])
        && contains_any(&lower, &["可视化", "mockup", "对比图", "流程图", "架构图"])
    {
        candidates.push(global_flow_candidate(
            "用可视化辅助方案讨论",
            "项目启动或方案讨论涉及界面、布局、架构或流程时，优先用轻量 mockup、对比图或流程图辅助讨论；简单文本决策不必强行可视化。",
            input,
            "visual-planning",
            0.88,
        ));
    }

    if contains_any(&lower, &["真实历史", "dry-run", "dry run"])
        && contains_any(
            &lower,
            &["提炼", "抽取", "生成", "最终质量", "用户视角", "质量"],
        )
    {
        candidates.push(global_flow_candidate(
            "用真实历史验证提炼质量",
            "修改提炼质量相关代码时，先跑合成测试和本地真实历史 dry-run，再阅读最终候选或卡片文本；真实历史只用于本地评估，不进入 Git 或 Golden Set。",
            input,
            "real-history-validation",
            0.91,
        ));
    }

    if contains_any(
        &lower,
        &[
            "人工看",
            "自己看",
            "最终质量",
            "用户视角",
            "需求覆盖",
            "验收",
        ],
    ) && contains_any(
        &lower,
        &["测试", "指标", "分数", "green tests", "tests pass"],
    ) {
        candidates.push(global_flow_candidate(
            "验收时人工检查真实质量",
            "交付或验收时，把测试和指标作为证据之一，还要从用户视角检查最终结果是否覆盖需求、文本是否清楚、体验是否可信。",
            input,
            "delivery-acceptance",
            0.89,
        ));
    }

    if contains_any(&lower, &["github", "issue", "pr"])
        && contains_any(
            &lower,
            &["另一个 agent", "另一个agent", "不要重复", "已有规划"],
        )
    {
        candidates.push(global_flow_candidate(
            "并行 GitHub 工作先查重",
            "多人或多 agent 并行使用 GitHub 工作时，先检查已有 Issue、PR 和规划，只补充缺口，避免重复规划或重复修改。",
            input,
            "github-deduplication",
            0.87,
        ));
    }

    candidates
}

fn global_flow_candidate(
    title: &str,
    body: &str,
    evidence: &str,
    template: &str,
    confidence: f32,
) -> Candidate {
    Candidate {
        title: title.to_string(),
        body: body.to_string(),
        kind: "procedure".to_string(),
        scope: "global".to_string(),
        memory_tier: crate::candidate::MemoryTier::CollaborationPreference,
        abstraction_of: None,
        abstracted_from: None,
        evidence: evidence.to_string(),
        confidence: Some(confidence),
        reason: Some("Inferred from cross-stage conversation flow with evidence.".to_string()),
        matched_template: Some(format!("global-flow:{template}")),
    }
}

pub(super) fn high_value_prompt_candidate(sentence: &str) -> Option<Candidate> {
    if !looks_like_high_value_prompt_signal(sentence) {
        return None;
    }

    let body = normalize_high_value_prompt_body(sentence);
    if body.len() < 28 || body.len() > 360 {
        return None;
    }

    Some(Candidate {
        title: title_from_high_value_prompt(&body),
        body,
        kind: "procedure".to_string(),
        scope: infer_scope(sentence).to_string(),
        memory_tier: crate::candidate::MemoryTier::ProjectRule,
        abstraction_of: None,
        abstracted_from: None,
        evidence: sentence.to_string(),
        confidence: Some(0.84),
        reason: Some(
            "Matched high-value prompt pattern: reusable agent handoff or project-improving workflow."
                .to_string(),
        ),
        matched_template: Some("high-value-prompt".to_string()),
    })
}

pub(super) fn principle_candidates(sentence: &str) -> Vec<Candidate> {
    let lower = sentence.to_lowercase();
    let has_principle = signals::has_principle_signal(&lower);
    let has_planning = signals::has_planning_heuristic_signal(&lower);
    let has_collaboration = signals::has_collaboration_preference_signal(&lower);
    if !has_principle && !has_planning && !has_collaboration {
        return Vec::new();
    }

    let body = normalize_methodology_body(sentence);
    if body.len() < 16 || body.len() > 360 {
        return Vec::new();
    }

    let mut candidates = Vec::new();
    if has_principle {
        candidates.push(Candidate {
            title: title_from_body(&body),
            body: body.clone(),
            kind: "procedure".to_string(),
            scope: "global".to_string(),
            memory_tier: crate::candidate::MemoryTier::CrossProjectPrinciple,
            abstraction_of: None,
            abstracted_from: None,
            evidence: sentence.to_string(),
            confidence: Some(0.8),
            reason: Some("Matched reusable cross-project principle signal.".to_string()),
            matched_template: Some("principle-signal".to_string()),
        });
    }
    if should_emit_collaboration_candidate(&lower, has_principle, has_planning, has_collaboration) {
        candidates.push(Candidate {
            title: title_from_body(&body),
            body,
            kind: "procedure".to_string(),
            scope: "global".to_string(),
            memory_tier: crate::candidate::MemoryTier::CollaborationPreference,
            abstraction_of: None,
            abstracted_from: None,
            evidence: sentence.to_string(),
            confidence: Some(if has_principle { 0.79 } else { 0.78 }),
            reason: Some(
                "Matched durable planning or collaboration preference signal.".to_string(),
            ),
            matched_template: Some("principle-signal".to_string()),
        });
    }
    candidates
}

pub(super) fn self_verification_candidate(sentence: &str) -> Option<Candidate> {
    let lower = sentence.to_lowercase();
    let markers = [
        "真实结果",
        "推理引擎",
        "检查有没有问题",
        "质量高不高",
        "自检",
        "dry-run",
        "dry run",
    ];
    if markers
        .iter()
        .filter(|marker| lower.contains(**marker))
        .count()
        < 2
    {
        return None;
    }
    let body = "当准备提交最终代码、分析结论或复杂任务结果时，先用真实输入或 dry-run 自检输出质量；发现问题后再修正。"
        .to_string();
    if body.len() < 12 || body.len() > 260 {
        return None;
    }
    Some(Candidate {
        title: title_from_body(&body),
        body,
        kind: "procedure".to_string(),
        scope: "global".to_string(),
        memory_tier: crate::candidate::MemoryTier::CollaborationPreference,
        abstraction_of: None,
        abstracted_from: None,
        evidence: sentence.to_string(),
        confidence: Some(0.98),
        reason: Some("Matched durable self-verification preference signal.".to_string()),
        matched_template: Some("self-verification-signal".to_string()),
    })
}

pub(super) fn project_startup_collaboration_candidate(sentence: &str) -> Option<Candidate> {
    let lower = sentence.to_lowercase();
    let has_visual_planning =
        contains_any(
            &lower,
            &["可视化", "mockup", "对比图", "流程图", "架构图", "浏览器"],
        ) && contains_any(&lower, &["讨论", "边聊边做", "规划", "布局", "方案"]);
    let has_reference_research =
        contains_any(
            &lower,
            &["借鉴", "参考", "开源项目", "同类产品", "相关内容"],
        ) && contains_any(&lower, &["规划", "方案", "启动", "实现新功能", "新增功能"]);
    if !has_visual_planning && !has_reference_research {
        return None;
    }

    let body = if has_visual_planning && has_reference_research {
        "项目启动或方案讨论涉及界面、布局、架构或流程时，优先用轻量 mockup、对比图、流程图等可视化辅助讨论，并可参考开源项目或同类产品后再规划。"
    } else if has_visual_planning {
        "项目启动或方案讨论涉及界面、布局、架构或流程时，优先用轻量 mockup、对比图或流程图等可视化辅助讨论。"
    } else {
        "项目启动或规划新功能时，先参考可借鉴的开源项目、同类产品或相关资料，再制定方案。"
    }
    .to_string();

    Some(Candidate {
        title: title_from_body(&body),
        body,
        kind: "procedure".to_string(),
        scope: "global".to_string(),
        memory_tier: crate::candidate::MemoryTier::CollaborationPreference,
        abstraction_of: None,
        abstracted_from: None,
        evidence: sentence.to_string(),
        confidence: Some(0.86),
        reason: Some(
            "Matched durable project-startup planning and collaboration preference signal."
                .to_string(),
        ),
        matched_template: Some("project-startup-collaboration".to_string()),
    })
}

pub(super) fn speed_validation_cadence_candidate(sentence: &str) -> Option<Candidate> {
    let lower = sentence.to_lowercase();
    let has_speed = contains_any(
        &lower,
        &[
            "迅速开发",
            "快速开发",
            "推进速度",
            "不要过多检查",
            "不要过多测试",
            "不过度测试",
            "小改",
        ],
    );
    let has_validation = contains_any(
        &lower,
        &[
            "必要时测试",
            "必要检查",
            "测试/检查",
            "完整测试",
            "总的测试",
            "完成前",
        ],
    ) || (contains_any(&lower, &["检查", "验证", "测试"])
        && contains_any(&lower, &["最后", "完成前", "总的"]));
    if !has_speed || !has_validation {
        return None;
    }

    let body = "快速推进开发时，小改只做必要检查；大改、收尾或交付前再跑完整测试，兼顾速度和质量。"
        .to_string();

    Some(Candidate {
        title: title_from_body(&body),
        body,
        kind: "procedure".to_string(),
        scope: "global".to_string(),
        memory_tier: crate::candidate::MemoryTier::CollaborationPreference,
        abstraction_of: None,
        abstracted_from: None,
        evidence: sentence.to_string(),
        confidence: Some(0.84),
        reason: Some(
            "Matched durable development cadence preference balancing speed and verification."
                .to_string(),
        ),
        matched_template: Some("speed-validation-cadence".to_string()),
    })
}

pub(super) fn existing_flow_planning_candidate(sentence: &str) -> Option<Candidate> {
    let lower = sentence.to_lowercase();
    let touches_existing_system = contains_any(
        &lower,
        &[
            "现有项目",
            "既有流程",
            "当前项目",
            "当前流程",
            "existing project",
            "existing flow",
        ],
    );
    let touches_change_surface = contains_any(
        &lower,
        &[
            "提炼",
            "memory card",
            "ui 流程",
            "ui流程",
            "架构",
            "规则",
            "pipeline",
        ],
    );
    let has_planning_order =
        contains_any(&lower, &["先理解", "再提出", "再制定", "before changing"]);
    if !touches_existing_system || !touches_change_surface || !has_planning_order {
        return None;
    }

    let body = "改动提炼、Memory Card、UI 流程或架构前，先理解现有项目和既有流程，再提出方案。"
        .to_string();

    Some(Candidate {
        title: title_from_body(&body),
        body,
        kind: "procedure".to_string(),
        scope: "global".to_string(),
        memory_tier: crate::candidate::MemoryTier::CollaborationPreference,
        abstraction_of: None,
        abstracted_from: None,
        evidence: sentence.to_string(),
        confidence: Some(0.87),
        reason: Some(
            "Matched durable preference to understand the existing project flow before changing it."
                .to_string(),
        ),
        matched_template: Some("existing-flow-planning".to_string()),
    })
}

pub(super) fn delivery_acceptance_candidate(sentence: &str) -> Option<Candidate> {
    let lower = sentence.to_lowercase();
    let has_test_green = contains_any(
        &lower,
        &[
            "绿色测试",
            "测试绿",
            "测试通过",
            "green tests",
            "tests pass",
        ],
    );
    let has_human_acceptance = contains_any(
        &lower,
        &[
            "用户实际感知",
            "用户视角",
            "需求覆盖",
            "实际感知",
            "体验质量",
            "user experience",
            "requirements coverage",
        ],
    );
    let has_acceptance_context = contains_any(
        &lower,
        &["交付", "验收", "最终", "完成", "acceptance", "delivery"],
    );
    if !has_test_green || !has_human_acceptance || !has_acceptance_context {
        return None;
    }

    let body =
        "交付验收时，绿色测试只是证据之一，还要从用户实际感知和需求覆盖判断质量。".to_string();

    Some(Candidate {
        title: title_from_body(&body),
        body,
        kind: "procedure".to_string(),
        scope: "global".to_string(),
        memory_tier: crate::candidate::MemoryTier::CollaborationPreference,
        abstraction_of: None,
        abstracted_from: None,
        evidence: sentence.to_string(),
        confidence: Some(0.89),
        reason: Some(
            "Matched durable delivery acceptance preference beyond green tests.".to_string(),
        ),
        matched_template: Some("delivery-acceptance".to_string()),
    })
}

pub(super) fn planning_deduplication_candidate(sentence: &str) -> Option<Candidate> {
    let lower = sentence.to_lowercase();
    let mentions_planning = contains_any(&lower, &["规划", "计划", "github", "issue"]);
    let mentions_dedup = contains_any(&lower, &["不要重复规划", "避免重复规划", "已经规划"]);
    if !mentions_planning || !mentions_dedup {
        return None;
    }

    let body =
        "使用 GitHub 或计划文档规划时，先检查已有规划，只补充缺口，避免重复规划。".to_string();

    Some(Candidate {
        title: title_from_body(&body),
        body,
        kind: "procedure".to_string(),
        scope: "global".to_string(),
        memory_tier: crate::candidate::MemoryTier::CollaborationPreference,
        abstraction_of: None,
        abstracted_from: None,
        evidence: sentence.to_string(),
        confidence: Some(0.85),
        reason: Some("Matched durable preference to avoid duplicating existing plans.".to_string()),
        matched_template: Some("planning-deduplication".to_string()),
    })
}

pub(super) fn parallel_agent_github_coordination_candidate(sentence: &str) -> Option<Candidate> {
    let lower = sentence.to_lowercase();
    let mentions_other_agent = contains_any(
        &lower,
        &["另一个agent", "另一个 agent", "其他agent", "other agent"],
    );
    let mentions_github = contains_any(&lower, &["github", "issue", "pr"]);
    let mentions_overlap = contains_any(
        &lower,
        &["不要重复", "注意不要重复", "避免重复", "已经规划", "已经在"],
    );
    if !mentions_other_agent || !mentions_github || !mentions_overlap {
        return None;
    }

    let body =
        "多人或多 agent 并行使用 GitHub 工作时，先检查已有 Issue、PR 和规划，避免重复规划或重复修改。"
            .to_string();

    Some(Candidate {
        title: title_from_body(&body),
        body,
        kind: "procedure".to_string(),
        scope: "global".to_string(),
        memory_tier: crate::candidate::MemoryTier::CollaborationPreference,
        abstraction_of: None,
        abstracted_from: None,
        evidence: sentence.to_string(),
        confidence: Some(0.86),
        reason: Some(
            "Matched durable coordination preference for parallel GitHub agent work.".to_string(),
        ),
        matched_template: Some("parallel-agent-github-coordination".to_string()),
    })
}

pub(super) fn local_only_golden_set_candidate(sentence: &str) -> Option<Candidate> {
    let lower = sentence.to_lowercase();
    let mentions_golden = contains_any(&lower, &["golden set", "目标测试集", "测试集"]);
    let mentions_local = contains_any(&lower, &["本地测试", "本地效果评估", "local"]);
    let mentions_privacy = contains_any(
        &lower,
        &[
            "不要把我自己的对话数据上传git",
            "不要上传",
            "真实数据",
            "对话数据",
            "git",
        ],
    );
    if !mentions_golden || !mentions_local || !mentions_privacy {
        return None;
    }

    let body =
        "Golden Set 只用于本地效果评估；不要把真实对话数据提交或上传到 Git，链路测试使用合成或匿名样本。"
            .to_string();

    Some(Candidate {
        title: title_from_body(&body),
        body,
        kind: "constraint".to_string(),
        scope: "global".to_string(),
        memory_tier: crate::candidate::MemoryTier::CollaborationPreference,
        abstraction_of: None,
        abstracted_from: None,
        evidence: sentence.to_string(),
        confidence: Some(0.94),
        reason: Some(
            "Matched durable privacy boundary for local Golden Set evaluation.".to_string(),
        ),
        matched_template: Some("local-only-golden-set".to_string()),
    })
}

fn contains_any(text: &str, markers: &[&str]) -> bool {
    markers.iter().any(|marker| text.contains(marker))
}

fn should_emit_collaboration_candidate(
    lower: &str,
    has_principle: bool,
    has_planning: bool,
    has_collaboration: bool,
) -> bool {
    if has_planning {
        return true;
    }
    if !has_collaboration {
        return false;
    }
    if !has_principle {
        return true;
    }
    [
        "审阅边界",
        "先 review",
        "先审阅",
        "不要让 ai",
        "小改快测",
        "大改重测",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

pub(super) fn scored_signal_candidate(
    sentence: &str,
    origin: chunk::ChunkOrigin,
) -> Option<Candidate> {
    let chunk = chunk::EvidenceChunk {
        id: textutil::slug(sentence),
        text: sentence.trim().to_string(),
        origin,
        source_kind: "local-text".to_string(),
        source_observations: Vec::new(),
    };
    let score = scoring::score_chunk(&chunk);
    if score.disposition != scoring::ExtractionDisposition::Candidate {
        return None;
    }

    let body = if sentence.contains("卡顿") || sentence.contains("不丝滑") {
        normalize_project_improvement_body(sentence)
    } else {
        normalize_body(sentence)
    };
    if body.len() < 12 || body.len() > 400 {
        return None;
    }

    Some(Candidate {
        title: title_from_scored_signal(&body, &score.matched_signal),
        body,
        kind: kind_from_scored_signal(&score.matched_signal).to_string(),
        scope: infer_scope(sentence).to_string(),
        memory_tier: crate::candidate::MemoryTier::ProjectRule,
        abstraction_of: None,
        abstracted_from: None,
        evidence: sentence.to_string(),
        confidence: Some((score.score / 5.0).clamp(0.7, 0.94)),
        reason: Some(score.reason),
        matched_template: Some(format!("score:{}", score.matched_signal)),
    })
}

pub(super) fn atomic_exception_candidate(sentence: &str) -> Option<Candidate> {
    let lower = sentence.to_lowercase();
    let has_exception_marker = lower.contains("保留")
        || lower.contains("例外")
        || lower.contains("except")
        || lower.contains("unless")
        || lower.contains("keep ");
    let has_reason = lower.contains("因为") || lower.contains("because") || lower.contains("需要");
    let has_specific_exception_subject =
        lower.contains("fetch") || lower.contains("readablestream") || lower.contains("stream");
    if !has_exception_marker || (!has_reason && !has_specific_exception_subject) {
        return None;
    }

    let body = normalize_body(sentence);
    if body.len() < 12 || body.len() > 260 {
        return None;
    }

    Some(Candidate {
        title: title_from_body(&body),
        body,
        kind: "constraint".to_string(),
        scope: infer_scope(sentence).to_string(),
        memory_tier: crate::candidate::MemoryTier::ProjectRule,
        abstraction_of: None,
        abstracted_from: None,
        evidence: sentence.to_string(),
        confidence: Some(0.82),
        reason: Some("Split atomic exception from a broader preference rule.".to_string()),
        matched_template: Some("atomic-exception".to_string()),
    })
}

fn kind_from_scored_signal(signal: &str) -> &'static str {
    match signal {
        "constraint" => "constraint",
        "procedure" | "correction" | "decision" | "ai_project_improvement" => "procedure",
        _ => "preference",
    }
}

fn title_from_scored_signal(body: &str, signal: &str) -> String {
    if signal == "ai_project_improvement" {
        "Review AI-Origin Project Improvement".to_string()
    } else if signal == "constraint" && body.contains("AGENTS.md") {
        "Protect AGENTS.md Drift Review".to_string()
    } else {
        title_from_body(body)
    }
}

pub(super) fn draft_id(candidate: &Candidate) -> String {
    let slug = textutil::slug(&candidate.title);
    let prefix = match candidate.scope.as_str() {
        "global" => "global",
        "agent" => "agent",
        _ => "project",
    };
    format!("{prefix}:{slug}")
}

/// 从证据块和评分结果构建提取元数据，用于记录溯源信息。
pub(super) fn extraction_metadata_for_chunk(
    chunk: &chunk::EvidenceChunk,
    score: &scoring::ExtractionScore,
    similar_record: Option<String>,
) -> candidate::ExtractionMetadata {
    let classification = classify::classify_chunk(chunk);
    let route = classification.artifact_kind.clone();
    candidate::ExtractionMetadata {
        origin: chunk.origin.as_str().to_string(),
        matched_signal: score.matched_signal.clone(),
        reason: score.reason.clone(),
        source_observations: chunk.source_observations.clone(),
        score_breakdown: score.breakdown.clone(),
        similar_record,
        tags: classification.tags.clone(),
        classification: Some(classification),
        suggested_action: Some(candidate::ExtractionAction::new_candidate_for_route(&route)),
        evidence_span: Some(EvidenceSpan {
            role: chunk.origin.as_str().to_string(),
            quote: chunk.text.clone(),
            observation_id: chunk.source_observations.first().cloned(),
            turn_id: None,
            surrounding_context: Vec::new(),
        }),
        memory_tier: crate::candidate::MemoryTier::ProjectRule,
        value_scores: Default::default(),
        abstraction_of: None,
        abstracted_from: None,
        pipeline_version: None,
        layer_trace: Vec::new(),
        rejected_at: None,
        evidence_bundle: None,
        card_function: None,
        value_claim: None,
        value_delta: None,
        target_context: None,
        synthesis_trace: Vec::new(),
    }
}

// ============================================================
// 旧版正则提取函数（从 signals.rs 移入，供 local engine 使用）
// ============================================================

pub(super) fn looks_like_rule(sentence: &str) -> bool {
    let lower = sentence.to_lowercase();
    let markers = [
        "以后",
        "必须",
        "不要",
        "禁止",
        "统一",
        "默认",
        "保留",
        "优先",
        "改用",
        "不再",
        "always",
        "never",
        "must",
        "prefer",
        "use ",
        "don't",
        "do not",
        "default to",
        "keep ",
    ];
    markers.iter().any(|marker| lower.contains(marker))
}

pub(super) fn looks_like_memory_card_signal(sentence: &str) -> bool {
    let lower = sentence.to_lowercase();
    if signals::is_low_value_task_sentence(&lower) {
        return false;
    }
    if signals::looks_like_unresolved_user_request(&lower)
        && !signals::has_strong_memory_marker(&lower)
    {
        return false;
    }

    let durable_markers = [
        "以后",
        "所有项目",
        "每次",
        "总是",
        "默认",
        "统一",
        "优先",
        "禁止",
        "不要",
        "必须",
        "记住",
        "always",
        "never",
        "prefer",
        "by default",
        "default to",
        "for every",
        "for all",
        "must",
        "do not",
    ];
    let domain_markers = [
        "agent",
        "skill",
        "memory_card",
        "claude",
        "codex",
        "cursor",
        "bun",
        "pnpm",
        "ky",
        "ofetch",
        "axios",
        "fetch",
        "cargo",
        "rust",
        "typescript",
        "react",
        "tauri",
        "test",
        "ui",
        "api",
        "http",
        "git",
        "mcp",
        "智能体",
        "技能",
        "规则",
        "项目",
        "测试",
        "前端",
        "后端",
        "界面",
        "代理",
    ];

    durable_markers.iter().any(|marker| lower.contains(marker))
        && domain_markers.iter().any(|marker| lower.contains(marker))
}

fn looks_like_high_value_prompt_signal(sentence: &str) -> bool {
    let lower = sentence.to_lowercase();
    if signals::is_low_value_task_sentence(&lower) {
        return false;
    }
    if signals::looks_like_unresolved_user_request(&lower)
        && !signals::has_explicit_memory_marker(&lower)
    {
        return false;
    }

    let prompt_markers = [
        "提示",
        "prompt",
        "可以先",
        "先让",
        "再由",
        "让 claude",
        "由 codex",
        "交给 claude",
        "codex 做",
        "输出",
        "清单",
        "步骤",
        "检查",
        "复用",
        "模式",
        "减少返工",
        "显著减少",
        "提高质量",
        "降低理解成本",
        "减少误解",
        "handoff",
        "checklist",
        "playbook",
        "workflow",
    ];
    let domain_markers = [
        "claude",
        "codex",
        "agent",
        "memory_card",
        "skill",
        "ui",
        "组件",
        "状态流",
        "按钮",
        "rust",
        "tauri",
        "react",
        "测试",
        "集成测试",
        "架构",
        "前端",
        "后端",
        "重构",
        "agent",
        "frontend",
        "backend",
        "architecture",
        "test",
        "refactor",
    ];
    let outcome_markers = [
        "减少返工",
        "显著减少",
        "提高质量",
        "降低理解成本",
        "减少误解",
        "更稳定",
        "更清晰",
        "可复用",
        "避免遗漏",
        "avoid rework",
        "reduce rework",
        "improve",
    ];

    prompt_markers
        .iter()
        .filter(|marker| lower.contains(**marker))
        .count()
        >= 2
        && domain_markers.iter().any(|marker| lower.contains(marker))
        && outcome_markers.iter().any(|marker| lower.contains(marker))
}

pub(super) fn normalize_body(sentence: &str) -> String {
    let mut body = sentence.trim().to_string();
    let replacements = [
        ("我再说最后一次，", ""),
        ("我再说最后一次", ""),
        ("以后", ""),
        ("请", ""),
        ("记住：", ""),
        ("记住:", ""),
    ];
    for (from, to) in replacements {
        body = body.replace(from, to);
    }
    body = body.trim_matches(['，', ',', ' ']).trim().to_string();
    if body.ends_with(';') {
        body.pop();
    }
    body
}

pub(super) fn normalize_project_improvement_body(sentence: &str) -> String {
    let lower = sentence.to_lowercase();
    if lower.contains("卡顿") || lower.contains("不丝滑") || lower.contains("not smooth") {
        return "保持使用过程流畅，避免每个操作都触发明显卡顿。".to_string();
    }

    let mut body = normalize_body(sentence);
    let replacements = [
        ("这次", ""),
        ("最终做法是", "Reusable approach:"),
        ("根因是", "Root cause:"),
        ("原因是", "Root cause:"),
    ];
    for (from, to) in replacements {
        body = body.replace(from, to);
    }
    body.trim_matches(['，', ',', ' ']).trim().to_string()
}

pub(super) fn normalize_high_value_prompt_body(sentence: &str) -> String {
    let body = normalize_body(sentence);
    let replacements = [
        ("这个提示能", "This prompt can "),
        ("这个提示可以", "This prompt can "),
        ("可以先", "Start by "),
        ("再由", "then hand off to "),
    ];
    let mut normalized = body;
    for (from, to) in replacements {
        normalized = normalized.replace(from, to);
    }
    normalized.trim_matches(['，', ',', ' ']).trim().to_string()
}

fn normalize_methodology_body(sentence: &str) -> String {
    let mut body = normalize_body(sentence);
    for prefix in ["global:", "project:", "agent:"] {
        if body.to_lowercase().starts_with(prefix) {
            body = body[prefix.len()..].trim().to_string();
        }
    }
    let lower = body.to_lowercase();

    if (lower.contains("不要固定") || lower.contains("固定规则词") || lower.contains("规则词"))
        && (lower.contains("always") || lower.contains("prefer") || lower.contains("必须"))
        && (lower.contains("高价值") || lower.contains("prompt") || lower.contains("memory_card"))
    {
        return "提炼高价值 MemoryCard 时不要只依赖固定规则词，要识别真实高价值表达。".to_string();
    }
    if lower.contains("你对用户视角") && lower.contains("体验") && lower.contains("核心功能")
    {
        return "开发评估优先关注核心功能、用户视角与体验质量。".to_string();
    }
    if lower.contains("希望结果能支持持续自我修正") || lower.contains("支持持续自我修正")
    {
        return "提炼结果应支持基于真实反馈持续自我修正。".to_string();
    }
    if lower.contains("更关心候选质量") || lower.contains("候选质量优先于数量") {
        return "候选质量优先于候选数量。".to_string();
    }
    if lower.contains("真实历史回归比样例更重要")
        || (lower.contains("重视真实历史回归") && lower.contains("不迷信"))
    {
        return "重视真实历史回归，不迷信静态样例。".to_string();
    }
    if lower.contains("审阅边界")
        && (lower.contains("先 review") || lower.contains("先审阅") || lower.contains("review"))
    {
        return "先 review 再 merge，保留人工审阅边界，不要让 AI 直接固化规则。".to_string();
    }
    if lower.contains("设计阶段")
        && lower.contains("先提问")
        && lower.contains("先澄清目标")
        && lower.contains("先规划")
    {
        return "设计阶段先提问、先澄清目标、先规划，并从用户视角检查方案。".to_string();
    }
    if (lower.contains("小改快测")
        || lower.contains("小修改快测")
        || lower.contains("小修改做快测"))
        && (lower.contains("大改重测")
            || lower.contains("大改详测")
            || lower.contains("大修改做完整回归")
            || lower.contains("大改再做完整回归"))
    {
        return "小改快测，大改重测。".to_string();
    }
    if lower.contains("核心功能优先") && lower.contains("体验优先") {
        return "开发阶段优先做稳核心功能和用户体验，避免过度堆周边功能。".to_string();
    }

    body
}

pub(super) fn title_from_body(body: &str) -> String {
    let words = body.split_whitespace().collect::<Vec<_>>();
    if words.len() >= 3 {
        return words.iter().take(6).copied().collect::<Vec<_>>().join(" ");
    }
    trim_title_boundary_punctuation(&body.chars().take(24).collect::<String>())
}

fn trim_title_boundary_punctuation(title: &str) -> String {
    title
        .trim()
        .trim_end_matches(['，', '、', ',', '；', ';', '。', ':', '：'])
        .trim()
        .to_string()
}

pub(super) fn title_from_project_improvement(body: &str) -> String {
    let lower = body.to_lowercase();
    if lower.contains("卡死") || lower.contains("卡顿") || lower.contains("freeze") {
        "Keep UI Responsive During Long Tasks".to_string()
    } else if lower.contains("增量") || lower.contains("incremental") {
        "Use Incremental Local Processing".to_string()
    } else if lower.contains("跨平台") || lower.contains("windows") || lower.contains("mac") {
        "Handle Cross-Platform Runtime Differences".to_string()
    } else if lower.contains("测试") || lower.contains("test") {
        "Preserve Regression Tests For Fixes".to_string()
    } else {
        title_from_body(body)
    }
}

pub(super) fn title_from_high_value_prompt(body: &str) -> String {
    let lower = body.to_lowercase();
    if lower.contains("claude") && lower.contains("codex") && lower.contains("ui") {
        "Use Agent Handoff Prompts For UI Refactors".to_string()
    } else if lower.contains("checklist") || lower.contains("清单") {
        "Use Checklist Prompts For Complex Agent Tasks".to_string()
    } else if lower.contains("架构") || lower.contains("architecture") {
        "Use Architecture Prompts Before Implementation".to_string()
    } else {
        title_from_body(body)
    }
}

pub(super) fn classify_kind(sentence: &str) -> &'static str {
    let lower = sentence.to_lowercase();
    if lower.contains("不要")
        || lower.contains("禁止")
        || lower.contains("never")
        || lower.contains("do not")
        || lower.contains("don't")
    {
        "constraint"
    } else {
        "preference"
    }
}

pub(super) fn infer_scope(sentence: &str) -> &'static str {
    let lower = sentence.to_lowercase();
    if lower.contains("所有项目") || lower.contains("全局") || lower.contains("for all") {
        "global"
    } else if lower.contains("claude") || lower.contains("codex") || lower.contains("agent") {
        "agent"
    } else {
        "project"
    }
}
