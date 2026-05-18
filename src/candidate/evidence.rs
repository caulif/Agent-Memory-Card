use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceTrust {
    UserDirect,
    UserFeedback,
    AssistantSummary,
    ToolOutput,
    Artifact,
    Unknown,
}

impl Default for SourceTrust {
    fn default() -> Self {
        Self::Unknown
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceValidity {
    Valid,
    Weak,
    Invalid,
}

impl Default for EvidenceValidity {
    fn default() -> Self {
        Self::Weak
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EvidenceQuoteRecord {
    pub observation_id: String,
    pub text: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EvidenceContextRecord {
    pub observation_id: String,
    #[serde(default)]
    pub before: Vec<String>,
    #[serde(default)]
    pub after: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EvidenceBundle {
    #[serde(default)]
    pub source_observation_ids: Vec<String>,
    #[serde(default)]
    pub quotes: Vec<EvidenceQuoteRecord>,
    #[serde(default)]
    pub context: Vec<EvidenceContextRecord>,
    #[serde(default)]
    pub source_trust: SourceTrust,
    #[serde(default)]
    pub validity: EvidenceValidity,
}
