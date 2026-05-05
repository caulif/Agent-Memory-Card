use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use regex::Regex;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::config;
use crate::fsutil;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub default: String,
    #[serde(default = "default_extraction_provider")]
    pub extraction_provider: String,
    #[serde(default = "default_max_candidates_per_batch")]
    pub max_candidates_per_batch: usize,
    #[serde(default = "default_min_confidence")]
    pub min_confidence: f32,
    pub providers: BTreeMap<String, Provider>,
    pub privacy: PrivacyConfig,
}

#[derive(Debug, Clone)]
pub struct ProviderRequest {
    pub system_prompt: String,
    pub user_prompt: String,
}

fn default_extraction_provider() -> String {
    "local".to_string()
}

fn default_max_candidates_per_batch() -> usize {
    20
}

fn default_min_confidence() -> f32 {
    0.7
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Provider {
    LocalHeuristic,
    OpenAiCompatible {
        base_url: String,
        model: String,
        api_key_env: String,
    },
    Anthropic {
        model: String,
        api_key_env: String,
        #[serde(default = "default_cache_system_prompt")]
        cache_system_prompt: bool,
    },
}

fn default_cache_system_prompt() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyConfig {
    pub upload_policy: String,
    pub redact_secrets: bool,
    pub include_code_context: bool,
    pub store_prompts_locally: bool,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self::local_default()
    }
}

impl ProviderConfig {
    fn local_default() -> Self {
        let mut providers = BTreeMap::new();
        providers.insert("local".to_string(), Provider::LocalHeuristic);
        providers.insert(
            "openai-compatible".to_string(),
            Provider::OpenAiCompatible {
                base_url: "http://localhost:11434/v1".to_string(),
                model: "qwen2.5-coder:7b".to_string(),
                api_key_env: "OPENAI_API_KEY".to_string(),
            },
        );
        Self {
            default: "local".to_string(),
            extraction_provider: "local".to_string(),
            max_candidates_per_batch: 20,
            min_confidence: 0.7,
            providers,
            privacy: PrivacyConfig {
                upload_policy: "ask".to_string(),
                redact_secrets: true,
                include_code_context: false,
                store_prompts_locally: true,
            },
        }
    }

    pub fn auto_detect() -> Self {
        let mut cfg = Self::local_default();
        if std::env::var("ANTHROPIC_API_KEY")
            .ok()
            .is_some_and(|value| !value.trim().is_empty())
        {
            cfg.providers.insert(
                "anthropic".to_string(),
                Provider::Anthropic {
                    model: "claude-sonnet-4-5".to_string(),
                    api_key_env: "ANTHROPIC_API_KEY".to_string(),
                    cache_system_prompt: true,
                },
            );
            cfg.default = "anthropic".to_string();
            cfg.extraction_provider = "anthropic".to_string();
        }
        cfg
    }
}

pub fn provider_config_path(project_root: &Path) -> Result<std::path::PathBuf> {
    let root = fsutil::normalize_project_root(project_root)?;
    Ok(config::kernel_dir(&root).join("providers.yml"))
}

pub fn init_provider_config(project_root: &Path) -> Result<ProviderConfig> {
    let root = fsutil::normalize_project_root(project_root)?;
    config::ensure_kernel_dir(&root)?;
    let cfg = ProviderConfig::auto_detect();
    fs::write(
        config::kernel_dir(&root).join("providers.yml"),
        serde_yaml::to_string(&cfg)?,
    )
    .context("write providers.yml")?;
    Ok(cfg)
}

pub fn load_or_default_provider_config(project_root: &Path) -> Result<ProviderConfig> {
    let path = provider_config_path(project_root)?;
    if !path.exists() {
        return Ok(ProviderConfig::auto_detect());
    }
    let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    serde_yaml::from_str(&text).with_context(|| format!("parse {}", path.display()))
}

pub fn provider_exists(project_root: &Path, name: &str) -> Result<bool> {
    let cfg = load_or_default_provider_config(project_root)?;
    Ok(cfg.providers.contains_key(name))
}

pub fn redact_secrets(input: &str) -> String {
    let patterns = [
        r"sk-[A-Za-z0-9_-]{12,}",
        r"ghp_[A-Za-z0-9_]{12,}",
        r"github_pat_[A-Za-z0-9_]{12,}",
        r"AKIA[0-9A-Z]{12,}",
        r"(?i)bearer\s+[A-Za-z0-9._~+/=-]{12,}",
        r#"(?i)(api[_-]?key|token|secret|password)\s*[:=]\s*['"]?[^'"\s,;]+"#,
    ];
    let mut redacted = input.to_string();
    for pattern in patterns {
        let re = Regex::new(pattern).expect("valid secret regex");
        redacted = re.replace_all(&redacted, "[REDACTED]").to_string();
    }
    redacted
}

/// 调用远程 LLM 提供商进行文本提取。
pub fn call_provider(
    cfg: &ProviderConfig,
    request: &ProviderRequest,
    max_tokens: usize,
) -> Result<String> {
    let provider_name = &cfg.extraction_provider;

    if provider_name == "local" {
        return Err(anyhow!(
            "local provider does not support remote calls; use --engine local for regex extraction"
        ));
    }

    let provider = cfg
        .providers
        .get(provider_name)
        .ok_or_else(|| anyhow!("provider `{provider_name}` is not configured"))?;

    match provider {
        Provider::LocalHeuristic => Err(anyhow!(
            "local-heuristic provider cannot make remote LLM calls"
        )),
        Provider::OpenAiCompatible {
            base_url,
            model,
            api_key_env,
        } => {
            let api_key = std::env::var(api_key_env).unwrap_or_default();
            let body = serde_json::json!({
                "model": model,
                "messages": [
                    {
                        "role": "system",
                        "content": request.system_prompt
                    },
                    {
                        "role": "user",
                        "content": request.user_prompt
                    }
                ],
                "temperature": 0.3,
                "max_tokens": max_tokens,
                "response_format": {"type": "json_object"},
                "stream": false
            });

            call_openai_compatible_api(base_url, &api_key, &body)
        }
        Provider::Anthropic {
            model,
            api_key_env,
            cache_system_prompt,
        } => {
            let api_key = std::env::var(api_key_env)
                .with_context(|| format!("missing Anthropic API key in {api_key_env}"))?;
            let body = anthropic_request_body(model, request, max_tokens, *cache_system_prompt);
            call_anthropic_api(&api_key, &body)
        }
    }
}

fn call_openai_compatible_api(
    base_url: &str,
    api_key: &str,
    body: &serde_json::Value,
) -> Result<String> {
    let endpoint = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let client = http_client()?;
    let mut builder = client.post(endpoint).json(body);
    if !api_key.is_empty() {
        builder = builder.bearer_auth(api_key);
    }
    let json = send_json(builder)?;

    json.get("choices")
        .and_then(|v| v.as_array())
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(|content| content.as_str())
        .map(str::to_string)
        .ok_or_else(|| anyhow!("provider returned no message content"))
}

fn call_anthropic_api(api_key: &str, body: &serde_json::Value) -> Result<String> {
    let client = http_client()?;
    let json = send_json(
        client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .json(body),
    )?;

    json.get("content")
        .and_then(|v| v.as_array())
        .and_then(|items| {
            items.iter().find_map(|item| {
                if item.get("type").and_then(|value| value.as_str()) == Some("text") {
                    item.get("text").and_then(|value| value.as_str())
                } else {
                    None
                }
            })
        })
        .map(str::to_string)
        .ok_or_else(|| anyhow!("Anthropic response returned no text content"))
}

fn anthropic_request_body(
    model: &str,
    request: &ProviderRequest,
    max_tokens: usize,
    cache_system_prompt: bool,
) -> serde_json::Value {
    let system = if cache_system_prompt {
        serde_json::json!([{
            "type": "text",
            "text": request.system_prompt,
            "cache_control": { "type": "ephemeral" }
        }])
    } else {
        serde_json::json!(request.system_prompt)
    };

    serde_json::json!({
        "model": model,
        "max_tokens": max_tokens,
        "system": system,
        "messages": [{
            "role": "user",
            "content": request.user_prompt
        }]
    })
}

fn http_client() -> Result<Client> {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .context("build HTTP client")
}

fn send_json(builder: reqwest::blocking::RequestBuilder) -> Result<serde_json::Value> {
    let response = builder.send().context("send provider request")?;
    let status = response.status();
    let text = response.text().context("read provider response")?;
    if !status.is_success() {
        return Err(anyhow!("provider HTTP {}: {}", status, text));
    }
    serde_json::from_str(&text).context("parse provider response JSON")
}

/// 查看提取提供商的名称
pub fn extraction_provider_name(cfg: &ProviderConfig) -> &str {
    &cfg.extraction_provider
}

/// 检查是否使用了 LLM 提取引擎
pub fn is_llm_extraction_enabled(project_root: &Path) -> Result<bool> {
    let cfg = load_or_default_provider_config(project_root)?;
    Ok(cfg.extraction_provider != "local")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn with_env_var<T>(key: &str, value: Option<&str>, f: impl FnOnce() -> T) -> T {
        let _guard = env_lock().lock().expect("env lock");
        let previous = std::env::var(key).ok();
        match value {
            Some(value) => unsafe { std::env::set_var(key, value) },
            None => unsafe { std::env::remove_var(key) },
        }
        let result = f();
        match previous.as_deref() {
            Some(value) => unsafe { std::env::set_var(key, value) },
            None => unsafe { std::env::remove_var(key) },
        }
        result
    }

    #[test]
    fn default_provider_is_local() {
        with_env_var("ANTHROPIC_API_KEY", None, || {
            let cfg = ProviderConfig::default();
            assert_eq!(cfg.default, "local");
            assert!(cfg.providers.contains_key("local"));
        });
    }

    #[test]
    fn init_writes_provider_config() {
        let temp = tempfile::tempdir().expect("tempdir");
        init_provider_config(temp.path()).expect("init");
        assert!(provider_config_path(temp.path()).expect("path").exists());
    }

    #[test]
    fn redacts_common_secret_shapes() {
        let input =
            "token=abc123456789xyz and bearer secretBearerToken12345 and sk-abc123456789xyz";
        let redacted = redact_secrets(input);

        assert!(!redacted.contains("abc123456789xyz"));
        assert!(!redacted.contains("secretBearerToken12345"));
        assert!(redacted.contains("[REDACTED]"));
    }

    #[test]
    fn default_extraction_config_fields() {
        with_env_var("ANTHROPIC_API_KEY", None, || {
            let cfg = ProviderConfig::default();
            assert_eq!(cfg.extraction_provider, "local");
            assert_eq!(cfg.max_candidates_per_batch, 20);
            assert_eq!(cfg.min_confidence, 0.7);
        });
    }

    #[test]
    fn local_provider_refuses_remote_call() {
        with_env_var("ANTHROPIC_API_KEY", None, || {
            let cfg = ProviderConfig::default();
            let result = call_provider(
                &cfg,
                &ProviderRequest {
                    system_prompt: "system".to_string(),
                    user_prompt: "user".to_string(),
                },
                512,
            );
            assert!(result.is_err());
            assert!(result.expect_err("must fail").to_string().contains("local"));
        });
    }

    #[test]
    fn is_llm_extraction_disabled_by_default() {
        with_env_var("ANTHROPIC_API_KEY", None, || {
            let temp = tempfile::tempdir().expect("tempdir");
            let enabled = is_llm_extraction_enabled(temp.path()).expect("check");
            assert!(!enabled);
        });
    }

    #[test]
    fn auto_detect_prefers_anthropic_when_key_present() {
        with_env_var("ANTHROPIC_API_KEY", Some("test-key"), || {
            let cfg = ProviderConfig::auto_detect();
            assert_eq!(cfg.default, "anthropic");
            assert_eq!(cfg.extraction_provider, "anthropic");
            assert!(matches!(
                cfg.providers.get("anthropic"),
                Some(Provider::Anthropic { .. })
            ));
        });
    }

    #[test]
    fn anthropic_request_body_marks_system_prompt_cacheable() {
        let body = anthropic_request_body(
            "claude-sonnet-4-5",
            &ProviderRequest {
                system_prompt: "system prompt".to_string(),
                user_prompt: "user prompt".to_string(),
            },
            512,
            true,
        );

        assert_eq!(body["model"], "claude-sonnet-4-5");
        assert_eq!(body["messages"][0]["content"], "user prompt");
        assert_eq!(body["system"][0]["cache_control"]["type"], "ephemeral");
    }
}
