use std::path::{Path, PathBuf};
use std::thread;

use agent_kernel::{
    build, candidate, catalog, config, draft, fsutil, kernel, observation, project_registry,
    rule_test, scanner, memory_card,
};
use serde::Serialize;

use crate::{
    CandidateUpdateInput, DesktopJobReplay, DesktopJobStart, DesktopTaskStore, DraftMergeInput, DraftUpdateInput,
    MemoryCardMergeInput, MemoryCardUpdateInput,
};

#[derive(Debug, Clone, Serialize)]
pub struct ProjectDashboard {
    pub project_path: String,
    pub candidate_count: usize,
    pub draft_count: usize,
    pub memory_card_count: usize,
    pub observation_count: usize,
    pub global_memory_card_count: usize,
    pub enabled_agents: Vec<String>,
    pub warning_count: usize,
}

#[derive(Debug, Serialize)]
pub struct ProjectReviewInbox {
    pub project_path: String,
    pub drafts: Vec<draft::DraftRecord>,
}

#[derive(Debug, Serialize)]
pub struct ProjectCandidateInbox {
    pub project_path: String,
    pub candidates: Vec<candidate::CandidateRecord>,
}

#[derive(Debug, Serialize)]
pub struct ProjectMemoryCardLibrary {
    pub project_path: String,
    pub memory_cards: Vec<memory_card::MemoryCardRecord>,
    pub global_memory_cards: Vec<memory_card::MemoryCardRecord>,
    pub catalog_status: catalog::CatalogStatus,
}

#[derive(Debug, Serialize)]
pub struct ProjectAssignmentView {
    pub project_path: String,
    pub enabled_agents: Vec<String>,
    pub target_matrix: memory_card::MemoryCardTargetMatrix,
}

#[derive(Debug, Serialize)]
pub struct ProjectQualityView {
    pub project_path: String,
    pub rule_ci: rule_test::RuleTestReport,
    pub build_preview: build::BuildReport,
    pub status: build::StatusReport,
}

pub fn load_project_dashboard(
    project_root: &Path,
    home: &Path,
) -> anyhow::Result<ProjectDashboard> {
    let root = fsutil::normalize_project_root(project_root)?;
    let config = config::load_or_default_project_config(&root)?;
    let enabled_agents = config
        .agents
        .iter()
        .filter(|(_, agent)| agent.enabled)
        .map(|(name, _)| name.clone())
        .collect::<Vec<_>>();

    Ok(ProjectDashboard {
        project_path: fsutil::path_to_slash(&root),
        candidate_count: candidate::list_visible_candidates(&root)?.len(),
        draft_count: draft::load_reviewable_drafts(&root)?.len(),
        memory_card_count: memory_card::load_memory_cards(&root)?.len(),
        observation_count: observation::count_observations(&root)?,
        global_memory_card_count: memory_card::count_global_memory_cards(home)?,
        enabled_agents,
        warning_count: 0,
    })
}

pub fn load_project_review_inbox(project_root: &Path) -> anyhow::Result<ProjectReviewInbox> {
    let root = fsutil::normalize_project_root(project_root)?;
    Ok(ProjectReviewInbox {
        project_path: fsutil::path_to_slash(&root),
        drafts: draft::load_reviewable_drafts(&root)?,
    })
}

pub fn load_project_candidate_inbox(project_root: &Path) -> anyhow::Result<ProjectCandidateInbox> {
    let root = fsutil::normalize_project_root(project_root)?;
    Ok(ProjectCandidateInbox {
        project_path: fsutil::path_to_slash(&root),
        candidates: candidate::list_visible_candidates(&root)?,
    })
}

pub fn approve_candidate_to_memory_card(
    project_root: &Path,
    id: &str,
) -> anyhow::Result<memory_card::MemoryCardRecord> {
    let root = fsutil::normalize_project_root(project_root)?;
    candidate::approve_candidate_to_memory_card(&root, id)
}

pub fn hide_candidate(
    project_root: &Path,
    id: &str,
) -> anyhow::Result<candidate::CandidateRecord> {
    let root = fsutil::normalize_project_root(project_root)?;
    candidate::hide_candidate(&root, id)
}

pub fn reject_candidate(
    project_root: &Path,
    id: &str,
    reason: Option<String>,
) -> anyhow::Result<candidate::CandidateRecord> {
    let root = fsutil::normalize_project_root(project_root)?;
    candidate::reject_candidate(&root, id, reason)
}

pub fn update_candidate(
    project_root: &Path,
    id: &str,
    input: CandidateUpdateInput,
) -> anyhow::Result<candidate::CandidateRecord> {
    let root = fsutil::normalize_project_root(project_root)?;
    candidate::update_candidate(&root, id, candidate_update_from_input(input))
}

pub fn gc_candidates(project_root: &Path) -> anyhow::Result<candidate::CandidateGcReport> {
    let root = fsutil::normalize_project_root(project_root)?;
    candidate::gc_candidates(&root)
}

pub fn approve_draft(project_root: &Path, id: &str) -> anyhow::Result<()> {
    let root = fsutil::normalize_project_root(project_root)?;
    draft::approve_draft(&root, id)
}

pub fn reject_draft(project_root: &Path, id: &str) -> anyhow::Result<()> {
    let root = fsutil::normalize_project_root(project_root)?;
    draft::reject_draft(&root, id)
}

pub fn update_draft(
    project_root: &Path,
    id: &str,
    input: DraftUpdateInput,
) -> anyhow::Result<draft::DraftRecord> {
    let root = fsutil::normalize_project_root(project_root)?;
    draft::update_draft(&root, id, draft_update_from_input(input))
}

pub fn merge_drafts(project_root: &Path, input: DraftMergeInput) -> anyhow::Result<draft::DraftRecord> {
    let root = fsutil::normalize_project_root(project_root)?;
    draft::merge_drafts(
        &root,
        &input.id,
        &input.title,
        input.sources,
        input.targets,
    )
}

pub fn set_agent_enabled(project_root: &Path, agent: &str, enabled: bool) -> anyhow::Result<()> {
    let root = fsutil::normalize_project_root(project_root)?;
    config::set_agent_enabled(&root, agent, enabled)
}

pub fn set_memory_card_targets(
    project_root: &Path,
    id: &str,
    targets: Vec<String>,
) -> anyhow::Result<()> {
    let root = fsutil::normalize_project_root(project_root)?;
    memory_card::set_memory_card_targets(&root, id, targets)
}

pub fn update_memory_card(
    project_root: &Path,
    id: &str,
    input: MemoryCardUpdateInput,
) -> anyhow::Result<memory_card::MemoryCardRecord> {
    let root = fsutil::normalize_project_root(project_root)?;
    memory_card::update_memory_card(&root, id, memory_card_update_from_input(input))
}

pub fn promote_memory_card_to_global(
    project_root: &Path,
    home: &Path,
    id: &str,
) -> anyhow::Result<memory_card::MemoryCardRecord> {
    let root = fsutil::normalize_project_root(project_root)?;
    memory_card::promote_memory_card_to_global(&root, home, id)
}

pub fn install_global_memory_card_to_project(
    project_root: &Path,
    home: &Path,
    id: &str,
    targets: Vec<String>,
) -> anyhow::Result<memory_card::MemoryCardRecord> {
    let root = fsutil::normalize_project_root(project_root)?;
    memory_card::install_global_memory_card_to_project(&root, home, id, targets)
}

pub fn merge_memory_cards(
    project_root: &Path,
    input: MemoryCardMergeInput,
) -> anyhow::Result<()> {
    let root = fsutil::normalize_project_root(project_root)?;
    memory_card::merge_memory_cards(
        &root,
        &input.id,
        &input.title,
        input.sources,
        input.targets,
    )
}

pub fn attach_memory_card_to_skill(
    project_root: &Path,
    memory_card_id: &str,
    skill_id: &str,
) -> anyhow::Result<()> {
    let root = fsutil::normalize_project_root(project_root)?;
    config::add_skill_supplement(&root, skill_id, memory_card_id)
}

pub fn install_catalog_package(
    project_root: &Path,
    package_id: &str,
    targets: Vec<String>,
) -> anyhow::Result<catalog::CatalogPackage> {
    let root = fsutil::normalize_project_root(project_root)?;
    catalog::install_catalog_package(&root, package_id, targets)
}

pub fn import_project(project_root: &Path, scan_home: bool) -> anyhow::Result<scanner::ImportSummary> {
    let root = fsutil::normalize_project_root(project_root)?;
    scanner::import_project(&root, scan_home)
}

pub fn import_artifact_drifts(project_root: &Path) -> anyhow::Result<build::ArtifactImportReport> {
    let root = fsutil::normalize_project_root(project_root)?;
    build::import_artifact_drifts(&root)
}

pub fn import_artifact_drift_path(
    project_root: &Path,
    artifact_path: &str,
) -> anyhow::Result<build::ArtifactImportReport> {
    let root = fsutil::normalize_project_root(project_root)?;
    build::import_artifact_drift_path(&root, artifact_path)
}

pub fn keep_artifact_drifts(
    project_root: &Path,
) -> anyhow::Result<build::ArtifactDriftResolutionReport> {
    let root = fsutil::normalize_project_root(project_root)?;
    build::keep_artifact_drifts(&root)
}

pub fn keep_artifact_drift_path(
    project_root: &Path,
    artifact_path: &str,
) -> anyhow::Result<build::ArtifactDriftResolutionReport> {
    let root = fsutil::normalize_project_root(project_root)?;
    build::keep_artifact_drift_path(&root, artifact_path)
}

pub fn discard_artifact_drifts(
    project_root: &Path,
) -> anyhow::Result<build::ArtifactDriftResolutionReport> {
    let root = fsutil::normalize_project_root(project_root)?;
    build::discard_artifact_drifts(&root)
}

pub fn discard_artifact_drift_path(
    project_root: &Path,
    artifact_path: &str,
) -> anyhow::Result<build::ArtifactDriftResolutionReport> {
    let root = fsutil::normalize_project_root(project_root)?;
    build::discard_artifact_drift_path(&root, artifact_path)
}

pub fn build_preview(project_root: &Path) -> anyhow::Result<build::BuildReport> {
    let root = fsutil::normalize_project_root(project_root)?;
    build::build_project(&root, true)
}

#[cfg(test)]
pub fn compile_project(project_root: &Path) -> anyhow::Result<build::BuildReport> {
    let root = fsutil::normalize_project_root(project_root)?;
    build::sync_project(&root)
}

pub fn sync_project_importing_artifact_drifts(
    project_root: &Path,
) -> anyhow::Result<build::ArtifactImportReport> {
    let root = fsutil::normalize_project_root(project_root)?;
    let import = build::import_artifact_drifts(&root)?;
    build::sync_project(&root)?;
    Ok(import)
}

fn draft_update_from_input(input: DraftUpdateInput) -> draft::DraftUpdate {
    draft::DraftUpdate {
        title: input.title,
        body: input.body,
        brief: input.brief,
        tags: input.tags,
        language: input.language,
        kind: input.kind,
        scope: input.scope,
        targets: input.targets,
    }
}

fn candidate_update_from_input(input: CandidateUpdateInput) -> candidate::CandidateUpdate {
    candidate::CandidateUpdate {
        title: input.title,
        body: input.body,
        brief: input.brief,
        tags: input.tags,
        language: input.language,
        kind: input.kind,
        scope: input.scope,
        targets: input.targets,
    }
}

fn memory_card_update_from_input(input: MemoryCardUpdateInput) -> memory_card::MemoryCardUpdate {
    memory_card::MemoryCardUpdate {
        title: input.title,
        body: input.body,
        brief: input.brief,
        tags: input.tags,
        language: input.language,
        kind: input.kind,
        scope: input.scope,
    }
}

pub fn load_project_memory_card_library(
    project_root: &Path,
    home: &Path,
) -> anyhow::Result<ProjectMemoryCardLibrary> {
    let root = fsutil::normalize_project_root(project_root)?;
    Ok(ProjectMemoryCardLibrary {
        project_path: fsutil::path_to_slash(&root),
        memory_cards: memory_card::load_memory_cards(&root)?,
        global_memory_cards: memory_card::load_global_memory_cards(home)?,
        catalog_status: catalog::catalog_status(&root)?,
    })
}

pub fn load_project_assignment_view(project_root: &Path) -> anyhow::Result<ProjectAssignmentView> {
    let root = fsutil::normalize_project_root(project_root)?;
    let config = config::load_or_default_project_config(&root)?;
    let enabled_agents = config
        .agents
        .iter()
        .filter(|(_, agent)| agent.enabled)
        .map(|(name, _)| name.clone())
        .collect::<Vec<_>>();

    Ok(ProjectAssignmentView {
        project_path: fsutil::path_to_slash(&root),
        enabled_agents,
        target_matrix: memory_card::memory_card_target_matrix(&root)?,
    })
}

pub fn load_project_quality_view(project_root: &Path) -> anyhow::Result<ProjectQualityView> {
    let root = fsutil::normalize_project_root(project_root)?;
    Ok(ProjectQualityView {
        project_path: fsutil::path_to_slash(&root),
        rule_ci: rule_test::run_rule_tests(&root)?,
        build_preview: build::build_project(&root, true)?,
        status: build::status_project(&root)?,
    })
}

pub fn start_evolve_project_job(
    task_store: DesktopTaskStore,
    project_path: String,
    home: PathBuf,
    targets: Vec<String>,
    dry_run: bool,
    engine: String,
) -> DesktopJobStart {
    let job_id = task_store.begin_job(
        "整理历史",
        "发现会话文件",
        "正在查找 Claude Code / Codex 的 JSONL 会话与历史文件",
        12,
    );
    task_store.attach_replay(
        &job_id,
        DesktopJobReplay {
            command: "evolve_project".to_string(),
            args: serde_json::json!({
                "projectPath": project_path,
                "home": fsutil::path_to_slash(&home),
                "targets": targets,
                "dryRun": dry_run,
                "engine": engine,
                "confirmedPolicy": kernel::KernelPolicy::agent_managed()
            }),
        },
    );
    let start = crate::job_start_from_status(&task_store.snapshot(), "已加入后台整理队列");
    let background_store = task_store.clone();
    let background_engine = engine.clone();
    let background_job_id = job_id.clone();
    let background_project_path = project_path.clone();
    let background_targets = targets.clone();
    thread::spawn(move || {
        background_store.job_step(
            &background_job_id,
            "导入增量观察",
            "正在按 observation-index 跳过已处理内容，只读取新增对话片段",
            34,
        );
        if background_store.cancel_requested(&background_job_id) {
            background_store.finish_job(&background_job_id, "整理已取消，未继续读取历史会话");
            return;
        }
        background_store.job_step(
            &background_job_id,
            "整理提炼材料",
            "正在脱敏、过滤噪声，并拼接可用于 Memory Card 合成的高价值材料；低价值片段不会进入推理阶段",
            55,
        );
        if background_store.cancel_requested(&background_job_id) {
            background_store.finish_job(&background_job_id, "整理已取消，未进入推理阶段");
            return;
        }
        let engine_description = if background_engine == "local" {
            "本地极速模式：只运行规则过滤与候选生成，不启动 Claude Code / Codex"
        } else {
            "深度模式：先完成本地高价值过滤，再只把少量候选交给外部 Agent 精炼"
        };
        background_store.job_step(&background_job_id, "运行整理引擎", engine_description, 72);
        let result = observation::evolve_local_conversations_with_engine(
            Path::new(&background_project_path),
            &home,
            background_targets,
            dry_run,
            &background_engine,
        );
        match result {
            Ok(report) => {
                background_store.job_step(
                    &background_job_id,
                    "写入候选建议",
                    &format!(
                        "已导入 {} 条增量观察，写入 {} 条候选建议",
                        report.imported, report.draft_candidates
                    ),
                    92,
                );
                background_store.finish_job(
                    &background_job_id,
                    &format!(
                        "历史对话整理完成：导入 {} 条观察，生成 {} 条候选",
                        report.imported, report.draft_candidates
                    ),
                );
            }
            Err(error) => {
                background_store.finish_job(&background_job_id, &format!("整理失败：{error}"));
            }
        }
    });
    start
}

pub fn start_scan_projects_job(
    task_store: DesktopTaskStore,
    home: PathBuf,
    roots: Vec<PathBuf>,
    max_depth: usize,
) -> DesktopJobStart {
    let job_id = task_store.begin_job("扫描", "扫描本地项目", "正在准备扫描根目录", 10);
    task_store.attach_replay(
        &job_id,
        DesktopJobReplay {
            command: "scan_projects".to_string(),
            args: serde_json::json!({
                "home": fsutil::path_to_slash(&home),
                "roots": roots
                    .iter()
                    .map(|root| fsutil::path_to_slash(root.as_path()))
                    .collect::<Vec<_>>(),
                "maxDepth": max_depth,
                "confirmedPolicy": kernel::KernelPolicy::agent_managed()
            }),
        },
    );
    let start = crate::job_start_from_status(&task_store.snapshot(), "已加入后台扫描队列");
    thread::spawn(move || {
        task_store.job_step(
            &job_id,
            "识别 Agent 项目",
            "正在查找 AGENTS.md、CLAUDE.md、.claude/skills 和 .agents/skills",
            35,
        );
        if task_store.cancel_requested(&job_id) {
            task_store.finish_job(&job_id, "扫描已取消，未继续读取本地项目");
            return;
        }
        task_store.job_step(
            &job_id,
            "读取本地目录",
            &format!(
                "正在扫描 {} 个根目录，最大深度 {}；完成后会自动刷新项目列表",
                roots.len(),
                max_depth
            ),
            58,
        );
        let result = project_registry::scan_and_register(&home, &roots, max_depth);
        match result {
            Ok(report) => {
                task_store.job_step(
                    &job_id,
                    "更新项目索引",
                    &format!("已发现 {} 个项目，正在写入本地 registry", report.discovered.len()),
                    88,
                );
                task_store.finish_job(
                    &job_id,
                    &format!("本地项目扫描完成，发现 {} 个项目", report.discovered.len()),
                );
            }
            Err(error) => {
                task_store.finish_job(&job_id, &format!("扫描失败：{error}"));
            }
        }
    });
    start
}

pub fn start_sync_project_job(
    task_store: DesktopTaskStore,
    project_path: String,
) -> DesktopJobStart {
    let job_id = task_store.begin_job(
        "同步",
        "编译 Agent 产物",
        "正在将已批准 Memory Card 编译到 Claude Code / Codex",
        18,
    );
    task_store.attach_replay(
        &job_id,
        DesktopJobReplay {
            command: "sync_project".to_string(),
            args: serde_json::json!({
                "projectPath": project_path,
                "confirmedPolicy": kernel::KernelPolicy::agent_managed()
            }),
        },
    );
    let start = crate::job_start_from_status(&task_store.snapshot(), "已加入后台同步队列");
    let background_store = task_store.clone();
    let background_project_path = project_path.clone();
    thread::spawn(move || {
        background_store.job_step(
            &job_id,
            "生成编译预览",
            "正在检查 CLAUDE.md / AGENTS.md 漂移和目标智能体配置",
            42,
        );
        if background_store.cancel_requested(&job_id) {
            background_store.finish_job(&job_id, "同步已取消，未写入 Agent 产物");
            return;
        }
        background_store.job_step(
            &job_id,
            "导入漂移草稿",
            "正在把 CLAUDE.md / AGENTS.md 手工改动导入 Draft，避免覆盖",
            58,
        );
        if background_store.cancel_requested(&job_id) {
            background_store.finish_job(&job_id, "同步已取消，未导入漂移或写入 Agent 产物");
            return;
        }
        background_store.job_step(
            &job_id,
            "写入 Agent 产物",
            "正在全量编译并写入 Claude Code / Codex 目标文件",
            76,
        );
        match sync_project_importing_artifact_drifts(Path::new(&background_project_path)) {
            Ok(report) => {
                if report.created > 0 {
                    background_store.finish_job(
                        &job_id,
                        &format!("已导入 {} 条漂移草稿并同步生成产物", report.created),
                    );
                } else {
                    background_store.finish_job(&job_id, "已同步生成产物");
                }
            }
            Err(error) => background_store.finish_job(&job_id, &format!("同步失败：{error}")),
        }
    });
    start
}

pub fn start_fuse_memory_cards_to_draft_job(
    task_store: DesktopTaskStore,
    project_path: String,
    input: MemoryCardMergeInput,
) -> DesktopJobStart {
    let job_id = task_store.begin_job(
        "融合 Memory Card",
        "创建融合草稿",
        "正在把多个 Memory Card 合成为一条可审阅草稿",
        15,
    );
    task_store.attach_replay(
        &job_id,
        DesktopJobReplay {
            command: "fuse_memory_cards_to_draft".to_string(),
            args: serde_json::json!({
                "projectPath": project_path,
                "input": input,
                "confirmedPolicy": kernel::KernelPolicy::agent_managed()
            }),
        },
    );
    let start = crate::job_start_from_status(&task_store.snapshot(), "已加入后台融合队列");
    let background_store = task_store.clone();
    let background_project_path = project_path.clone();
    let background_input = input.clone();
    thread::spawn(move || {
        background_store.job_step(
            &job_id,
            "读取源 Memory Card",
            &format!(
                "正在读取 {} 个源 Memory Card 并准备融合上下文",
                background_input.sources.len()
            ),
            38,
        );
        if background_store.cancel_requested(&job_id) {
            background_store.finish_job(&job_id, "融合已取消，未生成草稿");
            return;
        }
        background_store.job_step(
            &job_id,
            "写入融合草稿",
            "正在生成可编辑、可拒绝、可批准的 Draft Memory Card",
            76,
        );
        let result = draft::fuse_memory_cards_to_draft(
            Path::new(&background_project_path),
            &background_input.id,
            &background_input.title,
            background_input.sources,
            background_input.targets,
        );
        match result {
            Ok(_) => background_store.finish_job(&job_id, "Memory Card 融合草稿已生成"),
            Err(error) => background_store.finish_job(&job_id, &format!("融合失败：{error}")),
        }
    });
    start
}

#[cfg(test)]
#[path = "app_service_tests.rs"]
mod tests;
