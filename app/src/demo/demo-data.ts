import type {
  DesktopAppState,
  ProjectSnapshot,
  ProjectDashboard,
  ProjectReviewInbox,
  ProjectCandidateInbox,
  ProjectSkillletLibrary,
  ProjectAssignmentView,
  ProjectQualityView,
  RegisteredProject,
} from "../types/domain";

const demoProjects: RegisteredProject[] = [
  {
    name: "演示项目：智能体内核",
    path: "预览模式/智能体内核",
    markers: ["AGENTS.md", "package.json", "src"],
    agents: ["codex", "claude-code"],
    last_seen: "2026-05-02T09:30:00.000Z",
  },
  {
    name: "演示项目：知识库助手",
    path: "预览模式/知识库助手",
    markers: ["CLAUDE.md", "docs", "skills"],
    agents: ["codex"],
    last_seen: "2026-05-02T09:12:00.000Z",
  },
];

export function createDemoAppState(): DesktopAppState {
  return {
    home: "浏览器预览模式",
    scan_roots: ["预览模式/桌面", "预览模式/文档", "预览模式/代码仓库"],
    registry: {
      version: 1,
      projects: demoProjects.map((project) => ({ ...project, markers: [...project.markers], agents: [...project.agents] })),
    },
  };
}

export function createDemoProjectSnapshot(projectPath: string): ProjectSnapshot {
  const project = demoProjects.find((item) => item.path === projectPath) ?? demoProjects[0];
  const resolvedPath = projectPath || project.path;

  return {
    project_path: resolvedPath,
    drafts: [
      {
        id: "demo-draft-review",
        title: "为界面改动保留浏览器预览入口",
        brief: "前端需在无原生运行时自动降级为演示模式",
        kind: "rule",
        scope: "project",
        body: "当桌面运行时不可用时，前端进入演示模式，继续展示项目、草稿、技能片段和分配矩阵。",
        targets: ["codex", "claude-code"],
        evidence: "来自当前预览会话的静态示例。",
        confidence: 0.91,
        reason: "方便在普通浏览器中快速检查视觉和导航，不触发真实文件系统操作。",
        extraction: {
          origin: "user",
          matched_signal: "procedure",
          reason: "High-value durable procedure signal with reusable project impact.",
          source_observations: ["observation:demo"],
          score_breakdown: { durability: 0.8, actionability: 0.8, specificity: 0.8 },
          similar_record: null,
          tags: ["shape:procedure", "target:skill-dir", "hardness:medium"],
          classification: {
            signal: "procedure",
            artifact_kind: "workflow_skill",
            activation: "skill",
            hardness: "medium",
            control: "checklist",
            rationale: "Classified as procedure for workflow_skill with medium hardness.",
            tags: ["shape:procedure", "target:skill-dir", "hardness:medium"],
          },
        },
        matched_template: "preview-mode",
        status: "pending",
        tags: ["ui-design", "preview"],
        language: "zh-CN",
      },
      {
        id: "demo-draft-actions",
        title: "危险操作在演示模式中只显示提示",
        brief: "演示模式所有写操作仅更新顶部状态提示",
        kind: "observation",
        scope: "agent",
        body: "批准、删除、安装和同步按钮不会调用 Tauri 命令，只会更新顶部状态提示。",
        targets: ["codex"],
        evidence: "演示数据用于验证按钮状态和页面流转。",
        confidence: 0.87,
        status: "pending",
        tags: ["demo", "security"],
        language: "zh-CN",
      },
    ],
    skilllets: [
      {
        id: "preview-mode",
        title: "浏览器预览模式",
        brief: "无原生运行时自动降级为静态数据预览",
        kind: "skilllet",
        scope: "project",
        body: "检测到没有 Tauri 运行时后，加载静态项目快照，让页面可以继续导航和展示。",
        tags: ["ui-design", "preview"],
        language: "zh-CN",
      },
      {
        id: "safe-actions",
        title: "本地安全动作",
        brief: "预览模式下所有写操作只给出中文反馈",
        kind: "skilllet",
        scope: "agent",
        body: "演示模式中的操作只给出中文反馈，不访问真实文件系统。",
        tags: ["demo", "security"],
        language: "zh-CN",
      },
    ],
    global_skilllets: [
      {
        id: "global:keep-ui-chinese",
        title: "UI 语言保持中文优先",
        brief: "所有界面文案以简体中文作为首要语言",
        kind: "constraint",
        scope: "global",
        body: "全局约束：页面标签、按钮文案、提示信息、错误消息全部使用简体中文，确保中文用户可直接使用。",
        tags: ["ui-design", "chinese"],
        language: "zh-CN",
      },
      {
        id: "global:apple-like-polish",
        title: "Apple 风格界面规范",
        brief: "毛玻璃 + 圆角 + 柔和阴影的统一视觉语言",
        kind: "convention",
        scope: "global",
        body: "全局约定：使用 SF Pro / PingFang 字体、backdrop-filter 毛玻璃效果、大圆角、浅色背景渐变，保持清爽专业。",
        tags: ["ui-design", "style"],
        language: "zh-CN",
      },
    ],
    observations: [
      { id: "demo-observation-1", agent: "codex", source_path: `${resolvedPath}/AGENTS.md` },
      { id: "demo-observation-2", agent: "claude-code", source_path: `${resolvedPath}/CLAUDE.md` },
      { id: "demo-observation-3", source_path: `${resolvedPath}/docs/preview.md` },
    ],
    catalog_status: {
      items: [
        {
          installed: true,
          package: {
            id: "local-preview-core",
            title: "本地预览核心条目",
            description: "提供演示项目、草稿和同步矩阵，让浏览器预览保持完整。",
            version: "1.0.0",
            tags: ["预览", "本地", "安全"],
          },
        },
        {
          installed: false,
          package: {
            id: "review-polish",
            title: "审阅体验增强条目",
            description: "示例条目，用于展示安装卡片、标签和按钮在演示模式下的状态。",
            version: "0.4.2",
            tags: ["审阅", "界面", "演示"],
          },
        },
      ],
    },
    target_matrix: {
      agents: project.agents,
      rows: [
        {
          skilllet_id: "preview-mode",
          title: "浏览器预览模式",
          targets: { codex: true, "claude-code": project.agents.includes("claude-code") },
        },
        {
          skilllet_id: "safe-actions",
          title: "本地安全动作",
          targets: { codex: true, "claude-code": false },
        },
      ],
    },
    rule_ci: { passed: 12, failed: 0 },
    build_preview: {
      actions: [
        "将演示草稿同步到 Codex 预览目标",
        "更新 Claude Code 目标的技能片段索引",
        "生成本地技能片段状态预览",
      ],
      warnings: ["当前为演示数据；真实文件读取和写入需要通过 bun run app:dev 启动 Tauri。"],
    },
    status: {
      warnings: resolvedPath === project.path ? [] : ["该项目来自演示数据回退，未读取真实文件系统。"],
    },
  };
}

export function createDemoProjectDashboard(projectPath: string): ProjectDashboard {
  const snapshot = createDemoProjectSnapshot(projectPath);
  return {
    project_path: snapshot.project_path,
    candidate_count: createDemoProjectCandidateInbox(projectPath).candidates.length,
    draft_count: snapshot.drafts.length,
    skilllet_count: snapshot.skilllets.length,
    observation_count: snapshot.observations.length,
    global_skilllet_count: snapshot.global_skilllets?.length ?? 0,
    enabled_agents: ["claude-code", "codex"],
    warning_count: snapshot.status.warnings.length + snapshot.build_preview.warnings.length,
  };
}

export function createDemoProjectReviewInbox(projectPath: string): ProjectReviewInbox {
  const snapshot = createDemoProjectSnapshot(projectPath);
  return {
    project_path: snapshot.project_path,
    drafts: [...snapshot.drafts],
  };
}

export function createDemoProjectCandidateInbox(projectPath: string): ProjectCandidateInbox {
  const snapshot = createDemoProjectSnapshot(projectPath);
  return {
    project_path: snapshot.project_path,
    candidates: snapshot.drafts.slice(0, 2).map((draft, index) => ({
      id: `candidate:${draft.id}`,
      title: draft.title,
      kind: draft.kind,
      scope: draft.scope,
      body: draft.body,
      brief: draft.brief,
      tags: [...(draft.tags ?? [])],
      language: draft.language,
      targets: draft.targets,
      evidence: draft.evidence,
      confidence: Math.min(0.96, (draft.confidence ?? 0.8) + 0.03),
      reason: draft.reason ?? "演示候选来自本地高价值过滤。",
      matched_template: draft.matched_template,
      source_observations: [`demo-observation-${index + 1}`],
      status: "candidate",
    })),
  };
}

export function createDemoProjectSkillletLibrary(projectPath: string): ProjectSkillletLibrary {
  const snapshot = createDemoProjectSnapshot(projectPath);
  return {
    project_path: snapshot.project_path,
    skilllets: [...snapshot.skilllets],
    global_skilllets: [...(snapshot.global_skilllets ?? [])],
    catalog_status: snapshot.catalog_status,
  };
}

export function createDemoProjectAssignmentView(projectPath: string): ProjectAssignmentView {
  const snapshot = createDemoProjectSnapshot(projectPath);
  return {
    project_path: snapshot.project_path,
    enabled_agents: [...snapshot.target_matrix.agents],
    target_matrix: snapshot.target_matrix,
  };
}

export function createDemoProjectQualityView(projectPath: string): ProjectQualityView {
  const snapshot = createDemoProjectSnapshot(projectPath);
  return {
    project_path: snapshot.project_path,
    rule_ci: snapshot.rule_ci,
    build_preview: snapshot.build_preview,
    status: snapshot.status,
  };
}
