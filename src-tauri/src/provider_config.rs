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

#[derive(Debug, Clone, Serialize)]
pub struct SetupChecklistReport {
    pub runtime_mode: String,
    pub items: Vec<SetupChecklistItem>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SetupChecklistItem {
    pub label: String,
    pub status: String,
    pub detail: String,
    pub next_action: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderStatusReport {
    pub status: String,
    pub provider: String,
    pub protocol: String,
    pub base_url: String,
    pub model: String,
    pub api_key_env: String,
    pub proxy: Option<String>,
    pub checks: Vec<SetupChecklistItem>,
    pub next_actions: Vec<String>,
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

pub fn setup_checklist(project_root: &Path) -> anyhow::Result<SetupChecklistReport> {
    let provider_report = provider_status(project_root, false)?;
    let dev_mode = cfg!(debug_assertions);
    let mut items = vec![
        SetupChecklistItem {
            label: "App runtime".to_string(),
            status: if dev_mode { "warn" } else { "pass" }.to_string(),
            detail: if dev_mode {
                "当前是开发模式；运行桌面源码需要 Rust、Bun 和 Tauri toolchain。".to_string()
            } else {
                "当前是安装包/发布模式；普通使用不需要安装 Rust 或 Bun。".to_string()
            },
            next_action: if dev_mode {
                Some("如果只是使用产品，请改用 release installer；如果要开发，确认 cargo 和 bun 可用。".to_string())
            } else {
                None
            },
        },
        command_check_item("Rust / Cargo", "cargo", dev_mode),
        command_check_item("Bun", "bun", dev_mode),
    ];
    items.extend(provider_report.checks);
    Ok(SetupChecklistReport {
        runtime_mode: if dev_mode { "dev" } else { "installer" }.to_string(),
        items,
    })
}

pub fn provider_status(project_root: &Path, live_request: bool) -> anyhow::Result<ProviderStatusReport> {
    let root = fsutil::normalize_project_root(project_root)?;
    let cfg = provider::load_or_default_provider_config(&root)?;
    let provider_name = provider::extraction_provider_name(&cfg).to_string();
    let custom = load_custom_provider_config(&root)?;
    let proxy = proxy_env();
    let mut checks = Vec::new();
    let mut next_actions = Vec::new();

    if custom.enabled {
        checks.push(pass_item(
            "Provider selected",
            "custom-api 已启用为默认 LLM 提炼 Provider。",
        ));
    } else {
        checks.push(warn_item(
            "Provider selected",
            "当前未启用 custom-api；LLM 按钮可能会走 CLI Provider 或失败。",
            "在设置页启用第三方 Provider，或把整理引擎切回本地/CLI。",
        ));
        next_actions.push("启用第三方 Provider，或选择 local/claude-code/codex 引擎。".to_string());
    }

    if custom.base_url.trim().is_empty() {
        checks.push(fail_item(
            "Base URL",
            "Provider Base URL 为空。",
            "填写 OpenAI-compatible 或 Anthropic-compatible Base URL。",
        ));
        next_actions.push("填写 Provider Base URL。".to_string());
    } else {
        checks.push(pass_item("Base URL", &format!("已配置 {}", custom.base_url)));
    }

    if custom.model.trim().is_empty() {
        checks.push(fail_item(
            "Model",
            "Provider 模型名为空。",
            "填写模型名，例如 deepseek-chat 或 qwen2.5-coder:7b。",
        ));
        next_actions.push("填写 Provider 模型名。".to_string());
    } else {
        checks.push(pass_item("Model", &format!("已配置 {}", custom.model)));
    }

    let api_key_present = if custom.api_key_env.trim().is_empty() {
        false
    } else {
        std::env::var(custom.api_key_env.trim())
            .ok()
            .is_some_and(|value| !value.trim().is_empty())
    };
    if api_key_present {
        checks.push(pass_item(
            "API key",
            &format!("环境变量 {} 已存在。", custom.api_key_env),
        ));
    } else if custom.base_url.contains("localhost") || custom.base_url.contains("127.0.0.1") {
        checks.push(warn_item(
            "API key",
            "本地 Provider 未检测到 API key；Ollama 一类本地服务通常可以留空。",
            "如果远程服务要求鉴权，请在 SK 输入框保存一次 key。",
        ));
    } else {
        checks.push(fail_item(
            "API key",
            &format!("未检测到环境变量 {}。", custom.api_key_env),
            "在 SK 输入框粘贴 key 并保存，或在启动应用前设置同名环境变量。",
        ));
        next_actions.push(format!("设置 {}，然后重新测试 Provider。", custom.api_key_env));
    }

    checks.push(match proxy.as_deref() {
        Some(value) => pass_item("Proxy", &format!("检测到代理环境：{value}")),
        None => warn_item(
            "Proxy",
            "未检测到 HTTP_PROXY / HTTPS_PROXY / ALL_PROXY。",
            "如果 Provider 在当前网络不可达，请先设置代理再启动应用。",
        ),
    });

    if live_request && next_actions.is_empty() {
        let request = provider::ProviderRequest {
            system_prompt: "Return only JSON.".to_string(),
            user_prompt: "Return {\"ok\":true}.".to_string(),
            json_schema: None,
        };
        match provider::call_provider(&cfg, &request, 32) {
            Ok(_) => checks.push(pass_item("Live request", "Provider 返回了测试响应。")),
            Err(error) => {
                let action = actionable_provider_error(&error.to_string());
                checks.push(fail_item("Live request", &error.to_string(), &action));
                next_actions.push(action);
            }
        }
    }

    let status = if checks.iter().any(|item| item.status == "fail") {
        "fail"
    } else if checks.iter().any(|item| item.status == "warn") {
        "warn"
    } else {
        "pass"
    };

    Ok(ProviderStatusReport {
        status: status.to_string(),
        provider: provider_name,
        protocol: custom.protocol,
        base_url: custom.base_url,
        model: custom.model,
        api_key_env: custom.api_key_env,
        proxy,
        checks,
        next_actions,
    })
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

fn command_check_item(label: &str, binary: &str, needed: bool) -> SetupChecklistItem {
    let available = std::process::Command::new(binary)
        .arg("--version")
        .output()
        .ok()
        .is_some_and(|output| output.status.success());
    match (needed, available) {
        (true, true) => pass_item(label, &format!("{binary} 可用，适合开发模式。")),
        (true, false) => fail_item(
            label,
            &format!("开发模式需要 {binary}，但当前进程找不到它。"),
            &format!("安装 {binary} 后重启终端/桌面应用。"),
        ),
        (false, true) => pass_item(label, &format!("{binary} 可用，但安装包用户不依赖它。")),
        (false, false) => pass_item(label, &format!("安装包用户不需要 {binary}。")),
    }
}

fn pass_item(label: &str, detail: &str) -> SetupChecklistItem {
    SetupChecklistItem {
        label: label.to_string(),
        status: "pass".to_string(),
        detail: detail.to_string(),
        next_action: None,
    }
}

fn warn_item(label: &str, detail: &str, next_action: &str) -> SetupChecklistItem {
    SetupChecklistItem {
        label: label.to_string(),
        status: "warn".to_string(),
        detail: detail.to_string(),
        next_action: Some(next_action.to_string()),
    }
}

fn fail_item(label: &str, detail: &str, next_action: &str) -> SetupChecklistItem {
    SetupChecklistItem {
        label: label.to_string(),
        status: "fail".to_string(),
        detail: detail.to_string(),
        next_action: Some(next_action.to_string()),
    }
}

fn proxy_env() -> Option<String> {
    ["HTTPS_PROXY", "HTTP_PROXY", "ALL_PROXY"]
        .into_iter()
        .find_map(|key| {
            std::env::var(key)
                .ok()
                .filter(|value| !value.trim().is_empty())
                .map(|value| format!("{key}={value}"))
        })
}

fn actionable_provider_error(error: &str) -> String {
    let lower = error.to_ascii_lowercase();
    if lower.contains("401") || lower.contains("unauthorized") {
        "API key 被拒绝；检查 key 是否属于当前 Provider，保存后重试。".to_string()
    } else if lower.contains("403") || lower.contains("forbidden") {
        "Provider 拒绝访问；检查账号权限、模型权限或区域限制。".to_string()
    } else if lower.contains("timeout") || lower.contains("timed out") {
        "Provider 请求超时；检查代理、Base URL 和 AGENT_KERNEL_HTTP_TIMEOUT_SECS。".to_string()
    } else if lower.contains("dns") || lower.contains("connect") || lower.contains("connection") {
        "无法连接 Provider；检查 Base URL、代理环境和本机网络。".to_string()
    } else if lower.contains("no message content") || lower.contains("no text content") {
        "Provider 返回格式不符合预期；尝试切换协议或模型。".to_string()
    } else {
        "Provider 测试失败；检查 Base URL、协议、模型名、API key 和代理后重试。".to_string()
    }
}
