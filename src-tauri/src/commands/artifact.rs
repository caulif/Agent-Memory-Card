use std::path::Path;

use agent_kernel::{build, kernel};

use crate::app_service;
use crate::commands::project::{enforce_tauri_kernel_policy_for_project, project_ack};
use crate::jobs::{DesktopJobStart, DesktopTaskStore};
use crate::{CommandResult, ProjectMutationAck, error_to_string};

#[tauri::command]
pub fn import_artifact_drifts(
    project_path: String,
    confirmed_policy: Option<kernel::KernelPolicy>,
    decision_token: Option<String>,
) -> CommandResult<ProjectMutationAck> {
    enforce_tauri_kernel_policy_for_project(
        &project_path,
        kernel::KernelCommand::ImportArtifactDrifts,
        serde_json::json!({}),
        confirmed_policy,
        decision_token,
    )?;
    app_service::import_artifact_drifts(Path::new(&project_path)).map_err(error_to_string)?;
    project_ack(&project_path)
}

#[tauri::command]
pub fn import_artifact_drift_path(
    project_path: String,
    artifact_path: String,
    confirmed_policy: Option<kernel::KernelPolicy>,
    decision_token: Option<String>,
) -> CommandResult<ProjectMutationAck> {
    enforce_tauri_kernel_policy_for_project(
        &project_path,
        kernel::KernelCommand::ImportArtifactDrifts,
        serde_json::json!({ "artifact_path": artifact_path.clone() }),
        confirmed_policy,
        decision_token,
    )?;
    app_service::import_artifact_drift_path(Path::new(&project_path), &artifact_path)
        .map_err(error_to_string)?;
    project_ack(&project_path)
}

#[tauri::command]
pub fn keep_artifact_drifts(
    project_path: String,
    confirmed_policy: Option<kernel::KernelPolicy>,
    decision_token: Option<String>,
) -> CommandResult<ProjectMutationAck> {
    enforce_tauri_kernel_policy_for_project(
        &project_path,
        kernel::KernelCommand::KeepArtifactDrifts,
        serde_json::json!({}),
        confirmed_policy,
        decision_token,
    )?;
    app_service::keep_artifact_drifts(Path::new(&project_path)).map_err(error_to_string)?;
    project_ack(&project_path)
}

#[tauri::command]
pub fn keep_artifact_drift_path(
    project_path: String,
    artifact_path: String,
    confirmed_policy: Option<kernel::KernelPolicy>,
    decision_token: Option<String>,
) -> CommandResult<ProjectMutationAck> {
    enforce_tauri_kernel_policy_for_project(
        &project_path,
        kernel::KernelCommand::KeepArtifactDrifts,
        serde_json::json!({ "artifact_path": artifact_path.clone() }),
        confirmed_policy,
        decision_token,
    )?;
    app_service::keep_artifact_drift_path(Path::new(&project_path), &artifact_path)
        .map_err(error_to_string)?;
    project_ack(&project_path)
}

#[tauri::command]
pub fn discard_artifact_drifts(
    project_path: String,
    confirmed_policy: Option<kernel::KernelPolicy>,
    decision_token: Option<String>,
) -> CommandResult<ProjectMutationAck> {
    enforce_tauri_kernel_policy_for_project(
        &project_path,
        kernel::KernelCommand::DiscardArtifactDrifts,
        serde_json::json!({}),
        confirmed_policy,
        decision_token,
    )?;
    app_service::discard_artifact_drifts(Path::new(&project_path)).map_err(error_to_string)?;
    project_ack(&project_path)
}

#[tauri::command]
pub fn discard_artifact_drift_path(
    project_path: String,
    artifact_path: String,
    confirmed_policy: Option<kernel::KernelPolicy>,
    decision_token: Option<String>,
) -> CommandResult<ProjectMutationAck> {
    enforce_tauri_kernel_policy_for_project(
        &project_path,
        kernel::KernelCommand::DiscardArtifactDrifts,
        serde_json::json!({ "artifact_path": artifact_path.clone() }),
        confirmed_policy,
        decision_token,
    )?;
    app_service::discard_artifact_drift_path(Path::new(&project_path), &artifact_path)
        .map_err(error_to_string)?;
    project_ack(&project_path)
}

#[tauri::command]
pub fn build_preview(project_path: String) -> CommandResult<build::BuildReport> {
    app_service::build_preview(Path::new(&project_path)).map_err(error_to_string)
}

#[tauri::command]
pub fn sync_project(
    task_store: tauri::State<'_, DesktopTaskStore>,
    project_path: String,
    confirmed_policy: Option<kernel::KernelPolicy>,
    decision_token: Option<String>,
) -> CommandResult<DesktopJobStart> {
    enforce_tauri_kernel_policy_for_project(
        &project_path,
        kernel::KernelCommand::CompileProject { dry_run: false },
        serde_json::json!({ "dry_run": false }),
        confirmed_policy,
        decision_token,
    )?;
    Ok(app_service::start_sync_project_job(
        task_store.inner().clone(),
        project_path,
    ))
}

#[cfg(test)]
pub(crate) fn sync_project_for_test(
    project_path: String,
    confirmed_policy: Option<kernel::KernelPolicy>,
    decision_token: Option<String>,
) -> CommandResult<ProjectMutationAck> {
    enforce_tauri_kernel_policy_for_project(
        &project_path,
        kernel::KernelCommand::CompileProject { dry_run: false },
        serde_json::json!({ "dry_run": false }),
        confirmed_policy,
        decision_token,
    )?;
    app_service::compile_project(Path::new(&project_path)).map_err(error_to_string)?;
    project_ack(&project_path)
}
