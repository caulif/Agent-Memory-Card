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
    for line in input.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if !is_relevant_conversation_line(&value) {
            continue;
        }
        collect_user_authored_text(&value, &mut parts);
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
        || looks_like_dispatched_task_brief(text)
        || looks_like_context_continuation_summary(text)
        || looks_like_controller_worker_brief(text)
        || looks_like_file_scoped_task_prompt(text)
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
