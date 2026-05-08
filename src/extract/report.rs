use super::ExtractReport;
use std::collections::BTreeMap;

impl ExtractReport {
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("Agent Memory Kernel extract report\n\n");
        out.push_str(&format!("Provider: {}\n\n", self.provider));
        if self.redacted {
            out.push_str("Secrets: redacted\n\n");
        }
        let metrics = self.metrics();
        if !metrics.tier_mix.is_empty() {
            out.push_str("Metrics:\n");
            out.push_str(&format!(
                "  tier_mix: {}\n",
                format_counts(&metrics.tier_mix)
            ));
            if !metrics.placement_mix.is_empty() {
                out.push_str(&format!(
                    "  placement_mix: {}\n",
                    format_counts(&metrics.placement_mix)
                ));
            }
            out.push_str(&format!("  fallback_count: {}\n", metrics.fallback_count));
            out.push_str(&format!(
                "  feedback_penalty_count: {}\n\n",
                metrics.feedback_penalty_count
            ));
        }
        if self.created.is_empty() {
            if self.dry_run && !self.candidates.is_empty() {
                out.push_str("Draft candidates:\n");
                for candidate in &self.candidates {
                    out.push_str(&format!("- {}: {}\n", candidate.id, candidate.body));
                    out.push_str(&format!(
                        "  memory_tier: {}\n",
                        candidate.memory_tier.as_str()
                    ));
                    if let Some(confidence) = candidate.confidence {
                        out.push_str(&format!("  confidence: {:.0}%\n", confidence * 100.0));
                    }
                    if let Some(template) = candidate.matched_template.as_deref() {
                        out.push_str(&format!("  matched_template: {template}\n"));
                    }
                    if let Some(reason) = candidate.reason.as_deref() {
                        out.push_str(&format!("  reason: {reason}\n"));
                    }
                    if let Some(classification) = candidate.classification.as_ref() {
                        out.push_str(&format!(
                            "  classification: signal={}, artifact={}, hardness={}, activation={}\n",
                            classification.signal,
                            classification.artifact_kind,
                            classification.hardness,
                            classification.activation,
                        ));
                    }
                    if !candidate.tags.is_empty() {
                        out.push_str(&format!("  tags: {}\n", candidate.tags.join(", ")));
                    }
                }
            } else {
                out.push_str("No drafts created.\n");
            }
        } else {
            out.push_str("Drafts created:\n");
            for id in &self.created {
                out.push_str(&format!("- {id}\n"));
            }
        }
        if !self.skipped.is_empty() {
            out.push_str("\nSkipped:\n");
            for item in &self.skipped {
                out.push_str(&format!("- {item}\n"));
            }
        }
        out
    }

    pub fn metrics(&self) -> ExtractReportMetrics {
        let mut tier_mix = BTreeMap::new();
        let mut placement_mix = BTreeMap::new();
        let mut fallback_count = 0usize;
        for candidate in &self.candidates {
            *tier_mix
                .entry(candidate.memory_tier.as_str().to_string())
                .or_insert(0) += 1;
            if let Some(classification) = candidate.classification.as_ref() {
                *placement_mix
                    .entry(classification.artifact_kind.clone())
                    .or_insert(0) += 1;
            }
            if candidate
                .matched_template
                .as_deref()
                .is_some_and(|template| template.contains("fallback-methodology-template"))
            {
                fallback_count += 1;
            }
        }
        let feedback_penalty_count = self
            .skipped
            .iter()
            .filter(|item| item.contains("feedback-rejected"))
            .count()
            + self
                .candidates
                .iter()
                .filter(|candidate| {
                    candidate
                        .reason
                        .as_deref()
                        .is_some_and(|reason| reason.contains("feedback-rejected"))
                })
                .count();
        ExtractReportMetrics {
            tier_mix,
            placement_mix,
            fallback_count,
            feedback_penalty_count,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractReportMetrics {
    pub tier_mix: BTreeMap<String, usize>,
    pub placement_mix: BTreeMap<String, usize>,
    pub fallback_count: usize,
    pub feedback_penalty_count: usize,
}

fn format_counts(counts: &BTreeMap<String, usize>) -> String {
    counts
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join(", ")
}
