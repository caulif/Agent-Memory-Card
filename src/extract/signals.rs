use std::collections::BTreeSet;

/// 一个被标记为"可能有价值"的段落，包含原始文本和命中的信号类型标签。
/// 标签用于后续 LLM prompt 上下文，帮助 LLM 理解为什么这段被选中。
#[derive(Debug, Clone)]
pub(crate) struct CandidateParagraph {
    /// 原始文本（完整段落）
    pub text: String,
    /// 命中的信号类型
    pub signals: Vec<SignalType>,
}

/// 段落级信号类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SignalType {
    /// 偏好声明：明确表达"以后用 X 替代 Y"
    PreferenceDeclaration,
    /// 约束声明："禁止"、"不允许"、"不要" 等
    ConstraintDeclaration,
    /// 项目改进记录："根因是"、"最终做法是"
    ProjectImprovement,
    /// 流程描述：多步骤操作或工作流
    WorkflowDescription,
    /// 架构决策：选择 X 而非 Y 的技术决策
    ArchitectureDecision,
    /// 显式记忆标记："以后"、"记住"、"always"、"never"
    ExplicitMemoryMarker,
    /// 跨项目方法论原则
    PrincipleStatement,
    /// 规划与验证启发式
    PlanningHeuristic,
    /// 与 agent 协作的稳定偏好
    CollaborationPreference,
}

// ============================================================
// 第一层：文本拆分
// ============================================================

/// 按段落拆分文本：先按双换行（段落），段落内再按单换行/句号拆句子
pub(crate) fn split_into_paragraphs(input: &str) -> Vec<String> {
    input
        .split("\n\n")
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

/// 按句子拆分文本（保留向后兼容）
pub(super) fn split_sentences(input: &str) -> Vec<&str> {
    let mut sentences = Vec::new();
    let mut start = 0;

    for (index, ch) in input.char_indices() {
        if !is_sentence_boundary(input, index, ch) {
            continue;
        }

        let sentence = input[start..index].trim();
        if !sentence.is_empty() {
            sentences.push(sentence);
        }
        start = index + ch.len_utf8();
    }

    let sentence = input[start..].trim();
    if !sentence.is_empty() {
        sentences.push(sentence);
    }

    sentences
}

fn is_sentence_boundary(input: &str, index: usize, ch: char) -> bool {
    match ch {
        '\n' | '。' | '！' | '？' | '!' | '?' => true,
        '.' => is_period_sentence_boundary(input, index),
        _ => false,
    }
}

fn is_period_sentence_boundary(input: &str, index: usize) -> bool {
    let previous = input[..index].chars().rev().find(|ch| !ch.is_whitespace());
    let next = input[index + 1..].chars().next();

    !matches!(
        (previous, next),
        (Some(left), Some(right))
            if left.is_ascii_alphanumeric() && right.is_ascii_alphanumeric()
    )
}

// ============================================================
// 第二层：噪音过滤
// ============================================================

/// 判断句子是否为低价值的任务指令或系统噪音
pub(super) fn is_low_value_task_sentence(sentence: &str) -> bool {
    let lower = sentence.trim().to_lowercase();
    let trimmed = lower.trim();

    // 系统噪音标记（工具输出、元数据、系统命令）
    let noisy_markers = [
        "<local-command",
        "<command-name>",
        "error when starting",
        "stack trace",
        "exit code",
        "token_count",
        "base_instructions",
        "error[",
        "warning[",
    ];
    if noisy_markers.iter().any(|marker| trimmed.contains(marker)) {
        return true;
    }

    // 纯代码输出（无自然语言的行块）
    if is_code_output_block(trimmed) {
        return true;
    }

    // Agent 内部独白：第一人称 + 即时动词 + 无耐久标记
    if is_agent_internal_monologue(trimmed) {
        return true;
    }

    // 单次任务请求：开头位置 + 请求动词 + 无耐久标记
    if is_one_off_task_request(trimmed) {
        return true;
    }

    if super::gate::looks_like_one_off_project_execution_request(trimmed) {
        return true;
    }

    // 未解决的抱怨：用户表达不满或需求但无具体做法
    if looks_like_unresolved_user_request(trimmed) && !has_explicit_memory_marker(trimmed) {
        return true;
    }

    false
}

/// 判断是否为纯代码输出块（连续代码行，无人类自然语言）
fn is_code_output_block(text: &str) -> bool {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() < 3 {
        return false;
    }

    let code_line_count = lines
        .iter()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed.is_empty()
                || trimmed.starts_with("//")
                || trimmed.starts_with('#')
                || trimmed.starts_with('$')
                || trimmed.starts_with("---")
                || trimmed.starts_with("-->")
                || trimmed.contains(" = ")
                || trimmed.contains("->")
                || trimmed.contains("=>")
                || (trimmed.ends_with(';') && !is_natural_language_line(trimmed))
        })
        .count();

    // 超过 80% 的行看起来像代码 = 代码块
    code_line_count * 100 / lines.len() > 80
}

/// 判断一行文本是否为自然语言（而非代码）
fn is_natural_language_line(text: &str) -> bool {
    text.starts_with("以")
        || text.starts_with("今")
        || text.starts_with("这")
        || text.starts_with("我")
        || text.starts_with("你")
        || text.starts_with("该")
        || text.starts_with("如")
        || text.starts_with("If")
        || text.starts_with("We")
        || text.starts_with("You")
        || text.starts_with("This")
        || text.starts_with("The")
        || text.starts_with("A ")
}

/// 判断是否为 Agent 内部独白
fn is_agent_internal_monologue(text: &str) -> bool {
    let monologue_markers = [
        "let me",
        "i'll",
        "i will",
        "i need to",
        "正在分析",
        "我先",
        "让我",
        "我需要",
    ];
    let word_count = text.split_whitespace().count();

    let has_monologue = monologue_markers.iter().any(|marker| text.contains(marker));
    let is_short = word_count < 15;
    let has_project_nouns = text.contains("rust")
        || text.contains("tauri")
        || text.contains("claude")
        || text.contains("codex")
        || text.contains("bun")
        || text.contains("项目")
        || text.contains("测试");

    has_monologue && is_short && !has_project_nouns
}

/// 判断是否为单次任务请求（非可复用知识）
fn is_one_off_task_request(text: &str) -> bool {
    let request_starts = [
        "帮我", "请帮", "告诉", "如何", "怎么", "现在", "what is", "how do", "how to", "please",
    ];
    let is_request = request_starts.iter().any(|marker| text.starts_with(marker));

    // 如果有耐久标记，即使是"帮我"开头也可能有价值
    is_request && !has_explicit_memory_marker(text) && !has_strong_memory_marker(text)
}

// ============================================================
// 第三层：信号检测（判断段落是否有潜在价值）
// ============================================================

/// 检测所有候选段落（先过滤噪音，再对每个剩余段落做信号检测）
pub(crate) fn detect_candidate_paragraphs(paragraphs: &[String]) -> Vec<CandidateParagraph> {
    let mut candidates = Vec::new();

    for paragraph in paragraphs {
        if paragraph.trim().is_empty() {
            continue;
        }

        let sentences = split_sentences(paragraph);
        let mut paragraph_signals = Vec::new();
        let mut has_valuable_sentence = false;

        for sentence in &sentences {
            if is_low_value_task_sentence(sentence) {
                continue;
            }

            let mut sentence_signals = detect_sentence_signals(sentence);
            if !sentence_signals.is_empty() {
                has_valuable_sentence = true;
                paragraph_signals.append(&mut sentence_signals);
            }
        }

        if has_valuable_sentence {
            // 去重信号类型
            let mut unique_signals: Vec<SignalType> = Vec::new();
            let mut seen = BTreeSet::new();
            for signal in paragraph_signals {
                let key = format!("{:?}", signal);
                if seen.insert(key) {
                    unique_signals.push(signal);
                }
            }

            candidates.push(CandidateParagraph {
                text: paragraph.clone(),
                signals: unique_signals,
            });
        }
    }

    candidates
}

/// 对单个句子进行信号检测
fn detect_sentence_signals(sentence: &str) -> Vec<SignalType> {
    let lower = sentence.to_lowercase();
    let mut signals = Vec::new();

    // 偏好的检测：正向替代 + 耐久标记
    if has_preference_signal(&lower) {
        signals.push(SignalType::PreferenceDeclaration);
    }

    // 约束的检测
    if has_constraint_signal(&lower) {
        signals.push(SignalType::ConstraintDeclaration);
    }

    // 项目改进记录
    if looks_like_project_improvement_signal(sentence) {
        signals.push(SignalType::ProjectImprovement);
    }

    // 流程描述
    if has_workflow_signal(sentence) {
        signals.push(SignalType::WorkflowDescription);
    }

    // 架构决策
    if has_architecture_decision_signal(&lower) {
        signals.push(SignalType::ArchitectureDecision);
    }

    if has_principle_signal(&lower) {
        signals.push(SignalType::PrincipleStatement);
    }

    if has_planning_heuristic_signal(&lower) {
        signals.push(SignalType::PlanningHeuristic);
    }

    if has_collaboration_preference_signal(&lower) {
        signals.push(SignalType::CollaborationPreference);
    }

    // 显式记忆标记
    if has_explicit_memory_marker(sentence) {
        signals.push(SignalType::ExplicitMemoryMarker);
    }

    signals
}

fn has_preference_signal(lower: &str) -> bool {
    let durable_markers = [
        "以后",
        "所有",
        "每次",
        "总是",
        "默认",
        "统一",
        "优先",
        "改用",
        "always",
        "prefer",
        "by default",
        "default to",
        "for every",
        "for all",
    ];
    let tool_markers = [
        "axios",
        "bun",
        "ky",
        "ofetch",
        "vitest",
        "playwright",
        "cargo",
        "npm",
        "pnpm",
        "yarn",
        "fetch",
        "jest",
        "react",
        "tanstack",
        "turbo",
        "biome",
        "oxlint",
        "typescript",
        "rust",
        "go",
        "前端",
        "后端",
        "测试",
        "包管理",
    ];

    durable_markers.iter().any(|m| lower.contains(m))
        && tool_markers.iter().any(|m| lower.contains(m))
}

fn has_constraint_signal(lower: &str) -> bool {
    let constraint_markers = [
        "不要",
        "禁止",
        "不允许",
        "不能",
        "never",
        "do not",
        "don't",
        "must not",
        "should not",
    ];
    let reason_markers = ["因为", "否则", "会导致", "否则会", "because", "otherwise"];

    let has_constraint = constraint_markers.iter().any(|m| lower.contains(m));
    // 约束如果有理由说明更有价值
    let has_reason = reason_markers.iter().any(|m| lower.contains(m));

    has_constraint && has_reason
}

fn has_workflow_signal(sentence: &str) -> bool {
    let lower = sentence.to_lowercase();
    let step_markers = [
        "先",
        "再",
        "然后",
        "最后",
        "第一步",
        "第二步",
        "first",
        "then",
        "finally",
        "step 1",
        "step 2",
    ];
    let action_markers = [
        "运行", "检查", "验证", "确认", "提交", "构建", "run", "check", "verify", "build", "test",
    ];

    let step_count = step_markers.iter().filter(|m| lower.contains(*m)).count();
    let action_count = action_markers.iter().filter(|m| lower.contains(*m)).count();

    step_count >= 2 || (step_count >= 1 && action_count >= 2) || action_count >= 3
}

fn has_architecture_decision_signal(lower: &str) -> bool {
    let decision_markers = [
        "选用",
        "选择",
        "放弃",
        "不用",
        "取代",
        "决定",
        "chose",
        "decided to",
        "went with",
        "instead of",
    ];
    let reason_markers = [
        "因为",
        "原因",
        "考虑到",
        "基于",
        "由于",
        "because",
        "reason",
        "since",
        "due to",
    ];

    decision_markers.iter().any(|m| lower.contains(m))
        && reason_markers.iter().any(|m| lower.contains(m))
}

pub(super) fn has_principle_signal(lower: &str) -> bool {
    let principle_topics = [
        "核心功能",
        "用户视角",
        "用户体验",
        "体验优化",
        "体验",
        "可用性",
        "稳定性",
        "性能",
        "跨项目",
        "真正有价值",
        "真实历史",
        "候选质量",
        "候选数量",
        "持续修正",
        "自我修正",
        "自检",
        "真实结果",
        "推理引擎",
        "固定规则词",
        "规则词",
        "高价值 prompt",
        "高价值prompt",
        "高价值表达",
        "core functionality",
        "user perspective",
        "user experience",
        "feedback loop",
        "real history",
    ];
    let stance_markers = [
        "优先",
        "重要",
        "更重要",
        "更在意",
        "重视",
        "关心",
        "更关心",
        "评估",
        "判断",
        "标准",
        "应该",
        "希望",
        "倾向",
        "看重",
        "不要固定",
        "不要只",
        "matters",
        "priority",
        "evaluate",
        "judge by",
        "focus on",
        "prioritize",
    ];

    principle_topics.iter().any(|topic| lower.contains(topic))
        && stance_markers.iter().any(|marker| lower.contains(marker))
}

pub(super) fn has_planning_heuristic_signal(lower: &str) -> bool {
    let planning_topics = [
        "规划",
        "提问",
        "澄清目标",
        "拆问题",
        "拆任务",
        "分析",
        "小改快测",
        "小修改快测",
        "小修改做快测",
        "大改",
        "重测",
        "完整回归",
        "回归",
        "持续修正",
        "自检",
        "真实结果",
        "推理引擎",
        "planning",
        "clarifying",
        "decompose",
        "small change",
        "targeted test",
        "dry-run",
        "dry run",
        "regression",
    ];
    let order_markers = ["先", "再", "before", "first", "then"];

    planning_topics.iter().any(|topic| lower.contains(topic))
        && (order_markers.iter().any(|marker| lower.contains(marker))
            || planning_topics
                .iter()
                .filter(|topic| lower.contains(**topic))
                .count()
                >= 2)
}

pub(super) fn has_collaboration_preference_signal(lower: &str) -> bool {
    let collaboration_topics = [
        "审阅边界",
        "review boundary",
        "真实历史",
        "real history",
        "候选质量",
        "持续修正",
        "自我修正",
        "用户确认",
        "不要让 ai 直接",
        "先 review",
        "先审阅",
        "小改快测",
        "自检",
        "真实结果",
        "推理引擎",
        "检查有没有问题",
        "可视化",
        "mockup",
        "对比图",
        "流程图",
        "架构图",
        "targeted test",
        "dry-run",
        "dry run",
    ];
    let preference_markers = [
        "希望",
        "优先",
        "保留",
        "先",
        "不要",
        "重视",
        "关心",
        "更关心",
        "should",
        "prefer",
        "keep",
        "检查",
        "验证",
    ];

    (collaboration_topics
        .iter()
        .any(|topic| lower.contains(topic))
        || has_project_startup_reference_signal(lower))
        && preference_markers
            .iter()
            .any(|marker| lower.contains(marker))
}

fn has_project_startup_reference_signal(lower: &str) -> bool {
    (lower.contains("借鉴")
        || lower.contains("参考")
        || lower.contains("开源项目")
        || lower.contains("同类产品")
        || lower.contains("相关内容"))
        && (lower.contains("规划")
            || lower.contains("方案")
            || lower.contains("启动")
            || lower.contains("实现新功能")
            || lower.contains("新增功能"))
}

// ============================================================
// 信号检测辅助函数（保留并改进自原有代码）
// ============================================================

/// 检测项目改进/修复记录信号（保留并改进）
pub(super) fn looks_like_project_improvement_signal(sentence: &str) -> bool {
    let lower = sentence.to_lowercase();
    let trimmed = lower.trim();

    let outcome_markers = [
        "修复了",
        "解决了",
        "优化了",
        "重构了",
        "落地了",
        "实现了",
        "改成",
        "最终做法",
        "根因是",
        "原因是",
        "避免",
        "性能",
        "卡死",
        "卡顿",
        "显示不完整",
        "跨平台",
        "回滚",
        "通过测试",
        "fixed",
        "resolved",
        "root cause",
        "final approach",
        "implemented",
        "refactored",
        "improved",
        "avoid",
        "performance",
        "regression",
    ];
    let project_markers = [
        "项目",
        "ui",
        "界面",
        "前端",
        "后端",
        "tauri",
        "rust",
        "bun",
        "codex",
        "claude",
        "agent",
        "memory_card",
        "测试",
        "架构",
        "扫描",
        "整理",
        "增量",
        "启动",
        "project",
        "frontend",
        "backend",
        "architecture",
        "test",
        "startup",
        "incremental",
    ];
    let completed_markers = [
        "这次",
        "最终",
        "根因",
        "原因",
        "resolved",
        "root cause",
        "final approach",
    ];

    let has_outcome = outcome_markers.iter().any(|marker| lower.contains(marker));
    let has_project = project_markers.iter().any(|marker| lower.contains(marker));
    let has_completed = completed_markers
        .iter()
        .any(|marker| lower.contains(marker));

    if looks_like_code_analysis_noise(trimmed) {
        return false;
    }

    // 过滤未解决的抱怨（除非有完成标记或显式记忆标记）
    if looks_like_unresolved_user_request(trimmed)
        && !has_completed
        && !has_explicit_memory_marker(trimmed)
    {
        return false;
    }

    has_outcome && has_project
}

fn looks_like_code_analysis_noise(lower: &str) -> bool {
    let code_symbols = lower.contains("`")
        || lower.contains("fn ")
        || lower.contains("src/")
        || lower.contains(".rs")
        || lower.contains("::");
    let analysis_terms = [
        "函数",
        "信号检测",
        "层层递进",
        "有效地",
        "区分开",
        "噪声过滤",
        "实现细节",
        "代码中",
    ]
    .iter()
    .any(|marker| lower.contains(marker));
    code_symbols && analysis_terms
}

/// 检测未解决的用户请求/抱怨
pub(super) fn looks_like_unresolved_user_request(text: &str) -> bool {
    let weak_request = [
        "我希望",
        "希望",
        "我想",
        "我需要",
        "需要",
        "要求",
        "应该",
        "最好",
    ];
    let unresolved_only = [
        "能不能",
        "是否",
        "为什么",
        "现在这样",
        "太低",
        "有问题",
        "不对",
        "重复",
        "残留",
        "bug",
        "卡在",
        "太丑",
        "不好用",
        "能不能优化",
    ];
    if unresolved_only.iter().any(|marker| text.contains(marker)) {
        return true;
    }

    if weak_request.iter().any(|marker| text.contains(marker)) {
        let principle_topics = [
            "核心功能",
            "用户视角",
            "用户体验",
            "体验",
            "规划",
            "提问",
            "跨项目",
            "真正有价值",
            "真实历史",
            "快测",
            "回归",
            "审阅边界",
            "core functionality",
            "user perspective",
            "planning",
            "real history",
        ];
        if principle_topics.iter().any(|topic| text.contains(topic)) {
            return false;
        }
        return true;
    }
    false
}

/// 检查是否为显式记忆标记（"以后要多做某事"的概念）
pub(super) fn has_explicit_memory_marker(sentence: &str) -> bool {
    let lower = sentence.to_lowercase();
    [
        "以后",
        "所有项目",
        "所有请求",
        "每次",
        "总是",
        "默认",
        "统一",
        "优先",
        "记住",
        "always",
        "never",
        "prefer",
        "by default",
        "for all",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

/// 检查是否为强记忆标记（约束性更强）
pub(super) fn has_strong_memory_marker(sentence: &str) -> bool {
    let lower = sentence.to_lowercase();
    [
        "以后", "记住", "总是", "必须", "禁止", "不要", "always", "never", "must", "do not",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}
