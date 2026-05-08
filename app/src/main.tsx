import React from "react";
import { createRoot } from "react-dom/client";
import {
  planKernelCommand,
  scanProjectsJob,
  updateDraft,
  updateMemoryCard,
} from "./tauri-client";
import { useJobCenter } from "./hooks/useJobCenter";
import { useProjectActions } from "./hooks/useProjectActions";
import { useProjectReadModels } from "./hooks/useProjectReadModels";
import { useProjectSelection } from "./hooks/useProjectSelection";
import { JobCenter } from "./components/JobCenter";
import { Agents, Drafts, ProjectOverviewStrip, Settings, MemoryCards } from "./components/project-pages";
import { ActionButton, EmptyState, Panel, StatusList, TaskProgressBar } from "./components/common";
import {
  AppWindow,
  ArrowUp,
  Bell,
  Check,
  ChevronDown,
  ChevronRight,
  Circle,
  CloudCog,
  GitBranch,
  Layers3,
  Loader2,
  Pencil,
  Plus,
  RefreshCw,
  ScanLine,
  ShieldAlert,
  ShieldCheck,
  X,
  Boxes,
  HardDrive,
  Activity,
} from "lucide-react";
import "./styles.css";
import "./styles/jobs-progress.css";
import "./styles/project-detail.css";
import "./styles/themes.css";
import {
  buildEditFormFromDraft,
  buildEditFormFromMemoryCard,
  buildKernelPlanForEditor,
  confirmedAgentManagedPolicy,
  createDemoProjectSnapshot,
  deriveMemoryCardEvolution,
  describeDraftForReview,
  describeMemoryCardPlainly,
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
  nextMemoryCardTargets,
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
  type ProjectMemoryCardLibrary,
  type ProjectSnapshot,
} from "./ui-helpers";

const tauriRuntimeHint = "未连接到 Tauri 运行时。请在项目根目录使用 bun run app:dev 启动桌面应用。";

/** 侧边栏导航使用的英文页面标签（大视图标题用） */
const PAGE_DISPLAY: Record<PageId, { label: string; enLabel: string }> = {
  drafts: { label: "审阅", enLabel: "Draft Review" },
  "memory-cards": { label: "记忆卡", enLabel: "Memory Card Library" },
  agents: { label: "分配", enLabel: "Memory Card Loadout" },
  settings: { label: "设置", enLabel: "Settings" },
};

function App() {
  const [page, setPage] = React.useState<PageId>("drafts");
  const [pendingAction, setPendingAction] = React.useState("");
  const [message, setMessage] = React.useState("就绪。选择项目后可手动提炼历史对话。");
  const [previewMode, setPreviewMode] = React.useState(false);
  const [synthesisEngine, setSynthesisEngine] = React.useState<"claude-code" | "codex" | "local" | "llm">("claude-code");
  const [theme, setTheme] = React.useState<"light" | "dark" | "system">("system");
  const [projectMenuOpen, setProjectMenuOpen] = React.useState(false);
  const selectedProjectRef = React.useRef("");
  const readModels = useProjectReadModels({ page, previewMode, selectedProjectRef, setMessage });
  const {
    dashboard,
    setDashboard,
    candidateInbox,
    setCandidateInbox,
    reviewInbox,
    setReviewInbox,
    memory_cardLibrary,
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

  /** 系统主题监听 */
  React.useEffect(() => {
    if (theme !== "system") return;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => document.documentElement.setAttribute("data-theme", mq.matches ? "dark" : "light");
    apply();
    mq.addEventListener("change", apply);
    return () => mq.removeEventListener("change", apply);
  }, [theme]);

  /** 手动主题切换 */
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
    memory_cardLibrary?.catalog_status.items.filter((item) => item.installed).length ??
    snapshot?.catalog_status.items.filter((item) => item.installed).length ??
    0;
  const actionableJobCount = jobHistory.filter((job) => job.running || job.lifecycle === "failed").length;
  const memory_cardCount = memory_cardLibrary?.memory_cards.length ?? snapshot?.memory_cards.length ?? 0;
  const observationCount = snapshot?.observations.length ?? 0;

  /** 分配影响力数据 — 用于 Agents 页面右侧检查器 */
  const agentsMatrix = assignmentView?.target_matrix ?? snapshot?.target_matrix;
  const agentRows = agentsMatrix?.rows ?? [];
  const agentList = agentsMatrix?.agents ?? [];
  const allMemoryCardsForAssignment = React.useMemo(() => {
    const projectItems = memory_cardLibrary?.memory_cards ?? snapshot?.memory_cards ?? [];
    const globalItems = memory_cardLibrary?.global_memory_cards ?? snapshot?.global_memory_cards ?? [];
    return [...projectItems, ...globalItems];
  }, [memory_cardLibrary, snapshot]);
  const totalForAssignment = allMemoryCardsForAssignment.length;
  const assignedCount = agentRows.filter((row) => Object.values(row.targets).some(Boolean)).length;
  const coveragePct = totalForAssignment > 0 ? Math.round((assignedCount / totalForAssignment) * 100) : 0;
  const missingCount = totalForAssignment - assignedCount;
  const conflictCount = (qualityView?.status.warnings ?? snapshot?.status.warnings ?? []).length +
    (qualityView?.build_preview.warnings ?? snapshot?.build_preview.warnings ?? []).length;

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

  function handleNavClick(targetPage: PageId) {
    setPage(targetPage);
    setProjectMenuOpen(false);
  }

  return (
    <main className="shell">
      {/* ===== 左侧导航 ===== */}
      <aside className="sidebar" aria-label="导航">
        <div className="brand">
          <div className="brand-mark">
            <Activity size={17} />
          </div>
          <div>
            <strong>Agent Memory Kernel</strong>
            <span>本地技能工作台</span>
          </div>
        </div>

        <nav className="sidebar-nav" aria-label="功能导航">
          {PAGES.map((item) => {
            const Icon = item.icon;
            return (
              <button
                key={item.id}
                className={`nav-item ${page === item.id ? "active" : ""}`}
                onClick={() => handleNavClick(item.id)}
              >
                <Icon size={17} />
                {item.label}
              </button>
            );
          })}
        </nav>

      </aside>

      {/* ===== 右侧工作区 ===== */}
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

        {/* ===== Topbar ===== */}
        <header className="topbar">
          <div className="topbar-left">
            <div className="title-block">
              <h1>{activeProject ? PAGE_DISPLAY[page]?.enLabel ?? "工作台" : "Agent Memory Kernel"}</h1>
              {activeProject ? (
                <span className="path">{activeProject.path}</span>
              ) : null}
            </div>
          </div>

          <div className="topbar-right">
            {/* 项目选择器 */}
            <div className="project-selector">
              <button
                className="project-select-trigger"
                onClick={() => setProjectMenuOpen(!projectMenuOpen)}
                title={activeProject ? activeProject.name : "选择项目"}
              >
                {activeProject ? (
                  <>
                    <HardDrive size={14} />
                    <span>{activeProject.name}</span>
                  </>
                ) : (
                  <>
                    <HardDrive size={14} />
                    <span>选择项目</span>
                  </>
                )}
                <ChevronDown size={13} />
              </button>
              {projectMenuOpen ? (
                <div className="project-select-menu">
                  {projects.length === 0 ? (
                    <p className="empty">还没有发现项目。扫描后会读取常用目录。</p>
                  ) : (
                    projects.map((project) => (
                      <button
                        key={project.path}
                        className={`project-item ${project.path === selectedProject ? "active" : ""}`}
                        onClick={() => {
                          void chooseProject(project.path);
                          setProjectMenuOpen(false);
                        }}
                      >
                        <span>{project.name}</span>
                        <small>{formatAgents(project.agents)}</small>
                      </button>
                    ))
                  )}
                </div>
              ) : null}
            </div>

            {/* 扫描按钮 */}
            <button
              className="icon-button"
              title="扫描本地项目"
              disabled={pendingAction === "扫描"}
              onClick={() => void scanProjects()}
            >
              {pendingAction === "扫描" ? <Loader2 className="spin" size={15} /> : <ScanLine size={15} />}
            </button>

            {/* 任务中心 */}
            <button
              className={`icon-button ${actionableJobCount > 0 ? "has-badge" : ""}`}
              title="任务中心"
              onClick={() => setJobCenterOpen(true)}
            >
              <Bell size={15} />
              {actionableJobCount > 0 ? <span className="badge">{actionableJobCount}</span> : null}
            </button>

            {/* 状态指示 */}
            <div className="status-pill" title={message} role="status" aria-live="polite">
              {pendingAction || backendTaskStatus?.running ? (
                <Loader2 className="spin" size={13} />
              ) : (
                <Check size={13} />
              )}
              <span>{pendingAction ? `${pendingAction}中...` : message}</span>
            </div>
          </div>
        </header>

        {/* 概览指标条 — 仅审阅页显示 */}
        {page === "drafts" ? (
          <ProjectOverviewStrip snapshot={snapshot} dashboard={dashboard} installedCount={installedCount} />
        ) : null}

        {/* 内容区 */}
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
                  {page === "memory-cards" && (
                    <MemoryCards
                      snapshot={snapshot}
                      library={memory_cardLibrary}
                      pendingAction={pendingAction}
                      disabled={!memory_cardLibrary && !snapshot}
                      onAction={projectAction}
                      previewMode={previewMode}
                      projectPath={selectedProject}
                      onRefresh={() => {
                        void loadReadModelsForPage(selectedProject, "memory-cards");
                      }}
                    />
                  )}
                  {page === "agents" && (
                    <Agents
                      snapshot={snapshot}
                      assignment={assignmentView}
                      library={memory_cardLibrary}
                      pendingAction={pendingAction}
                      disabled={!assignmentView && !snapshot}
                      onAction={projectAction}
                      projects={projects}
                    />
                  )}
                  {page === "settings" && <Settings state={state} snapshot={snapshot} projectPath={selectedProject} previewMode={previewMode} synthesisEngine={synthesisEngine} theme={theme} onThemeChange={setTheme} />}
                </>
              )}
            </section>

            {/* 右侧检查器 — 按页面显示不同内容 */}
            <div className="inspector">
              {page === "drafts" ? (
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
              ) : page === "agents" ? (
                <Panel title="分配影响力" icon={GitBranch}>
                  <div className="assignment-impact">
                    <div className="impact-ring-wrap">
                      <svg width="100" height="100" viewBox="0 0 100 100">
                        <circle cx="50" cy="50" r="42" fill="none" stroke="var(--color-border)" strokeWidth="6" />
                        <circle
                          cx="50"
                          cy="50"
                          r="42"
                          fill="none"
                          stroke="var(--color-accent)"
                          strokeWidth="6"
                          strokeDasharray={`${(coveragePct / 100) * 263.9} 263.9`}
                          strokeLinecap="round"
                          transform="rotate(-90 50 50)"
                        />
                      </svg>
                      <div className="impact-ring-text">
                        <strong>{coveragePct}%</strong>
                        <span>覆盖率</span>
                      </div>
                    </div>
                    <div className="impact-stats">
                      <div className="impact-stat">
                        <strong>{missingCount}</strong>
                        <span>未分配</span>
                      </div>
                      <div className="impact-stat">
                        <strong>{conflictCount}</strong>
                        <span>冲突</span>
                      </div>
                    </div>
                    <button className="secondary-action" style={{ width: "100%" }} disabled>
                      查看详情
                    </button>
                  </div>
                </Panel>
              ) : page === "memory-cards" ? (
                <Panel title="演化摘要" icon={Boxes}>
                  <div className="quality">
                    <span>片段 {memory_cardCount}</span>
                    <span>已装 {installedCount}</span>
                  </div>
                  <StatusList
                    empty="暂无演化信号。"
                    items={[
                      ...(qualityView?.status.warnings ?? snapshot?.status.warnings ?? []).slice(0, 4),
                    ]}
                    tone="warning"
                  />
                </Panel>
              ) : null}
            </div>
          </div>
        </section>
      </section>

      {/* 任务中心浮层 */}
      {jobCenterOpen ? (
        <JobCenter
          jobs={jobHistory}
          currentJob={backendTaskStatus}
          onCancel={cancelJob}
          onRetry={retryJob}
          onClearHistory={() => {
            if (!selectedProject) return;
            void projectAction("清除历史", "已清除所有历史记录", "clear_project_history");
          }}
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
