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
    if looks_like_isolated_aesthetic_fragment(&lower) {
        return reject("isolated-aesthetic-fragment");
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

fn looks_like_isolated_aesthetic_fragment(lower: &str) -> bool {
    if has_durable_scope_marker(lower) {
        return false;
    }

    let aesthetic = [
        "不要太丑",
        "太丑",
        "极简也可以很美",
        "不要太生硬",
        "人的味道",
        "情绪表达",
        "不好看",
        "美观",
        "审美",
    ]
    .iter()
    .any(|marker| lower.contains(marker));
    if !aesthetic {
        return false;
    }

    let operational_context = [
        "ui", "界面", "组件", "按钮", "布局", "readme", "文案", "帖子", "宣传", "写作", "生成",
        "输出", "设计", "产品",
    ]
    .iter()
    .any(|marker| lower.contains(marker));

    !operational_context || lower.starts_with("保留例外")
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
        || looks_like_one_off_project_execution_request(lower)
}

pub(crate) fn looks_like_one_off_project_execution_request(lower: &str) -> bool {
    let local_execution_goal = lower.contains("/goal")
        || lower.starts_with("goal ")
        || lower.contains("实现现有计划")
        || lower.contains("完成m0")
        || lower.contains("完成 m0")
        || lower.contains("自己选择")
        || lower.contains("直到完成整个项目");
    let current_skill_workflow_goal = (lower.contains("接下来用") || lower.contains("接下来使用"))
        && (lower.contains("skills") || lower.contains("skill"))
        && lower.contains("github")
        && !lower.contains("每次")
        && !lower.contains("所有项目");
    let local_eval_threshold = (lower.contains("top 10")
        || lower.contains("top-10")
        || lower.contains("至少 3")
        || lower.contains("至少-3"))
        && (lower.contains("dry-run") || lower.contains("真实历史"));
    let local_phase_choice = (lower.contains("我同意") || lower.contains("同意"))
        && (lower.contains("优先做") || lower.contains("先做"))
        && (lower.contains("再用") || lower.contains("再做"))
        && !lower.contains("每次")
        && !lower.contains("所有项目");
    let product_surface = lower.contains("拖动")
        || lower.contains("字幕")
        || lower.contains("声音")
        || lower.contains("按钮")
        || lower.contains("页面")
        || lower.contains("封面")
        || lower.contains("图标")
        || lower.contains("教程")
        || lower.contains("版本号")
        || lower.contains("监测")
        || lower.contains("标记过")
        || lower.contains("产品上还缺")
        || lower.contains("修复上面的问题");
    let local_product_feedback = ((lower.contains("上面")
        || lower.contains("下面")
        || lower.contains("这个")
        || lower.contains("这个项目")
        || lower.contains("当前")
        || lower.contains("现有"))
        && product_surface)
        || (product_surface
            && (lower.contains("不要")
                || lower.contains("应该")
                || lower.contains("统一")
                || lower.contains("只要")
                || lower.contains("保留例外")));
    let current_implementation_plan = (lower.contains("现有")
        || lower.contains("当前")
        || lower.contains("先把")
        || lower.contains("摊平")
        || lower.contains("管线"))
        && (lower.contains("实现")
            || lower.contains("修改")
            || lower.contains("测试面")
            || lower.contains("相关类型")
            || lower.contains("提炼管线"))
        && !lower.contains("以后")
        && !lower.contains("每次")
        && !lower.contains("所有项目");
    let current_failure_analysis = (lower.contains("为什么会这样")
        || lower.contains("先分析一下为什么")
        || lower.contains("解析失败")
        || lower.contains("格式不完整")
        || lower.contains("被截断"))
        && !lower.contains("以后")
        && !lower.contains("每次")
        && !lower.contains("所有项目");
    let contextless_exception = lower.contains("保留例外")
        && (lower.contains("不要下很多") || lower.contains("尽可能少下"));
    let imported_agent_instruction = lower.contains("do not revert edits made by")
        || lower.contains("css selectors in tests")
        || lower.contains("never use a code sent by")
        || lower.contains("generated classes")
        || lower.contains("only checks implementation details")
        || lower.contains("expected fail because")
        || lower.contains("if easy extract small handlers")
        || lower.contains("pet stage warning")
        || lower.contains("preserve existing builder responsibility");
    let agent_status_narration = (lower.starts_with("我会")
        || lower.starts_with("我先")
        || lower.starts_with("我已经")
        || lower.contains("\n我会")
        || lower.contains("\n我先")
        || lower.contains("\n我已经")
        || lower.contains("我已经用 spec-driven-develop")
        || lower.contains("我已经用-spec-driven-develop"))
        && !lower.contains("用户")
        && !lower.contains("我希望")
        && !lower.contains("我同意");

    if local_execution_goal
        || local_phase_choice
        || current_skill_workflow_goal
        || local_eval_threshold
        || local_product_feedback
        || current_implementation_plan
        || current_failure_analysis
        || contextless_exception
        || imported_agent_instruction
        || agent_status_narration
    {
        return true;
    }

    if has_durable_scope_marker(lower) {
        return false;
    }

    let request_surface = [
        "请",
        "帮我",
        "告诉我",
        "看看",
        "检查",
        "分析",
        "修改",
        "删除",
        "加上",
        "补一版",
        "打一个",
        "推送",
        "发布",
        "下载",
        "命名",
    ]
    .iter()
    .any(|marker| lower.contains(marker));
    let project_artifact = [
        "宣传帖",
        "帖子",
        "标题",
        "readme",
        "release",
        "tag",
        "截图",
        "开源",
        "仓库",
        "提交",
        "强推",
        "文案",
        "产品说明",
    ]
    .iter()
    .any(|marker| lower.contains(marker));
    let generic_analysis = (lower.contains("全面分析") || lower.contains("分析一下"))
        && (lower.contains("不足")
            || lower.contains("优化用户体验")
            || lower.contains("可以优化")
            || lower.contains("告诉我"));
    let copy_tone_request = (lower.contains("ai 味道")
        || lower.contains("ai味道")
        || lower.contains("生硬")
        || lower.contains("人的味道")
        || lower.contains("情绪表达"))
        && (lower.contains("写") || lower.contains("宣传") || lower.contains("帖子"));

    (request_surface && project_artifact) || generic_analysis || copy_tone_request
}

fn has_durable_scope_marker(lower: &str) -> bool {
    [
        "以后",
        "每次",
        "所有项目",
        "所有请求",
        "长期",
        "总是",
        "默认",
        "统一",
        "always",
        "for every",
        "for all",
        "by default",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
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
