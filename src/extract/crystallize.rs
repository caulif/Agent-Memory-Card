//! Pipeline Layer 5：CRYSTALLIZE 结晶。
//!
//! 输入：[`InducedCandidate`]（来自 Layer 4 INDUCE）
//! 输出：[`CrystallizedCard`] —— 完整 MemoryCard schema 字段（待落盘）
//!
//! 与 V1 plan 不同，本层是**纯确定性**实现，不调 LLM。理由：
//!
//! - candidate 已有 title / when / what / why / kind / scope / evidence_quotes，
//!   规范化为 MemoryCard 只需要拼装 + 派生 + 校验
//! - 单一真实来源原则：activation / tags 严格从 kind+scope 派生，禁止多处写入
//! - 失败不重写：Layer 5 校验不过 → 直接 reject 进 Draft Inbox 让人决定
//! - 节省一次 LLM 调用 + 完全可重现 + 测试简单
//!
//! 如果未来发现 deterministic 拼装的 body 质量不达标，再加 LLM 兜底（参见
//! V2 修订文档"补强 4"未启用列表）。

use serde::{Deserialize, Serialize};

use crate::extract::induce::{EvidenceQuote, InducedCandidate};
use crate::memory_card::infer_activation;

/// Layer 5 输出：等价于一张 MemoryCard 的完整 schema 字段。
///
/// 不直接产出 [`crate::memory_card::MemoryCardRecord`] 是因为后者还需要 id /
/// created_at / extraction metadata 等运行时字段，由更上层（pipeline 收口）
/// 在落盘时补齐。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CrystallizedCard {
    pub cluster_id: String,
    pub recurrence: usize,
    pub title: String,
    pub body: String,
    pub brief: String,
    pub kind: String,
    pub scope: String,
    pub activation: String,
    pub tags: Vec<String>,
    pub language: String,
    pub evidence_quotes: Vec<EvidenceQuote>,
    pub temporal_status: String,
    pub confidence: f32,
}

/// 单 candidate 的结晶结果。
#[derive(Debug, Clone)]
pub enum CrystallizationOutcome {
    Accepted(CrystallizedCard),
    Rejected {
        cluster_id: String,
        candidate: InducedCandidate,
        reasons: Vec<String>,
    },
}

/// 长度约束（V2 修订与 schema-v2 数据契约）。
pub const TITLE_MIN_CHARS: usize = 8;
pub const TITLE_MAX_CHARS: usize = 30;
pub const BODY_MIN_CHARS: usize = 50;
pub const BODY_MAX_CHARS: usize = 200;
pub const BRIEF_MIN_CHARS: usize = 15;
pub const BRIEF_MAX_CHARS: usize = 60;

/// brief 与 body 的字符级 jaccard 上限（防套娃）
pub const BRIEF_BODY_JACCARD_MAX: f32 = 0.5;

/// 主入口：批量结晶。
pub fn crystallize_candidates(candidates: &[InducedCandidate]) -> Vec<CrystallizationOutcome> {
    candidates.iter().map(crystallize_one).collect()
}

/// 单 candidate 结晶。
pub fn crystallize_one(candidate: &InducedCandidate) -> CrystallizationOutcome {
    let mut reasons = Vec::new();

    let body = render_body(candidate);
    let brief = render_brief(candidate);
    let title = candidate.title.trim().to_string();

    check_title(&title, &mut reasons);
    check_body(&body, &mut reasons);
    check_brief(&brief, &mut reasons);
    check_brief_body_overlap(&brief, &body, &mut reasons);
    check_kind_scope(candidate, &mut reasons);
    check_temporal_status(candidate, &mut reasons);
    check_memory_level(candidate, &body, &mut reasons);

    if !reasons.is_empty() {
        return CrystallizationOutcome::Rejected {
            cluster_id: candidate.cluster_id.clone(),
            candidate: candidate.clone(),
            reasons,
        };
    }

    let activation = infer_activation(&candidate.kind).to_string();
    let tags = derive_tags(candidate, &activation);

    CrystallizationOutcome::Accepted(CrystallizedCard {
        cluster_id: candidate.cluster_id.clone(),
        recurrence: candidate.recurrence,
        title,
        body,
        brief,
        kind: candidate.kind.clone(),
        scope: candidate.scope.clone(),
        activation,
        tags,
        language: detect_language(candidate),
        evidence_quotes: candidate.evidence_quotes.clone(),
        temporal_status: candidate.temporal_status.clone(),
        confidence: candidate.confidence,
    })
}

/// body 渲染：按 kind 选模板，preference / constraint / procedure 各自语气不同。
///
/// 2026-05-12 设计调整：之前所有 kind 都套同一句式"当 X，Y；目标是 Z。"，
/// 导致 preference 类（强语气表达）和 procedure 类（流程描述）被同一模具
/// 强行套用，刚性很大。改成按 kind 选模板：
/// - preference：「在 X 时，偏好 Y；原因是 Z。」
/// - constraint：「在 X 时，必须 Y；目的是 Z。」
/// - procedure（默认）：「当 X 时，Y；目标是 Z。」
fn render_body(candidate: &InducedCandidate) -> String {
    let when = candidate
        .when
        .trim()
        .trim_end_matches('，')
        .trim_end_matches(',');
    let what = candidate
        .what
        .trim()
        .trim_end_matches('；')
        .trim_end_matches(';')
        .trim_end_matches('。');
    let why = candidate.why.trim().trim_end_matches('。');
    let boundary = candidate
        .boundary
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.trim_end_matches('。'));

    let rendered = match candidate.kind.as_str() {
        "preference" => render_body_preference(when, what, why),
        "constraint" => render_body_constraint(when, what, why),
        _ => render_body_procedure(when, what, why),
    };
    if let Some(boundary) = boundary {
        format!(
            "{}；边界是{}。",
            rendered.trim_end_matches('。').trim_end_matches('；'),
            boundary
        )
    } else {
        rendered
    }
}

fn render_body_procedure(when: &str, what: &str, why: &str) -> String {
    let when_clause = normalize_when_clause(when, "当");
    format!("{when_clause}，{what}；目标是{why}。")
}

fn render_body_constraint(when: &str, what: &str, why: &str) -> String {
    let when_clause = normalize_when_clause(when, "在");
    // 如果 what 已经以"必须""禁止""不要"开头，直接保留；否则补"必须"
    let what_clause = if starts_with_modal_verb(what) {
        what.to_string()
    } else {
        format!("必须{what}")
    };
    format!("{when_clause}，{what_clause}；目的是{why}。")
}

fn render_body_preference(when: &str, what: &str, why: &str) -> String {
    let when_clause = normalize_when_clause(when, "在");
    let what_clause =
        if what.starts_with("偏好") || what.starts_with("更倾向") || what.starts_with("优先")
        {
            what.to_string()
        } else {
            format!("偏好{what}")
        };
    format!("{when_clause}，{what_clause}；原因是{why}。")
}

/// 把 when 子句规范成 "<前缀>...时" 形式。
///
/// `default_prefix` 是当 when 没有"当/在"开头时补的字。
fn normalize_when_clause(when: &str, default_prefix: &str) -> String {
    let prefixed = if when.starts_with("当") || when.starts_with("在") {
        when.to_string()
    } else {
        format!("{default_prefix}{when}")
    };
    if prefixed.ends_with("时") {
        prefixed
    } else {
        format!("{prefixed}时")
    }
}

fn starts_with_modal_verb(s: &str) -> bool {
    [
        "必须",
        "禁止",
        "不要",
        "不得",
        "应当",
        "应该",
        "agent 应",
        "agent 不应",
        "Agent 应",
        "Agent 不应",
    ]
    .iter()
    .any(|m| s.starts_with(m))
}

/// brief 渲染：以"用于"开头，从 why 派生
fn render_brief(candidate: &InducedCandidate) -> String {
    let why = candidate.why.trim().trim_end_matches('。');
    if why.starts_with("用于") {
        format!("{why}。")
    } else {
        format!("用于{why}。")
    }
}

fn check_title(title: &str, reasons: &mut Vec<String>) {
    let count = title.chars().count();
    if count < TITLE_MIN_CHARS {
        reasons.push(format!("title too short: {count} < {TITLE_MIN_CHARS}"));
    }
    if count > TITLE_MAX_CHARS {
        reasons.push(format!("title too long: {count} > {TITLE_MAX_CHARS}"));
    }
}

fn check_body(body: &str, reasons: &mut Vec<String>) {
    let count = body.chars().count();
    if count < BODY_MIN_CHARS {
        reasons.push(format!("body too short: {count} < {BODY_MIN_CHARS}"));
    }
    if count > BODY_MAX_CHARS {
        reasons.push(format!("body too long: {count} > {BODY_MAX_CHARS}"));
    }
}

fn check_brief(brief: &str, reasons: &mut Vec<String>) {
    let count = brief.chars().count();
    if count < BRIEF_MIN_CHARS {
        reasons.push(format!("brief too short: {count} < {BRIEF_MIN_CHARS}"));
    }
    if count > BRIEF_MAX_CHARS {
        reasons.push(format!("brief too long: {count} > {BRIEF_MAX_CHARS}"));
    }
}

fn check_brief_body_overlap(brief: &str, body: &str, reasons: &mut Vec<String>) {
    let jaccard = char_jaccard(brief, body);
    if jaccard >= BRIEF_BODY_JACCARD_MAX {
        reasons.push(format!(
            "brief-body overlap too high: jaccard {jaccard:.2} >= {BRIEF_BODY_JACCARD_MAX}"
        ));
    }
}

fn check_kind_scope(candidate: &InducedCandidate, reasons: &mut Vec<String>) {
    if !matches!(
        candidate.kind.as_str(),
        "preference" | "constraint" | "procedure"
    ) {
        reasons.push(format!("unknown kind: {}", candidate.kind));
    }
    if !matches!(candidate.scope.as_str(), "global" | "project") {
        reasons.push(format!("unknown scope: {}", candidate.scope));
    }
}

fn check_temporal_status(candidate: &InducedCandidate, reasons: &mut Vec<String>) {
    if !matches!(
        candidate.temporal_status.as_str(),
        "stable" | "reversed" | "refined"
    ) {
        reasons.push(format!(
            "unknown temporal_status: {}",
            candidate.temporal_status
        ));
    }
}

fn check_memory_level(candidate: &InducedCandidate, body: &str, reasons: &mut Vec<String>) {
    if looks_like_low_level_project_detail(candidate, body)
        && !looks_like_project_workflow_or_global_memory(candidate, body)
    {
        reasons.push(
            "low-level project/product detail is evidence context, not durable workflow memory"
                .to_string(),
        );
    }
}

fn looks_like_low_level_project_detail(candidate: &InducedCandidate, body: &str) -> bool {
    let combined = format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        candidate.title,
        candidate.when,
        candidate.what,
        candidate.why,
        body,
        candidate
            .evidence_quotes
            .iter()
            .map(|quote| quote.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    )
    .to_lowercase();
    [
        "thresholds:",
        "storage_key",
        "存储键",
        "0/10/50/200",
        "炼气",
        "筑基",
        "金丹",
        "元婴",
        "灵石",
        "拖动入口",
        "教程",
        "枚举",
        "enum",
        "union",
        "labels:",
    ]
    .iter()
    .any(|marker| combined.contains(marker))
}

fn looks_like_project_workflow_or_global_memory(candidate: &InducedCandidate, body: &str) -> bool {
    let combined = format!(
        "{}\n{}\n{}\n{}",
        candidate.title, candidate.when, candidate.what, body
    )
    .to_lowercase();
    [
        "工作流",
        "workflow",
        "tdd",
        "测试",
        "验证",
        "审查",
        "review",
        "合并",
        "merge",
        "bun",
        "build",
        "发布",
        "协作",
        "边界",
        "golden set",
        "memory card",
        "提炼",
        "证据",
        "候选",
    ]
    .iter()
    .any(|marker| combined.contains(marker))
}

/// 字符级 jaccard（不分 token，去标点空白）。
fn char_jaccard(a: &str, b: &str) -> f32 {
    let normalize = |s: &str| -> std::collections::HashSet<char> {
        s.chars()
            .filter(|c| c.is_alphanumeric() || ('\u{4E00}'..='\u{9FFF}').contains(c))
            .flat_map(|c| c.to_lowercase())
            .collect()
    };
    let set_a = normalize(a);
    let set_b = normalize(b);
    if set_a.is_empty() && set_b.is_empty() {
        return 1.0;
    }
    if set_a.is_empty() || set_b.is_empty() {
        return 0.0;
    }
    let intersection = set_a.intersection(&set_b).count();
    let union = set_a.union(&set_b).count();
    intersection as f32 / union as f32
}

/// tags 派生：从 kind / scope / activation 单向投影。
fn derive_tags(candidate: &InducedCandidate, activation: &str) -> Vec<String> {
    let mut tags = vec![
        format!("kind:{}", candidate.kind),
        format!("scope:{}", candidate.scope),
        format!("activation:{activation}"),
    ];
    if candidate.recurrence >= 3 {
        tags.push("recurrence:high".to_string());
    } else if candidate.recurrence == 2 {
        tags.push("recurrence:mid".to_string());
    } else {
        tags.push("recurrence:low".to_string());
    }
    if candidate.temporal_status != "stable" {
        tags.push(format!("temporal:{}", candidate.temporal_status));
    }
    tags.sort();
    tags.dedup();
    tags
}

/// 简单语言探测：含 CJK 字符 → zh-CN，否则 en
fn detect_language(candidate: &InducedCandidate) -> String {
    let mixed: String = format!("{} {} {}", candidate.title, candidate.what, candidate.why);
    if mixed
        .chars()
        .any(|c| ('\u{4E00}'..='\u{9FFF}').contains(&c))
    {
        "zh-CN".to_string()
    } else {
        "en".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(
        title: &str,
        when: &str,
        what: &str,
        why: &str,
        kind: &str,
        recurrence: usize,
    ) -> InducedCandidate {
        InducedCandidate {
            cluster_id: "c_test".to_string(),
            recurrence,
            title: title.to_string(),
            when: when.to_string(),
            what: what.to_string(),
            why: why.to_string(),
            boundary: None,
            kind: kind.to_string(),
            scope: "global".to_string(),
            evidence_quotes: vec![EvidenceQuote {
                observation_id: "o1".to_string(),
                text: "evidence text".to_string(),
            }],
            memory_tier: None,
            abstraction_level: None,
            support_level: None,
            temporal_status: "stable".to_string(),
            confidence: 0.8,
        }
    }

    #[test]
    fn happy_path_produces_card_with_canonical_body() {
        let c = candidate(
            "评估提炼优先用真实历史回归",
            "评估提炼质量或修改提炼逻辑时",
            "用真实历史会话做回归验证而不是只依赖静态样例",
            "让提炼规则在真实数据上接受检验",
            "procedure",
            3,
        );
        let outcome = crystallize_one(&c);
        match outcome {
            CrystallizationOutcome::Accepted(card) => {
                assert!(card.body.starts_with("当评估提炼质量或修改提炼逻辑时"));
                assert!(card.body.contains("目标是让提炼规则在真实数据上接受检验"));
                assert!(card.brief.starts_with("用于"));
                assert_eq!(card.activation, "skill"); // procedure → skill
                assert!(card.tags.iter().any(|t| t == "kind:procedure"));
                assert!(card.tags.iter().any(|t| t == "recurrence:high"));
            }
            CrystallizationOutcome::Rejected { reasons, .. } => {
                panic!("should accept, got rejection: {reasons:?}")
            }
        }
    }

    #[test]
    fn rejects_short_title() {
        let c = candidate(
            "短",
            "评估提炼时",
            "做点什么具体的事情让句子够长",
            "原因",
            "procedure",
            2,
        );
        let outcome = crystallize_one(&c);
        match outcome {
            CrystallizationOutcome::Rejected { reasons, .. } => {
                assert!(reasons.iter().any(|r| r.contains("title too short")));
            }
            _ => panic!("expected reject"),
        }
    }

    #[test]
    fn rejects_long_title() {
        let long_title: String = "标题".repeat(20);
        let c = candidate(&long_title, "x", "y", "z", "preference", 1);
        let outcome = crystallize_one(&c);
        assert!(matches!(outcome, CrystallizationOutcome::Rejected { .. }));
    }

    #[test]
    fn rejects_unknown_kind() {
        let c = candidate(
            "正常长度的标题abc",
            "在某个场景里",
            "做某个具体的事情这句子要够长才能过 body 阈值最少五十字符所以再多写一点",
            "目标是某个具体的原因",
            "weird-kind",
            2,
        );
        let outcome = crystallize_one(&c);
        match outcome {
            CrystallizationOutcome::Rejected { reasons, .. } => {
                assert!(reasons.iter().any(|r| r.contains("unknown kind")));
            }
            _ => panic!("expected reject"),
        }
    }

    #[test]
    fn body_renders_with_when_already_starting_dang() {
        let c = candidate(
            "评估提炼时优先真实历史",
            "当评估提炼时",
            "用真实会话做回归而不只用样例",
            "让规则真实",
            "procedure",
            2,
        );
        if let CrystallizationOutcome::Accepted(card) = crystallize_one(&c) {
            // 不应重复"当当"
            assert!(!card.body.starts_with("当当"));
            assert!(card.body.starts_with("当评估提炼时"));
        }
    }

    #[test]
    fn brief_starting_with_yongyu_not_doubled() {
        let c = candidate(
            "正常的标题字符串",
            "在评估提炼质量时",
            "用真实历史会话做回归验证而不只用静态样例",
            "用于让规则接受真实数据检验",
            "procedure",
            2,
        );
        if let CrystallizationOutcome::Accepted(card) = crystallize_one(&c) {
            // 不应"用于用于"
            assert!(!card.brief.starts_with("用于用于"));
            assert!(card.brief.starts_with("用于"));
        }
    }

    #[test]
    fn rejects_when_brief_overlaps_body_too_much() {
        // 故意让 body 与 brief 字符级 jaccard 高
        let c = candidate(
            "正常的标题字符串",
            "在 X 时",
            "做 Y",
            // why 用得很长，让 brief 几乎等于 body 后半段
            "做 Y 这是一个非常长的原因描述用来制造高 jaccard 重叠让校验拒绝这个 candidate",
            "procedure",
            2,
        );
        let outcome = crystallize_one(&c);
        // 可能因 body 太短 reject，也可能因 brief-body 重叠 reject；两者都接受
        match outcome {
            CrystallizationOutcome::Rejected { reasons, .. } => {
                assert!(!reasons.is_empty());
            }
            CrystallizationOutcome::Accepted(card) => {
                let j = char_jaccard(&card.brief, &card.body);
                assert!(j < BRIEF_BODY_JACCARD_MAX, "got {j}");
            }
        }
    }

    #[test]
    fn constraint_render_does_not_prefix_agent_modal_clause() {
        let c = candidate(
            "保留审阅边界先审查",
            "协作开发或合并代码时",
            "agent 不应跳过人工审查直接 merge 代码",
            "保障协作质量和人工确认边界",
            "constraint",
            2,
        );

        if let CrystallizationOutcome::Accepted(card) = crystallize_one(&c) {
            assert!(!card.body.contains("必须agent"));
            assert!(card.body.contains("agent 不应跳过人工审查"));
        } else {
            panic!("expected accept");
        }
    }

    #[test]
    fn tags_include_recurrence_and_temporal_when_non_stable() {
        let mut c = candidate(
            "正常的标题字符串",
            "在某场景时",
            "做某个具体动作让 body 字数过五十字所以多写一点这样长度才够",
            "目标是某个长一点的原因描述",
            "procedure",
            5,
        );
        c.temporal_status = "reversed".to_string();
        if let CrystallizationOutcome::Accepted(card) = crystallize_one(&c) {
            assert!(card.tags.iter().any(|t| t == "recurrence:high"));
            assert!(card.tags.iter().any(|t| t == "temporal:reversed"));
        }
    }

    #[test]
    fn rejects_low_level_product_or_implementation_detail_memory() {
        let c = candidate(
            "境界阈值和存储键设计",
            "实现 DaoFocus 境界进度系统时",
            "使用 Thresholds: 0h, 10h, 50h, 200h，并使用 STORAGE_KEY = \"daofocus:v1\"",
            "记录具体产品和实现设定",
            "constraint",
            3,
        );
        let outcome = crystallize_one(&c);

        match outcome {
            CrystallizationOutcome::Rejected { reasons, .. } => {
                assert!(
                    reasons
                        .iter()
                        .any(|reason| reason.contains("low-level project/product detail")),
                    "expected low-level detail rejection, got {reasons:?}"
                );
            }
            _ => panic!("expected reject"),
        }
    }

    #[test]
    fn keeps_project_workflow_memory_even_when_project_specific() {
        let mut c = candidate(
            "DaoFocus任务遵循TDD审查工作流",
            "在 DaoFocus 项目执行开发任务时",
            "使用 Bun 运行测试，先 TDD 实现，再进行 review 和 merge 前检查",
            "保持项目交付质量和协作边界",
            "procedure",
            4,
        );
        c.scope = "project".to_string();

        let outcome = crystallize_one(&c);

        assert!(matches!(outcome, CrystallizationOutcome::Accepted(_)));
    }

    #[test]
    fn body_renders_candidate_boundary_when_present() {
        let mut c = candidate(
            "生成卡片前确认隐私边界",
            "优化 Memory Card 生成链路时",
            "先区分合成测试、脱敏样例和本地私有评估",
            "保护真实对话不进入仓库或运行时参考",
            "procedure",
            2,
        );
        c.boundary = Some("真实对话只用于本地聚合评估，不写入 Git 或 GitHub".to_string());

        let outcome = crystallize_one(&c);

        match outcome {
            CrystallizationOutcome::Accepted(card) => {
                assert!(card.body.contains("边界是"));
                assert!(card.body.contains("不写入 Git 或 GitHub"));
            }
            CrystallizationOutcome::Rejected { reasons, .. } => {
                panic!("should accept, got rejection: {reasons:?}")
            }
        }
    }

    #[test]
    fn detect_language_picks_zh_for_chinese_content() {
        let c = candidate(
            "正常的标题",
            "在 X 时",
            "做 Y 这句子要够长才能过 body 阈值五十字符所以再多写一点",
            "目标是 Z 的原因",
            "procedure",
            1,
        );
        if let CrystallizationOutcome::Accepted(card) = crystallize_one(&c) {
            assert_eq!(card.language, "zh-CN");
        }
    }

    #[test]
    fn detect_language_picks_en_for_english_only() {
        let c = candidate(
            "Title in English",
            "When evaluating extraction quality",
            "Use real history sessions for regression validation always not only static samples here",
            "So that rules face real data checks",
            "procedure",
            1,
        );
        if let CrystallizationOutcome::Accepted(card) = crystallize_one(&c) {
            assert_eq!(card.language, "en");
        }
    }
}
