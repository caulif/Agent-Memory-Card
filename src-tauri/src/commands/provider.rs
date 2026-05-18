use std::path::Path;

use agent_kernel::kernel;

use crate::commands::project::enforce_tauri_kernel_policy_for_project;
use crate::provider_config;
use crate::{CommandResult, error_to_string};

#[tauri::command]
pub fn get_custom_provider_config(
    project_path: String,
) -> CommandResult<provider_config::CustomProviderConfig> {
    provider_config::load_custom_provider_config(Path::new(&project_path)).map_err(error_to_string)
}

#[tauri::command]
pub fn save_custom_provider_config(
    project_path: String,
    input: provider_config::CustomProviderConfig,
    confirmed_policy: Option<kernel::KernelPolicy>,
    decision_token: Option<String>,
) -> CommandResult<provider_config::CustomProviderConfig> {
    enforce_tauri_kernel_policy_for_project(
        &project_path,
        kernel::KernelCommand::ConfigureProvider,
        serde_json::json!({ "input": input.clone() }),
        confirmed_policy,
        decision_token,
    )?;
    provider_config::save_custom_provider_config(Path::new(&project_path), input)
        .map_err(error_to_string)
}

#[tauri::command]
pub fn get_setup_checklist(
    project_path: String,
) -> CommandResult<provider_config::SetupChecklistReport> {
    provider_config::setup_checklist(Path::new(&project_path)).map_err(error_to_string)
}

#[tauri::command]
pub fn test_provider_status(
    project_path: String,
    live_request: bool,
) -> CommandResult<provider_config::ProviderStatusReport> {
    provider_config::provider_status(Path::new(&project_path), live_request)
        .map_err(error_to_string)
}
