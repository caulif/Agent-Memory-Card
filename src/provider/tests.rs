use super::*;

#[test]
fn default_provider_is_claude_cli() {
    with_test_env_vars(
        &[("ANTHROPIC_AUTH_TOKEN", None), ("ANTHROPIC_API_KEY", None)],
        || {
            let cfg = ProviderConfig::default();
            assert_eq!(cfg.default, "claude-cli");
            assert_eq!(cfg.extraction_provider, "claude-cli");
            assert_eq!(extraction_provider_name(&cfg), "claude-cli");
            assert!(cfg.providers.contains_key("claude-cli"));
            assert!(cfg.providers.contains_key("local"));
        },
    );
}

#[test]
fn init_writes_provider_config() {
    let temp = tempfile::tempdir().expect("tempdir");
    init_provider_config(temp.path()).expect("init");
    assert!(provider_config_path(temp.path()).expect("path").exists());
}

#[test]
fn redacts_common_secret_shapes() {
    let input = "token=abc123456789xyz and bearer secretBearerToken12345 and sk-abc123456789xyz";
    let redacted = redact_secrets(input);

    assert!(!redacted.contains("abc123456789xyz"));
    assert!(!redacted.contains("secretBearerToken12345"));
    assert!(redacted.contains("[REDACTED]"));
}

#[test]
fn default_extraction_config_fields() {
    with_test_env_vars(
        &[("ANTHROPIC_AUTH_TOKEN", None), ("ANTHROPIC_API_KEY", None)],
        || {
            let cfg = ProviderConfig::default();
            assert_eq!(cfg.extraction_provider, "claude-cli");
            assert_eq!(
                extraction_provider_name(&cfg),
                "claude-cli",
                "extract role should fall back to legacy extraction_provider"
            );
            assert_eq!(cfg.max_candidates_per_batch, 20);
            assert_eq!(cfg.min_confidence, 0.7);
        },
    );
}

#[test]
fn http_provider_timeout_defaults_to_real_extraction_budget() {
    with_test_env_var("AGENT_KERNEL_HTTP_TIMEOUT_SECS", None, || {
        assert_eq!(http_timeout_duration(), std::time::Duration::from_secs(120));
    });
}

#[test]
fn http_provider_timeout_can_be_overridden_for_real_provider_runs() {
    with_test_env_var("AGENT_KERNEL_HTTP_TIMEOUT_SECS", Some("300"), || {
        assert_eq!(http_timeout_duration(), std::time::Duration::from_secs(300));
    });
}

#[test]
fn local_provider_refuses_remote_call() {
    with_test_env_vars(
        &[("ANTHROPIC_AUTH_TOKEN", None), ("ANTHROPIC_API_KEY", None)],
        || {
            let mut cfg = ProviderConfig::default();
            cfg.default = "local".to_string();
            cfg.extraction_provider = "local".to_string();
            let result = call_provider(
                &cfg,
                &ProviderRequest {
                    system_prompt: "system".to_string(),
                    user_prompt: "user".to_string(),
                    json_schema: None,
                },
                512,
            );
            assert!(result.is_err());
            assert!(result.expect_err("must fail").to_string().contains("local"));
        },
    );
}

#[test]
fn is_llm_extraction_enabled_by_default() {
    with_test_env_vars(
        &[("ANTHROPIC_AUTH_TOKEN", None), ("ANTHROPIC_API_KEY", None)],
        || {
            let temp = tempfile::tempdir().expect("tempdir");
            let enabled = is_llm_extraction_enabled(temp.path()).expect("check");
            assert!(enabled);
        },
    );
}

#[test]
fn auto_detect_keeps_claude_cli_default_when_anthropic_key_present() {
    with_test_env_vars(
        &[
            ("ANTHROPIC_AUTH_TOKEN", None),
            ("ANTHROPIC_API_KEY", Some("test-key")),
            ("ANTHROPIC_BASE_URL", None),
            ("ANTHROPIC_MODEL", None),
        ],
        || {
            let cfg = ProviderConfig::auto_detect();
            assert_eq!(cfg.default, "claude-cli");
            assert_eq!(cfg.extraction_provider, "claude-cli");
            assert_eq!(cfg.role_providers.update.as_deref(), None);
            assert!(matches!(
                cfg.providers.get("anthropic"),
                Some(Provider::Anthropic { .. })
            ));
        },
    );
}

#[test]
fn auto_detect_supports_anthropic_compatible_env_names() {
    with_test_env_vars(
        &[
            ("ANTHROPIC_AUTH_TOKEN", Some("test-token")),
            ("ANTHROPIC_API_KEY", None),
            (
                "ANTHROPIC_BASE_URL",
                Some("https://api.deepseek.com/anthropic"),
            ),
            ("ANTHROPIC_MODEL", Some("deepseek-v4-flash")),
        ],
        || {
            let cfg = ProviderConfig::auto_detect();
            assert!(matches!(
                cfg.providers.get("anthropic"),
                Some(Provider::Anthropic { base_url, model, api_key_env, .. })
                    if base_url == "https://api.deepseek.com/anthropic"
                        && model == "deepseek-v4-flash"
                        && api_key_env == "ANTHROPIC_AUTH_TOKEN"
            ));
        },
    );
}

#[test]
fn existing_provider_config_is_augmented_from_anthropic_env() {
    with_test_env_vars(
        &[
            ("ANTHROPIC_AUTH_TOKEN", Some("test-token")),
            ("ANTHROPIC_API_KEY", None),
            (
                "ANTHROPIC_BASE_URL",
                Some("https://api.deepseek.com/anthropic"),
            ),
            ("ANTHROPIC_MODEL", Some("deepseek-v4-flash")),
        ],
        || {
            let temp = tempfile::tempdir().expect("tempdir");
            let cfg = ProviderConfig::local_default();
            std::fs::create_dir_all(config::kernel_dir(temp.path())).expect("kernel dir");
            std::fs::write(
                provider_config_path(temp.path()).expect("provider path"),
                serde_yaml::to_string(&cfg).expect("serialize"),
            )
            .expect("write providers");

            let loaded = load_or_default_provider_config(temp.path()).expect("load providers");
            assert!(matches!(
                loaded.providers.get("anthropic"),
                Some(Provider::Anthropic { base_url, model, api_key_env, .. })
                    if base_url == "https://api.deepseek.com/anthropic"
                        && model == "deepseek-v4-flash"
                        && api_key_env == "ANTHROPIC_AUTH_TOKEN"
            ));
        },
    );
}

#[test]
fn role_provider_overrides_legacy_extraction_provider() {
    let mut cfg = ProviderConfig {
        extraction_provider: "anthropic".to_string(),
        ..ProviderConfig::default()
    };
    cfg.role_providers.extract = Some("openai-compatible".to_string());
    cfg.role_providers.abstract_ = Some("claude-cli".to_string());

    assert_eq!(
        provider_name_for_role(&cfg, ProviderRole::Extract),
        "openai-compatible"
    );
    assert_eq!(
        provider_name_for_role(&cfg, ProviderRole::Update),
        "anthropic"
    );
    assert_eq!(
        provider_name_for_role(&cfg, ProviderRole::Abstract),
        "claude-cli"
    );
    assert_eq!(
        provider_name_for_role(&cfg, ProviderRole::Refine),
        "anthropic"
    );
}

#[test]
fn anthropic_request_body_marks_system_prompt_cacheable() {
    let body = anthropic_request_body(
        "claude-sonnet-4-5",
        &ProviderRequest {
            system_prompt: "system prompt".to_string(),
            user_prompt: "user prompt".to_string(),
            json_schema: None,
        },
        512,
        true,
        Some("1h"),
        true,
    );

    assert_eq!(body["model"], "claude-sonnet-4-5");
    assert_eq!(body["messages"][0]["content"], "user prompt");
    assert_eq!(body["system"][0]["cache_control"]["type"], "ephemeral");
    assert_eq!(body["system"][0]["cache_control"]["ttl"], "1h");
}

#[test]
fn anthropic_request_body_includes_json_schema_output_config() {
    let body = anthropic_request_body(
        "claude-sonnet-4-5",
        &ProviderRequest {
            system_prompt: "system prompt".to_string(),
            user_prompt: "user prompt".to_string(),
            json_schema: Some(ProviderJsonSchema {
                name: "AtomicFactExtraction".to_string(),
                strict: true,
                schema: serde_json::json!({
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "title": { "type": "string" }
                        },
                        "required": ["title"]
                    }
                }),
            }),
        },
        1024,
        true,
        Some("1h"),
        true,
    );

    assert_eq!(body["output_config"]["format"]["type"], "json_schema");
    assert_eq!(
        body["output_config"]["format"]["schema"]["name"],
        "AtomicFactExtraction"
    );
    assert_eq!(body["output_config"]["format"]["schema"]["strict"], true);
}

#[test]
fn anthropic_unstructured_json_body_removes_output_config() {
    let body = anthropic_request_body(
        "deepseek-v4-flash",
        &ProviderRequest {
            system_prompt: "system prompt".to_string(),
            user_prompt: "extract candidates".to_string(),
            json_schema: Some(ProviderJsonSchema {
                name: "Candidates".to_string(),
                strict: false,
                schema: serde_json::json!({
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "title": { "type": "string" }
                        }
                    }
                }),
            }),
        },
        1024,
        true,
        Some("1h"),
        true,
    );

    let fallback = anthropic_unstructured_json_body(&body);

    assert!(fallback.get("output_config").is_none());
    assert_eq!(fallback["model"], "deepseek-v4-flash");
    assert_eq!(fallback["max_tokens"], 4096);
    let prompt = fallback["messages"][0]["content"].as_str().expect("prompt");
    assert!(prompt.contains("extract candidates"));
    assert!(prompt.contains("Return only valid JSON"));
    assert!(prompt.contains("\"type\":\"array\""));
}

#[test]
fn anthropic_request_body_can_prompt_for_json_without_output_config() {
    let body = anthropic_request_body(
        "deepseek-v4-flash",
        &ProviderRequest {
            system_prompt: "system prompt".to_string(),
            user_prompt: "extract candidates".to_string(),
            json_schema: Some(ProviderJsonSchema {
                name: "Candidates".to_string(),
                strict: false,
                schema: serde_json::json!({
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "title": { "type": "string" }
                        }
                    }
                }),
            }),
        },
        1024,
        false,
        None,
        false,
    );

    assert!(body.get("output_config").is_none());
    let prompt = body["messages"][0]["content"].as_str().expect("prompt");
    assert!(prompt.contains("extract candidates"));
    assert!(prompt.contains("Return only valid JSON"));
    assert!(prompt.contains("\"type\":\"array\""));
}

#[test]
fn anthropic_beta_header_combines_cache_and_structured_output_betas() {
    assert_eq!(
        anthropic_beta_header(Some("1h"), true).as_deref(),
        Some("extended-cache-ttl-2025-04-11,structured-outputs-2025-11-13")
    );
    assert_eq!(
        anthropic_beta_header(None, true).as_deref(),
        Some("structured-outputs-2025-11-13")
    );
}

#[test]
fn cli_provider_can_round_trip_prompt_via_stdin() {
    let script_dir = tempfile::tempdir().expect("tempdir");

    #[cfg(windows)]
    let (binary, args) = {
        let script_path = script_dir.path().join("echo-provider.ps1");
        std::fs::write(&script_path, "@($input) -join \"`n\" | Write-Output").expect("script");
        (
            "powershell",
            vec![
                "-NoProfile".to_string(),
                "-File".to_string(),
                script_path.display().to_string(),
            ],
        )
    };

    #[cfg(not(windows))]
    let (binary, args) = {
        let script_path = script_dir.path().join("echo-provider.sh");
        std::fs::write(&script_path, "cat\n").expect("script");
        ("sh", vec![script_path.display().to_string()])
    };
    let output = call_cli_subprocess(
        binary,
        &args,
        &ProviderRequest {
            system_prompt: "system prompt".to_string(),
            user_prompt: "user prompt".to_string(),
            json_schema: None,
        },
        5,
    )
    .expect("cli output");

    assert!(output.contains("system prompt"));
    assert!(output.contains("user prompt"));
}

#[test]
fn debug_redacts_api_key_env() {
    let openai = Provider::OpenAiCompatible {
        base_url: "http://localhost:11434/v1".into(),
        model: "qwen2.5-coder:7b".into(),
        api_key_env: "OPENAI_API_KEY".into(),
    };
    let anthropic = Provider::Anthropic {
        base_url: "https://api.anthropic.com".into(),
        model: "claude-sonnet-4-5".into(),
        api_key_env: "ANTHROPIC_API_KEY".into(),
        cache_system_prompt: true,
        cache_ttl: Some("1h".into()),
    };

    let openai_dbg = format!("{openai:?}");
    let anthropic_dbg = format!("{anthropic:?}");

    // 确认包含非敏感的 provider 和 model 信息
    assert!(openai_dbg.contains("OpenAiCompatible"));
    assert!(openai_dbg.contains("qwen2.5-coder:7b"));
    assert!(anthropic_dbg.contains("Anthropic"));
    assert!(anthropic_dbg.contains("claude-sonnet-4-5"));

    // 确认不暴露真实环境变量名
    assert!(!openai_dbg.contains("OPENAI_API_KEY"));
    assert!(!anthropic_dbg.contains("ANTHROPIC_API_KEY"));

    // 确认使用了占位标记
    assert!(openai_dbg.contains("<redacted>"));
    assert!(anthropic_dbg.contains("<redacted>"));
}

#[test]
fn debug_provider_config_contains_providers_without_secrets() {
    let cfg = ProviderConfig::default();
    let dbg = format!("{cfg:?}");

    // ProviderConfig 使用 Provider 的自定义 Debug，不应泄露密钥
    assert!(!dbg.contains("OPENAI_API_KEY"));
    assert!(!dbg.contains("ANTHROPIC_API_KEY"));
    assert!(dbg.contains("OpenAiCompatible"));
    assert!(dbg.contains("LocalHeuristic"));
}
