use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use anyhow::Result;
use chrono::Utc;
use serde::Serialize;

use crate::build;
use crate::draft;
use crate::rule_test;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::candidate::ExtractionMetadata;
    use crate::memory_card;

    #[test]
    fn review_report_serializes_pending_drafts() {
        let temp = tempfile::tempdir().expect("tempdir");
        draft::add_draft(
            temp.path(),
            draft::NewDraft {
                id: "project:prefer-bun".to_string(),
                title: "Prefer Bun".to_string(),
                body: "Use Bun for JavaScript package management and scripts.".to_string(),
                kind: "preference".to_string(),
                scope: "project".to_string(),
                targets: vec!["codex".to_string()],
                evidence: "manual test".to_string(),
                confidence: None,
                reason: None,
                matched_template: None,
                extraction: ExtractionMetadata::default(),
            },
        )
        .expect("add draft");

        let report = review_project(temp.path()).expect("review");
        let value = serde_json::to_value(report).expect("json");

        assert_eq!(value["drafts"][0]["id"], "project:prefer-bun");
        assert_eq!(value["summary"]["drafts_pending"], 1);
    }

    #[test]
    fn review_decision_can_approve_a_draft() {
        let temp = tempfile::tempdir().expect("tempdir");
        draft::add_draft(
            temp.path(),
            draft::NewDraft {
                id: "project:prefer-bun".to_string(),
                title: "Prefer Bun".to_string(),
                body: "Use Bun for JavaScript package management and scripts.".to_string(),
                kind: "preference".to_string(),
                scope: "project".to_string(),
                targets: vec!["codex".to_string()],
                evidence: "manual test".to_string(),
                confidence: None,
                reason: None,
                matched_template: None,
                extraction: ExtractionMetadata::default(),
            },
        )
        .expect("add draft");

        let result = apply_review_decisions(
            temp.path(),
            &[ReviewDecision::ApproveDraft(
                "project:prefer-bun".to_string(),
            )],
        )
        .expect("apply");

        assert_eq!(result[0].status, "applied");
        assert!(draft::load_drafts(temp.path()).expect("drafts").is_empty());
    }

    #[test]
    fn review_decision_records_feedback_event_for_eval_harvest() {
        let temp = tempfile::tempdir().expect("tempdir");
        draft::add_draft(
            temp.path(),
            draft::NewDraft {
                id: "project:temporary-rule".to_string(),
                title: "Temporary Rule".to_string(),
                body: "This sprint only, avoid touching build.rs.".to_string(),
                kind: "constraint".to_string(),
                scope: "project".to_string(),
                targets: vec!["codex".to_string()],
                evidence: "这轮先别改 build.rs，只看 app 页面".to_string(),
                confidence: Some(0.8),
                reason: Some("temporary task boundary".to_string()),
                matched_template: Some("pipeline-v2".to_string()),
                extraction: ExtractionMetadata {
                    source_observations: vec!["obs:test:temporary".to_string()],
                    ..ExtractionMetadata::default()
                },
            },
        )
        .expect("add draft");

        apply_review_decisions(
            temp.path(),
            &[ReviewDecision::RejectDraftWithReason {
                id: "project:temporary-rule".to_string(),
                reason: "阶段性任务边界，不应进入长期记忆".to_string(),
            }],
        )
        .expect("apply");

        let events = std::fs::read_to_string(
            temp.path()
                .join(".agent-kernel")
                .join("eval-corpus")
                .join("review-events.jsonl"),
        )
        .expect("review events");
        assert!(events.contains("\"decision\":\"reject\""));
        assert!(events.contains("阶段性任务边界"));
        assert!(events.contains("obs:test:temporary"));
    }

    #[test]
    fn review_summary_counts_artifact_drifts() {
        let temp = tempfile::tempdir().expect("tempdir");
        memory_card::add_memory_card(
            temp.path(),
            "project:codex-rule",
            "Codex Rule",
            "Use Bun for JavaScript package management and scripts.",
            "preference",
            "project",
            vec!["codex".to_string()],
        )
        .expect("add memory_card");
        build::sync_project(temp.path()).expect("sync");
        std::fs::write(temp.path().join("AGENTS.md"), "manual edit").expect("manual edit");

        let report = review_project(temp.path()).expect("review");
        let value = serde_json::to_value(report).expect("json");

        assert_eq!(value["summary"]["artifact_drifts"], 1);
    }
}

#[derive(Debug, Serialize)]
pub struct ReviewSummary {
    pub drafts_pending: usize,
    pub rule_tests_failed: usize,
    pub artifact_drifts: usize,
}

#[derive(Debug, Serialize)]
pub struct ReviewReport {
    pub summary: ReviewSummary,
    pub drafts: Vec<draft::DraftRecord>,
    pub mirror_status: build::StatusReport,
    pub rule_ci: rule_test::RuleTestReport,
    pub build_preview: build::BuildReport,
}

#[derive(Debug, Clone)]
pub enum ReviewDecision {
    ApproveDraft(String),
    RejectDraft(String),
    RejectDraftWithReason { id: String, reason: String },
}

#[derive(Debug, Serialize)]
pub struct ReviewDecisionResult {
    pub action: String,
    pub id: String,
    pub status: String,
}

impl ReviewReport {
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("Agent Memory Kernel Review\n\n");
        out.push_str("## Draft Inbox\n\n");
        if self.drafts.is_empty() {
            out.push_str("No drafts pending.\n");
        } else {
            for draft in &self.drafts {
                out.push_str(&format!(
                    "- {}: {} [{}]\n",
                    draft.id, draft.title, draft.status
                ));
            }
        }
        out.push('\n');
        out.push_str(&format!(
            "## Mirror Status\n\n{}\n",
            self.mirror_status.render()
        ));
        out.push_str(&format!("## Rule CI\n\n{}\n", self.rule_ci.render()));
        out.push_str(&format!(
            "## Build Preview\n\n{}\n",
            self.build_preview.render()
        ));
        out
    }
}

pub fn review_project(project_root: &Path) -> Result<ReviewReport> {
    let drafts = draft::load_drafts(project_root)?;

    let mirror_status = build::status_project(project_root)?;
    let rule_ci = rule_test::run_rule_tests(project_root)?;
    let build_preview = build::build_project(project_root, true)?;

    Ok(ReviewReport {
        summary: ReviewSummary {
            drafts_pending: drafts.len(),
            rule_tests_failed: rule_ci.failed,
            artifact_drifts: mirror_status.artifact_drift_count(),
        },
        drafts,
        mirror_status,
        rule_ci,
        build_preview,
    })
}

pub fn apply_review_decisions(
    project_root: &Path,
    decisions: &[ReviewDecision],
) -> Result<Vec<ReviewDecisionResult>> {
    let mut results = Vec::new();
    for decision in decisions {
        match decision {
            ReviewDecision::ApproveDraft(id) => {
                if let Some(draft) = draft_snapshot(project_root, id)? {
                    append_review_feedback_event(
                        project_root,
                        &draft,
                        ReviewFeedbackDecision::Approve,
                        None,
                    )?;
                }
                draft::approve_draft(project_root, id)?;
                results.push(ReviewDecisionResult {
                    action: "approve-draft".to_string(),
                    id: id.clone(),
                    status: "applied".to_string(),
                });
            }
            ReviewDecision::RejectDraft(id) => {
                if let Some(draft) = draft_snapshot(project_root, id)? {
                    append_review_feedback_event(
                        project_root,
                        &draft,
                        ReviewFeedbackDecision::Reject,
                        None,
                    )?;
                }
                draft::reject_draft(project_root, id)?;
                results.push(ReviewDecisionResult {
                    action: "reject-draft".to_string(),
                    id: id.clone(),
                    status: "applied".to_string(),
                });
            }
            ReviewDecision::RejectDraftWithReason { id, reason } => {
                if let Some(draft) = draft_snapshot(project_root, id)? {
                    append_review_feedback_event(
                        project_root,
                        &draft,
                        ReviewFeedbackDecision::Reject,
                        Some(reason),
                    )?;
                }
                draft::reject_draft(project_root, id)?;
                results.push(ReviewDecisionResult {
                    action: "reject-draft".to_string(),
                    id: id.clone(),
                    status: "applied".to_string(),
                });
            }
        }
    }
    Ok(results)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
enum ReviewFeedbackDecision {
    Approve,
    Reject,
}

#[derive(Debug, Clone, Serialize)]
struct ReviewFeedbackEvent<'a> {
    timestamp: String,
    decision: ReviewFeedbackDecision,
    id: &'a str,
    title: &'a str,
    kind: &'a str,
    scope: &'a str,
    body: &'a str,
    evidence: &'a str,
    source_observations: &'a [String],
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'a str>,
}

fn draft_snapshot(project_root: &Path, id: &str) -> Result<Option<draft::DraftRecord>> {
    Ok(draft::load_drafts(project_root)?
        .into_iter()
        .find(|draft| draft.id == id))
}

fn append_review_feedback_event(
    project_root: &Path,
    draft: &draft::DraftRecord,
    decision: ReviewFeedbackDecision,
    reason: Option<&str>,
) -> Result<()> {
    let root = crate::fsutil::normalize_project_root(project_root)?;
    let dir = crate::config::kernel_dir(&root).join("eval-corpus");
    std::fs::create_dir_all(&dir)?;
    let event = ReviewFeedbackEvent {
        timestamp: Utc::now().to_rfc3339(),
        decision,
        id: &draft.id,
        title: &draft.title,
        kind: &draft.kind,
        scope: &draft.scope,
        body: &draft.body,
        evidence: &draft.evidence,
        source_observations: &draft.extraction.source_observations,
        reason,
    };
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("review-events.jsonl"))?;
    writeln!(file, "{}", serde_json::to_string(&event)?)?;
    Ok(())
}
