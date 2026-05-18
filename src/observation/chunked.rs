use std::collections::BTreeSet;
use std::path::Path;

use anyhow::Result;

use crate::candidate::MemoryTier;
use crate::extract;

use super::ObservationRecord;
use super::flow;

pub(super) fn synthesis_material(observations: &[ObservationRecord], max_chars: usize) -> String {
    synthesis_material_chunks(observations, max_chars, max_chars)
        .into_iter()
        .next()
        .unwrap_or_default()
}

pub(crate) fn prefiltered_synthesis_material(
    observations: &[ObservationRecord],
    max_chars: usize,
) -> String {
    let mut scored = observations
        .iter()
        .filter_map(|observation| {
            let score = methodology_signal_score(&observation.body) * 4
                + durable_signal_score(&observation.body);
            (score > 0).then_some((score, observation.created_at.clone(), observation))
        })
        .collect::<Vec<_>>();
    scored.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| right.1.cmp(&left.1)));

    let mut out = String::new();
    for (_, _, observation) in scored.into_iter().take(40) {
        if out.len() >= max_chars {
            break;
        }
        let entry = compact_observation_entry(observation);
        if out.len() + entry.len() > max_chars && !out.is_empty() {
            break;
        }
        out.push_str(&entry);
    }
    if out.trim().is_empty() {
        synthesis_material(observations, max_chars.min(24_000))
    } else {
        out
    }
}

pub(super) fn extract_local_chunks_to_report(
    project_root: &Path,
    observations: &[ObservationRecord],
    targets: Vec<String>,
    source: &str,
    max_candidates: usize,
) -> Result<extract::ExtractReport> {
    let mut merged = extract::ExtractReport {
        created: Vec::new(),
        skipped: Vec::new(),
        candidates: Vec::new(),
        dry_run: true,
        provider: "local-hybrid-merge".to_string(),
        redacted: false,
    };
    let mut seen = BTreeSet::new();

    merge_flow_candidates(
        project_root,
        observations,
        targets.clone(),
        source,
        max_candidates,
        "local".to_string(),
        &mut merged,
        &mut seen,
    )?;

    for gold_line in global_methodology_lines(observations, 12) {
        let report = extract::extract_high_value_text_to_drafts(
            project_root,
            &gold_line,
            targets.clone(),
            &format!("{source}, gold methodology prefilter"),
            Some("local".to_string()),
            true,
            3,
        )?;
        merged.redacted |= report.redacted;
        merged.skipped.extend(report.skipped);
        for candidate in report.candidates {
            if is_rejected_preview(&candidate) {
                continue;
            }
            if seen.insert(candidate.id.clone()) {
                merged.candidates.push(candidate);
            }
        }
    }

    let snippet_materials = methodology_snippet_materials(observations, 12);
    if !snippet_materials.is_empty() {
        let chunk_source = format!("{source}, methodology prefilter");
        let snippet_text = combined_snippet_material(&snippet_materials);
        let report = extract::extract_high_value_text_to_drafts(
            project_root,
            &snippet_text,
            targets.clone(),
            &chunk_source,
            Some("local".to_string()),
            true,
            max_candidates,
        )?;
        merged.redacted |= report.redacted;
        merged.skipped.extend(report.skipped);
        for candidate in report.candidates {
            if is_rejected_preview(&candidate) {
                continue;
            }
            if seen.insert(candidate.id.clone()) {
                merged.candidates.push(candidate);
            }
        }
    }

    if merged.candidates.len() >= max_candidates
        && merged
            .candidates
            .iter()
            .any(|candidate| candidate.memory_tier == MemoryTier::ProjectRule)
    {
        balanced_truncate_previews(&mut merged.candidates, max_candidates);
        return Ok(merged);
    }

    let chunks = synthesis_material_chunks(observations, 12_000, 48_000);
    for (index, chunk) in chunks.iter().enumerate() {
        let chunk_source = format!("{source}, chunk {}/{}", index + 1, chunks.len());
        let report = extract::extract_high_value_text_to_drafts(
            project_root,
            chunk,
            targets.clone(),
            &chunk_source,
            Some("local".to_string()),
            true,
            max_candidates,
        )?;
        merged.redacted |= report.redacted;
        merged.skipped.extend(report.skipped);
        for candidate in report.candidates {
            if is_rejected_preview(&candidate) {
                continue;
            }
            if seen.insert(candidate.id.clone()) {
                merged.candidates.push(candidate);
            }
        }
    }

    balanced_truncate_previews(&mut merged.candidates, max_candidates);
    Ok(merged)
}

fn is_rejected_preview(candidate: &extract::ExtractCandidatePreview) -> bool {
    candidate
        .classification
        .as_ref()
        .is_some_and(|classification| classification.artifact_kind == "reject")
}

fn methodology_snippets(body: &str) -> String {
    let mut lines = body
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| looks_methodology_rich(line))
        .filter(|line| line.chars().count() >= 8)
        .map(|line| (methodology_line_score(line), line))
        .collect::<Vec<_>>();
    lines.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| left.1.chars().count().cmp(&right.1.chars().count()))
            .then_with(|| left.1.cmp(right.1))
    });
    lines
        .into_iter()
        .map(|(_, line)| line)
        .take(8)
        .collect::<Vec<_>>()
        .join("\n")
}

fn methodology_snippet_materials(
    observations: &[ObservationRecord],
    limit: usize,
) -> Vec<(String, String)> {
    let mut materials = Vec::new();
    if let Some(global_snippets) = global_methodology_snippets(observations, 64) {
        materials.push(("gold-methodology-snippets".to_string(), global_snippets));
    }

    let mut scored = observations
        .iter()
        .filter_map(|observation| {
            let snippets = methodology_snippets(&observation.body);
            if snippets.trim().is_empty() {
                return None;
            }
            Some((
                methodology_signal_score(&observation.body),
                observation.created_at.clone(),
                observation.id.clone(),
                snippets,
            ))
        })
        .collect::<Vec<_>>();
    scored.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| right.1.cmp(&left.1)));
    materials.extend(
        scored
            .into_iter()
            .take(limit.saturating_sub(materials.len()))
            .map(|(_, _, id, snippets)| (id, snippets)),
    );
    materials
}

fn global_methodology_snippets(observations: &[ObservationRecord], limit: usize) -> Option<String> {
    let snippets = global_methodology_lines(observations, limit).join("\n");
    (!snippets.trim().is_empty()).then_some(snippets)
}

fn global_methodology_lines(observations: &[ObservationRecord], limit: usize) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut lines = observations
        .iter()
        .flat_map(|observation| observation.body.lines())
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| looks_methodology_rich(line))
        .filter(|line| !looks_like_methodology_artifact_line(line))
        .filter(|line| line.chars().count() >= 8 && line.chars().count() <= 180)
        .filter_map(|line| {
            let normalized = line
                .trim_start_matches(['-', '*', ' '])
                .trim_matches(['"', '"', '“', '”', '\'', '`'])
                .trim()
                .to_string();
            if normalized.is_empty() || !seen.insert(normalized.clone()) {
                return None;
            }
            Some((methodology_line_score(&normalized), normalized))
        })
        .collect::<Vec<_>>();
    lines.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| left.1.chars().count().cmp(&right.1.chars().count()))
            .then_with(|| left.1.cmp(&right.1))
    });
    lines
        .into_iter()
        .take(limit)
        .map(|(_, line)| line)
        .collect::<Vec<_>>()
}

fn looks_like_methodology_artifact_line(line: &str) -> bool {
    let lower = line.to_lowercase();
    line.contains('│')
        || line.contains('|')
        || lower.contains("expected_memory_tier")
        || lower.contains("collaboration_preference")
        || lower.contains("cross_project_principle")
        || lower.contains("project_rule")
        || lower.contains("gold set")
        || lower.contains("至少多少条")
        || lower.contains("principle_named_terms")
        || lower.contains("planningheuristic")
        || lower.contains("principlestatement")
        || lower.contains("let principle_topics")
        || lower.contains("例:")
        || lower.contains("例：")
        || lower.contains("候选（")
        || lower.contains("按你说的")
        || lower.contains("我希望得到的是对其他项目")
        || lower.contains("我希望得到候选")
        || lower.contains("真正有价值的语句")
        || lower.contains("[\"")
        || lower.contains("\",")
}

fn combined_snippet_material(snippet_materials: &[(String, String)]) -> String {
    snippet_materials
        .iter()
        .map(|(id, snippets)| format!("source: {id}\n{snippets}"))
        .collect::<Vec<_>>()
        .join("\n---\n")
}

pub(super) fn extract_llm_chunks_to_report(
    project_root: &Path,
    observations: &[ObservationRecord],
    targets: Vec<String>,
    source: &str,
    max_candidates: usize,
) -> Result<extract::ExtractReport> {
    let mut merged = extract::ExtractReport {
        created: Vec::new(),
        skipped: Vec::new(),
        candidates: Vec::new(),
        dry_run: true,
        provider: "llm-chunked".to_string(),
        redacted: false,
    };
    let mut seen = BTreeSet::new();

    merge_flow_candidates(
        project_root,
        observations,
        targets.clone(),
        source,
        max_candidates,
        "llm".to_string(),
        &mut merged,
        &mut seen,
    )?;

    if let Some(gold_snippets) = global_methodology_snippets(observations, 12) {
        let report = extract::extract_high_value_text_to_drafts(
            project_root,
            &gold_snippets,
            targets.clone(),
            &format!("{source}, gold methodology prefilter"),
            None,
            true,
            max_candidates,
        )?;
        merged.redacted |= report.redacted;
        merged.skipped.extend(report.skipped);
        for candidate in report.candidates {
            if is_rejected_preview(&candidate) {
                continue;
            }
            if seen.insert(candidate.id.clone()) {
                merged.candidates.push(candidate);
            }
        }
    }

    let snippet_materials = methodology_snippet_materials(observations, 12);
    if !snippet_materials.is_empty() {
        let chunk_source = format!("{source}, methodology prefilter");
        let snippet_text = combined_snippet_material(&snippet_materials);
        let report = extract::extract_high_value_text_to_drafts(
            project_root,
            &snippet_text,
            targets.clone(),
            &chunk_source,
            None,
            true,
            max_candidates,
        )?;
        merged.redacted |= report.redacted;
        merged.skipped.extend(report.skipped);
        for candidate in report.candidates {
            if is_rejected_preview(&candidate) {
                continue;
            }
            if seen.insert(candidate.id.clone()) {
                merged.candidates.push(candidate);
            }
        }
    }

    if merged.candidates.len() >= max_candidates
        && merged
            .candidates
            .iter()
            .any(|candidate| candidate.memory_tier == MemoryTier::ProjectRule)
    {
        balanced_truncate_previews(&mut merged.candidates, max_candidates);
        return Ok(merged);
    }

    let chunks = synthesis_material_chunks(observations, 12_000, 48_000);
    for (index, chunk) in chunks.iter().enumerate() {
        let chunk_source = format!("{source}, chunk {}/{}", index + 1, chunks.len());
        let report = extract::extract_high_value_text_to_drafts(
            project_root,
            chunk,
            targets.clone(),
            &chunk_source,
            None,
            true,
            max_candidates,
        )?;
        merged.redacted |= report.redacted;
        merged.skipped.extend(report.skipped);
        for candidate in report.candidates {
            if is_rejected_preview(&candidate) {
                continue;
            }
            if seen.insert(candidate.id.clone()) {
                merged.candidates.push(candidate);
            }
        }
    }

    balanced_truncate_previews(&mut merged.candidates, max_candidates);
    Ok(merged)
}

fn merge_flow_candidates(
    project_root: &Path,
    observations: &[ObservationRecord],
    targets: Vec<String>,
    source: &str,
    max_candidates: usize,
    provider_name: String,
    merged: &mut extract::ExtractReport,
    seen: &mut BTreeSet<String>,
) -> Result<()> {
    let flow_summary = flow::summarize_conversation_flow(observations);
    if flow_summary.is_empty() {
        return Ok(());
    }
    let flow_material = flow_summary.to_extraction_material();
    let chunk_source = format!("{source}, global flow prefilter");
    let provider = if provider_name == "local" {
        Some("local".to_string())
    } else {
        None
    };
    let report = extract::extract_high_value_text_to_drafts(
        project_root,
        &flow_material,
        targets,
        &chunk_source,
        provider,
        true,
        max_candidates,
    )?;
    merged.redacted |= report.redacted;
    merged.skipped.extend(report.skipped);
    for candidate in report.candidates {
        if is_rejected_preview(&candidate) {
            continue;
        }
        if seen.insert(candidate.id.clone()) {
            merged.candidates.push(candidate);
        }
    }
    Ok(())
}

fn balanced_truncate_previews(
    candidates: &mut Vec<extract::ExtractCandidatePreview>,
    max_candidates: usize,
) {
    candidates.retain(|candidate| !looks_like_preview_noise(candidate));
    dedupe_previews_by_body(candidates);
    if candidates.len() <= max_candidates {
        sort_previews_by_confidence(candidates);
        return;
    }

    sort_previews_by_confidence(candidates);
    let mut selected = Vec::new();
    let mut used = vec![false; candidates.len()];
    let has_project = candidates
        .iter()
        .any(|candidate| candidate.memory_tier == MemoryTier::ProjectRule);
    let target_non_project = candidates
        .iter()
        .filter(|candidate| candidate.memory_tier != MemoryTier::ProjectRule)
        .count()
        .min(7)
        .min(max_candidates.saturating_sub(usize::from(has_project)));

    for (index, candidate) in candidates.iter().enumerate() {
        if selected.len() >= target_non_project {
            break;
        }
        if candidate.memory_tier != MemoryTier::ProjectRule {
            selected.push(candidate.clone());
            used[index] = true;
        }
    }

    for (index, candidate) in candidates.iter().enumerate() {
        if selected.len() >= max_candidates {
            break;
        }
        if !used[index] {
            selected.push(candidate.clone());
        }
    }

    sort_previews_by_confidence(&mut selected);
    *candidates = selected;
}

fn looks_like_preview_noise(candidate: &extract::ExtractCandidatePreview) -> bool {
    let combined =
        format!("{}\n{}\n{}", candidate.id, candidate.title, candidate.body).to_lowercase();
    candidate.id.starts_with("global:global-")
        || candidate.id.starts_with("project:project-")
        || candidate.id.starts_with("agent:agent-")
        || combined.contains("cross-project-principl")
        || combined.contains("cross_project_principl")
        || (combined.contains("always-on-rule")
            && (combined.contains("axios-bun") || combined.contains("这类项目工具偏好")))
        || (combined.contains("现在的架构")
            && (combined.contains("功能也实现不了") || combined.contains("体验也")))
        || (combined.contains("优化架构") && combined.contains("体验也很不"))
        || combined.contains("do-not-keep-a-test-that")
        || combined.contains("never-use-a-code-sent-by")
        || combined.contains("never use a code sent by")
        || combined.contains("expected-fail-because")
        || combined.contains("if-easy-extract-small-handlers")
        || combined.contains("pet-stage-warning")
        || combined.contains("settings-panel-target-actions")
        || combined.contains("preserve-existing-builder-responsibility")
        || combined.contains("only checks implementation details")
        || combined.contains("generated classes")
        || combined.contains("css selectors in tests")
        || combined.contains("我会先快速检查")
        || combined.contains("我已经用-spec-driven-develop")
        || combined.contains("我已经用 spec-driven-develop")
        || combined.contains("reason-多处明确")
        || combined.contains("接下来使用这个skills")
        || combined.contains("接下来用这个skills")
        || ((combined.contains("top-10") || combined.contains("top 10"))
            && combined.contains("真实历史"))
        || ((combined.contains("现有项目") || combined.contains("当前项目"))
            && (combined.contains("全面提升") || combined.contains("实现")))
}

fn sort_previews_by_confidence(candidates: &mut [extract::ExtractCandidatePreview]) {
    candidates.sort_by(|a, b| {
        b.confidence
            .unwrap_or(0.0)
            .total_cmp(&a.confidence.unwrap_or(0.0))
            .then_with(|| a.id.cmp(&b.id))
    });
}

fn dedupe_previews_by_body(candidates: &mut Vec<extract::ExtractCandidatePreview>) {
    sort_previews_by_confidence(candidates);
    let mut seen = BTreeSet::new();
    candidates.retain(|candidate| {
        let key = format!(
            "{}:{}:{}",
            candidate.memory_tier.as_str(),
            candidate.scope,
            candidate.body.trim().to_lowercase()
        );
        seen.insert(key)
    });
}

fn methodology_signal_score(body: &str) -> usize {
    let lower = body.to_lowercase();
    [
        "核心功能",
        "用户视角",
        "用户体验",
        "体验优化",
        "先规划",
        "先提问",
        "真实历史",
        "审阅边界",
        "候选质量",
        "持续修正",
        "自我修正",
        "快测",
        "回归",
        "真实结果",
        "推理引擎",
        "检查有没有问题",
        "可视化",
        "mockup",
        "对比图",
        "流程图",
        "架构图",
        "开源项目",
        "同类产品",
        "借鉴",
        "参考",
        "质量高不高",
        "自检",
        "dry-run",
        "dry run",
        "core functionality",
        "user perspective",
        "user experience",
        "planning",
        "real history",
        "feedback loop",
    ]
    .iter()
    .filter(|marker| lower.contains(**marker))
    .count()
}

fn methodology_line_score(line: &str) -> usize {
    let lower = line.to_lowercase();
    let mut score = methodology_signal_score(line) * 2 + durable_signal_score(line);
    for marker in [
        "真实历史回归",
        "候选质量",
        "审阅边界",
        "持续自我修正",
        "不迷信静态样例",
        "小改快测",
        "大改重测",
        "真实结果",
        "推理引擎",
        "检查有没有问题",
        "质量高不高",
        "自检",
        "先 review",
        "不要让 ai 直接",
    ] {
        if lower.contains(marker) {
            score += 6;
        }
    }
    if lower.contains("例子") || lower.contains("比如") || lower.contains("修改计划") {
        score = score.saturating_sub(1);
    }
    score
}

fn durable_signal_score(body: &str) -> usize {
    let lower = body.to_lowercase();
    [
        "以后", "每次", "默认", "统一", "必须", "不要", "always", "prefer", "must", "default",
        "use ", "do not",
    ]
    .iter()
    .filter(|marker| lower.contains(**marker))
    .count()
}

pub(super) fn synthesis_material_chunks(
    observations: &[ObservationRecord],
    chunk_chars: usize,
    total_max_chars: usize,
) -> Vec<String> {
    let mut sorted = observations.to_vec();
    sorted.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut total = 0usize;

    for observation in sorted {
        if total >= total_max_chars {
            break;
        }
        let entry = observation_entry(&observation);
        if !current.is_empty() && current.len() + entry.len() > chunk_chars {
            total += current.len();
            chunks.push(std::mem::take(&mut current));
            if total >= total_max_chars {
                break;
            }
        }
        current.push_str(&entry);
    }
    if !current.trim().is_empty() && total < total_max_chars {
        chunks.push(current);
    }
    chunks
}

fn observation_entry(observation: &ObservationRecord) -> String {
    let mut body = observation.body.replace('\0', "");
    if body.len() > 4_000 {
        body = truncate_head_and_tail(&body, 2_000, 2_000);
    }
    format!(
        "\n--- {} ---\nsource: {}\nagent: {}\nevidence: {}\ntext:\n{}\n",
        observation.id,
        observation.source_kind,
        observation.agent.as_deref().unwrap_or("unknown"),
        observation.evidence,
        body
    )
}

fn compact_observation_entry(observation: &ObservationRecord) -> String {
    let snippets = methodology_snippets(&observation.body);
    let mut body = if snippets.trim().is_empty() {
        observation.body.replace('\0', "")
    } else {
        snippets
    };
    if body.len() > 1_200 {
        body = truncate_head_and_tail(&body, 700, 500);
    }
    format!(
        "\n--- {} ---\nsource: {}\nagent: {}\nevidence: {}\ntext:\n{}\n",
        observation.id,
        observation.source_kind,
        observation.agent.as_deref().unwrap_or("unknown"),
        observation.evidence,
        body
    )
}

#[allow(dead_code)]
fn truncate_utf8_boundary(text: &mut String, max_len: usize) {
    if text.len() <= max_len {
        return;
    }
    let mut new_len = max_len;
    while new_len > 0 && !text.is_char_boundary(new_len) {
        new_len -= 1;
    }
    text.truncate(new_len);
}

fn truncate_head_and_tail(text: &str, head_len: usize, tail_len: usize) -> String {
    if text.len() <= head_len + tail_len + 32 {
        return text.to_string();
    }

    let mut safe_head = head_len.min(text.len());
    while safe_head > 0 && !text.is_char_boundary(safe_head) {
        safe_head -= 1;
    }

    let mut safe_tail_start = text.len().saturating_sub(tail_len);
    while safe_tail_start < text.len() && !text.is_char_boundary(safe_tail_start) {
        safe_tail_start += 1;
    }

    format!(
        "{}\n...\n{}\n",
        &text[..safe_head],
        &text[safe_tail_start..]
    )
}

fn looks_methodology_rich(body: &str) -> bool {
    let lower = body.to_lowercase();
    [
        "核心功能",
        "用户视角",
        "体验",
        "先规划",
        "先提问",
        "真实历史",
        "审阅边界",
        "候选质量",
        "持续修正",
        "自我修正",
        "快测",
        "回归",
        "真实结果",
        "推理引擎",
        "检查有没有问题",
        "可视化",
        "mockup",
        "对比图",
        "流程图",
        "架构图",
        "开源项目",
        "同类产品",
        "借鉴",
        "参考",
        "质量高不高",
        "自检",
        "dry-run",
        "dry run",
        "core functionality",
        "user perspective",
        "planning first",
        "real history",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(id: &str, created_at: &str, body: &str) -> ObservationRecord {
        ObservationRecord {
            id: id.to_string(),
            source_kind: "test".to_string(),
            source_path: "test.jsonl".to_string(),
            agent: Some("codex".to_string()),
            body: body.to_string(),
            evidence: "test".to_string(),
            redacted: false,
            created_at: created_at.to_string(),
        }
    }

    #[test]
    fn chunks_preserve_recent_observations_without_one_large_lump() {
        let observations = vec![
            observation("old", "2026-01-01T00:00:00Z", &"old ".repeat(2000)),
            observation("new", "2026-01-02T00:00:00Z", "以后用 pnpm。"),
        ];

        let chunks = synthesis_material_chunks(&observations, 500, 10_000);

        assert!(chunks.len() >= 2);
        assert!(chunks[0].contains("以后用 pnpm"));
    }

    #[test]
    fn observation_entry_truncates_multibyte_text_without_panicking() {
        let entry = observation_entry(&observation(
            "zh",
            "2026-01-02T00:00:00Z",
            &"提炼规则。".repeat(1200),
        ));

        assert!(entry.contains("提炼规则"));
    }

    #[test]
    fn long_observation_entry_keeps_tail_context() {
        let body = format!("{}\n{}", "前文".repeat(1200), "尾部关键信号：先规划后修改");
        let entry = observation_entry(&observation("tail", "2026-01-03T00:00:00Z", &body));

        assert!(entry.contains("尾部关键信号"));
        assert!(entry.contains("前文"));
    }

    #[test]
    fn methodology_snippets_extract_concise_high_value_lines() {
        let body = "普通噪音\n设计阶段：先提问、先澄清目标、先规划、从用户视角看。\n小改快测，大改重测。\n别的闲聊";
        let snippets = methodology_snippets(body);

        assert!(snippets.contains("先提问"));
        assert!(snippets.contains("小改快测"));
        assert!(!snippets.contains("普通噪音"));
    }

    #[test]
    fn methodology_snippets_keep_self_verification_feedback() {
        let body = "普通噪音\n自己用推理引擎跑一下真实结果，检查有没有问题，质量高不高，然后分析原因再优化。\n别的闲聊";
        let snippets = methodology_snippets(body);
        assert!(snippets.contains("推理引擎"), "{snippets}");
        assert!(snippets.contains("真实结果"), "{snippets}");
    }

    #[test]
    fn methodology_snippets_prioritize_late_specific_preferences_over_early_repetition() {
        let body = format!(
            "{}\n重视真实历史回归，不迷信静态样例。\n更关心候选质量，而不是候选数量。\n希望结果能支持持续自我修正。",
            (0..12)
                .map(|index| format!("早期分析行 {index}: 核心功能和用户体验需要继续优化。"))
                .collect::<Vec<_>>()
                .join("\n")
        );

        let snippets = methodology_snippets(&body);

        assert!(snippets.contains("真实历史回归"), "{snippets}");
        assert!(snippets.contains("候选质量"), "{snippets}");
        assert!(snippets.contains("持续自我修正"), "{snippets}");
    }

    #[test]
    fn methodology_snippet_materials_prioritize_signal_density() {
        let observations = vec![
            observation("recent-weak", "2026-01-03T00:00:00Z", "先规划。"),
            observation(
                "older-rich",
                "2026-01-02T00:00:00Z",
                "核心功能优先，先把主流程做稳。\n用户视角和用户体验很重要。\n真实历史回归要保留。",
            ),
        ];

        let materials = methodology_snippet_materials(&observations, 1);

        assert_eq!(materials[0].0, "gold-methodology-snippets");
        assert!(materials[0].1.contains("核心功能"));
    }

    #[test]
    fn methodology_snippet_materials_put_global_high_signal_lines_first() {
        let observations = vec![
            observation(
                "older-rich",
                "2026-01-02T00:00:00Z",
                "核心功能优先，先把主流程做稳。\n│ collaboration_preference │ 20 │ 希望候选质量优先于数量 │\n重视真实历史回归，不迷信静态样例。\n更关心候选质量，而不是候选数量。",
            ),
            observation("recent-weak", "2026-01-03T00:00:00Z", "先规划。"),
        ];

        let materials = methodology_snippet_materials(&observations, 2);

        assert_eq!(materials[0].0, "gold-methodology-snippets");
        assert!(materials[0].1.contains("真实历史回归"));
        assert!(materials[0].1.contains("候选质量"));
        assert!(!materials[0].1.contains("collaboration_preference"));
    }

    #[test]
    fn balanced_preview_selection_keeps_non_project_tiers() {
        let mut candidates = Vec::new();
        for index in 0..12 {
            candidates.push(extract::ExtractCandidatePreview {
                id: format!("project:{index}"),
                title: format!("Project {index}"),
                body: format!("Project-local rule {index}"),
                kind: "constraint".to_string(),
                scope: "project".to_string(),
                memory_tier: crate::candidate::MemoryTier::ProjectRule,
                abstraction_of: None,
                abstracted_from: None,
                evidence: "test".to_string(),
                confidence: Some(0.99 - index as f32 * 0.01),
                reason: None,
                matched_template: None,
                classification: None,
                tags: Vec::new(),
                suggested_action: None,
                operation: extract::lifecycle::MemoryCardOperation::Add,
                quality_flags: Vec::new(),
            });
        }
        candidates.push(extract::ExtractCandidatePreview {
            id: "global:core".to_string(),
            title: "Focus On Core Functionality".to_string(),
            body: "Focus on the core functionality first.".to_string(),
            kind: "principle".to_string(),
            scope: "global".to_string(),
            memory_tier: crate::candidate::MemoryTier::CrossProjectPrinciple,
            abstraction_of: None,
            abstracted_from: None,
            evidence: "test".to_string(),
            confidence: Some(0.62),
            reason: None,
            matched_template: None,
            classification: None,
            tags: Vec::new(),
            suggested_action: None,
            operation: extract::lifecycle::MemoryCardOperation::Add,
            quality_flags: Vec::new(),
        });
        candidates.push(extract::ExtractCandidatePreview {
            id: "global:plan".to_string(),
            title: "Plan Before Editing".to_string(),
            body: "Before implementation, ask clarifying questions.".to_string(),
            kind: "procedure".to_string(),
            scope: "global".to_string(),
            memory_tier: crate::candidate::MemoryTier::CollaborationPreference,
            abstraction_of: None,
            abstracted_from: None,
            evidence: "test".to_string(),
            confidence: Some(0.61),
            reason: None,
            matched_template: None,
            classification: None,
            tags: Vec::new(),
            suggested_action: None,
            operation: extract::lifecycle::MemoryCardOperation::Add,
            quality_flags: Vec::new(),
        });

        balanced_truncate_previews(&mut candidates, 10);

        assert!(candidates.iter().any(|candidate| candidate.memory_tier
            == crate::candidate::MemoryTier::CrossProjectPrinciple));
        assert!(candidates.iter().any(|candidate| candidate.memory_tier
            == crate::candidate::MemoryTier::CollaborationPreference));
    }

    #[test]
    fn balanced_preview_selection_drops_replay_artifacts() {
        let mut candidates = vec![
            extract::ExtractCandidatePreview {
                id: "global:cross-project-principl".to_string(),
                title: "cross-project-principl".to_string(),
                body: "cross-project-principle".to_string(),
                kind: "principle".to_string(),
                scope: "global".to_string(),
                memory_tier: crate::candidate::MemoryTier::CrossProjectPrinciple,
                abstraction_of: None,
                abstracted_from: None,
                evidence: "test".to_string(),
                confidence: Some(0.99),
                reason: None,
                matched_template: None,
                classification: None,
                tags: Vec::new(),
                suggested_action: None,
                operation: extract::lifecycle::MemoryCardOperation::Add,
                quality_flags: Vec::new(),
            },
            extract::ExtractCandidatePreview {
                id: "global:bad-architecture".to_string(),
                title: "优化架构".to_string(),
                body: "优化架构，现在的架构功能也实现不了，体验也很不好。".to_string(),
                kind: "principle".to_string(),
                scope: "global".to_string(),
                memory_tier: crate::candidate::MemoryTier::CrossProjectPrinciple,
                abstraction_of: None,
                abstracted_from: None,
                evidence: "test".to_string(),
                confidence: Some(0.98),
                reason: None,
                matched_template: None,
                classification: None,
                tags: Vec::new(),
                suggested_action: None,
                operation: extract::lifecycle::MemoryCardOperation::Add,
                quality_flags: Vec::new(),
            },
            extract::ExtractCandidatePreview {
                id: "global:fast-tests".to_string(),
                title: "小改快测".to_string(),
                body: "小改快测，大改重测。".to_string(),
                kind: "procedure".to_string(),
                scope: "global".to_string(),
                memory_tier: crate::candidate::MemoryTier::CollaborationPreference,
                abstraction_of: None,
                abstracted_from: None,
                evidence: "test".to_string(),
                confidence: Some(0.7),
                reason: None,
                matched_template: None,
                classification: None,
                tags: Vec::new(),
                suggested_action: None,
                operation: extract::lifecycle::MemoryCardOperation::Add,
                quality_flags: Vec::new(),
            },
        ];

        balanced_truncate_previews(&mut candidates, 10);

        assert_eq!(candidates.len(), 1);
        assert!(candidates[0].body.contains("小改快测"));
    }

    #[test]
    fn local_chunk_extraction_surfaces_multiple_gold_methodology_candidates() {
        let temp = tempfile::tempdir().expect("tempdir");
        let observations = vec![observation(
            "gold",
            "2026-01-04T00:00:00Z",
            "协作阶段：先 review 再 merge、先保留审阅边界、不要让 AI 直接固化规则\n重视真实历史回归，不迷信静态样例\n希望结果能支持持续自我修正\n更关心候选质量，而不是候选数量\n验证阶段：小改快测、大改详测、优先针对性测试\n开发阶段：核心功能优先、体验优先、别过度堆功能、真实历史重测",
        )];

        let report = extract_local_chunks_to_report(
            temp.path(),
            &observations,
            vec!["codex".to_string()],
            "test",
            8,
        )
        .expect("extract");

        assert!(
            report.candidates.len() >= 4,
            "expected multiple gold methodology candidates: {:?}",
            report.candidates
        );
        assert!(
            report
                .candidates
                .iter()
                .all(|candidate| !candidate.body.contains("候选质量"))
        );
    }

    #[test]
    fn local_chunk_extraction_surfaces_self_verification_memory() {
        let temp = tempfile::tempdir().expect("tempdir");
        let observations = vec![observation(
            "gold",
            "2026-01-04T00:00:00Z",
            "自己用推理引擎跑一下真实结果，检查有没有问题，质量高不高，然后分析原因再优化。\n候选质量优先于候选数量。\n真实历史回归比样例更重要。",
        )];

        let report = extract_local_chunks_to_report(temp.path(), &observations, vec![], "test", 8)
            .expect("extract");

        assert!(
            report.candidates.iter().any(|candidate| {
                candidate
                    .body
                    .contains("当准备提交最终代码、分析结论或复杂任务结果时")
                    && candidate.body.contains("dry-run 自检")
            }),
            "{:?}",
            report.candidates
        );
    }

    #[test]
    fn local_chunk_extraction_surfaces_global_flow_candidates() {
        let temp = tempfile::tempdir().expect("tempdir");
        let observations = vec![
            observation(
                "startup",
                "2026-01-01T00:00:00Z",
                "我同意，此外有可以借鉴的开源项目或者任何内容也可以借鉴。",
            ),
            observation(
                "middle",
                "2026-01-01T01:00:00Z",
                "不是只看指标，最终质量你也要自己看一下。",
            ),
            observation(
                "end",
                "2026-01-01T02:00:00Z",
                "最后用真实历史 dry-run 测试，并从用户视角验收。",
            ),
        ];

        let report = extract_local_chunks_to_report(
            temp.path(),
            &observations,
            vec!["codex".to_string()],
            "test",
            8,
        )
        .expect("report");

        assert!(
            report.candidates.iter().any(|candidate| {
                candidate
                    .matched_template
                    .as_deref()
                    .is_some_and(|template| template == "global-flow:reference-research")
            }),
            "{report:#?}"
        );
        assert!(
            report.candidates.iter().any(|candidate| {
                candidate
                    .matched_template
                    .as_deref()
                    .is_some_and(|template| template == "global-flow:real-history-validation")
            }),
            "{report:#?}"
        );
    }
}
