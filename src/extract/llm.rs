//! LLM 驱动提取引擎：将候选段落发送给 LLM，提取结构化知识。
//!
//! 负责 Prompt 构建和 JSON 解析，实际的 HTTP 调用通过 provider 模块。

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
    /// 是否标记为噪音
    #[serde(default)]
    pub is_noise: bool,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum MemoryOperation {
    Add,
    Update,
    Supersede,
    Conflict,
    Noop,
}

#[cfg_attr(not(test), allow(dead_code))]
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

请分析以下被标记为"可能有价值"的对话段落。先提取 atomic facts，再判断是否值得进入记忆系统。对每个段落：
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
- rationale 必须说明 evidence_quote：引用原文中支持该结论的短句，不要输出 observation id、sha256 或内部评分字段
- title 跟随原文语言（中文原文用中文标题，英文原文用英文标题）
- 只提取对以后任务有用的内容，忽略所有一次性指令

Few-shot:
输入："以后 HTTP 默认用 Axios，但上传大文件保留 fetch，因为需要 ReadableStream。"
输出两条 atomic facts：
1. Use Axios for default HTTP requests. evidence_quote="HTTP 默认用 Axios"
2. Keep fetch for large uploads that require ReadableStream. evidence_quote="上传大文件保留 fetch"

Update phase schema:
后续会把每条 atomic fact 与相似 Skilllet 比较，并要求你只返回 ADD / UPDATE / SUPERSEDE / CONFLICT / NOOP 之一。"#
            .to_string(),
        user_prompt: format!("待分析的对话段落：\n{material}"),
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

    // 先尝试直接解析 JSON 数组
    if let Ok(knowledge) = serde_json::from_str::<Vec<LlmExtractedKnowledge>>(trimmed) {
        return Ok(knowledge);
    }

    // 尝试找到 JSON 数组的起止位置（处理 LLM 输出中可能包含的前后文字）
    if let Some(start) = trimmed.find('[')
        && let Some(end) = trimmed.rfind(']')
    {
        let json_slice = &trimmed[start..=end];
        if let Ok(knowledge) = serde_json::from_str::<Vec<LlmExtractedKnowledge>>(json_slice) {
            return Ok(knowledge);
        }
    }

    // 尝试按行解析（不同 LLM 可能输出不同格式）
    if let Some(start) = trimmed.find('{')
        && let Some(end) = trimmed.rfind('}')
    {
        let json_slice = &trimmed[start..=end];
        if let Ok(item) = serde_json::from_str::<LlmExtractedKnowledge>(json_slice) {
            return Ok(vec![item]);
        }
    }

    Err(ParseError::InvalidJson(trimmed.chars().take(200).collect()))
}

#[allow(dead_code)]
pub(super) fn build_update_decision_prompt(
    atomic_fact: &str,
    similar_skilllets: &[(String, String)],
) -> provider::ProviderRequest {
    let mut similar = String::new();
    for (id, body) in similar_skilllets {
        similar.push_str(&format!("- id: {id}\n  body: {body}\n"));
    }
    provider::ProviderRequest {
        system_prompt:
            r#"You decide how a newly extracted atomic memory should affect existing Skilllets.

Return only JSON with:
- operation: add | update | supersede | conflict | noop
- target_id: existing Skilllet id when applicable
- body: revised reusable rule when applicable
- reason: concise explanation

Use add for genuinely new durable knowledge.
Use update for clearer wording of the same rule.
Use supersede when the user reverses or replaces an old rule.
Use conflict when both rules may be valid but need human review.
Use noop when the fact is duplicate, low value, or already implied."#
                .to_string(),
        user_prompt: format!(
            "Atomic fact:\n{atomic_fact}\n\nSimilar Skilllets:\n{}",
            if similar.is_empty() {
                "(none)".to_string()
            } else {
                similar
            }
        ),
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn parse_update_decision_output(output: &str) -> Result<LlmUpdateDecision, ParseError> {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return Err(ParseError::EmptyOutput);
    }
    if let Ok(decision) = serde_json::from_str::<LlmUpdateDecision>(trimmed) {
        return Ok(decision);
    }
    if let Some(start) = trimmed.find('{')
        && let Some(end) = trimmed.rfind('}')
    {
        let json_slice = &trimmed[start..=end];
        if let Ok(decision) = serde_json::from_str::<LlmUpdateDecision>(json_slice) {
            return Ok(decision);
        }
    }
    Err(ParseError::InvalidJson(trimmed.chars().take(200).collect()))
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
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::EmptyOutput => write!(f, "LLM returned empty output"),
            ParseError::InvalidJson(preview) => {
                write!(f, "failed to parse LLM JSON output: {preview}...")
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

        let items = parse_extraction_output(output).expect("parse with wrapper");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, KnowledgeKind::Procedure);
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
                is_noise: false,
            },
            LlmExtractedKnowledge {
                kind: KnowledgeKind::Preference,
                title: "Noise".into(),
                body: "noisy content".into(),
                confidence: 0.5,
                rationale: "test".into(),
                is_noise: true,
            },
            LlmExtractedKnowledge {
                kind: KnowledgeKind::Preference,
                title: "Low".into(),
                body: "low confidence".into(),
                confidence: 0.6,
                rationale: "test".into(),
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
