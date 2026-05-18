use agent_kernel::textutil;

#[test]
fn slug_keeps_ascii_and_cjk_words() {
    assert_eq!(
        textutil::slug("Use Axios for HTTP requests!"),
        "use-axios-for-http-requests"
    );
    assert_eq!(textutil::slug("统一使用 Axios 请求"), "统一使用-axios-请求");
    assert_eq!(textutil::slug("!!!"), "draft");
}

#[test]
fn normalize_string_list_trims_sorts_and_dedupes() {
    let values = vec![
        " codex ".to_string(),
        "".to_string(),
        "claude-code".to_string(),
        "codex".to_string(),
    ];

    assert_eq!(
        textutil::normalize_string_list(values),
        vec!["claude-code".to_string(), "codex".to_string()]
    );
}

#[test]
fn similarity_matches_near_duplicate_http_preferences() {
    let left = "Use Axios for frontend HTTP requests.";
    let right = "Prefer Axios for frontend API requests.";

    assert!(textutil::jaccard_similarity(left, right) >= 0.5);
}

#[test]
fn similarity_does_not_merge_unrelated_tool_preferences() {
    let left = "Use Axios for frontend HTTP requests.";
    let right = "Use Bun for JavaScript package management and scripts.";

    assert!(textutil::jaccard_similarity(left, right) < 0.35);
}
