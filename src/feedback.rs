use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::candidate::MemoryTier;
use crate::config;
use crate::fsutil;
use crate::textutil;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackEvent {
    pub event_id: String,
    pub item_kind: String,
    pub item_id: String,
    pub decision: String,
    pub body: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rejection_category: Option<String>,
    pub signature: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RejectionLearningReport {
    pub total_rejections: usize,
    pub categorized_rejections: usize,
    pub negative_eval_examples: usize,
    pub suppressible_signatures: usize,
    pub estimated_false_positive_reduction: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FeedbackPenalty {
    pub confidence_delta: f32,
    pub rejections: usize,
    pub reason: String,
}

pub fn record_feedback(
    project_root: &Path,
    item_kind: &str,
    item_id: &str,
    decision: &str,
    body: &str,
    reason: Option<String>,
) -> Result<()> {
    config::ensure_kernel_dir(project_root)?;
    let created_at = Utc::now().to_rfc3339();
    let signature = feedback_signature(item_kind, body);
    let rejection_category = if is_rejection(decision) {
        Some(categorize_rejection(reason.as_deref(), body))
    } else {
        None
    };
    let event = FeedbackEvent {
        event_id: format!(
            "fb:{}",
            textutil::slug(
                &fsutil::sha256_text(&format!("{item_kind}:{item_id}:{decision}:{created_at}"))
                    [..16]
            )
        ),
        item_kind: item_kind.to_string(),
        item_id: item_id.to_string(),
        decision: decision.to_string(),
        body: body.to_string(),
        reason,
        rejection_category,
        signature,
        created_at,
    };
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(feedback_path(project_root))
        .context("open feedback log")?;
    writeln!(file, "{}", serde_json::to_string(&event)?).context("write feedback log")?;
    Ok(())
}

pub fn rejection_learning_report(project_root: &Path) -> Result<RejectionLearningReport> {
    let events = load_feedback(project_root)?
        .into_iter()
        .filter(|event| is_rejection(&event.decision))
        .collect::<Vec<_>>();
    let mut grouped = BTreeMap::<String, Vec<FeedbackEvent>>::new();
    for event in &events {
        grouped
            .entry(event.signature.clone())
            .or_default()
            .push(event.clone());
    }
    let negative_eval_examples = grouped.len();
    let suppressible_signatures = grouped.values().filter(|events| events.len() >= 2).count();
    let estimated_false_positive_reduction = grouped
        .values()
        .filter(|events| events.len() >= 2)
        .map(|events| events.len().saturating_sub(1))
        .sum();
    Ok(RejectionLearningReport {
        total_rejections: events.len(),
        categorized_rejections: events
            .iter()
            .filter(|event| event.rejection_category.is_some())
            .count(),
        negative_eval_examples,
        suppressible_signatures,
        estimated_false_positive_reduction,
    })
}

pub fn write_rejection_negative_eval_proposals(project_root: &Path) -> Result<Option<PathBuf>> {
    let events = load_feedback(project_root)?
        .into_iter()
        .filter(|event| is_rejection(&event.decision))
        .collect::<Vec<_>>();
    if events.is_empty() {
        return Ok(None);
    }
    let mut grouped = BTreeMap::<String, Vec<FeedbackEvent>>::new();
    for event in events {
        grouped
            .entry(event.signature.clone())
            .or_default()
            .push(event);
    }
    let dir = config::kernel_dir(project_root).join("eval-corpus");
    fs::create_dir_all(&dir).context("create eval corpus directory")?;
    let path = dir.join("rejected-negative-proposals.yml");
    let report = rejection_learning_report(project_root)?;
    let mut out = String::new();
    out.push_str("kind: rejection-negative-eval-proposals\n");
    out.push_str("review_required: true\n");
    out.push_str(&format!(
        "generated_at: {}\n",
        yaml_string(&Utc::now().to_rfc3339())
    ));
    out.push_str("learning_report:\n");
    out.push_str(&format!(
        "  total_rejections: {}\n",
        report.total_rejections
    ));
    out.push_str(&format!(
        "  categorized_rejections: {}\n",
        report.categorized_rejections
    ));
    out.push_str(&format!(
        "  negative_eval_examples: {}\n",
        report.negative_eval_examples
    ));
    out.push_str(&format!(
        "  suppressible_signatures: {}\n",
        report.suppressible_signatures
    ));
    out.push_str(&format!(
        "  estimated_false_positive_reduction: {}\n",
        report.estimated_false_positive_reduction
    ));
    out.push_str("negatives:\n");
    for (signature, events) in grouped {
        let sample = events
            .first()
            .map(|event| event.body.as_str())
            .unwrap_or_default();
        let reason = dominant_reason(&events);
        let category = dominant_category(&events);
        let event_ids = events
            .iter()
            .map(|event| event.event_id.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "  - id: {}\n    signature: {}\n    rejection_category: {}\n    reason: {}\n    source_event_ids: [{}]\n    user_messages:\n      - {}\n    expected_reduction: {}\n",
            yaml_string(&format!("rejected-{}", signature.trim_start_matches("sig:"))),
            yaml_string(&signature),
            yaml_string(&category),
            yaml_string(&reason),
            event_ids,
            yaml_string(sample),
            yaml_string("future matching suggestions should be downranked or skipped before review")
        ));
    }
    fs::write(&path, out).with_context(|| format!("write {}", path.display()))?;
    Ok(Some(path))
}

pub fn load_feedback(project_root: &Path) -> Result<Vec<FeedbackEvent>> {
    let path = feedback_path(project_root);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).context("parse feedback event"))
        .collect()
}

pub fn feedback_penalty_for_body(
    project_root: &Path,
    item_kind: &str,
    body: &str,
) -> Result<Option<FeedbackPenalty>> {
    let signature = feedback_signature(item_kind, body);
    let mut reasons = BTreeMap::<String, usize>::new();
    let rejections = load_feedback(project_root)?
        .into_iter()
        .filter(|event| event.item_kind == item_kind)
        .filter(|event| event.signature == signature)
        .filter(|event| is_rejection(&event.decision))
        .inspect(|event| {
            if let Some(reason) = event.reason.as_deref() {
                *reasons.entry(reason.to_string()).or_default() += 1;
            }
        })
        .count();
    if rejections < 2 {
        return Ok(None);
    }
    let confidence_delta = (rejections as f32 * -0.08).max(-0.25);
    let reason = reasons
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(reason, _)| reason)
        .unwrap_or_else(|| "repeated user rejection".to_string());
    Ok(Some(FeedbackPenalty {
        confidence_delta,
        rejections,
        reason,
    }))
}

pub fn feedback_penalty_for_candidate_body(
    project_root: &Path,
    item_kind: &str,
    scope: &str,
    memory_tier: &MemoryTier,
    body: &str,
) -> Result<Option<FeedbackPenalty>> {
    let contextual_signature =
        feedback_signature_with_context(item_kind, scope, memory_tier.as_str(), body);
    let legacy_signature = feedback_signature(item_kind, body);
    let mut reasons = BTreeMap::<String, usize>::new();
    let rejections = load_feedback(project_root)?
        .into_iter()
        .filter(|event| event.item_kind == item_kind)
        .filter(|event| {
            event.signature == contextual_signature || event.signature == legacy_signature
        })
        .filter(|event| is_rejection(&event.decision))
        .inspect(|event| {
            if let Some(reason) = event.reason.as_deref() {
                *reasons.entry(reason.to_string()).or_default() += 1;
            }
        })
        .count();
    if rejections < 2 {
        return Ok(None);
    }
    let confidence_delta = (rejections as f32 * -0.08).max(-0.25);
    let reason = reasons
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(reason, _)| reason)
        .unwrap_or_else(|| "repeated user rejection".to_string());
    Ok(Some(FeedbackPenalty {
        confidence_delta,
        rejections,
        reason,
    }))
}

pub fn write_weekly_reflexion_proposal(project_root: &Path) -> Result<Option<PathBuf>> {
    let events = load_feedback(project_root)?;
    let mut grouped = BTreeMap::<String, Vec<FeedbackEvent>>::new();
    for event in events
        .into_iter()
        .filter(|event| is_rejection(&event.decision))
    {
        grouped
            .entry(event.signature.clone())
            .or_default()
            .push(event);
    }
    let mut patterns = grouped
        .into_iter()
        .filter(|(_, events)| events.len() >= 3)
        .collect::<Vec<_>>();
    if patterns.is_empty() {
        return Ok(None);
    }
    patterns.sort_by_key(|(_, events)| Reverse(events.len()));
    let dir = config::kernel_dir(project_root).join("extraction-reflections");
    fs::create_dir_all(&dir).context("create extraction reflection directory")?;
    let date = Utc::now().format("%Y-%m-%d").to_string();
    let path = dir.join(format!("{date}.yml"));
    let mut out = String::new();
    out.push_str("kind: extraction-reflection\n");
    out.push_str("review_required: true\n");
    out.push_str("patterns:\n");
    for (signature, events) in patterns.into_iter().take(10) {
        let sample = events
            .first()
            .map(|event| event.body.as_str())
            .unwrap_or_default();
        let reason = dominant_reason(&events);
        out.push_str(&format!(
            "  - signature: {signature}\n    rejections: {}\n    sample: {}\n    reason: {}\n    proposed_change: Downrank future candidates with this signature unless edited by the user.\n",
            events.len(),
            yaml_string(sample),
            yaml_string(&reason)
        ));
    }
    fs::write(&path, out).with_context(|| format!("write {}", path.display()))?;
    Ok(Some(path))
}

pub fn categorize_rejection(reason: Option<&str>, body: &str) -> String {
    let text = format!(
        "{} {}",
        reason.unwrap_or_default().to_lowercase(),
        body.to_lowercase()
    );
    if text.contains("conflict") || text.contains("冲突") || text.contains("不符合") {
        "conflicts-existing-memory".to_string()
    } else if text.contains("duplicate") || text.contains("重复") || text.contains("已有") {
        "duplicate".to_string()
    } else if text.contains("generic") || text.contains("泛") || text.contains("空泛") {
        "too-generic".to_string()
    } else if text.contains("temporary") || text.contains("一次性") || text.contains("临时") {
        "temporary-context".to_string()
    } else if text.contains("obsolete") || text.contains("过时") || text.contains("不用") {
        "obsolete".to_string()
    } else if text.contains("wrong") || text.contains("错误") || text.contains("不对") {
        "incorrect".to_string()
    } else {
        "other".to_string()
    }
}

fn feedback_path(project_root: &Path) -> std::path::PathBuf {
    config::kernel_dir(project_root).join("feedback.jsonl")
}

fn feedback_signature(item_kind: &str, body: &str) -> String {
    let normalized_body = canonical_feedback_body(body);
    let normalized = textutil::tokenize(&normalized_body)
        .into_iter()
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "sig:{}",
        textutil::slug(&fsutil::sha256_text(&format!("{item_kind}:{normalized}"))[..16])
    )
}

fn feedback_signature_with_context(
    item_kind: &str,
    scope: &str,
    memory_tier: &str,
    body: &str,
) -> String {
    let normalized_body = canonical_feedback_body(body);
    let normalized = textutil::tokenize(&normalized_body)
        .into_iter()
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "sig:{}",
        textutil::slug(
            &fsutil::sha256_text(&format!("{item_kind}:{scope}:{memory_tier}:{normalized}"))[..16]
        )
    )
}

fn canonical_feedback_body(body: &str) -> String {
    let mut normalized = body.to_lowercase();
    for (from, to) in [
        ("小修改做快测", "小改快测"),
        ("小修改快测", "小改快测"),
        ("大修改做完整回归测试", "大改重测"),
        ("大修改做完整回归", "大改重测"),
        ("大改再做完整回归测试", "大改重测"),
        ("大改再做完整回归", "大改重测"),
        ("大改详测", "大改重测"),
    ] {
        normalized = normalized.replace(from, to);
    }
    normalized
}

fn is_rejection(decision: &str) -> bool {
    matches!(decision, "reject" | "rejected")
}

fn dominant_reason(events: &[FeedbackEvent]) -> String {
    let mut counts = BTreeMap::<String, usize>::new();
    for event in events {
        if let Some(reason) = event.reason.as_deref() {
            *counts.entry(reason.to_string()).or_default() += 1;
        }
    }
    counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(reason, _)| reason)
        .unwrap_or_else(|| "repeated user rejection".to_string())
}

fn dominant_category(events: &[FeedbackEvent]) -> String {
    let mut counts = BTreeMap::<String, usize>::new();
    for event in events {
        if let Some(category) = event.rejection_category.as_deref() {
            *counts.entry(category.to_string()).or_default() += 1;
        }
    }
    counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(category, _)| category)
        .unwrap_or_else(|| "other".to_string())
}

fn yaml_string(value: &str) -> String {
    serde_yaml::to_string(value)
        .unwrap_or_else(|_| format!("{value:?}"))
        .trim()
        .trim_start_matches("---")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_feedback_as_jsonl() {
        let temp = tempfile::tempdir().expect("tempdir");

        record_feedback(
            temp.path(),
            "candidate",
            "project:prefer-bun",
            "approved",
            "Use Bun for JavaScript package management.",
            Some("good candidate".to_string()),
        )
        .expect("record feedback");

        let events = load_feedback(temp.path()).expect("load feedback");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].decision, "approved");
        assert_eq!(events[0].item_id, "project:prefer-bun");
        assert!(events[0].signature.starts_with("sig:"));
        assert_eq!(events[0].rejection_category, None);
    }

    #[test]
    fn repeated_rejections_create_feedback_penalty() {
        let temp = tempfile::tempdir().expect("tempdir");
        for item_id in ["one", "two", "three"] {
            record_feedback(
                temp.path(),
                "candidate",
                item_id,
                "rejected",
                "Use npm for JavaScript package management.",
                Some("obsolete package manager".to_string()),
            )
            .expect("record feedback");
        }

        let penalty = feedback_penalty_for_body(
            temp.path(),
            "candidate",
            "Use npm for JavaScript package management.",
        )
        .expect("penalty")
        .expect("repeated reject penalty");

        assert!(penalty.confidence_delta < 0.0);
        assert_eq!(penalty.rejections, 3);
        assert!(penalty.reason.contains("obsolete package manager"));
        let events = load_feedback(temp.path()).expect("load feedback");
        assert!(
            events
                .iter()
                .all(|event| { event.rejection_category.as_deref() == Some("obsolete") })
        );
    }

    #[test]
    fn rejection_reasons_are_categorized() {
        assert_eq!(
            categorize_rejection(Some("too generic"), "Best practices"),
            "too-generic"
        );
        assert_eq!(
            categorize_rejection(Some("一次性调试请求"), "临时命令"),
            "temporary-context"
        );
        assert_eq!(
            categorize_rejection(Some("和已有规则冲突"), "Use npm"),
            "conflicts-existing-memory"
        );
    }

    #[test]
    fn rejected_feedback_writes_negative_eval_proposals() {
        let temp = tempfile::tempdir().expect("tempdir");
        for item_id in ["one", "two"] {
            record_feedback(
                temp.path(),
                "candidate",
                item_id,
                "rejected",
                "Always use npm install for packages.",
                Some("conflicts with Bun preference".to_string()),
            )
            .expect("record feedback");
        }

        let path = write_rejection_negative_eval_proposals(temp.path())
            .expect("write proposals")
            .expect("proposal path");
        let text = fs::read_to_string(path).expect("read proposals");
        let report = rejection_learning_report(temp.path()).expect("learning report");

        assert_eq!(report.total_rejections, 2);
        assert_eq!(report.categorized_rejections, 2);
        assert_eq!(report.negative_eval_examples, 1);
        assert_eq!(report.suppressible_signatures, 1);
        assert_eq!(report.estimated_false_positive_reduction, 1);
        assert!(text.contains("rejection-negative-eval-proposals"));
        assert!(text.contains("review_required: true"));
        assert!(text.contains("conflicts-existing-memory"));
        assert!(text.contains("Always use npm install for packages."));
    }

    #[test]
    fn candidate_feedback_penalty_matches_legacy_body_signature() {
        let temp = tempfile::tempdir().expect("tempdir");
        for item_id in ["one", "two"] {
            record_feedback(
                temp.path(),
                "candidate",
                item_id,
                "rejected",
                "Prefer vague best practices for every project.",
                Some("too generic".to_string()),
            )
            .expect("record feedback");
        }

        let penalty = feedback_penalty_for_candidate_body(
            temp.path(),
            "candidate",
            "global",
            &MemoryTier::CrossProjectPrinciple,
            "Prefer vague best practices for every project.",
        )
        .expect("penalty")
        .expect("contextual penalty");

        assert_eq!(penalty.rejections, 2);
        assert!(penalty.confidence_delta < 0.0);
    }

    #[test]
    fn weekly_reflexion_writes_reviewable_proposal() {
        let temp = tempfile::tempdir().expect("tempdir");
        for item_id in ["one", "two", "three"] {
            record_feedback(
                temp.path(),
                "candidate",
                item_id,
                "rejected",
                "Always use npm install for packages.",
                Some("conflicts with Bun preference".to_string()),
            )
            .expect("record feedback");
        }

        let path = write_weekly_reflexion_proposal(temp.path())
            .expect("write proposal")
            .expect("proposal path");
        let text = fs::read_to_string(path).expect("read proposal");

        assert!(text.contains("extraction-reflection"));
        assert!(text.contains("conflicts with Bun preference"));
        assert!(text.contains("review_required: true"));
    }
}
