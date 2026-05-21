import type {
  DesktopAppState,
  ProjectSnapshot,
  ProjectDashboard,
  ProjectReviewInbox,
  ProjectCandidateInbox,
  ProjectMemoryCardLibrary,
  ProjectSkillLibrary,
  ProjectAssignmentView,
  ProjectEvalRunView,
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
        body: "当桌面运行时不可用时，前端进入演示模式，继续展示项目、草稿、Memory Card 和装填槽位。",
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
    memory_cards: [
      {
        id: "preview-mode",
        title: "浏览器预览模式",
        brief: "无原生运行时自动降级为静态数据预览",
        kind: "memory_card",
        scope: "project",
        body: "检测到没有 Tauri 运行时后，加载静态项目快照，让页面可以继续导航和展示。",
        tags: ["ui-design", "preview"],
        language: "zh-CN",
        activation: "skill",
        extraction: {
          card_function: "skill_targeted",
          value_claim: "补强预览 Skill 的验收边界，让用户能先看到功能闭环是否真的可用。",
          value_delta: {
            existing_behavior: "预览 Skill 已能展示静态数据。",
            missing_part: "缺少对真实用户路径和验收边界的说明。",
            new_behavior: "下次使用预览 Skill 时先确认关键路径、证据展示和动作按钮是否完整。",
            why_not_duplicate: "它不是重复描述预览模式，而是补上验收与使用边界。",
          },
          target_context: {
            target_type: "project_skill",
            target_id: "project:obsidian-markdown",
            why_this_target: "该 Skill 负责项目预览与文档路径，最需要这张 Memory Card 的验收边界。",
          },
          skill_usefulness: {
            target_skill_id: "project:obsidian-markdown",
            before_behavior: "Before: Skill `obsidian-markdown` covers note editing and documentation paths.",
            after_behavior: "After: proposal adds acceptance to the target Skill behavior.",
            improved_axes: ["acceptance"],
            missing_axes: ["boundary"],
            verdict: "counterfactual_pass",
            score: 0.48,
          },
          synthesis_trace: [
            { step: "value_delta", summary: "Compared demo preview behavior against the Skill target.", item_ids: [] },
            { step: "rewrite", summary: "Rendered as a Skill-targeted Memory Card.", item_ids: [] },
          ],
        },
      },
      {
        id: "safe-actions",
        title: "本地安全动作",
        brief: "预览模式下所有写操作只给出中文反馈",
        kind: "memory_card",
        scope: "agent",
        body: "演示模式中的操作只给出中文反馈，不访问真实文件系统。",
        tags: ["demo", "security"],
        language: "zh-CN",
      },
    ],
    global_memory_cards: [
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
          memory_card_id: "preview-mode",
          title: "浏览器预览模式",
          targets: { codex: true, "claude-code": project.agents.includes("claude-code") },
        },
        {
          memory_card_id: "safe-actions",
          title: "本地安全动作",
          targets: { codex: true, "claude-code": false },
        },
      ],
    },
    rule_ci: { passed: 12, failed: 0 },
    build_preview: {
      actions: [
        "将演示草稿同步到 Codex 预览目标",
        "更新 Claude Code 目标的 Memory Card 索引",
        "生成本地 Memory Card 状态预览",
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
    memory_card_count: snapshot.memory_cards.length,
    observation_count: snapshot.observations.length,
    global_memory_card_count: snapshot.global_memory_cards?.length ?? 0,
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
      extraction: createDemoCandidateSynthesis(draft.extraction, index),
      status: "candidate",
    })),
  };
}

function createDemoCandidateSynthesis(base: ProjectSnapshot["drafts"][number]["extraction"], index: number) {
  if (index === 1) {
    return {
      ...(base ?? {}),
      card_function: "library",
      synthesis_action: "already_covered",
      synthesis_stop_reason: "already_covered",
      value_claim: "这条安全边界已由既有 Memory Card 覆盖，正确动作是归档候选而不是新增重复卡。",
      value_delta: {
        existing_behavior: "既有 Memory Card「本地安全动作」已说明预览模式不访问真实文件系统。",
        missing_part: "未发现新的触发、动作或边界，只是重复同一安全约束。",
        new_behavior: "审阅时选择 No New Card，保持规则库和 Skill 上下文精简。",
        why_not_duplicate: "重复卡会让 agent 在相同安全边界上读取两份相似说明，降低上下文清晰度。",
      },
      target_context: {
        target_type: "memory_card",
        target_id: "safe-actions",
        why_this_target: "该既有 Memory Card 已充分覆盖演示模式写操作边界。",
      },
      suggested_action: {
        action: "already_covered",
        route: "memory_card",
        target_record: "safe-actions",
        record_id: "safe-actions",
        similarity: 0.94,
        reason: "既有 Memory Card 已覆盖同一行为边界。",
        rationale: "No Card Is A Success: 保持规则库精简。",
      },
      synthesis_trace: [
        {
          step: "search_observations",
          summary: "Read the direct evidence about preview write actions.",
          item_ids: ["demo-observation-2"],
        },
        {
          step: "search_memory_cards",
          summary: "Compared 2 Memory Card matches; top `safe-actions` is `already_covered_candidate`.",
          item_ids: ["safe-actions"],
        },
        {
          step: "find_memory_duplicates",
          summary: "Classified the candidate as already covered by the existing Memory Card.",
          item_ids: ["safe-actions"],
        },
        {
          step: "stop",
          summary: "Stopped with already_covered after producing no-new-card decision.",
          item_ids: [],
        },
      ],
    };
  }
  return {
    ...(base ?? {}),
    card_function: "skill_targeted",
    synthesis_action: "skill_targeted_card",
    synthesis_stop_reason: "skill_gap_found",
    value_claim: "补强目标 Skill 的预览验收边界，避免用户只能看到静态卡片却不知道是否可用。",
    value_delta: {
      existing_behavior: "现有预览流程能加载静态数据。",
      missing_part: "缺少一眼可见的验收标准和未来行为改进。",
      new_behavior: "审阅时展示 Value Delta，先判断卡片是否真的改善 Skill 或工作流。",
      why_not_duplicate: "它补上 Skill 的验收边界，不是重复描述预览模式。",
    },
    target_context: {
      target_type: "project_skill",
      target_id: "project:obsidian-markdown",
      why_this_target: "这是一个 Skill-targeted Memory Card 示例，展示如何把提炼结果转成可挂载的 Skill 上下文。",
    },
    skill_usefulness: {
      target_skill_id: "project:obsidian-markdown",
      before_behavior: "Before: Skill `obsidian-markdown` covers note editing and documentation paths.",
      after_behavior: "After: proposal adds boundary and acceptance to the target Skill behavior.",
      improved_axes: ["boundary", "acceptance"],
      missing_axes: [],
      verdict: "counterfactual_pass",
      score: 0.72,
    },
    synthesis_trace: [
      {
        step: "search_observations",
        summary: "Read demo review evidence for the first-run preview gap.",
        item_ids: ["demo-observation-1"],
      },
      {
        step: "search_memory_cards",
        summary: "Compared existing Memory Cards; none fully covers the Skill acceptance boundary.",
        item_ids: ["preview-mode", "safe-actions"],
      },
      {
        step: "search_skills",
        summary: "Matched a project-level Skill target for preview and documentation work.",
        item_ids: ["project:obsidian-markdown"],
      },
      {
        step: "compare_with_skill",
        summary: "Found a concrete Skill gap: the Skill lacks first-run acceptance checks.",
        item_ids: ["project:obsidian-markdown"],
      },
      {
        step: "evaluate_skill_usefulness",
        summary: "Counterfactual pass: after proposal adds boundary and acceptance checks to the target Skill.",
        item_ids: ["project:obsidian-markdown"],
      },
      {
        step: "stop",
        summary: "Stopped with skill_gap_found after producing a targeted card.",
        item_ids: [],
      },
    ],
  };
}

export function createDemoProjectMemoryCardLibrary(projectPath: string): ProjectMemoryCardLibrary {
  const snapshot = createDemoProjectSnapshot(projectPath);
  return {
    project_path: snapshot.project_path,
    memory_cards: [...snapshot.memory_cards],
    global_memory_cards: [...(snapshot.global_memory_cards ?? [])],
    catalog_status: snapshot.catalog_status,
  };
}

export function createDemoProjectSkillLibrary(projectPath: string): ProjectSkillLibrary {
  const snapshot = createDemoProjectSnapshot(projectPath);
  return {
    project_path: snapshot.project_path,
    generated_at: "demo",
    source_counts: { project: 2 },
    skills: [
      {
        id: "project:obsidian-markdown",
        name: "obsidian-markdown",
        description: "Use when editing Obsidian notes with wikilinks, callouts, embeds, and frontmatter.",
        source_path: `${snapshot.project_path}/.github/obsidian-skills/skills/obsidian-markdown`,
        source_kind: "project",
        source_hash: "demo-hash-obsidian-markdown",
        warnings: [],
        mirror_targets: ["codex", "claude-code"],
        linked_memory_cards: [snapshot.memory_cards[0]!],
        recommended_memory_cards: [snapshot.memory_cards[1]!],
      },
      {
        id: "project:json-canvas",
        name: "json-canvas",
        description: "Use when creating or editing JSON Canvas files.",
        source_path: `${snapshot.project_path}/.github/obsidian-skills/skills/json-canvas`,
        source_kind: "project",
        source_hash: "demo-hash-json-canvas",
        warnings: ["description is short; agents may not dispatch it reliably"],
        mirror_targets: ["codex"],
        linked_memory_cards: [],
        recommended_memory_cards: snapshot.memory_cards,
      },
    ],
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

export function createDemoProjectEvalRunView(projectPath: string): ProjectEvalRunView {
  return {
    project_path: projectPath,
    status: "attention",
    provider: "deterministic",
    pipeline_version: 1,
    timestamp: "2026-05-18T12:00:00+08:00",
    recall: { label: "Recall", percent: 86, count: 12, total: 14, status: "pass" },
    precision: { label: "Precision", percent: 94, count: 15, total: 16, status: "pass" },
    one_off_false_positive: { label: "One-off false positives", percent: 6, count: 1, total: 16, status: "fail" },
    duplicate_cluster_risk: { label: "Duplicate risk", percent: 0, count: 0, total: 14, status: "pass" },
    evidence_validity: { label: "Evidence validity", percent: 100, count: 14, total: 14, status: "pass" },
    provider_evidence_validity: null,
    recommendations: [
      "Inspect one-off leaks and raise recurrence requirements for temporary preferences.",
    ],
  };
}
