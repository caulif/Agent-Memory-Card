//! Memory Card 完整性校验：扫描 .agent-kernel/memory-cards/**/*.yml，确保每张卡
//! 满足溯源链不空、observation 文件存在、evidence_quote 是原文子串、activation
//! 字段与 extraction.classification.activation 一致等约束。任何不一致都通过
//! 进程退出码非 0 让 CI 立即可见。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::candidate::EvidenceSpan;
use crate::config;
use crate::fsutil;
use crate::memory_card::{self, MemoryCardRecord};
use crate::observation::ObservationRecord;

/// 单张卡的校验失败项。
#[derive(Debug, Clone)]
pub struct VerifyIssue {
    pub memory_card_id: String,
    pub flag: String,
    pub message: String,
}

/// 整体校验报告。
#[derive(Debug, Default)]
pub struct VerifyReport {
    pub checked: usize,
    pub issues: Vec<VerifyIssue>,
}

impl VerifyReport {
    pub fn is_clean(&self) -> bool {
        self.issues.is_empty()
    }

    pub fn render(&self) -> String {
        if self.issues.is_empty() {
            return format!("Memory Card verify: {} card(s) clean.\n", self.checked);
        }
        let mut out = format!(
            "Memory Card verify: {} card(s) checked, {} issue(s) found.\n",
            self.checked,
            self.issues.len()
        );
        for issue in &self.issues {
            out.push_str(&format!(
                "- [{}] {}: {}\n",
                issue.flag, issue.memory_card_id, issue.message
            ));
        }
        out
    }
}

/// 扫描所有已批准 Memory Card 并返回 issue 列表。
pub fn verify_project(project_root: &Path) -> Result<VerifyReport> {
    let root = fsutil::normalize_project_root(project_root)?;
    let cards = memory_card::load_memory_cards(&root)?;
    let mut report = VerifyReport {
        checked: cards.len(),
        issues: Vec::new(),
    };
    let observation_index = build_observation_index(&root)?;

    for card in cards {
        verify_card(&card, &observation_index, &mut report.issues);
    }

    Ok(report)
}

fn verify_card(
    card: &MemoryCardRecord,
    observations: &BTreeMap<String, ObservationRecord>,
    issues: &mut Vec<VerifyIssue>,
) {
    let extraction = match card.extraction.as_ref() {
        Some(metadata) => metadata,
        None => {
            issues.push(VerifyIssue {
                memory_card_id: card.id.clone(),
                flag: "missing-extraction".to_string(),
                message: "Memory Card has no extraction metadata; provenance unverifiable."
                    .to_string(),
            });
            return;
        }
    };

    if extraction.source_observations.is_empty() {
        issues.push(VerifyIssue {
            memory_card_id: card.id.clone(),
            flag: "missing-source-observations".to_string(),
            message: "extraction.source_observations is empty; lineage cannot be traced."
                .to_string(),
        });
    }

    for observation_id in &extraction.source_observations {
        if !observations.contains_key(observation_id) {
            issues.push(VerifyIssue {
                memory_card_id: card.id.clone(),
                flag: "missing-observation-file".to_string(),
                message: format!(
                    "source observation `{observation_id}` not found under .agent-kernel/observations/"
                ),
            });
        }
    }

    if let Some(span) = extraction.evidence_span.as_ref() {
        verify_quote(card, span, observations, issues);
    }

    verify_activation(card, extraction, issues);
}

fn verify_quote(
    card: &MemoryCardRecord,
    span: &EvidenceSpan,
    observations: &BTreeMap<String, ObservationRecord>,
    issues: &mut Vec<VerifyIssue>,
) {
    let quote = span.quote.trim();
    if quote.is_empty() {
        return;
    }
    let candidate_observations: Vec<&ObservationRecord> = match span.observation_id.as_deref() {
        Some(id) => observations.get(id).into_iter().collect(),
        None => observations.values().collect(),
    };
    if candidate_observations.is_empty() {
        return;
    }
    let any_match = candidate_observations
        .iter()
        .any(|observation| observation.body.contains(quote));
    if !any_match {
        issues.push(VerifyIssue {
            memory_card_id: card.id.clone(),
            flag: "evidence-quote-not-found".to_string(),
            message: format!(
                "evidence_span.quote not found verbatim in any source observation body (quote head: \"{}\")",
                quote.chars().take(40).collect::<String>()
            ),
        });
    }
}

fn verify_activation(
    card: &MemoryCardRecord,
    extraction: &crate::candidate::ExtractionMetadata,
    issues: &mut Vec<VerifyIssue>,
) {
    let Some(classification) = extraction.classification.as_ref() else {
        return;
    };
    if classification.activation.trim().is_empty() {
        return;
    }
    let expected = normalize_activation(&classification.activation);
    let actual = card.activation.trim();
    if actual != expected {
        issues.push(VerifyIssue {
            memory_card_id: card.id.clone(),
            flag: "activation-mismatch".to_string(),
            message: format!(
                "record.activation=`{actual}` does not match extraction.classification.activation=`{expected}`"
            ),
        });
    }

    let activation_tag = format!("activation:{expected}");
    let conflicting_tags: Vec<&String> = card
        .tags
        .iter()
        .filter(|tag| tag.starts_with("activation:") && **tag != activation_tag)
        .collect();
    if !conflicting_tags.is_empty() {
        issues.push(VerifyIssue {
            memory_card_id: card.id.clone(),
            flag: "activation-tag-conflict".to_string(),
            message: format!(
                "tags contain {} which conflict with classification activation `{expected}`",
                conflicting_tags
                    .iter()
                    .map(|tag| tag.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        });
    }
}

fn normalize_activation(value: &str) -> String {
    value.replace('_', "-")
}

fn build_observation_index(project_root: &Path) -> Result<BTreeMap<String, ObservationRecord>> {
    let dir = config::kernel_dir(project_root).join("observations");
    let mut map = BTreeMap::new();
    if !dir.exists() {
        return Ok(map);
    }
    for entry in walkdir::WalkDir::new(&dir).follow_links(false) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path: PathBuf = entry.path().to_path_buf();
        if path.extension().and_then(|ext| ext.to_str()) != Some("yml") {
            continue;
        }
        let text = fs::read_to_string(&path)
            .with_context(|| format!("read observation {}", path.display()))?;
        let record: ObservationRecord = match serde_yaml::from_str(&text) {
            Ok(record) => record,
            Err(_) => continue,
        };
        map.insert(record.id.clone(), record);
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::candidate::{EvidenceSpan, ExtractionMetadata};
    use crate::extract::classify::KnowledgeClassification;

    fn card_with_extraction(extraction: Option<ExtractionMetadata>) -> MemoryCardRecord {
        MemoryCardRecord {
            schema_version: 1,
            id: "global:test".to_string(),
            title: "Test".to_string(),
            kind: "preference".to_string(),
            scope: "global".to_string(),
            body: "always do x".to_string(),
            brief: String::new(),
            tags: Vec::new(),
            language: "en".to_string(),
            activation: "always-on".to_string(),
            trigger_description: None,
            source_project: None,
            extraction,
            approved_from: None,
            evidence: None,
            merge_history: Vec::new(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    fn observation_with_body(id: &str, body: &str) -> ObservationRecord {
        ObservationRecord {
            id: id.to_string(),
            source_kind: "test".to_string(),
            source_path: "test".to_string(),
            agent: Some("test".to_string()),
            body: body.to_string(),
            evidence: "test".to_string(),
            redacted: false,
            created_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn missing_extraction_reports_issue() {
        let card = card_with_extraction(None);
        let mut issues = Vec::new();
        verify_card(&card, &BTreeMap::new(), &mut issues);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].flag, "missing-extraction");
    }

    #[test]
    fn empty_source_observations_reports_issue() {
        let extraction = ExtractionMetadata {
            classification: Some(KnowledgeClassification {
                activation: "always_on".to_string(),
                ..Default::default()
            }),
            ..Default::default()
        };
        let card = card_with_extraction(Some(extraction));
        let mut issues = Vec::new();
        verify_card(&card, &BTreeMap::new(), &mut issues);
        assert!(
            issues
                .iter()
                .any(|issue| issue.flag == "missing-source-observations")
        );
    }

    #[test]
    fn activation_mismatch_reports_issue() {
        let extraction = ExtractionMetadata {
            source_observations: vec!["obs:test:abc".to_string()],
            classification: Some(KnowledgeClassification {
                activation: "skill".to_string(),
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut card = card_with_extraction(Some(extraction));
        card.activation = "always-on".to_string();
        let mut observations = BTreeMap::new();
        observations.insert(
            "obs:test:abc".to_string(),
            observation_with_body("obs:test:abc", "any body"),
        );
        let mut issues = Vec::new();
        verify_card(&card, &observations, &mut issues);
        assert!(
            issues
                .iter()
                .any(|issue| issue.flag == "activation-mismatch")
        );
    }

    #[test]
    fn evidence_quote_must_be_substring_of_observation_body() {
        let extraction = ExtractionMetadata {
            source_observations: vec!["obs:test:abc".to_string()],
            evidence_span: Some(EvidenceSpan {
                role: "user".to_string(),
                quote: "phantom quote".to_string(),
                observation_id: Some("obs:test:abc".to_string()),
                turn_id: None,
                surrounding_context: Vec::new(),
            }),
            classification: Some(KnowledgeClassification {
                activation: "always_on".to_string(),
                ..Default::default()
            }),
            ..Default::default()
        };
        let card = card_with_extraction(Some(extraction));
        let mut observations = BTreeMap::new();
        observations.insert(
            "obs:test:abc".to_string(),
            observation_with_body("obs:test:abc", "another body without the magic word"),
        );
        let mut issues = Vec::new();
        verify_card(&card, &observations, &mut issues);
        assert!(
            issues
                .iter()
                .any(|issue| issue.flag == "evidence-quote-not-found")
        );
    }

    #[test]
    fn clean_card_has_no_issues() {
        let extraction = ExtractionMetadata {
            source_observations: vec!["obs:test:abc".to_string()],
            evidence_span: Some(EvidenceSpan {
                role: "user".to_string(),
                quote: "real quote".to_string(),
                observation_id: Some("obs:test:abc".to_string()),
                turn_id: None,
                surrounding_context: Vec::new(),
            }),
            classification: Some(KnowledgeClassification {
                activation: "always_on".to_string(),
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut card = card_with_extraction(Some(extraction));
        card.activation = "always-on".to_string();
        let mut observations = BTreeMap::new();
        observations.insert(
            "obs:test:abc".to_string(),
            observation_with_body("obs:test:abc", "context with real quote inside"),
        );
        let mut issues = Vec::new();
        verify_card(&card, &observations, &mut issues);
        assert!(issues.is_empty(), "{issues:?}");
    }
}
