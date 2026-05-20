//! Pipeline Layer 1：STRIP 去噪。
//!
//! 输入：[`ObservationRecord`] 列表（已经从 jsonl session 抽出来的片段）
//! 输出：[`StrippedMessage`] 列表（去除噪声后的"自然语言段"）
//!
//! 这一层只做确定性去噪，不调 LLM、不算 embedding。
//!
//! 删除：
//! - `<system-reminder>...</system-reminder>` 系统注入块
//! - `<tool_use>` / `<tool_result>` / `<function_calls>` / `<function_results>` 工具回显
//! - 完全重复的消息（按规范化文本判定，跨 observation 去重）
//! - 过短消息（去噪后字符数 < 20）
//! - 纯空白 / 纯标点 / 不含字母与中文的消息
//! - 已知的 system prompt 模板（开头匹配固定话术）
//!
//! 保留：原始内容里"自然语言"部分。
//!
//! 不做：
//! - 截断（留给 Layer 2 TRUNCATE）
//! - 聚类（留给 Layer 3 CLUSTER）
//! - 任何 LLM 调用

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::observation::ObservationRecord;

/// 去噪后的单条消息片段。
///
/// 字段尽量贴近 [`ObservationRecord`]，方便后续层回溯到原 observation。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StrippedMessage {
    /// 原 observation 的 id（如 `obs:claude-code:sha256:04384`）
    pub observation_id: String,

    /// 推断的角色：`user` / `assistant` / `system` / `unknown`
    pub role: String,

    /// 去噪后的纯文本内容
    pub body: String,

    /// 原字节长度（debug，用于压缩比统计）
    pub original_len: usize,

    /// 去噪后的字节长度（debug）
    pub stripped_len: usize,

    /// observation 的创建时间，Layer 3 / Layer 4 排序时使用
    pub created_at: String,

    /// 原 source_kind（如 `claude-code-session` / `codex-session`）
    pub source_kind: String,
}

/// 流水线 Layer 1 主入口：把 observation 列表去噪成 [`StrippedMessage`] 列表。
///
/// # 流程
///
/// 1. 对每条 observation 调 [`strip_body`] 去掉 XML 块与多余空白
/// 2. 用 [`is_meaningful`] 过滤短/纯标点消息
/// 3. 用 [`looks_like_system_prompt`] 过滤已知 system prompt 模板
/// 4. 用 [`normalize_for_dedup`] 做跨 observation 去重
/// 5. 用 [`infer_role`] 给每条消息打粗略 role
///
/// # 性能
///
/// O(N) 时间 + O(N) 内存（HashSet 存归一化文本指纹）。
pub fn strip_observations(records: &[ObservationRecord]) -> Vec<StrippedMessage> {
    let mut seen_normalized: HashSet<String> = HashSet::new();
    let mut output: Vec<StrippedMessage> = Vec::with_capacity(records.len());

    for record in records {
        let stripped = strip_body(&record.body);
        if !is_meaningful(&stripped) {
            continue;
        }
        if looks_like_system_prompt(&stripped) {
            continue;
        }
        for (suffix, body) in split_high_signal_methodology(&stripped) {
            if !is_meaningful(&body) {
                continue;
            }
            let dedup_key = normalize_for_dedup(&body);
            if dedup_key.is_empty() || !seen_normalized.insert(dedup_key) {
                continue;
            }
            let role = infer_role(&record.source_kind, &body);
            let stripped_len = body.len();
            output.push(StrippedMessage {
                observation_id: format!("{}{}", record.id, suffix),
                role,
                body,
                original_len: record.body.len(),
                stripped_len,
                created_at: record.created_at.clone(),
                source_kind: record.source_kind.clone(),
            });
        }
    }

    output
}

fn split_high_signal_methodology(body: &str) -> Vec<(String, String)> {
    let mut parts = Vec::new();
    for (index, line) in methodology_signal_lines(body).into_iter().enumerate() {
        parts.push((format!("#signal{}", index + 1), line));
    }
    let body_key = normalize_for_dedup(body);
    if !parts
        .iter()
        .any(|(_, part)| normalize_for_dedup(part) == body_key)
    {
        parts.push(("".to_string(), body.to_string()));
    }
    parts
}

fn methodology_signal_lines(body: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut lines = body
        .lines()
        .flat_map(split_methodology_sentences)
        .map(|line| normalize_methodology_line(&line))
        .filter(|line| {
            let count = line.chars().count();
            (MIN_MEANINGFUL_CHARS..=MAX_METHODOLOGY_SIGNAL_CHARS).contains(&count)
        })
        .filter(|line| looks_like_methodology_signal(line))
        .filter(|line| !looks_like_methodology_artifact(line))
        .filter(|line| seen.insert(normalize_for_dedup(line)))
        .map(|line| (methodology_signal_score(&line), line))
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
        .take(MAX_METHODOLOGY_SIGNALS_PER_OBSERVATION)
        .map(|(_, line)| line)
        .collect()
}

fn split_methodology_sentences(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    if trimmed.chars().count() <= MAX_METHODOLOGY_SIGNAL_CHARS {
        return vec![trimmed.to_string()];
    }
    trimmed
        .split(['。', '；', ';'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| part.to_string())
        .collect()
}

fn normalize_methodology_line(line: &str) -> String {
    line.trim()
        .trim_start_matches(['-', '*', '•', ' ', '\t'])
        .trim_matches(['"', '"', '“', '”', '\'', '`'])
        .trim()
        .to_string()
}

fn looks_like_methodology_signal(line: &str) -> bool {
    methodology_signal_score(line) >= 2
}

fn methodology_signal_score(line: &str) -> usize {
    let lower = line.to_lowercase();
    [
        "真实历史",
        "真实结果",
        "真实输入",
        "静态样例",
        "持续修正",
        "自我修正",
        "候选质量",
        "少而精",
        "质量优先",
        "质量高不高",
        "审阅边界",
        "人工审阅",
        "先 review",
        "先确认",
        "直接固化",
        "小改快测",
        "大改重测",
        "大改详测",
        "快测",
        "详测",
        "回归",
        "自检",
        "dry-run",
        "dry run",
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
        "先规划",
        "先提问",
        "先澄清",
        "用户视角",
        "核心功能",
        "real history",
        "real input",
        "dry run",
        "human review",
        "review boundary",
        "regression",
        "self-check",
    ]
    .iter()
    .filter(|marker| lower.contains(**marker))
    .count()
}

fn looks_like_methodology_artifact(line: &str) -> bool {
    let lower = line.to_lowercase();
    line.contains('│')
        || line.contains('|')
        || lower.contains("expected_memory_tier")
        || lower.contains("collaboration_preference")
        || lower.contains("cross_project_principle")
        || lower.contains("gold set")
}

/// 去掉 XML 标签块、合并多余空白、trim 首尾。
///
/// 处理顺序很重要：先剥 XML（避免标签内的 whitespace 影响 collapse），
/// 再 collapse_whitespace（让 `<tag>` 之间的换行合并），最后 trim。
fn strip_body(body: &str) -> String {
    let mut text = extract_jsonl_text_items(body)
        .or_else(|| extract_wrapped_user_request(body))
        .unwrap_or_else(|| body.to_string());
    for tag in [
        "system-reminder",
        "tool_use",
        "tool_result",
        "function_calls",
        "function_results",
        "turn_aborted",
        "command-name",
        "command-message",
        "command-args",
    ] {
        text = strip_xml_block(&text, tag);
    }
    text = collapse_whitespace(&text);
    text.trim().to_string()
}

fn extract_wrapped_user_request(body: &str) -> Option<String> {
    let marker = "[User Request]";
    let start = body.find(marker)? + marker.len();
    let mut text = body[start..].trim().to_string();
    for stop in [
        "\nWeb search results",
        "\nLinks:",
        "\nREMINDER:",
        "\nNow I have",
        "\n[Assistant Rules",
        "\n<system-reminder>",
    ] {
        if let Some(pos) = text.find(stop) {
            text.truncate(pos);
        }
    }
    (!text.trim().is_empty()).then(|| text.trim().to_string())
}

fn extract_jsonl_text_items(body: &str) -> Option<String> {
    let mut out = Vec::new();
    let mut parsed = 0usize;
    let lines = body
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if lines.is_empty() || !lines.iter().all(|line| line.starts_with('{')) {
        return None;
    }
    for line in lines {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            return None;
        };
        parsed += 1;
        collect_user_json_text(&value, &mut out);
    }
    if parsed == 0 || out.is_empty() {
        None
    } else {
        Some(out.join("\n"))
    }
}

fn collect_user_json_text(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            if map.get("role").and_then(|v| v.as_str()) == Some("user") {
                if let Some(content) = map.get("content") {
                    collect_text_blocks(content, out);
                }
                if let Some(text) = map.get("text") {
                    collect_text_blocks(text, out);
                }
            }
            if map.get("type").and_then(|v| v.as_str()) == Some("user") {
                if let Some(message) = map.get("message") {
                    collect_text_blocks(message, out);
                }
                if let Some(content) = map.get("content") {
                    collect_text_blocks(content, out);
                }
            }
            if let Some(message) = map.get("message") {
                collect_user_json_text(message, out);
            }
            if let Some(payload) = map.get("payload") {
                collect_user_json_text(payload, out);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                collect_user_json_text(value, out);
            }
        }
        _ => {}
    }
}

fn collect_text_blocks(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::String(text) => push_natural_text(text, out),
        serde_json::Value::Array(values) => {
            for value in values {
                collect_text_blocks(value, out);
            }
        }
        serde_json::Value::Object(map) => match map.get("type").and_then(|v| v.as_str()) {
            Some("text" | "input_text") => {
                if let Some(text) = map.get("text").and_then(|v| v.as_str()) {
                    push_natural_text(text, out);
                }
            }
            Some("tool_result" | "tool_use" | "thinking" | "reasoning" | "output_text") => {}
            _ => {
                if let Some(content) = map.get("content") {
                    collect_text_blocks(content, out);
                }
            }
        },
        _ => {}
    }
}

fn push_natural_text(text: &str, out: &mut Vec<String>) {
    let text = text.trim();
    if text.chars().count() >= MIN_MEANINGFUL_CHARS
        && text.chars().any(|c| c.is_alphabetic() || is_cjk(c))
    {
        out.push(text.to_string());
    }
}

/// 删除 `<tag ...>...</tag>` 块。
///
/// - 支持带属性的开标签（`<tool_use name="Bash">`）
/// - 未闭合时丢弃从 open 到字符串末尾（防止恶意输入吞掉后续合法内容）
/// - 不递归嵌套：嵌套场景在我们数据里没出现，先简化
fn strip_xml_block(text: &str, tag: &str) -> String {
    let open_prefix = format!("<{tag}");
    let close_tag = format!("</{tag}>");
    let mut result = String::with_capacity(text.len());
    let mut remainder = text;

    loop {
        let Some(open_pos) = remainder.find(&open_prefix) else {
            result.push_str(remainder);
            break;
        };
        result.push_str(&remainder[..open_pos]);
        let after_open = &remainder[open_pos..];
        match after_open.find(&close_tag) {
            Some(close_offset) => {
                let total = close_offset + close_tag.len();
                remainder = &after_open[total..];
            }
            None => {
                // 未闭合：丢弃尾部
                break;
            }
        }
    }

    result
}

/// 多空白合并：连续空格→单空格；连续换行→单换行；段落感被保留。
fn collapse_whitespace(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last_was_newline = false;
    let mut last_was_space = false;
    for c in text.chars() {
        if c == '\n' || c == '\r' {
            if !last_was_newline {
                out.push('\n');
                last_was_newline = true;
                last_was_space = true;
            }
        } else if c.is_whitespace() {
            if !last_was_space {
                out.push(' ');
                last_was_space = true;
            }
        } else {
            out.push(c);
            last_was_space = false;
            last_was_newline = false;
        }
    }
    out
}

/// 是否值得保留：去噪后字符数 ≥ [`MIN_MEANINGFUL_CHARS`] 且至少有一个字母或中文字符。
fn is_meaningful(body: &str) -> bool {
    if body.chars().count() < MIN_MEANINGFUL_CHARS {
        return false;
    }
    body.chars().any(|c| c.is_alphabetic() || is_cjk(c))
}

/// 最小有意义字符数。
///
/// 2026-05-12 基线把它从 20 降到 8：99 obs 跑流水线时 76% 被这条阈值干掉，
/// 用户的"嗯""好的""ok 继续"这类反复确认信号是聚类要的高频证据。聚类层与
/// INDUCE 层会再做语义判断，STRIP 不需要在这里做语义过滤。
pub const MIN_MEANINGFUL_CHARS: usize = 8;
const MAX_METHODOLOGY_SIGNAL_CHARS: usize = 220;
const MAX_METHODOLOGY_SIGNALS_PER_OBSERVATION: usize = 8;

/// 是否为 CJK 中日韩统一汉字。
fn is_cjk(c: char) -> bool {
    matches!(c,
        '\u{3400}'..='\u{4DBF}' | '\u{4E00}'..='\u{9FFF}' | '\u{20000}'..='\u{2A6DF}'
    )
}

/// 已知 system prompt 模板：开头匹配以下任一前缀就 drop。
///
/// 这个清单是经验性的，会随着新 prompt 的出现增补。**不在 prompt 库里**而是
/// 写死代码，因为：1) 数量少；2) 增量调整需要 code review；3) 防止误杀真实
/// user 表达。
fn looks_like_system_prompt(body: &str) -> bool {
    const KNOWN_PROMPT_HEADS: &[&str] = &[
        // Agent Memory Kernel 自家 prompt
        "你是 Agent 长期记忆",
        "你是 Agent Memory Kernel",
        "You are helping build Agent Memory Kernel",
        "Extract only durable, high-value agent skills",
        // 子 agent 协作 prompt（Codex / Claude Code 多角色任务分派）
        "你是 Agent-Kernel",
        "You are Claude Code",
        "You are acting as",
        "You are a code review assistant",
        "You are an interactive agent",
        "You are a Team Member",
        "你是 Codex 的",
        "你是 Claude Code",
        "你是这个项目的",
        "你是 Agent-Kernel 的",
        "你是 Agent-Kernel 项目",
        // 一次性任务派发 prompt（含命令式动词）
        "Implement the next",
        "Implement the following",
        "Refactor `",
        "Return JSON:",
        "Model switch:",
        "请帮我重构",
        "请只读复审",
        "请只读运行",
        "请只读审查",
        "思考并回答下面的问题",
        "客观审查这个项目",
        // Anthropic CLI 模板
        "<command-name>",
        "<command-message>",
    ];
    let head: String = body.chars().take(120).collect();
    if KNOWN_PROMPT_HEADS
        .iter()
        .any(|prefix| head.contains(prefix))
    {
        return true;
    }
    // 兜底启发：开头"你是"+ 80 字符内出现 system-prompt 信号
    if head.starts_with("你是") || head.starts_with("# You are") || head.starts_with("You are ") {
        const SYSTEM_SIGNALS: &[&str] = &[
            "你的任务",
            "你的角色",
            "你的写入范围",
            "工作区",
            "项目路径",
            "请只读",
            "请只做",
            "Your role",
            "Your task",
            "Only modify",
            "Do not stage",
            "do not commit",
            "act as",
        ];
        let head_long: String = body.chars().take(220).collect();
        return SYSTEM_SIGNALS
            .iter()
            .any(|signal| head_long.contains(signal));
    }
    false
}

/// 归一化用于跨 observation 去重：删空白/标点，保留字母数字与 CJK，转小写。
///
/// 同一段话被复制到多个 session、加了不同空白，归一化后应该完全相等。
fn normalize_for_dedup(body: &str) -> String {
    body.chars()
        .filter(|c| c.is_alphanumeric() || is_cjk(*c))
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// 推断角色：仅基于文本启发，不查 jsonl meta。
///
/// 当前规则：
/// - 开头含"你是" / "Your task" / "You are" → `system`（应该已经被 [`looks_like_system_prompt`] 拦走，这里是兜底）
/// - 其他 → `unknown`（Layer 4 INDUCE 不需要严格区分 user/assistant，作为 evidence 都可以引用）
fn infer_role(_source_kind: &str, body: &str) -> String {
    let head: String = body.chars().take(40).collect();
    if head.contains("你是") || head.contains("Your task") || head.contains("You are") {
        "system".to_string()
    } else {
        "unknown".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs(id: &str, body: &str) -> ObservationRecord {
        ObservationRecord {
            id: id.to_string(),
            source_kind: "claude-code-session".to_string(),
            source_path: "test.jsonl".to_string(),
            agent: Some("claude-code".to_string()),
            body: body.to_string(),
            evidence: "test".to_string(),
            redacted: false,
            created_at: "2026-05-01T00:00:00+00:00".to_string(),
        }
    }

    #[test]
    fn strips_system_reminder_block_and_keeps_surrounding_text() {
        let records = vec![obs(
            "o1",
            "我希望你帮我重构这块代码。\n<system-reminder>临时注入</system-reminder>\n要保留可读性。",
        )];
        let out = strip_observations(&records);
        assert_eq!(out.len(), 1);
        assert!(!out[0].body.contains("临时注入"));
        assert!(out[0].body.contains("我希望你帮我重构这块代码"));
        assert!(out[0].body.contains("要保留可读性"));
    }

    #[test]
    fn strips_function_calls_block() {
        let records = vec![obs(
            "o1",
            "请帮我查一下当前目录下所有 yml 文件的大小。\n<function_calls>\n<invoke name=\"Bash\">\n</invoke>\n</function_calls>\n查完之后请告诉我哪一个文件最大。",
        )];
        let out = strip_observations(&records);
        assert_eq!(out.len(), 1);
        assert!(!out[0].body.contains("invoke"));
        assert!(out[0].body.contains("请帮我查一下当前目录"));
    }

    #[test]
    fn strips_unclosed_block_safely() {
        let records = vec![obs(
            "o1",
            "下面这段是一段正常的开头描述能通过最小长度阈值。\n<tool_use>没有闭合的标签后面的内容应该被丢弃但不会 panic",
        )];
        let out = strip_observations(&records);
        assert_eq!(out.len(), 1);
        assert!(out[0].body.contains("正常的开头描述"));
        assert!(!out[0].body.contains("丢弃"));
    }

    #[test]
    fn dedups_messages_with_same_normalized_text() {
        let records = vec![
            obs("o1", "评估提炼质量时，优先用真实历史会话做回归验证。"),
            obs("o2", "评估提炼质量时，   优先用真实历史会话做回归验证。\n"),
        ];
        let out = strip_observations(&records);
        assert_eq!(out.len(), 1, "duplicate body should be deduped");
    }

    #[test]
    fn drops_short_messages() {
        let records = vec![
            obs("o1", "好的"),
            obs("o2", "ok"),
            obs("o3", "这是一条足够长的有意义中文消息能被保留下来。"),
        ];
        let out = strip_observations(&records);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].observation_id, "o3");
    }

    #[test]
    fn drops_pure_punctuation() {
        let records = vec![obs("o1", "............-------------,,,,")];
        let out = strip_observations(&records);
        assert!(out.is_empty(), "pure punctuation should be dropped");
    }

    #[test]
    fn drops_known_system_prompt_template() {
        let records = vec![obs(
            "o1",
            "你是 Agent 长期记忆的最终质检与改写器。\n\n目标：把候选转写成高质量 memory_card",
        )];
        let out = strip_observations(&records);
        assert!(out.is_empty(), "known system prompt should be filtered out");
    }

    #[test]
    fn keeps_user_request_from_claude_wrapped_prompt() {
        let records = vec![obs(
            "o1",
            "[Assistant Rules - You MUST follow these instructions]\nnoise\n[User Request]\n先检查现有代码是否能实现这个功能，要求最小化修改，不要改太多。\nWeb search results for query: example\nirrelevant search output",
        )];
        let out = strip_observations(&records);
        assert_eq!(out.len(), 1);
        assert!(out[0].body.contains("先检查现有代码"));
        assert!(!out[0].body.contains("Assistant Rules"));
        assert!(!out[0].body.contains("Web search results"));
    }

    #[test]
    fn extracts_user_text_from_codex_jsonl_observation() {
        let records = vec![obs(
            "o1",
            "{\"timestamp\":\"2026-04-26T06:45:00.000Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"user\",\"content\":[{\"type\":\"input_text\",\"text\":\"修复前先判断问题是否真实存在，并补一个回归测试。\"}]}}\n{\"timestamp\":\"2026-04-26T06:45:35.742Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"我准备改成主进程周期性按真实光标位置双向纠正，并补一个回归测试。\"}]}}\n{\"timestamp\":\"2026-04-26T06:46:00.000Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"agent_message\",\"message\":\"运行测试确保无回归。\"}}",
        )];
        let out = strip_observations(&records);
        assert_eq!(out.len(), 1);
        assert!(out[0].body.contains("问题是否真实存在"));
        assert!(!out[0].body.contains("真实光标位置"));
        assert!(!out[0].body.contains("运行测试确保无回归"));
        assert!(!out[0].body.contains("timestamp"));
        assert!(!out[0].body.contains("payload"));
    }

    #[test]
    fn splits_global_methodology_signals_out_of_long_observation() {
        let records = vec![obs(
            "o1",
            "项目实现细节很多。\n重视真实历史回归，不迷信静态样例。\n提交前自己用推理引擎跑一下真实结果，检查有没有问题，质量高不高，然后分析原因再优化。\n小改快测，大改重测。\n协作阶段：先 review 再 merge、先保留审阅边界、不要让 AI 直接固化规则。\n后面继续说具体 UI 发布范围。",
        )];

        let out = strip_observations(&records);

        assert!(
            out.iter()
                .any(|message| message.observation_id.starts_with("o1#signal")
                    && message.body.contains("真实历史回归")),
            "{out:?}"
        );
        assert!(
            out.iter().any(|message| message.body.contains("推理引擎")),
            "{out:?}"
        );
        assert!(
            out.iter().any(|message| message.body.contains("小改快测")),
            "{out:?}"
        );
        assert!(
            out.iter().any(|message| message.body.contains("审阅边界")),
            "{out:?}"
        );
        assert!(out.iter().any(|message| message.observation_id == "o1"));
    }

    #[test]
    fn does_not_create_methodology_signal_for_plain_readonly_task_boundary() {
        let records = vec![obs(
            "o1",
            "只读任务：阅读 src/eval.rs 和 prompts/induce.md，不要改代码。输出当前流程概要和风险。",
        )];

        let out = strip_observations(&records);

        assert_eq!(out.len(), 1);
        assert_eq!(out[0].observation_id, "o1");
        assert!(!out[0].observation_id.contains("#signal"));
    }

    #[test]
    fn does_not_let_json_example_replace_plain_text_observation() {
        let records = vec![obs(
            "o1",
            "修改配置时请保留这条用户说明。\n{\"text\":\"这是示例 JSON，不是 observation 主体\"}",
        )];
        let out = strip_observations(&records);
        assert_eq!(out.len(), 1);
        assert!(out[0].body.contains("修改配置时请保留"));
        assert!(out[0].body.contains("示例 JSON"));
    }

    #[test]
    fn preserves_chinese_english_mix() {
        let records = vec![obs(
            "o1",
            "我希望前端 HTTP 请求统一用 axios，不要再用 fetch。",
        )];
        let out = strip_observations(&records);
        assert_eq!(out.len(), 1);
        assert!(out[0].body.contains("axios"));
        assert!(out[0].body.contains("fetch"));
        assert!(out[0].body.contains("前端"));
    }

    #[test]
    fn collapses_extra_whitespace_and_blank_lines() {
        let records = vec![obs(
            "o1",
            "第一行\n\n\n\n\n第二行包含足够字符够通过 is_meaningful。",
        )];
        let out = strip_observations(&records);
        assert_eq!(out.len(), 1);
        // 多个连续换行应合并为单个
        assert!(!out[0].body.contains("\n\n"));
        assert!(out[0].body.contains("第一行"));
        assert!(out[0].body.contains("第二行"));
    }
}
