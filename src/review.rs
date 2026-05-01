use std::path::Path;

use anyhow::Result;

use crate::build;
use crate::draft;
use crate::rule_test;

pub struct ReviewReport {
    sections: Vec<(String, String)>,
}

impl ReviewReport {
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("Agent-Kernel Review\n\n");
        for (title, body) in &self.sections {
            out.push_str(&format!("## {title}\n\n{body}\n"));
        }
        out
    }
}

pub fn review_project(project_root: &Path) -> Result<ReviewReport> {
    let drafts = draft::load_drafts(project_root)?;
    let draft_body = if drafts.is_empty() {
        "No drafts pending.\n".to_string()
    } else {
        drafts
            .iter()
            .map(|draft| format!("- {}: {} [{}]", draft.id, draft.title, draft.status))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"
    };

    let status = build::status_project(project_root)?.render();
    let rule_tests = rule_test::run_rule_tests(project_root)?.render();
    let build_preview = build::build_project(project_root, true)?.render();

    Ok(ReviewReport {
        sections: vec![
            ("Draft Inbox".to_string(), draft_body),
            ("Mirror Status".to_string(), status),
            ("Rule CI".to_string(), rule_tests),
            ("Build Preview".to_string(), build_preview),
        ],
    })
}
