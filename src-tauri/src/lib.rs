use std::path::{Path, PathBuf};
use std::thread;

mod app_service;
mod commands;
mod jobs;
mod provider_config;

use agent_kernel::{
    build, catalog, config, draft, fsutil, kernel, observation, project_registry, rule_test,
    memory_card,
};
use jobs::{job_start_from_status, DesktopJobReplay, DesktopJobStart, DesktopTaskStore};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, Serialize)]
pub struct DesktopAppState {
    pub home: String,
    pub scan_roots: Vec<String>,
    pub registry: project_registry::ProjectRegistry,
}

#[derive(Debug, Serialize)]
pub struct ProjectSnapshot {
    pub project_path: String,
    pub config: config::ProjectConfig,
    pub candidates: Vec<agent_kernel::candidate::CandidateRecord>,
    pub memory_cards: Vec<memory_card::MemoryCardRecord>,
    pub global_memory_cards: Vec<memory_card::MemoryCardRecord>,
    pub drafts: Vec<draft::DraftRecord>,
    pub observations: Vec<observation::ObservationRecord>,
    pub skill_index: config::SkillIndex,
    pub lock: config::ProjectLock,
    pub catalog_status: catalog::CatalogStatus,
    pub catalog_validation: catalog::CatalogValidationReport,
    pub target_matrix: memory_card::MemoryCardTargetMatrix,
    pub rule_ci: rule_test::RuleTestReport,
    pub build_preview: build::BuildReport,
    pub status: build::StatusReport,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftUpdateInput {
    pub title: Option<String>,
    pub body: Option<String>,
    pub brief: Option<String>,
    pub tags: Option<Vec<String>>,
    pub language: Option<String>,
    pub kind: Option<String>,
    pub scope: Option<String>,
    pub targets: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateUpdateInput {
    pub title: Option<String>,
    pub body: Option<String>,
    pub brief: Option<String>,
    pub tags: Option<Vec<String>>,
    pub language: Option<String>,
    pub kind: Option<String>,
    pub scope: Option<String>,
    pub targets: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCardUpdateInput {
    pub title: Option<String>,
    pub body: Option<String>,
    pub brief: Option<String>,
    pub tags: Option<Vec<String>>,
    pub language: Option<String>,
    pub kind: Option<String>,
    pub scope: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftMergeInput {
    pub id: String,
    pub title: String,
    pub sources: Vec<String>,
    #[serde(default)]
    pub targets: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCardMergeInput {
    pub id: String,
    pub title: String,
    pub sources: Vec<String>,
    #[serde(default)]
    pub targets: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KernelPlanInput {
    pub command: kernel::KernelCommand,
    pub policy: kernel::KernelPolicy,
    pub project_path: Option<String>,
    #[serde(default)]
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectMutationAck {
    pub project_path: String,
}

pub(crate) type CommandResult<T> = Result<T, String>;

pub fn run() {
    let task_store = DesktopTaskStore::with_job_history(&default_home_dir());
    let startup_store = task_store.clone();
    tauri::Builder::default()
        .manage(task_store)
        .setup(move |_| {
            let setup_store = startup_store.clone();
            thread::spawn(move || {
                let job_id = setup_store.begin_job(
                    "启动",
                    "发现注册项目",
                    "正在准备本地工作台，不会在启动时阻塞整理历史",
                    18,
                );
                setup_store.job_step(
                    &job_id,
                    "等待用户操作",
                    "点击“提炼高价值 Memory Card”后才会读取历史；删除、编辑和切换页面保持可用",
                    80,
                );
                setup_store.finish_job(&job_id, "工作台已就绪");
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::project::get_app_state,
            commands::project::plan_kernel_command,
            commands::project::scan_projects,
            commands::project::add_project,
            commands::project::get_project_dashboard,
            commands::project::get_project_candidate_inbox,
            commands::project::get_project_review_inbox,
            commands::project::get_project_memory_card_library,
            commands::project::get_project_skill_library,
            commands::project::get_project_assignment_view,
            commands::project::get_project_quality_view,
            commands::project::get_project_eval_run,
            commands::provider::get_custom_provider_config,
            commands::provider::save_custom_provider_config,
            commands::provider::get_setup_checklist,
            commands::provider::test_provider_status,
            commands::project::get_project_snapshot,
            commands::project::import_project,
            commands::review::review_project,
            commands::review::approve_draft,
            commands::review::reject_draft,
            commands::review::promote_candidate,
            commands::review::hide_candidate,
            commands::review::reject_candidate,
            commands::review::update_candidate,
            commands::review::gc_candidates,
            commands::review::update_draft,
            commands::review::merge_drafts,
            commands::assignment::set_agent_enabled,
            commands::assignment::set_memory_card_targets,
            commands::assignment::clear_memory_card_targets,
            commands::project::clear_project_history,
            commands::memory::update_memory_card,
            commands::memory::delete_memory_card,
            commands::memory::promote_memory_card_to_global,
            commands::memory::install_global_memory_card_to_project,
            commands::memory::merge_memory_cards,
            commands::memory::fuse_memory_cards_to_draft,
            commands::memory::attach_memory_card_to_skill,
            commands::memory::install_catalog_package,
            commands::review::evolve_project,
            commands::artifact::import_artifact_drifts,
            commands::artifact::import_artifact_drift_path,
            commands::artifact::keep_artifact_drifts,
            commands::artifact::keep_artifact_drift_path,
            commands::artifact::discard_artifact_drifts,
            commands::artifact::discard_artifact_drift_path,
            commands::artifact::build_preview,
            commands::artifact::sync_project,
            commands::jobs::get_task_status,
            commands::jobs::get_job_history,
            commands::jobs::cancel_job
        ])
        .run(tauri::generate_context!())
        .expect("error while running Agent Memory Kernel desktop app");
}

pub(crate) fn app_state_for_home(home: &Path) -> anyhow::Result<DesktopAppState> {
    let cwd = std::env::current_dir()?;
    let scan_roots = project_registry::default_scan_roots(&cwd)
        .into_iter()
        .map(|path| fsutil::path_to_slash(&path))
        .collect();
    Ok(DesktopAppState {
        home: fsutil::path_to_slash(home),
        scan_roots,
        registry: project_registry::load_registry(home)?,
    })
}

pub(crate) fn load_project_snapshot(project_root: &Path) -> anyhow::Result<ProjectSnapshot> {
    let root = fsutil::normalize_project_root(project_root)?;
    let catalog = catalog::load_or_default_catalog(&root)?;

    Ok(ProjectSnapshot {
        project_path: fsutil::path_to_slash(&root),
        config: config::load_or_default_project_config(&root)?,
        candidates: agent_kernel::candidate::load_candidates(&root)?,
        memory_cards: memory_card::load_memory_cards(&root)?,
        global_memory_cards: memory_card::load_global_memory_cards(&default_home_dir())?,
        drafts: draft::load_drafts(&root)?,
        observations: observation::load_observations(&root)?,
        skill_index: config::load_skill_index(&root)?,
        lock: config::load_lock(&root)?,
        catalog_status: catalog::catalog_status(&root)?,
        catalog_validation: catalog::validate_catalog(&catalog),
        target_matrix: memory_card::memory_card_target_matrix(&root)?,
        rule_ci: rule_test::run_rule_tests(&root)?,
        build_preview: build::build_project(&root, true)?,
        status: build::status_project(&root)?,
    })
}

pub(crate) fn default_home_dir() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub(crate) fn error_to_string(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
