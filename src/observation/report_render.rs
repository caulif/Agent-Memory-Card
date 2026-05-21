use super::{
    ObservationEvolveReport, ObservationImportReport, ObservationReplayReport,
    ObservationSynthesisReport,
};

impl ObservationImportReport {
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("Agent Memory Kernel observation import\n\n");
        out.push_str(&format!("Created observations: {}\n", self.created));
        out.push_str(&format!("Skipped sources: {}\n", self.skipped));
        if !self.observations.is_empty() {
            out.push_str("\nObservations:\n");
            for id in &self.observations {
                out.push_str(&format!("- {id}\n"));
            }
        }
        out
    }
}

impl ObservationSynthesisReport {
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("Agent Memory Kernel observation synthesis\n\n");
        out.push_str(&format!("Engine: {}\n", self.engine));
        out.push_str(&format!("Candidates written: {}\n", self.created));
        out.push_str(&format!("Candidate previews: {}\n", self.candidates));
        out.push_str(&format!("Skipped observations: {}\n", self.skipped));
        if !self.drafts.is_empty() {
            out.push_str("\nDrafts:\n");
            for id in &self.drafts {
                out.push_str(&format!("- {id}\n"));
            }
        }
        if !self.candidate_drafts.is_empty() {
            out.push_str("\nCandidate records:\n");
            for id in &self.candidate_drafts {
                out.push_str(&format!("- {id}\n"));
            }
        }
        if self.dry_run {
            out.push_str("Mode: dry run\n");
        }
        out
    }
}

impl ObservationEvolveReport {
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("Agent Memory Kernel local evolution\n\n");
        out.push_str(&format!("Engine: {}\n", self.engine));
        out.push_str(&format!("Observations imported: {}\n", self.imported));
        out.push_str(&format!("Import skipped: {}\n", self.import_skipped));
        out.push_str(&format!("Candidates written: {}\n", self.drafts_created));
        out.push_str(&format!("Candidate previews: {}\n", self.draft_candidates));
        out.push_str(&format!("Synthesis skipped: {}\n", self.synthesis_skipped));
        if !self.drafts.is_empty() {
            out.push_str("\nDrafts:\n");
            for id in &self.drafts {
                out.push_str(&format!("- {id}\n"));
            }
        }
        if !self.candidate_drafts.is_empty() {
            out.push_str("\nCandidate records:\n");
            for id in &self.candidate_drafts {
                out.push_str(&format!("- {id}\n"));
            }
        }
        if self.dry_run {
            out.push_str("Mode: dry run\n");
        }
        out
    }
}

impl ObservationReplayReport {
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("Agent Memory Kernel local replay\n\n");
        out.push_str(&format!("Engine: {}\n", self.engine));
        out.push_str(&format!(
            "Discovered sources: {}\n",
            self.discovered_sources
        ));
        out.push_str(&format!("Matched sources: {}\n", self.matched_sources));
        out.push_str(&format!(
            "Excluded other-project sources: {}\n",
            self.excluded_other_project
        ));
        out.push_str(&format!(
            "Excluded unknown-project sources: {}\n",
            self.excluded_unknown_project
        ));
        out.push_str(&format!("Observations imported: {}\n", self.imported));
        out.push_str(&format!("Import skipped: {}\n", self.import_skipped));
        out.push_str(&format!("Candidates written: {}\n", self.drafts_created));
        out.push_str(&format!("Candidate previews: {}\n", self.draft_candidates));
        out.push_str(&format!("Synthesis skipped: {}\n", self.synthesis_skipped));
        if !self.drafts.is_empty() {
            out.push_str("\nDrafts:\n");
            for id in &self.drafts {
                out.push_str(&format!("- {id}\n"));
            }
        }
        if !self.candidate_drafts.is_empty() {
            out.push_str("\nCandidate records:\n");
            for id in &self.candidate_drafts {
                out.push_str(&format!("- {id}\n"));
            }
        }
        if !self.candidate_previews.is_empty() {
            out.push_str("\nCandidate preview text:\n");
            for candidate in &self.candidate_previews {
                out.push_str(&format!(
                    "- {} [{} / {} / {}] confidence={:.2} template={}\n  title: {}\n  body: {}\n",
                    candidate.id,
                    candidate.scope,
                    candidate.kind,
                    candidate.memory_tier,
                    candidate.confidence.unwrap_or(0.0),
                    candidate.matched_template.as_deref().unwrap_or(""),
                    candidate.title,
                    candidate.body
                ));
            }
        }
        if self.dry_run {
            out.push_str("Mode: dry run\n");
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observation::CandidatePreviewText;

    #[test]
    fn replay_render_includes_candidate_preview_text() {
        let report = ObservationReplayReport {
            engine: "local".to_string(),
            discovered_sources: 1,
            matched_sources: 1,
            excluded_other_project: 0,
            excluded_unknown_project: 0,
            imported: 1,
            import_skipped: 0,
            drafts_created: 0,
            draft_candidates: 1,
            synthesis_skipped: 0,
            drafts: Vec::new(),
            candidate_drafts: vec!["global:validate-real-history".to_string()],
            candidate_previews: vec![CandidatePreviewText {
                id: "global:validate-real-history".to_string(),
                title: "用真实历史验证提炼质量".to_string(),
                body: "修改提炼质量相关代码时，先跑合成测试和本地真实历史 dry-run。".to_string(),
                scope: "global".to_string(),
                kind: "procedure".to_string(),
                memory_tier: "collaboration_preference".to_string(),
                confidence: Some(0.91),
                matched_template: Some("global-flow:real-history-validation".to_string()),
            }],
            dry_run: true,
        };

        let rendered = report.render();

        assert!(rendered.contains("Candidate preview text"), "{rendered}");
        assert!(rendered.contains("用真实历史验证提炼质量"), "{rendered}");
        assert!(
            rendered.contains("global-flow:real-history-validation"),
            "{rendered}"
        );
    }
}
