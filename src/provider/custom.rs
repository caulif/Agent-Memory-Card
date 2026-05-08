use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow};

use crate::config;
use crate::fsutil;

use super::{
    Provider, ProviderConfig, ProviderRoleConfig, load_or_default_provider_config,
    provider_config_path,
};

pub fn save_custom_openai_compatible_provider(
    project_root: &Path,
    base_url: &str,
    model: &str,
    api_key_env: &str,
    enabled: bool,
) -> Result<ProviderConfig> {
    let normalized_base_url = base_url.trim();
    let normalized_model = model.trim();
    let normalized_api_key_env = api_key_env.trim();
    if normalized_base_url.is_empty() {
        return Err(anyhow!("custom API base URL must not be empty"));
    }
    if normalized_model.is_empty() {
        return Err(anyhow!("custom API model must not be empty"));
    }

    let mut cfg = load_or_default_provider_config(project_root)?;
    cfg.providers.insert(
        "custom-api".to_string(),
        Provider::OpenAiCompatible {
            base_url: normalized_base_url.to_string(),
            model: normalized_model.to_string(),
            api_key_env: normalized_api_key_env.to_string(),
        },
    );

    if enabled {
        cfg.default = "custom-api".to_string();
        cfg.extraction_provider = "custom-api".to_string();
        cfg.role_providers.extract = Some("custom-api".to_string());
        cfg.role_providers.abstract_ = Some("custom-api".to_string());
        cfg.role_providers.judge = Some("custom-api".to_string());
        cfg.role_providers.refine = Some("custom-api".to_string());
        cfg.role_providers.update = Some("custom-api".to_string());
    } else if cfg.extraction_provider == "custom-api" {
        cfg.default = "local".to_string();
        cfg.extraction_provider = "local".to_string();
        cfg.role_providers = ProviderRoleConfig {
            extract: clear_custom_role(cfg.role_providers.extract),
            abstract_: clear_custom_role(cfg.role_providers.abstract_),
            judge: clear_custom_role(cfg.role_providers.judge),
            refine: clear_custom_role(cfg.role_providers.refine),
            update: clear_custom_role(cfg.role_providers.update),
        };
    }

    save_provider_config(project_root, &cfg)?;
    Ok(cfg)
}

fn save_provider_config(project_root: &Path, cfg: &ProviderConfig) -> Result<()> {
    let root = fsutil::normalize_project_root(project_root)?;
    config::ensure_kernel_dir(&root)?;
    fs::write(provider_config_path(&root)?, serde_yaml::to_string(cfg)?)
        .context("write providers.yml")
}

fn clear_custom_role(value: Option<String>) -> Option<String> {
    value.filter(|provider| provider != "custom-api")
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
    fn save_custom_provider_can_enable_and_disable_llm_extraction() {
        with_env_var("ANTHROPIC_API_KEY", None, || {
            let temp = tempfile::tempdir().expect("tempdir");
            let enabled_cfg = save_custom_openai_compatible_provider(
                temp.path(),
                "http://localhost:11434/v1",
                "qwen2.5-coder:7b",
                "OPENAI_API_KEY",
                true,
            )
            .expect("save enabled custom provider");

            assert_eq!(enabled_cfg.extraction_provider, "custom-api");
            assert_eq!(
                enabled_cfg.role_providers.extract.as_deref(),
                Some("custom-api")
            );
            assert!(matches!(
                enabled_cfg.providers.get("custom-api"),
                Some(Provider::OpenAiCompatible { base_url, model, api_key_env })
                    if base_url == "http://localhost:11434/v1"
                        && model == "qwen2.5-coder:7b"
                        && api_key_env == "OPENAI_API_KEY"
            ));
            assert!(super::super::is_llm_extraction_enabled(temp.path()).expect("llm enabled"));

            let disabled_cfg = save_custom_openai_compatible_provider(
                temp.path(),
                "http://localhost:11434/v1",
                "qwen2.5-coder:7b",
                "OPENAI_API_KEY",
                false,
            )
            .expect("save disabled custom provider");

            assert_eq!(disabled_cfg.extraction_provider, "local");
            assert_eq!(disabled_cfg.role_providers.extract, None);
            assert!(!super::super::is_llm_extraction_enabled(temp.path()).expect("llm disabled"));
            assert!(disabled_cfg.providers.contains_key("custom-api"));
        });
    }
}
