use std::path::{Path, PathBuf};

use agent_kernel::{kernel, memory_card};

use crate::app_service;
use crate::commands::project::{enforce_tauri_kernel_policy_for_project, project_ack};
use crate::jobs::{DesktopJobStart, DesktopTaskStore};
use crate::{
    CommandResult, MemoryCardMergeInput, MemoryCardUpdateInput, ProjectMutationAck,
    default_home_dir, error_to_string,
};

#[tauri::command]
pub fn update_memory_card(
    project_path: String,
    id: String,
    input: MemoryCardUpdateInput,
    confirmed_policy: Option<kernel::KernelPolicy>,
    decision_token: Option<String>,
) -> CommandResult<memory_card::MemoryCardRecord> {
    let payload = serde_json::json!({ "id": id.clone(), "input": input.clone() });
    enforce_tauri_kernel_policy_for_project(
        &project_path,
        kernel::KernelCommand::UpdateMemoryCard { id: id.clone() },
        payload,
        confirmed_policy,
        decision_token,
    )?;
    app_service::update_memory_card(Path::new(&project_path), &id, input).map_err(error_to_string)
}

#[tauri::command]
pub fn delete_memory_card(
    project_path: String,
    id: String,
    confirmed_policy: Option<kernel::KernelPolicy>,
    decision_token: Option<String>,
) -> CommandResult<ProjectMutationAck> {
    enforce_tauri_kernel_policy_for_project(
        &project_path,
        kernel::KernelCommand::DeleteMemoryCard { id: id.clone() },
        serde_json::json!({ "id": id.clone() }),
        confirmed_policy,
        decision_token,
    )?;
    memory_card::delete_memory_card(Path::new(&project_path), &id).map_err(error_to_string)?;
    project_ack(&project_path)
}

#[tauri::command]
pub fn promote_memory_card_to_global(
    project_path: String,
    home: Option<String>,
    id: String,
    confirmed_policy: Option<kernel::KernelPolicy>,
    decision_token: Option<String>,
) -> CommandResult<ProjectMutationAck> {
    enforce_tauri_kernel_policy_for_project(
        &project_path,
        kernel::KernelCommand::PromoteMemoryCardToGlobal { id: id.clone() },
        serde_json::json!({ "id": id.clone() }),
        confirmed_policy,
        decision_token,
    )?;
    let home = home.map(PathBuf::from).unwrap_or_else(default_home_dir);
    app_service::promote_memory_card_to_global(Path::new(&project_path), &home, &id)
        .map_err(error_to_string)?;
    project_ack(&project_path)
}

#[tauri::command]
pub fn install_global_memory_card_to_project(
    project_path: String,
    home: Option<String>,
    id: String,
    targets: Vec<String>,
    confirmed_policy: Option<kernel::KernelPolicy>,
    decision_token: Option<String>,
) -> CommandResult<ProjectMutationAck> {
    enforce_tauri_kernel_policy_for_project(
        &project_path,
        kernel::KernelCommand::InstallGlobalMemoryCard {
            id: id.clone(),
            targets: targets.clone(),
        },
        serde_json::json!({ "id": id.clone(), "targets": targets.clone() }),
        confirmed_policy,
        decision_token,
    )?;
    let home = home.map(PathBuf::from).unwrap_or_else(default_home_dir);
    app_service::install_global_memory_card_to_project(
        Path::new(&project_path),
        &home,
        &id,
        targets,
    )
    .map_err(error_to_string)?;
    project_ack(&project_path)
}

#[tauri::command]
pub fn merge_memory_cards(
    project_path: String,
    input: MemoryCardMergeInput,
    confirmed_policy: Option<kernel::KernelPolicy>,
    decision_token: Option<String>,
) -> CommandResult<ProjectMutationAck> {
    let payload = serde_json::json!({ "input": input.clone() });
    enforce_tauri_kernel_policy_for_project(
        &project_path,
        kernel::KernelCommand::MergeMemoryCards {
            ids: input.sources.clone(),
        },
        payload,
        confirmed_policy,
        decision_token,
    )?;
    app_service::merge_memory_cards(Path::new(&project_path), input).map_err(error_to_string)?;
    project_ack(&project_path)
}

#[tauri::command]
pub fn fuse_memory_cards_to_draft(
    task_store: tauri::State<'_, DesktopTaskStore>,
    project_path: String,
    input: MemoryCardMergeInput,
    confirmed_policy: Option<kernel::KernelPolicy>,
    decision_token: Option<String>,
) -> CommandResult<DesktopJobStart> {
    let payload = serde_json::json!({ "input": input.clone() });
    enforce_tauri_kernel_policy_for_project(
        &project_path,
        kernel::KernelCommand::FuseMemoryCards {
            ids: input.sources.clone(),
            engine: "local".to_string(),
        },
        payload,
        confirmed_policy,
        decision_token,
    )?;
    Ok(app_service::start_fuse_memory_cards_to_draft_job(
        task_store.inner().clone(),
        project_path,
        input,
    ))
}

#[tauri::command]
pub fn attach_memory_card_to_skill(
    project_path: String,
    memory_card_id: String,
    skill_id: String,
    confirmed_policy: Option<kernel::KernelPolicy>,
    decision_token: Option<String>,
) -> CommandResult<ProjectMutationAck> {
    enforce_tauri_kernel_policy_for_project(
        &project_path,
        kernel::KernelCommand::AttachMemoryCardToSkill {
            memory_card_id: memory_card_id.clone(),
            skill_id: skill_id.clone(),
        },
        serde_json::json!({ "memory_card_id": memory_card_id.clone(), "skill_id": skill_id.clone() }),
        confirmed_policy,
        decision_token,
    )?;
    app_service::attach_memory_card_to_skill(Path::new(&project_path), &memory_card_id, &skill_id)
        .map_err(error_to_string)?;
    project_ack(&project_path)
}

#[tauri::command]
pub fn install_catalog_package(
    project_path: String,
    package_id: String,
    targets: Vec<String>,
    confirmed_policy: Option<kernel::KernelPolicy>,
    decision_token: Option<String>,
) -> CommandResult<ProjectMutationAck> {
    enforce_tauri_kernel_policy_for_project(
        &project_path,
        kernel::KernelCommand::InstallCatalogPackage {
            id: package_id.clone(),
            targets: targets.clone(),
        },
        serde_json::json!({ "package_id": package_id.clone(), "targets": targets.clone() }),
        confirmed_policy,
        decision_token,
    )?;
    app_service::install_catalog_package(Path::new(&project_path), &package_id, targets)
        .map_err(error_to_string)?;
    project_ack(&project_path)
}
