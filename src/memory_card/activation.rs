use super::{ExtractionMetadata, MemoryCardRecord, infer_activation};

pub(super) fn activation_label_from_classification(value: &str) -> String {
    let normalized = value.replace('_', "-");
    match normalized.as_str() {
        "always-on" | "skill" | "manual" | "path-glob" | "model-decision" => normalized,
        _ => "model-decision".to_string(),
    }
}

pub(super) fn derive_activation(
    extraction: &Option<ExtractionMetadata>,
    kind: &str,
    existing: Option<&MemoryCardRecord>,
) -> String {
    let from_metadata = |metadata: &ExtractionMetadata| {
        metadata
            .classification
            .as_ref()
            .filter(|classification| !classification.activation.trim().is_empty())
            .map(|classification| activation_label_from_classification(&classification.activation))
    };
    if let Some(label) = extraction.as_ref().and_then(from_metadata) {
        return label;
    }
    if let Some(record) = existing {
        if let Some(label) = record.extraction.as_ref().and_then(from_metadata) {
            return label;
        }
        return record.activation.clone();
    }
    infer_activation(kind).to_string()
}
