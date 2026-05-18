use std::collections::BTreeMap;
use std::path::Path;

use crate::candidate::MemoryTier;
use crate::provider::{self, ProviderJsonSchema, ProviderRequest};

use super::Candidate;
use super::candidate_factory::title_from_body;

#[derive(Debug, serde::Deserialize)]
struct RefineBatch {
    items: Vec<RefineItem>,
}

#[derive(Debug, serde::Deserialize)]
struct RefineItem {
    index: usize,
    decision: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    memory_tier: Option<String>,
    #[serde(default)]
    confidence: Option<f32>,
    #[serde(default)]
    reason: String,
}

pub(super) fn refine_candidates(
    project_root: &Path,
    candidates: &[Candidate],
    allow_provider_refine: bool,
) -> (Vec<Option<Candidate>>, Vec<String>) {
    if candidates.is_empty() {
        return (Vec::new(), Vec::new());
    }

    let mut messages = Vec::new();
    if allow_provider_refine {
        match refine_with_provider(project_root, candidates) {
            Ok(Some(refined)) => return (refined, messages),
            Ok(None) => messages.push(
                "LLM final rewrite skipped: refine provider unavailable; falling back to deterministic baseline (no body rewrite)."
                    .to_string(),
            ),
            Err(error) => messages.push(format!(
                "LLM final rewrite failed: {error}; falling back to deterministic baseline (no body rewrite)."
            )),
        }
    }

    let refined = candidates
        .iter()
        .map(|candidate| refine_deterministic(candidate, &mut messages))
        .collect::<Vec<_>>();
    (refined, messages)
}

pub(super) fn refine_candidates_with_llm_required(
    project_root: &Path,
    candidates: &[Candidate],
) -> (Vec<Option<Candidate>>, Vec<String>) {
    if candidates.is_empty() {
        return (Vec::new(), Vec::new());
    }

    match refine_with_provider(project_root, candidates) {
        Ok(Some(refined)) => (refined, Vec::new()),
        Ok(None) => (
            vec![None; candidates.len()],
            vec![
                "LLM final rewrite failed: refine provider unavailable; suppressing unrefined candidates."
                    .to_string(),
            ],
        ),
        Err(error) => (
            vec![None; candidates.len()],
            vec![format!(
                "LLM final rewrite failed: {error}; suppressing unrefined candidates."
            )],
        ),
    }
}

fn refine_with_provider(
    project_root: &Path,
    candidates: &[Candidate],
) -> anyhow::Result<Option<Vec<Option<Candidate>>>> {
    let cfg = provider::load_or_default_provider_config(project_root)?;
    let request = build_refine_prompt(candidates);
    let output = match provider::call_provider_for_role(
        &cfg,
        provider::ProviderRole::Refine,
        &request,
        4096,
    ) {
        Ok(output) => output,
        Err(_) => return Ok(None),
    };
    let parsed: RefineBatch = serde_json::from_str(output.trim())?;
    let mut by_index = BTreeMap::new();
    for item in parsed.items {
        by_index.insert(item.index, item);
    }

    let mut out = Vec::new();
    for (index, candidate) in candidates.iter().enumerate() {
        let Some(item) = by_index.remove(&index) else {
            out.push(Some(candidate.clone()));
            continue;
        };
        if item.decision == "reject" {
            out.push(None);
            continue;
        }
        let mut refined = candidate.clone();
        if let Some(body) = item.body.map(|body| body.trim().to_string())
            && is_valid_refined_body(&body)
        {
            refined.body = body;
        }
        refined.title = item
            .title
            .filter(|title| !title.trim().is_empty())
            .unwrap_or_else(|| title_from_body(&refined.body));
        if let Some(kind) = item.kind.filter(|kind| is_known_kind(kind)) {
            refined.kind = kind;
        }
        if let Some(memory_tier) = item.memory_tier.as_deref().and_then(memory_tier_from_str) {
            refined.memory_tier = memory_tier;
            refined.scope = if refined.memory_tier == MemoryTier::ProjectRule {
                "project".to_string()
            } else {
                "global".to_string()
            };
        }
        if let Some(confidence) = item.confidence {
            refined.confidence = Some(confidence.clamp(0.0, 1.0));
        }
        refined.reason = Some(if item.reason.trim().is_empty() {
            "Refined final candidate into condition-action agent memory.".to_string()
        } else {
            item.reason
        });
        refined.matched_template = Some("llm-memory-refine".to_string());
        out.push(Some(refined));
    }
    Ok(Some(out))
}

fn build_refine_prompt(candidates: &[Candidate]) -> ProviderRequest {
    let items = candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| {
            serde_json::json!({
                "index": index,
                "title": candidate.title,
                "body": candidate.body,
                "kind": candidate.kind,
                "scope": candidate.scope,
                "memory_tier": candidate.memory_tier.as_str(),
                "evidence": candidate.evidence,
            })
        })
        .collect::<Vec<_>>();
    ProviderRequest {
        system_prompt: r#"你是 Agent 长期记忆的最终质检与改写器。

目标：把已经筛选出的候选转写成高质量 agent memory/memory_card，或剔除低质量候选。

高质量 memory 必须包含：
- When：什么场景触发
- What/How：Agent 应该怎么做
- Why/Boundary：目标、边界或禁止越界点

规则：
- 如果候选只是一次性任务、半截句、内部 ID、列表标题或无长期价值，decision=reject
- 如果候选有长期价值，decision=keep，并把 body 改写成“当……时，……；目标是……”这类可执行规则
- body 必须是最终可审阅 MemoryCard 格式：当/When <触发场景>，<agent 动作>；目标/边界是 <原因、限制或人工确认点>
- 不要只复述事实；必须写成未来 agent 能执行的规则
- 不要保留 project:/global: 这类候选 ID 前缀
- 涉及 MemoryCard、候选规则固化、review/merge 边界的治理规则，默认 memory_tier=project_rule，除非证据明确说适用于所有项目
- 不要编造原文没有支持的工具、路径、数字 SLA
- 中文原文默认输出中文
- assistant synthesis 相关内容必须保留“先确认用户接受/真实历史证据/人工审阅边界”
- 明显一次性边界如“这次只读”“不要写文件”“只改 CSS”必须 reject
- 只返回 JSON"#
            .to_string(),
        user_prompt: serde_json::json!({ "candidates": items }).to_string(),
        json_schema: Some(refine_schema()),
    }
}

fn refine_schema() -> ProviderJsonSchema {
    ProviderJsonSchema {
        name: "FinalMemoryRefinement".to_string(),
        strict: true,
        schema: serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "items": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "index": { "type": "integer" },
                            "decision": { "type": "string", "enum": ["keep", "reject"] },
                            "title": { "type": ["string", "null"] },
                            "body": { "type": ["string", "null"] },
                            "kind": { "type": ["string", "null"], "enum": ["preference", "constraint", "procedure", "correction", "decision", "supplement", null] },
                            "memory_tier": { "type": ["string", "null"], "enum": ["project_rule", "cross_project_principle", "collaboration_preference", null] },
                            "confidence": { "type": ["number", "null"] },
                            "reason": { "type": "string" }
                        },
                        "required": ["index", "decision", "title", "body", "kind", "memory_tier", "confidence", "reason"]
                    }
                }
            },
            "required": ["items"]
        }),
    }
}

/// LLM provider 不可用时的最小确定性回退路径。
///
/// 不再尝试用关键词匹配把 candidate body 改写成"模板话"。命中低质量残留则拒绝；
/// 通过基础长度校验则原样保留。最终的 When/What/Why 改写必须由 LLM 完成；
/// 如果 provider 不可用，外层应明确告知用户而不是产出虚假高置信度卡。
fn refine_deterministic(candidate: &Candidate, messages: &mut Vec<String>) -> Option<Candidate> {
    let lower = candidate.body.to_lowercase();
    if looks_like_low_quality_leftover(&lower) {
        messages.push(format!(
            "{}: refine-reject (low-quality leftover)",
            super::candidate_factory::draft_id(candidate)
        ));
        return None;
    }
    if !is_valid_refined_body(&candidate.body) {
        return None;
    }
    Some(candidate.clone())
}

fn looks_like_low_quality_leftover(lower: &str) -> bool {
    lower.contains("cross-project-principl")
        || lower.contains("global:")
        || lower.contains("project:")
        || lower.ends_with("-质")
}

fn is_valid_refined_body(body: &str) -> bool {
    let body = body.trim();
    body.chars().count() >= 16
        && body.chars().count() <= 320
        && !body.to_lowercase().contains("cross-project-principl")
}

fn memory_tier_from_str(value: &str) -> Option<MemoryTier> {
    match value {
        "project_rule" => Some(MemoryTier::ProjectRule),
        "cross_project_principle" => Some(MemoryTier::CrossProjectPrinciple),
        "collaboration_preference" => Some(MemoryTier::CollaborationPreference),
        _ => None,
    }
}

fn is_known_kind(kind: &str) -> bool {
    matches!(
        kind,
        "preference" | "constraint" | "procedure" | "correction" | "decision" | "supplement"
    )
}
