import type {
  CandidateRecord,
  DraftRecord,
  MemoryCardRecord,
  ProjectAssignmentView,
  ProjectQualityView,
  ArtifactPreviewRow,
} from "../types/domain";

type ReviewRecord = CandidateRecord | DraftRecord;

export type EvidenceSummary = {
  sourceCount: number;
  confidenceLabel: string;
  routeLabel: string;
  compileLabel: string;
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

export type ReviewClosureState = {
  candidateCount: number;
  draftCount: number;
  memoryCardCount: number;
  unassignedCount: number;
  driftWarningCount: number;
  buildActionCount: number;
  nextStep: ReviewClosureStep;
};

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
  const warnings: string[] = [];
  const bundleValidity = record.extraction?.evidence_bundle?.validity;
  const sourceTrust = record.extraction?.evidence_bundle?.source_trust;
  if (sources.size === 0) warnings.push("缺少来源观察");
  if (bundleValidity === "invalid") warnings.push("证据校验失败");
  if (bundleValidity === "weak") warnings.push("证据较弱");
  if (sourceTrust === "assistant_summary" || sourceTrust === "artifact") warnings.push("来源可信度偏低");
  if (!evidencePreview) warnings.push("缺少证据摘要");
  if (confidence > 0 && confidence < 0.72) warnings.push("置信度偏低");

  return {
    sourceCount: sources.size,
    confidenceLabel: confidence > 0 ? `${Math.round(confidence * 100)}%` : "未评分",
    routeLabel: routeLabel(record.extraction?.suggested_action?.route),
    compileLabel: compileLabel(record.extraction?.suggested_action?.compile_enabled),
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
    doneMessage: "已把手动改动导入为待审草稿。",
    description: "把检测到的手动改动转成待审草稿，保留人工判断。",
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
      label: "审查候选",
      detail: `先处理 ${candidateCount} 条系统建议。`,
    };
  }
  if (draftCount > 0) {
    return {
      id: "review-drafts",
      label: "审查草稿",
      detail: `还有 ${draftCount} 条草稿等待批准或拒绝。`,
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
    label: "提炼候选",
    detail: "运行本地提炼，从真实历史中发现可审查建议。",
  };
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
