use crate::candidate::{self, EvidenceSpan};
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
