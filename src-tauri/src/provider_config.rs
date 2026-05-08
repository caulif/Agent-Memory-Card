use std::path::Path;

use agent_kernel::{fsutil, provider};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomProviderConfig {
    pub enabled: bool,
    pub base_url: String,
    pub model: String,
    pub api_key_env: String,
}

pub fn load_custom_provider_config(project_root: &Path) -> anyhow::Result<CustomProviderConfig> {
    let root = fsutil::normalize_project_root(project_root)?;
    let cfg = provider::load_or_default_provider_config(&root)?;
    let (base_url, model, api_key_env) = match cfg.providers.get("custom-api") {
        Some(provider::Provider::OpenAiCompatible {
            base_url,
            model,
            api_key_env,
        }) => (base_url.clone(), model.clone(), api_key_env.clone()),
        _ => (
            "http://localhost:11434/v1".to_string(),
            "qwen2.5-coder:7b".to_string(),
            "OPENAI_API_KEY".to_string(),
        ),
    };

    Ok(CustomProviderConfig {
        enabled: provider::extraction_provider_name(&cfg) == "custom-api",
        base_url,
        model,
        api_key_env,
    })
}

pub fn save_custom_provider_config(
    project_root: &Path,
    input: CustomProviderConfig,
) -> anyhow::Result<CustomProviderConfig> {
    let root = fsutil::normalize_project_root(project_root)?;
    let cfg = provider::save_custom_openai_compatible_provider(
        &root,
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
    let Some(provider::Provider::OpenAiCompatible {
        base_url,
        model,
        api_key_env,
    }) = cfg.providers.get("custom-api") else {
        anyhow::bail!("custom-api provider was not saved");
    };

    Ok(CustomProviderConfig {
        enabled: provider::extraction_provider_name(cfg) == "custom-api",
        base_url: base_url.clone(),
        model: model.clone(),
        api_key_env: api_key_env.clone(),
    })
}
