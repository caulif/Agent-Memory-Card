import type {
  CandidateRecord,
  DraftRecord,
  MemoryCardRecord,
  ProjectDashboard,
  ProjectAssignmentView,
  ProjectQualityView,
  ArtifactPreviewRow,
  SyncCheckpoint,
} from "../types/domain";

type ReviewRecord = CandidateRecord | DraftRecord;

export type EvidenceSummary = {
  sourceCount: number;
  quoteCount: number;
  confidenceLabel: string;
  recurrenceLabel: string;
  riskLabel: string;
  riskTone: "safe" | "review" | "blocked";
  routeLabel: string;
  compileLabel: string;
  actionLabel: string;
  artifactImpactLabel: string;
  status: "grounded" | "weak";
  warnings: string[];
  evidencePreview: string;
};

export type ReviewClosureStep = {
  id:
    | "run-evolution"
    | "review-candidates"
    | "review-drafts"
    | "assign-cards"
    | "resolve-drift"
    | "preview-artifacts";
  label: string;
  detail: string;
};

export type WorkflowProductTerm = {
  implementation: string;
  product: string;
  description: string;
};

export type WorkflowPrimaryAction =
  | "select-project"
  | "run-evolution"
  | "review-suggestions"
  | "approve-memory-cards"
  | "assign-loadout"
  | "resolve-artifact-drift"
  | "preview-artifacts"
  | "sync-artifacts"
  | "monitor-jobs";

export type WorkflowStageId =
  | "select-project"
  | "discover"
  | "review"
  | "approve"
  | "loadout"
  | "artifact-preview"
  | "sync"
  | "verify";

export type WorkflowStage = {
  id: WorkflowStageId;
  productLabel: string;
  implementationTerms: string[];
  primaryAction: WorkflowPrimaryAction;
  done: boolean;
  active: boolean;
  blocked: boolean;
  detail: string;
};

export type WorkflowContractInput = {
  hasProject: boolean;
  dashboard?: ProjectDashboard | null;
  candidates?: CandidateRecord[];
  drafts?: DraftRecord[];
  memoryCards?: Array<MemoryCardRecord | Pick<MemoryCardRecord, "id">>;
  assignment?: ProjectAssignmentView | null;
  quality?: ProjectQualityView | null;
  runningJobCount?: number;
};

export type WorkflowContractState = {
  productTerms: WorkflowProductTerm[];
  stages: WorkflowStage[];
  warnings: WorkflowWarning[];
  primaryAction: {
    id: WorkflowPrimaryAction;
    label: string;
    detail: string;
    targetPage?: "drafts" | "memory-cards" | "agents" | "settings";
  };
};

export type WorkflowWarning = {
  id: string;
  label: string;
  detail: string;
  targetPage: "drafts" | "memory-cards" | "agents" | "settings";
  tone: "warning" | "blocked" | "info";
};

export type ReviewClosureState = {
  candidateCount: number;
  draftCount: number;
  memoryCardCount: number;
  unassignedCount: number;
  driftWarningCount: number;
  buildActionCount: number;
  nextStep: ReviewClosureStep;
};

export const workflowProductTerms: WorkflowProductTerm[] = [
  {
    implementation: "Candidate",
    product: "Suggestion",
    description: "A system-proposed memory item that still needs human review.",
  },
  {
    implementation: "Draft",
    product: "Suggestion",
    description: "A reviewable suggestion; users should not need to distinguish it from Candidate.",
  },
  {
    implementation: "Memory Card",
    product: "Memory Card",
    description: "A human-approved durable memory unit.",
  },
  {
    implementation: "Assignment Matrix",
    product: "Agent Loadout",
    description: "The set of Memory Cards equipped for each target agent.",
  },
  {
    implementation: "Build Preview / Drift",
    product: "Artifact Preview",
    description: "The exact generated-file diff and any safety blockers before sync.",
  },
];

export function buildWorkflowContractState(input: WorkflowContractInput): WorkflowContractState {
  const candidateCount = input.candidates?.filter((candidate) => candidate.status === "candidate").length
    ?? input.dashboard?.candidate_count
    ?? 0;
  const draftCount = input.drafts?.length ?? input.dashboard?.draft_count ?? 0;
  const memoryCards = input.memoryCards ?? [];
  const memoryCardCount = memoryCards.length || input.dashboard?.memory_card_count || 0;
  const observationCount = input.dashboard?.observation_count ?? 0;
  const unassignedCount = countUnassigned(memoryCards, input.assignment ?? null);
  const artifactSummary = summarizeArtifactPreview(input.quality ?? null);
  const runningJobCount = input.runningJobCount ?? 0;

  const hasReviewedSuggestions = candidateCount === 0 && draftCount === 0;
  const hasApprovedCards = memoryCardCount > 0;
  const hasLoadoutReady = hasApprovedCards && unassignedCount === 0;
  const hasArtifactActions = artifactSummary.actionCount > 0;
  const hasBlockingDrift = artifactSummary.status === "blocked";

  const stages: WorkflowStage[] = [
    {
      id: "select-project",
      productLabel: "Select Project",
      implementationTerms: ["ProjectRegistry", "ProjectDashboard"],
      primaryAction: "select-project",
      done: input.hasProject,
      active: !input.hasProject,
      blocked: false,
      detail: input.hasProject ? "A project is active." : "Choose or scan a local project first.",
    },
    {
      id: "discover",
      productLabel: "Discover Suggestions",
      implementationTerms: ["Observation", "Candidate", "Draft"],
      primaryAction: "run-evolution",
      done: candidateCount > 0 || draftCount > 0 || observationCount > 0 || hasApprovedCards,
      active: input.hasProject && candidateCount === 0 && draftCount === 0 && !hasApprovedCards && !hasArtifactActions,
      blocked: !input.hasProject || runningJobCount > 0,
      detail: runningJobCount > 0
        ? `${runningJobCount} background job(s) are running.`
        : "Run local evolution to produce reviewable suggestions.",
    },
    {
      id: "review",
      productLabel: "Review Suggestions",
      implementationTerms: ["Candidate", "Draft", "Review Inbox"],
      primaryAction: "review-suggestions",
      done: hasReviewedSuggestions,
      active: input.hasProject && (candidateCount > 0 || draftCount > 0),
      blocked: hasBlockingDrift,
      detail: `${candidateCount + draftCount} suggestion(s) need review.`,
    },
    {
      id: "approve",
      productLabel: "Approve Memory Cards",
      implementationTerms: ["DraftRecord", "MemoryCardRecord"],
      primaryAction: "approve-memory-cards",
      done: hasApprovedCards,
      active: input.hasProject && hasReviewedSuggestions && !hasApprovedCards,
      blocked: candidateCount > 0 || draftCount > 0,
      detail: hasApprovedCards ? `${memoryCardCount} Memory Card(s) are approved.` : "Approve at least one durable suggestion.",
    },
    {
      id: "loadout",
      productLabel: "Assign Agent Loadout",
      implementationTerms: ["Assignment Matrix", "target_matrix"],
      primaryAction: "assign-loadout",
      done: hasLoadoutReady,
      active: input.hasProject && hasApprovedCards && unassignedCount > 0,
      blocked: !hasApprovedCards,
      detail: `${unassignedCount} Memory Card(s) are not assigned to an agent.`,
    },
    {
      id: "artifact-preview",
      productLabel: "Preview Artifacts",
      implementationTerms: ["BuildPreview", "ArtifactPreviewRow"],
      primaryAction: hasBlockingDrift ? "resolve-artifact-drift" : "preview-artifacts",
      done: hasArtifactActions && !hasBlockingDrift,
      active: input.hasProject && hasLoadoutReady && (hasArtifactActions || hasBlockingDrift),
      blocked: hasBlockingDrift || !hasLoadoutReady,
      detail: artifactSummary.detail,
    },
    {
      id: "sync",
      productLabel: "Sync Artifacts",
      implementationTerms: ["sync_project", "AGENTS.md", "CLAUDE.md"],
      primaryAction: "sync-artifacts",
      done: false,
      active: input.hasProject && hasLoadoutReady && hasArtifactActions && !hasBlockingDrift,
      blocked: hasBlockingDrift || !hasArtifactActions,
      detail: hasArtifactActions ? "Generated file changes are ready for explicit sync." : "No artifact changes are ready yet.",
    },
    {
      id: "verify",
      productLabel: "Verify And Recover",
      implementationTerms: ["Rule CI", "StatusReport", "JobCenter"],
      primaryAction: "monitor-jobs",
      done: false,
      active: input.hasProject && runningJobCount > 0,
      blocked: false,
      detail: "Check jobs, rule results, and drift recovery after sync.",
    },
  ];

  return {
    productTerms: workflowProductTerms,
    stages,
    warnings: buildWorkflowWarnings({
      candidateCount,
      draftCount,
      unassignedCount,
      artifactSummary,
      runningJobCount,
    }),
    primaryAction: chooseWorkflowPrimaryAction(stages),
  };
}

export type ArtifactPreviewSummary = {
  status: "empty" | "ready" | "blocked";
  actionCount: number;
  warningCount: number;
  actions: string[];
  warnings: string[];
  targets: ArtifactPreviewTarget[];
  blockingTargets: ArtifactPreviewTarget[];
  recoveryAction?: ArtifactPreviewRecoveryAction;
  recoveryActions?: ArtifactPreviewRecoveryAction[];
  lastSync?: SyncCheckpoint | null;
  headline: string;
  detail: string;
};

export type ArtifactPreviewRecoveryAction = {
  actionKey: string;
  label: string;
  command: string;
  doneMessage: string;
  description: string;
  tone: "safe" | "destructive";
  confirmMessage?: string;
};

export type ArtifactPreviewTarget = {
  kind: "artifact" | "skill";
  label: string;
  path: string;
  status?: string;
  agent?: string;
  diffPreview?: string[];
  diffLines?: string[];
  diffTruncated?: boolean;
};

export type ArtifactBlockingTargetPage = {
  items: ArtifactPreviewTarget[];
  page: number;
  pageSize: number;
  total: number;
  totalPages: number;
  hasPrevious: boolean;
  hasNext: boolean;
};

export function summarizeCandidateEvidence(candidate: CandidateRecord): EvidenceSummary {
  return summarizeReviewEvidence(candidate);
}

export function summarizeDraftEvidence(draft: DraftRecord): EvidenceSummary {
  return summarizeReviewEvidence(draft);
}

function summarizeReviewEvidence(record: ReviewRecord): EvidenceSummary {
  const sources = new Set<string>();
  for (const id of record.extraction?.source_observations ?? []) {
    if (id.trim()) sources.add(id.trim());
  }
  for (const id of record.extraction?.evidence_bundle?.source_observation_ids ?? []) {
    if (id.trim()) sources.add(id.trim());
  }
  for (const quote of record.extraction?.evidence_bundle?.quotes ?? []) {
    if (quote.observation_id?.trim()) sources.add(quote.observation_id.trim());
  }
  if ("source_observations" in record) {
    for (const id of record.source_observations ?? []) {
      if (id.trim()) sources.add(id.trim());
    }
  }

  const evidencePreview = (record.evidence ?? "").trim();
  const confidence = record.confidence ?? 0;
  const quotes = record.extraction?.evidence_bundle?.quotes ?? [];
  const warnings: string[] = [];
  const bundleValidity = record.extraction?.evidence_bundle?.validity;
  const sourceTrust = record.extraction?.evidence_bundle?.source_trust;
  const suggestedAction = record.extraction?.suggested_action;
  if (sources.size === 0) warnings.push("缺少来源观察");
  if (bundleValidity === "invalid") warnings.push("证据校验失败");
  if (bundleValidity === "weak") warnings.push("证据较弱");
  if (sourceTrust === "assistant_summary" || sourceTrust === "artifact") warnings.push("来源可信度偏低");
  if (!evidencePreview) warnings.push("缺少证据摘要");
  if (confidence > 0 && confidence < 0.72) warnings.push("置信度偏低");

  return {
    sourceCount: sources.size,
    quoteCount: quotes.length,
    confidenceLabel: confidence > 0 ? `${Math.round(confidence * 100)}%` : "未评分",
    recurrenceLabel: recurrenceLabel(sources.size, quotes.length),
    riskLabel: riskLabel(warnings, confidence, sources.size, suggestedAction?.action),
    riskTone: riskTone(warnings, confidence, sources.size),
    routeLabel: routeLabel(suggestedAction?.route),
    compileLabel: compileLabel(suggestedAction?.compile_enabled),
    actionLabel: extractionActionLabel(suggestedAction?.action),
    artifactImpactLabel: artifactImpactLabel(suggestedAction?.route, suggestedAction?.compile_enabled),
    status: warnings.length === 0 && bundleValidity !== "weak" ? "grounded" : "weak",
    warnings,
    evidencePreview,
  };
}

export function buildReviewClosureState({
  candidates,
  drafts,
  memoryCards,
  assignment,
  quality,
}: {
  candidates: CandidateRecord[];
  drafts: DraftRecord[];
  memoryCards: Array<MemoryCardRecord | Pick<MemoryCardRecord, "id">>;
  assignment: ProjectAssignmentView | null;
  quality: ProjectQualityView | null;
}): ReviewClosureState {
  const unassignedCount = countUnassigned(memoryCards, assignment);
  const driftWarningCount = (quality?.status?.warnings ?? []).filter((warning) =>
    warning.toLowerCase().includes("drift"),
  ).length;
  const buildActionCount = quality?.build_preview?.actions?.length ?? 0;

  return {
    candidateCount: candidates.length,
    draftCount: drafts.length,
    memoryCardCount: memoryCards.length,
    unassignedCount,
    driftWarningCount,
    buildActionCount,
    nextStep: chooseNextStep({
      candidateCount: candidates.length,
      draftCount: drafts.length,
      unassignedCount,
      driftWarningCount,
      buildActionCount,
    }),
  };
}

export function summarizeArtifactPreview(quality: ProjectQualityView | null): ArtifactPreviewSummary {
  const actions = quality?.build_preview?.actions ?? [];
  const artifactRows = quality?.build_preview?.artifact_previews ?? [];
  const statusWarnings = quality?.status?.warnings ?? [];
  const buildWarnings = quality?.build_preview?.warnings ?? [];
  const lastSync = quality?.status?.last_sync ?? null;
  const warnings = [...statusWarnings, ...buildWarnings];
  const rowDriftCount = artifactRows.filter((row) => isBlockingArtifactStatus(row.status)).length;
  const warningDriftCount = statusWarnings.filter((warning) => warning.toLowerCase().includes("drift")).length;
  const driftCount = rowDriftCount + warningDriftCount;
  const targets =
    artifactRows.length > 0 ? artifactRows.map(structuredArtifactTarget) : actions.map(parseArtifactTarget);
  const actionCount = artifactRows.length > 0 ? artifactRows.filter((row) => row.status !== "unchanged").length : actions.length;

  if (actionCount === 0 && warnings.length === 0) {
    return {
      status: "empty",
      actionCount: 0,
      warningCount: 0,
      actions: [],
      warnings: [],
      targets: [],
      blockingTargets: [],
      headline: "暂无写入动作",
      detail: "批准并分配 Memory Card 后，这里会显示将要生成或更新的 Agent 文件。",
    };
  }

  if (driftCount > 0) {
    return {
      status: "blocked",
      actionCount,
      warningCount: warnings.length,
      actions,
      warnings,
      targets,
      blockingTargets: targets.filter((target) => isBlockingArtifactStatus(target.status ?? "")),
      recoveryAction: driftRecoveryActions[0],
      recoveryActions: driftRecoveryActions,
      lastSync,
      headline: "写入前需处理 Drift",
      detail: `${actionCount} 个生成动作已计算，但 ${driftCount} 个 drift 警告会阻止安全写入。`,
    };
  }

  return {
    status: "ready",
    actionCount,
    warningCount: warnings.length,
    actions,
    warnings,
    targets,
    blockingTargets: [],
    lastSync,
    headline: "可预览写入",
    detail: `${actionCount} 个生成动作等待检查，写入前请确认目标 Agent 文件变化。`,
  };
}

export function paginateArtifactBlockingTargets(
  targets: ArtifactPreviewTarget[],
  page: number,
  pageSize = 6,
): ArtifactBlockingTargetPage {
  const safePageSize = Math.max(1, pageSize);
  const totalPages = Math.max(1, Math.ceil(targets.length / safePageSize));
  const clampedPage = Math.min(Math.max(0, page), totalPages - 1);
  const start = clampedPage * safePageSize;
  return {
    items: targets.slice(start, start + safePageSize),
    page: clampedPage,
    pageSize: safePageSize,
    total: targets.length,
    totalPages,
    hasPrevious: clampedPage > 0,
    hasNext: clampedPage < totalPages - 1,
  };
}

const driftRecoveryActions: ArtifactPreviewRecoveryAction[] = [
  {
    actionKey: "导入 Drift",
    label: "导入为草稿",
    command: "import_artifact_drifts",
    doneMessage: "已把手动改动导入为待审建议。",
    description: "把检测到的手动改动转成待审建议，保留人工判断。",
    tone: "safe",
  },
  {
    actionKey: "保留 Drift",
    label: "保留手改",
    command: "keep_artifact_drifts",
    doneMessage: "已接受当前生成文件的手动改动。",
    description: "接受当前文件内容，并更新 artifact lock，不改写手动内容。",
    tone: "safe",
  },
  {
    actionKey: "丢弃 Drift",
    label: "丢弃手改",
    command: "discard_artifact_drifts",
    doneMessage: "已恢复为 Memory Card 生成的文件内容。",
    description: "用 Memory Card 生成内容覆盖 drift 文件，会丢弃当前手动改动。",
    tone: "destructive",
    confirmMessage: "这会丢弃当前 drift 文件里的手动改动，并恢复为 Memory Card 生成内容。确定继续吗？",
  },
];

function structuredArtifactTarget(row: ArtifactPreviewRow): ArtifactPreviewTarget {
  return {
    kind: row.kind.includes(":skills") ? "skill" : "artifact",
    label: `${row.status} · ${row.path}`,
    path: row.path,
    status: row.status,
    agent: row.agent,
    diffPreview: row.diff_preview,
    diffLines: row.diff_lines ?? row.diff_preview,
    diffTruncated: row.diff_truncated ?? false,
  };
}

function isBlockingArtifactStatus(status: string): boolean {
  return status === "drifted" || status === "unmanaged";
}

function parseArtifactTarget(action: string): ArtifactPreviewTarget {
  const mirrorMatch = action.match(/(?:Would mirror|Mirrored) skill `([^`]+)` -> (.+)$/);
  if (mirrorMatch) {
    return {
      kind: "skill",
      label: mirrorMatch[1] ?? action,
      path: (mirrorMatch[2] ?? action).trim(),
    };
  }

  const artifactMatch = action.match(/(?:Would write|Wrote|Write|Update)\s+(.+)$/);
  const path = (artifactMatch?.[1] ?? action).trim();
  return {
    kind: "artifact",
    label: action,
    path,
  };
}

function chooseNextStep({
  candidateCount,
  draftCount,
  unassignedCount,
  driftWarningCount,
  buildActionCount,
}: {
  candidateCount: number;
  draftCount: number;
  unassignedCount: number;
  driftWarningCount: number;
  buildActionCount: number;
}): ReviewClosureStep {
  if (driftWarningCount > 0) {
    return {
      id: "resolve-drift",
      label: "先处理 Drift",
      detail: `${driftWarningCount} 个生成产物存在手动改动，写入前需要导入或确认。`,
    };
  }
  if (candidateCount > 0) {
    return {
      id: "review-candidates",
      label: "审查建议",
      detail: `先处理 ${candidateCount} 条系统建议。`,
    };
  }
  if (draftCount > 0) {
    return {
      id: "review-drafts",
      label: "审查建议",
      detail: `还有 ${draftCount} 条建议等待批准或拒绝。`,
    };
  }
  if (unassignedCount > 0) {
    return {
      id: "assign-cards",
      label: "分配 Memory Card",
      detail: `${unassignedCount} 张卡尚未分配给 Agent。`,
    };
  }
  if (buildActionCount > 0) {
    return {
      id: "preview-artifacts",
      label: "预览写入",
      detail: `${buildActionCount} 个生成产物动作等待检查。`,
    };
  }
  return {
    id: "run-evolution",
    label: "提炼建议",
    detail: "运行本地提炼，从真实历史中发现可审查建议。",
  };
}

function chooseWorkflowPrimaryAction(stages: WorkflowStage[]): WorkflowContractState["primaryAction"] {
  const active = stages.find((stage) => stage.active && !stage.blocked) ?? stages.find((stage) => stage.active);
  const blockedDrift = stages.find((stage) => stage.id === "artifact-preview" && stage.blocked && stage.active);
  const blocked = blockedDrift ?? stages.find((stage) => stage.active && stage.blocked);
  const stage = blocked ?? active ?? stages.find((item) => !item.done) ?? stages[stages.length - 1];
  if (!stage) {
    return {
      id: "monitor-jobs",
      label: "检查状态",
      detail: "查看 Job Center、质量状态和生成产物。",
    };
  }

  switch (stage.primaryAction) {
    case "select-project":
      return {
        id: "select-project",
        label: "选择项目",
        detail: "先扫描或打开一个本地 Claude Code / Codex 项目。",
      };
    case "run-evolution":
      return {
        id: "run-evolution",
        label: "提炼建议",
        detail: stage.detail,
        targetPage: "drafts",
      };
    case "review-suggestions":
      return {
        id: "review-suggestions",
        label: "审阅建议",
        detail: stage.detail,
        targetPage: "drafts",
      };
    case "approve-memory-cards":
      return {
        id: "approve-memory-cards",
        label: "批准记忆卡",
        detail: stage.detail,
        targetPage: "drafts",
      };
    case "assign-loadout":
      return {
        id: "assign-loadout",
        label: "配置 Agent Loadout",
        detail: stage.detail,
        targetPage: "agents",
      };
    case "resolve-artifact-drift":
      return {
        id: "resolve-artifact-drift",
        label: "处理 Artifact Drift",
        detail: stage.detail,
        targetPage: "drafts",
      };
    case "preview-artifacts":
      return {
        id: "preview-artifacts",
        label: "预览 Artifact Diff",
        detail: stage.detail,
        targetPage: "agents",
      };
    case "sync-artifacts":
      return {
        id: "sync-artifacts",
        label: "同步 Agent 文件",
        detail: stage.detail,
        targetPage: "agents",
      };
    case "monitor-jobs":
      return {
        id: "monitor-jobs",
        label: "检查任务状态",
        detail: stage.detail,
      };
  }
}

function buildWorkflowWarnings({
  candidateCount,
  draftCount,
  unassignedCount,
  artifactSummary,
  runningJobCount,
}: {
  candidateCount: number;
  draftCount: number;
  unassignedCount: number;
  artifactSummary: ArtifactPreviewSummary;
  runningJobCount: number;
}): WorkflowWarning[] {
  const warnings: WorkflowWarning[] = [];
  const suggestionCount = candidateCount + draftCount;
  if (suggestionCount > 0) {
    warnings.push({
      id: "pending-suggestions",
      label: `${suggestionCount} 条待审建议`,
      detail: "先确认来源证据，再批准为 Memory Card。",
      targetPage: "drafts",
      tone: "info",
    });
  }
  if (artifactSummary.status === "blocked") {
    warnings.push({
      id: "artifact-drift",
      label: `${artifactSummary.blockingTargets.length || artifactSummary.warningCount} 个 Artifact Drift`,
      detail: "写入前需要导入、保留或丢弃手动改动。",
      targetPage: "drafts",
      tone: "blocked",
    });
  }
  if (unassignedCount > 0) {
    warnings.push({
      id: "unassigned-loadout",
      label: `${unassignedCount} 张卡未配置 Loadout`,
      detail: "批准后的 Memory Card 需要分配给 Codex 或 Claude Code。",
      targetPage: "agents",
      tone: "warning",
    });
  }
  if (artifactSummary.status === "ready") {
    warnings.push({
      id: "artifact-preview",
      label: `${artifactSummary.actionCount} 个 Artifact 动作`,
      detail: "同步前检查 AGENTS.md / CLAUDE.md / Skill diff。",
      targetPage: "agents",
      tone: "info",
    });
  }
  if (runningJobCount > 0) {
    warnings.push({
      id: "running-jobs",
      label: `${runningJobCount} 个后台任务`,
      detail: "查看 Job Center 的进度、失败或重试动作。",
      targetPage: "drafts",
      tone: "info",
    });
  }
  return warnings;
}

function countUnassigned(
  memoryCards: Array<MemoryCardRecord | Pick<MemoryCardRecord, "id">>,
  assignment: ProjectAssignmentView | null,
): number {
  if (memoryCards.length === 0) return 0;
  const rows = assignment?.target_matrix?.rows ?? [];
  return memoryCards.filter((card) => {
    const row = rows.find((item) => item.memory_card_id === card.id);
    if (!row) return true;
    return !Object.values(row.targets ?? {}).some(Boolean);
  }).length;
}

export function routeLabel(route?: string | null): string {
  switch (route) {
    case "always_on_rule":
      return "写入规则";
    case "workflow_skill":
      return "生成 Skill 草稿";
    case "skill_supplement":
      return "补充 Skill";
    case "review_only":
      return "仅审阅";
    default:
      return "待分流";
  }
}

export function compileLabel(enabled?: boolean | null): string {
  return enabled ? "会编译" : "不编译";
}

export function extractionActionLabel(action?: string | null): string {
  switch (action) {
    case "merge_into_existing":
      return "合并到现有 Memory Card";
    case "new_candidate":
      return "新 Memory Card";
    case "review_only":
      return "仅审阅";
    default:
      return "待审动作";
  }
}

function recurrenceLabel(sourceCount: number, quoteCount: number): string {
  if (sourceCount >= 2) return `复现证据 ${sourceCount} 来源`;
  if (sourceCount === 1 && quoteCount > 1) return `单来源 ${quoteCount} 引文`;
  if (sourceCount === 1) return "单次证据";
  return "无来源";
}

function riskTone(warnings: string[], confidence: number, sourceCount: number): EvidenceSummary["riskTone"] {
  if (warnings.includes("证据校验失败") || sourceCount === 0) return "blocked";
  if (warnings.length > 0 || (confidence > 0 && confidence < 0.72) || sourceCount === 1) return "review";
  return "safe";
}

function riskLabel(
  warnings: string[],
  confidence: number,
  sourceCount: number,
  action?: string | null,
): string {
  const tone = riskTone(warnings, confidence, sourceCount);
  if (tone === "blocked") return "高风险";
  if (action === "merge_into_existing") return "合并需复核";
  if (tone === "review") return "需复核";
  return "低风险";
}

function artifactImpactLabel(route?: string | null, compileEnabled?: boolean | null): string {
  if (compileEnabled === false) return `${routeLabel(route)}，不会写入 Agent 文件`;
  return `${routeLabel(route)}，${compileLabel(compileEnabled)}`;
}
