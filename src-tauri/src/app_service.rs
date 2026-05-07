use std::path::{Path, PathBuf};
use std::thread;

use agent_kernel::{
    build, candidate, catalog, config, draft, fsutil, kernel, observation, project_registry,
    rule_test, scanner, skilllet,
};
use serde::Serialize;

use crate::{
    DesktopJobReplay, DesktopJobStart, DesktopTaskStore, DraftMergeInput, DraftUpdateInput,
    SkillletMergeInput, SkillletUpdateInput,
};

#[derive(Debug, Clone, Serialize)]
pub struct ProjectDashboard {
    pub project_path: String,
    pub candidate_count: usize,
    pub draft_count: usize,
    pub skilllet_count: usize,
    pub observation_count: usize,
    pub global_skilllet_count: usize,
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
pub struct ProjectSkillletLibrary {
    pub project_path: String,
    pub skilllets: Vec<skilllet::SkillletRecord>,
    pub global_skilllets: Vec<skilllet::SkillletRecord>,
    pub catalog_status: catalog::CatalogStatus,
}

#[derive(Debug, Serialize)]
pub struct ProjectAssignmentView {
    pub project_path: String,
    pub enabled_agents: Vec<String>,
    pub target_matrix: skilllet::SkillletTargetMatrix,
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
        skilllet_count: skilllet::load_skilllets(&root)?.len(),
        observation_count: observation::count_observations(&root)?,
        global_skilllet_count: skilllet::count_global_skilllets(home)?,
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

pub fn approve_candidate_to_skilllet(
    project_root: &Path,
    id: &str,
) -> anyhow::Result<skilllet::SkillletRecord> {
    let root = fsutil::normalize_project_root(project_root)?;
    candidate::approve_candidate_to_skilllet(&root, id)
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

pub fn set_skilllet_targets(
    project_root: &Path,
    id: &str,
    targets: Vec<String>,
) -> anyhow::Result<()> {
    let root = fsutil::normalize_project_root(project_root)?;
    skilllet::set_skilllet_targets(&root, id, targets)
}

pub fn update_skilllet(
    project_root: &Path,
    id: &str,
    input: SkillletUpdateInput,
) -> anyhow::Result<skilllet::SkillletRecord> {
    let root = fsutil::normalize_project_root(project_root)?;
    skilllet::update_skilllet(&root, id, skilllet_update_from_input(input))
}

pub fn promote_skilllet_to_global(
    project_root: &Path,
    home: &Path,
    id: &str,
) -> anyhow::Result<skilllet::SkillletRecord> {
    let root = fsutil::normalize_project_root(project_root)?;
    skilllet::promote_skilllet_to_global(&root, home, id)
}

pub fn install_global_skilllet_to_project(
    project_root: &Path,
    home: &Path,
    id: &str,
    targets: Vec<String>,
) -> anyhow::Result<skilllet::SkillletRecord> {
    let root = fsutil::normalize_project_root(project_root)?;
    skilllet::install_global_skilllet_to_project(&root, home, id, targets)
}

pub fn merge_skilllets(
    project_root: &Path,
    input: SkillletMergeInput,
) -> anyhow::Result<()> {
    let root = fsutil::normalize_project_root(project_root)?;
    skilllet::merge_skilllets(
        &root,
        &input.id,
        &input.title,
        input.sources,
        input.targets,
    )
}

pub fn attach_skilllet_to_skill(
    project_root: &Path,
    skilllet_id: &str,
    skill_id: &str,
) -> anyhow::Result<()> {
    let root = fsutil::normalize_project_root(project_root)?;
    config::add_skill_supplement(&root, skill_id, skilllet_id)
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

pub fn build_preview(project_root: &Path) -> anyhow::Result<build::BuildReport> {
    let root = fsutil::normalize_project_root(project_root)?;
    build::build_project(&root, true)
}

#[cfg(test)]
pub fn compile_project(project_root: &Path) -> anyhow::Result<build::BuildReport> {
    let root = fsutil::normalize_project_root(project_root)?;
    build::sync_project(&root)
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

fn skilllet_update_from_input(input: SkillletUpdateInput) -> skilllet::SkillletUpdate {
    skilllet::SkillletUpdate {
        title: input.title,
        body: input.body,
        brief: input.brief,
        tags: input.tags,
        language: input.language,
        kind: input.kind,
        scope: input.scope,
    }
}

pub fn load_project_skilllet_library(
    project_root: &Path,
    home: &Path,
) -> anyhow::Result<ProjectSkillletLibrary> {
    let root = fsutil::normalize_project_root(project_root)?;
    Ok(ProjectSkillletLibrary {
        project_path: fsutil::path_to_slash(&root),
        skilllets: skilllet::load_skilllets(&root)?,
        global_skilllets: skilllet::load_global_skilllets(home)?,
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
        target_matrix: skilllet::skilllet_target_matrix(&root)?,
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
            "正在脱敏、过滤噪声，并拼接可用于 Skilllet 合成的高价值材料；低价值片段不会进入推理阶段",
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
        "正在将已批准 Skilllet 编译到 Claude Code / Codex",
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
            "写入 Agent 产物",
            "正在全量编译并写入 Claude Code / Codex 目标文件",
            76,
        );
        match build::sync_project(Path::new(&background_project_path)) {
            Ok(_) => background_store.finish_job(&job_id, "已同步生成产物"),
            Err(error) => background_store.finish_job(&job_id, &format!("同步失败：{error}")),
        }
    });
    start
}

pub fn start_fuse_skilllets_to_draft_job(
    task_store: DesktopTaskStore,
    project_path: String,
    input: SkillletMergeInput,
) -> DesktopJobStart {
    let job_id = task_store.begin_job(
        "融合Skilllet",
        "创建融合草稿",
        "正在把多个 Skilllet 合成为一条可审阅草稿",
        15,
    );
    task_store.attach_replay(
        &job_id,
        DesktopJobReplay {
            command: "fuse_skilllets_to_draft".to_string(),
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
            "读取源 Skilllet",
            &format!(
                "正在读取 {} 个源 Skilllet 并准备融合上下文",
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
            "正在生成可编辑、可拒绝、可批准的 Draft Skilllet",
            76,
        );
        let result = draft::fuse_skilllets_to_draft(
            Path::new(&background_project_path),
            &background_input.id,
            &background_input.title,
            background_input.sources,
            background_input.targets,
        );
        match result {
            Ok(_) => background_store.finish_job(&job_id, "Skilllet 融合草稿已生成"),
            Err(error) => background_store.finish_job(&job_id, &format!("融合失败：{error}")),
        }
    });
    start
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed_draft(project_root: &Path, id: &str) {
        draft::add_draft(
            project_root,
            draft::NewDraft {
                id: id.to_string(),
                title: "Prefer Bun".to_string(),
                body: "Use Bun for JavaScript package management and scripts.".to_string(),
                kind: "preference".to_string(),
                scope: "project".to_string(),
                targets: vec!["codex".to_string()],
                evidence: "service test".to_string(),
                confidence: Some(0.9),
                reason: Some("Repeated correction".to_string()),
                matched_template: None,
                extraction: agent_kernel::candidate::ExtractionMetadata::default(),
            },
        )
        .expect("seed draft");
    }

    fn seed_skilllet(project_root: &Path, id: &str) {
        skilllet::add_skilllet(
            project_root,
            id,
            "Use Axios",
            "Use Axios for frontend HTTP requests.",
            "preference",
            "project",
            vec!["codex".to_string()],
        )
        .expect("seed skilllet");
    }

    #[test]
    fn approve_draft_promotes_to_skilllet_and_removes_from_inbox() {
        let temp = tempfile::tempdir().expect("tempdir");
        seed_draft(temp.path(), "project:prefer-bun");

        approve_draft(temp.path(), "project:prefer-bun").expect("approve draft");
        let skilllets = skilllet::load_skilllets(temp.path()).expect("skilllets");

        assert_eq!(skilllets[0].id, "project:prefer-bun");
        assert_eq!(load_project_review_inbox(temp.path()).expect("inbox").drafts.len(), 0);
        assert_eq!(skilllets.len(), 1);
    }

    #[test]
    fn reject_draft_removes_draft_without_creating_skilllet() {
        let temp = tempfile::tempdir().expect("tempdir");
        seed_draft(temp.path(), "project:reject-me");

        reject_draft(temp.path(), "project:reject-me").expect("reject draft");

        assert_eq!(load_project_review_inbox(temp.path()).expect("inbox").drafts.len(), 0);
        assert_eq!(skilllet::load_skilllets(temp.path()).expect("skilllets").len(), 0);
    }

    #[test]
    fn update_draft_maps_editor_input_and_persists_fields() {
        let temp = tempfile::tempdir().expect("tempdir");
        seed_draft(temp.path(), "project:editable");

        let updated = update_draft(
            temp.path(),
            "project:editable",
            DraftUpdateInput {
                title: Some("Prefer Bun".to_string()),
                body: Some("Use Bun for JavaScript package management.".to_string()),
                brief: Some("Use Bun".to_string()),
                tags: Some(vec!["tooling".to_string()]),
                language: Some("en".to_string()),
                kind: Some("preference".to_string()),
                scope: Some("project".to_string()),
                targets: Some(Vec::new()),
            },
        )
        .expect("update draft");

        assert_eq!(updated.title, "Prefer Bun");
        assert_eq!(updated.brief, "Use Bun");
        assert_eq!(updated.tags, vec!["tooling"]);
        assert_eq!(updated.targets, Vec::<String>::new());
    }

    #[test]
    fn merge_drafts_creates_combined_draft_and_removes_sources() {
        let temp = tempfile::tempdir().expect("tempdir");
        seed_draft(temp.path(), "project:first");
        seed_draft(temp.path(), "project:second");

        merge_drafts(
            temp.path(),
            DraftMergeInput {
                id: "project:merged".to_string(),
                title: "Merged".to_string(),
                sources: vec!["project:first".to_string(), "project:second".to_string()],
                targets: vec!["codex".to_string()],
            },
        )
        .expect("merge drafts");

        let drafts = draft::load_drafts(temp.path()).expect("drafts");
        assert_eq!(drafts.len(), 1);
        let merged = drafts
            .iter()
            .find(|draft| draft.id == "project:merged")
            .expect("merged draft");
        assert!(merged.body.contains("Use Bun"));
        assert!(!drafts.iter().any(|draft| draft.id == "project:first"));
        assert!(!drafts.iter().any(|draft| draft.id == "project:second"));
    }

    #[test]
    fn assignment_services_update_agent_and_skilllet_targets() {
        let temp = tempfile::tempdir().expect("tempdir");
        seed_skilllet(temp.path(), "project:use-axios");

        set_agent_enabled(temp.path(), "claude-code", false).expect("disable agent");
        set_skilllet_targets(
            temp.path(),
            "project:use-axios",
            vec!["claude-code".to_string()],
        )
        .expect("set targets");

        let assignment = load_project_assignment_view(temp.path()).expect("assignment");
        assert!(!assignment.enabled_agents.contains(&"claude-code".to_string()));
        let row = assignment
            .target_matrix
            .rows
            .iter()
            .find(|row| row.skilllet_id == "project:use-axios")
            .expect("skilllet row");
        assert_eq!(row.targets.get("claude-code"), Some(&true));
    }

    #[test]
    fn update_skilllet_maps_editor_input_and_persists_fields() {
        let temp = tempfile::tempdir().expect("tempdir");
        seed_skilllet(temp.path(), "project:editable");

        let updated = update_skilllet(
            temp.path(),
            "project:editable",
            SkillletUpdateInput {
                title: Some("Use Axios Everywhere".to_string()),
                body: Some("Use Axios for every frontend HTTP request.".to_string()),
                brief: Some("Axios standard".to_string()),
                tags: Some(vec!["frontend".to_string()]),
                language: Some("en".to_string()),
                kind: Some("preference".to_string()),
                scope: Some("project".to_string()),
            },
        )
        .expect("update skilllet");

        assert_eq!(updated.title, "Use Axios Everywhere");
        assert_eq!(updated.brief, "Axios standard");
        assert_eq!(updated.tags, vec!["frontend"]);
    }

    #[test]
    fn global_skilllet_services_promote_and_install_with_targets() {
        let source = tempfile::tempdir().expect("source");
        let target = tempfile::tempdir().expect("target");
        let home = tempfile::tempdir().expect("home");
        seed_skilllet(source.path(), "project:review-before-sync");

        let global = promote_skilllet_to_global(source.path(), home.path(), "project:review-before-sync")
            .expect("promote global");
        let installed = install_global_skilllet_to_project(
            target.path(),
            home.path(),
            &global.id,
            vec!["codex".to_string()],
        )
        .expect("install global");

        assert_eq!(global.id, "global:review-before-sync");
        assert_eq!(installed.id, "global:review-before-sync");
        assert_eq!(skilllet::load_skilllets(target.path()).expect("skilllets").len(), 1);
        let config = config::load_or_default_project_config(target.path()).expect("config");
        assert_eq!(config.skilllets.include[0].targets, vec!["codex"]);
    }

    #[test]
    fn merge_skilllets_service_creates_combined_skilllet() {
        let temp = tempfile::tempdir().expect("tempdir");
        seed_skilllet(temp.path(), "project:ui-a");
        seed_skilllet(temp.path(), "project:ui-b");

        merge_skilllets(
            temp.path(),
            SkillletMergeInput {
                id: "project:ui-merged".to_string(),
                title: "Merged UI".to_string(),
                sources: vec!["project:ui-a".to_string(), "project:ui-b".to_string()],
                targets: vec!["codex".to_string()],
            },
        )
        .expect("merge skilllets");

        let merged = skilllet::load_skilllets(temp.path())
            .expect("skilllets")
            .into_iter()
            .find(|record| record.id == "project:ui-merged")
            .expect("merged skilllet");
        assert_eq!(merged.id, "project:ui-merged");
        assert!(merged.body.contains("Use Axios"));
    }

    #[test]
    fn attach_skilllet_service_updates_skill_supplements() {
        let temp = tempfile::tempdir().expect("tempdir");
        seed_skilllet(temp.path(), "project:use-axios");
        config::save_skill_index(
            temp.path(),
            &config::SkillIndex {
                generated_at: "test".to_string(),
                skills: vec![config::SkillRecord {
                    id: "skill:frontend".to_string(),
                    name: "Frontend".to_string(),
                    description: "Frontend workflow".to_string(),
                    source_path: ".agents/skills/frontend".to_string(),
                    source_kind: "project".to_string(),
                    source_hash: "hash".to_string(),
                    warnings: Vec::new(),
                }],
            },
        )
        .expect("skill index");

        attach_skilllet_to_skill(temp.path(), "project:use-axios", "skill:frontend")
            .expect("attach skilllet");

        let config = config::load_or_default_project_config(temp.path()).expect("config");
        assert_eq!(config.skills.supplements[0].skill, "skill:frontend");
        assert_eq!(config.skills.supplements[0].skilllets[0], "project:use-axios");
    }

    #[test]
    fn install_catalog_package_service_installs_skilllet_with_targets() {
        let temp = tempfile::tempdir().expect("tempdir");

        let package = install_catalog_package(
            temp.path(),
            "core:rust-quality-gate",
            vec!["codex".to_string()],
        )
        .expect("install catalog package");

        assert_eq!(package.id, "core:rust-quality-gate");
        let skilllets = skilllet::load_skilllets(temp.path()).expect("skilllets");
        assert_eq!(skilllets[0].id, "core:rust-quality-gate");
        let config = config::load_or_default_project_config(temp.path()).expect("config");
        assert_eq!(config.skilllets.include[0].targets, vec!["codex"]);
    }

    #[test]
    fn sync_job_service_records_replayable_background_job_ticket() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = crate::DesktopTaskStore::default();

        let start = start_sync_project_job(store.clone(), fsutil::path_to_slash(temp.path()));

        let status = store.snapshot();
        let replay = status.replay.expect("replay payload");
        assert!(start.accepted);
        assert_eq!(start.key, "同步");
        assert_eq!(status.job_id, start.job_id);
        assert_eq!(replay.command, "sync_project");
        assert_eq!(replay.args["projectPath"], fsutil::path_to_slash(temp.path()));
    }

    #[test]
    fn evolve_job_service_records_engine_targets_and_home_in_replay() {
        let temp = tempfile::tempdir().expect("tempdir");
        let home = tempfile::tempdir().expect("home");
        let store = crate::DesktopTaskStore::default();

        let start = start_evolve_project_job(
            store.clone(),
            fsutil::path_to_slash(temp.path()),
            home.path().to_path_buf(),
            vec!["codex".to_string()],
            true,
            "local".to_string(),
        );

        let status = store.snapshot();
        let replay = status.replay.expect("replay payload");
        assert!(start.accepted);
        assert_eq!(start.key, "整理历史");
        assert_eq!(status.job_id, start.job_id);
        assert_eq!(replay.command, "evolve_project");
        assert_eq!(replay.args["home"], fsutil::path_to_slash(home.path()));
        assert_eq!(replay.args["targets"][0], "codex");
        assert_eq!(replay.args["dryRun"], true);
        assert_eq!(replay.args["engine"], "local");
    }

    #[test]
    fn scan_job_service_records_roots_depth_and_home_in_replay() {
        let root = tempfile::tempdir().expect("root");
        let home = tempfile::tempdir().expect("home");
        let store = crate::DesktopTaskStore::default();

        let start = start_scan_projects_job(
            store.clone(),
            home.path().to_path_buf(),
            vec![root.path().to_path_buf()],
            3,
        );

        let status = store.snapshot();
        let replay = status.replay.expect("replay payload");
        assert!(start.accepted);
        assert_eq!(start.key, "扫描");
        assert_eq!(status.job_id, start.job_id);
        assert_eq!(replay.command, "scan_projects");
        assert_eq!(replay.args["home"], fsutil::path_to_slash(home.path()));
        assert_eq!(replay.args["roots"][0], fsutil::path_to_slash(root.path()));
        assert_eq!(replay.args["maxDepth"], 3);
    }

    #[test]
    fn fuse_job_service_records_precise_input_in_replay() {
        let project = tempfile::tempdir().expect("project");
        let store = crate::DesktopTaskStore::default();
        let input = SkillletMergeInput {
            id: "draft:fused-ui".to_string(),
            title: "融合 UI 规则".to_string(),
            sources: vec!["project:ui-a".to_string(), "project:ui-b".to_string()],
            targets: vec!["codex".to_string()],
        };

        let start = start_fuse_skilllets_to_draft_job(
            store.clone(),
            fsutil::path_to_slash(project.path()),
            input,
        );

        let status = store.snapshot();
        let replay = status.replay.expect("replay payload");
        assert!(start.accepted);
        assert_eq!(start.key, "融合Skilllet");
        assert_eq!(status.job_id, start.job_id);
        assert_eq!(replay.command, "fuse_skilllets_to_draft");
        assert_eq!(replay.args["input"]["id"], "draft:fused-ui");
        assert_eq!(replay.args["input"]["sources"][1], "project:ui-b");
        assert_eq!(replay.args["input"]["targets"][0], "codex");
    }

    #[test]
    fn import_project_service_initializes_project_metadata() {
        let temp = tempfile::tempdir().expect("tempdir");

        import_project(temp.path(), false).expect("import project");

        let config = config::load_or_default_project_config(temp.path()).expect("config");
        assert_eq!(config.version, 1);
    }

    #[test]
    fn quality_mutation_services_return_build_reports() {
        let temp = tempfile::tempdir().expect("tempdir");

        let preview = build_preview(temp.path()).expect("build preview");
        import_artifact_drifts(temp.path()).expect("import drifts");
        compile_project(temp.path()).expect("compile project");

        assert!(preview.render().contains("AGENTS.md"));
    }
}
