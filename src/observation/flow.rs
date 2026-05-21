use std::collections::BTreeSet;

use serde::Serialize;

use super::ObservationRecord;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ConversationStage {
    Startup,
    Exploration,
    Implementation,
    Review,
    Closure,
}

impl ConversationStage {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Startup => "startup",
            Self::Exploration => "exploration",
            Self::Implementation => "implementation",
            Self::Review => "review",
            Self::Closure => "closure",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum FlowSignalKind {
    UserDirect,
    AcceptedOffer,
    Correction,
    RepeatedPattern,
    ValidationFeedback,
}

impl FlowSignalKind {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::UserDirect => "user_direct",
            Self::AcceptedOffer => "accepted_offer",
            Self::Correction => "correction",
            Self::RepeatedPattern => "repeated_pattern",
            Self::ValidationFeedback => "validation_feedback",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct FlowSignal {
    pub(crate) kind: FlowSignalKind,
    pub(crate) observation_id: String,
    pub(crate) snippet: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ConversationFlowSegment {
    pub(crate) stage: ConversationStage,
    pub(crate) observation_ids: Vec<String>,
    pub(crate) signals: Vec<FlowSignal>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ConversationFlowSummary {
    pub(crate) segments: Vec<ConversationFlowSegment>,
}

impl ConversationFlowSummary {
    pub(crate) fn is_empty(&self) -> bool {
        self.segments
            .iter()
            .all(|segment| segment.signals.is_empty())
    }

    pub(crate) fn to_extraction_material(&self) -> String {
        let mut out = String::new();
        out.push_str("ConversationFlowSummary:\n");
        for segment in &self.segments {
            if segment.signals.is_empty() {
                continue;
            }
            out.push_str(&format!("\n[stage:{}]\n", segment.stage.as_str()));
            for signal in &segment.signals {
                out.push_str(&format!(
                    "- signal:{} obs:{} text:{}\n",
                    signal.kind.as_str(),
                    signal.observation_id,
                    signal.snippet
                ));
            }
        }
        out
    }
}

pub(crate) fn summarize_conversation_flow(
    observations: &[ObservationRecord],
) -> ConversationFlowSummary {
    let mut chronological = observations.to_vec();
    chronological.sort_by(|left, right| left.created_at.cmp(&right.created_at));

    let total = chronological.len();
    let mut segments = vec![
        ConversationFlowSegment {
            stage: ConversationStage::Startup,
            observation_ids: Vec::new(),
            signals: Vec::new(),
        },
        ConversationFlowSegment {
            stage: ConversationStage::Exploration,
            observation_ids: Vec::new(),
            signals: Vec::new(),
        },
        ConversationFlowSegment {
            stage: ConversationStage::Implementation,
            observation_ids: Vec::new(),
            signals: Vec::new(),
        },
        ConversationFlowSegment {
            stage: ConversationStage::Review,
            observation_ids: Vec::new(),
            signals: Vec::new(),
        },
        ConversationFlowSegment {
            stage: ConversationStage::Closure,
            observation_ids: Vec::new(),
            signals: Vec::new(),
        },
    ];

    for (index, observation) in chronological.iter().enumerate() {
        let stage = stage_for_position(index, total);
        let segment = segments
            .iter_mut()
            .find(|segment| segment.stage == stage)
            .expect("stage segment exists");
        segment.observation_ids.push(observation.id.clone());
        segment
            .signals
            .extend(flow_signals_for_observation(observation, &segment.stage));
    }

    for segment in &mut segments {
        dedupe_signals(&mut segment.signals);
    }

    ConversationFlowSummary { segments }
}

fn stage_for_position(index: usize, total: usize) -> ConversationStage {
    if total <= 1 {
        return ConversationStage::Startup;
    }
    let ratio = index as f32 / total as f32;
    if ratio < 0.20 {
        ConversationStage::Startup
    } else if ratio < 0.45 {
        ConversationStage::Exploration
    } else if ratio < 0.75 {
        ConversationStage::Implementation
    } else if ratio < 0.90 {
        ConversationStage::Review
    } else {
        ConversationStage::Closure
    }
}

fn flow_signals_for_observation(
    observation: &ObservationRecord,
    stage: &ConversationStage,
) -> Vec<FlowSignal> {
    let lower = observation.body.to_lowercase();
    if looks_like_generated_or_task_chatter(&lower) {
        return Vec::new();
    }

    let mut signals = Vec::new();
    for line in observation.body.lines().map(str::trim) {
        if line.is_empty() || line.chars().count() < 8 {
            continue;
        }
        let line_lower = line.to_lowercase();
        if looks_like_generated_or_task_chatter(&line_lower) {
            continue;
        }

        let kind = if looks_like_accepted_offer(&line_lower) {
            Some(FlowSignalKind::AcceptedOffer)
        } else if looks_like_correction(&line_lower) {
            Some(FlowSignalKind::Correction)
        } else if looks_like_validation_feedback(&line_lower, stage) {
            Some(FlowSignalKind::ValidationFeedback)
        } else if looks_like_repeated_pattern(&line_lower) {
            Some(FlowSignalKind::RepeatedPattern)
        } else if looks_like_user_direct(&line_lower, stage) {
            Some(FlowSignalKind::UserDirect)
        } else {
            None
        };

        if let Some(kind) = kind {
            signals.push(FlowSignal {
                kind,
                observation_id: observation.id.clone(),
                snippet: compact_snippet(line, 240),
            });
        }
    }

    signals
}

fn looks_like_user_direct(lower: &str, stage: &ConversationStage) -> bool {
    let durable = contains_any(
        lower,
        &[
            "以后", "每次", "默认", "长期", "优先", "不要", "必须", "always", "prefer", "must",
            "do not",
        ],
    );
    let planning = contains_any(
        lower,
        &[
            "规划",
            "方案",
            "启动",
            "开源项目",
            "同类产品",
            "参考",
            "借鉴",
            "可视化",
            "mockup",
            "流程图",
            "架构图",
        ],
    );
    let validation = contains_any(
        lower,
        &["真实历史", "dry-run", "测试", "验收", "用户视角", "质量"],
    );

    durable || matches!(stage, ConversationStage::Startup) && planning || validation
}

fn looks_like_accepted_offer(lower: &str) -> bool {
    let acceptance = contains_any(
        lower,
        &[
            "我同意",
            "同意",
            "可以",
            "就按这个",
            "按这个方式",
            "这样可以",
            "yes",
            "sounds good",
        ],
    );
    let offered_method = contains_any(
        lower,
        &[
            "可视化",
            "mockup",
            "对比图",
            "流程图",
            "架构图",
            "开源项目",
            "同类产品",
            "参考",
            "借鉴",
            "规划",
            "测试",
            "dry-run",
        ],
    );
    acceptance && offered_method
}

fn looks_like_correction(lower: &str) -> bool {
    contains_any(
        lower,
        &[
            "不是",
            "不对",
            "应该",
            "为什么没有",
            "漏掉",
            "质量太差",
            "不能只",
            "不应该",
            "不要只",
        ],
    ) && contains_any(
        lower,
        &[
            "以后", "每次", "质量", "提取", "提炼", "生成", "测试", "验收",
        ],
    )
}

fn looks_like_repeated_pattern(lower: &str) -> bool {
    contains_any(lower, &["一般", "通常", "每段对话", "开头", "中间", "最后"])
        && contains_any(lower, &["长期", "规划", "偏好", "测试", "验收", "值得关注"])
}

fn looks_like_validation_feedback(lower: &str, stage: &ConversationStage) -> bool {
    let validation = contains_any(
        lower,
        &[
            "测试",
            "验收",
            "人工看",
            "自己看",
            "最终质量",
            "用户视角",
            "需求覆盖",
            "真实历史",
            "dry-run",
        ],
    );
    validation
        && (matches!(
            stage,
            ConversationStage::Implementation
                | ConversationStage::Review
                | ConversationStage::Closure
        ) || contains_any(lower, &["以后", "每次", "必须", "不要只"]))
}

fn looks_like_generated_or_task_chatter(lower: &str) -> bool {
    contains_any(
        lower,
        &[
            "/goal",
            "subagent_notification",
            "只读任务",
            "scope for this run",
            "read these files first",
            "generated by agent memory kernel",
            "do not edit directly",
            "available skills",
            "assistant rules",
            "this session is being continued",
            "我会先",
            "我已经",
            "我会把这轮",
            "我会按",
        ],
    ) || ((lower.contains("top 10") || lower.contains("top-10")) && lower.contains("真实历史"))
}

fn dedupe_signals(signals: &mut Vec<FlowSignal>) {
    let mut seen = BTreeSet::new();
    signals.retain(|signal| {
        let key = format!(
            "{}:{}",
            signal.kind.as_str(),
            signal.snippet.trim().to_lowercase()
        );
        seen.insert(key)
    });
}

fn compact_snippet(text: &str, max_chars: usize) -> String {
    let cleaned = text.replace(['\0', '\n'], " ");
    if cleaned.chars().count() <= max_chars {
        return cleaned;
    }
    let head = cleaned.chars().take(max_chars).collect::<String>();
    format!("{head}...")
}

fn contains_any(text: &str, markers: &[&str]) -> bool {
    markers.iter().any(|marker| text.contains(marker))
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
    fn flow_summary_preserves_startup_planning_signal() {
        let observations = vec![
            observation(
                "startup",
                "2026-01-01T00:00:00Z",
                "我同意，此外有可以借鉴的开源项目或者任何内容也可以借鉴。",
            ),
            observation("middle", "2026-01-01T01:00:00Z", "开始实现。"),
            observation("end", "2026-01-01T02:00:00Z", "测试通过。"),
        ];

        let summary = summarize_conversation_flow(&observations);
        let material = summary.to_extraction_material();

        assert!(material.contains("stage:startup"), "{material}");
        assert!(material.contains("开源项目"), "{material}");
        assert!(material.contains("accepted_offer"), "{material}");
    }

    #[test]
    fn flow_summary_marks_middle_correction_and_final_validation() {
        let observations = vec![
            observation("a", "2026-01-01T00:00:00Z", "先规划一下整体方案。"),
            observation("b", "2026-01-01T01:00:00Z", "开始修改代码。"),
            observation(
                "c",
                "2026-01-01T02:00:00Z",
                "不是只看指标，最终质量你也要自己看一下。",
            ),
            observation(
                "d",
                "2026-01-01T03:00:00Z",
                "最后用真实历史 dry-run 测试，并从用户视角验收。",
            ),
        ];

        let summary = summarize_conversation_flow(&observations);
        let material = summary.to_extraction_material();

        assert!(material.contains("correction"), "{material}");
        assert!(material.contains("validation_feedback"), "{material}");
        assert!(material.contains("dry-run"), "{material}");
    }

    #[test]
    fn flow_summary_filters_execution_chatter() {
        let observations = vec![observation(
            "goal",
            "2026-01-01T00:00:00Z",
            "/goal 接下来用这个 skills 结合 github 进行规划和实现，直到完成",
        )];

        let summary = summarize_conversation_flow(&observations);

        assert!(summary.is_empty(), "{summary:#?}");
    }
}
