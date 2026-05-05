use crate::candidate;
use crate::textutil;

use super::{Candidate, chunk, classify, scoring, signals};

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
        evidence: sentence.to_string(),
        confidence: Some(0.84),
        reason: Some(
            "Matched high-value prompt pattern: reusable agent handoff or project-improving workflow."
                .to_string(),
        ),
        matched_template: Some("high-value-prompt".to_string()),
    })
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

    let body = normalize_body(sentence);
    if body.len() < 12 || body.len() > 400 {
        return None;
    }

    Some(Candidate {
        title: title_from_scored_signal(&body, &score.matched_signal),
        body,
        kind: kind_from_scored_signal(&score.matched_signal).to_string(),
        scope: infer_scope(sentence).to_string(),
        evidence: sentence.to_string(),
        confidence: Some((score.score / 5.0).clamp(0.7, 0.94)),
        reason: Some(score.reason),
        matched_template: Some(format!("score:{}", score.matched_signal)),
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
    format!("project:{slug}")
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
    ];
    markers.iter().any(|marker| lower.contains(marker))
}

pub(super) fn looks_like_skilllet_signal(sentence: &str) -> bool {
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
        "skilllet",
        "claude",
        "codex",
        "cursor",
        "bun",
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
        "skilllet",
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

pub(super) fn title_from_body(body: &str) -> String {
    let words = body.split_whitespace().collect::<Vec<_>>();
    if words.len() >= 3 {
        return words.iter().take(6).copied().collect::<Vec<_>>().join(" ");
    }
    body.chars().take(24).collect()
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
