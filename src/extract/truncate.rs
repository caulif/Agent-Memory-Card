//! Pipeline Layer 2：TRUNCATE 截断。
//!
//! 输入：[`StrippedMessage`] 列表（来自 Layer 1 STRIP）
//! 输出：[`TruncatedMessage`] 列表
//!
//! 长消息保留头部（场景描述）+ 尾部（结论 / 决策），中间折叠为
//! `[... 省略 N 字符 ...]` 标记。短消息原样通过。
//!
//! 不做：
//! - 去噪（Layer 1 已处理）
//! - 聚类（Layer 3）
//! - 任何 LLM 调用
//!
//! 阈值：默认 > 1500 字符触发截断；保头 750 + 保尾 750。这两个数字来自
//! V2 修订文档的"修订 6"附近经验值，可以通过 [`truncate_messages_with`]
//! 自定义。

use serde::{Deserialize, Serialize};

use crate::extract::strip::StrippedMessage;

/// 截断后的单条消息片段。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TruncatedMessage {
    /// 原 observation id（与 STRIP 一致，便于回溯）
    pub observation_id: String,

    /// role（与 STRIP 一致）
    pub role: String,

    /// 截断后的文本（含 `[... 省略 N 字符 ...]` 标记）
    pub body: String,

    /// STRIP 后的字节长度（截断前）
    pub original_len: usize,

    /// 截断后字节长度（含省略标记）
    pub truncated_len: usize,

    /// 是否实际触发了截断
    pub was_truncated: bool,

    /// observation 创建时间
    pub created_at: String,

    /// 原 source_kind
    pub source_kind: String,
}

/// 默认阈值：超过这么多字符就截断。
pub const DEFAULT_TRUNCATE_THRESHOLD_CHARS: usize = 1500;

/// 默认保留头部字符数。
pub const DEFAULT_HEAD_CHARS: usize = 750;

/// 默认保留尾部字符数。
pub const DEFAULT_TAIL_CHARS: usize = 750;

/// 流水线 Layer 2 主入口：用默认阈值截断长消息。
pub fn truncate_messages(messages: &[StrippedMessage]) -> Vec<TruncatedMessage> {
    truncate_messages_with(
        messages,
        DEFAULT_TRUNCATE_THRESHOLD_CHARS,
        DEFAULT_HEAD_CHARS,
        DEFAULT_TAIL_CHARS,
    )
}

/// 自定义阈值的截断入口（测试 / debug 用）。
pub fn truncate_messages_with(
    messages: &[StrippedMessage],
    keep_threshold_chars: usize,
    head_chars: usize,
    tail_chars: usize,
) -> Vec<TruncatedMessage> {
    messages
        .iter()
        .map(|m| truncate_one(m, keep_threshold_chars, head_chars, tail_chars))
        .collect()
}

fn truncate_one(
    message: &StrippedMessage,
    keep_threshold_chars: usize,
    head_chars: usize,
    tail_chars: usize,
) -> TruncatedMessage {
    let char_count = message.body.chars().count();
    if char_count <= keep_threshold_chars {
        return TruncatedMessage {
            observation_id: message.observation_id.clone(),
            role: message.role.clone(),
            body: message.body.clone(),
            original_len: message.stripped_len,
            truncated_len: message.stripped_len,
            was_truncated: false,
            created_at: message.created_at.clone(),
            source_kind: message.source_kind.clone(),
        };
    }
    let truncated = head_tail_truncate(&message.body, head_chars, tail_chars);
    let truncated_len = truncated.len();
    TruncatedMessage {
        observation_id: message.observation_id.clone(),
        role: message.role.clone(),
        body: truncated,
        original_len: message.stripped_len,
        truncated_len,
        was_truncated: true,
        created_at: message.created_at.clone(),
        source_kind: message.source_kind.clone(),
    }
}

/// 头尾截断：保留前 head_chars 字符 + 后 tail_chars 字符，中间用标记替代。
///
/// 基于字符数（chars()）而非字节数，UTF-8 边界自然正确。
fn head_tail_truncate(body: &str, head_chars: usize, tail_chars: usize) -> String {
    let total_chars = body.chars().count();
    if total_chars <= head_chars + tail_chars {
        return body.to_string();
    }
    let head: String = body.chars().take(head_chars).collect();
    let tail: String = body.chars().skip(total_chars - tail_chars).collect();
    let omitted = total_chars - head_chars - tail_chars;
    format!("{head}\n\n[... 省略 {omitted} 字符 ...]\n\n{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stripped(id: &str, body: &str) -> StrippedMessage {
        StrippedMessage {
            observation_id: id.to_string(),
            role: "unknown".to_string(),
            body: body.to_string(),
            original_len: body.len(),
            stripped_len: body.len(),
            created_at: "2026-05-01T00:00:00+00:00".to_string(),
            source_kind: "claude-code-session".to_string(),
        }
    }

    #[test]
    fn short_message_passes_through_unchanged() {
        let messages = vec![stripped("o1", "短消息能直接通过不变。")];
        let out = truncate_messages(&messages);
        assert_eq!(out.len(), 1);
        assert!(!out[0].was_truncated);
        assert_eq!(out[0].body, "短消息能直接通过不变。");
    }

    #[test]
    fn long_message_keeps_head_and_tail_with_marker() {
        let head_marker = "HEAD_SCENE_DESCRIPTION:";
        let tail_marker = "TAIL_CONCLUSION_HERE.";
        let middle_marker = "MIDDLE_SHOULD_BE_DROPPED";
        let body = format!(
            "{head_marker}{}{}{}{tail_marker}",
            "h".repeat(740),
            middle_marker.repeat(200),
            "t".repeat(740)
        );
        let messages = vec![stripped("o1", &body)];
        let out = truncate_messages(&messages);
        assert_eq!(out.len(), 1);
        assert!(out[0].was_truncated);
        assert!(out[0].body.contains(head_marker));
        assert!(out[0].body.contains(tail_marker));
        assert!(out[0].body.contains("[... 省略"));
        // 中间标记不应出现在头 750 与尾 750 中
        let head_slice: String = out[0].body.chars().take(750).collect();
        assert!(!head_slice.contains(middle_marker));
    }

    #[test]
    fn handles_chinese_utf8_boundary_correctly() {
        let body: String = "中文字符".repeat(2000);
        let messages = vec![stripped("o1", &body)];
        let out = truncate_messages(&messages);
        assert_eq!(out.len(), 1);
        assert!(out[0].was_truncated);
        // 确认输出依然是合法 UTF-8（不会截半个汉字）
        assert!(out[0].body.is_char_boundary(0));
        assert!(out[0].body.is_char_boundary(out[0].body.len()));
    }

    #[test]
    fn message_at_threshold_does_not_truncate() {
        let body: String = "a".repeat(DEFAULT_TRUNCATE_THRESHOLD_CHARS);
        let messages = vec![stripped("o1", &body)];
        let out = truncate_messages(&messages);
        assert!(!out[0].was_truncated);
    }

    #[test]
    fn message_one_over_threshold_triggers_truncate() {
        let body: String = "a".repeat(DEFAULT_TRUNCATE_THRESHOLD_CHARS + 1);
        let messages = vec![stripped("o1", &body)];
        let out = truncate_messages(&messages);
        assert!(out[0].was_truncated);
    }

    #[test]
    fn custom_thresholds_respected() {
        let body: String = "x".repeat(300);
        let messages = vec![stripped("o1", &body)];
        let out = truncate_messages_with(&messages, 100, 30, 30);
        assert_eq!(out.len(), 1);
        assert!(out[0].was_truncated);
        // head 30 + 省略标记 + tail 30
        let head: String = out[0].body.chars().take(30).collect();
        assert_eq!(head, "x".repeat(30));
    }

    #[test]
    fn order_and_metadata_preserved() {
        let messages = vec![
            stripped("o1", "first"),
            stripped("o2", "second"),
            stripped("o3", "third"),
        ];
        let out = truncate_messages(&messages);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].observation_id, "o1");
        assert_eq!(out[1].observation_id, "o2");
        assert_eq!(out[2].observation_id, "o3");
    }

    #[test]
    fn omitted_count_correct() {
        let body: String = "a".repeat(2500);
        let messages = vec![stripped("o1", &body)];
        let out = truncate_messages_with(&messages, 1500, 750, 750);
        assert!(out[0].body.contains("[... 省略 1000 字符 ...]"));
    }
}
