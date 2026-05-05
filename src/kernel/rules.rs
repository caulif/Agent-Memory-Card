use serde::{Deserialize, Serialize};

use crate::kernel::policy::KernelRisk;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum KernelValue {
    Noise,
    Candidate,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleAssessment {
    pub value: KernelValue,
    pub risk: KernelRisk,
    pub tags: Vec<String>,
    pub reason: String,
}

pub fn assess_text(text: &str) -> RuleAssessment {
    let lower = text.to_lowercase();
    let mut tags = infer_tags(&lower);
    let value = infer_value(&lower, &tags);
    let risk = match value {
        KernelValue::Noise => KernelRisk::Low,
        KernelValue::Candidate => KernelRisk::Medium,
        KernelValue::High => KernelRisk::Medium,
    };
    if value == KernelValue::Noise && !tags.iter().any(|tag| tag == "noise") {
        tags.push("noise".to_string());
    }
    tags.sort();
    tags.dedup();

    RuleAssessment {
        value: value.clone(),
        risk,
        tags,
        reason: match value {
            KernelValue::Noise => {
                "Rule engine classified this as one-off task chatter.".to_string()
            }
            KernelValue::Candidate => {
                "Rule engine found reusable signals, but confidence needs review.".to_string()
            }
            KernelValue::High => {
                "Rule engine found durable preference, workflow, or agent handoff language."
                    .to_string()
            }
        },
    }
}

fn infer_value(lower: &str, tags: &[String]) -> KernelValue {
    let rule_markers = [
        "以后",
        "必须",
        "不要",
        "默认",
        "prefer",
        "always",
        "never",
        "workflow",
        "checklist",
        "每次",
    ];
    let high_value_markers = [
        "复用", "沉淀", "流程", "测试", "handoff", "claude", "codex", "skill", "skilllet", "架构",
    ];
    let noise_markers = ["继续", "优化一下", "修一下", "再来", "改一下", "看一下"];

    if rule_markers.iter().any(|marker| lower.contains(marker))
        || high_value_markers
            .iter()
            .any(|marker| lower.contains(marker))
        || tags.len() >= 2
    {
        return KernelValue::High;
    }
    if noise_markers.iter().any(|marker| lower.contains(marker)) && lower.chars().count() <= 32 {
        return KernelValue::Noise;
    }
    KernelValue::Candidate
}

fn infer_tags(lower: &str) -> Vec<String> {
    let mappings: [(&str, &[&str]); 12] = [
        ("ui-design", &["ui", "界面", "设计", "视觉", "按钮", "组件"]),
        (
            "frontend",
            &["frontend", "前端", "react", "typescript", "axios"],
        ),
        ("backend", &["backend", "后端", "api", "server"]),
        ("rust", &["rust", "cargo", "clippy"]),
        ("tauri", &["tauri", "desktop", "桌面"]),
        (
            "testing",
            &["test", "testing", "vitest", "playwright", "测试"],
        ),
        (
            "performance",
            &["performance", "性能", "卡顿", "卡死", "responsive"],
        ),
        (
            "agent-handoff",
            &["claude", "codex", "agent", "智能体", "交接"],
        ),
        ("code-style", &["style", "lint", "格式", "代码风格"]),
        (
            "workflow",
            &["workflow", "流程", "步骤", "procedure", "checklist"],
        ),
        ("safety", &["safety", "安全", "secret", "redact", "禁止"]),
        ("skilllet", &["skilllet", "skill", "技能"]),
    ];
    mappings
        .iter()
        .filter(|(_, markers)| markers.iter().any(|marker| lower.contains(marker)))
        .map(|(tag, _)| (*tag).to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_filter_low_value_task_chatter() {
        let assessment = assess_text("继续优化一下，然后再修一下这个按钮");

        assert_eq!(assessment.value, KernelValue::Noise);
        assert!(assessment.tags.contains(&"noise".to_string()));
        assert_eq!(assessment.risk, KernelRisk::Low);
    }

    #[test]
    fn rules_tag_high_value_agent_handoff_and_ui_design() {
        let assessment =
            assess_text("以后 UI 改动都先调用 Claude Code 做视觉优化，然后 Codex 审核并运行测试。");

        assert_eq!(assessment.value, KernelValue::High);
        assert!(assessment.tags.contains(&"ui-design".to_string()));
        assert!(assessment.tags.contains(&"agent-handoff".to_string()));
        assert!(assessment.tags.contains(&"testing".to_string()));
    }
}
