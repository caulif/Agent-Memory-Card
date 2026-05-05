use serde::{Deserialize, Serialize};

/// 证据块的来源类型，标记内容的原始出处。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChunkOrigin {
    User,
    Assistant,
    Artifact,
    AiSynthesis,
    Unknown,
}

impl ChunkOrigin {
    /// 从字符串标签解析来源类型，不区分大小写。
    pub fn from_label(label: &str) -> Self {
        match label.trim().to_ascii_lowercase().as_str() {
            "user" => Self::User,
            "assistant" => Self::Assistant,
            "artifact" => Self::Artifact,
            "ai-synthesis" | "ai_synthesis" => Self::AiSynthesis,
            _ => Self::Unknown,
        }
    }

    /// 返回来源类型的字符串表示。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Artifact => "artifact",
            Self::AiSynthesis => "ai-synthesis",
            Self::Unknown => "unknown",
        }
    }
}

/// 证据块：提取漏斗的最小可判断单元。
/// 从已有 observations 中派生，无需改写存储层。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceChunk {
    pub id: String,
    pub text: String,
    pub origin: ChunkOrigin,
    pub source_kind: String,
    pub source_observations: Vec<String>,
}

/// 将质量测试用例转换为 EvidenceChunk，供评分管线使用。
pub fn text_case_to_chunk(id: &str, origin: &str, input: &str) -> EvidenceChunk {
    EvidenceChunk {
        id: id.to_string(),
        text: input.trim().to_string(),
        origin: ChunkOrigin::from_label(origin),
        source_kind: "quality-fixture".to_string(),
        source_observations: Vec::new(),
    }
}
