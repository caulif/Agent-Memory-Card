use std::collections::BTreeSet;

use serde::Serialize;

use super::ObservationRecord;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum FailureSignalKind {
    PipelineBreak,
    FinalQualityCorrection,
    FalsePositiveNoise,
    PrivacyBoundary,
    ScopeBoundary,
    RefactorCorrection,
    TransferableWorkflow,
    ReviewIterateLoop,
}

impl FailureSignalKind {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::PipelineBreak => "pipeline_break",
            Self::FinalQualityCorrection => "final_quality_correction",
            Self::FalsePositiveNoise => "false_positive_noise",
            Self::PrivacyBoundary => "privacy_boundary",
            Self::ScopeBoundary => "scope_boundary",
            Self::RefactorCorrection => "refactor_correction",
            Self::TransferableWorkflow => "transferable_workflow",
            Self::ReviewIterateLoop => "review_iterate_loop",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct FailureSignal {
    pub(crate) kind: FailureSignalKind,
    pub(crate) observation_id: String,
    pub(crate) snippet: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct FailureFlowSummary {
    pub(crate) signals: Vec<FailureSignal>,
}

impl FailureFlowSummary {
    pub(crate) fn is_empty(&self) -> bool {
        self.signals.is_empty()
    }

    pub(crate) fn to_extraction_material(&self) -> String {
        let mut out = String::new();
        out.push_str("FailureFlowSummary:\n");
        for signal in &self.signals {
            out.push_str(&format!(
                "- signal:{} obs:{} text:{}\n",
                signal.kind.as_str(),
                signal.observation_id,
                signal.snippet
            ));
        }
        out
    }
}

pub(crate) fn summarize_failure_flow(observations: &[ObservationRecord]) -> FailureFlowSummary {
    let mut chronological = observations.to_vec();
    chronological.sort_by(|left, right| left.created_at.cmp(&right.created_at));
    let mut signals = Vec::new();

    for observation in &chronological {
        for line in observation.body.lines().map(str::trim) {
            if line.is_empty() || line.chars().count() < 8 {
                continue;
            }
            let lower = line.to_lowercase();
            if looks_like_generated_or_agent_chatter(&lower) {
                continue;
            }
            if looks_like_pipeline_break(&lower) {
                signals.push(signal(
                    FailureSignalKind::PipelineBreak,
                    observation.id.clone(),
                    line,
                ));
            }
            if looks_like_final_quality_correction(&lower) {
                signals.push(signal(
                    FailureSignalKind::FinalQualityCorrection,
                    observation.id.clone(),
                    line,
                ));
            }
            if looks_like_false_positive_noise(&lower) {
                signals.push(signal(
                    FailureSignalKind::FalsePositiveNoise,
                    observation.id.clone(),
                    line,
                ));
            }
            if looks_like_privacy_boundary(&lower) {
                signals.push(signal(
                    FailureSignalKind::PrivacyBoundary,
                    observation.id.clone(),
                    line,
                ));
            }
            if looks_like_scope_boundary(&lower) {
                signals.push(signal(
                    FailureSignalKind::ScopeBoundary,
                    observation.id.clone(),
                    line,
                ));
            }
            if looks_like_refactor_correction(&lower) {
                signals.push(signal(
                    FailureSignalKind::RefactorCorrection,
                    observation.id.clone(),
                    line,
                ));
            }
            if looks_like_transferable_workflow(&lower) {
                signals.push(signal(
                    FailureSignalKind::TransferableWorkflow,
                    observation.id.clone(),
                    line,
                ));
            }
            if looks_like_review_iterate_loop(&lower) {
                signals.push(signal(
                    FailureSignalKind::ReviewIterateLoop,
                    observation.id.clone(),
                    line,
                ));
            }
        }
    }

    dedupe_signals(&mut signals);
    FailureFlowSummary { signals }
}

fn signal(kind: FailureSignalKind, observation_id: String, text: &str) -> FailureSignal {
    FailureSignal {
        kind,
        observation_id,
        snippet: compact_snippet(text, 240),
    }
}

fn looks_like_pipeline_break(lower: &str) -> bool {
    let has_stage = contains_any(
        lower,
        &[
            "llm induction",
            "induction",
            "json",
            "解析失败",
            "parse",
            "crystallize",
            "候选为 0",
            "候选为0",
        ],
    );
    let has_break = contains_any(
        lower,
        &[
            "截断",
            "格式不完整",
            "没有进入",
            "失败",
            "truncated",
            "incomplete",
            "failed",
        ],
    );
    has_stage && has_break
}

fn looks_like_final_quality_correction(lower: &str) -> bool {
    contains_any(
        lower,
        &[
            "不能只看指标",
            "不要只看指标",
            "不止看指标",
            "自己看最终",
            "人工看",
            "质量太差",
            "最终卡片",
            "最终质量",
        ],
    ) && contains_any(lower, &["质量", "卡片", "候选", "指标", "结果"])
}

fn looks_like_false_positive_noise(lower: &str) -> bool {
    let false_positive = contains_any(
        lower,
        &["不应该存在", "不应该提取", "不应该计入", "不能固化", "误召"],
    );
    let noise = contains_any(
        lower,
        &[
            "代码分析",
            "实现细节",
            "extract.rs",
            "taxonomy",
            "优先保留稳定偏好",
            "抽取规则",
            "候选质量",
        ],
    );
    false_positive && noise
}

fn looks_like_privacy_boundary(lower: &str) -> bool {
    contains_any(lower, &["真实历史", "golden set", "对话数据", "dry-run"])
        && contains_any(
            lower,
            &[
                "只用于本地",
                "本地效果评估",
                "不要上传",
                "不要提交",
                "不进入 git",
            ],
        )
}

fn looks_like_scope_boundary(lower: &str) -> bool {
    contains_any(lower, &["全局", "局部", "项目", "global", "local"])
        && contains_any(lower, &["分层", "边界", "不要污染", "区分", "scope"])
        && contains_any(lower, &["记忆", "memory", "卡片"])
}

fn looks_like_refactor_correction(lower: &str) -> bool {
    contains_any(lower, &["重构", "拆分", "模块", "架构", "refactor"])
        && contains_any(
            lower,
            &[
                "纠偏",
                "犯过的错",
                "缺点",
                "问题",
                "质量太差",
                "优化",
                "改进",
            ],
        )
        && contains_any(lower, &["经验", "吸取", "流程", "工作流", "边界", "高质量"])
}

fn looks_like_transferable_workflow(lower: &str) -> bool {
    contains_any(
        lower,
        &[
            "迁移到其他项目",
            "其他项目",
            "全局偏好",
            "全局工作流",
            "自己的工作流",
            "可迁移",
            "通用",
        ],
    ) && contains_any(
        lower,
        &["项目偏好", "特殊偏好", "流程", "模块", "记忆卡片", "固化"],
    )
}

fn looks_like_review_iterate_loop(lower: &str) -> bool {
    contains_any(lower, &["审核", "人工", "自己看", "真实数据", "真实历史"])
        && contains_any(lower, &["分析问题", "解决方案", "修改", "优化", "反馈"])
        && contains_any(lower, &["直到", "符合预期", "结果", "生成了什么", "测试"])
}

fn looks_like_generated_or_agent_chatter(lower: &str) -> bool {
    contains_any(
        lower,
        &[
            "subagent_notification",
            "available skills",
            "generated by agent memory kernel",
            "do not edit directly",
            "我会先",
            "我正在",
            "我已经",
        ],
    )
}

fn dedupe_signals(signals: &mut Vec<FailureSignal>) {
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
    fn failure_flow_detects_pipeline_break_and_quality_correction() {
        let observations = vec![
            observation(
                "a",
                "2026-01-01T00:00:00Z",
                "LLM induction 阶段返回的 JSON 被截断/格式不完整，解析失败了，所以没有进入最终 crystallize 卡片阶段。",
            ),
            observation(
                "b",
                "2026-01-01T01:00:00Z",
                "不能只看指标，要自己看最终卡片质量。",
            ),
        ];

        let summary = summarize_failure_flow(&observations);
        let material = summary.to_extraction_material();

        assert!(material.contains("FailureFlowSummary:"), "{material}");
        assert!(material.contains("signal:pipeline_break"), "{material}");
        assert!(
            material.contains("signal:final_quality_correction"),
            "{material}"
        );
    }

    #[test]
    fn failure_flow_detects_noise_privacy_and_scope_boundaries() {
        let observations = vec![
            observation(
                "noise",
                "2026-01-01T00:00:00Z",
                "像“改 prompt 或 render，都要抽样看最终卡片，不要只看分数”这种话，就不应该存在，代码分析和抽取规则说明不能固化。",
            ),
            observation(
                "privacy",
                "2026-01-01T01:00:00Z",
                "Golden Set 只用于本地效果评估，不要把真实对话数据上传 git，真实历史 dry-run 只用于本地。",
            ),
            observation(
                "scope",
                "2026-01-01T02:00:00Z",
                "项目记忆和全局记忆要分层，区分局部流程和全局协作偏好，避免污染全局 Memory。",
            ),
        ];

        let material = summarize_failure_flow(&observations).to_extraction_material();

        assert!(
            material.contains("signal:false_positive_noise"),
            "{material}"
        );
        assert!(material.contains("signal:privacy_boundary"), "{material}");
        assert!(material.contains("signal:scope_boundary"), "{material}");
    }

    #[test]
    fn failure_flow_detects_refactor_transferable_and_review_loop_signals() {
        let observations = vec![
            observation(
                "refactor",
                "2026-01-01T00:00:00Z",
                "我有几次重构和纠偏，这些都可以吸取经验，形成高质量工作流里的模块。",
            ),
            observation(
                "transfer",
                "2026-01-01T01:00:00Z",
                "项目特殊偏好也可以提取或修改为全局偏好，迁移到其他项目或放进自己的工作流。",
            ),
            observation(
                "review",
                "2026-01-01T02:00:00Z",
                "测试的时候必须看看真实数据上生成了什么记忆卡片，然后结合目标审核，分析问题和解决方案，修改优化直到符合预期。",
            ),
        ];

        let material = summarize_failure_flow(&observations).to_extraction_material();

        assert!(
            material.contains("signal:refactor_correction"),
            "{material}"
        );
        assert!(
            material.contains("signal:transferable_workflow"),
            "{material}"
        );
        assert!(
            material.contains("signal:review_iterate_loop"),
            "{material}"
        );
    }
}
