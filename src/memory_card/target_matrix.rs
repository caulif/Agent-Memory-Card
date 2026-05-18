use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::config;
use crate::fsutil;

use super::load_memory_cards;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCardTargetMatrix {
    pub agents: Vec<String>,
    pub rows: Vec<MemoryCardTargetMatrixRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCardTargetMatrixRow {
    pub memory_card_id: String,
    pub title: String,
    pub scope: String,
    pub targets: std::collections::BTreeMap<String, bool>,
}

impl MemoryCardTargetMatrix {
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("Agent Memory Kernel Memory Card Target Matrix\n\n");
        if self.rows.is_empty() {
            out.push_str("No Memory Cards found.\n");
            return out;
        }
        out.push_str(&format!("Memory Card | {}\n", self.agents.join(" | ")));
        out.push_str(&format!(
            "{}\n",
            std::iter::repeat_n("---", self.agents.len() + 1)
                .collect::<Vec<_>>()
                .join(" | ")
        ));
        for row in &self.rows {
            let cells = self
                .agents
                .iter()
                .map(|agent| {
                    if row.targets.get(agent).copied().unwrap_or(false) {
                        "yes"
                    } else {
                        "no"
                    }
                })
                .collect::<Vec<_>>();
            out.push_str(&format!("{} | {}\n", row.memory_card_id, cells.join(" | ")));
        }
        out
    }
}

pub fn memory_card_target_matrix(project_root: &Path) -> Result<MemoryCardTargetMatrix> {
    let root = fsutil::normalize_project_root(project_root)?;
    let project = config::load_or_default_project_config(&root)?;
    let memory_cards = load_memory_cards(&root)?;
    let agents = project.agents.keys().cloned().collect::<Vec<_>>();
    let refs = project
        .memory_cards
        .include
        .iter()
        .map(|item| (item.id.clone(), item.targets.clone()))
        .collect::<std::collections::BTreeMap<_, _>>();

    let rows = memory_cards
        .into_iter()
        .map(|record| {
            let explicit_targets = refs.get(&record.id);
            let targets = agents
                .iter()
                .map(|agent| {
                    let assigned = if let Some(explicit_targets) = explicit_targets {
                        explicit_targets.iter().any(|target| target == agent)
                    } else {
                        project
                            .agents
                            .get(agent)
                            .map(|agent| agent.enabled)
                            .unwrap_or(false)
                    };
                    (agent.clone(), assigned)
                })
                .collect();
            MemoryCardTargetMatrixRow {
                memory_card_id: record.id,
                title: record.title,
                scope: record.scope,
                targets,
            }
        })
        .collect();

    Ok(MemoryCardTargetMatrix { agents, rows })
}
