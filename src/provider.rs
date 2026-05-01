use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::config;
use crate::fsutil;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub default: String,
    pub providers: BTreeMap<String, Provider>,
    pub privacy: PrivacyConfig,
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
}
