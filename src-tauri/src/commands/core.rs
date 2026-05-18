use std::path::{Path, PathBuf};
use agent_kernel::{build, config, draft, fsutil, kernel, project_registry, review, memory_card};
use serde_json::Value;

use crate::app_service;
use crate::jobs::{DesktopJobStart, DesktopTaskStore};
use crate::provider_config;
use crate::{app_state_for_home, default_home_dir, error_to_string, load_project_snapshot, CandidateUpdateInput, CommandResult, DesktopAppState, DraftMergeInput, DraftUpdateInput, KernelPlanInput, ProjectMutationAck, ProjectSnapshot, MemoryCardMergeInput, MemoryCardUpdateInput};
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
    memory_card::clear_memory_card_targets(Path::new(&project_path), agent).map_err(error_to_string)?;
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

fn clear_project_history_files(root: &Path) -> anyhow::Result<()> {
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
    app_service::install_global_memory_card_to_project(Path::new(&project_path), &home, &id, targets)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clear_memory_card_targets_command_clears_agent_assignments() {
        let temp = tempfile::tempdir().expect("tempdir");
        memory_card::add_memory_card(
            temp.path(),
            "project:use-axios",
            "Use Axios",
            "Use Axios for frontend HTTP requests.",
            "preference",
            "project",
            vec!["codex".to_string(), "claude-code".to_string()],
        )
        .expect("add memory_card");

        let ack = clear_memory_card_targets(
            fsutil::path_to_slash(temp.path()),
            Some("codex".to_string()),
            Some(kernel::KernelPolicy::agent_managed()),
            None,
        )
        .expect("clear targets");

        assert_eq!(ack.project_path, fsutil::path_to_slash(temp.path()));
        let project = agent_kernel::config::load_or_default_project_config(temp.path())
            .expect("project");
        assert_eq!(project.memory_cards.include[0].targets, vec!["claude-code"]);
        let audit = kernel::load_audit_entries(temp.path()).expect("audit");
        assert_eq!(audit[0].command, "clear-memory-card-targets");
        assert_eq!(audit[0].status, kernel::KernelAuditStatus::Authorized);
    }

    #[test]
    fn clear_project_history_removes_observation_index() {
        let temp = tempfile::tempdir().expect("tempdir");
        let kernel_dir = config::kernel_dir(temp.path());
        std::fs::create_dir_all(kernel_dir.join("observations")).expect("observations dir");
        std::fs::write(
            kernel_dir.join("observations").join("old.yml"),
            "id: obs:old\n",
        )
        .expect("observation");
        std::fs::write(
            kernel_dir.join("observation-index.yml"),
            "version: 1\nsources:\n- source_path: old.jsonl\n",
        )
        .expect("index");

        clear_project_history_files(temp.path()).expect("clear history");

        assert!(!kernel_dir.join("observations").exists());
        assert!(!kernel_dir.join("observation-index.yml").exists());
    }

    #[test]
    fn delete_memory_card_command_removes_assignment_and_audits() {
        let temp = tempfile::tempdir().expect("tempdir");
        memory_card::add_memory_card(
            temp.path(),
            "project:use-axios",
            "Use Axios",
            "Use Axios for frontend HTTP requests.",
            "preference",
            "project",
            vec!["codex".to_string()],
        )
        .expect("add memory_card");

        let token = kernel::create_decision_token(
            temp.path(),
            &kernel::KernelCommand::DeleteMemoryCard {
                id: "project:use-axios".to_string(),
            },
            &serde_json::json!({ "id": "project:use-axios" }),
            &kernel::KernelPolicy::manual(),
        )
        .expect("token");

        delete_memory_card(
            fsutil::path_to_slash(temp.path()),
            "project:use-axios".to_string(),
            Some(kernel::KernelPolicy::agent_managed()),
            Some(token),
        )
        .expect("delete memory_card");

        assert!(memory_card::load_memory_cards(temp.path()).expect("memory_cards").is_empty());
        let project = config::load_or_default_project_config(temp.path()).expect("project");
        assert!(project.memory_cards.include.is_empty());
        let audit = kernel::load_audit_entries(temp.path()).expect("audit");
        assert_eq!(audit[0].command, "delete-memory-card");
        assert_eq!(audit[0].status, kernel::KernelAuditStatus::Authorized);
    }
}
