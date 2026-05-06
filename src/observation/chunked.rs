use std::collections::BTreeSet;
use std::path::Path;

use anyhow::Result;

use crate::extract;

use super::ObservationRecord;

pub(super) fn synthesis_material(observations: &[ObservationRecord], max_chars: usize) -> String {
    synthesis_material_chunks(observations, max_chars, max_chars)
        .into_iter()
        .next()
        .unwrap_or_default()
}

pub(super) fn extract_llm_chunks_to_report(
    project_root: &Path,
    observations: &[ObservationRecord],
    targets: Vec<String>,
    source: &str,
    max_candidates: usize,
) -> Result<extract::ExtractReport> {
    let chunks = synthesis_material_chunks(observations, 16_000, 80_000);
    let mut merged = extract::ExtractReport {
        created: Vec::new(),
        skipped: Vec::new(),
        candidates: Vec::new(),
        dry_run: true,
        provider: "llm-chunked".to_string(),
        redacted: false,
    };
    let mut seen = BTreeSet::new();

    for (index, chunk) in chunks.iter().enumerate() {
        let chunk_source = format!("{source}, chunk {}/{}", index + 1, chunks.len());
        let report = extract::extract_high_value_text_to_drafts(
            project_root,
            chunk,
            targets.clone(),
            &chunk_source,
            None,
            true,
            max_candidates,
        )?;
        merged.redacted |= report.redacted;
        merged.skipped.extend(report.skipped);
        for candidate in report.candidates {
            if seen.insert(candidate.id.clone()) {
                merged.candidates.push(candidate);
            }
        }
    }

    merged.candidates.sort_by(|a, b| {
        b.confidence
            .unwrap_or(0.0)
            .total_cmp(&a.confidence.unwrap_or(0.0))
            .then_with(|| a.id.cmp(&b.id))
    });
    merged.candidates.truncate(max_candidates);
    Ok(merged)
}

fn synthesis_material_chunks(
    observations: &[ObservationRecord],
    chunk_chars: usize,
    total_max_chars: usize,
) -> Vec<String> {
    let mut sorted = observations.to_vec();
    sorted.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut total = 0usize;

    for observation in sorted {
        if total >= total_max_chars {
            break;
        }
        let entry = observation_entry(&observation);
        if !current.is_empty() && current.len() + entry.len() > chunk_chars {
            total += current.len();
            chunks.push(std::mem::take(&mut current));
            if total >= total_max_chars {
                break;
            }
        }
        current.push_str(&entry);
    }
    if !current.trim().is_empty() && total < total_max_chars {
        chunks.push(current);
    }
    chunks
}

fn observation_entry(observation: &ObservationRecord) -> String {
    let mut body = observation.body.replace('\0', "");
    if body.len() > 4_000 {
        body.truncate(4_000);
    }
    format!(
        "\n---\nsource: {}\nagent: {}\nevidence: {}\ntext:\n{}\n",
        observation.source_kind,
        observation.agent.as_deref().unwrap_or("unknown"),
        observation.evidence,
        body
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(id: &str, created_at: &str, body: &str) -> ObservationRecord {
        ObservationRecord {
            id: id.to_string(),
            source_kind: "test".to_string(),
            source_path: "test.jsonl".to_string(),
            agent: Some("codex".to_string()),
            body: body.to_string(),
            evidence: "test".to_string(),
            redacted: false,
            created_at: created_at.to_string(),
        }
    }

    #[test]
    fn chunks_preserve_recent_observations_without_one_large_lump() {
        let observations = vec![
            observation("old", "2026-01-01T00:00:00Z", &"old ".repeat(2000)),
            observation("new", "2026-01-02T00:00:00Z", "以后用 pnpm。"),
        ];

        let chunks = synthesis_material_chunks(&observations, 500, 10_000);

        assert!(chunks.len() >= 2);
        assert!(chunks[0].contains("以后用 pnpm"));
    }
}
