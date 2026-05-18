use std::path::{Path, PathBuf};

use agent_kernel::{draft, kernel, memory_card, review};

use crate::app_service;
use crate::commands::project::{enforce_tauri_kernel_policy_for_project, project_ack};
use crate::jobs::{DesktopJobStart, DesktopTaskStore};
use crate::{
    CandidateUpdateInput, CommandResult, DraftMergeInput, DraftUpdateInput, ProjectMutationAck,
    default_home_dir, error_to_string,
};

#[tauri::command]
pub fn review_project(project_path: String) -> CommandResult<review::ReviewReport> {
    review::review_project(Path::new(&project_path)).map_err(error_to_string)
}

#[tauri::command]
pub fn approve_draft(
    project_path: String,
    id: String,
    confirmed_policy: Option<kernel::KernelPolicy>,
    decision_token: Option<String>,
) -> CommandResult<ProjectMutationAck> {
    enforce_tauri_kernel_policy_for_project(
        &project_path,
        kernel::KernelCommand::ApproveDraft { id: id.clone() },
        serde_json::json!({ "id": id.clone() }),
        confirmed_policy,
        decision_token,
    )?;
    app_service::approve_draft(Path::new(&project_path), &id).map_err(error_to_string)?;
    project_ack(&project_path)
}

#[tauri::command]
pub fn reject_draft(
    project_path: String,
    id: String,
    confirmed_policy: Option<kernel::KernelPolicy>,
    decision_token: Option<String>,
) -> CommandResult<ProjectMutationAck> {
    enforce_tauri_kernel_policy_for_project(
        &project_path,
        kernel::KernelCommand::RejectDraft { id: id.clone() },
        serde_json::json!({ "id": id.clone() }),
        confirmed_policy,
        decision_token,
    )?;
    app_service::reject_draft(Path::new(&project_path), &id).map_err(error_to_string)?;
    project_ack(&project_path)
}

#[tauri::command]
pub fn promote_candidate(
    project_path: String,
    id: String,
    confirmed_policy: Option<kernel::KernelPolicy>,
    decision_token: Option<String>,
) -> CommandResult<memory_card::MemoryCardRecord> {
    enforce_tauri_kernel_policy_for_project(
        &project_path,
        kernel::KernelCommand::PromoteCandidate { id: id.clone() },
        serde_json::json!({ "id": id.clone() }),
        confirmed_policy,
        decision_token,
    )?;
    app_service::approve_candidate_to_memory_card(Path::new(&project_path), &id)
        .map_err(error_to_string)
}

#[tauri::command]
pub fn hide_candidate(
    project_path: String,
    id: String,
    confirmed_policy: Option<kernel::KernelPolicy>,
    decision_token: Option<String>,
) -> CommandResult<agent_kernel::candidate::CandidateRecord> {
    enforce_tauri_kernel_policy_for_project(
        &project_path,
        kernel::KernelCommand::HideCandidate { id: id.clone() },
        serde_json::json!({ "id": id.clone() }),
        confirmed_policy,
        decision_token,
    )?;
    app_service::hide_candidate(Path::new(&project_path), &id).map_err(error_to_string)
}

#[tauri::command]
pub fn reject_candidate(
    project_path: String,
    id: String,
    reason: Option<String>,
    confirmed_policy: Option<kernel::KernelPolicy>,
    decision_token: Option<String>,
) -> CommandResult<agent_kernel::candidate::CandidateRecord> {
    enforce_tauri_kernel_policy_for_project(
        &project_path,
        kernel::KernelCommand::RejectCandidate { id: id.clone() },
        serde_json::json!({ "id": id.clone(), "reason": reason.clone() }),
        confirmed_policy,
        decision_token,
    )?;
    app_service::reject_candidate(Path::new(&project_path), &id, reason).map_err(error_to_string)
}

#[tauri::command]
pub fn update_candidate(
    project_path: String,
    id: String,
    input: CandidateUpdateInput,
    confirmed_policy: Option<kernel::KernelPolicy>,
    decision_token: Option<String>,
) -> CommandResult<agent_kernel::candidate::CandidateRecord> {
    let payload = serde_json::json!({ "id": id.clone(), "input": input.clone() });
    enforce_tauri_kernel_policy_for_project(
        &project_path,
        kernel::KernelCommand::UpdateCandidate { id: id.clone() },
        payload,
        confirmed_policy,
        decision_token,
    )?;
    app_service::update_candidate(Path::new(&project_path), &id, input).map_err(error_to_string)
}

#[tauri::command]
pub fn gc_candidates(
    project_path: String,
    confirmed_policy: Option<kernel::KernelPolicy>,
    decision_token: Option<String>,
) -> CommandResult<agent_kernel::candidate::CandidateGcReport> {
    enforce_tauri_kernel_policy_for_project(
        &project_path,
        kernel::KernelCommand::GcCandidates,
        serde_json::json!({}),
        confirmed_policy,
        decision_token,
    )?;
    app_service::gc_candidates(Path::new(&project_path)).map_err(error_to_string)
}

#[tauri::command]
pub fn update_draft(
    project_path: String,
    id: String,
    input: DraftUpdateInput,
    confirmed_policy: Option<kernel::KernelPolicy>,
    decision_token: Option<String>,
) -> CommandResult<draft::DraftRecord> {
    let payload = serde_json::json!({ "id": id.clone(), "input": input.clone() });
    enforce_tauri_kernel_policy_for_project(
        &project_path,
        kernel::KernelCommand::UpdateDraft { id: id.clone() },
        payload,
        confirmed_policy,
        decision_token,
    )?;
    app_service::update_draft(Path::new(&project_path), &id, input).map_err(error_to_string)
}

#[tauri::command]
pub fn merge_drafts(
    project_path: String,
    input: DraftMergeInput,
    confirmed_policy: Option<kernel::KernelPolicy>,
    decision_token: Option<String>,
) -> CommandResult<ProjectMutationAck> {
    let payload = serde_json::json!({ "input": input.clone() });
    enforce_tauri_kernel_policy_for_project(
        &project_path,
        kernel::KernelCommand::MergeDrafts {
            ids: input.sources.clone(),
        },
        payload,
        confirmed_policy,
        decision_token,
    )?;
    app_service::merge_drafts(Path::new(&project_path), input).map_err(error_to_string)?;
    project_ack(&project_path)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn evolve_project(
    task_store: tauri::State<'_, DesktopTaskStore>,
    project_path: String,
    home: Option<String>,
    targets: Vec<String>,
    dry_run: bool,
    engine: Option<String>,
    confirmed_policy: Option<kernel::KernelPolicy>,
    decision_token: Option<String>,
) -> CommandResult<DesktopJobStart> {
    let home = home.map(PathBuf::from).unwrap_or_else(default_home_dir);
    let engine = engine.unwrap_or_else(|| "local".to_string());
    enforce_tauri_kernel_policy_for_project(
        &project_path,
        kernel::KernelCommand::EvolveProject {
            dry_run,
            engine: engine.clone(),
        },
        serde_json::json!({ "targets": targets.clone(), "dry_run": dry_run, "engine": engine.clone() }),
        confirmed_policy,
        decision_token,
    )?;
    Ok(app_service::start_evolve_project_job(
        task_store.inner().clone(),
        project_path,
        home,
        targets,
        dry_run,
        engine,
    ))
}
