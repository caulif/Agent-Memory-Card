use super::*;
use crate::extract::truncate::TruncatedMessage;

fn truncated(id: &str, body: &str) -> TruncatedMessage {
    TruncatedMessage {
        observation_id: id.to_string(),
        role: "unknown".to_string(),
        body: body.to_string(),
        original_len: body.len(),
        truncated_len: body.len(),
        was_truncated: false,
        created_at: "2026-05-01T00:00:00+00:00".to_string(),
        source_kind: "claude-code-session".to_string(),
    }
}

fn cluster_with(messages: Vec<TruncatedMessage>) -> MessageCluster {
    MessageCluster {
        cluster_id: "c_test_001".to_string(),
        recurrence: messages.len(),
        messages,
        mean_similarity: 0.85,
        backend: "fastembed".to_string(),
    }
}

#[test]
fn batch_induce_prompt_trims_long_message_bodies() {
    let long_body = format!("{}{}", "真实历史回归".repeat(120), "尾部不应出现在 prompt");
    let cluster = cluster_with(vec![truncated("o1", &long_body)]);

    let request = build_batch_induce_request(&[cluster], 3);

    assert!(request.user_prompt.contains("真实历史回归"));
    assert!(!request.user_prompt.contains("尾部不应出现在 prompt"));
    assert!(request.user_prompt.chars().count() < 2500);
}

#[test]
fn induce_system_prompt_treats_project_workflow_as_first_class_memory() {
    assert!(SYSTEM_PROMPT.contains("项目工作流"));
    assert!(SYSTEM_PROMPT.contains("产品细节"));
    assert!(SYSTEM_PROMPT.contains("不能直接沉淀成卡"));
}

#[test]
fn batch_candidate_preserves_recurrence_and_clamps_singleton_confidence() {
    let recurring = cluster_with(vec![
        truncated("o1", "真实历史回归比静态样例更重要。"),
        truncated("o2", "真实历史回归应该作为验收基准。"),
    ]);
    let singleton = MessageCluster {
        cluster_id: "c_single".to_string(),
        recurrence: 1,
        messages: vec![truncated("o3", "候选质量优先于数量。")],
        mean_similarity: 1.0,
        backend: "jaccard".to_string(),
    };
    let raw = r#"[
        {"title":"优先真实历史回归","when":"评估提炼质量时","what":"优先使用真实历史回归","why":"让规则接受真实数据检验","kind":"procedure","scope":"global","evidence_quotes":[{"observation_id":"o1","text":"真实历史回归"}],"temporal_status":"stable","confidence":0.95},
        {"title":"优先保证候选质量","when":"生成候选时","what":"保证候选质量优先于数量","why":"减少低价值候选","kind":"preference","scope":"global","evidence_quotes":[{"observation_id":"o3","text":"候选质量优先"}],"temporal_status":"stable","confidence":0.95}
    ]"#;

    let candidates =
        parse_batch_induce_response(raw, &[recurring, singleton]).expect("batch parses");

    assert_eq!(candidates[0].recurrence, 2);
    assert!((candidates[0].confidence - 0.95).abs() < 1e-6);
    assert_eq!(candidates[1].recurrence, 1);
    assert!(candidates[1].confidence <= SINGLETON_CONFIDENCE_CEILING);
}

#[test]
fn batch_induce_request_uses_prompt_json_without_structured_schema() {
    let cluster = cluster_with(vec![truncated("o1", "真实历史回归比静态样例更重要。")]);

    let request = build_batch_induce_request(&[cluster], 3);

    assert!(
        request.json_schema.is_none(),
        "batch provider calls should avoid provider-specific structured output wrappers"
    );
    assert!(request.user_prompt.contains("\"candidates\""));
}

#[test]
fn batch_induce_request_demands_concise_fields() {
    let cluster = cluster_with(vec![truncated("o1", "真实历史回归比静态样例更重要。")]);

    let request = build_batch_induce_request(&[cluster], 3);

    assert!(request.user_prompt.contains("Keep field values concise"));
    assert!(
        request
            .user_prompt
            .contains("boundary under 40 Chinese characters")
    );
}

#[test]
fn batch_induce_provider_call_gets_large_json_token_budget() {
    struct TokenCaptureProvider {
        seen_tokens: std::sync::Mutex<Vec<usize>>,
    }

    impl InduceProvider for TokenCaptureProvider {
        fn call(&self, _request: &ProviderRequest, max_tokens: usize) -> Result<String> {
            self.seen_tokens.lock().unwrap().push(max_tokens);
            Ok(r#"{"candidates":[]}"#.to_string())
        }
    }

    let cluster = cluster_with(vec![truncated("o1", "真实历史回归比静态样例更重要。")]);
    let provider = TokenCaptureProvider {
        seen_tokens: std::sync::Mutex::new(Vec::new()),
    };

    let _ = induce_batch_with_provider(&provider, &[cluster], 3);

    assert_eq!(provider.seen_tokens.lock().unwrap().as_slice(), &[8192]);
}

#[test]
fn incomplete_json_response_reports_likely_truncation() {
    let raw = r#"{"candidates":[{"title":"优先真实历史回归","boundary":"没有闭合"#;

    let error = parse_batch_induce_response(raw, &[]).expect_err("truncated JSON should fail");

    assert!(
        error.to_string().contains("likely-truncated-json"),
        "error should point to truncation, got: {error:?}"
    );
}

#[test]
fn batch_response_accepts_candidates_object_wrapper() {
    let cluster = cluster_with(vec![truncated("o1", "真实历史回归比静态样例更重要。")]);
    let raw = r#"{
        "candidates": [
            {"title":"优先真实历史回归","when":"评估提炼质量时","what":"优先使用真实历史回归","why":"让规则接受真实数据检验","kind":"procedure","scope":"global","evidence_quotes":[{"observation_id":"o1","text":"真实历史回归"}],"temporal_status":"stable","confidence":0.9}
        ]
    }"#;

    let candidates = parse_batch_induce_response(raw, &[cluster]).expect("object wrapper parses");

    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].title, "优先真实历史回归");
}

#[test]
fn batch_response_keeps_valid_items_when_another_item_has_bad_evidence() {
    let cluster = cluster_with(vec![truncated("o1", "真实历史回归比静态样例更重要。")]);
    let raw = r#"[
        {"title":"优先真实历史回归","when":"评估提炼质量时","what":"优先使用真实历史回归","why":"让规则接受真实数据检验","kind":"procedure","scope":"global","evidence_quotes":[{"observation_id":"o1","text":"真实历史回归"}],"temporal_status":"stable","confidence":0.9},
        {"title":"坏证据候选","when":"任何时候","what":"编造不存在的事实","why":"错误示例","kind":"procedure","scope":"global","evidence_quotes":[{"observation_id":"o-missing","text":"不存在"}],"temporal_status":"stable","confidence":0.9}
    ]"#;

    let candidates = parse_batch_induce_response(raw, &[cluster]).expect("keeps valid items");

    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].title, "优先真实历史回归");
}
