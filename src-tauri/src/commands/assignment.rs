use std::path::Path;

use agent_kernel::{kernel, memory_card};

use crate::app_service;
use crate::commands::project::{enforce_tauri_kernel_policy_for_project, project_ack};
use crate::{CommandResult, ProjectMutationAck, error_to_string};

#[tauri::command]
pub fn set_agent_enabled(
    project_path: String,
    agent: String,
    enabled: bool,
    confirmed_policy: Option<kernel::KernelPolicy>,
    decision_token: Option<String>,
) -> CommandResult<ProjectMutationAck> {
    enforce_tauri_kernel_policy_for_project(
        &project_path,
        kernel::KernelCommand::SetAgentEnabled {
            agent: agent.clone(),
            enabled,
        },
        serde_json::json!({ "agent": agent.clone(), "enabled": enabled }),
        confirmed_policy,
        decision_token,
    )?;
    app_service::set_agent_enabled(Path::new(&project_path), &agent, enabled)
        .map_err(error_to_string)?;
    project_ack(&project_path)
}

#[tauri::command]
pub fn set_memory_card_targets(
    project_path: String,
    id: String,
    targets: Vec<String>,
    confirmed_policy: Option<kernel::KernelPolicy>,
    decision_token: Option<String>,
) -> CommandResult<ProjectMutationAck> {
    enforce_tauri_kernel_policy_for_project(
        &project_path,
        kernel::KernelCommand::AssignMemoryCard {
            id: id.clone(),
            targets: targets.clone(),
        },
        serde_json::json!({ "id": id.clone(), "targets": targets.clone() }),
        confirmed_policy,
        decision_token,
    )?;
    app_service::set_memory_card_targets(Path::new(&project_path), &id, targets)
        .map_err(error_to_string)?;
    project_ack(&project_path)
}

#[tauri::command]
pub fn clear_memory_card_targets(
    project_path: String,
    agent: Option<String>,
    confirmed_policy: Option<kernel::KernelPolicy>,
    decision_token: Option<String>,
) -> CommandResult<ProjectMutationAck> {
    enforce_tauri_kernel_policy_for_project(
        &project_path,
        kernel::KernelCommand::ClearMemoryCardTargets {
            agent: agent.clone(),
        },
        serde_json::json!({ "agent": agent.clone() }),
        confirmed_policy,
        decision_token,
    )?;
    memory_card::clear_memory_card_targets(Path::new(&project_path), agent)
        .map_err(error_to_string)?;
    project_ack(&project_path)
}
