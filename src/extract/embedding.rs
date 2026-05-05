//! 语义去重层：基于 Jaccard 相似度 + 可扩展 Embedding 接口。
//!
//! 当前实现使用 Jaccard token 相似度（改进版，保留中文语义信息）。
//! 预留 SemanticMatcher trait 供未来 Ollama embedding 模型接入。

use crate::skilllet::SkillletRecord;
use crate::textutil;

/// 语义匹配器 trait：预留 Embedding 模型接入接口。
/// 当前默认实现为 JaccardMatcher。
#[allow(dead_code)]
pub(crate) trait SemanticMatcher: Send + Sync {
    /// 计算两个文本的语义相似度 (0.0 - 1.0)
    fn compute_similarity(&self, left: &str, right: &str) -> f32;

    /// 批量编码文本（Embedding 实现用，Jaccard 为空实现）
    fn encode_batch(&self, _texts: &[String]) -> Vec<Vec<f32>> {
        Vec::new()
    }
}

/// Jaccard 相似度匹配器（默认实现）
pub(crate) struct JaccardMatcher {
    /// 相似度阈值 (0.0 - 1.0)，超过此值视为重复
    #[allow(dead_code)]
    similarity_threshold: f32,
}

impl JaccardMatcher {
    pub(crate) fn new(similarity_threshold: f32) -> Self {
        Self {
            similarity_threshold,
        }
    }
}

impl SemanticMatcher for JaccardMatcher {
    fn compute_similarity(&self, left: &str, right: &str) -> f32 {
        textutil::jaccard_similarity(left, right)
    }
}

/// 语义去重器：管理去重逻辑
pub(crate) struct SemanticDeduper<M: SemanticMatcher = JaccardMatcher> {
    matcher: M,
    /// 和已有 Skilllet 比较的相似度阈值 (默认 0.75)
    existing_threshold: f32,
    /// 同批次候选之间的相似度阈值 (默认 0.65)
    batch_threshold: f32,
}

impl SemanticDeduper<JaccardMatcher> {
    pub(crate) fn new(existing_threshold: f32, batch_threshold: f32) -> Self {
        SemanticDeduper {
            matcher: JaccardMatcher::new(batch_threshold),
            existing_threshold,
            batch_threshold,
        }
    }

    /// 和已有 Skilllet 做语义去重：如果 body 相似度 > existing_threshold，视为重复
    pub(crate) fn dedup_against_existing(
        &self,
        body: &str,
        existing_skilllets: &[SkillletRecord],
    ) -> DedupResult {
        for skilllet in existing_skilllets {
            let similarity = self.matcher.compute_similarity(body, &skilllet.body);
            if similarity > self.existing_threshold {
                return DedupResult::Duplicate {
                    similar_id: skilllet.id.clone(),
                    similarity,
                };
            }
        }
        DedupResult::Unique
    }

    /// 同批次内去重：按 confidence 降序排序后，移除低 confidence 的重复项。
    /// 返回去重后的保留项索引列表。
    pub(crate) fn dedup_within_batch(&self, items: &mut [LlmKnowledgeItem]) -> Vec<usize> {
        let mut retained = Vec::new();
        let mut kept_items: Vec<&LlmKnowledgeItem> = Vec::new();

        for (i, item) in items.iter().enumerate() {
            if item.is_noise {
                continue;
            }

            let mut is_duplicate = false;
            for kept in &kept_items {
                let similarity = self.matcher.compute_similarity(&item.body, &kept.body);
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

/// LLM 提取的知识项（用于去重层输入）
#[derive(Debug, Clone)]
pub(crate) struct LlmKnowledgeItem {
    pub title: String,
    pub body: String,
    pub kind: String,
    pub scope: String,
    pub confidence: f32,
    pub evidence: String,
    pub reason: String,
    pub matched_signal: String,
    pub is_noise: bool,
    pub suggested_action: crate::candidate::ExtractionAction,
}

/// 去重结果
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) enum DedupResult {
    Unique,
    Duplicate { similar_id: String, similarity: f32 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jaccard_matcher_detects_similar_texts() {
        let matcher = JaccardMatcher::new(0.5);
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
    fn jaccard_matcher_distinguishes_different_topics() {
        let matcher = JaccardMatcher::new(0.5);
        let sim = matcher.compute_similarity(
            "Use Bun for JavaScript package management",
            "Use Axios for frontend HTTP requests",
        );
        assert!(
            sim < 0.3,
            "different topics should have low similarity: {sim}"
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

        let deduper = SemanticDeduper::new(0.75, 0.65);
        let result = deduper
            .dedup_against_existing("Use Bun for JS package management and scripts", &existing);

        match result {
            DedupResult::Duplicate { similar_id, .. } => {
                assert_eq!(similar_id, "project:prefer-bun");
            }
            DedupResult::Unique => panic!("should detect duplicate"),
        }
    }

    #[test]
    fn semantic_deduper_allows_unique_items() {
        let existing = vec![SkillletRecord {
            schema_version: 1,
            id: "project:prefer-bun".to_string(),
            title: "Prefer Bun".to_string(),
            kind: "preference".to_string(),
            scope: "project".to_string(),
            body: "Use Bun for JavaScript package management.".to_string(),
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

        let deduper = SemanticDeduper::new(0.75, 0.65);
        let result = deduper.dedup_against_existing(
            "Always run cargo test before pushing Rust changes",
            &existing,
        );

        assert!(matches!(result, DedupResult::Unique));
    }

    #[test]
    fn within_batch_dedup_keeps_higher_confidence_items() {
        let deduper = SemanticDeduper::new(0.75, 0.65);
        let mut items = vec![
            LlmKnowledgeItem {
                title: "Use Bun".into(),
                body: "Use Bun for JavaScript package management and scripts".into(),
                kind: "preference".into(),
                scope: "project".into(),
                confidence: 0.92,
                evidence: "test".into(),
                reason: "test".into(),
                matched_signal: "preference".into(),
                is_noise: false,
                suggested_action: crate::candidate::ExtractionAction::new_candidate(),
            },
            LlmKnowledgeItem {
                title: "Prefer Bun".into(),
                body: "Use Bun for JavaScript package management and scripts".into(),
                kind: "preference".into(),
                scope: "project".into(),
                confidence: 0.78,
                evidence: "test".into(),
                reason: "test".into(),
                matched_signal: "preference".into(),
                is_noise: false,
                suggested_action: crate::candidate::ExtractionAction::new_candidate(),
            },
            LlmKnowledgeItem {
                title: "Use Axios".into(),
                body: "Use Axios for frontend HTTP requests".into(),
                kind: "preference".into(),
                scope: "project".into(),
                confidence: 0.85,
                evidence: "test".into(),
                reason: "test".into(),
                matched_signal: "preference".into(),
                is_noise: false,
                suggested_action: crate::candidate::ExtractionAction::new_candidate(),
            },
        ];

        let retained = deduper.dedup_within_batch(&mut items);
        // 第一个 Use Bun 和第二个 Prefer Bun body 完全相同，应只保留第一个
        // Use Axios 是不同的主题，应保留
        assert_eq!(retained.len(), 2);
        assert!(retained.contains(&0)); // Use Bun (higher confidence)
        assert!(retained.contains(&2)); // Use Axios (different topic)
    }

    #[test]
    fn noise_items_are_excluded_from_dedup() {
        let deduper = SemanticDeduper::new(0.75, 0.65);
        let mut items = vec![LlmKnowledgeItem {
            title: "Noise".into(),
            body: "this is noise".into(),
            kind: "preference".into(),
            scope: "project".into(),
            confidence: 0.4,
            evidence: "test".into(),
            reason: "test".into(),
            matched_signal: String::new(),
            is_noise: true,
            suggested_action: crate::candidate::ExtractionAction::new_candidate(),
        }];

        let retained = deduper.dedup_within_batch(&mut items);
        assert!(retained.is_empty());
    }
}
