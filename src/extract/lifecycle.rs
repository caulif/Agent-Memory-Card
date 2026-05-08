use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MemoryCardOperation {
    #[default]
    Add,
    Update,
    Supersede,
    Conflict,
    Noop,
}

impl MemoryCardOperation {
    pub fn as_str(&self) -> &'static str {
        match self {
            MemoryCardOperation::Add => "add",
            MemoryCardOperation::Update => "update",
            MemoryCardOperation::Supersede => "supersede",
            MemoryCardOperation::Conflict => "conflict",
            MemoryCardOperation::Noop => "noop",
        }
    }
}
