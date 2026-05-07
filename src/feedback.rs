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
    pub signature: String,
    pub created_at: String,
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
