//! LLM 驱动提取引擎：将候选段落发送给 LLM，提取结构化知识。
//!
//! 负责 Prompt 构建和 JSON 解析，实际的 HTTP 调用通过 provider 模块。

use crate::candidate;
use crate::provider;

use super::signals::{CandidateParagraph, SignalType};

/// LLM 提取的单个知识项
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub(super) struct LlmExtractedKnowledge {
    /// 知识类型
    pub kind: KnowledgeKind,
    /// 简短标题（中文或英文，跟随原文语言）
    pub title: String,
    /// 规范化的可复用描述
    pub body: String,
    /// LLM 判断的置信度
    pub confidence: f32,
    /// LLM 解释为什么保留/拒绝
    #[serde(default)]
    pub rationale: String,
    /// 原文中支持该事实的短引用
    #[serde(default)]
    pub evidence_quote: Option<String>,
    /// 原文语言：zh / en / mixed
    #[serde(default)]
    pub language: Option<String>,
    /// 记忆层级猜测：project_rule / cross_project_principle / collaboration_preference
    #[serde(default)]
    pub memory_tier_guess: Option<String>,
    /// 跨项目可复用性评分
    #[serde(default)]
    pub reusability_score: Option<f32>,
    /// 长期有效性评分
    #[serde(default)]
    pub durability_score: Option<f32>,
    /// 自包含和可执行程度评分
    #[serde(default)]
    pub specificity_score: Option<f32>,
    /// 来源可信度评分，用户明确确认最高，assistant 自说自话较低
    #[serde(default)]
    pub source_trust_score: Option<f32>,
    /// 是否标记为噪音
    #[serde(default)]
    pub is_noise: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum MemoryOperation {
    Add,
    Update,
    Supersede,
    Conflict,
    Noop,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub(super) struct LlmUpdateDecision {
    pub operation: MemoryOperation,
    #[serde(default)]
    pub target_id: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum JudgeDecision {
    Keep,
    Reject,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub(super) struct LlmQualityJudgment {
    pub decision: JudgeDecision,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub confidence: Option<f32>,
    #[serde(default)]
    pub durability: Option<f32>,
    #[serde(default)]
    pub reusability: Option<f32>,
    #[serde(default)]
    pub specificity: Option<f32>,
    #[serde(default)]
    pub evidence_grounded: Option<f32>,
    #[serde(default)]
    pub noise_risk: Option<f32>,
}

/// 知识类型枚举
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum KnowledgeKind {
    /// 工具/库/方法的偏好选择
    Preference,
    /// 禁止或不推荐的做法
    Constraint,
    /// 可复用的操作流程或步骤
    Procedure,
    /// 用户反复纠正 agent 的内容
    Correction,
    /// 架构决策及选择理由
    Decision,
    /// 针对特定技能的使用经验
    Supplement,
}

/// LLM 提取请求
#[allow(dead_code)]
pub(super) struct LlmExtractRequest {
    /// 候选段落（最多 20 段/次）
    pub paragraphs: Vec<CandidateParagraph>,
    /// 最大 tokens
    pub max_tokens: usize,
}

/// 构建 LLM 提取请求，分离稳定 system prompt 和变化的 user prompt，便于缓存。
pub(super) fn build_extraction_prompt(
    paragraphs: &[CandidateParagraph],
) -> provider::ProviderRequest {
    let material = build_extraction_material(paragraphs);
    provider::ProviderRequest {
        system_prompt: r#"你是一个从 Agent 编程对话中提取可复用知识的助手。

请分析以下被标记为"可能有价值"的对话段落。先提取 atomic facts，再判断是否值得进入记忆系统。LLM 是语义过滤层：宁可少提取，也不要把一次性任务转成记忆。对每个段落：
1. 判断它是否真的包含可复用的 agent 知识（一次性的任务指令、抱怨、未解决的请求不算）
2. 如果是，提取为结构化知识项；一句话包含默认规则和例外时，拆成多条 atomic facts
3. 如果只是单次任务指令、抱怨、或噪音，标记为 is_noise: true，仍需返回该项

可复用知识类型（对应 kind 字段）：
- preference: 工具/库/方法的偏好选择（如"以后用 X 替代 Y"）
- constraint: 禁止或不推荐的做法（如"不要直接改生成的代码"）
- procedure: 可复用的操作流程或步骤（如"发布前按 clippy → test → lock 顺序检查"）
- correction: 用户反复纠正 agent 的内容（如"你已经第三次忘了跑测试了"）
- decision: 架构决策及选择理由（如"选 SQLite 而不是 Postgres 因为..."）
- supplement: 针对特定技能的使用经验（如"用 officecli 时先检查文档是否存在"）

要求：
- 返回纯 JSON 数组，不要包含 markdown 代码块标记
- 最多 15 项
- confidence 取值 0.0-1.0，低于 0.75 的知识项通常不值得保留
- body 必须简洁、通用、命令式、可复用
- evidence_quote 必须从原文复制支持该结论的短句，不要输出 observation id、sha256 或内部评分字段
- language 只能是 zh / en / mixed
- memory_tier_guess 只能是 project_rule / cross_project_principle / collaboration_preference
- reusability_score 取值 0.0-1.0，用来判断这条知识是否值得抽象成跨项目原则
- durability_score 取值 0.0-1.0，判断它是否长期有效而不是一次性任务
- specificity_score 取值 0.0-1.0，判断它是否自包含、具体、可执行
- source_trust_score 取值 0.0-1.0，用户明确表达或确认最高，assistant 未确认内容较低
- title 跟随原文语言（中文原文用中文标题，英文原文用英文标题）
- 只提取对以后任务有用的内容，忽略所有一次性指令
- 每条 keep 项都必须隐含 Trigger + Action + Boundary/Why；缺任何一项就标记为 is_noise
- assistant-origin 或 AI synthesis 只有在原文包含“用户确认/基于用户确认/accepted/采纳/最终做法”等证据时才可 keep，否则 reject 或降低 source_trust_score

Placement rubric（用于 memory_tier_guess 和 rationale）：
- project_rule: 当前项目的短规则、约束、工具选择、验证要求
- cross_project_principle: 去掉项目/工具专名后仍成立的产品或工程原则
- collaboration_preference: 用户希望 agent 如何计划、验证、审阅、汇报、处理反馈
- workflow_skill: 多步骤流程、handoff prompt、检查清单；仍用 procedure kind，但 rationale 说明它适合 Skill
- review_only: assistant-origin、治理边界、风险较高或需要人工措辞的内容；仍可 keep，但 rationale 必须说明需要 review

Few-shot:
输入："以后 HTTP 默认用 Axios，但上传大文件保留 fetch，因为需要 ReadableStream。"
输出两条 atomic facts：
1. Use Axios for default HTTP requests. evidence_quote="HTTP 默认用 Axios"
2. Keep fetch for large uploads that require ReadableStream. evidence_quote="上传大文件保留 fetch"

Update phase schema:
后续会把每条 atomic fact 与相似 MemoryCard 比较，并要求你只返回 ADD / UPDATE / SUPERSEDE / CONFLICT / NOOP 之一。"#
            .to_string(),
        user_prompt: format!("待分析的对话段落：\n{material}"),
        json_schema: Some(extraction_schema()),
    }
}

/// 将候选段落格式化为 LLM 输入
fn build_extraction_material(paragraphs: &[CandidateParagraph]) -> String {
    let mut out = String::new();
    for (i, paragraph) in paragraphs.iter().enumerate() {
        let signal_labels = paragraph
            .signals
            .iter()
            .map(signal_label)
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "--- 段落 {} [信号: {}] ---\n{}\n\n",
            i + 1,
            signal_labels,
            paragraph.text
        ));
    }
    out
}

fn signal_label(signal: &SignalType) -> &'static str {
    match signal {
        SignalType::PreferenceDeclaration => "偏好声明",
        SignalType::ConstraintDeclaration => "约束声明",
        SignalType::ProjectImprovement => "项目改进",
        SignalType::WorkflowDescription => "流程描述",
        SignalType::ArchitectureDecision => "架构决策",
        SignalType::ExplicitMemoryMarker => "耐久标记",
        SignalType::PrincipleStatement => "方法论原则",
        SignalType::PlanningHeuristic => "规划启发",
        SignalType::CollaborationPreference => "协作偏好",
    }
}

/// 解析 LLM 返回的 JSON 结果
pub(super) fn parse_extraction_output(
    output: &str,
) -> Result<Vec<LlmExtractedKnowledge>, ParseError> {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return Err(ParseError::EmptyOutput);
    }

    let items = serde_json::from_str::<Vec<LlmExtractedKnowledge>>(trimmed)
        .map_err(|_| ParseError::InvalidJson(trimmed.chars().take(200).collect()))?;
    validate_extracted_knowledge(&items)?;
    Ok(items)
}

fn validate_extracted_knowledge(items: &[LlmExtractedKnowledge]) -> Result<(), ParseError> {
    for (index, item) in items.iter().enumerate() {
        if item.is_noise {
            continue;
        }
        for (field, value) in [
            ("memory_tier_guess", item.memory_tier_guess.as_deref()),
            (
                "reusability_score",
                item.reusability_score.map(|_| "present"),
            ),
            ("durability_score", item.durability_score.map(|_| "present")),
            (
                "specificity_score",
                item.specificity_score.map(|_| "present"),
            ),
            (
                "source_trust_score",
                item.source_trust_score.map(|_| "present"),
            ),
        ] {
            if value.is_none() {
                return Err(ParseError::MissingRequired(format!(
                    "item {index} missing required {field}"
                )));
            }
        }
    }
    Ok(())
}

pub(super) fn build_update_decision_prompt(
    atomic_fact: &str,
    similar_memory_cards: &[(String, String)],
) -> provider::ProviderRequest {
    let mut similar = String::new();
    for (id, body) in similar_memory_cards {
        similar.push_str(&format!("- id: {id}\n  body: {body}\n"));
    }
    provider::ProviderRequest {
        system_prompt:
            r#"You decide how a newly extracted atomic memory should affect existing MemoryCards.

Return only JSON with:
- operation: add | update | supersede | conflict | noop
- target_id: existing MemoryCard id when applicable
- body: revised reusable rule when applicable
- reason: concise explanation

Use add for genuinely new durable knowledge.
Use update for clearer wording of the same rule.
Use supersede when the user reverses or replaces an old rule.
Use conflict when both rules may be valid but need human review.
Use noop when the fact is duplicate, low value, or already implied."#
                .to_string(),
        user_prompt: format!(
            "Atomic fact:\n{atomic_fact}\n\nSimilar MemoryCards:\n{}",
            if similar.is_empty() {
                "(none)".to_string()
            } else {
                similar
            }
        ),
        json_schema: Some(update_decision_schema()),
    }
}

pub(super) fn parse_update_decision_output(output: &str) -> Result<LlmUpdateDecision, ParseError> {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return Err(ParseError::EmptyOutput);
    }
    serde_json::from_str::<LlmUpdateDecision>(trimmed)
        .map_err(|_| ParseError::InvalidJson(trimmed.chars().take(200).collect()))
}

pub(super) fn run_update_decision(
    project_root: &std::path::Path,
    atomic_fact: &str,
    similar_memory_cards: &[(String, String)],
) -> Result<LlmUpdateDecision, LlmExtractionError> {
    let prompt = build_update_decision_prompt(atomic_fact, similar_memory_cards);
    let cfg = provider::load_or_default_provider_config(project_root)
        .map_err(|e| LlmExtractionError::ProviderError(e.to_string()))?;
    let output =
        provider::call_provider_for_role(&cfg, provider::ProviderRole::Update, &prompt, 768)
            .map_err(|e| LlmExtractionError::ProviderError(e.to_string()))?;
    parse_update_decision_output(&output).map_err(|e| {
        LlmExtractionError::ParseError(format!(
            "{e}: {}",
            &output.chars().take(200).collect::<String>()
        ))
    })
}

pub(super) fn build_quality_judge_prompt(
    body: &str,
    evidence: &str,
    existing_memory_cards: &[(String, String)],
) -> provider::ProviderRequest {
    let mut existing = String::new();
    for (id, body) in existing_memory_cards {
        existing.push_str(&format!("- id: {id}\n  body: {body}\n"));
    }
    provider::ProviderRequest {
        system_prompt: r#"You are a binary judge for extracted Agent memory candidates.

Return only JSON:
{
  "decision": "keep" | "reject",
  "reason": "short reason",
  "confidence": 0.0-1.0,
  "durability": 0.0-1.0,
  "reusability": 0.0-1.0,
  "specificity": 0.0-1.0,
  "evidence_grounded": 0.0-1.0,
  "noise_risk": 0.0-1.0
}

KEEP only when the candidate is durable, reusable, self-contained, evidence-grounded, and useful for future coding-agent behavior.
REJECT one-off tasks, current diff/write-scope boundaries, unresolved questions, generic advice, duplicated existing MemoryCards, internal metadata leaks, assistant-injected rules without user acceptance, and vague personality preferences.
Prefer REVIEW-ONLY style reasoning for assistant-origin governance memories even when they are useful.
Good keep examples:
- 当改动较小时先运行针对性快测；风险较高时运行完整回归。
- 当候选来自 assistant synthesis 时，先要求用户确认再固化。
Bad reject examples:
- 这次只读，不要写文件。
- 为什么这条候选分数这么低？
- 当前实现里 memory_gate 有一个函数。
Judge the candidate independently. Do not compare candidates against each other."#
            .to_string(),
        user_prompt: format!(
            "Candidate body:\n{body}\n\nEvidence quote:\n{evidence}\n\nExisting similar MemoryCards:\n{}",
            if existing.is_empty() {
                "(none)".to_string()
            } else {
                existing
            }
        ),
        json_schema: Some(quality_judge_schema()),
    }
}

pub(super) fn parse_quality_judgment_output(
    output: &str,
) -> Result<LlmQualityJudgment, ParseError> {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return Err(ParseError::EmptyOutput);
    }
    serde_json::from_str::<LlmQualityJudgment>(trimmed)
        .map_err(|_| ParseError::InvalidJson(trimmed.chars().take(200).collect()))
}

pub(super) fn run_quality_judge(
    project_root: &std::path::Path,
    body: &str,
    evidence: &str,
    existing_memory_cards: &[(String, String)],
) -> Result<LlmQualityJudgment, LlmExtractionError> {
    let prompt = build_quality_judge_prompt(body, evidence, existing_memory_cards);
    let cfg = provider::load_or_default_provider_config(project_root)
        .map_err(|e| LlmExtractionError::ProviderError(e.to_string()))?;
    let first = run_quality_judge_once(&cfg, &prompt)?;
    if !is_ambiguous_quality_judgment(&first, &cfg.judge) {
        return Ok(first);
    }
    let mut judgments = vec![first];
    for _ in 0..2 {
        judgments.push(run_quality_judge_once(&cfg, &prompt)?);
    }
    Ok(combine_quality_judgments(judgments))
}

fn run_quality_judge_once(
    cfg: &provider::ProviderConfig,
    prompt: &provider::ProviderRequest,
) -> Result<LlmQualityJudgment, LlmExtractionError> {
    let output = provider::call_provider_for_role(cfg, provider::ProviderRole::Judge, prompt, 512)
        .map_err(|e| LlmExtractionError::ProviderError(e.to_string()))?;
    parse_quality_judgment_output(&output).map_err(|e| {
        LlmExtractionError::ParseError(format!(
            "{e}: {}",
            &output.chars().take(200).collect::<String>()
        ))
    })
}

fn is_ambiguous_quality_judgment(
    judgment: &LlmQualityJudgment,
    cfg: &provider::JudgeConfig,
) -> bool {
    judgment
        .noise_risk
        .is_some_and(|score| score >= cfg.ambiguous_noise_low && score <= cfg.ambiguous_noise_high)
        || judgment.evidence_grounded.is_some_and(|score| {
            score >= cfg.ambiguous_grounding_low && score <= cfg.ambiguous_grounding_high
        })
}

fn combine_quality_judgments(judgments: Vec<LlmQualityJudgment>) -> LlmQualityJudgment {
    let keep_votes = judgments
        .iter()
        .filter(|judgment| judgment.decision == JudgeDecision::Keep)
        .count();
    let reject_votes = judgments.len().saturating_sub(keep_votes);
    let decision = if keep_votes >= reject_votes {
        JudgeDecision::Keep
    } else {
        JudgeDecision::Reject
    };
    let reason = judgments
        .iter()
        .map(|judgment| judgment.reason.as_str())
        .filter(|reason| !reason.trim().is_empty())
        .collect::<Vec<_>>()
        .join("; ");
    LlmQualityJudgment {
        decision,
        reason,
        confidence: average_score(judgments.iter().filter_map(|judgment| judgment.confidence)),
        durability: average_score(judgments.iter().filter_map(|judgment| judgment.durability)),
        reusability: average_score(judgments.iter().filter_map(|judgment| judgment.reusability)),
        specificity: average_score(judgments.iter().filter_map(|judgment| judgment.specificity)),
        evidence_grounded: average_score(
            judgments
                .iter()
                .filter_map(|judgment| judgment.evidence_grounded),
        ),
        noise_risk: average_score(judgments.iter().filter_map(|judgment| judgment.noise_risk)),
    }
}

fn average_score(values: impl Iterator<Item = f32>) -> Option<f32> {
    let values = values.collect::<Vec<_>>();
    if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<f32>() / values.len() as f32)
    }
}

fn extraction_schema() -> provider::ProviderJsonSchema {
    provider::ProviderJsonSchema {
        name: "AtomicFactExtraction".to_string(),
        strict: true,
        schema: serde_json::json!({
            "type": "array",
            "items": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "kind": { "type": "string", "enum": ["preference", "constraint", "procedure", "correction", "decision", "supplement"] },
                    "title": { "type": "string" },
                    "body": { "type": "string" },
                    "confidence": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
                    "rationale": { "type": "string" },
                    "evidence_quote": { "type": ["string", "null"] },
                    "language": { "type": ["string", "null"], "enum": ["zh", "en", "mixed", null] },
                    "memory_tier_guess": { "type": ["string", "null"], "enum": ["project_rule", "cross_project_principle", "collaboration_preference", null] },
                    "reusability_score": { "type": ["number", "null"], "minimum": 0.0, "maximum": 1.0 },
                    "durability_score": { "type": ["number", "null"], "minimum": 0.0, "maximum": 1.0 },
                    "specificity_score": { "type": ["number", "null"], "minimum": 0.0, "maximum": 1.0 },
                    "source_trust_score": { "type": ["number", "null"], "minimum": 0.0, "maximum": 1.0 },
                    "is_noise": { "type": "boolean" }
                },
                "required": ["kind", "title", "body", "confidence", "rationale", "evidence_quote", "language", "memory_tier_guess", "reusability_score", "durability_score", "specificity_score", "source_trust_score", "is_noise"]
            }
        }),
    }
}

fn update_decision_schema() -> provider::ProviderJsonSchema {
    provider::ProviderJsonSchema {
        name: "MemoryCardUpdateDecision".to_string(),
        strict: true,
        schema: serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "operation": { "type": "string", "enum": ["add", "update", "supersede", "conflict", "noop"] },
                "target_id": { "type": ["string", "null"] },
                "body": { "type": ["string", "null"] },
                "reason": { "type": "string" }
            },
            "required": ["operation", "target_id", "body", "reason"]
        }),
    }
}

fn quality_judge_schema() -> provider::ProviderJsonSchema {
    provider::ProviderJsonSchema {
        name: "CandidateQualityJudgment".to_string(),
        strict: true,
        schema: serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "decision": { "type": "string", "enum": ["keep", "reject"] },
                "reason": { "type": "string" },
                "confidence": { "type": ["number", "null"], "minimum": 0.0, "maximum": 1.0 },
                "durability": { "type": ["number", "null"], "minimum": 0.0, "maximum": 1.0 },
                "reusability": { "type": ["number", "null"], "minimum": 0.0, "maximum": 1.0 },
                "specificity": { "type": ["number", "null"], "minimum": 0.0, "maximum": 1.0 },
                "evidence_grounded": { "type": ["number", "null"], "minimum": 0.0, "maximum": 1.0 },
                "noise_risk": { "type": ["number", "null"], "minimum": 0.0, "maximum": 1.0 }
            },
            "required": ["decision", "reason", "confidence", "durability", "reusability", "specificity", "evidence_grounded", "noise_risk"]
        }),
    }
}

pub(super) fn action_from_update_decision(
    decision: &LlmUpdateDecision,
    fallback: candidate::ExtractionAction,
) -> candidate::ExtractionAction {
    match decision.operation {
        MemoryOperation::Add => candidate::ExtractionAction::new_candidate(),
        MemoryOperation::Update => {
            let Some(target_id) = decision.target_id.clone() else {
                return fallback;
            };
            let mut action = candidate::ExtractionAction::merge_into_existing(target_id, 0.85);
            action.reason = Some(decision.reason.clone());
            action
        }
        MemoryOperation::Supersede | MemoryOperation::Conflict | MemoryOperation::Noop => {
            let action_name = match decision.operation {
                MemoryOperation::Supersede => "supersede",
                MemoryOperation::Conflict => "conflict",
                MemoryOperation::Noop => "noop",
                MemoryOperation::Add | MemoryOperation::Update => unreachable!(),
            };
            candidate::ExtractionAction {
                action: action_name.to_string(),
                route: fallback.route,
                target_record: decision.target_id.clone(),
                compile_enabled: fallback.compile_enabled,
                record_id: decision.target_id.clone(),
                similarity: fallback.similarity,
                reason: Some(decision.reason.clone()),
                rationale: fallback.rationale,
            }
        }
    }
}

/// 发送提取请求到 LLM 并在必要时回退
pub(super) fn run_llm_extraction(
    project_root: &std::path::Path,
    paragraphs: &[CandidateParagraph],
    max_tokens: usize,
) -> Result<Vec<LlmExtractedKnowledge>, LlmExtractionError> {
    if paragraphs.is_empty() {
        return Ok(Vec::new());
    }

    let prompt = build_extraction_prompt(paragraphs);
    let cfg = provider::load_or_default_provider_config(project_root)
        .map_err(|e| LlmExtractionError::ProviderError(e.to_string()))?;

    let result = provider::call_provider(&cfg, &prompt, max_tokens);

    match result {
        Ok(output) => parse_extraction_output(&output).map_err(|e| {
            LlmExtractionError::ParseError(format!(
                "{e}: {}",
                &output.chars().take(200).collect::<String>()
            ))
        }),
        Err(e) => {
            // LLM 不可用时回退提示
            Err(LlmExtractionError::ProviderError(e.to_string()))
        }
    }
}

// ============================================================
// 错误类型
// ============================================================

#[derive(Debug, Clone)]
pub(super) enum ParseError {
    EmptyOutput,
    InvalidJson(String),
    MissingRequired(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::EmptyOutput => write!(f, "LLM returned empty output"),
            ParseError::InvalidJson(preview) => {
                write!(f, "failed to parse LLM JSON output: {preview}...")
            }
            ParseError::MissingRequired(message) => {
                write!(f, "missing required LLM field: {message}")
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(super) enum LlmExtractionError {
    ProviderError(String),
    ParseError(String),
}

impl std::fmt::Display for LlmExtractionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LlmExtractionError::ProviderError(msg) => write!(f, "LLM provider error: {msg}"),
            LlmExtractionError::ParseError(msg) => write!(f, "LLM parse error: {msg}"),
        }
    }
}

// ============================================================
// 辅助函数
// ============================================================

/// 从 LLM 提取的知识项中筛选出可用的部分
pub(super) fn filter_usable_knowledge(
    items: Vec<LlmExtractedKnowledge>,
    min_confidence: f32,
) -> Vec<LlmExtractedKnowledge> {
    items
        .into_iter()
        .filter(|item| {
            !item.is_noise
                && item.confidence >= min_confidence
                && !item.title.trim().is_empty()
                && !item.body.trim().is_empty()
                && item.body.len() <= 400
        })
        .collect()
}

/// 将 KnowledgeKind 转换为 kind 字符串
pub(super) fn knowledge_kind_to_str(kind: &KnowledgeKind) -> &'static str {
    match kind {
        KnowledgeKind::Preference => "preference",
        KnowledgeKind::Constraint => "constraint",
        KnowledgeKind::Procedure => "procedure",
        KnowledgeKind::Correction => "correction",
        KnowledgeKind::Decision => "procedure", // 架构决策归入 procedure
        KnowledgeKind::Supplement => "procedure", // 技能补充归入 procedure
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_output_returns_error() {
        let result = parse_extraction_output("");
        assert!(result.is_err());
    }

    #[test]
    fn parse_valid_json_array() {
        let json = r#"[
            {
                "kind": "preference",
                "title": "Use Axios",
                "body": "Use Axios for frontend HTTP requests.",
                "confidence": 0.92,
                "rationale": "Repeated preference detected.",
                "evidence_quote": "Use Axios",
                "language": "en",
                "memory_tier_guess": "project_rule",
                "reusability_score": 0.4,
                "durability_score": 0.8,
                "specificity_score": 0.7,
                "source_trust_score": 1.0,
                "is_noise": false
            }
        ]"#;

        let items = parse_extraction_output(json).expect("parse valid JSON");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Use Axios");
        assert_eq!(items[0].confidence, 0.92);
        assert!(!items[0].is_noise);
    }

    #[test]
    fn parse_json_with_text_wrapper() {
        let output = r#"Here are the extracted items:
[
    {
        "kind": "procedure",
        "title": "Run Clippy Before Push",
        "body": "Always run cargo clippy before pushing Rust changes.",
        "confidence": 0.88,
        "rationale": "Recurring workflow pattern.",
        "is_noise": false
    }
]
That concludes the extraction."#;

        let error = parse_extraction_output(output).expect_err("strict parse rejects wrapper");
        assert!(error.to_string().contains("failed to parse"));
    }

    #[test]
    fn parse_non_noise_item_requires_all_value_scores() {
        let json = r#"[
            {
                "kind": "preference",
                "title": "Incomplete",
                "body": "Use durable evidence for extraction changes.",
                "confidence": 0.92,
                "rationale": "Missing score fields.",
                "evidence_quote": "durable evidence",
                "language": "en",
                "memory_tier_guess": "collaboration_preference",
                "is_noise": false
            }
        ]"#;

        let error = parse_extraction_output(json).expect_err("missing scores should fail");

        assert!(error.to_string().contains("missing required"));
    }

    #[test]
    fn filter_removes_noise_and_low_confidence() {
        let items = vec![
            LlmExtractedKnowledge {
                kind: KnowledgeKind::Preference,
                title: "Good".into(),
                body: "Useful knowledge".into(),
                confidence: 0.9,
                rationale: "test".into(),
                evidence_quote: Some("Useful knowledge".into()),
                language: Some("en".into()),
                memory_tier_guess: Some("project_rule".into()),
                reusability_score: Some(0.4),
                durability_score: Some(0.8),
                specificity_score: Some(0.7),
                source_trust_score: Some(1.0),
                is_noise: false,
            },
            LlmExtractedKnowledge {
                kind: KnowledgeKind::Preference,
                title: "Noise".into(),
                body: "noisy content".into(),
                confidence: 0.5,
                rationale: "test".into(),
                evidence_quote: None,
                language: None,
                memory_tier_guess: None,
                reusability_score: None,
                durability_score: None,
                specificity_score: None,
                source_trust_score: None,
                is_noise: true,
            },
            LlmExtractedKnowledge {
                kind: KnowledgeKind::Preference,
                title: "Low".into(),
                body: "low confidence".into(),
                confidence: 0.6,
                rationale: "test".into(),
                evidence_quote: None,
                language: None,
                memory_tier_guess: None,
                reusability_score: None,
                durability_score: None,
                specificity_score: None,
                source_trust_score: None,
                is_noise: false,
            },
        ];

        let usable = filter_usable_knowledge(items, 0.7);
        assert_eq!(usable.len(), 1);
        assert_eq!(usable[0].title, "Good");
    }

    #[test]
    fn build_extraction_prompt_includes_signal_labels() {
        let paragraphs = vec![CandidateParagraph {
            text: "Always use Bun for JavaScript package management.".into(),
            signals: vec![SignalType::PreferenceDeclaration],
        }];

        let prompt = build_extraction_prompt(&paragraphs);
        assert!(prompt.user_prompt.contains("Always use Bun"));
        assert!(prompt.user_prompt.contains("偏好声明"));
        assert!(prompt.system_prompt.contains("可复用知识类型"));
        assert!(prompt.system_prompt.contains("atomic facts"));
        assert!(prompt.system_prompt.contains("evidence_quote"));
    }

    #[test]
    fn extraction_schema_requires_multidimensional_value_scores() {
        let schema = extraction_schema().schema;
        let required = schema["items"]["required"]
            .as_array()
            .expect("required fields");

        for field in [
            "durability_score",
            "specificity_score",
            "source_trust_score",
        ] {
            assert!(
                required.iter().any(|value| value.as_str() == Some(field)),
                "{field} should be required in structured LLM extraction"
            );
            assert!(
                schema["items"]["properties"].get(field).is_some(),
                "{field} should have a schema property"
            );
        }
    }

    #[test]
    fn quality_judge_schema_exposes_rubric_scores() {
        let schema = quality_judge_schema().schema;
        let required = schema["required"].as_array().expect("required fields");

        for field in [
            "durability",
            "reusability",
            "specificity",
            "evidence_grounded",
            "noise_risk",
        ] {
            assert!(
                required.iter().any(|value| value.as_str() == Some(field)),
                "{field} should be required in quality judge schema"
            );
            assert!(
                schema["properties"].get(field).is_some(),
                "{field} should have a schema property"
            );
        }
    }

    #[test]
    fn parse_update_decision_output_supports_five_operations() {
        let output = r#"{
            "operation": "supersede",
            "target_id": "project:prefer-bun",
            "body": "Prefer pnpm for workspace package management.",
            "reason": "User reversed the previous package manager preference."
        }"#;

        let decision = parse_update_decision_output(output).expect("parse decision");

        assert_eq!(decision.operation, MemoryOperation::Supersede);
        assert_eq!(decision.target_id.as_deref(), Some("project:prefer-bun"));
        assert!(decision.reason.contains("reversed"));
    }

    #[test]
    fn parse_quality_judgment_output_supports_binary_judge() {
        let output = r#"{
            "decision": "reject",
            "reason": "This is an unresolved one-off request.",
            "confidence": 0.91
        }"#;

        let judgment = parse_quality_judgment_output(output).expect("parse judgment");

        assert_eq!(judgment.decision, JudgeDecision::Reject);
        assert!(judgment.reason.contains("unresolved"));
        assert_eq!(judgment.confidence, Some(0.91));
    }

    #[test]
    fn update_decision_maps_to_supersede_action() {
        let decision = LlmUpdateDecision {
            operation: MemoryOperation::Supersede,
            target_id: Some("project:prefer-bun".to_string()),
            body: Some("Prefer pnpm for workspace package management.".to_string()),
            reason: "User replaced the previous package manager preference.".to_string(),
        };

        let action = action_from_update_decision(
            &decision,
            crate::candidate::ExtractionAction::new_candidate(),
        );

        assert_eq!(action.action, "supersede");
        assert_eq!(action.target_record.as_deref(), Some("project:prefer-bun"));
        assert!(
            action
                .reason
                .as_deref()
                .unwrap_or_default()
                .contains("replaced")
        );
    }

    #[test]
    fn knowledge_kind_converts_to_valid_kind_string() {
        assert_eq!(
            knowledge_kind_to_str(&KnowledgeKind::Preference),
            "preference"
        );
        assert_eq!(
            knowledge_kind_to_str(&KnowledgeKind::Constraint),
            "constraint"
        );
        assert_eq!(
            knowledge_kind_to_str(&KnowledgeKind::Procedure),
            "procedure"
        );
        assert_eq!(
            knowledge_kind_to_str(&KnowledgeKind::Correction),
            "correction"
        );
        assert_eq!(knowledge_kind_to_str(&KnowledgeKind::Decision), "procedure");
        assert_eq!(
            knowledge_kind_to_str(&KnowledgeKind::Supplement),
            "procedure"
        );
    }
}
