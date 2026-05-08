use super::ExtractionAction;

impl ExtractionAction {
    pub fn new_candidate() -> Self {
        Self::new_candidate_for_route("always_on_rule")
    }

    pub fn new_candidate_for_route(route: &str) -> Self {
        Self {
            action: "new_candidate".to_string(),
            route: normalize_route(route),
            target_record: None,
            compile_enabled: Some(compile_enabled_for_route(route)),
            record_id: None,
            similarity: None,
            reason: None,
            rationale: Some(rationale_for_route(route)),
        }
    }

    pub fn merge_into_existing(record_id: String, similarity: f32) -> Self {
        Self::merge_into_existing_for_route(record_id, similarity, "always_on_rule")
    }

    pub fn merge_into_existing_for_route(record_id: String, similarity: f32, route: &str) -> Self {
        Self {
            action: "merge_into_existing".to_string(),
            route: normalize_route(route),
            target_record: Some(record_id.clone()),
            compile_enabled: Some(compile_enabled_for_route(route)),
            record_id: Some(record_id),
            similarity: Some(similarity),
            reason: Some(
                "Similar durable knowledge already exists; review as a merge instead of creating another Memory Card."
                    .to_string(),
            ),
            rationale: Some(rationale_for_route(route)),
        }
    }

    pub fn with_route(mut self, route: &str) -> Self {
        self.route = normalize_route(route);
        self.compile_enabled = Some(compile_enabled_for_route(route));
        self.rationale = Some(rationale_for_route(route));
        if self.target_record.is_none() {
            self.target_record = self.record_id.clone();
        }
        self
    }
}

fn normalize_route(route: &str) -> String {
    match route {
        "always_on_rule" | "workflow_skill" | "skill_supplement" | "review_only" => {
            route.to_string()
        }
        _ => "review_only".to_string(),
    }
}

fn compile_enabled_for_route(route: &str) -> bool {
    route == "always_on_rule"
}

fn rationale_for_route(route: &str) -> String {
    match route {
        "always_on_rule" => {
            "Compile this Memory Card into AGENTS.md / CLAUDE.md after review.".to_string()
        }
        "workflow_skill" => {
            "Keep this Memory Card as a workflow Skill draft for a SKILL.md target.".to_string()
        }
        "skill_supplement" => {
            "Attach this Memory Card as supplemental guidance to an existing Skill.".to_string()
        }
        "review_only" => {
            "Keep this Memory Card in review/library only; do not compile by default.".to_string()
        }
        _ => "Keep this Memory Card in review/library only; do not compile by default.".to_string(),
    }
}
