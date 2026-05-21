use std::collections::HashSet;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::Value;

use crate::fsutil;

use super::ConversationFile;
use super::sessions_index;
pub(super) fn collect_jsonl(
    dir: &Path,
    agent: &str,
    source_kind: &str,
    files: &mut Vec<ConversationFile>,
) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    let indexed = sessions_index::load_sessions_index(dir)?;
    let mut indexed_paths = HashSet::new();
    for entry in indexed {
        indexed_paths.insert(entry.jsonl_path.clone());
        files.push(ConversationFile {
            agent: agent.to_string(),
            source_kind: source_kind.to_string(),
            project_path: Some(entry.project_path),
            path: entry.jsonl_path,
        });
    }
    for entry in walkdir::WalkDir::new(dir).follow_links(false) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.path().extension().and_then(|value| value.to_str()) == Some("jsonl") {
            let path = entry.path().to_path_buf();
            if path_contains_component(&path, "subagents") {
                continue;
            }
            if indexed_paths.contains(&path) {
                continue;
            }
            files.push(ConversationFile {
                agent: agent.to_string(),
                source_kind: source_kind.to_string(),
                project_path: project_path_from_jsonl(&path).ok().flatten(),
                path,
            });
        }
    }
    Ok(())
}

pub(super) fn collect_single_jsonl(
    file: &Path,
    agent: &str,
    source_kind: &str,
    files: &mut Vec<ConversationFile>,
) {
    if file.is_file() {
        if path_contains_component(file, "subagents") {
            return;
        }
        files.push(ConversationFile {
            agent: agent.to_string(),
            source_kind: source_kind.to_string(),
            project_path: project_path_from_jsonl(file).ok().flatten(),
            path: file.to_path_buf(),
        });
    }
}

fn path_contains_component(path: &Path, needle: &str) -> bool {
    path.components().any(|component| {
        component
            .as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case(needle)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_path_from_jsonl_uses_sessions_index_metadata() {
        let temp = tempfile::tempdir().expect("tempdir");
        let project_dir = temp.path().join("-C-Users-alex-Projects-myapp");
        fs::create_dir_all(&project_dir).expect("project dir");
        let jsonl = project_dir.join("session-1.jsonl");
        fs::write(&jsonl, "{}\n").expect("jsonl");
        fs::write(
            project_dir.join("sessions-index.json"),
            r#"{
  "sessions": {
    "session-1": {
      "summary": "use axios",
      "messageCount": 12,
      "gitBranch": "main",
      "lastModified": 1710000000000
    }
  }
}"#,
        )
        .expect("sessions index");

        let project_path = project_path_from_jsonl(&jsonl)
            .expect("project path")
            .expect("path from sessions index");

        assert!(
            project_path.ends_with(
                Path::new("Users")
                    .join("alex")
                    .join("Projects")
                    .join("myapp")
            )
        );
    }

    #[test]
    fn extract_jsonl_messages_skips_tool_results_and_code_dumps() {
        let input = concat!(
            r#"{"type":"user","message":{"role":"user","content":"Use Bun for scripts."}}"#,
            "\n",
            r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","content":"1\tuse std::fs;\n2\tfn noisy() {}"}]}}"#
        );

        let extracted = extract_jsonl_messages(input).expect("extracted");

        assert_eq!(extracted, "Use Bun for scripts.");
    }

    #[test]
    fn extract_jsonl_messages_skips_dispatched_task_briefs() {
        let input = r#"{"type":"user","message":{"role":"user","content":"You are the UI implementation agent for this Rust app.\n\nScope: edit only src/desktop.rs.\nDo not commit.\n\nRequirements:\n1. Keep layout compact.\n2. Existing tests should remain green.\n\nReturn a concise summary of changes and tests run."}}"#;

        assert!(extract_jsonl_messages(input).is_none());
    }

    #[test]
    fn extract_jsonl_messages_keeps_plain_user_text_blocks() {
        let input = r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"以后统一用 pnpm 管理前端依赖。"}]}}"#;

        let extracted = extract_jsonl_messages(input).expect("extracted");

        assert_eq!(extracted, "以后统一用 pnpm 管理前端依赖。");
    }

    #[test]
    fn extract_jsonl_messages_keeps_confirmed_visual_planning_offer_context() {
        let input = concat!(
            r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"有些内容可能用浏览器里的可视化更容易讨论，比如设置页布局和架构图。我可以边聊边做轻量 mockup、对比图或流程图给你看。要试试吗？"}]}}"#,
            "\n",
            r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"我同意，此外有可以借鉴的开源项目或者任何内容也可以借鉴。"}]}}"#
        );

        let extracted = extract_jsonl_messages(input).expect("extracted");

        assert!(extracted.contains("可视化更容易讨论"), "{extracted}");
        assert!(extracted.contains("轻量 mockup"), "{extracted}");
        assert!(extracted.contains("用户同意使用这个方式"), "{extracted}");
        assert!(extracted.contains("开源项目"), "{extracted}");
    }

    #[test]
    fn extract_jsonl_messages_keeps_confirmed_reference_research_offer_context() {
        let input = concat!(
            r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"我建议先搜索同类开源项目和成熟产品，整理可借鉴的结构，再拆成项目启动规划。"}]}}"#,
            "\n",
            r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"我同意，按这个方式规划。"}]}}"#
        );

        let extracted = extract_jsonl_messages(input).expect("extracted");

        assert!(extracted.contains("同类开源项目"), "{extracted}");
        assert!(extracted.contains("用户同意使用这个方式"), "{extracted}");
    }

    #[test]
    fn extract_jsonl_messages_keeps_confirmed_validation_cadence_offer_context() {
        let input = concat!(
            r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"为了推进速度，可以小改只做必要检查，大改或完成前再跑一次完整测试。"}]}}"#,
            "\n",
            r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"可以，就按这个节奏。"}]}}"#
        );

        let extracted = extract_jsonl_messages(input).expect("extracted");

        assert!(extracted.contains("小改只做必要检查"), "{extracted}");
        assert!(extracted.contains("完成前再跑一次完整测试"), "{extracted}");
        assert!(extracted.contains("用户同意使用这个方式"), "{extracted}");
    }

    #[test]
    fn extract_jsonl_messages_skips_generated_session_instruction_blocks() {
        let input = r#"{"type":"user","message":{"role":"user","content":"[Assistant Rules - You MUST follow these instructions]\n[Available Skills]\nTo use a skill, read its SKILL.md file when needed.\n\n## Team Mode\nOnly bring up Team in either of these cases."}}"#;

        assert!(extract_jsonl_messages(input).is_none());
    }

    #[test]
    fn extract_jsonl_messages_skips_context_continuation_summaries() {
        let input = r#"{"type":"user","message":{"role":"user","content":"This session is being continued from a previous conversation that ran out of context. The summary below covers the earlier portion of the conversation.\n\nSummary:\n## 1. Primary Request and Intent\n用户提出了两个主要请求。"}} "#;

        assert!(extract_jsonl_messages(input).is_none());
    }

    #[test]
    fn extract_jsonl_messages_skips_controller_subagent_briefs() {
        let input = r#"{"type":"user","message":{"role":"user","content":"You are Claude Code working in an existing Rust/Tauri/React repository.\n\nController: Codex is coordinating and will review your changes. You are not alone in the codebase.\n\nGoal: Implement the backend/core portion of docs/superpowers/plans/2026-05-03-high-quality-extraction.md.\n\nRead these files first:\n- AGENTS.md\n- src/extract.rs\n\nScope for this run:\n- Implement plan Tasks 1 through 7 only."}}"#;

        assert!(extract_jsonl_messages(input).is_none());
    }

    #[test]
    fn extract_jsonl_messages_skips_file_scoped_review_tasks() {
        let input = r#"{"type":"user","message":{"role":"user","content":"Review the current uncommitted changes in C:\\Users\\15893\\Documents\\New project, focusing only on src/desktop.rs native egui UI.\n\nPlease inspect the current diff. If you find a concrete UI bug or small focused improvement, edit src/desktop.rs directly. Keep scope tight."}}"#;

        assert!(extract_jsonl_messages(input).is_none());
    }

    #[test]
    fn extract_jsonl_messages_skips_chinese_ui_agent_task_briefs() {
        let input = r#"{"type":"user","message":{"role":"user","content":"你是这个项目的 UI 优化代理。请直接修改前端文件，遵守现有中文 Apple-like 风格，不要改 Rust 后端。\n\n当前项目：Agent Memory Kernel Tauri app，工作目录 C:\\Users\\15893\\Documents\\New project。\n需要实现：\n1. app/src/ui-helpers.ts 增加 DesktopTaskStatus 类型。"}} "#;

        assert!(extract_jsonl_messages(input).is_none());
    }

    #[test]
    fn extract_jsonl_messages_skips_readonly_history_scan_agent_briefs() {
        let input = r#"{"type":"user","message":{"role":"user","content":"只读任务。你在 C:\\Users\\15893\\Documents\\New project 仓库中，目标是全量扫描该项目相关的本地历史对话，找出“每段对话开头”更偏长期/规划/启动方式、项目启动偏好、方案规划偏好的内容，作为本地目标测试集候选。不要修改任何文件，不要写入仓库，不要上传任何真实对话数据。可以读取本地 Codex/Claude conversation jsonl 和项目 .agent-kernel 观察记录。输出：1) 你扫描了哪些来源类别和数量；2) 值得计入记忆卡片的候选。"}} "#;

        assert!(extract_jsonl_messages(input).is_none());
    }

    #[test]
    fn extract_jsonl_messages_skips_subagent_notification_blobs() {
        let input = r#"{"type":"user","message":{"role":"user","content":"<subagent_notification>\n{\"agent_path\":\"agent\",\"status\":{\"completed\":\"扫描完成。\\n\\n**值得计入记忆卡片的候选**\\n- 交付验收时，绿色测试只是证据之一。\"}}\n</subagent_notification>"}}"#;

        assert!(extract_jsonl_messages(input).is_none());
    }
}

pub(super) fn conversation_belongs_to_project(
    file: &ConversationFile,
    project_root: &Path,
) -> bool {
    conversation_project_match(file, project_root, false).is_match()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ConversationProjectMatch {
    Match,
    UnknownProject,
    OtherProject,
}

impl ConversationProjectMatch {
    pub(super) fn is_match(self) -> bool {
        matches!(self, Self::Match)
    }
}

pub(super) fn conversation_project_match(
    file: &ConversationFile,
    project_root: &Path,
    include_unknown_project: bool,
) -> ConversationProjectMatch {
    let Some(project_path) = file.project_path.as_deref() else {
        return if include_unknown_project {
            ConversationProjectMatch::Match
        } else {
            ConversationProjectMatch::UnknownProject
        };
    };
    let Ok(source_root) = fsutil::normalize_project_root(project_path) else {
        return ConversationProjectMatch::OtherProject;
    };
    let Ok(target_root) = fsutil::normalize_project_root(project_root) else {
        return ConversationProjectMatch::OtherProject;
    };
    if source_root == target_root || source_root.starts_with(&target_root) {
        ConversationProjectMatch::Match
    } else {
        ConversationProjectMatch::OtherProject
    }
}

pub(super) fn project_path_from_jsonl(file: &Path) -> Result<Option<PathBuf>> {
    if let Some(project_path) = sessions_index::session_project_path(file)? {
        return Ok(Some(project_path));
    }
    let handle = fs::File::open(file).with_context(|| format!("open {}", file.display()))?;
    let reader = BufReader::new(handle);
    let mut scanned_bytes = 0usize;
    for line in reader.lines().take(200) {
        let line = line.with_context(|| format!("read {}", file.display()))?;
        scanned_bytes += line.len();
        if scanned_bytes > 256 * 1024 {
            break;
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if let Some(path) = find_string_key(&value, &["cwd", "project"]) {
            return Ok(Some(PathBuf::from(path)));
        }
    }
    Ok(None)
}

pub(super) fn find_string_key(value: &Value, keys: &[&str]) -> Option<String> {
    match value {
        Value::Object(map) => {
            for key in keys {
                if let Some(Value::String(path)) = map.get(*key) {
                    return Some(path.clone());
                }
            }
            map.values().find_map(|child| find_string_key(child, keys))
        }
        Value::Array(items) => items.iter().find_map(|child| find_string_key(child, keys)),
        _ => None,
    }
}

pub(super) fn normalize_observation_body(input: &str, file: &Path, source_kind: &str) -> String {
    let is_jsonl = file.extension().and_then(|value| value.to_str()) == Some("jsonl")
        || source_kind.contains("session");
    if !is_jsonl {
        return input.trim().to_string();
    }
    extract_jsonl_messages(input).unwrap_or_default()
}

pub(super) fn extract_jsonl_messages(input: &str) -> Option<String> {
    let mut parts = Vec::new();
    let mut pending_confirmable_offer: Option<String> = None;
    for line in input.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if !is_relevant_conversation_line(&value) {
            let mut assistant_parts = Vec::new();
            collect_assistant_authored_text(&value, &mut assistant_parts);
            if let Some(offer) = assistant_parts
                .into_iter()
                .find(|text| looks_like_confirmable_methodology_offer(text))
            {
                pending_confirmable_offer = Some(offer);
            }
            continue;
        }
        let before_len = parts.len();
        collect_user_authored_text(&value, &mut parts);
        if let Some(offer) = pending_confirmable_offer.take()
            && parts[before_len..]
                .iter()
                .any(|part| looks_like_user_acceptance_with_optional_references(part))
        {
            parts.push(confirmed_methodology_context(&offer));
        }
    }
    if parts.is_empty() {
        None
    } else {
        let cleaned = sanitize_extracted_messages(&parts.join("\n"));
        if cleaned.trim().is_empty() {
            None
        } else {
            Some(cleaned)
        }
    }
}

pub(super) fn is_relevant_conversation_line(value: &Value) -> bool {
    if value
        .get("isMeta")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return false;
    }
    let raw = value.to_string().to_lowercase();
    if raw.contains("base_instructions")
        || raw.contains("<local-command")
        || raw.contains("<command-name>")
        || raw.contains("token_count")
    {
        return false;
    }
    if value
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|kind| {
            matches!(
                kind,
                "session_meta"
                    | "queue-operation"
                    | "turn_context"
                    | "attachment"
                    | "event_msg"
                    | "reasoning"
            )
        })
    {
        return false;
    }
    contains_user_authored_text(value)
}

pub(super) fn contains_user_authored_text(value: &Value) -> bool {
    let mut parts = Vec::new();
    collect_user_authored_text(value, &mut parts);
    !parts.is_empty()
}

pub(super) fn collect_user_authored_text(value: &Value, parts: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if map.get("role").and_then(Value::as_str) == Some("user") {
                if let Some(content) = map.get("content") {
                    collect_text_blocks(content, parts);
                }
                if let Some(text) = map.get("text") {
                    collect_text_blocks(text, parts);
                }
            }
            if map.get("type").and_then(Value::as_str) == Some("user") {
                if let Some(message) = map.get("message") {
                    collect_text_blocks(message, parts);
                }
                if let Some(content) = map.get("content") {
                    collect_text_blocks(content, parts);
                }
            }
            if map.get("type").and_then(Value::as_str) == Some("user_message")
                && let Some(content) = map.get("content")
            {
                collect_text_blocks(content, parts);
            }
            if let Some(message) = map.get("message") {
                collect_user_authored_text(message, parts);
            }
            if let Some(payload) = map.get("payload") {
                collect_user_authored_text(payload, parts);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_user_authored_text(item, parts);
            }
        }
        _ => {}
    }
}

fn collect_assistant_authored_text(value: &Value, parts: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if map.get("role").and_then(Value::as_str) == Some("assistant") {
                if let Some(content) = map.get("content") {
                    collect_assistant_text_blocks(content, parts);
                }
                if let Some(text) = map.get("text") {
                    collect_assistant_text_blocks(text, parts);
                }
            }
            if map.get("type").and_then(Value::as_str) == Some("assistant")
                && let Some(content) = map.get("content")
            {
                collect_assistant_text_blocks(content, parts);
            }
            if let Some(message) = map.get("message") {
                collect_assistant_authored_text(message, parts);
            }
            if let Some(payload) = map.get("payload") {
                collect_assistant_authored_text(payload, parts);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_assistant_authored_text(item, parts);
            }
        }
        _ => {}
    }
}

fn collect_assistant_text_blocks(value: &Value, parts: &mut Vec<String>) {
    match value {
        Value::String(text) => push_clean_text(parts, text),
        Value::Array(items) => {
            for item in items {
                collect_assistant_text_blocks(item, parts);
            }
        }
        Value::Object(map) => match map.get("type").and_then(Value::as_str) {
            Some("text" | "input_text" | "output_text") => {
                if let Some(Value::String(text)) = map.get("text") {
                    push_clean_text(parts, text);
                }
            }
            Some("tool_result" | "tool_use" | "thinking" | "reasoning") => {}
            _ => {
                if let Some(Value::String(text)) = map.get("text") {
                    push_clean_text(parts, text);
                } else if let Some(content) = map.get("content") {
                    collect_assistant_text_blocks(content, parts);
                }
            }
        },
        _ => {}
    }
}

fn looks_like_confirmable_methodology_offer(text: &str) -> bool {
    let lower = text.to_lowercase();
    let visual_planning = (lower.contains("可视化")
        || lower.contains("mockup")
        || lower.contains("对比图")
        || lower.contains("流程图")
        || lower.contains("架构图")
        || lower.contains("浏览器"))
        && (lower.contains("讨论") || lower.contains("给你看") || lower.contains("要试试"));
    let reference_research = (lower.contains("借鉴")
        || lower.contains("参考")
        || lower.contains("开源项目")
        || lower.contains("同类产品")
        || lower.contains("成熟产品")
        || lower.contains("相关内容"))
        && (lower.contains("规划") || lower.contains("方案") || lower.contains("启动"));
    let validation_cadence = (lower.contains("小改")
        || lower.contains("快速")
        || lower.contains("推进速度")
        || lower.contains("完整测试")
        || lower.contains("总的测试"))
        && (lower.contains("检查")
            || lower.contains("测试")
            || lower.contains("验证")
            || lower.contains("完成前"));
    visual_planning || reference_research || validation_cadence
}

fn looks_like_user_acceptance_with_optional_references(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("我同意")
        || lower.contains("同意")
        || lower.contains("可以")
        || lower.contains("试试")
        || lower.contains("ok")
}

fn confirmed_methodology_context(offer: &str) -> String {
    let mut text = offer.trim().to_string();
    if text.chars().count() > 220 {
        text = text.chars().take(220).collect();
    }
    format!("{text}\n用户同意使用这个方式。")
}

fn collect_text_blocks(value: &Value, parts: &mut Vec<String>) {
    match value {
        Value::String(text) => push_clean_text(parts, text),
        Value::Array(items) => {
            for item in items {
                collect_text_blocks(item, parts);
            }
        }
        Value::Object(map) => {
            let block_type = map.get("type").and_then(Value::as_str);
            match block_type {
                Some("text" | "input_text") => {
                    if let Some(Value::String(text)) = map.get("text") {
                        push_clean_text(parts, text);
                    }
                }
                Some("tool_result" | "tool_use" | "thinking" | "reasoning" | "output_text") => {}
                _ => {
                    if let Some(Value::String(text)) = map.get("text") {
                        push_clean_text(parts, text);
                    } else if let Some(content) = map.get("content") {
                        collect_text_blocks(content, parts);
                    }
                }
            }
        }
        _ => {}
    }
}

fn push_clean_text(parts: &mut Vec<String>, text: &str) {
    let trimmed = text.trim();
    if trimmed.is_empty() || should_skip_message_blob(trimmed) {
        return;
    }
    if parts.last().is_some_and(|existing| existing == trimmed) {
        return;
    }
    parts.push(trimmed.to_string());
}

fn sanitize_extracted_messages(input: &str) -> String {
    let mut cleaned = Vec::new();
    let mut skipping_block = false;
    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !cleaned.last().is_some_and(|line: &String| line.is_empty()) {
                cleaned.push(String::new());
            }
            continue;
        }
        if starts_filtered_block(trimmed) {
            skipping_block = true;
            continue;
        }
        if skipping_block {
            if ends_filtered_block(trimmed) {
                skipping_block = false;
            }
            continue;
        }
        if should_drop_message_line(trimmed) {
            continue;
        }
        cleaned.push(trimmed.to_string());
    }
    cleaned.join("\n").trim().to_string()
}

fn should_skip_message_blob(text: &str) -> bool {
    looks_like_generated_session_instructions(text)
        || looks_like_subagent_notification_blob(text)
        || looks_like_dispatched_task_brief(text)
        || looks_like_context_continuation_summary(text)
        || looks_like_controller_worker_brief(text)
        || looks_like_readonly_history_scan_brief(text)
        || looks_like_file_scoped_task_prompt(text)
}

fn looks_like_subagent_notification_blob(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("<subagent_notification>") && lower.contains("</subagent_notification>")
}

fn looks_like_generated_session_instructions(text: &str) -> bool {
    let lower = text.to_lowercase();
    let markers = [
        "[assistant rules",
        "[available skills]",
        "[skills location]",
        "team mode",
        "only bring up team",
        "handle the task yourself in the current chat by default",
        "to use a skill, read its skill.md file when needed",
    ];
    markers
        .iter()
        .filter(|marker| lower.contains(**marker))
        .count()
        >= 2
}

fn looks_like_dispatched_task_brief(text: &str) -> bool {
    let lower = text.to_lowercase();
    let declares_role = lower.contains("you are ") || lower.contains("你是 ");
    let task_markers = [
        "scope:",
        "requirements:",
        "important rules:",
        "current failing command:",
        "do not modify files outside",
        "do not touch .agent-kernel/observations",
        "return a concise summary of changes and tests run",
        "existing tests should remain green",
        "return done",
        "review src/",
        "please inspect the current diff",
        "请在当前仓库中只修改",
        "请只做分析，不修改文件",
        "完成后请简要说明改了哪些文件",
        "只输出简洁的改动方案",
        "请给出具体实现建议",
        "当前桌面前端是 tauri + react",
        "当前失败命令",
        "不要写完整文件",
    ];
    (declares_role && task_markers.iter().any(|marker| lower.contains(marker)))
        || (lower.contains("requirements:") && lower.contains("scope:"))
        || (lower.contains("要求：") && lower.contains("只修改"))
}

fn looks_like_context_continuation_summary(text: &str) -> bool {
    let lower = text.to_lowercase();
    (lower.contains("this session is being continued from a previous conversation")
        && lower.contains("summary:"))
        || (lower.contains("ran out of context") && lower.contains("primary request and intent"))
}

fn looks_like_controller_worker_brief(text: &str) -> bool {
    let lower = text.to_lowercase();
    let coordinator_markers = [
        "controller: codex is coordinating",
        "you are not alone in the codebase",
        "scope for this run:",
        "read these files first:",
        "goal: implement the backend/core portion",
    ];
    coordinator_markers
        .iter()
        .filter(|marker| lower.contains(**marker))
        .count()
        >= 2
}

fn looks_like_readonly_history_scan_brief(text: &str) -> bool {
    let lower = text.to_lowercase();
    let readonly = lower.contains("只读任务")
        || lower.contains("不要修改任何文件")
        || lower.contains("do not modify any files");
    let history_scan = (lower.contains("全量扫描") || lower.contains("扫描"))
        && (lower.contains("历史对话")
            || lower.contains("conversation jsonl")
            || lower.contains("观察记录")
            || lower.contains("目标测试集"));
    let reporting_shape = lower.contains("输出：")
        || lower.contains("output:")
        || lower.contains("候选")
        || lower.contains("噪声类型");

    readonly && history_scan && reporting_shape
}

fn looks_like_file_scoped_task_prompt(text: &str) -> bool {
    let lower = text.to_lowercase();
    let file_scope = lower.contains("src/")
        || lower.contains("app/")
        || lower.contains(".rs")
        || lower.contains(".tsx")
        || lower.contains(".ts")
        || lower.contains("工作目录")
        || lower.contains("当前项目：")
        || lower.contains("project path")
        || lower.contains("focusing only on");
    let action_markers = [
        "review the current uncommitted changes",
        "please inspect the current diff",
        "edit src/",
        "edit app/",
        "modify only",
        "please directly modify",
        "do not stop for confirmation",
        "required code changes:",
        "需要实现：",
        "请直接修改前端文件",
        "请编辑代码后简要说明",
        "已有失败测试",
        "run cargo fmt",
    ];
    file_scope && action_markers.iter().any(|marker| lower.contains(marker))
}

fn starts_filtered_block(line: &str) -> bool {
    matches!(
        line,
        "<INSTRUCTIONS>"
            | "<environment_context>"
            | "<permissions instructions>"
            | "<app-context>"
            | "<skills_instructions>"
            | "<plugins_instructions>"
    ) || line.starts_with("# AGENTS.md instructions for ")
}

fn ends_filtered_block(line: &str) -> bool {
    matches!(
        line,
        "</INSTRUCTIONS>"
            | "</environment_context>"
            | "</permissions instructions>"
            | "</app-context>"
            | "</skills_instructions>"
            | "</plugins_instructions>"
    )
}

fn should_drop_message_line(line: &str) -> bool {
    let lower = line.to_lowercase();
    lower.contains("generated by agent-kernel")
        || lower.contains("do not edit directly")
        || lower.starts_with("<system-reminder>")
        || lower.starts_with("</system-reminder>")
        || lower.starts_with("[from user]")
        || lower.starts_with("the file ")
        || lower.starts_with("todos have been modified successfully")
        || lower.starts_with("compiling ")
        || lower.starts_with("running ")
        || lower.starts_with("test result:")
        || lower.starts_with("exit code ")
        || lower.starts_with("wall time:")
        || lower.starts_with("output:")
        || line.starts_with("1\t")
        || line.chars().take_while(|ch| ch.is_ascii_digit()).count() >= 2 && line.contains('\t')
}
