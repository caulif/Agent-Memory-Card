use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::config;
use crate::fsutil;
use crate::skilllet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftRecord {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub scope: String,
    pub body: String,
    pub targets: Vec<String>,
    pub evidence: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_template: Option<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct NewDraft {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub scope: String,
    pub body: String,
    pub targets: Vec<String>,
    pub evidence: String,
    pub confidence: Option<f32>,
    pub reason: Option<String>,
    pub matched_template: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct DraftUpdate {
    pub title: Option<String>,
    pub body: Option<String>,
    pub kind: Option<String>,
    pub scope: Option<String>,
    pub targets: Option<Vec<String>>,
}

pub fn add_draft(project_root: &Path, draft: NewDraft) -> Result<()> {
    let root = fsutil::normalize_project_root(project_root)?;
    config::ensure_kernel_dir(&root)?;
    let now = Utc::now().to_rfc3339();
    let record = DraftRecord {
        id: draft.id.clone(),
        title: draft.title,
        kind: draft.kind,
        scope: draft.scope,
        body: draft.body,
        targets: draft.targets,
        evidence: draft.evidence,
        confidence: draft.confidence,
        reason: draft.reason,
        matched_template: draft.matched_template,
        status: "draft".to_string(),
        created_at: now.clone(),
        updated_at: now,
    };
    let path = draft_path(&root, &draft.id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_yaml::to_string(&record)?)?;
    Ok(())
}

pub fn load_drafts(project_root: &Path) -> Result<Vec<DraftRecord>> {
    let root = fsutil::normalize_project_root(project_root)?;
    let dir = config::kernel_dir(&root).join("drafts");
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut records: Vec<DraftRecord> = Vec::new();
    for entry in walkdir::WalkDir::new(dir).follow_links(false) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.path().extension().and_then(|value| value.to_str()) != Some("yml") {
            continue;
        }
        records.push(serde_yaml::from_str(&fs::read_to_string(entry.path())?)?);
    }
    records.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(records)
}

pub fn approve_draft(project_root: &Path, id: &str) -> Result<()> {
    let root = fsutil::normalize_project_root(project_root)?;
    let path = draft_path(&root, id);
    if !path.exists() {
        return Err(anyhow!("draft `{id}` does not exist"));
    }
    let draft: DraftRecord = serde_yaml::from_str(&fs::read_to_string(&path)?)?;
    skilllet::add_skilllet(
        &root,
        &draft.id,
        &draft.title,
        &draft.body,
        &draft.kind,
        &draft.scope,
        draft.targets,
    )?;
    fs::remove_file(path)?;
    Ok(())
}

pub fn reject_draft(project_root: &Path, id: &str) -> Result<()> {
    let root = fsutil::normalize_project_root(project_root)?;
    let path = draft_path(&root, id);
    if !path.exists() {
        return Err(anyhow!("draft `{id}` does not exist"));
    }
    fs::remove_file(path)?;
    Ok(())
}

pub fn update_draft(project_root: &Path, id: &str, update: DraftUpdate) -> Result<DraftRecord> {
    let root = fsutil::normalize_project_root(project_root)?;
    let path = draft_path(&root, id);
    if !path.exists() {
        return Err(anyhow!("draft `{id}` does not exist"));
    }
    let mut draft: DraftRecord = serde_yaml::from_str(&fs::read_to_string(&path)?)?;
    if let Some(title) = update.title {
        draft.title = title;
    }
    if let Some(body) = update.body {
        draft.body = body;
    }
    if let Some(kind) = update.kind {
        draft.kind = kind;
    }
    if let Some(scope) = update.scope {
        draft.scope = scope;
    }
    if let Some(mut targets) = update.targets {
        targets.sort();
        targets.dedup();
        draft.targets = targets;
    }
    draft.updated_at = Utc::now().to_rfc3339();
    fs::write(path, serde_yaml::to_string(&draft)?)?;
    Ok(draft)
}

pub fn merge_drafts(
    project_root: &Path,
    id: &str,
    title: &str,
    source_ids: Vec<String>,
    targets: Vec<String>,
) -> Result<DraftRecord> {
    if source_ids.len() < 2 {
        return Err(anyhow!("at least two source drafts are required"));
    }

    let root = fsutil::normalize_project_root(project_root)?;
    let drafts = load_drafts(&root)?
        .into_iter()
        .map(|draft| (draft.id.clone(), draft))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut missing = Vec::new();
    let mut sources = Vec::new();
    for source_id in &source_ids {
        let Some(draft) = drafts.get(source_id) else {
            missing.push(source_id.clone());
            continue;
        };
        sources.push(draft.clone());
    }
    if !missing.is_empty() {
        return Err(anyhow!("missing source drafts: {}", missing.join(", ")));
    }

    let body = sources
        .iter()
        .map(|draft| format!("## {}\n\n{}", draft.title, draft.body))
        .collect::<Vec<_>>()
        .join("\n\n");
    let evidence = format!("Merged Drafts: {}", source_ids.join(", "));
    let reason = Some(format!(
        "Merged {} drafts into one review candidate.",
        sources.len()
    ));
    let confidence = sources
        .iter()
        .map(|draft| draft.confidence.unwrap_or(0.0))
        .min_by(f32::total_cmp);
    let mut targets = targets;
    targets.sort();
    targets.dedup();

    add_draft(
        &root,
        NewDraft {
            id: id.to_string(),
            title: title.to_string(),
            body,
            kind: "procedure".to_string(),
            scope: "project".to_string(),
            targets,
            evidence,
            confidence,
            reason,
            matched_template: Some("manual:draft-merge".to_string()),
        },
    )?;

    load_drafts(&root)?
        .into_iter()
        .find(|draft| draft.id == id)
        .ok_or_else(|| anyhow!("merged draft `{id}` was not written"))
}

fn draft_path(project_root: &Path, id: &str) -> PathBuf {
    let safe = id.replace(':', "/").replace(['\\', ' '], "-");
    config::kernel_dir(project_root)
        .join("drafts")
        .join(format!("{safe}.yml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approves_draft_into_skilllet() {
        let temp = tempfile::tempdir().expect("tempdir");
        add_draft(
            temp.path(),
            NewDraft {
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
            },
        )
        .expect("add draft");

        approve_draft(temp.path(), "project:prefer-bun").expect("approve");

        assert!(load_drafts(temp.path()).expect("drafts").is_empty());
        let skilllets = skilllet::load_skilllets(temp.path()).expect("skilllets");
        assert_eq!(skilllets[0].id, "project:prefer-bun");
    }

    #[test]
    fn draft_explainability_fields_roundtrip() {
        let temp = tempfile::tempdir().expect("tempdir");
        add_draft(
            temp.path(),
            NewDraft {
                id: "project:prefer-bun".to_string(),
                title: "Prefer Bun".to_string(),
                body: "Use Bun for JavaScript package management and scripts.".to_string(),
                kind: "preference".to_string(),
                scope: "project".to_string(),
                targets: vec!["codex".to_string()],
                evidence: "manual test".to_string(),
                confidence: Some(0.92),
                reason: Some("Matched project preference template".to_string()),
                matched_template: Some("project:prefer-bun".to_string()),
            },
        )
        .expect("add draft");

        let drafts = load_drafts(temp.path()).expect("drafts");

        assert_eq!(drafts[0].confidence, Some(0.92));
        assert_eq!(
            drafts[0].reason.as_deref(),
            Some("Matched project preference template")
        );
        assert_eq!(
            drafts[0].matched_template.as_deref(),
            Some("project:prefer-bun")
        );
    }

    #[test]
    fn updates_draft_reviewable_fields_without_losing_explainability() {
        let temp = tempfile::tempdir().expect("tempdir");
        add_draft(
            temp.path(),
            NewDraft {
                id: "project:prefer-bun".to_string(),
                title: "Prefer Bun".to_string(),
                body: "Use Bun for JavaScript package management and scripts.".to_string(),
                kind: "preference".to_string(),
                scope: "project".to_string(),
                targets: vec!["codex".to_string()],
                evidence: "manual test".to_string(),
                confidence: Some(0.92),
                reason: Some("Matched project preference template".to_string()),
                matched_template: Some("built-in:Prefer Bun".to_string()),
            },
        )
        .expect("add draft");

        let updated = update_draft(
            temp.path(),
            "project:prefer-bun",
            DraftUpdate {
                title: Some("Prefer Bun Runtime".to_string()),
                body: Some(
                    "Use Bun for package management, scripts, and JS runtime tasks.".to_string(),
                ),
                targets: Some(vec!["claude-code".to_string(), "codex".to_string()]),
                ..Default::default()
            },
        )
        .expect("update draft");

        assert_eq!(updated.title, "Prefer Bun Runtime");
        assert_eq!(
            updated.body,
            "Use Bun for package management, scripts, and JS runtime tasks."
        );
        assert_eq!(updated.targets, vec!["claude-code", "codex"]);
        assert_eq!(updated.confidence, Some(0.92));
        assert_eq!(
            updated.matched_template.as_deref(),
            Some("built-in:Prefer Bun")
        );

        let drafts = load_drafts(temp.path()).expect("drafts");
        assert_eq!(drafts[0].title, "Prefer Bun Runtime");
    }

    #[test]
    fn merges_drafts_into_new_reviewable_draft_without_removing_sources() {
        let temp = tempfile::tempdir().expect("tempdir");
        add_draft(
            temp.path(),
            NewDraft {
                id: "project:use-axios".to_string(),
                title: "Use Axios".to_string(),
                body: "Use Axios for frontend HTTP requests.".to_string(),
                kind: "preference".to_string(),
                scope: "project".to_string(),
                targets: vec!["codex".to_string()],
                evidence: "observation:a".to_string(),
                confidence: Some(0.92),
                reason: Some("Matched HTTP client preference".to_string()),
                matched_template: Some("built-in:Use Axios".to_string()),
            },
        )
        .expect("add axios draft");
        add_draft(
            temp.path(),
            NewDraft {
                id: "project:prefer-bun".to_string(),
                title: "Prefer Bun".to_string(),
                body: "Use Bun for package management and scripts.".to_string(),
                kind: "preference".to_string(),
                scope: "project".to_string(),
                targets: vec!["claude-code".to_string()],
                evidence: "observation:b".to_string(),
                confidence: Some(0.84),
                reason: Some("Matched package manager preference".to_string()),
                matched_template: Some("built-in:Prefer Bun".to_string()),
            },
        )
        .expect("add bun draft");

        let merged = merge_drafts(
            temp.path(),
            "project:frontend-defaults",
            "Frontend Defaults",
            vec![
                "project:use-axios".to_string(),
                "project:prefer-bun".to_string(),
            ],
            vec!["codex".to_string(), "claude-code".to_string()],
        )
        .expect("merge drafts");

        assert_eq!(merged.id, "project:frontend-defaults");
        assert_eq!(merged.title, "Frontend Defaults");
        assert_eq!(merged.kind, "procedure");
        assert_eq!(merged.scope, "project");
        assert_eq!(merged.targets, vec!["claude-code", "codex"]);
        assert!(merged.body.contains("## Use Axios"));
        assert!(merged.body.contains("Use Bun for package management"));
        assert!(merged.evidence.contains("Merged Drafts"));
        assert!(merged.evidence.contains("project:use-axios"));
        assert_eq!(merged.confidence, Some(0.84));
        assert!(
            merged
                .reason
                .as_deref()
                .unwrap()
                .contains("Merged 2 drafts")
        );

        let drafts = load_drafts(temp.path()).expect("drafts");
        assert_eq!(drafts.len(), 3);
        assert!(drafts.iter().any(|draft| draft.id == "project:use-axios"));
        assert!(drafts.iter().any(|draft| draft.id == "project:prefer-bun"));
        assert!(
            drafts
                .iter()
                .any(|draft| draft.id == "project:frontend-defaults")
        );
    }

    #[test]
    fn merge_requires_at_least_two_source_drafts() {
        let temp = tempfile::tempdir().expect("tempdir");
        add_draft(
            temp.path(),
            NewDraft {
                id: "project:solo".to_string(),
                title: "Solo".to_string(),
                body: "One draft is not a merge.".to_string(),
                kind: "preference".to_string(),
                scope: "project".to_string(),
                targets: vec!["codex".to_string()],
                evidence: "manual test".to_string(),
                confidence: None,
                reason: None,
                matched_template: None,
            },
        )
        .expect("add solo");

        let err = merge_drafts(
            temp.path(),
            "project:solo-merged",
            "Solo Merged",
            vec!["project:solo".to_string()],
            vec!["codex".to_string()],
        )
        .expect_err("single source should fail");

        assert!(err.to_string().contains("at least two source drafts"));
        assert_eq!(load_drafts(temp.path()).expect("drafts").len(), 1);
    }
}
