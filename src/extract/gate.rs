use serde::{Deserialize, Serialize};

use super::chunk::EvidenceChunk;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateDecision {
    pub disposition: String,
    pub reason: String,
}

pub fn future_value_gate(chunk: &EvidenceChunk) -> GateDecision {
    let lower = chunk.text.to_lowercase();

    if looks_like_soft_personality_without_operational_trigger(&lower) {
        return reject("soft-personality-without-operational-trigger");
    }
    if looks_like_one_off_task(&lower) {
        return reject("one-off-task");
    }
    if looks_like_generic_advice(&lower) {
        return reject("generic-advice-without-local-trigger");
    }
    if looks_like_unresolved_request(&lower) {
        return reject("unresolved-request");
    }
    if looks_like_prompt_or_log_noise(&lower) {
        return reject("prompt-or-log-noise");
    }

    GateDecision {
        disposition: "pass".to_string(),
        reason: "future-value-gate-passed".to_string(),
    }
}

fn reject(reason: &str) -> GateDecision {
    GateDecision {
        disposition: "reject".to_string(),
        reason: reason.to_string(),
    }
}

fn looks_like_soft_personality_without_operational_trigger(lower: &str) -> bool {
    let soft = lower.contains("热情")
        || lower.contains("积极")
        || lower.contains("舒服")
        || lower.contains("品味")
        || lower.contains("personality")
        || lower.contains("friendly")
        || lower.contains("warm");
    let operational = lower.contains("测试")
        || lower.contains("test")
        || lower.contains("agent")
        || lower.contains("codex")
        || lower.contains("claude")
        || lower.contains("skill")
        || lower.contains("workflow")
        || lower.contains("流程")
        || lower.contains("文件")
        || lower.contains("代码")
        || lower.contains("ui");
    soft && !operational
}

fn looks_like_one_off_task(lower: &str) -> bool {
    lower.contains("这个按钮")
        || lower.contains("改成蓝色")
        || lower.contains("帮我看看")
        || lower.contains("修一下")
        || lower.contains("看一下页面")
        || lower.contains("只读")
        || lower.contains("不要写文件")
        || lower.contains("不要编辑文件")
        || lower.contains("不要提交")
        || lower.contains("只输出")
        || lower.contains("只修改")
        || lower.contains("仅修改")
        || lower.contains("本轮只允许")
        || lower.contains("这次只审查")
        || lower.contains("只审查 diff")
        || lower.contains("modify only")
        || lower.contains("read-only")
}

fn looks_like_generic_advice(lower: &str) -> bool {
    (lower.contains("代码整洁") && lower.contains("文档完善"))
        || lower.contains("软件项目应该")
        || lower.contains("best practices")
        || lower.contains("keep code clean")
}

fn looks_like_unresolved_request(lower: &str) -> bool {
    lower.contains("能不能")
        || lower.contains("先想想")
        || lower.contains("maybe")
        || lower.contains("有没有可能")
}

fn looks_like_prompt_or_log_noise(lower: &str) -> bool {
    lower.contains("base_instructions")
        || lower.contains("<local-command")
        || lower.contains("error[")
        || lower.contains("stack trace")
        || lower.contains("--> src/")
}
