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
        if self.dry_run {
            out.push_str("Mode: dry run\n");
        }
        out
    }
}
