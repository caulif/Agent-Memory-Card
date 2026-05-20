//! Pipeline Layer 4：INDUCE 归纳。

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::extract::cluster::MessageCluster;
use crate::provider::{
    ProviderConfig, ProviderJsonSchema, ProviderRequest, ProviderRole, call_provider_for_role,
};

/// Layer 4 输出：从一个 cluster 归纳出的 candidate。
///
/// 还没规范化为 MemoryCard，需要 Layer 5 CRYSTALLIZE。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InducedCandidate {
    /// 来自的 cluster_id
    pub cluster_id: String,

    /// 簇大小（recurrence 信号）
    pub recurrence: usize,

    /// 8-30 字简体中文动宾结构
    pub title: String,

    /// 触发场景
    pub when: String,

    /// 动作约束
    pub what: String,

    /// 目的或边界
    pub why: String,

    /// 适用边界或例外；新 prompt 会要求，旧 provider 输出可为空。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boundary: Option<String>,

    /// `preference` / `constraint` / `procedure`
    pub kind: String,

    /// `global` / `project`
    pub scope: String,

    /// 至少 1 条；每条 text 必须是簇内某条消息的字面子串
    pub evidence_quotes: Vec<EvidenceQuote>,

    /// `project_rule` / `cross_project_principle` / `collaboration_preference`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_tier: Option<String>,

    /// `too_low` / `good` / `too_high`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abstraction_level: Option<String>,

    /// `strong` / `medium` / `weak`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub support_level: Option<String>,

    /// `stable` / `reversed` / `refined`（V2 修订 5）
    pub temporal_status: String,

    /// 0.5-1.0；recurrence=1 时应 < 0.85
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvidenceQuote {
    pub observation_id: String,
    pub text: String,
}

/// 抽象 LLM 调用，便于测试 mock。
pub trait InduceProvider {
    fn call(&self, request: &ProviderRequest, max_tokens: usize) -> Result<String>;
}

/// 默认实现：走 [`crate::provider::call_provider_for_role`] 调真实 API。
pub struct DefaultInduceProvider<'a> {
    pub cfg: &'a ProviderConfig,
}

impl InduceProvider for DefaultInduceProvider<'_> {
    fn call(&self, request: &ProviderRequest, max_tokens: usize) -> Result<String> {
        call_provider_for_role(self.cfg, ProviderRole::Extract, request, max_tokens)
    }
}

/// Layer 4 主入口（默认 provider）。
pub fn induce_from_clusters(
    cfg: &ProviderConfig,
    clusters: &[MessageCluster],
) -> Vec<InductionOutcome> {
    let provider = DefaultInduceProvider { cfg };
    induce_with_provider(&provider, clusters)
}

/// 用任意 [`InduceProvider`] 实现归纳（测试 / 自定义路径用）。
pub fn induce_with_provider(
    provider: &dyn InduceProvider,
    clusters: &[MessageCluster],
) -> Vec<InductionOutcome> {
    clusters
        .iter()
        .map(|cluster| induce_one(provider, cluster))
        .collect()
}

pub fn induce_batch_with_provider(
    provider: &dyn InduceProvider,
    clusters: &[MessageCluster],
    max_cards: usize,
) -> Vec<InductionOutcome> {
    if clusters.is_empty() || max_cards == 0 {
        return Vec::new();
    }
    let request = build_batch_induce_request(clusters, max_cards);
    let raw = match provider.call(&request, 8192) {
        Ok(text) => text,
        Err(error) => {
            return vec![InductionOutcome::Failed {
                cluster_id: "batch".to_string(),
                error: error.to_string(),
            }];
        }
    };
    match parse_batch_induce_response(&raw, clusters) {
        Ok(candidates) => candidates
            .into_iter()
            .map(InductionOutcome::Accepted)
            .collect(),
        Err(error) => vec![InductionOutcome::Failed {
            cluster_id: "batch".to_string(),
            error: format!("parse: {error}"),
        }],
    }
}

/// 单簇归纳的结果（成功 / 拒绝 / 失败）。
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum InductionOutcome {
    Accepted(InducedCandidate),
    /// LLM 主动 reject（这个簇不构成规则）
    Rejected {
        cluster_id: String,
        reason: String,
    },
    /// 调用或解析失败（网络 / schema 不合法 / evidence 校验失败）
    Failed {
        cluster_id: String,
        error: String,
    },
}

fn induce_one(provider: &dyn InduceProvider, cluster: &MessageCluster) -> InductionOutcome {
    let request = build_induce_request(cluster);
    let raw = match provider.call(&request, 1200) {
        Ok(text) => text,
        Err(error) => {
            return InductionOutcome::Failed {
                cluster_id: cluster.cluster_id.clone(),
                error: error.to_string(),
            };
        }
    };
    match parse_induce_response(&raw, cluster) {
        Ok(Some(candidate)) => match validate_evidence_quotes(&candidate, cluster) {
            Ok(()) => InductionOutcome::Accepted(candidate),
            Err(error) => InductionOutcome::Failed {
                cluster_id: cluster.cluster_id.clone(),
                error: error.to_string(),
            },
        },
        Ok(None) => InductionOutcome::Rejected {
            cluster_id: cluster.cluster_id.clone(),
            reason: "LLM marked cluster as not a rule".to_string(),
        },
        Err(error) => InductionOutcome::Failed {
            cluster_id: cluster.cluster_id.clone(),
            error: format!("parse: {error}"),
        },
    }
}

const SYSTEM_PROMPT: &str = include_str!("../../prompts/induce.md");

/// 计算 SYSTEM_PROMPT 的 sha256 前 8 位作为 prompt 指纹。
///
/// 用于 layer_trace：当 prompt 被改动后，所有新卡的 `extraction.layer_trace`
/// 里 `induce` 层会带新指纹，可以做 A/B 对比与回归归因。
pub fn induce_prompt_hash() -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(SYSTEM_PROMPT.as_bytes());
    let digest = hasher.finalize();
    digest
        .iter()
        .take(4)
        .map(|b| format!("{:02x}", b))
        .collect::<String>()
}

/// 单簇 confidence 上限（recurrence=1 时 LLM 不应有 ≥ 0.85 的把握）。
pub const SINGLETON_CONFIDENCE_CEILING: f32 = 0.80;

/// 当 recurrence == 1 时把 confidence 强制压到 [`SINGLETON_CONFIDENCE_CEILING`] 以下。
///
/// 设计文档"单 quote 一次性请求被收录"的兜底：LLM 偶尔会对单条措辞强烈的消息
/// 给 0.9 分，但没有多重证据时这种高分不可信。返回值被写入 [`InducedCandidate`]，
/// 后续 review 流程看到低分就知道这是单证据卡。
pub fn clamp_singleton_confidence(recurrence: usize, confidence: f32) -> f32 {
    if recurrence <= 1 && confidence > SINGLETON_CONFIDENCE_CEILING {
        SINGLETON_CONFIDENCE_CEILING
    } else {
        confidence
    }
}

fn build_induce_request(cluster: &MessageCluster) -> ProviderRequest {
    let user_prompt = format!(
        "cluster_id: {}\nrecurrence: {}\nbackend: {}\nmean_similarity: {:.3}\n\nmessages (chronological order):\n{}",
        cluster.cluster_id,
        cluster.recurrence,
        cluster.backend,
        cluster.mean_similarity,
        cluster
            .messages
            .iter()
            .enumerate()
            .map(|(i, m)| format!(
                "\n--- [{}] obs={} created={} ---\n{}",
                i + 1,
                m.observation_id,
                m.created_at,
                m.body
            ))
            .collect::<Vec<_>>()
            .join("")
    );

    ProviderRequest {
        system_prompt: SYSTEM_PROMPT.to_string(),
        user_prompt,
        json_schema: Some(induce_schema()),
    }
}

fn build_batch_induce_request(clusters: &[MessageCluster], max_cards: usize) -> ProviderRequest {
    let clusters_text = clusters
        .iter()
        .enumerate()
        .map(|(index, cluster)| {
            format!(
                "\n=== cluster {} id={} recurrence={} backend={} mean_similarity={:.3} ===\n{}",
                index + 1,
                cluster.cluster_id,
                cluster.recurrence,
                cluster.backend,
                cluster.mean_similarity,
                cluster
                    .messages
                    .iter()
                    .enumerate()
                    .map(|(i, m)| format!(
                        "\n--- [{}] obs={} created={} ---\n{}",
                        i + 1,
                        m.observation_id,
                        m.created_at,
                        compact_batch_body(&m.body)
                    ))
                    .collect::<Vec<_>>()
                    .join("")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let user_prompt = format!(
        "You are seeing the highest-signal evidence clusters selected by the deterministic pipeline.\n\
Synthesize at most {max_cards} durable MemoryCard candidates across these clusters.\n\
Preserve lane diversity: include durable project workflow rules and global reusable rules when evidence supports them.\n\
Also include memory-governance, validation, review-boundary, and candidate-quality rules when evidence supports them.\n\
Merge duplicate concepts instead of emitting near-duplicates.\n\
Treat product details, numeric thresholds, UI entry points, tutorial copy, storage keys, enum names, and one-off feature settings as evidence context, not Memory Cards.\n\
If a project-specific workflow rule is present, prefer a concrete project-scoped workflow card over a low-level product-detail card.\n\
Keep field values concise: title under 18 Chinese characters; when/what/why under 50 Chinese characters each; boundary under 40 Chinese characters.\n\
Return only valid JSON with this shape: {{\"candidates\": [{{\"cluster_id\":\"...\",\"title\":\"...\",\"when\":\"...\",\"what\":\"...\",\"why\":\"...\",\"boundary\":\"...\",\"kind\":\"preference|constraint|procedure\",\"scope\":\"global|project\",\"memory_tier\":\"project_rule|cross_project_principle|collaboration_preference\",\"abstraction_level\":\"too_low|good|too_high\",\"support_level\":\"strong|medium|weak\",\"evidence_quotes\":[{{\"observation_id\":\"...\",\"text\":\"literal quote\"}}],\"temporal_status\":\"stable|reversed|refined\",\"confidence\":0.0}}]}}.\n\
Do not include markdown fences or explanatory text. Each item must cite literal evidence_quotes from the messages.\n\
Use cluster_id from one cited cluster as the candidate cluster_id when you include it.\n\
If no durable rule exists, return {{\"candidates\": []}}.\n\n\
clusters:\n{clusters_text}"
    );

    ProviderRequest {
        system_prompt: SYSTEM_PROMPT.to_string(),
        user_prompt,
        json_schema: None,
    }
}

const BATCH_MESSAGE_MAX_CHARS: usize = 500;

fn compact_batch_body(body: &str) -> String {
    let count = body.chars().count();
    if count <= BATCH_MESSAGE_MAX_CHARS {
        return body.to_string();
    }
    body.chars().take(BATCH_MESSAGE_MAX_CHARS).collect()
}

fn induce_schema() -> ProviderJsonSchema {
    ProviderJsonSchema {
        name: "induce_candidate".to_string(),
        strict: false, // oneOf 在严格模式下兼容性问题，先关掉
        schema: serde_json::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
                "title": {"type": "string"},
                "when": {"type": "string"},
                "what": {"type": "string"},
                "why": {"type": "string"},
                "boundary": {"type": "string"},
                "kind": {"enum": ["preference", "constraint", "procedure"]},
                "scope": {"enum": ["global", "project"]},
                "memory_tier": {"enum": ["project_rule", "cross_project_principle", "collaboration_preference"]},
                "abstraction_level": {"enum": ["too_low", "good", "too_high"]},
                "support_level": {"enum": ["strong", "medium", "weak"]},
                "evidence_quotes": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "observation_id": {"type": "string"},
                            "text": {"type": "string"}
                        },
                        "required": ["observation_id", "text"]
                    }
                },
                "temporal_status": {"enum": ["stable", "reversed", "refined"]},
                "confidence": {"type": "number"},
                "reject": {"type": "boolean"},
                "reason": {"type": "string"}
            }
        }),
    }
}

/// 解析 LLM 响应。返回 Ok(None) 表示 LLM 主动拒绝。
fn parse_induce_response(
    response: &str,
    cluster: &MessageCluster,
) -> Result<Option<InducedCandidate>> {
    let value: serde_json::Value = parse_json_lenient(response)?;

    if value
        .get("reject")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        return Ok(None);
    }

    let title = require_string(&value, "title")?;
    let when = require_string(&value, "when")?;
    let what = require_string(&value, "what")?;
    let why = require_string(&value, "why")?;
    let boundary = optional_nonempty_string(&value, "boundary");
    let kind = require_string(&value, "kind")?;
    let scope = require_string(&value, "scope")?;
    let memory_tier = optional_nonempty_string(&value, "memory_tier");
    let abstraction_level = optional_nonempty_string(&value, "abstraction_level");
    let support_level = optional_nonempty_string(&value, "support_level");
    let temporal_status = value
        .get("temporal_status")
        .and_then(|v| v.as_str())
        .unwrap_or("stable")
        .to_string();
    let confidence = value
        .get("confidence")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.6) as f32;
    // 单簇硬约束（v2 修订）：recurrence=1 的簇没有多重证据，LLM 给的高 confidence
    // 不可信——一次性请求最容易在这里假阳性。强制下调到 0.80 以下，让审核
    // 看到这是低置信记忆。
    let confidence = clamp_singleton_confidence(cluster.recurrence, confidence);

    let evidence_quotes = value
        .get("evidence_quotes")
        .and_then(|v| v.as_array())
        .context("evidence_quotes missing or not array")?
        .iter()
        .map(|q| EvidenceQuote {
            observation_id: q
                .get("observation_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            text: q
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        })
        .collect();

    let mut candidate = InducedCandidate {
        cluster_id: cluster.cluster_id.clone(),
        recurrence: cluster.recurrence,
        title,
        when,
        what,
        why,
        boundary,
        kind,
        scope,
        evidence_quotes,
        memory_tier,
        abstraction_level,
        support_level,
        temporal_status,
        confidence,
    };
    normalize_methodology_scope(&mut candidate);
    Ok(Some(candidate))
}

fn parse_batch_induce_response(
    response: &str,
    clusters: &[MessageCluster],
) -> Result<Vec<InducedCandidate>> {
    let value = parse_json_lenient(response)?;
    let array = if let Some(array) = value.as_array() {
        array
    } else {
        value
            .get("candidates")
            .and_then(|value| value.as_array())
            .context("batch induce response is not an array or candidates object")?
    };
    let mut candidates = Vec::new();
    for item in array {
        if item
            .get("reject")
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
        {
            continue;
        }
        let mut candidate = candidate_from_value(item, "batch")?;
        if candidate.cluster_id == "batch" {
            candidate.cluster_id =
                cluster_id_for_evidence(&candidate, clusters).unwrap_or_else(|| {
                    clusters
                        .first()
                        .map(|cluster| cluster.cluster_id.clone())
                        .unwrap_or_else(|| "batch".to_string())
                });
        }
        candidate.recurrence = recurrence_for_evidence(&candidate, clusters);
        candidate.confidence =
            clamp_singleton_confidence(candidate.recurrence, candidate.confidence);
        normalize_methodology_scope(&mut candidate);
        if validate_evidence_quotes_across_clusters(&candidate, clusters).is_ok() {
            candidates.push(candidate);
        }
    }
    Ok(candidates)
}

fn candidate_from_value(
    value: &serde_json::Value,
    fallback_cluster_id: &str,
) -> Result<InducedCandidate> {
    let title = require_string(value, "title")?;
    let when = require_string(value, "when")?;
    let what = require_string(value, "what")?;
    let why = require_string(value, "why")?;
    let boundary = optional_nonempty_string(value, "boundary");
    let kind = require_string(value, "kind")?;
    let scope = require_string(value, "scope")?;
    let memory_tier = optional_nonempty_string(value, "memory_tier");
    let abstraction_level = optional_nonempty_string(value, "abstraction_level");
    let support_level = optional_nonempty_string(value, "support_level");
    let temporal_status = value
        .get("temporal_status")
        .and_then(|v| v.as_str())
        .unwrap_or("stable")
        .to_string();
    let confidence = value
        .get("confidence")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.6) as f32;
    let evidence_quotes = value
        .get("evidence_quotes")
        .and_then(|v| v.as_array())
        .context("evidence_quotes missing or not array")?
        .iter()
        .map(|q| EvidenceQuote {
            observation_id: q
                .get("observation_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            text: q
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        })
        .collect();

    Ok(InducedCandidate {
        cluster_id: value
            .get("cluster_id")
            .and_then(|v| v.as_str())
            .unwrap_or(fallback_cluster_id)
            .to_string(),
        recurrence: 1,
        title,
        when,
        what,
        why,
        boundary,
        kind,
        scope,
        evidence_quotes,
        memory_tier,
        abstraction_level,
        support_level,
        temporal_status,
        confidence,
    })
}

fn normalize_methodology_scope(candidate: &mut InducedCandidate) {
    if looks_like_global_methodology(candidate) {
        candidate.scope = "global".to_string();
    }
}

fn looks_like_global_methodology(candidate: &InducedCandidate) -> bool {
    let combined = format!(
        "{}\n{}\n{}\n{}",
        candidate.title, candidate.when, candidate.what, candidate.why
    )
    .to_lowercase();
    [
        "真实历史",
        "真实结果",
        "真实输入",
        "静态样例",
        "持续修正",
        "自我修正",
        "候选质量",
        "质量优先",
        "少而精",
        "少而准",
        "审阅边界",
        "人工审阅",
        "先 review",
        "先确认用户",
        "直接固化",
        "小改快测",
        "大改重测",
        "大改详测",
        "dry-run",
        "dry run",
        "推理引擎",
        "检查有没有问题",
        "质量高不高",
        "先规划",
        "先澄清",
        "先提问",
        "real history",
        "real input",
        "human review",
        "review boundary",
    ]
    .iter()
    .any(|marker| combined.contains(*marker))
}

/// 容忍 LLM 输出包含 markdown 代码块包裹的 JSON。
fn parse_json_lenient(response: &str) -> Result<serde_json::Value> {
    let trimmed = response.trim();
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
        return Ok(value);
    }
    let first_object = trimmed.find('{');
    let first_array = trimmed.find('[');
    let (start, end_char) = match (first_object, first_array) {
        (Some(object), Some(array)) if array < object => (array, ']'),
        (Some(object), _) => (object, '}'),
        (None, Some(array)) => (array, ']'),
        (None, None) => anyhow::bail!("no JSON object or array in induce response"),
    };
    let Some(end) = trimmed.rfind(end_char) else {
        anyhow::bail!(
            "likely-truncated-json: no closing {end_char} in induce response; prefix={:.200}",
            trimmed
        );
    };
    serde_json::from_str(&trimmed[start..=end])
        .with_context(|| format!("parse induce JSON: {:.200}", trimmed))
}

fn require_string(value: &serde_json::Value, field: &str) -> Result<String> {
    Ok(value
        .get(field)
        .and_then(|v| v.as_str())
        .with_context(|| format!("missing field `{field}`"))?
        .to_string())
}

fn optional_nonempty_string(value: &serde_json::Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

/// 校验 evidence_quotes：每条 text 必须出现在簇内对应 observation_id 的消息中。
///
/// 用规范化匹配（去标点 + 空白），所以 quote 不必字符级 byte-equal。
fn validate_evidence_quotes(candidate: &InducedCandidate, cluster: &MessageCluster) -> Result<()> {
    validate_evidence_quotes_across_clusters(candidate, std::slice::from_ref(cluster))
}

fn validate_evidence_quotes_across_clusters(
    candidate: &InducedCandidate,
    clusters: &[MessageCluster],
) -> Result<()> {
    if candidate.evidence_quotes.is_empty() {
        anyhow::bail!("evidence_quotes is empty");
    }
    for eq in &candidate.evidence_quotes {
        let matching_body = clusters
            .iter()
            .flat_map(|cluster| cluster.messages.iter())
            .find(|message| {
                evidence_id_matches(&message.observation_id, &eq.observation_id)
                    && normalized_contains(&message.body, &eq.text)
            })
            .map(|message| message.body.as_str());
        if matching_body.is_none() {
            anyhow::bail!(
                "evidence_quote text not found in obs {}: {:.80}",
                eq.observation_id,
                eq.text
            );
        }
    }
    Ok(())
}

fn cluster_id_for_evidence(
    candidate: &InducedCandidate,
    clusters: &[MessageCluster],
) -> Option<String> {
    candidate.evidence_quotes.iter().find_map(|quote| {
        clusters.iter().find_map(|cluster| {
            cluster
                .messages
                .iter()
                .any(|message| evidence_id_matches(&message.observation_id, &quote.observation_id))
                .then(|| cluster.cluster_id.clone())
        })
    })
}

fn recurrence_for_evidence(candidate: &InducedCandidate, clusters: &[MessageCluster]) -> usize {
    let mut recurrence = 1usize;
    for quote in &candidate.evidence_quotes {
        for cluster in clusters {
            if cluster
                .messages
                .iter()
                .any(|message| evidence_id_matches(&message.observation_id, &quote.observation_id))
            {
                recurrence = recurrence.max(cluster.recurrence);
            }
        }
    }
    recurrence
}

fn evidence_id_matches(message_id: &str, quote_id: &str) -> bool {
    message_id == quote_id
        || message_id
            .strip_prefix(quote_id)
            .is_some_and(|rest| rest.starts_with('#'))
}

/// 规范化子串匹配：去全部空白与标点后比较。
pub(crate) fn normalized_contains(haystack: &str, needle: &str) -> bool {
    let normalize = |s: &str| -> String {
        s.chars()
            .filter(|c| c.is_alphanumeric() || is_cjk(*c))
            .flat_map(|c| c.to_lowercase())
            .collect()
    };
    let h = normalize(haystack);
    let n = normalize(needle);
    if n.is_empty() {
        return false;
    }
    h.contains(&n)
}

fn is_cjk(c: char) -> bool {
    matches!(c, '\u{3400}'..='\u{4DBF}' | '\u{4E00}'..='\u{9FFF}' | '\u{20000}'..='\u{2A6DF}')
}

#[cfg(test)]
mod batch_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::truncate::TruncatedMessage;

    fn truncated(id: &str, body: &str) -> TruncatedMessage {
        TruncatedMessage {
            observation_id: id.to_string(),
            role: "unknown".to_string(),
            body: body.to_string(),
            original_len: body.len(),
            truncated_len: body.len(),
            was_truncated: false,
            created_at: "2026-05-01T00:00:00+00:00".to_string(),
            source_kind: "claude-code-session".to_string(),
        }
    }

    fn cluster_with(messages: Vec<TruncatedMessage>) -> MessageCluster {
        MessageCluster {
            cluster_id: "c_test_001".to_string(),
            recurrence: messages.len(),
            messages,
            mean_similarity: 0.85,
            backend: "fastembed".to_string(),
        }
    }

    /// Mock provider that returns a fixed JSON string.
    struct StubProvider {
        response: String,
    }

    impl InduceProvider for StubProvider {
        fn call(&self, _request: &ProviderRequest, _max_tokens: usize) -> Result<String> {
            Ok(self.response.clone())
        }
    }

    #[test]
    fn parse_accept_response_extracts_all_fields() {
        let cluster = cluster_with(vec![
            truncated("o1", "评估提炼时优先用真实历史会话做回归"),
            truncated("o2", "记得用真实历史回归不要只依赖样例"),
        ]);
        let response = r#"{
            "title": "评估提炼优先用真实历史回归",
            "when": "在评估提炼质量或修改提炼逻辑时",
            "what": "优先使用真实历史会话做回归验证",
            "why": "让规则接受真实数据检验",
            "boundary": "只在评估提炼质量时使用，不把真实对话写入可提交 fixture",
            "kind": "procedure",
            "scope": "global",
            "memory_tier": "cross_project_principle",
            "abstraction_level": "good",
            "support_level": "strong",
            "evidence_quotes": [
                {"observation_id": "o1", "text": "优先用真实历史会话做回归"},
                {"observation_id": "o2", "text": "不要只依赖样例"}
            ],
            "temporal_status": "stable",
            "confidence": 0.85
        }"#;
        let provider = StubProvider {
            response: response.to_string(),
        };
        let outcomes = induce_with_provider(&provider, &[cluster]);
        assert_eq!(outcomes.len(), 1);
        match &outcomes[0] {
            InductionOutcome::Accepted(c) => {
                assert_eq!(c.title, "评估提炼优先用真实历史回归");
                assert_eq!(c.kind, "procedure");
                assert_eq!(
                    c.boundary.as_deref(),
                    Some("只在评估提炼质量时使用，不把真实对话写入可提交 fixture")
                );
                assert_eq!(c.memory_tier.as_deref(), Some("cross_project_principle"));
                assert_eq!(c.abstraction_level.as_deref(), Some("good"));
                assert_eq!(c.support_level.as_deref(), Some("strong"));
                assert_eq!(c.evidence_quotes.len(), 2);
                assert!((c.confidence - 0.85).abs() < 1e-3);
            }
            other => panic!("expected accept, got {:?}", other),
        }
    }

    #[test]
    fn parse_reject_response() {
        let cluster = cluster_with(vec![truncated("o1", "请帮我修一下这个 bug")]);
        let response = r#"{"reject": true, "reason": "one-off request"}"#;
        let provider = StubProvider {
            response: response.to_string(),
        };
        let outcomes = induce_with_provider(&provider, &[cluster]);
        assert!(matches!(outcomes[0], InductionOutcome::Rejected { .. }));
    }

    #[test]
    fn parse_legacy_accept_response_keeps_optional_quality_fields_empty() {
        let cluster = cluster_with(vec![
            truncated("o1", "评估提炼时优先用合成样例回归质量门"),
            truncated("o2", "不要让质量评估依赖私人对话原文"),
        ]);
        let response = r#"{
            "title": "评估质量门优先合成回归",
            "when": "评估 Memory Card 质量门时",
            "what": "优先使用合成样例覆盖边界行为",
            "why": "避免测试依赖私人对话原文",
            "kind": "procedure",
            "scope": "global",
            "evidence_quotes": [
                {"observation_id": "o1", "text": "优先用合成样例回归质量门"}
            ],
            "temporal_status": "stable",
            "confidence": 0.84
        }"#;
        let provider = StubProvider {
            response: response.to_string(),
        };

        let outcomes = induce_with_provider(&provider, &[cluster]);

        match &outcomes[0] {
            InductionOutcome::Accepted(c) => {
                assert_eq!(c.boundary, None);
                assert_eq!(c.memory_tier, None);
                assert_eq!(c.abstraction_level, None);
                assert_eq!(c.support_level, None);
            }
            other => panic!("expected accept, got {:?}", other),
        }
    }

    #[test]
    fn batch_induce_provider_call_gets_large_json_token_budget() {
        struct TokenCaptureProvider {
            seen_tokens: std::sync::Mutex<Vec<usize>>,
        }

        impl InduceProvider for TokenCaptureProvider {
            fn call(&self, _request: &ProviderRequest, max_tokens: usize) -> Result<String> {
                self.seen_tokens.lock().unwrap().push(max_tokens);
                Ok(r#"{"candidates":[]}"#.to_string())
            }
        }

        let cluster = cluster_with(vec![truncated("o1", "真实历史回归比静态样例更重要。")]);
        let provider = TokenCaptureProvider {
            seen_tokens: std::sync::Mutex::new(Vec::new()),
        };

        let _ = induce_batch_with_provider(&provider, &[cluster], 3);

        assert_eq!(provider.seen_tokens.lock().unwrap().as_slice(), &[8192]);
    }

    #[test]
    fn incomplete_json_response_reports_likely_truncation() {
        let raw = r#"{"candidates":[{"title":"优先真实历史回归","boundary":"没有闭合"#;

        let error = parse_batch_induce_response(raw, &[]).expect_err("truncated JSON should fail");

        assert!(
            error.to_string().contains("likely-truncated-json"),
            "error should point to truncation, got: {error:?}"
        );
    }

    #[test]
    fn batch_induce_request_demands_concise_fields() {
        let cluster = cluster_with(vec![truncated("o1", "真实历史回归比静态样例更重要。")]);

        let request = build_batch_induce_request(&[cluster], 3);

        assert!(request.user_prompt.contains("Keep field values concise"));
        assert!(
            request
                .user_prompt
                .contains("boundary under 40 Chinese characters")
        );
    }

    #[test]
    fn evidence_quote_not_in_cluster_marks_failed() {
        let cluster = cluster_with(vec![truncated("o1", "评估提炼优先真实历史回归")]);
        let response = r#"{
            "title": "捏造规则",
            "when": "总是",
            "what": "用 axios",
            "why": "拦截器好",
            "kind": "preference",
            "scope": "global",
            "evidence_quotes": [{"observation_id": "o1", "text": "我喜欢 axios"}],
            "temporal_status": "stable",
            "confidence": 0.9
        }"#;
        let provider = StubProvider {
            response: response.to_string(),
        };
        let outcomes = induce_with_provider(&provider, &[cluster]);
        match &outcomes[0] {
            InductionOutcome::Failed { error, .. } => assert!(error.contains("not found")),
            other => panic!("expected failed, got {:?}", other),
        }
    }

    #[test]
    fn evidence_quote_observation_id_not_in_cluster_marks_failed() {
        let cluster = cluster_with(vec![truncated("o1", "正常文本")]);
        let response = r#"{
            "title": "捏造来源",
            "when": "x",
            "what": "y",
            "why": "z",
            "kind": "preference",
            "scope": "global",
            "evidence_quotes": [{"observation_id": "o-不存在", "text": "正常文本"}],
            "temporal_status": "stable",
            "confidence": 0.9
        }"#;
        let provider = StubProvider {
            response: response.to_string(),
        };
        let outcomes = induce_with_provider(&provider, &[cluster]);
        assert!(matches!(outcomes[0], InductionOutcome::Failed { .. }));
    }

    #[test]
    fn evidence_quote_can_reference_base_observation_for_signal_fragment() {
        let cluster = cluster_with(vec![truncated(
            "o1#signal1",
            "重视真实历史回归，不迷信静态样例。",
        )]);
        let response = r#"{
            "title": "优先真实历史回归",
            "when": "评估提炼质量时",
            "what": "优先使用真实历史做回归验证",
            "why": "让规则接受真实数据检验",
            "kind": "procedure",
            "scope": "global",
            "evidence_quotes": [{"observation_id": "o1", "text": "真实历史回归"}],
            "temporal_status": "stable",
            "confidence": 0.9
        }"#;
        let provider = StubProvider {
            response: response.to_string(),
        };

        let outcomes = induce_with_provider(&provider, &[cluster]);

        assert!(matches!(outcomes[0], InductionOutcome::Accepted(_)));
    }

    #[test]
    fn singleton_high_confidence_is_clamped_to_ceiling() {
        // 单消息簇 + LLM 高置信 → 必须被压到 SINGLETON_CONFIDENCE_CEILING
        let cluster = MessageCluster {
            cluster_id: "c_singleton".to_string(),
            recurrence: 1,
            messages: vec![truncated("o1", "请每次都用真实历史回归验证提炼逻辑")],
            mean_similarity: 1.0,
            backend: "fastembed".to_string(),
        };
        let response = format!(
            r#"{{
                "title": "用真实历史做回归验证提炼",
                "when": "评估提炼时",
                "what": "用真实历史会话做回归",
                "why": "让规则接受真实数据检验",
                "kind": "procedure",
                "scope": "global",
                "evidence_quotes": [{{"observation_id": "o1", "text": "真实历史回归"}}],
                "temporal_status": "stable",
                "confidence": 0.95
            }}"#
        );
        let provider = StubProvider { response };
        let outcomes = induce_with_provider(&provider, &[cluster]);
        match &outcomes[0] {
            InductionOutcome::Accepted(c) => {
                assert_eq!(c.recurrence, 1);
                assert!(
                    c.confidence <= SINGLETON_CONFIDENCE_CEILING + 1e-6,
                    "singleton confidence {} not clamped",
                    c.confidence
                );
            }
            other => panic!("expected accept, got {:?}", other),
        }
    }

    #[test]
    fn methodology_candidate_is_normalized_to_global_scope() {
        let cluster = cluster_with(vec![truncated(
            "o1",
            "生成候选时更关心候选质量，而不是候选数量，要少而精。",
        )]);
        let response = r#"{
            "title": "保证候选质量优先",
            "when": "优化本项目的 memory 候选流程时",
            "what": "以候选质量优先于数量，少而精地筛选",
            "why": "让候选长期稳定、可回溯、可执行",
            "kind": "preference",
            "scope": "project",
            "evidence_quotes": [{"observation_id": "o1", "text": "候选质量，而不是候选数量"}],
            "temporal_status": "stable",
            "confidence": 0.9
        }"#;
        let provider = StubProvider {
            response: response.to_string(),
        };
        let outcomes = induce_with_provider(&provider, &[cluster]);
        match &outcomes[0] {
            InductionOutcome::Accepted(c) => {
                assert_eq!(c.scope, "global");
            }
            other => panic!("expected accept, got {:?}", other),
        }
    }

    #[test]
    fn multi_message_cluster_keeps_high_confidence() {
        let cluster = cluster_with(vec![
            truncated("o1", "用真实历史做回归验证提炼"),
            truncated("o2", "改提炼前必须用真实历史跑一遍"),
        ]);
        let response = r#"{
            "title": "用真实历史做回归验证提炼",
            "when": "评估提炼时",
            "what": "用真实历史会话做回归",
            "why": "让规则接受真实数据检验",
            "kind": "procedure",
            "scope": "global",
            "evidence_quotes": [
                {"observation_id": "o1", "text": "用真实历史做回归"},
                {"observation_id": "o2", "text": "必须用真实历史"}
            ],
            "temporal_status": "stable",
            "confidence": 0.92
        }"#;
        let provider = StubProvider {
            response: response.to_string(),
        };
        let outcomes = induce_with_provider(&provider, &[cluster]);
        match &outcomes[0] {
            InductionOutcome::Accepted(c) => {
                assert!(
                    (c.confidence - 0.92).abs() < 1e-3,
                    "multi-message confidence should not be clamped, got {}",
                    c.confidence
                );
            }
            other => panic!("expected accept, got {:?}", other),
        }
    }

    #[test]
    fn parse_json_lenient_strips_markdown_fence() {
        let value = parse_json_lenient("```json\n{\"a\":1}\n```").expect("ok");
        assert_eq!(value["a"], 1);
    }

    #[test]
    fn parse_json_lenient_strips_markdown_fenced_array() {
        let value = parse_json_lenient("```json\n[{\"a\":1}]\n```").expect("ok");
        assert_eq!(value[0]["a"], 1);
    }

    #[test]
    fn normalized_contains_handles_punctuation_and_whitespace() {
        assert!(normalized_contains(
            "评估提炼时，优先 用 真实-历史 ！",
            "优先用真实历史"
        ));
        assert!(!normalized_contains("用 axios", "用 fetch"));
    }

    #[test]
    fn empty_clusters_produce_empty_outcomes() {
        let provider = StubProvider {
            response: "{}".to_string(),
        };
        let outcomes = induce_with_provider(&provider, &[]);
        assert!(outcomes.is_empty());
    }

    #[test]
    fn provider_error_marks_failed() {
        struct ErrProvider;
        impl InduceProvider for ErrProvider {
            fn call(&self, _: &ProviderRequest, _: usize) -> Result<String> {
                Err(anyhow::anyhow!("network down"))
            }
        }
        let cluster = cluster_with(vec![truncated("o1", "正常文本能进 cluster")]);
        let outcomes = induce_with_provider(&ErrProvider, &[cluster]);
        match &outcomes[0] {
            InductionOutcome::Failed { error, .. } => assert!(error.contains("network down")),
            other => panic!("expected failed, got {:?}", other),
        }
    }
}
