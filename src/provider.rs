use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use regex::Regex;
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
}

pub fn provider_config_path(project_root: &Path) -> Result<std::path::PathBuf> {
    let root = fsutil::normalize_project_root(project_root)?;
    Ok(config::kernel_dir(&root).join("providers.yml"))
}

pub fn init_provider_config(project_root: &Path) -> Result<ProviderConfig> {
    let root = fsutil::normalize_project_root(project_root)?;
    config::ensure_kernel_dir(&root)?;
    let cfg = ProviderConfig::default();
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
        return Ok(ProviderConfig::default());
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

/// 调用 OpenAI 兼容 API 进行文本提取。
/// 支持本地 Ollama (HTTP) 和 OpenAI 兼容 API (HTTPS)。
/// 超时: 30s。
pub fn call_provider(cfg: &ProviderConfig, prompt: &str, max_tokens: usize) -> Result<String> {
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
                        "content": "You are a knowledge extraction assistant. Always respond with valid JSON arrays."
                    },
                    {
                        "role": "user",
                        "content": prompt
                    }
                ],
                "temperature": 0.3,
                "max_tokens": max_tokens,
                "response_format": {"type": "json_object"},
                "stream": false
            });

            call_openai_compatible_api(base_url, model, &api_key, &body.to_string(), max_tokens)
        }
    }
}

/// 通过 TcpStream 调用 OpenAI 兼容 API（HTTP 方式）。
/// 适用场景：本地 Ollama 等 HTTP（非 HTTPS）后端。
fn call_openai_compatible_api(
    base_url: &str,
    model: &str,
    api_key: &str,
    body_json: &str,
    _max_tokens: usize,
) -> Result<String> {
    let url = format!("{base_url}/chat/completions");

    // 解析 URL 获取 host 和 path
    let (host, port, path) = parse_http_url(&url)?;

    let addr = format!("{host}:{port}");
    let request = format!(
        "POST {path} HTTP/1.1\r\n\
         Host: {host}\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         {}\r\n\
         \r\n\
         {body_json}",
        body_json.len(),
        if api_key.is_empty() {
            String::new()
        } else {
            format!("Authorization: Bearer {api_key}\r\n")
        },
    );

    let mut stream = TcpStream::connect_timeout(
        &addr
            .parse()
            .map_err(|e| anyhow!("invalid address {addr}: {e}"))?,
        Duration::from_secs(30),
    )
    .map_err(|e| anyhow!("connect to {addr} failed: {e}"))?;

    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .context("set read timeout")?;
    stream
        .set_write_timeout(Some(Duration::from_secs(30)))
        .context("set write timeout")?;

    stream
        .write_all(request.as_bytes())
        .context("write request")?;

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .context("read response")?;

    // 解析 HTTP 响应：跳过头部，找到 JSON body
    if let Some(body_start) = response.find("\r\n\r\n") {
        let body = &response[body_start + 4..];
        let body = body.trim();

        // 尝试解析 chat completions 响应格式
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(body) {
            // 检查错误
            if let Some(error) = value.get("error") {
                let message = error
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("unknown error");
                return Err(anyhow!("API error: {message}"));
            }

            // 提取 choices[0].message.content
            if let Some(content) = value
                .get("choices")
                .and_then(|c| c.get(0))
                .and_then(|c| c.get("message"))
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_str())
            {
                return Ok(content.to_string());
            }
        }

        return Err(anyhow!(
            "unexpected API response: {}",
            body.chars().take(300).collect::<String>()
        ));
    }

    Err(anyhow!(
        "invalid HTTP response from {model}: {}",
        response.chars().take(200).collect::<String>()
    ))
}

/// 解析 HTTP URL 为 (host, port, path)
fn parse_http_url(url: &str) -> Result<(String, u16, String)> {
    let url = url.trim();

    // 去掉 http:// 前缀
    let without_scheme = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .ok_or_else(|| anyhow!("unsupported URL scheme in {url}"))?;

    let (host_port, path) = if let Some(slash_pos) = without_scheme.find('/') {
        let (hp, p) = without_scheme.split_at(slash_pos);
        (hp, p.to_string())
    } else {
        (without_scheme, "/".to_string())
    };

    let (host, port) = if let Some(colon_pos) = host_port.rfind(':') {
        let (h, p) = host_port.split_at(colon_pos);
        let port = p[1..]
            .parse::<u16>()
            .map_err(|_| anyhow!("invalid port in {url}"))?;
        (h.to_string(), port)
    } else {
        (host_port.to_string(), 80u16)
    };

    Ok((host, port, path))
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

    #[test]
    fn default_provider_is_local() {
        let cfg = ProviderConfig::default();
        assert_eq!(cfg.default, "local");
        assert!(cfg.providers.contains_key("local"));
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
        let cfg = ProviderConfig::default();
        assert_eq!(cfg.extraction_provider, "local");
        assert_eq!(cfg.max_candidates_per_batch, 20);
        assert_eq!(cfg.min_confidence, 0.7);
    }

    #[test]
    fn parse_url_with_port_and_path() {
        let (host, port, path) = parse_http_url("http://localhost:11434/v1").expect("parse URL");
        assert_eq!(host, "localhost");
        assert_eq!(port, 11434);
        assert_eq!(path, "/v1");
    }

    #[test]
    fn parse_url_default_port_80() {
        let (host, port, path) = parse_http_url("http://localhost/api").expect("parse URL");
        assert_eq!(host, "localhost");
        assert_eq!(port, 80);
        assert_eq!(path, "/api");
    }

    #[test]
    fn local_provider_refuses_remote_call() {
        let cfg = ProviderConfig::default();
        let result = call_provider(&cfg, "test prompt", 512);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("local"));
    }

    #[test]
    fn is_llm_extraction_disabled_by_default() {
        let temp = tempfile::tempdir().expect("tempdir");
        let enabled = is_llm_extraction_enabled(temp.path()).expect("check");
        assert!(!enabled);
    }
}
