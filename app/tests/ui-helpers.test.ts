import { describe, expect, test } from "bun:test";
import {
  buildEditFormFromDraft,
  buildEditFormFromSkilllet,
  buildKernelPlanForEditor,
  buildKernelPlanForInvoke,
  confirmedAgentManagedPolicy,
  createDemoAppState,
  createDemoProjectCandidateInbox,
  createDemoProjectAssignmentView,
  createDemoProjectDashboard,
  createDemoProjectQualityView,
  createDemoProjectReviewInbox,
  createDemoProjectSkillletLibrary,
  createDemoProjectSnapshot,
  describeDraftForReview,
  PACKAGE_MANAGER_EXPLANATION,
  describeSkillletPlainly,
  deriveSkillletEvolution,
  EDITABLE_AGENTS,
  filterCandidatesForInbox,
  formatAgent,
  formatAgents,
  getPreviewActionMessage,
  getPreviewPlanResult,
  KIND_OPTIONS,
  manualReviewPolicy,
  normalizePlanReviewResult,
  parseCommaTags,
  removeCandidateFromInbox,
  removeDraftFromSnapshot,
  resolveTaskProgress,
  SCOPE_OPTIONS,
  taskProgressForAction,
  filterRecordsByTag,
  formatJobLifecycle,
  formatJobStartMessage,
  sortCandidatesForInbox,
  isChineseLabel,
  isTauriRuntimeUnavailable,
  nextSkillletTargets,
  PAGES,
  projectOverviewMetrics,
  translateKind,
  translateScope,
} from "../src/ui-helpers";
import { readFileSync } from "node:fs";

describe("UI helper labels", () => {
  test("formats known and unknown agents", () => {
    expect(formatAgent("codex")).toBe("Codex");
    expect(formatAgent("claude-code")).toBe("Claude Code");
    expect(formatAgent("custom-agent")).toBe("custom-agent");
    expect(formatAgents([])).toBe("未配置智能体");
    expect(formatAgents(["codex", "claude-code"])).toBe("Codex / Claude Code");
  });

  test("translates draft kind and scope labels", () => {
    expect(translateKind("rule")).toBe("规则");
    expect(translateKind("skilllet")).toBe("技能片段");
    expect(translateKind("observation")).toBe("观察");
    expect(translateKind("preference")).toBe("偏好");
    expect(translateKind("constraint")).toBe("约束");
    expect(translateKind("procedure")).toBe("流程");
    expect(translateKind("unknown-kind")).toBe("unknown-kind");

    expect(translateScope("project")).toBe("项目");
    expect(translateScope("global")).toBe("全局");
    expect(translateScope("agent")).toBe("智能体");
    expect(translateScope("unknown-scope")).toBe("unknown-scope");
  });

  test("keeps page labels Chinese-first", () => {
    expect(PAGES.map((page) => page.label)).toEqual(["审阅", "技能片段", "分配", "包管理", "设置"]);
    expect(PAGES.every((page) => isChineseLabel(page.label))).toBe(true);
  });

  test("does not use native browser confirm dialogs for kernel actions", () => {
    const mainSource = readFileSync("src/main.tsx", "utf8");
    expect(mainSource).not.toContain("window.confirm");
  });

  test("tauri config keeps a restrictive CSP for file-writing commands", () => {
    const tauriConfig = JSON.parse(readFileSync("../src-tauri/tauri.conf.json", "utf8"));
    const csp = tauriConfig.app.security.csp;

    expect(typeof csp).toBe("string");
    expect(csp).toContain("default-src 'self'");
    expect(csp).toContain("object-src 'none'");
    expect(csp).toContain("frame-ancestors 'none'");
  });

  test("quickstart opens the current Tauri app instead of the removed egui app", () => {
    const quickstart = readFileSync("../docs/quickstart.md", "utf8");

    expect(quickstart).toContain("bun run app:dev");
    expect(quickstart).not.toContain("cargo run -- app");
    expect(quickstart).not.toContain("Rust/egui");
  });

  test("keeps project switching available while background tasks run", () => {
    const mainSource = readFileSync("src/main.tsx", "utf8");
    const clientSource = readFileSync("src/tauri-client.ts", "utf8");
    expect(mainSource).not.toContain("disabled={isBusy}\n                onClick={() => void chooseProject(project.path)}");
    expect(mainSource).toContain("useProjectReadModels");
    expect(clientSource).toContain("get_project_dashboard");
  });

  test("handles background dashboard loader errors without unhandled void rejections", () => {
    const hookSource = readFileSync("src/hooks/useProjectReadModels.ts", "utf8");
    expect(hookSource).toContain("const loadDashboard");
    expect(hookSource).toContain("catch (error)");
    expect(hookSource).toContain("概览加载失败");
  });

  test("uses inbox as the default page without automatic full snapshot loading", () => {
    const mainSource = readFileSync("src/main.tsx", "utf8");
    const draftsSource = readFileSync("src/components/pages/Drafts.tsx", "utf8");
    expect(mainSource).toContain('React.useState<PageId>("drafts")');
    expect(mainSource).not.toContain("void loadSnapshot(first);");
    expect(mainSource).not.toContain("void loadSnapshot(projectPath);");
    expect(mainSource).not.toContain("void loadSnapshot(path);");
    expect(draftsSource).toContain("Inbox 工作台");
  });

  test("startup loads only project registry without auto-opening the first project", () => {
    const mainSource = readFileSync("src/main.tsx", "utf8");
    const selectionSource = readFileSync("src/hooks/useProjectSelection.ts", "utf8");

    expect(mainSource).not.toContain('runTask("刷新"');
    expect(selectionSource).toContain('const projectPath = stillExists ? current : "";');
    expect(selectionSource).not.toContain('next.registry.projects[0]?.path || ""');
  });

  test("keeps quality checks out of the default inbox read model path", () => {
    const strategySource = readFileSync("src/project-read-models.ts", "utf8");

    expect(strategySource).toContain('page === "settings"');
    expect(strategySource).not.toContain('"canvas"');
  });
});

describe("preview mode helpers", () => {
  test("detects browser previews without the Tauri runtime", () => {
    expect(isTauriRuntimeUnavailable(new Error("window.__TAURI_INTERNALS__ is undefined"))).toBe(true);
    expect(isTauriRuntimeUnavailable("Command get_app_state not found because Tauri is unavailable")).toBe(true);
    expect(isTauriRuntimeUnavailable("Cannot read properties of undefined (reading 'invoke')")).toBe(true);
    expect(isTauriRuntimeUnavailable("network timeout")).toBe(false);
  });

  test("creates a Chinese demo app state with a selectable project", () => {
    const state = createDemoAppState();

    expect(state.registry.projects).toHaveLength(2);
    expect(state.registry.projects[0]?.name).toBe("演示项目：智能体内核");
    expect(state.registry.projects[0]?.path).toBe("预览模式/智能体内核");
    expect(state.registry.projects.every((project) => project.agents.length > 0)).toBe(true);
  });

  test("creates snapshots for known and unknown preview projects", () => {
    const known = createDemoProjectSnapshot("预览模式/智能体内核");
    const fallback = createDemoProjectSnapshot("预览模式/未知项目");

    expect(known.project_path).toBe("预览模式/智能体内核");
    expect(known.drafts.length).toBeGreaterThan(0);
    expect(known.catalog_status.items.some((item) => item.installed)).toBe(true);
    expect(fallback.project_path).toBe("预览模式/未知项目");
    expect(fallback.status.warnings.join("")).toContain("演示数据");
  });

  test("creates lightweight dashboards for first paint before the full snapshot", () => {
    const dashboard = createDemoProjectDashboard("预览模式/智能体内核");
    const metrics = projectOverviewMetrics(null, dashboard, 3);

    expect(dashboard.project_path).toBe("预览模式/智能体内核");
    expect(dashboard.candidate_count).toBeGreaterThan(0);
    expect(metrics).toEqual({
      candidateCount: dashboard.candidate_count,
      draftCount: dashboard.draft_count,
      skillletCount: dashboard.skilllet_count,
      observationCount: dashboard.observation_count,
      installedCount: 3,
    });
  });

  test("creates page-level demo read models from the same preview snapshot", () => {
    const projectPath = "预览模式/智能体内核";
    const snapshot = createDemoProjectSnapshot(projectPath);
    const inbox = createDemoProjectReviewInbox(projectPath);
    const library = createDemoProjectSkillletLibrary(projectPath);
    const assignment = createDemoProjectAssignmentView(projectPath);
    const quality = createDemoProjectQualityView(projectPath);

    expect(inbox.drafts.length).toBe(snapshot.drafts.length);
    expect(library.skilllets.length).toBe(snapshot.skilllets.length);
    expect(library.catalog_status.items.length).toBe(snapshot.catalog_status.items.length);
    expect(assignment.target_matrix.rows.length).toBe(snapshot.target_matrix.rows.length);
    expect(quality.rule_ci.passed).toBe(snapshot.rule_ci.passed);
    expect(JSON.stringify(inbox)).toContain("High-value durable");
  });

  test("demo read-model output contains extraction classification artifact kind and structured tags", () => {
    const projectPath = "预览模式/智能体内核";
    const snapshot = createDemoProjectSnapshot(projectPath);
    const draftWithClassification = snapshot.drafts.find((d) => d.extraction?.classification);
    expect(draftWithClassification).toBeDefined();
    expect(draftWithClassification!.extraction!.classification!.artifact_kind).toBe("workflow_skill");
    expect(draftWithClassification!.extraction!.classification!.hardness).toBe("medium");
    expect(JSON.stringify(snapshot)).toContain("artifact_kind");
    expect(JSON.stringify(snapshot)).toContain("hardness:medium");
  });

  test("returns harmless Chinese action messages for preview mode", () => {
    expect(getPreviewActionMessage("扫描")).toContain("预览模式");
    expect(getPreviewActionMessage("批准-abc")).toContain("已从草稿列表移除");
    expect(getPreviewActionMessage("删除-abc")).toContain("已从草稿列表移除");
    expect(getPreviewActionMessage("安装-demo")).toContain("不会写入文件");
  });
});

describe("skilllet evolution helpers", () => {
  test("describes skilllets in plain Chinese", () => {
    expect(
      describeSkillletPlainly({
        id: "project:ui-responsive",
        title: "Keep UI Responsive During Long Tasks",
        kind: "procedure",
        scope: "project",
        body: "Run long scans in background tasks and refresh UI state separately.",
      }),
    ).toContain("以后遇到类似任务");
  });

  test("derives engineering evolution state", () => {
    const snapshot = createDemoProjectSnapshot("预览模式/智能体内核");
    const insight = deriveSkillletEvolution(snapshot.skilllets[0]!, snapshot);

    expect(insight.confidence).toBeGreaterThan(0);
    expect(insight.stability).toBeGreaterThan(0);
    expect(insight.timeline.length).toBeGreaterThan(0);
    expect(["active", "dormant"]).toContain(insight.activity);
  });

  test("toggles skilllet targets freely and allows inactive empty assignment", () => {
    expect(nextSkillletTargets(["codex"], "claude-code", true)).toEqual(["claude-code", "codex"]);
    expect(nextSkillletTargets(["codex", "claude-code"], "codex", false)).toEqual(["claude-code"]);
    expect(nextSkillletTargets(["codex"], "codex", false)).toEqual([]);
  });

  test("filters drafts and skilllets by tags", () => {
    const snapshot = createDemoProjectSnapshot("预览模式/智能体内核");
    const records = [...snapshot.drafts, ...snapshot.skilllets];

    expect(filterRecordsByTag(records, "ui-design").map((item) => item.title)).toContain("浏览器预览模式");
    expect(filterRecordsByTag(records, "all").length).toBe(records.length);
  });

  test("describes review drafts in plain Chinese when brief is missing", () => {
    const description = describeDraftForReview({
      id: "draft-without-brief",
      title: "Use Axios",
      kind: "preference",
      scope: "project",
      body: "Use Axios for frontend HTTP requests instead of Fetch.",
      targets: ["codex"],
      status: "draft",
      confidence: 0.82,
    });

    expect(description).toContain("Axios");
    expect(description).toContain("前端");
    expect(description).not.toContain("用于审阅");
    expect(description).not.toContain("写入");
    expect(description.length).toBeLessThanOrEqual(90);
  });

  test("derives staged progress for startup and long-running actions", () => {
    expect(taskProgressForAction("启动").label).toContain("建立本地记忆索引");
    expect(taskProgressForAction("整理历史").percent).toBeGreaterThan(0);
    expect(taskProgressForAction("").percent).toBe(100);
  });

  test("uses backend task status ahead of local heuristic progress", () => {
    const progress = resolveTaskProgress("整理历史", {
      key: "启动",
      label: "读取本地会话",
      description: "首次启动正在增量整理历史",
      percent: 62,
      running: true,
      message: "后台处理中",
      details: [],
    });

    expect(progress.label).toBe("读取本地会话");
    expect(progress.percent).toBe(62);
    expect(progress.description).toContain("增量整理");
  });

  test("explains long-running local conversation processing instead of looking frozen at 55 percent", () => {
    const progress = resolveTaskProgress("启动", {
      key: "启动",
      label: "读取本地会话",
      description: "首次启动会处理历史记录，之后只处理新增内容",
      percent: 55,
      running: true,
      message: "后台处理中",
      details: [{ label: "导入增量", description: "正在读取新增 JSONL 行", percent: 55 }],
    });

    expect(progress.percent).toBe(55);
    expect(progress.description).not.toContain("仍在处理");
    expect(progress.description).toContain("首次");
  });

  test("keeps backend task detail rows for the activity window", () => {
    const status = {
      job_id: "job-local-1",
      key: "整理历史",
      stage: "运行整理引擎",
      label: "运行整理引擎",
      description: "本地极速模式不会启动外部 Agent",
      percent: 78,
      running: true,
      message: "后台处理中",
      started_at: "1000",
      finished_at: null,
      result_summary: null,
      details: [
        { label: "发现会话文件", description: "找到 12 个 Claude/Codex JSONL", percent: 20 },
        { label: "运行整理引擎", description: "只把本地过滤后的候选交给深度引擎", percent: 78 },
      ],
      logs: [
        { timestamp: "1000", stage: "发现会话文件", description: "找到 12 个 Claude/Codex JSONL", percent: 20 },
        { timestamp: "1100", stage: "运行整理引擎", description: "只把本地过滤后的候选交给深度引擎", percent: 78 },
      ],
    };

    expect(status.details.at(-1)?.label).toBe("运行整理引擎");
    expect(status.logs.at(-1)?.stage).toBe("运行整理引擎");
    expect(resolveTaskProgress("整理历史", status).percent).toBe(78);
  });

  test("task progress can surface job metadata for a non-blocking job center", () => {
    const progressSource = readFileSync("src/components/common.tsx", "utf8");

    expect(progressSource).toContain("backendStatus?.job_id");
    expect(progressSource).toContain("backendStatus?.result_summary");
    expect(progressSource).toContain("backendStatus?.started_at");
    expect(progressSource).toContain("backendStatus?.logs");
  });

  test("formats job start payloads as clear Chinese task tickets", () => {
    expect(
      formatJobStartMessage({
        job_id: "job-evolve-1",
        key: "整理历史",
        stage: "发现会话文件",
        message: "已加入后台整理队列",
        accepted: true,
      }),
    ).toBe("整理历史已加入后台：已加入后台整理队列（任务 job-evolve-1）");
  });

  test("formats job lifecycle states for the Job Center", () => {
    expect(formatJobLifecycle("running")).toBe("运行中");
    expect(formatJobLifecycle("cancelling")).toBe("取消中");
    expect(formatJobLifecycle("completed")).toBe("已完成");
    expect(formatJobLifecycle("failed")).toBe("失败");
    expect(formatJobLifecycle("cancelled")).toBe("已取消");
  });

  test("renders job center controls and cancel command wiring", () => {
    const mainSource = readFileSync("src/main.tsx", "utf8");
    const hookSource = readFileSync("src/hooks/useJobCenter.ts", "utf8");
    const clientSource = readFileSync("src/tauri-client.ts", "utf8");

    expect(clientSource).toContain("get_job_history");
    expect(clientSource).toContain("cancel_job");
    expect(hookSource).toContain("retryJob");
    expect(mainSource).toContain("onRetry");
    expect(mainSource).toContain("JobCenter");
    expect(mainSource).toContain("任务中心");
  });

  test("job center supports filtering and marks precise-retry jobs", () => {
    const jobCenterSource = readFileSync("src/components/JobCenter.tsx", "utf8");

    expect(jobCenterSource).toContain("statusFilter");
    expect(jobCenterSource).toContain("typeFilter");
    expect(jobCenterSource).toContain('aria-label="按状态筛选"');
    expect(jobCenterSource).toContain('aria-label="按类型筛选"');
    expect(jobCenterSource).toContain("job.replay?.command");
    expect(jobCenterSource).toContain("可重试");
    expect(jobCenterSource).toContain("融合Skilllet");
  });

  test("job retry uses the persisted replay command instead of guessing only by job key", () => {
    const hookSource = readFileSync("src/hooks/useJobCenter.ts", "utf8");

    expect(hookSource).toContain("job.replay");
    expect(hookSource).toContain("retryJobCommand(job.replay.command");
    expect(hookSource).not.toContain('if (job.key === "整理历史")');
  });

  test("refreshes read models after background scan, evolution and sync jobs complete", () => {
    const hookSource = readFileSync("src/hooks/useJobCenter.ts", "utf8");

    expect(hookSource).toContain("handledJobIdsRef");
    expect(hookSource).toContain("reloadAppStateFromBackend");
    expect(hookSource).toContain('job.key === "扫描"');
    expect(hookSource).toContain('job.key === "整理历史" || job.key === "同步" || job.key === "融合Skilllet"');
    expect(hookSource).not.toContain('if (command === "sync_project" && actionProjectPath === selectedProjectRef.current)');
  });

  test("fast inbox decisions do not wait for full read-model refresh before unblocking", () => {
    const actionSource = readFileSync("src/hooks/useProjectActions.ts", "utf8");

    expect(actionSource).toContain("void refreshAfterMutation(actionProjectPath, command)");
    expect(actionSource).not.toContain("await refreshAfterMutation(actionProjectPath, command);");
  });

  test("project mutations obtain backend decision tokens before invoking writes", () => {
    const actionSource = readFileSync("src/hooks/useProjectActions.ts", "utf8");

    expect(actionSource).toContain("buildKernelPlanForInvoke(command, args)");
    expect(actionSource).toContain("planKernelCommand({");
    expect(actionSource).toContain("decisionToken");
  });

  test("optimistic inbox removal fires before mutation, with backend reload on failure", () => {
    const actionSource = readFileSync("src/hooks/useProjectActions.ts", "utf8");
    const nonPreviewPath = actionSource.slice(
      actionSource.indexOf("if (previewMode)"),
      actionSource.indexOf("await runTask(actionKey"),
    );
    const afterPreviewReturn = nonPreviewPath.slice(nonPreviewPath.indexOf("return;"));

    expect(afterPreviewReturn).toContain("applyCandidateOptimism();");
    expect(afterPreviewReturn).toContain("applyDraftOptimism();");
    expect(actionSource).toContain("void refreshAfterMutation(actionProjectPath, command)");
  });

  test("loads page-level read models instead of relying only on the full project snapshot", () => {
    const mainSource = readFileSync("src/main.tsx", "utf8");
    const strategySource = readFileSync("src/project-read-models.ts", "utf8");

    expect(strategySource).toContain("get_project_review_inbox");
    expect(strategySource).toContain("get_project_skilllet_library");
    expect(strategySource).toContain("get_project_assignment_view");
    expect(strategySource).toContain("get_project_quality_view");
    expect(mainSource).toContain("useProjectReadModels");
    expect(strategySource).toContain("loadProjectReadModelsFromTauri");
    expect(mainSource).toContain("loadReadModelsForPage");
    expect(mainSource).toContain("reviewInbox");
    expect(mainSource).toContain("skillletLibrary");
    expect(mainSource).toContain("assignmentView");
    expect(mainSource).toContain("qualityView");
  });

  test("package manager explanation names reusable Skilllet packages and target agents", () => {
    expect(PACKAGE_MANAGER_EXPLANATION).toContain("Skilllet 包");
    expect(PACKAGE_MANAGER_EXPLANATION).toContain("Claude Code");
    expect(PACKAGE_MANAGER_EXPLANATION).toContain("Codex");
  });

  test("removes approved or deleted drafts from the current snapshot immediately", () => {
    const snapshot = createDemoProjectSnapshot("预览模式/智能体内核");
    const draftId = snapshot.drafts[0]!.id;
    const next = removeDraftFromSnapshot(snapshot, draftId);

    expect(next.drafts.some((draft) => draft.id === draftId)).toBe(false);
    expect(snapshot.drafts.some((draft) => draft.id === draftId)).toBe(true);
    expect(next.project_path).toBe(snapshot.project_path);
  });

  test("removes candidate actions from the current candidate inbox immediately", () => {
    const inbox = createDemoProjectCandidateInbox("预览模式/智能体内核");
    const candidateId = inbox.candidates[0]!.id;
    const next = removeCandidateFromInbox(inbox, candidateId);

    expect(next?.candidates.some((candidate) => candidate.id === candidateId)).toBe(false);
    expect(inbox.candidates.some((candidate) => candidate.id === candidateId)).toBe(true);
    expect(next?.project_path).toBe(inbox.project_path);
  });

  test("sorts and filters candidate inbox records for high-confidence review", () => {
    const candidates = [
      { id: "low", title: "Low", body: "x", kind: "rule", scope: "project", targets: [], evidence: "e", confidence: 0.4, status: "candidate" as const },
      { id: "templated", title: "Templated", body: "x", kind: "preference", scope: "project", targets: [], evidence: "e", confidence: 0.9, matched_template: "prefer-tool", status: "candidate" as const },
      { id: "plain", title: "Plain", body: "x", kind: "constraint", scope: "project", targets: [], evidence: "e", confidence: 0.82, status: "candidate" as const },
    ];

    expect(sortCandidatesForInbox(candidates).map((candidate) => candidate.id)).toEqual(["templated", "plain", "low"]);
    expect(filterCandidatesForInbox(candidates, "high").map((candidate) => candidate.id)).toEqual(["templated", "plain"]);
    expect(filterCandidatesForInbox(candidates, "low").map((candidate) => candidate.id)).toEqual(["low"]);
    expect(filterCandidatesForInbox(candidates, "preference").map((candidate) => candidate.id)).toEqual(["templated"]);
  });
});

describe("edit helpers", () => {
  test("parses comma and Chinese-comma separated tags", () => {
    expect(parseCommaTags("")).toEqual([]);
    expect(parseCommaTags("ui-design, security")).toEqual(["ui-design", "security"]);
    expect(parseCommaTags("界面，安全，审核")).toEqual(["界面", "安全", "审核"]);
    expect(parseCommaTags("a, b, , c")).toEqual(["a", "b", "c"]);
    expect(parseCommaTags("  leading, trailing  ")).toEqual(["leading", "trailing"]);
  });

  test("returns preview plan result based on kind", () => {
    const approveResult = getPreviewPlanResult("rule");
    expect(approveResult.disposition).toBe("execute");
    expect(approveResult.reviewRequired).toBe(false);

    const reviewResult = getPreviewPlanResult("constraint");
    expect(reviewResult.disposition).toBe("review-required");
    expect(reviewResult.reviewRequired).toBe(true);
  });

  test("normalizes Rust snake_case kernel decision fields", () => {
    const normalized = normalizePlanReviewResult({
      risk: "medium",
      disposition: "review-required",
      reason: "Manual mode keeps mutating kernel commands in human review.",
      requires_human_review: true,
      decision_token: "token-1",
    });

    expect(normalized.reviewRequired).toBe(true);
    expect(normalized.requires_human_review).toBe(true);
    expect(normalized.disposition).toBe("review-required");
    expect(normalized.decisionToken).toBe("token-1");
  });

  test("builds edit form from draft record", () => {
    const draft = {
      id: "test-draft",
      title: "测试草稿",
      body: "测试内容",
      kind: "rule",
      scope: "project",
      evidence: "来源",
      status: "pending",
      targets: ["codex", "claude-code"],
      brief: "简要",
      tags: ["demo", "test"],
    };
    const form = buildEditFormFromDraft(draft);
    expect(form.title).toBe("测试草稿");
    expect(form.brief).toBe("简要");
    expect(form.body).toBe("测试内容");
    expect(form.kind).toBe("rule");
    expect(form.scope).toBe("project");
    expect(form.tagsInput).toContain("demo");
    expect(form.tagsInput).toContain("test");
    expect(form.targets).toEqual(["codex", "claude-code"]);
  });

  test("builds edit form from skilllet record (no targets)", () => {
    const skilllet = {
      id: "test-skilllet",
      title: "测试片段",
      body: "内容",
      kind: "procedure",
      scope: "global",
      tags: ["a"],
    };
    const form = buildEditFormFromSkilllet(skilllet);
    expect(form.title).toBe("测试片段");
    expect(form.targets).toEqual([]);
    expect(form.tagsInput).toBe("a");
  });

  test("edit form defaults missing fields to empty", () => {
    const draft = {
      id: "minimal",
      title: "最小",
      body: "内容",
      kind: "observation",
      scope: "agent",
      evidence: "",
      status: "pending",
      targets: [],
    };
    const form = buildEditFormFromDraft(draft);
    expect(form.brief).toBe("");
    expect(form.tagsInput).toBe("");
    expect(form.targets).toEqual([]);
  });

  test("exposes kind, scope and agent options", () => {
    expect(KIND_OPTIONS).toContain("rule");
    expect(KIND_OPTIONS).toContain("constraint");
    expect(SCOPE_OPTIONS).toContain("project");
    expect(SCOPE_OPTIONS).toContain("global");
    expect(EDITABLE_AGENTS).toContain("codex");
    expect(EDITABLE_AGENTS).toContain("claude-code");
  });

  test("builds confirmed policy payload for user-approved mutations", () => {
    expect(confirmedAgentManagedPolicy()).toEqual({
      mode: "agent-managed",
      auto_approve_min_confidence: 0.72,
      allow_file_writes: true,
      require_review_for_high_risk: false,
    });
    expect(manualReviewPolicy().mode).toBe("manual");
  });

  test("builds kernel plan payloads matching Tauri mutation envelopes", () => {
    expect(buildKernelPlanForInvoke("approve_draft", { id: "project:prefer-bun" })).toEqual({
      command: { type: "approve-draft", id: "project:prefer-bun" },
      payload: { id: "project:prefer-bun" },
    });
    expect(buildKernelPlanForInvoke("promote_candidate", { id: "candidate:prefer-bun" })).toEqual({
      command: { type: "promote-candidate", id: "candidate:prefer-bun" },
      payload: { id: "candidate:prefer-bun" },
    });
    expect(buildKernelPlanForInvoke("hide_candidate", { id: "candidate:prefer-bun" })).toEqual({
      command: { type: "hide-candidate", id: "candidate:prefer-bun" },
      payload: { id: "candidate:prefer-bun" },
    });
    expect(buildKernelPlanForInvoke("reject_candidate", { id: "candidate:prefer-bun", reason: "noise" })).toEqual({
      command: { type: "reject-candidate", id: "candidate:prefer-bun" },
      payload: { id: "candidate:prefer-bun", reason: "noise" },
    });
    expect(buildKernelPlanForInvoke("sync_project")).toEqual({
      command: { type: "compile-project", dry_run: false },
      payload: { dry_run: false },
    });
    expect(buildKernelPlanForInvoke("evolve_project", { targets: ["codex"], dryRun: false })).toEqual({
      command: { type: "evolve-project", dry_run: false, engine: "local" },
      payload: { targets: ["codex"], dry_run: false, engine: "local" },
    });
    expect(buildKernelPlanForInvoke("install_catalog_package", { packageId: "core:rust", targets: ["codex"] })).toEqual({
      command: { type: "install-catalog-package", id: "core:rust", targets: ["codex"] },
      payload: { package_id: "core:rust", targets: ["codex"] },
    });
  });

  test("builds editor plan payload with edited input", () => {
    expect(buildKernelPlanForEditor("draft", "project:demo", { title: "Demo" })).toEqual({
      command: { type: "update-draft", id: "project:demo" },
      payload: { id: "project:demo", input: { title: "Demo" } },
    });
  });
});
