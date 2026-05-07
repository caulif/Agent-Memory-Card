use crate::candidate::MemoryTier;
use crate::provider;

use super::{Candidate, llm};

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub(super) struct LlmAbstractResponse {
    pub abstract_possible: bool,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub memory_tier: Option<String>,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct AbstractedKnowledge {
    pub body: String,
    pub memory_tier: Option<MemoryTier>,
    pub reason: String,
}

pub(super) fn build_abstract_prompt(project_candidate: &Candidate) -> provider::ProviderRequest {
    provider::ProviderRequest {
        system_prompt: r#"你把“项目规则”重写成“跨项目方法论”或“协作偏好”。

输入：一条带具体项目语境的 atomic fact。
输出：去掉项目专名、路径、特定 tool 后，仍然成立的命令式原则；如果去掉这些信息后规则就站不住，输出 abstract_possible: false。

要求：
- 保留判断标准，也就是什么时候用、为什么重要
- 仍然命令式、自包含、单句
- 不允许出现 cargo / Bun / Rust / AGENTS.md / Claude Code / Codex 这类项目或工具专名
- memory_tier 只能是 cross_project_principle 或 collaboration_preference
- 只返回 JSON，不要 markdown"#
            .to_string(),
        user_prompt: format!(
            "Project rule:\n{}\n\nEvidence:\n{}",
            project_candidate.body, project_candidate.evidence
        ),
        json_schema: Some(abstract_schema()),
    }
}

pub(super) fn parse_abstract_output(
    output: &str,
) -> Result<Option<AbstractedKnowledge>, llm::ParseError> {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return Err(llm::ParseError::EmptyOutput);
    }
    let response = serde_json::from_str::<LlmAbstractResponse>(trimmed)
        .map_err(|_| llm::ParseError::InvalidJson(trimmed.chars().take(200).collect()))?;
    if !response.abstract_possible {
        return Ok(None);
    }
    let Some(body) = response.body.map(|body| body.trim().to_string()) else {
        return Ok(None);
    };
    if body.len() < 12 || body.len() > 260 || contains_forbidden_project_term(&body) {
        return Ok(None);
    }
    Ok(Some(AbstractedKnowledge {
        body,
        memory_tier: memory_tier_from_output(response.memory_tier.as_deref()),
        reason: if response.reason.trim().is_empty() {
            "Abstracted project-context rule into a reusable principle.".to_string()
        } else {
            response.reason
        },
    }))
}

pub(super) fn run_candidate_abstraction(
    project_root: &std::path::Path,
    project_candidate: &Candidate,
) -> Result<Option<AbstractedKnowledge>, llm::LlmExtractionError> {
    let prompt = build_abstract_prompt(project_candidate);
    let cfg = provider::load_or_default_provider_config(project_root)
        .map_err(|e| llm::LlmExtractionError::ProviderError(e.to_string()))?;
    let output =
        provider::call_provider_for_role(&cfg, provider::ProviderRole::Abstract, &prompt, 512)
            .map_err(|e| llm::LlmExtractionError::ProviderError(e.to_string()))?;
    let parsed = parse_abstract_output(&output).map_err(|e| {
        llm::LlmExtractionError::ParseError(format!(
            "{e}: {}",
            output.chars().take(200).collect::<String>()
        ))
    })?;
    if parsed.as_ref().is_some_and(|spec| {
        contains_dynamic_project_term(project_root, &spec.body).unwrap_or(false)
    }) {
        return Ok(None);
    }
    Ok(parsed)
}

fn abstract_schema() -> provider::ProviderJsonSchema {
    provider::ProviderJsonSchema {
        name: "SkillletAbstraction".to_string(),
        strict: true,
        schema: serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "abstract_possible": { "type": "boolean" },
                "body": { "type": ["string", "null"] },
                "memory_tier": { "type": ["string", "null"], "enum": ["cross_project_principle", "collaboration_preference", null] },
                "reason": { "type": "string" }
            },
            "required": ["abstract_possible", "body", "memory_tier", "reason"]
        }),
    }
}

fn memory_tier_from_output(value: Option<&str>) -> Option<MemoryTier> {
    match value {
        Some("collaboration_preference") => Some(MemoryTier::CollaborationPreference),
        Some("cross_project_principle") => Some(MemoryTier::CrossProjectPrinciple),
        _ => None,
    }
}

fn contains_forbidden_project_term(body: &str) -> bool {
    let lower = body.to_lowercase();
    if [
        "cargo",
        "bun",
        "rust",
        "agents.md",
        "claude code",
        "codex",
        "axios",
        "pnpm",
        "npm",
        "agent-kernel",
        "skilllet",
        "tauri",
        "react",
        "evidencechunk",
    ]
    .iter()
    .any(|term| lower.contains(term))
    {
        return true;
    }

    lower.contains(".rs")
        || lower.contains(".md")
        || lower.contains(".tsx")
        || lower.contains(".ts")
        || lower.contains('/')
        || lower.contains('\\')
        || body.split_whitespace().any(is_project_specific_token)
}

fn is_project_specific_token(token: &str) -> bool {
    let trimmed = token.trim_matches(|ch: char| {
        ch.is_ascii_punctuation() || matches!(ch, '。' | '，' | '；' | '：' | '（' | '）')
    });
    if trimmed.len() < 3 {
        return false;
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.len() >= 2
        && lower.starts_with('v')
        && lower[1..]
            .chars()
            .all(|ch| ch.is_ascii_digit() || ch == '.')
        && lower[1..].chars().any(|ch| ch.is_ascii_digit())
    {
        return true;
    }
    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
        && trimmed.chars().any(|ch| ch.is_ascii_uppercase())
    {
        return true;
    }
    let mut saw_lower = false;
    for ch in trimmed.chars() {
        if ch.is_ascii_lowercase() {
            saw_lower = true;
        } else if saw_lower && ch.is_ascii_uppercase() {
            return true;
        }
    }
    false
}

fn contains_dynamic_project_term(
    project_root: &std::path::Path,
    body: &str,
) -> std::io::Result<bool> {
    let terms = dynamic_project_terms(project_root)?;
    let lower = body.to_lowercase();
    Ok(terms
        .iter()
        .any(|term| lower.contains(&term.to_lowercase())))
}

fn dynamic_project_terms(project_root: &std::path::Path) -> std::io::Result<Vec<String>> {
    let dir = project_root.join(".agent-kernel").join("observations");
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut counts = std::collections::BTreeMap::<String, usize>::new();
    for entry in std::fs::read_dir(dir)?.take(200) {
        let path = entry?.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("yml") {
            continue;
        }
        let text = std::fs::read_to_string(path)?;
        for token in text
            .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '-'))
            .filter(|token| is_project_specific_token(token))
        {
            if !matches!(
                token.to_ascii_lowercase().as_str(),
                "http" | "json" | "yaml" | "html" | "utf8" | "api"
            ) {
                *counts.entry(token.to_string()).or_default() += 1;
            }
        }
    }
    Ok(counts
        .into_iter()
        .filter_map(|(term, count)| (count >= 5).then_some(term))
        .take(64)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project_candidate(body: &str) -> Candidate {
        Candidate {
            title: "Project Rule".to_string(),
            body: body.to_string(),
            kind: "procedure".to_string(),
            scope: "project".to_string(),
            memory_tier: MemoryTier::ProjectRule,
            abstraction_of: None,
            abstracted_from: None,
            evidence: body.to_string(),
            confidence: Some(0.9),
            reason: None,
            matched_template: None,
        }
    }

    #[test]
    fn abstract_prompt_uses_schema_and_project_context() {
        let prompt = build_abstract_prompt(&project_candidate(
            "This project should focus on Skilllet generation as the core feature.",
        ));

        assert!(prompt.system_prompt.contains("跨项目方法论"));
        assert!(prompt.user_prompt.contains("Skilllet generation"));
        assert_eq!(
            prompt.json_schema.as_ref().expect("schema").name,
            "SkillletAbstraction"
        );
    }

    #[test]
    fn parse_abstract_output_rejects_project_terms() {
        let output = r#"{
            "abstract_possible": true,
            "body": "Always run cargo test before changing Rust extraction code.",
            "memory_tier": "collaboration_preference",
            "reason": "Still contains tool-specific terms."
        }"#;

        let parsed = parse_abstract_output(output).expect("parse");

        assert!(parsed.is_none());
    }

    #[test]
    fn parse_abstract_output_rejects_identifier_paths_and_versions() {
        for body in [
            "Keep EvidenceChunk validation reusable across extraction stages.",
            "Always review app/src/main.tsx before changing workflow behavior.",
            "Prefer v2.6 replay fixtures for abstraction changes.",
        ] {
            let output = format!(
                r#"{{
                    "abstract_possible": true,
                    "body": {body:?},
                    "memory_tier": "cross_project_principle",
                    "reason": "Contains project-specific residue."
                }}"#
            );

            let parsed = parse_abstract_output(&output).expect("parse");
            assert!(parsed.is_none(), "{body} should be rejected");
        }
    }

    #[test]
    fn parse_abstract_output_keeps_valid_principle() {
        let output = r#"{
            "abstract_possible": true,
            "body": "Focus on the core workflow before expanding secondary capabilities.",
            "memory_tier": "cross_project_principle",
            "reason": "Reusable product prioritization principle."
        }"#;

        let parsed = parse_abstract_output(output)
            .expect("parse")
            .expect("abstracted");

        assert_eq!(parsed.memory_tier, Some(MemoryTier::CrossProjectPrinciple));
        assert!(parsed.body.contains("core workflow"));
    }
}
