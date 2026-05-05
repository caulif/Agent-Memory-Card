import React from "react";
import { createRoot } from "react-dom/client";
import {
  planKernelCommand,
  scanProjectsJob,
  updateDraft,
  updateSkilllet,
} from "./tauri-client";
import { useJobCenter } from "./hooks/useJobCenter";
import { useProjectActions } from "./hooks/useProjectActions";
import { useProjectReadModels } from "./hooks/useProjectReadModels";
import { useProjectSelection } from "./hooks/useProjectSelection";
import { JobCenter } from "./components/JobCenter";
import { Agents, Catalog, Drafts, ProjectOverviewStrip, Settings, Skilllets } from "./components/project-pages";
import { ActionButton, EmptyState, Panel, StatusList, TaskProgressBar } from "./components/common";
import {
  AppWindow,
  ArrowUp,
  Bell,
  Check,
  ChevronRight,
  Circle,
  CloudCog,
  GitBranch,
  Layers3,
  Loader2,
  PackagePlus,
  Pencil,
  Plus,
  RefreshCw,
  ScanLine,
  ShieldAlert,
  ShieldCheck,
  Store,
  X,
} from "lucide-react";
import "./styles.css";
import "./styles/jobs-progress.css";
import "./styles/project-detail.css";
import "./styles/themes.css";
import {
  buildEditFormFromDraft,
  buildEditFormFromSkilllet,
  buildKernelPlanForEditor,
  confirmedAgentManagedPolicy,
  createDemoProjectSnapshot,
  deriveSkillletEvolution,
  describeDraftForReview,
  describeSkillletPlainly,
  EDITABLE_AGENTS,
  filterRecordsByTag,
  formatJobLifecycle,
  formatJobStartMessage,
  formatAgent,
  formatAgents,
  filterCandidatesForInbox,
  getPreviewActionMessage,
  getPreviewPlanResult,
  type CandidateFilter,
  KIND_OPTIONS,
  manualReviewPolicy,
  normalizePlanReviewResult,
  nextSkillletTargets,
  PACKAGE_MANAGER_EXPLANATION,
  PAGES,
  projectOverviewMetrics,
  parseCommaTags,
  resolveTaskProgress,
  SCOPE_OPTIONS,
  sortCandidatesForInbox,
  taskProgressForAction,
  translateKind,
  translateScope,
  type DesktopAppState,
  type DesktopTaskStatus,
  type CandidateRecord,
  type EditFormData,
  type EvolutionInsight,
  type PageId,
  type PlanReviewResult,
  type ProjectDashboard,
  type ProjectCandidateInbox,
  type ProjectReviewInbox,
  type ProjectAssignmentView,
  type ProjectSkillletLibrary,
  type ProjectSnapshot,
} from "./ui-helpers";

const tauriRuntimeHint = "未连接到 Tauri 运行时。请在项目根目录使用 bun run app:dev 启动桌面应用。";

function App() {
  const [page, setPage] = React.useState<PageId>("drafts");
  const [pendingAction, setPendingAction] = React.useState("");
  const [message, setMessage] = React.useState("就绪。选择项目后可手动提炼历史对话。");
  const [previewMode, setPreviewMode] = React.useState(false);
  const [synthesisEngine, setSynthesisEngine] = React.useState<"claude-code" | "codex" | "local">("local");
  const [theme, setTheme] = React.useState<"light" | "dark" | "system">("system");
  const selectedProjectRef = React.useRef("");
  const readModels = useProjectReadModels({ page, previewMode, selectedProjectRef, setMessage });
  const {
    dashboard,
    setDashboard,
    candidateInbox,
    setCandidateInbox,
    reviewInbox,
    setReviewInbox,
    skillletLibrary,
    assignmentView,
    qualityView,
    clearProjectReadModels,
    loadDashboard,
    loadReadModelsForPage,
  } = readModels;

  const activateProject = React.useCallback((projectPath: string, targetPage: PageId) => {
    setDashboard(null);
    clearProjectReadModels();
    setMessage("项目已打开，正在后台加载工作台数据。");
    window.setTimeout(() => {
      void loadDashboard(projectPath);
      void loadReadModelsForPage(projectPath, targetPage);
    }, 0);
  }, [clearProjectReadModels, loadDashboard, loadReadModelsForPage, setDashboard, setMessage]);

  const clearProject = React.useCallback(() => {
    setDashboard(null);
    clearProjectReadModels();
  }, [clearProjectReadModels, setDashboard]);

  const selection = useProjectSelection({
    page,
    previewMode,
    setPreviewMode,
    selectedProjectRef,
    setMessage,
    onProjectActivated: activateProject,
    onProjectCleared: clearProject,
  });
  const { state, selectedProject, appStateError, enterPreviewMode, refreshAppState, reloadAppStateFromBackend, chooseProject } = selection;

  const actions = useProjectActions({
    selectedProject,
    selectedProjectRef,
    page,
    previewMode,
    setMessage,
    setPendingAction,
    setCandidateInbox,
    setReviewInbox,
    setDashboard,
    loadDashboard,
    loadReadModelsForPage,
  });
  const { runTask, projectAction, batchCandidateAction } = actions;

  const { backendTaskStatus, jobHistory, jobCenterOpen, setJobCenterOpen, cancelJob, retryJob } = useJobCenter({
    previewMode,
    selectedProjectRef,
    page,
    setMessage,
    reloadAppStateFromBackend,
    loadDashboard,
    loadReadModelsForPage,
  });

  React.useEffect(() => {
    void refreshAppState()
      .then((nextMessage) => setMessage(nextMessage))
      .catch((error) => setMessage(formatErrorMessage(error)));
  }, [refreshAppState, setMessage]);

  // 系统主题监听
  React.useEffect(() => {
    if (theme !== "system") return;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => document.documentElement.setAttribute("data-theme", mq.matches ? "dark" : "light");
    apply();
    mq.addEventListener("change", apply);
    return () => mq.removeEventListener("change", apply);
  }, [theme]);

  // 手动主题切换
  React.useEffect(() => {
    if (theme === "system") return;
    document.documentElement.setAttribute("data-theme", theme);
  }, [theme]);

  React.useEffect(() => {
    if (!selectedProject) return;
    void loadReadModelsForPage(selectedProject, page);
  }, [page, selectedProject, previewMode, loadReadModelsForPage]);

  const projects = state?.registry.projects ?? [];
  const activeProject = projects.find((project) => project.path === selectedProject);
  const snapshot = React.useMemo(
    () => (previewMode && selectedProject ? createDemoProjectSnapshot(selectedProject) : null),
    [previewMode, selectedProject],
  );
  const installedCount =
    skillletLibrary?.catalog_status.items.filter((item) => item.installed).length ??
    snapshot?.catalog_status.items.filter((item) => item.installed).length ??
    0;

  async function scanProjects() {
    if (previewMode) {
      enterPreviewMode(selectedProject);
      setMessage(getPreviewActionMessage("扫描"));
      return;
    }

    await runTask("扫描", "本地项目扫描完成", async () => {
      const job = await scanProjectsJob();
      return formatJobStartMessage(job);
    });
  }

  return (
    <main className="shell">
      <aside className="sidebar" aria-label="本地项目">
        <div className="traffic-lights" aria-hidden="true">
          <span className="close" />
          <span className="minimize" />
          <span className="zoom" />
        </div>

        <div className="brand">
          <div className="brand-mark">
            <AppWindow size={18} />
          </div>
          <div>
            <strong>智能体内核</strong>
            <span>本地技能工作台</span>
          </div>
        </div>

        <ActionButton
          className="primary-action"
          icon={ScanLine}
          label="扫描本地项目"
          busyLabel="正在扫描"
          busy={pendingAction === "扫描"}
          disabled={pendingAction === "扫描"}
          onClick={scanProjects}
        />

        <section className="project-list">
          <div className="section-label">本地项目</div>
          {projects.length === 0 ? (
            <p className="empty">还没有发现项目。扫描后会读取常用目录和当前工作区附近的项目。</p>
          ) : (
            projects.map((project) => (
              <button
                key={project.path}
                className={`project-item ${project.path === selectedProject ? "active" : ""}`}
                onClick={() => void chooseProject(project.path)}
              >
                <span>{project.name}</span>
                <small>{formatAgents(project.agents)}</small>
              </button>
            ))
          )}
        </section>
      </aside>

      <section className="workspace" aria-label="工作区">
        {previewMode ? (
          <div className="preview-banner" role="status">
            <ShieldCheck size={16} />
            <div>
              <strong>浏览器演示模式</strong>
              <span>当前使用静态示例数据，可自由切换页面和点击按钮。真实扫描、批准、同步和文件写入需要在项目根目录运行 bun run app:dev 启动 Tauri。</span>
            </div>
          </div>
        ) : null}

        <header className="topbar">
          <div className="title-block">
            <p className="eyebrow">本地优先 · 人工审核 · 多智能体同步</p>
            <h1>{activeProject?.name ?? "选择一个项目开始"}</h1>
            <span className="path">{activeProject?.path ?? state?.home ?? "正在读取本机目录"}</span>
          </div>
          <div className="topbar-actions">
            <TaskProgressBar pendingAction={pendingAction} message={message} backendStatus={backendTaskStatus} />
            <button className="job-center-trigger" onClick={() => setJobCenterOpen(true)}>
              任务中心
              <span>{jobHistory.length}</span>
            </button>
          </div>
        </header>

        <nav className="tabs" aria-label="功能切换">
          {PAGES.map((item) => {
            const Icon = item.icon;
            return (
              <button key={item.id} className={page === item.id ? "active" : ""} onClick={() => setPage(item.id)}>
                <Icon size={15} />
                {item.label}
              </button>
            );
          })}
        </nav>

        <ProjectOverviewStrip snapshot={snapshot} dashboard={dashboard} installedCount={installedCount} />

        <section className="content">
          <div className="workspace-layout">
            <section className="primary-pane">
              {appStateError ? (
                <EmptyState title="无法读取桌面运行时" description={appStateError} />
              ) : (
                <>
                  {page === "drafts" && (
                    <Drafts
                      snapshot={snapshot}
                      candidates={candidateInbox}
                      inbox={reviewInbox}
                      pendingAction={pendingAction}
                      disabled={!candidateInbox && !reviewInbox && !snapshot}
                      onAction={projectAction}
                      onBatchCandidateAction={batchCandidateAction}
                      previewMode={previewMode}
                      projectPath={selectedProject}
                      synthesisEngine={synthesisEngine}
                      onSynthesisEngineChange={setSynthesisEngine}
                      onRefresh={() => {
                        void loadReadModelsForPage(selectedProject, "drafts");
                      }}
                    />
                  )}
                  {page === "skilllets" && (
                    <Skilllets
                      snapshot={snapshot}
                      library={skillletLibrary}
                      pendingAction={pendingAction}
                      disabled={!skillletLibrary && !snapshot}
                      onAction={projectAction}
                      previewMode={previewMode}
                      projectPath={selectedProject}
                      onRefresh={() => {
                        void loadReadModelsForPage(selectedProject, "skilllets");
                      }}
                    />
                  )}
                  {page === "agents" && (
                    <Agents
                      snapshot={snapshot}
                      assignment={assignmentView}
                      library={skillletLibrary}
                      pendingAction={pendingAction}
                      disabled={!assignmentView && !snapshot}
                      onAction={projectAction}
                    />
                  )}
                  {page === "catalog" && (
                    <Catalog
                      snapshot={snapshot}
                      library={skillletLibrary}
                      pendingAction={pendingAction}
                      disabled={!skillletLibrary && !snapshot}
                      onAction={projectAction}
                    />
                  )}
                  {page === "settings" && <Settings state={state} snapshot={snapshot} synthesisEngine={synthesisEngine} theme={theme} onThemeChange={setTheme} />}
                </>
              )}
            </section>

            <aside className="inspector" aria-label="项目辅助信息">
              <Panel title="质量状态" icon={ShieldCheck}>
                <div className="quality">
                  <span>通过 {qualityView?.rule_ci.passed ?? snapshot?.rule_ci.passed ?? 0}</span>
                  <span>失败 {qualityView?.rule_ci.failed ?? snapshot?.rule_ci.failed ?? 0}</span>
                </div>
                <StatusList
                  empty="暂无质量警告。"
                  items={[
                    ...(qualityView?.status.warnings ?? snapshot?.status.warnings ?? []),
                    ...(qualityView?.build_preview.warnings ?? snapshot?.build_preview.warnings ?? []),
                  ]}
                  tone="warning"
                />
              </Panel>

              <button className="quick-store" onClick={() => setPage("catalog")}>
                <PackagePlus size={16} />
                打开包管理器
                <ChevronRight size={15} />
              </button>
            </aside>
          </div>
        </section>
      </section>
      {jobCenterOpen ? (
        <JobCenter
          jobs={jobHistory}
          currentJob={backendTaskStatus}
          onCancel={cancelJob}
          onRetry={retryJob}
          onClose={() => setJobCenterOpen(false)}
        />
      ) : null}
    </main>
  );
}

function formatErrorMessage(error: unknown) {
  const rawMessage = error instanceof Error ? error.message : String(error);
  const normalized = rawMessage.toLowerCase();

  if (normalized.includes("__tauri") || normalized.includes("tauri") || normalized.includes("get_app_state")) {
    return tauriRuntimeHint;
  }

  return rawMessage || tauriRuntimeHint;
}

createRoot(document.getElementById("root")!).render(<App />);
