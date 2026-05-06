use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SkillletOperation {
    #[default]
    Add,
    Update,
    Supersede,
    Conflict,
    Noop,
}

impl SkillletOperation {
    pub fn as_str(&self) -> &'static str {
        match self {
            SkillletOperation::Add => "add",
            SkillletOperation::Update => "update",
            SkillletOperation::Supersede => "supersede",
            SkillletOperation::Conflict => "conflict",
            SkillletOperation::Noop => "noop",
        }
    }
}
