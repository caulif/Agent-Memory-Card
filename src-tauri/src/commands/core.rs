use std::path::{Path, PathBuf};
use agent_kernel::{build, draft, fsutil, kernel, project_registry, review, skilllet};
use serde_json::Value;

use crate::app_service;
use crate::jobs::{DesktopJobStart, DesktopTaskStore};
use crate::{app_state_for_home, default_home_dir, error_to_string, load_project_snapshot, CommandResult, DesktopAppState, DraftMergeInput, DraftUpdateInput, KernelPlanInput, ProjectMutationAck, ProjectSnapshot, SkillletMergeInput, SkillletUpdateInput};
fn project_ack(project_path: &str) -> CommandResult<ProjectMutationAck> {
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

pub(crate) fn plan_kernel_command_input(input: KernelPlanInput) -> CommandResult<kernel::KernelDecision> {
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
pub fn get_project_skilllet_library(
    project_path: String,
    home: Option<String>,
) -> CommandResult<app_service::ProjectSkillletLibrary> {
    let home = home.map(PathBuf::from).unwrap_or_else(default_home_dir);
    app_service::load_project_skilllet_library(Path::new(&project_path), &home)
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
) -> CommandResult<skilllet::SkillletRecord> {
    enforce_tauri_kernel_policy_for_project(
        &project_path,
        kernel::KernelCommand::PromoteCandidate { id: id.clone() },
        serde_json::json!({ "id": id.clone() }),
        confirmed_policy,
        decision_token,
    )?;
    app_service::approve_candidate_to_skilllet(Path::new(&project_path), &id)
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
pub fn set_skilllet_targets(
    project_path: String,
    id: String,
    targets: Vec<String>,
    confirmed_policy: Option<kernel::KernelPolicy>,
    decision_token: Option<String>,
) -> CommandResult<ProjectMutationAck> {
    enforce_tauri_kernel_policy_for_project(
        &project_path,
        kernel::KernelCommand::AssignSkilllet {
            id: id.clone(),
            targets: targets.clone(),
        },
        serde_json::json!({ "id": id.clone(), "targets": targets.clone() }),
        confirmed_policy,
        decision_token,
    )?;
    app_service::set_skilllet_targets(Path::new(&project_path), &id, targets)
        .map_err(error_to_string)?;
    project_ack(&project_path)
}

#[tauri::command]
pub fn update_skilllet(
    project_path: String,
    id: String,
    input: SkillletUpdateInput,
    confirmed_policy: Option<kernel::KernelPolicy>,
    decision_token: Option<String>,
) -> CommandResult<skilllet::SkillletRecord> {
    let payload = serde_json::json!({ "id": id.clone(), "input": input.clone() });
    enforce_tauri_kernel_policy_for_project(
        &project_path,
        kernel::KernelCommand::UpdateSkilllet { id: id.clone() },
        payload,
        confirmed_policy,
        decision_token,
    )?;
    app_service::update_skilllet(Path::new(&project_path), &id, input).map_err(error_to_string)
}

#[tauri::command]
pub fn promote_skilllet_to_global(
    project_path: String,
    home: Option<String>,
    id: String,
    confirmed_policy: Option<kernel::KernelPolicy>,
    decision_token: Option<String>,
) -> CommandResult<ProjectMutationAck> {
    enforce_tauri_kernel_policy_for_project(
        &project_path,
        kernel::KernelCommand::PromoteSkillletToGlobal { id: id.clone() },
        serde_json::json!({ "id": id.clone() }),
        confirmed_policy,
        decision_token,
    )?;
    let home = home.map(PathBuf::from).unwrap_or_else(default_home_dir);
    app_service::promote_skilllet_to_global(Path::new(&project_path), &home, &id)
        .map_err(error_to_string)?;
    project_ack(&project_path)
}

#[tauri::command]
pub fn install_global_skilllet_to_project(
    project_path: String,
    home: Option<String>,
    id: String,
    targets: Vec<String>,
    confirmed_policy: Option<kernel::KernelPolicy>,
    decision_token: Option<String>,
) -> CommandResult<ProjectMutationAck> {
    enforce_tauri_kernel_policy_for_project(
        &project_path,
        kernel::KernelCommand::InstallGlobalSkilllet {
            id: id.clone(),
            targets: targets.clone(),
        },
        serde_json::json!({ "id": id.clone(), "targets": targets.clone() }),
        confirmed_policy,
        decision_token,
    )?;
    let home = home.map(PathBuf::from).unwrap_or_else(default_home_dir);
    app_service::install_global_skilllet_to_project(Path::new(&project_path), &home, &id, targets)
        .map_err(error_to_string)?;
    project_ack(&project_path)
}

#[tauri::command]
pub fn merge_skilllets(
    project_path: String,
    input: SkillletMergeInput,
    confirmed_policy: Option<kernel::KernelPolicy>,
    decision_token: Option<String>,
) -> CommandResult<ProjectMutationAck> {
    let payload = serde_json::json!({ "input": input.clone() });
    enforce_tauri_kernel_policy_for_project(
        &project_path,
        kernel::KernelCommand::MergeSkilllets {
            ids: input.sources.clone(),
        },
        payload,
        confirmed_policy,
        decision_token,
    )?;
    app_service::merge_skilllets(Path::new(&project_path), input).map_err(error_to_string)?;
    project_ack(&project_path)
}

#[tauri::command]
pub fn fuse_skilllets_to_draft(
    task_store: tauri::State<'_, DesktopTaskStore>,
    project_path: String,
    input: SkillletMergeInput,
    confirmed_policy: Option<kernel::KernelPolicy>,
    decision_token: Option<String>,
) -> CommandResult<DesktopJobStart> {
    let payload = serde_json::json!({ "input": input.clone() });
    enforce_tauri_kernel_policy_for_project(
        &project_path,
        kernel::KernelCommand::FuseSkilllets {
            ids: input.sources.clone(),
            engine: "local".to_string(),
        },
        payload,
        confirmed_policy,
        decision_token,
    )?;
    Ok(app_service::start_fuse_skilllets_to_draft_job(
        task_store.inner().clone(),
        project_path,
        input,
    ))
}

#[tauri::command]
pub fn attach_skilllet_to_skill(
    project_path: String,
    skilllet_id: String,
    skill_id: String,
    confirmed_policy: Option<kernel::KernelPolicy>,
    decision_token: Option<String>,
) -> CommandResult<ProjectMutationAck> {
    enforce_tauri_kernel_policy_for_project(
        &project_path,
        kernel::KernelCommand::AttachSkillletToSkill {
            skilllet_id: skilllet_id.clone(),
            skill_id: skill_id.clone(),
        },
        serde_json::json!({ "skilllet_id": skilllet_id.clone(), "skill_id": skill_id.clone() }),
        confirmed_policy,
        decision_token,
    )?;
    app_service::attach_skilllet_to_skill(Path::new(&project_path), &skilllet_id, &skill_id)
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
