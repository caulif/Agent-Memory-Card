use std::collections::BTreeMap;
use std::path::Path;

use agent_kernel::candidate::{
    EvidenceBundle, EvidenceQuoteRecord, EvidenceValidity, ExtractionMetadata, LayerTraceEntry,
    SourceTrust,
};
use agent_kernel::draft::{NewDraft, add_draft};
use agent_kernel::extract::induce::InducedCandidate;
use agent_kernel::extract::pipeline::PipelineReport;
use anyhow::Result;

pub fn save_pipeline_cards(project: &Path, report: &PipelineReport) -> Result<()> {
    let global_trace: Vec<LayerTraceEntry> =
        ["strip", "truncate", "cluster", "induce", "crystallize"]
            .iter()
            .filter_map(|layer| {
                report
                    .layer_timings_ms
                    .get(*layer)
                    .map(|ms| LayerTraceEntry {
                        layer: layer.to_string(),
                        ms: *ms,
                        info: BTreeMap::new(),
                    })
            })
            .collect();

    for card in &report.cards {
        let id = format!("pipeline:{}", card.cluster_id);
        let evidence = card
            .evidence_quotes
            .iter()
            .map(|q| format!("{}: {}", q.observation_id, q.text))
            .collect::<Vec<_>>()
            .join("\n");
        let mut per_card_trace = global_trace.clone();
        for entry in per_card_trace.iter_mut() {
            match entry.layer.as_str() {
                "cluster" => {
                    entry
                        .info
                        .insert("cluster_id".to_string(), card.cluster_id.clone());
                    entry
                        .info
                        .insert("recurrence".to_string(), card.recurrence.to_string());
                }
                "induce" => {
                    entry
                        .info
                        .insert("temporal_status".to_string(), card.temporal_status.clone());
                    entry
                        .info
                        .insert("confidence".to_string(), format!("{:.3}", card.confidence));
                    entry.info.insert(
                        "prompt_hash".to_string(),
                        agent_kernel::extract::induce::induce_prompt_hash(),
                    );
                }
                _ => {}
            }
        }
        let extraction = ExtractionMetadata {
            origin: "pipeline-v2".to_string(),
            matched_signal: format!("cluster:{} recurrence:{}", card.cluster_id, card.recurrence),
            reason: format!("temporal_status={}", card.temporal_status),
            source_observations: card
                .evidence_quotes
                .iter()
                .map(|q| q.observation_id.clone())
                .collect(),
            evidence_bundle: Some(EvidenceBundle {
                source_observation_ids: card
                    .evidence_quotes
                    .iter()
                    .map(|q| q.observation_id.clone())
                    .collect(),
                quotes: card
                    .evidence_quotes
                    .iter()
                    .map(|q| EvidenceQuoteRecord {
                        observation_id: q.observation_id.clone(),
                        text: q.text.clone(),
                        role: "unknown".to_string(),
                        created_at: String::new(),
                    })
                    .collect(),
                context: Vec::new(),
                source_trust: SourceTrust::UserDirect,
                validity: EvidenceValidity::Valid,
            }),
            tags: card.tags.clone(),
            pipeline_version: Some(report.pipeline_version),
            layer_trace: per_card_trace,
            ..Default::default()
        };
        add_draft(
            project,
            NewDraft {
                id,
                title: card.title.clone(),
                kind: card.kind.clone(),
                scope: card.scope.clone(),
                body: card.body.clone(),
                targets: Vec::new(),
                evidence,
                confidence: Some(card.confidence),
                reason: Some(format!("pipeline-v2 recurrence={}", card.recurrence)),
                matched_template: Some("pipeline-v2".to_string()),
                extraction,
            },
        )?;
    }

    for rejected in &report.crystallize_rejects {
        let candidate = &rejected.candidate;
        let id = format!("pipeline-needs-edit:{}", rejected.cluster_id);
        let evidence = candidate
            .evidence_quotes
            .iter()
            .map(|q| format!("{}: {}", q.observation_id, q.text))
            .collect::<Vec<_>>()
            .join("\n");
        let mut per_card_trace = global_trace.clone();
        for entry in per_card_trace.iter_mut() {
            match entry.layer.as_str() {
                "cluster" => {
                    entry
                        .info
                        .insert("cluster_id".to_string(), rejected.cluster_id.clone());
                    entry
                        .info
                        .insert("recurrence".to_string(), candidate.recurrence.to_string());
                }
                "induce" => {
                    entry.info.insert(
                        "temporal_status".to_string(),
                        candidate.temporal_status.clone(),
                    );
                    entry.info.insert(
                        "confidence".to_string(),
                        format!("{:.3}", candidate.confidence),
                    );
                    entry.info.insert(
                        "prompt_hash".to_string(),
                        agent_kernel::extract::induce::induce_prompt_hash(),
                    );
                }
                "crystallize" => {
                    entry
                        .info
                        .insert("rejected".to_string(), rejected.reasons.join("; "));
                }
                _ => {}
            }
        }
        let source_observations = candidate
            .evidence_quotes
            .iter()
            .map(|q| q.observation_id.clone())
            .collect::<Vec<_>>();
        let extraction = ExtractionMetadata {
            origin: "pipeline-v2".to_string(),
            matched_signal: format!(
                "cluster:{} recurrence:{}",
                rejected.cluster_id, candidate.recurrence
            ),
            reason: format!("crystallize rejected: {}", rejected.reasons.join("; ")),
            source_observations: source_observations.clone(),
            evidence_bundle: Some(EvidenceBundle {
                source_observation_ids: source_observations,
                quotes: candidate
                    .evidence_quotes
                    .iter()
                    .map(|q| EvidenceQuoteRecord {
                        observation_id: q.observation_id.clone(),
                        text: q.text.clone(),
                        role: "unknown".to_string(),
                        created_at: String::new(),
                    })
                    .collect(),
                context: Vec::new(),
                source_trust: SourceTrust::UserDirect,
                validity: EvidenceValidity::Valid,
            }),
            tags: vec!["needs-edit".to_string(), "crystallize-reject".to_string()],
            pipeline_version: Some(report.pipeline_version),
            layer_trace: per_card_trace,
            rejected_at: Some("crystallize".to_string()),
            ..Default::default()
        };
        add_draft(
            project,
            NewDraft {
                id,
                title: candidate.title.clone(),
                kind: candidate.kind.clone(),
                scope: candidate.scope.clone(),
                body: render_rejected_candidate_body(candidate),
                targets: Vec::new(),
                evidence,
                confidence: Some(candidate.confidence),
                reason: Some(format!(
                    "pipeline-v2 needs edit: {}",
                    rejected.reasons.join("; ")
                )),
                matched_template: Some("pipeline-v2:needs-edit".to_string()),
                extraction,
            },
        )?;
    }
    Ok(())
}

pub fn write_pipeline_report_json(project: &Path, report: &PipelineReport) -> Result<()> {
    let dir = project.join("docs").join("runs");
    std::fs::create_dir_all(&dir)?;
    let ts = chrono::Utc::now().format("%Y%m%dT%H%M%S");
    let path = dir.join(format!("pipeline-{ts}.json"));
    std::fs::write(&path, serde_json::to_string_pretty(report)?)?;
    println!("Wrote {}", path.display());
    Ok(())
}

fn render_rejected_candidate_body(candidate: &InducedCandidate) -> String {
    let when = candidate.when.trim().trim_end_matches(['，', ',']);
    let what = candidate
        .what
        .trim()
        .trim_end_matches(['；', ';', '。', '.']);
    let why = candidate.why.trim().trim_end_matches(['。', '.']);
    let when_clause = if when.starts_with("当") || when.starts_with("在") {
        if when.ends_with("时") {
            when.to_string()
        } else {
            format!("{when}时")
        }
    } else {
        format!("当{when}时")
    };
    format!("{when_clause}，{what}；目标是{why}。")
}

#[cfg(test)]
mod tests {
    use agent_kernel::extract::induce::{EvidenceQuote, InducedCandidate};
    use agent_kernel::extract::pipeline::{CrystallizeReject, PipelineReport};

    use super::*;

    #[test]
    fn save_pipeline_cards_writes_crystallize_rejects_as_editable_drafts() {
        let temp = tempfile::tempdir().expect("tempdir");
        let report = PipelineReport {
            observations_in: 1,
            stripped_kept: 1,
            truncated_count: 1,
            truncated_long_messages: 0,
            clusters_in: 1,
            induce_accepted: 1,
            induce_rejected: 0,
            induce_failed: 0,
            induce_calls: 1,
            induce_clusters_selected: 1,
            crystallize_accepted: 0,
            crystallize_rejected: 1,
            cards: Vec::new(),
            crystallize_rejects: vec![CrystallizeReject {
                cluster_id: "c_needs_edit".to_string(),
                candidate: InducedCandidate {
                    cluster_id: "c_needs_edit".to_string(),
                    recurrence: 2,
                    title: "代码变更后重复质量审查".to_string(),
                    when: "完成代码变更后".to_string(),
                    what: "执行代码质量审查并在修复后再次审查".to_string(),
                    why: "执行代码质量审查并在修复后再次审查".to_string(),
                    boundary: Some("只适用于完成代码变更后的质量审查流程。".to_string()),
                    kind: "procedure".to_string(),
                    scope: "global".to_string(),
                    evidence_quotes: vec![EvidenceQuote {
                        observation_id: "obs:1".to_string(),
                        text: "执行代码质量审查".to_string(),
                    }],
                    memory_tier: Some("cross_project_principle".to_string()),
                    abstraction_level: Some("good".to_string()),
                    support_level: Some("strong".to_string()),
                    temporal_status: "stable".to_string(),
                    confidence: 0.9,
                },
                reasons: vec!["brief-body overlap too high".to_string()],
            }],
            stage_metrics: Vec::new(),
            failures_preview: Vec::new(),
            layer_timings_ms: Default::default(),
            pipeline_version: 1,
        };

        save_pipeline_cards(temp.path(), &report).expect("save");
        let drafts = agent_kernel::draft::load_drafts(temp.path()).expect("drafts");

        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].id, "pipeline-needs-edit:c_needs_edit");
        assert_eq!(
            drafts[0].extraction.rejected_at.as_deref(),
            Some("crystallize")
        );
        assert!(
            drafts[0]
                .reason
                .as_deref()
                .unwrap_or_default()
                .contains("brief-body overlap")
        );
    }
}
