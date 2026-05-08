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

mod custom;
pub use custom::save_custom_openai_compatible_provider;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub default: String,
    #[serde(default = "default_extraction_provider")]
    pub extraction_provider: String,
    #[serde(default)]
    pub role_providers: ProviderRoleConfig,
    #[serde(default)]
    pub fallback_methodology_templates: bool,
    #[serde(default = "default_max_candidates_per_batch")]
    pub max_candidates_per_batch: usize,
    #[serde(default = "default_min_confidence")]
    pub min_confidence: f32,
    #[serde(default)]
    pub judge: JudgeConfig,
    pub providers: BTreeMap<String, Provider>,
    pub privacy: PrivacyConfig,
}

#[derive(Debug, Clone)]
pub struct ProviderRequest {
    pub system_prompt: String,
    pub user_prompt: String,
    pub json_schema: Option<ProviderJsonSchema>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderJsonSchema {
    pub name: String,
    pub schema: serde_json::Value,
    #[serde(default)]
    pub strict: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderRoleConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extract: Option<String>,
    #[serde(default, rename = "abstract", skip_serializing_if = "Option::is_none")]
    pub abstract_: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub judge: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refine: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgeConfig {
    #[serde(default = "default_noise_risk_reject")]
    pub noise_risk_reject: f32,
    #[serde(default = "default_evidence_grounded_min_non_project")]
    pub evidence_grounded_min_non_project: f32,
    #[serde(default = "default_ambiguous_noise_low")]
    pub ambiguous_noise_low: f32,
    #[serde(default = "default_ambiguous_noise_high")]
    pub ambiguous_noise_high: f32,
    #[serde(default = "default_ambiguous_grounding_low")]
    pub ambiguous_grounding_low: f32,
    #[serde(default = "default_ambiguous_grounding_high")]
    pub ambiguous_grounding_high: f32,
}

impl Default for JudgeConfig {
    fn default() -> Self {
        Self {
            noise_risk_reject: default_noise_risk_reject(),
            evidence_grounded_min_non_project: default_evidence_grounded_min_non_project(),
            ambiguous_noise_low: default_ambiguous_noise_low(),
            ambiguous_noise_high: default_ambiguous_noise_high(),
            ambiguous_grounding_low: default_ambiguous_grounding_low(),
            ambiguous_grounding_high: default_ambiguous_grounding_high(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderRole {
    Extract,
    Abstract,
    Judge,
    Refine,
    Update,
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

fn default_noise_risk_reject() -> f32 {
    0.35
}

fn default_evidence_grounded_min_non_project() -> f32 {
    0.70
}

fn default_ambiguous_noise_low() -> f32 {
    0.25
}

fn default_ambiguous_noise_high() -> f32 {
    0.45
}

fn default_ambiguous_grounding_low() -> f32 {
    0.55
}

fn default_ambiguous_grounding_high() -> f32 {
    0.75
}

#[derive(Clone, Serialize, Deserialize)]
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cache_ttl: Option<String>,
    },
    ClaudeCli {
        binary: String,
        #[serde(default)]
        extra_args: Vec<String>,
        #[serde(default = "default_cli_timeout_secs")]
        timeout_secs: u64,
    },
    CodexCli {
        binary: String,
        #[serde(default)]
        extra_args: Vec<String>,
        #[serde(default = "default_cli_timeout_secs")]
        timeout_secs: u64,
    },
}

impl std::fmt::Debug for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LocalHeuristic => f.debug_struct("LocalHeuristic").finish(),
            Self::OpenAiCompatible {
                base_url,
                model,
                api_key_env,
            } => f
                .debug_struct("OpenAiCompatible")
                .field("base_url", base_url)
                .field("model", model)
                .field("api_key_env", &KeyEnvRedacted(api_key_env))
                .finish(),
            Self::Anthropic {
                model,
                api_key_env,
                cache_system_prompt,
                cache_ttl,
            } => f
                .debug_struct("Anthropic")
                .field("model", model)
                .field("api_key_env", &KeyEnvRedacted(api_key_env))
                .field("cache_system_prompt", cache_system_prompt)
                .field("cache_ttl", cache_ttl)
                .finish(),
            Self::ClaudeCli {
                binary,
                extra_args,
                timeout_secs,
            } => f
                .debug_struct("ClaudeCli")
                .field("binary", binary)
                .field("extra_args", extra_args)
                .field("timeout_secs", timeout_secs)
                .finish(),
            Self::CodexCli {
                binary,
                extra_args,
                timeout_secs,
            } => f
                .debug_struct("CodexCli")
                .field("binary", binary)
                .field("extra_args", extra_args)
                .field("timeout_secs", timeout_secs)
                .finish(),
        }
    }
}

/// 在 Debug 输出中安全地表示环境变量名，不暴露值。
struct KeyEnvRedacted<'a>(&'a str);

impl std::fmt::Debug for KeyEnvRedacted<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0.is_empty() {
            write!(f, "<unset>")
        } else {
            write!(f, "<redacted>")
        }
    }
}

fn default_cache_system_prompt() -> bool {
    true
}

fn default_cli_timeout_secs() -> u64 {
    120
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
        providers.insert(
            "claude-cli".to_string(),
            Provider::ClaudeCli {
                binary: "claude".to_string(),
                extra_args: vec![
                    "--print".to_string(),
                    "--output-format".to_string(),
                    "text".to_string(),
                    "--dangerously-skip-permissions".to_string(),
                ],
                timeout_secs: default_cli_timeout_secs(),
            },
        );
        providers.insert(
            "codex-cli".to_string(),
            Provider::CodexCli {
                binary: "codex".to_string(),
                extra_args: vec![
                    "exec".to_string(),
                    "--dangerously-bypass-approvals-and-sandbox".to_string(),
                ],
                timeout_secs: default_cli_timeout_secs(),
            },
        );
        Self {
            default: "local".to_string(),
            extraction_provider: "local".to_string(),
            role_providers: ProviderRoleConfig::default(),
            fallback_methodology_templates: false,
            max_candidates_per_batch: 20,
            min_confidence: 0.7,
            judge: JudgeConfig::default(),
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
                    cache_ttl: Some("1h".to_string()),
                },
            );
            cfg.default = "anthropic".to_string();
            cfg.extraction_provider = "anthropic".to_string();
            cfg.role_providers = ProviderRoleConfig {
                extract: Some("anthropic".to_string()),
                abstract_: Some("anthropic".to_string()),
                judge: Some("anthropic".to_string()),
                refine: Some("anthropic".to_string()),
                update: Some("anthropic".to_string()),
            };
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
        if let Ok(re) = Regex::new(pattern) {
            redacted = re.replace_all(&redacted, "[REDACTED]").to_string();
        }
    }
    redacted
}

/// 调用远程 LLM 提供商进行文本提取。
pub fn call_provider(
    cfg: &ProviderConfig,
    request: &ProviderRequest,
    max_tokens: usize,
) -> Result<String> {
    call_provider_for_role(cfg, ProviderRole::Extract, request, max_tokens)
}

pub fn call_provider_for_role(
    cfg: &ProviderConfig,
    role: ProviderRole,
    request: &ProviderRequest,
    max_tokens: usize,
) -> Result<String> {
    let provider_name = provider_name_for_role(cfg, role);

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
            let mut body = serde_json::json!({
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
            if let Some(schema) = request.json_schema.as_ref() {
                body["response_format"] = serde_json::json!({
                    "type": "json_schema",
                    "json_schema": {
                        "name": schema.name,
                        "strict": schema.strict,
                        "schema": schema.schema
                    }
                });
            }

            call_openai_compatible_api(base_url, &api_key, &body)
        }
        Provider::Anthropic {
            model,
            api_key_env,
            cache_system_prompt,
            cache_ttl,
        } => {
            let api_key = std::env::var(api_key_env)
                .with_context(|| format!("missing Anthropic API key in {api_key_env}"))?;
            let body = anthropic_request_body(
                model,
                request,
                max_tokens,
                *cache_system_prompt,
                cache_ttl.as_deref(),
            );
            call_anthropic_api(&api_key, &body, cache_ttl.as_deref())
        }
        Provider::ClaudeCli {
            binary,
            extra_args,
            timeout_secs,
        }
        | Provider::CodexCli {
            binary,
            extra_args,
            timeout_secs,
        } => call_cli_subprocess(binary, extra_args, request, *timeout_secs),
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

fn call_anthropic_api(
    api_key: &str,
    body: &serde_json::Value,
    cache_ttl: Option<&str>,
) -> Result<String> {
    let client = http_client()?;
    let mut builder = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01");
    if let Some(beta) = anthropic_beta_header(cache_ttl, body.get("output_config").is_some()) {
        builder = builder.header("anthropic-beta", beta);
    }
    let json = send_json(builder.json(body))?;

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

fn anthropic_beta_header(cache_ttl: Option<&str>, structured_outputs: bool) -> Option<String> {
    let mut betas = Vec::new();
    if cache_ttl == Some("1h") {
        betas.push("extended-cache-ttl-2025-04-11");
    }
    if structured_outputs {
        betas.push("structured-outputs-2025-11-13");
    }
    if betas.is_empty() {
        None
    } else {
        Some(betas.join(","))
    }
}

fn anthropic_request_body(
    model: &str,
    request: &ProviderRequest,
    max_tokens: usize,
    cache_system_prompt: bool,
    cache_ttl: Option<&str>,
) -> serde_json::Value {
    let system = if cache_system_prompt {
        let cache_control = match cache_ttl {
            Some(ttl) => serde_json::json!({ "type": "ephemeral", "ttl": ttl }),
            None => serde_json::json!({ "type": "ephemeral" }),
        };
        serde_json::json!([{
            "type": "text",
            "text": request.system_prompt,
            "cache_control": cache_control
        }])
    } else {
        serde_json::json!(request.system_prompt)
    };

    let mut body = serde_json::json!({
        "model": model,
        "max_tokens": max_tokens,
        "system": system,
        "messages": [{
            "role": "user",
            "content": request.user_prompt
        }]
    });
    if let Some(schema) = request.json_schema.as_ref() {
        body["output_config"] = serde_json::json!({
            "format": {
                "type": "json_schema",
                "schema": {
                    "name": schema.name,
                    "strict": schema.strict,
                    "schema": schema.schema
                }
            }
        });
    }
    body
}

fn call_cli_subprocess(
    binary: &str,
    extra_args: &[String],
    request: &ProviderRequest,
    timeout_secs: u64,
) -> Result<String> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    use std::thread;
    use std::time::{Duration, Instant};

    let mut command = Command::new(binary);
    command.args(extra_args);

    let mut prompt = request.system_prompt.clone();
    prompt.push_str("\n\n---\n\n");
    prompt.push_str(&request.user_prompt);

    if let Some(schema) = request.json_schema.as_ref()
        && binary.eq_ignore_ascii_case("claude")
        && !extra_args.iter().any(|arg| arg == "--json-schema")
    {
        command.arg("--json-schema");
        command.arg(serde_json::to_string(&schema.schema)?);
        if !extra_args.iter().any(|arg| arg == "--output-format") {
            command.arg("--output-format");
            command.arg("json");
        }
    }

    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .with_context(|| format!("spawn CLI provider `{binary}`"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(prompt.as_bytes())
            .with_context(|| format!("write prompt to `{binary}` stdin"))?;
    }

    let deadline = Instant::now() + Duration::from_secs(timeout_secs.max(1));
    loop {
        if let Some(status) = child.try_wait().context("poll CLI provider")? {
            let output = child
                .wait_with_output()
                .context("collect CLI provider output")?;
            if !status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow!("CLI provider `{binary}` failed: {stderr}"));
            }
            let stdout = String::from_utf8(output.stdout).context("decode CLI stdout")?;
            return Ok(stdout.trim().to_string());
        }
        if Instant::now() >= deadline {
            let kill_error = child.kill().err();
            let wait_error = child.wait().err();
            return Err(anyhow!(
                "CLI provider `{binary}` timed out after {timeout_secs}s; kill_error={kill_error:?}; wait_error={wait_error:?}"
            ));
        }
        thread::sleep(Duration::from_millis(25));
    }
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
    provider_name_for_role(cfg, ProviderRole::Extract)
}

/// 检查是否使用了 LLM 提取引擎
pub fn is_llm_extraction_enabled(project_root: &Path) -> Result<bool> {
    let cfg = load_or_default_provider_config(project_root)?;
    Ok(extraction_provider_name(&cfg) != "local")
}

fn provider_name_for_role(cfg: &ProviderConfig, role: ProviderRole) -> &str {
    let role_name = match role {
        ProviderRole::Extract => cfg.role_providers.extract.as_deref(),
        ProviderRole::Abstract => cfg
            .role_providers
            .abstract_
            .as_deref()
            .or(cfg.role_providers.extract.as_deref()),
        ProviderRole::Judge => cfg.role_providers.judge.as_deref(),
        ProviderRole::Refine => cfg
            .role_providers
            .refine
            .as_deref()
            .or(cfg.role_providers.judge.as_deref()),
        ProviderRole::Update => cfg.role_providers.update.as_deref(),
    };
    role_name.unwrap_or(&cfg.extraction_provider)
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
            assert_eq!(
                extraction_provider_name(&cfg),
                "local",
                "extract role should fall back to legacy extraction_provider"
            );
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
                    json_schema: None,
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
            assert_eq!(cfg.role_providers.update.as_deref(), Some("anthropic"));
            assert!(matches!(
                cfg.providers.get("anthropic"),
                Some(Provider::Anthropic { .. })
            ));
        });
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
        );

        assert_eq!(body["output_config"]["format"]["type"], "json_schema");
        assert_eq!(
            body["output_config"]["format"]["schema"]["name"],
            "AtomicFactExtraction"
        );
        assert_eq!(body["output_config"]["format"]["schema"]["strict"], true);
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
        let script_path = script_dir.path().join("echo-provider.ps1");
        std::fs::write(&script_path, "@($input) -join \"`n\" | Write-Output").expect("script");

        let output = call_cli_subprocess(
            "powershell",
            &[
                "-NoProfile".to_string(),
                "-File".to_string(),
                script_path.display().to_string(),
            ],
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
}
