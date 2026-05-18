use std::path::Path;

use agent_kernel::{fsutil, provider};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomProviderConfig {
    pub enabled: bool,
    #[serde(default = "default_provider_protocol")]
    pub protocol: String,
    pub base_url: String,
    pub model: String,
    pub api_key_env: String,
    #[serde(default, skip_serializing)]
    pub api_key: Option<String>,
}

fn default_provider_protocol() -> String {
    "openai-compatible".to_string()
}

pub fn load_custom_provider_config(project_root: &Path) -> anyhow::Result<CustomProviderConfig> {
    let root = fsutil::normalize_project_root(project_root)?;
    let cfg = provider::load_or_default_provider_config(&root)?;
    let (protocol, base_url, model, api_key_env) = match cfg.providers.get("custom-api") {
        Some(provider::Provider::OpenAiCompatible {
            base_url,
            model,
            api_key_env,
        }) => (
            "openai-compatible".to_string(),
            base_url.clone(),
            model.clone(),
            api_key_env.clone(),
        ),
        Some(provider::Provider::Anthropic {
            base_url,
            model,
            api_key_env,
            ..
        }) => (
            "anthropic-compatible".to_string(),
            base_url.clone(),
            model.clone(),
            api_key_env.clone(),
        ),
        _ => (
            "openai-compatible".to_string(),
            "http://localhost:11434/v1".to_string(),
            "qwen2.5-coder:7b".to_string(),
            "OPENAI_API_KEY".to_string(),
        ),
    };

    Ok(CustomProviderConfig {
        enabled: provider::extraction_provider_name(&cfg) == "custom-api",
        protocol,
        base_url,
        model,
        api_key_env,
        api_key: None,
    })
}

pub fn save_custom_provider_config(
    project_root: &Path,
    input: CustomProviderConfig,
) -> anyhow::Result<CustomProviderConfig> {
    let root = fsutil::normalize_project_root(project_root)?;
    if let Some(api_key) = input.api_key.as_deref().map(str::trim).filter(|key| !key.is_empty()) {
        unsafe { std::env::set_var(input.api_key_env.trim(), api_key) };
    }
    let cfg = provider::save_custom_provider(
        &root,
        &input.protocol,
        &input.base_url,
        &input.model,
        &input.api_key_env,
        input.enabled,
    )?;
    load_custom_provider_config_from_cfg(&cfg)
}

fn load_custom_provider_config_from_cfg(
    cfg: &provider::ProviderConfig,
) -> anyhow::Result<CustomProviderConfig> {
    let (protocol, base_url, model, api_key_env) = match cfg.providers.get("custom-api") {
        Some(provider::Provider::OpenAiCompatible {
            base_url,
            model,
            api_key_env,
        }) => (
            "openai-compatible".to_string(),
            base_url.clone(),
            model.clone(),
            api_key_env.clone(),
        ),
        Some(provider::Provider::Anthropic {
            base_url,
            model,
            api_key_env,
            ..
        }) => (
            "anthropic-compatible".to_string(),
            base_url.clone(),
            model.clone(),
            api_key_env.clone(),
        ),
        _ => anyhow::bail!("custom-api provider was not saved"),
    };

    Ok(CustomProviderConfig {
        enabled: provider::extraction_provider_name(cfg) == "custom-api",
        protocol,
        base_url,
        model,
        api_key_env,
        api_key: None,
    })
}
