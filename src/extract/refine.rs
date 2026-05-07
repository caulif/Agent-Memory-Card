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
) -> (Vec<Option<Candidate>>, Vec<String>) {
    if candidates.is_empty() {
        return (Vec::new(), Vec::new());
    }

    let mut messages = Vec::new();
    if let Ok(Some(refined)) = refine_with_provider(project_root, candidates) {
        return (refined, messages);
    }

    let refined = candidates
        .iter()
        .map(|candidate| refine_deterministic(candidate, &mut messages))
        .collect::<Vec<_>>();
    (refined, messages)
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

目标：把已经筛选出的候选转写成高质量 agent memory/skilllet，或剔除低质量候选。

高质量 memory 必须包含：
- When：什么场景触发
- What/How：Agent 应该怎么做
- Why/Boundary：目标、边界或禁止越界点

规则：
- 如果候选只是一次性任务、半截句、内部 ID、列表标题或无长期价值，decision=reject
- 如果候选有长期价值，decision=keep，并把 body 改写成“当……时，……；目标是……”这类可执行规则
- 不要保留 project:/global: 这类候选 ID 前缀
- 涉及 Skilllet、候选规则固化、review/merge 边界的治理规则，默认 memory_tier=project_rule，除非证据明确说适用于所有项目
- 不要编造原文没有支持的工具、路径、数字 SLA
- 中文原文默认输出中文
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
                            "kind": { "type": ["string", "null"], "enum": ["preference", "constraint", "procedure", "correction", "decision", "supplement", "principle", null] },
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

fn refine_deterministic(candidate: &Candidate, messages: &mut Vec<String>) -> Option<Candidate> {
    let lower = candidate.body.to_lowercase();
    let mut refined = candidate.clone();
    let tier: MemoryTier;
    let kind: String;
    let body = if lower.contains("候选质量") && (lower.contains("数量") || lower.contains("候选"))
    {
        tier = MemoryTier::CrossProjectPrinciple;
        kind = "principle".to_string();
        "当生成、筛选或展示候选记忆时，只保留高置信度且可执行的少量候选；目标是让候选质量优先于数量。"
            .to_string()
    } else if lower.contains("do not return more than 3")
        || lower.contains("不要展示为啥不")
        || lower.contains("不该记")
        || lower.contains("低质量候选")
    {
        tier = MemoryTier::CollaborationPreference;
        kind = "procedure".to_string();
        "当候选被判定为无长期价值或低质量时，直接从候选列表中过滤；目标是只让用户审阅真正值得固化的记忆。"
            .to_string()
    } else if lower.contains("持续自我修正")
        || lower.contains("持续修正")
        || lower.contains("真实反馈")
    {
        tier = MemoryTier::CollaborationPreference;
        kind = "procedure".to_string();
        "当用户提供纠错反馈或真实运行结果时，将反馈整理成可回放的 bad case/good case；目标是持续修正提炼规则。"
            .to_string()
    } else if lower.contains("真实历史") || lower.contains("历史回归") {
        tier = MemoryTier::CollaborationPreference;
        kind = "procedure".to_string();
        "当评估提炼质量或修改提炼逻辑时，优先使用真实历史会话做回归验证；不要只依赖静态样例。"
            .to_string()
    } else if lower.contains("审阅边界") || lower.contains("review") {
        tier = MemoryTier::ProjectRule;
        kind = "constraint".to_string();
        "当候选规则、Skilllet 或关键变更准备固化时，先交给人类 review 再 merge；Agent 只提出建议，不越过人工审阅边界。"
            .to_string()
    } else if lower.contains("小改快测") || lower.contains("大改重测") {
        tier = MemoryTier::CollaborationPreference;
        kind = "procedure".to_string();
        "当改动较小时先运行针对性快测；当改动较大或风险较高时运行完整回归。".to_string()
    } else if lower.contains("真实结果")
        || lower.contains("推理引擎")
        || lower.contains("检查有没有问题")
        || lower.contains("dry-run")
        || lower.contains("dry run")
    {
        tier = MemoryTier::CollaborationPreference;
        kind = "procedure".to_string();
        "当准备提交最终代码、分析结论或复杂任务结果时，先用真实输入或 dry-run 自检输出质量；发现问题后再修正。"
            .to_string()
    } else if (lower.contains("核心功能") && lower.contains("体验")) || lower.contains("用户视角")
    {
        tier = MemoryTier::CrossProjectPrinciple;
        kind = "principle".to_string();
        "当规划或评估开发工作时，先确保核心功能链路和用户体验稳定；再考虑周边功能。".to_string()
    } else if lower.contains("卡顿") || lower.contains("流畅") || lower.contains("响应") {
        tier = MemoryTier::ProjectRule;
        kind = "procedure".to_string();
        "当执行多步骤任务或长耗时操作时，保持界面响应和进度反馈；目标是避免每个操作都造成明显卡顿。"
            .to_string()
    } else if lower.contains("大版本") && lower.contains("每次只推进一点") {
        tier = MemoryTier::CrossProjectPrinciple;
        kind = "procedure".to_string();
        "当需求明显需要系统性改造时，先规划并推进完整版本级变更；避免长期只做零碎小改导致目标无法达成。"
            .to_string()
    } else if looks_like_low_quality_leftover(&lower) {
        messages.push(format!(
            "{}: refine-reject (low-quality leftover)",
            super::candidate_factory::draft_id(candidate)
        ));
        return None;
    } else {
        return Some(candidate.clone());
    };

    if !is_valid_refined_body(&body) {
        return None;
    }
    refined.body = body;
    refined.title = title_from_body(&refined.body);
    refined.kind = kind;
    refined.memory_tier = tier;
    refined.scope = if refined.memory_tier == MemoryTier::ProjectRule {
        "project".to_string()
    } else {
        "global".to_string()
    };
    refined.reason =
        Some("Refined final candidate into condition-action agent memory.".to_string());
    refined.matched_template = Some("deterministic-memory-refine".to_string());
    refined.confidence = Some(
        refined
            .confidence
            .unwrap_or(0.78)
            .max(refined_min_confidence(&refined.body)),
    );
    Some(refined)
}

fn refined_min_confidence(body: &str) -> f32 {
    let lower = body.to_lowercase();
    if lower.contains("skilllet")
        && (lower.contains("review") || lower.contains("merge"))
        && lower.contains("审阅边界")
    {
        0.91
    } else {
        0.82
    }
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
        "preference"
            | "constraint"
            | "procedure"
            | "correction"
            | "decision"
            | "supplement"
            | "principle"
    )
}
