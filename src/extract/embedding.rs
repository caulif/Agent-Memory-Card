//! 语义去重层：优先使用 fastembed，本地不可用时回退到 Jaccard。

use std::sync::Mutex;

use fastembed::TextEmbedding;

use crate::skilllet::SkillletRecord;
use crate::textutil;

pub(crate) trait SemanticMatcher: Send + Sync {
    fn compute_similarity(&self, left: &str, right: &str) -> f32;

    #[cfg_attr(not(test), allow(dead_code))]
    fn matcher_name(&self) -> &'static str;
}

pub(crate) struct JaccardMatcher;

impl JaccardMatcher {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl SemanticMatcher for JaccardMatcher {
    fn compute_similarity(&self, left: &str, right: &str) -> f32 {
        textutil::jaccard_similarity(left, right)
    }

    fn matcher_name(&self) -> &'static str {
        "jaccard"
    }
}

pub(crate) struct FastEmbedMatcher {
    model: Mutex<TextEmbedding>,
}

impl FastEmbedMatcher {
    pub(crate) fn try_new() -> anyhow::Result<Self> {
        let model = TextEmbedding::try_new(Default::default())?;
        Ok(Self {
            model: Mutex::new(model),
        })
    }

    fn embed_pair(&self, left: &str, right: &str) -> anyhow::Result<(Vec<f32>, Vec<f32>)> {
        let mut model = self
            .model
            .lock()
            .map_err(|_| anyhow::anyhow!("fastembed model lock poisoned"))?;
        let embeddings = model.embed(vec![left.to_string(), right.to_string()], None)?;
        let left = embeddings
            .first()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("missing first embedding"))?;
        let right = embeddings
            .get(1)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("missing second embedding"))?;
        Ok((left, right))
    }
}

impl SemanticMatcher for FastEmbedMatcher {
    fn compute_similarity(&self, left: &str, right: &str) -> f32 {
        self.embed_pair(left, right)
            .map(|(left, right)| cosine_similarity(&left, &right))
            .unwrap_or_else(|_| textutil::jaccard_similarity(left, right))
    }

    fn matcher_name(&self) -> &'static str {
        "fastembed"
    }
}

pub(crate) struct SemanticDeduper {
    matcher: Mutex<Option<Box<dyn SemanticMatcher>>>,
    existing_threshold: f32,
    batch_threshold: f32,
}

impl SemanticDeduper {
    pub(crate) fn new(existing_threshold: f32, batch_threshold: f32) -> Self {
        Self {
            matcher: Mutex::new(None),
            existing_threshold,
            batch_threshold,
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn with_matcher(
        matcher: Box<dyn SemanticMatcher>,
        existing_threshold: f32,
        batch_threshold: f32,
    ) -> Self {
        Self {
            matcher: Mutex::new(Some(matcher)),
            existing_threshold,
            batch_threshold,
        }
    }

    fn compute_similarity(&self, left: &str, right: &str) -> f32 {
        let mut matcher = self
            .matcher
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let matcher = matcher.get_or_insert_with(default_matcher);
        matcher.compute_similarity(left, right)
    }

    pub(crate) fn dedup_against_existing(
        &self,
        body: &str,
        existing_skilllets: &[SkillletRecord],
    ) -> DedupResult {
        for skilllet in existing_skilllets {
            let similarity = self.compute_similarity(body, &skilllet.body);
            if similarity > self.existing_threshold {
                return DedupResult::Duplicate {
                    similar_id: skilllet.id.clone(),
                    similarity,
                };
            }
        }
        DedupResult::Unique
    }

    pub(crate) fn top_similar_skilllets(
        &self,
        body: &str,
        existing_skilllets: &[SkillletRecord],
        limit: usize,
        min_similarity: f32,
    ) -> Vec<(String, String)> {
        let mut scored = existing_skilllets
            .iter()
            .filter_map(|skilllet| {
                let similarity = self.compute_similarity(body, &skilllet.body);
                (similarity >= min_similarity)
                    .then(|| (skilllet.id.clone(), skilllet.body.clone(), similarity))
            })
            .collect::<Vec<_>>();
        scored.sort_by(|left, right| right.2.total_cmp(&left.2));
        scored
            .into_iter()
            .take(limit)
            .map(|(id, body, _)| (id, body))
            .collect()
    }

    pub(crate) fn dedup_within_batch(&self, items: &mut [LlmKnowledgeItem]) -> Vec<usize> {
        let mut retained = Vec::new();
        let mut kept_items: Vec<&LlmKnowledgeItem> = Vec::new();

        for (i, item) in items.iter().enumerate() {
            if item.is_noise {
                continue;
            }

            let mut is_duplicate = false;
            for kept in &kept_items {
                let similarity = self.compute_similarity(&item.body, &kept.body);
                if similarity > self.batch_threshold {
                    is_duplicate = true;
                    break;
                }
            }

            if !is_duplicate {
                kept_items.push(item);
                retained.push(i);
            }
        }

        retained
    }
}

pub(crate) fn default_matcher() -> Box<dyn SemanticMatcher> {
    if cfg!(test) {
        return Box::new(JaccardMatcher::new());
    }

    if std::env::var("AGENT_KERNEL_DISABLE_FASTEMBED")
        .ok()
        .is_some_and(|value| value == "1")
    {
        return Box::new(JaccardMatcher::new());
    }

    FastEmbedMatcher::try_new()
        .map(|matcher| Box::new(matcher) as Box<dyn SemanticMatcher>)
        .unwrap_or_else(|_| Box::new(JaccardMatcher::new()))
}

#[derive(Debug, Clone)]
pub(crate) struct LlmKnowledgeItem {
    pub title: String,
    pub body: String,
    pub kind: String,
    pub scope: String,
    pub memory_tier: crate::candidate::MemoryTier,
    pub abstraction_of: Option<String>,
    pub abstracted_from: Option<String>,
    pub confidence: f32,
    pub evidence: String,
    pub reason: String,
    pub matched_signal: String,
    pub is_noise: bool,
    pub suggested_action: crate::candidate::ExtractionAction,
    pub durability_score: Option<f32>,
    pub reusability_score: Option<f32>,
    pub specificity_score: Option<f32>,
    pub source_trust_score: Option<f32>,
}

#[derive(Debug, Clone)]
pub(crate) enum DedupResult {
    Unique,
    Duplicate { similar_id: String, similarity: f32 },
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    if left.is_empty() || right.is_empty() || left.len() != right.len() {
        return 0.0;
    }

    let mut dot = 0.0f32;
    let mut left_norm = 0.0f32;
    let mut right_norm = 0.0f32;
    for (left, right) in left.iter().zip(right.iter()) {
        dot += left * right;
        left_norm += left * left;
        right_norm += right * right;
    }
    if left_norm == 0.0 || right_norm == 0.0 {
        return 0.0;
    }
    dot / (left_norm.sqrt() * right_norm.sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn with_fastembed_disabled<T>(f: impl FnOnce() -> T) -> T {
        let _guard = env_lock().lock().expect("env lock");
        let previous = std::env::var("AGENT_KERNEL_DISABLE_FASTEMBED").ok();
        unsafe { std::env::set_var("AGENT_KERNEL_DISABLE_FASTEMBED", "1") };
        let result = f();
        match previous.as_deref() {
            Some(value) => unsafe { std::env::set_var("AGENT_KERNEL_DISABLE_FASTEMBED", value) },
            None => unsafe { std::env::remove_var("AGENT_KERNEL_DISABLE_FASTEMBED") },
        }
        result
    }

    #[test]
    fn default_matcher_falls_back_to_jaccard_when_fastembed_is_disabled() {
        with_fastembed_disabled(|| {
            let matcher = default_matcher();
            assert_eq!(matcher.matcher_name(), "jaccard");
        });
    }

    #[test]
    fn cosine_similarity_matches_identical_vectors() {
        let similarity = cosine_similarity(&[1.0, 2.0, 3.0], &[1.0, 2.0, 3.0]);
        assert!((similarity - 1.0).abs() < 0.0001);
    }

    #[test]
    fn jaccard_matcher_detects_similar_texts() {
        let matcher = JaccardMatcher::new();
        let sim = matcher.compute_similarity(
            "Use Bun for JavaScript package management",
            "Use Bun for package management and scripts",
        );
        assert!(
            sim > 0.5,
            "similar texts should have high similarity: {sim}"
        );
    }

    #[test]
    fn semantic_deduper_detects_duplicate_against_existing_skilllet() {
        let existing = vec![SkillletRecord {
            schema_version: 1,
            id: "project:prefer-bun".to_string(),
            title: "Prefer Bun".to_string(),
            kind: "preference".to_string(),
            scope: "project".to_string(),
            body: "Use Bun for JavaScript package management and scripts.".to_string(),
            brief: String::new(),
            tags: Vec::new(),
            language: "en".to_string(),
            activation: "always-on".to_string(),
            trigger_description: None,
            source_project: None,
            extraction: None,
            approved_from: None,
            evidence: None,
            merge_history: Vec::new(),
            created_at: String::new(),
            updated_at: String::new(),
        }];

        let deduper = SemanticDeduper::with_matcher(Box::new(JaccardMatcher::new()), 0.75, 0.65);
        let result = deduper
            .dedup_against_existing("Use Bun for JS package management and scripts", &existing);

        match result {
            DedupResult::Duplicate { similar_id, .. } => {
                assert_eq!(similar_id, "project:prefer-bun")
            }
            DedupResult::Unique => panic!("should detect duplicate"),
        }
    }

    #[test]
    fn semantic_deduper_does_not_initialize_matcher_for_empty_existing_set() {
        let deduper = SemanticDeduper::new(0.75, 0.65);

        let result = deduper.dedup_against_existing("Use Bun for package management", &[]);

        assert!(matches!(result, DedupResult::Unique));
        assert!(
            deduper
                .matcher
                .lock()
                .expect("matcher lock")
                .as_ref()
                .is_none(),
            "matcher should stay lazy when there is nothing to compare"
        );
    }
}
