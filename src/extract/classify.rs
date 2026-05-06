use serde::{Deserialize, Serialize};

use super::chunk::EvidenceChunk;

/// 知识分类：提取漏斗对证据块的分类结果。
///
/// 包含信号类型、目标制品、激活方式、硬度、控制方式、理由和结构标签。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct KnowledgeClassification {
    pub signal: String,
    pub artifact_kind: String,
    pub activation: String,
    pub hardness: String,
    pub control: String,
    pub rationale: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// 对证据块执行确定性分类，返回知识分类结果。
pub fn classify_chunk(chunk: &EvidenceChunk) -> KnowledgeClassification {
    let text = chunk.text.trim();
    let lower = text.to_lowercase();

    let signal = detect_signal(chunk, &lower);
    let artifact_kind = artifact_kind_for(&signal, &lower);
    let hardness = hardness_for(&signal, &artifact_kind, &lower);
    let activation = activation_for(&artifact_kind, &hardness);
    let control = control_for(&signal, &hardness, &lower);
    let mut tags = structured_tags(
        &signal,
        &artifact_kind,
        &activation,
        &hardness,
        &lower,
        chunk,
    );
    tags.sort();
    tags.dedup();

    KnowledgeClassification {
        rationale: rationale_for(&signal, &artifact_kind, &hardness),
        signal,
        artifact_kind,
        activation,
        hardness,
        control,
        tags,
    }
}

fn detect_signal(chunk: &EvidenceChunk, lower: &str) -> String {
    if looks_like_noise(lower) {
        return String::new();
    }
    if looks_like_accepted_ai_project_improvement(chunk, lower) {
        return "ai_project_improvement".to_string();
    }
    if looks_like_correction(lower) {
        return "correction".to_string();
    }
    if looks_like_decision(lower) {
        return "decision".to_string();
    }
    if looks_like_validation(lower) {
        return "validation".to_string();
    }
    if looks_like_template(lower) {
        return "template".to_string();
    }
    if looks_like_constraint(lower) {
        return "constraint".to_string();
    }
    if looks_like_preference(lower) {
        return "preference".to_string();
    }
    if looks_like_procedure(lower) {
        return "procedure".to_string();
    }
    String::new()
}

fn artifact_kind_for(signal: &str, lower: &str) -> String {
    match signal {
        "" => "reject",
        "ai_project_improvement" => "review_only",
        _ if lower.contains("skill_supplement")
            || lower.contains("supplement")
            || lower.contains("附加到已有")
            || lower.contains("补充到已有")
            || (lower.contains("skill") && lower.contains("补充")) =>
        {
            "skill_supplement"
        }
        "template" => "workflow_skill",
        "procedure" if lower.contains("claude code") && lower.contains("codex") => "workflow_skill",
        "procedure" => "workflow_skill",
        "validation" | "correction" | "constraint" | "decision" | "preference" => "always_on_rule",
        _ => "reject",
    }
    .to_string()
}

fn hardness_for(signal: &str, artifact_kind: &str, lower: &str) -> String {
    if lower.contains("token=")
        || lower.contains("secret")
        || lower.contains("credential")
        || lower.contains("删除")
        || lower.contains("rm -rf")
        || lower.contains("publish")
        || lower.contains("release")
    {
        return "critical".to_string();
    }
    if matches!(
        signal,
        "constraint" | "correction" | "decision" | "validation" | "ai_project_improvement"
    ) || artifact_kind == "review_only"
        || lower.contains("agents.md")
        || lower.contains("claude.md")
        || lower.contains("cargo test")
        || lower.contains("clippy")
    {
        return "high".to_string();
    }
    if matches!(artifact_kind, "workflow_skill" | "skill_supplement")
        || matches!(signal, "procedure" | "template")
    {
        return "medium".to_string();
    }
    "low".to_string()
}

fn activation_for(artifact_kind: &str, hardness: &str) -> String {
    match artifact_kind {
        "workflow_skill" | "skill_supplement" => "skill",
        "path_rule" => "path_glob",
        "review_only" if hardness == "critical" => "manual",
        "review_only" => "manual",
        "always_on_rule" => "always_on",
        _ => "model_decision",
    }
    .to_string()
}

fn control_for(signal: &str, hardness: &str, lower: &str) -> String {
    if matches!(signal, "constraint" | "ai_project_improvement")
        || lower.contains("不要")
        || lower.contains("do not")
    {
        return "prohibition".to_string();
    }
    if matches!(
        signal,
        "procedure" | "template" | "validation" | "correction"
    ) || lower.contains("先")
        || lower.contains("再")
        || lower.contains("then")
        || lower.contains("cargo test")
    {
        return "checklist".to_string();
    }
    if hardness == "high" || hardness == "critical" {
        return "exact_sequence".to_string();
    }
    if signal == "preference" {
        return "default".to_string();
    }
    "principle".to_string()
}

#[allow(clippy::too_many_arguments)]
fn structured_tags(
    signal: &str,
    artifact_kind: &str,
    activation: &str,
    hardness: &str,
    lower: &str,
    _chunk: &EvidenceChunk,
) -> Vec<String> {
    if signal.is_empty() || artifact_kind == "reject" {
        return Vec::new();
    }

    let shape_tag = if signal == "ai_project_improvement" {
        if lower.contains("evidencechunk") || lower.contains("漏斗") {
            "decision"
        } else if lower.contains("不是直接写") {
            "constraint"
        } else {
            signal
        }
    } else {
        signal
    };
    let mut tags = vec![
        format!("shape:{shape_tag}"),
        format!("hardness:{hardness}"),
        format!("activation:{}", activation.replace('_', "-")),
    ];

    if artifact_kind == "always_on_rule" {
        tags.push("target:agents-md".to_string());
    }
    if artifact_kind == "workflow_skill" || artifact_kind == "skill_supplement" {
        tags.push("target:skill-dir".to_string());
    }
    if lower.contains("codex") {
        tags.push("target:codex".to_string());
    }
    if lower.contains("claude") {
        tags.push("target:claude-code".to_string());
    }
    if lower.contains("rust") || lower.contains("cargo") {
        tags.push("domain:rust".to_string());
    }
    if lower.contains("tauri") {
        tags.push("domain:tauri".to_string());
    }
    if lower.contains("react")
        || lower.contains("frontend")
        || lower.contains("前端")
        || lower.contains("ui")
    {
        tags.push("domain:frontend".to_string());
    }
    if lower.contains("ui") || lower.contains("界面") || lower.contains("视觉") {
        tags.push("domain:ui".to_string());
    }
    if lower.contains("test")
        || lower.contains("测试")
        || lower.contains("fixture")
        || lower.contains("clippy")
    {
        tags.push("domain:testing".to_string());
    }
    if lower.contains("bun")
        || lower.contains("npm")
        || lower.contains("pnpm")
        || lower.contains("yarn")
        || lower.contains("package")
        || lower.contains("脚本")
    {
        tags.push("domain:build".to_string());
    }
    if lower.contains("agents.md")
        || lower.contains("claude.md")
        || lower.contains("draft")
        || lower.contains("skilllet")
        || lower.contains("审阅")
    {
        tags.push("domain:governance".to_string());
    }
    if lower.contains("agent")
        || lower.contains("codex")
        || lower.contains("claude")
        || lower.contains("智能体")
    {
        tags.push("domain:agent-behavior".to_string());
    }
    if lower.contains("architecture")
        || lower.contains("架构")
        || lower.contains("database")
        || lower.contains("file-native")
    {
        tags.push("domain:architecture".to_string());
    }
    if lower.contains("evidencechunk")
        || lower.contains("candidate")
        || lower.contains("noise gate")
        || lower.contains("提取")
    {
        tags.push("domain:extraction".to_string());
    }
    // evidence:accepted-ai 标记：当文本明确包含用户确认接受 AI 改善时，
    // 不依赖 origin 来判断（显式"用户确认"文本本身就是证据）。
    if signal == "ai_project_improvement" && lower.contains("用户确认") {
        tags.push("evidence:accepted-ai".to_string());
    }
    if matches!(signal, "correction") {
        tags.push("evidence:user-correction".to_string());
    }
    if matches!(signal, "validation" | "correction") {
        tags.push("shape:validation".to_string());
    }

    tags
}

fn rationale_for(signal: &str, artifact_kind: &str, hardness: &str) -> String {
    if signal.is_empty() || artifact_kind == "reject" {
        return "Rejected because the text does not contain durable reusable agent knowledge."
            .to_string();
    }
    format!("Classified as {signal} for {artifact_kind} with {hardness} hardness.")
}

fn looks_like_noise(lower: &str) -> bool {
    let generic = lower.contains("代码整洁") && lower.contains("文档完善");
    let one_off =
        lower.contains("这个按钮") || lower.contains("改成蓝色") || lower.contains("帮我看看");
    let unresolved =
        lower.contains("能不能") || lower.contains("先想想") || lower.contains("maybe");
    let shell =
        lower.contains("error[") || lower.contains("stack trace") || lower.contains("--> src/");
    generic || one_off || unresolved || shell
}

fn looks_like_accepted_ai_project_improvement(_chunk: &EvidenceChunk, lower: &str) -> bool {
    // Accept regardless of origin when explicit acceptance markers and
    // project/governance terms are present. The triple-condition gate below
    // (accepted + project + governance) is narrow enough to reject one-off/generic/noise.
    let accepted =
        lower.contains("用户确认") || lower.contains("基于用户确认") || lower.contains("accepted");
    let project_terms = lower.contains("candidate")
        || lower.contains("draft")
        || lower.contains("skilllet")
        || lower.contains("evidencechunk")
        || lower.contains("项目");
    let governance_terms = lower.contains("审阅")
        || lower.contains("review")
        || lower.contains("noise gate")
        || lower.contains("value score")
        || lower.contains("直接写")
        || lower.contains("项目核心");
    accepted && project_terms && governance_terms
}

fn looks_like_correction(lower: &str) -> bool {
    let has_prevention_marker = lower.contains("以后")
        || lower.contains("下次")
        || lower.contains("必须")
        || lower.contains("先")
        || lower.contains("再")
        || lower.contains("always")
        || lower.contains("before")
        || lower.contains("不要再");
    (lower.contains("忘了") || lower.contains("again") || lower.contains("不要再"))
        && (lower.contains("测试")
            || lower.contains("test")
            || lower.contains("规则")
            || lower.contains("fixture"))
        && has_prevention_marker
}

fn looks_like_decision(lower: &str) -> bool {
    let decision = lower.contains("keep ")
        || lower.contains("decided")
        || lower.contains("chose")
        || lower.contains("决定")
        || lower.contains("选用");
    let project = lower.contains("file-native")
        || lower.contains("database")
        || lower.contains("vector")
        || lower.contains("architecture")
        || lower.contains("架构")
        || lower.contains("rebuildable")
        || lower.contains("v1");
    decision && project
}

fn looks_like_validation(lower: &str) -> bool {
    (lower.contains("cargo test")
        || lower.contains("clippy")
        || lower.contains("fixture")
        || lower.contains("build"))
        && (lower.contains("以后")
            || lower.contains("always")
            || lower.contains("必须")
            || lower.contains("before"))
}

fn looks_like_template(lower: &str) -> bool {
    (lower.contains("提示")
        || lower.contains("prompt")
        || lower.contains("清单")
        || lower.contains("handoff"))
        && (lower.contains("claude") || lower.contains("codex") || lower.contains("agent"))
}

fn looks_like_constraint(lower: &str) -> bool {
    if lower.contains("不要再") {
        return false;
    }
    lower.contains("不要")
        || lower.contains("禁止")
        || lower.contains("never")
        || lower.contains("do not")
        || lower.contains("don't")
        || lower.contains("must not")
}

fn looks_like_preference(lower: &str) -> bool {
    let durable = lower.contains("以后")
        || lower.contains("always")
        || lower.contains("prefer")
        || lower.starts_with("use ")
        || lower.contains(" use ")
        || lower.contains("默认")
        || lower.contains("统一")
        || lower.contains("优先");
    let tool = lower.contains("bun")
        || lower.contains("axios")
        || lower.contains("ky")
        || lower.contains("ofetch")
        || lower.contains("vitest")
        || lower.contains("playwright")
        || lower.contains("cargo")
        || lower.contains("npm")
        || lower.contains("pnpm")
        || lower.contains("yarn")
        || lower.contains("http")
        || lower.contains("请求")
        || lower.contains("fetch");
    durable && tool
}

fn looks_like_procedure(lower: &str) -> bool {
    (lower.contains("先") && lower.contains("再"))
        || (lower.contains("first") && lower.contains("then"))
        || lower.contains("流程")
        || lower.contains("workflow")
}
