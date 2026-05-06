use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::chunk::EvidenceChunk;
use super::classify::{KnowledgeClassification, classify_chunk};
use super::gate::future_value_gate;

/// 评分处置结果：拒绝、边界或候选。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExtractionDisposition {
    Reject,
    Borderline,
    Candidate,
}

/// 提取评分：包含总分、处置、信号类型、理由和评分明细。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionScore {
    pub score: f32,
    pub disposition: ExtractionDisposition,
    pub matched_signal: String,
    pub reason: String,
    pub breakdown: BTreeMap<String, f32>,
}

/// 对单个证据块执行确定性评分，返回评分结果。
/// 基于 classify_chunk 的分类结果，按计划维度打分。
pub fn score_chunk(chunk: &EvidenceChunk) -> ExtractionScore {
    let mut breakdown = BTreeMap::new();
    let gate = future_value_gate(chunk);
    if gate.disposition == "reject" {
        return ExtractionScore {
            score: 0.0,
            disposition: ExtractionDisposition::Reject,
            matched_signal: String::new(),
            reason: gate.reason,
            breakdown,
        };
    }

    let classification = classify_chunk(chunk);

    // 噪音或空信号直接拒绝
    if classification.artifact_kind == "reject" || classification.signal.is_empty() {
        return ExtractionScore {
            score: 0.0,
            disposition: ExtractionDisposition::Reject,
            matched_signal: classification.signal,
            reason: classification.rationale,
            breakdown,
        };
    }

    let matched_signal = classification.signal.clone();
    let text = chunk.text.trim();
    let lower = text.to_ascii_lowercase();

    // 计划维度：正向分数
    add(
        &mut breakdown,
        "future_agent_value",
        future_agent_value(&classification),
    );
    add(
        &mut breakdown,
        "grounding",
        grounding_score(&classification),
    );
    add(
        &mut breakdown,
        "actionability",
        actionability_score(&classification),
    );
    add(
        &mut breakdown,
        "trigger_clarity",
        trigger_clarity(&classification),
    );
    add(&mut breakdown, "specificity", specificity_score(text));
    add(
        &mut breakdown,
        "recurrence_or_correction",
        recurrence_or_correction(&classification, &lower),
    );
    add(
        &mut breakdown,
        "validation_value",
        validation_value(&classification, &lower),
    );
    add(
        &mut breakdown,
        "fragility_value",
        fragility_value(&classification, &lower),
    );
    add(&mut breakdown, "novelty", novelty_score(&classification));

    // 计划维度：惩罚项
    add(&mut breakdown, "one_off_penalty", -one_off_penalty(&lower));
    add(
        &mut breakdown,
        "unresolved_penalty",
        -unresolved_penalty(&lower),
    );
    add(
        &mut breakdown,
        "generic_advice_penalty",
        -generic_advice_penalty(&lower),
    );
    add(
        &mut breakdown,
        "transcript_noise_penalty",
        -transcript_noise_penalty(&lower),
    );
    add(
        &mut breakdown,
        "prompt_leak_penalty",
        -prompt_leak_penalty(&lower),
    );
    add(
        &mut breakdown,
        "sensitive_content_penalty",
        -sensitive_content_penalty(&lower),
    );

    let score = breakdown.values().sum::<f32>();
    let disposition = if score >= 3.2 && !matched_signal.is_empty() {
        ExtractionDisposition::Candidate
    } else if score >= 2.4 && !matched_signal.is_empty() {
        ExtractionDisposition::Borderline
    } else {
        ExtractionDisposition::Reject
    };

    let reason = match &disposition {
        ExtractionDisposition::Candidate => {
            format!("High-value durable {matched_signal} signal with reusable project impact.")
        }
        ExtractionDisposition::Borderline => {
            format!("Borderline {matched_signal} signal; keep for optional provider review.")
        }
        ExtractionDisposition::Reject => classification.rationale,
    };

    ExtractionScore {
        score,
        disposition,
        matched_signal,
        reason,
        breakdown,
    }
}

// ============================================================
// 评分辅助函数
// ============================================================

fn add(map: &mut BTreeMap<String, f32>, key: &str, value: f32) {
    map.insert(key.to_string(), value);
}

/// 未来智能体价值：不同信号类型对后续 agent 的可复用程度。
fn future_agent_value(classification: &KnowledgeClassification) -> f32 {
    match classification.signal.as_str() {
        "constraint" | "correction" => 1.4,
        "decision" | "ai_project_improvement" => 1.2,
        "procedure" | "validation" => 1.0,
        "preference" | "template" => 0.8,
        _ => 0.2,
    }
}

/// 接地程度：知识的具体/可执行程度，基于硬度等级。
fn grounding_score(classification: &KnowledgeClassification) -> f32 {
    match classification.hardness.as_str() {
        "critical" => 0.8,
        "high" => 0.6,
        "medium" => 0.4,
        _ => 0.2,
    }
}

/// 可操作性：基于控制类型判断 agent 能否直接执行。
fn actionability_score(classification: &KnowledgeClassification) -> f32 {
    match classification.control.as_str() {
        "exact_sequence" | "checklist" | "prohibition" => 0.8,
        "default" => 0.5,
        "principle" => 0.3,
        _ => 0.2,
    }
}

/// 触发清晰度：基于激活方式的明确程度。
fn trigger_clarity(classification: &KnowledgeClassification) -> f32 {
    match classification.activation.as_str() {
        "always_on" => 1.0,
        "skill" => 0.7,
        "manual" => 0.4,
        _ => 0.3,
    }
}

/// 特异性：文本中包含具体项目名词/技术术语的得分。
fn specificity_score(text: &str) -> f32 {
    let named_terms = [
        "AGENTS",
        "CLAUDE",
        "Claude Code",
        "Codex",
        "Draft",
        "Skilllet",
        "cargo",
        "Rust",
        "Bun",
        "EvidenceChunk",
        "Axios",
        "HTTP",
        "fetch",
        "ky",
        "ofetch",
        "JavaScript",
        "npm",
        "pnpm",
    ];
    let lower = text.to_ascii_lowercase();
    if named_terms
        .iter()
        .any(|t| lower.contains(&t.to_ascii_lowercase()))
    {
        0.8
    } else {
        0.2
    }
}

/// 重复性/纠错价值：对反复出现的问题或纠错信号的加分。
fn recurrence_or_correction(classification: &KnowledgeClassification, lower: &str) -> f32 {
    if classification.signal == "correction" {
        1.2
    } else if lower.contains("又")
        || lower.contains("again")
        || lower.contains("repeated")
        || lower.contains("每次")
        || lower.matches("以后").count() >= 2
    {
        0.6
    } else {
        0.0
    }
}

/// 验证价值：包含测试/验证相关信号的加分。
fn validation_value(classification: &KnowledgeClassification, lower: &str) -> f32 {
    if classification.signal == "validation" {
        0.8
    } else if lower.contains("cargo test") || lower.contains("clippy") || lower.contains("fixture")
    {
        0.5
    } else {
        0.0
    }
}

/// 脆弱性价值：防止易错模式或危险操作的加分。
fn fragility_value(classification: &KnowledgeClassification, lower: &str) -> f32 {
    if classification.signal == "constraint" && lower.contains("不要") {
        0.4
    } else if lower.contains("禁止")
        || lower.contains("never")
        || lower.contains("do not")
        || lower.contains("must not")
    {
        0.3
    } else {
        0.0
    }
}

/// 新颖性：首次出现或新引入的知识的加分。
fn novelty_score(classification: &KnowledgeClassification) -> f32 {
    match classification.signal.as_str() {
        "ai_project_improvement" | "decision" => 0.5,
        "correction" | "validation" => 0.3,
        _ => 0.1,
    }
}

/// 一次性任务惩罚：文本看起来像一次性任务请求。
fn one_off_penalty(lower: &str) -> f32 {
    if lower.contains("这个按钮") || lower.contains("改成蓝色") || lower.contains("帮我看看")
    {
        2.0
    } else {
        0.0
    }
}

/// 未解决请求惩罚：包含不确定/未定语言。
fn unresolved_penalty(lower: &str) -> f32 {
    if lower.contains("能不能") || lower.contains("maybe") || lower.contains("先想想") {
        1.5
    } else {
        0.0
    }
}

/// 泛泛建议惩罚：通用、无项目特定内容的建议。
fn generic_advice_penalty(lower: &str) -> f32 {
    if lower.contains("代码整洁") && lower.contains("文档完善") {
        2.0
    } else {
        0.0
    }
}

/// 对话噪音惩罚：shell 输出、错误栈等无知识价值的文本。
fn transcript_noise_penalty(lower: &str) -> f32 {
    if lower.contains("error[") || lower.contains("--> src/") {
        1.5
    } else {
        0.0
    }
}

/// Prompt 泄露惩罚：检测意外的 prompt 注入或泄露。
fn prompt_leak_penalty(_lower: &str) -> f32 {
    0.0
}

/// 敏感内容惩罚：包含 token/密钥等不应被提取的内容。
fn sensitive_content_penalty(lower: &str) -> f32 {
    if lower.contains("token=") || lower.contains("secret") || lower.contains("credential") {
        2.0
    } else {
        0.0
    }
}
