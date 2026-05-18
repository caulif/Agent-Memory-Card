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
pub use custom::{save_custom_openai_compatible_provider, save_custom_provider};

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
    "claude-cli".to_string()
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
        #[serde(default = "default_anthropic_base_url")]
        base_url: String,
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
                base_url,
                model,
                api_key_env,
                cache_system_prompt,
                cache_ttl,
            } => f
                .debug_struct("Anthropic")
                .field("base_url", base_url)
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

fn default_anthropic_base_url() -> String {
    "https://api.anthropic.com".to_string()
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
            default: "claude-cli".to_string(),
            extraction_provider: "claude-cli".to_string(),
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
        if let Some((api_key_env, _)) = anthropic_key_env_value() {
            let base_url = std::env::var("ANTHROPIC_BASE_URL")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(default_anthropic_base_url);
            let model = std::env::var("ANTHROPIC_MODEL")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "claude-sonnet-4-5".to_string());
            cfg.providers.insert(
                "anthropic".to_string(),
                Provider::Anthropic {
                    base_url,
                    model,
                    api_key_env,
                    cache_system_prompt: true,
                    cache_ttl: Some("1h".to_string()),
                },
            );
        }
        cfg
    }
}

fn anthropic_key_env_value() -> Option<(String, String)> {
    ["ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_API_KEY"]
        .into_iter()
        .find_map(|key| {
            std::env::var(key)
                .ok()
                .filter(|value| !value.trim().is_empty())
                .map(|value| (key.to_string(), value))
        })
}

fn merge_detected_providers(mut cfg: ProviderConfig) -> ProviderConfig {
    let detected = ProviderConfig::auto_detect();
    for (name, provider) in detected.providers {
        cfg.providers.entry(name).or_insert(provider);
    }
    cfg
}

fn env_override(value: &str, env_name: &str) -> String {
    std::env::var(env_name)
        .ok()
        .filter(|env_value| !env_value.trim().is_empty())
        .unwrap_or_else(|| value.to_string())
}

fn anthropic_api_key(api_key_env: &str) -> Result<String> {
    match std::env::var(api_key_env) {
        Ok(value) if !value.trim().is_empty() => Ok(value),
        _ => anthropic_key_env_value()
            .map(|(_, value)| value)
            .ok_or_else(|| anyhow!("missing Anthropic API key in {api_key_env}")),
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
    let cfg = serde_yaml::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
    Ok(merge_detected_providers(cfg))
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
            base_url,
            model,
            api_key_env,
            cache_system_prompt,
            cache_ttl,
        } => {
            let api_key = anthropic_api_key(api_key_env)?;
            let base_url = env_override(base_url, "ANTHROPIC_BASE_URL");
            let model = env_override(model, "ANTHROPIC_MODEL");
            let body = anthropic_request_body(
                &model,
                request,
                max_tokens,
                *cache_system_prompt,
                cache_ttl.as_deref(),
                anthropic_supports_structured_output(&base_url),
            );
            call_anthropic_api(&base_url, &api_key, &body, cache_ttl.as_deref())
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
    base_url: &str,
    api_key: &str,
    body: &serde_json::Value,
    cache_ttl: Option<&str>,
) -> Result<String> {
    call_anthropic_api_once(base_url, api_key, body, cache_ttl).or_else(|error| {
        if !anthropic_no_text_error(&error) || body.get("output_config").is_none() {
            return Err(error);
        }
        let fallback = anthropic_unstructured_json_body(body);
        call_anthropic_api_once(base_url, api_key, &fallback, cache_ttl).with_context(|| {
            format!(
                "structured Anthropic response had no text content; fallback also failed: {error}"
            )
        })
    })
}

fn call_anthropic_api_once(
    base_url: &str,
    api_key: &str,
    body: &serde_json::Value,
    cache_ttl: Option<&str>,
) -> Result<String> {
    let endpoint = format!("{}/v1/messages", base_url.trim_end_matches('/'));
    let client = http_client()?;
    let mut builder = client
        .post(endpoint)
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

fn anthropic_no_text_error(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause
            .to_string()
            .contains("Anthropic response returned no text content")
    })
}

fn anthropic_unstructured_json_body(body: &serde_json::Value) -> serde_json::Value {
    let mut fallback = body.clone();
    let Some(schema) = fallback
        .pointer("/output_config/format/schema/schema")
        .cloned()
    else {
        return fallback;
    };
    fallback.as_object_mut().map(|object| {
        object.remove("output_config");
        let max_tokens = object
            .get("max_tokens")
            .and_then(|value| value.as_u64())
            .unwrap_or(0)
            .saturating_mul(4)
            .max(4096);
        object.insert("max_tokens".to_string(), serde_json::json!(max_tokens));
    });
    if let Some(messages) = fallback
        .get_mut("messages")
        .and_then(|value| value.as_array_mut())
        && let Some(first_message) = messages.first_mut()
        && let Some(content) = first_message.get_mut("content")
        && let Some(text) = content.as_str()
    {
        let schema_text = serde_json::to_string(&schema).unwrap_or_else(|_| schema.to_string());
        *content = serde_json::Value::String(format!(
            "{text}\n\nReturn only valid JSON matching this JSON Schema. Do not include markdown fences or explanatory text:\n{schema_text}"
        ));
    }
    fallback
}

fn anthropic_supports_structured_output(base_url: &str) -> bool {
    !base_url.to_ascii_lowercase().contains("api.deepseek.com")
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
    supports_structured_output: bool,
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
        if !supports_structured_output {
            if let Some(messages) = body
                .get_mut("messages")
                .and_then(|value| value.as_array_mut())
                && let Some(first_message) = messages.first_mut()
                && let Some(content) = first_message.get_mut("content")
                && let Some(text) = content.as_str()
            {
                let schema_text = serde_json::to_string(&schema.schema)
                    .unwrap_or_else(|_| schema.schema.to_string());
                *content = serde_json::Value::String(format!(
                    "{text}\n\nReturn only valid JSON matching this JSON Schema. Do not include markdown fences or explanatory text:\n{schema_text}"
                ));
            }
            return body;
        }
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
        .timeout(http_timeout_duration())
        .build()
        .context("build HTTP client")
}

fn http_timeout_duration() -> Duration {
    std::env::var("AGENT_KERNEL_HTTP_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|secs| *secs > 0)
        .map(Duration::from_secs)
        .unwrap_or_else(|| Duration::from_secs(120))
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
mod tests;
