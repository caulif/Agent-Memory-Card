use std::path::{Path, PathBuf};

use agent_kernel::{config, fsutil, kernel, project_registry};
use serde_json::Value;

use crate::app_service;
use crate::jobs::{DesktopJobStart, DesktopTaskStore};
use crate::{
    CommandResult, DesktopAppState, KernelPlanInput, ProjectMutationAck, ProjectSnapshot,
    app_state_for_home, default_home_dir, error_to_string, load_project_snapshot,
};

pub(crate) fn project_ack(project_path: &str) -> CommandResult<ProjectMutationAck> {
    let root = fsutil::normalize_project_root(Path::new(project_path)).map_err(error_to_string)?;
    Ok(ProjectMutationAck {
        project_path: fsutil::path_to_slash(&root),
    })
}

#[tauri::command]
pub fn get_app_state(home: Option<String>) -> CommandResult<DesktopAppState> {
    let home = home.map(PathBuf::from).unwrap_or_else(default_home_dir);
    app_state_for_home(&home).map_err(error_to_string)
}

#[tauri::command]
pub fn plan_kernel_command(input: KernelPlanInput) -> CommandResult<kernel::KernelDecision> {
    plan_kernel_command_input(input)
}

pub(crate) fn plan_kernel_command_input(
    input: KernelPlanInput,
) -> CommandResult<kernel::KernelDecision> {
    let mut decision = kernel::plan_command(&input.command, &input.policy);
    if let Some(project_path) = input.project_path {
        let token = kernel::create_decision_token(
            Path::new(&project_path),
            &input.command,
            &input.payload,
            &input.policy,
        )
        .map_err(error_to_string)?;
        decision.decision_token = Some(token);
    }
    Ok(decision)
}

pub(crate) fn enforce_tauri_kernel_policy(
    command: kernel::KernelCommand,
    confirmed_policy: Option<kernel::KernelPolicy>,
) -> CommandResult<kernel::KernelDecision> {
    let policy = confirmed_policy.unwrap_or_else(kernel::KernelPolicy::manual);
    kernel::enforce_command(&command, &policy).map_err(error_to_string)
}

pub(crate) fn enforce_tauri_kernel_policy_for_project(
    project_path: &str,
    command: kernel::KernelCommand,
    payload: Value,
    confirmed_policy: Option<kernel::KernelPolicy>,
    decision_token: Option<String>,
) -> CommandResult<kernel::KernelDecision> {
    let policy = confirmed_policy.unwrap_or_else(kernel::KernelPolicy::manual);
    let decision = kernel::plan_command(&command, &policy);
    let authorized = matches!(
        decision.disposition,
        kernel::KernelDisposition::Execute | kernel::KernelDisposition::DraftOnly
    );
    let token_error = if authorized && requires_decision_token(policy.mode, decision.risk) {
        match decision_token {
            Some(token) => {
                kernel::consume_decision_token(Path::new(project_path), &token, &command, &payload)
                    .err()
                    .map(error_to_string)
            }
            None => Some(
                "decision token is required for medium/high-risk project mutations".to_string(),
            ),
        }
    } else {
        None
    };
    let authorized = authorized && token_error.is_none();
    let status = if authorized {
        kernel::KernelAuditStatus::Authorized
    } else {
        kernel::KernelAuditStatus::Blocked
    };
    let entry =
        kernel::KernelAuditEntry::from_decision("tauri", &decision, policy.mode, status.clone());
    kernel::append_audit_entry(Path::new(project_path), &entry).map_err(error_to_string)?;

    if authorized {
        Ok(decision)
    } else if let Some(error) = token_error {
        Err(format!(
            "kernel policy blocked command '{}' because confirmation failed: {}",
            decision.command, error
        ))
    } else {
        Err(format!(
            "kernel policy blocked command '{}' with risk {:?}, disposition {:?}: {}",
            decision.command, decision.risk, decision.disposition, decision.reason
        ))
    }
}

fn requires_decision_token(_mode: kernel::AutomationMode, risk: kernel::KernelRisk) -> bool {
    matches!(risk, kernel::KernelRisk::Medium | kernel::KernelRisk::High)
}

#[tauri::command]
pub fn scan_projects(
    task_store: tauri::State<'_, DesktopTaskStore>,
    home: Option<String>,
    roots: Vec<String>,
    max_depth: usize,
    confirmed_policy: Option<kernel::KernelPolicy>,
) -> CommandResult<DesktopJobStart> {
    enforce_tauri_kernel_policy(
        kernel::KernelCommand::ScanProjects { max_depth },
        confirmed_policy,
    )?;
    let home = home.map(PathBuf::from).unwrap_or_else(default_home_dir);
    let roots = if roots.is_empty() {
        project_registry::default_scan_roots(&std::env::current_dir().map_err(error_to_string)?)
    } else {
        roots.into_iter().map(PathBuf::from).collect()
    };
    Ok(app_service::start_scan_projects_job(
        task_store.inner().clone(),
        home,
        roots,
        max_depth,
    ))
}

#[tauri::command]
pub fn add_project(
    home: Option<String>,
    path: String,
    confirmed_policy: Option<kernel::KernelPolicy>,
) -> CommandResult<project_registry::RegisteredProject> {
    enforce_tauri_kernel_policy(
        kernel::KernelCommand::AddProject { path: path.clone() },
        confirmed_policy,
    )?;
    let home = home.map(PathBuf::from).unwrap_or_else(default_home_dir);
    project_registry::add_project(&home, Path::new(&path)).map_err(error_to_string)
}

#[tauri::command]
pub fn get_project_snapshot(project_path: String) -> CommandResult<ProjectSnapshot> {
    load_project_snapshot(Path::new(&project_path)).map_err(error_to_string)
}

#[tauri::command]
pub fn get_project_dashboard(
    project_path: String,
    home: Option<String>,
) -> CommandResult<app_service::ProjectDashboard> {
    let home = home.map(PathBuf::from).unwrap_or_else(default_home_dir);
    app_service::load_project_dashboard(Path::new(&project_path), &home).map_err(error_to_string)
}

#[tauri::command]
pub fn get_project_review_inbox(
    project_path: String,
) -> CommandResult<app_service::ProjectReviewInbox> {
    app_service::load_project_review_inbox(Path::new(&project_path)).map_err(error_to_string)
}

#[tauri::command]
pub fn get_project_candidate_inbox(
    project_path: String,
) -> CommandResult<app_service::ProjectCandidateInbox> {
    app_service::load_project_candidate_inbox(Path::new(&project_path)).map_err(error_to_string)
}

#[tauri::command]
pub fn get_project_memory_card_library(
    project_path: String,
    home: Option<String>,
) -> CommandResult<app_service::ProjectMemoryCardLibrary> {
    let home = home.map(PathBuf::from).unwrap_or_else(default_home_dir);
    app_service::load_project_memory_card_library(Path::new(&project_path), &home)
        .map_err(error_to_string)
}

#[tauri::command]
pub fn get_project_assignment_view(
    project_path: String,
) -> CommandResult<app_service::ProjectAssignmentView> {
    app_service::load_project_assignment_view(Path::new(&project_path)).map_err(error_to_string)
}

#[tauri::command]
pub fn get_project_quality_view(
    project_path: String,
) -> CommandResult<app_service::ProjectQualityView> {
    app_service::load_project_quality_view(Path::new(&project_path)).map_err(error_to_string)
}

#[tauri::command]
pub fn get_project_eval_run(
    project_path: String,
) -> CommandResult<app_service::ProjectEvalRunView> {
    app_service::load_project_eval_run_view(Path::new(&project_path)).map_err(error_to_string)
}

#[tauri::command]
pub fn import_project(
    project_path: String,
    scan_home: bool,
    confirmed_policy: Option<kernel::KernelPolicy>,
    decision_token: Option<String>,
) -> CommandResult<ProjectMutationAck> {
    enforce_tauri_kernel_policy_for_project(
        &project_path,
        kernel::KernelCommand::ImportProject { scan_home },
        serde_json::json!({ "scan_home": scan_home }),
        confirmed_policy,
        decision_token,
    )?;
    app_service::import_project(Path::new(&project_path), scan_home).map_err(error_to_string)?;
    project_ack(&project_path)
}

#[tauri::command]
pub fn clear_project_history(
    task_store: tauri::State<'_, DesktopTaskStore>,
    project_path: String,
    confirmed_policy: Option<kernel::KernelPolicy>,
    decision_token: Option<String>,
) -> CommandResult<ProjectMutationAck> {
    let command = kernel::KernelCommand::ClearProjectHistory;
    enforce_tauri_kernel_policy_for_project(
        &project_path,
        command,
        serde_json::json!({}),
        confirmed_policy,
        decision_token,
    )?;
    let root = fsutil::normalize_project_root(Path::new(&project_path)).map_err(error_to_string)?;
    clear_project_history_files(&root).map_err(error_to_string)?;
    task_store.clear_history();
    project_ack(&project_path)
}

pub(crate) fn clear_project_history_files(root: &Path) -> anyhow::Result<()> {
    let kernel_dir = config::kernel_dir(root);
    for name in ["candidates", "drafts", "memory-cards", "observations"] {
        let path = kernel_dir.join(name);
        if path.exists() {
            std::fs::remove_dir_all(&path)?;
        }
    }
    for name in [
        "audit-log.jsonl",
        "decision-tokens.jsonl",
        "observation-index.yml",
    ] {
        let path = kernel_dir.join(name);
        if path.exists() {
            std::fs::remove_file(path)?;
        }
    }
    let mut project = config::load_or_default_project_config(root)?;
    project.memory_cards.include.clear();
    project.skills.supplements.clear();
    config::save_project_config(root, &project)
}
