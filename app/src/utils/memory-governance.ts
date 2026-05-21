import type { MemoryCardRecord, ProjectAssignmentView } from "../types/domain";

export type MemoryCardGovernance = {
  assignedTargets: string[];
  status: "ready" | "needs-review";
  lifecycle: "active" | "dormant" | "expired";
  sourceLabel: string;
  warnings: string[];
  mergeCount: number;
  lineage: MemoryCardLineageItem[];
  conflictDetails: MemoryCardConflictDetail[];
  mergeDraftAction?: MemoryCardMergeDraftAction;
  lifecycleAction?: MemoryCardLifecycleAction;
  expireAction?: MemoryCardLifecycleAction;
};

export type MemoryCardLineageItem = {
  label: string;
  value: string;
};

export type MemoryCardConflictDetail = {
  id: string;
  title: string;
  similarityLabel: string;
};

export type MemoryGovernanceSummary = {
  total: number;
  assigned: number;
  unassigned: number;
  missingSource: number;
  needsReview: number;
  conflicts: number;
  dormant: number;
  expired: number;
};

export type MemoryGovernanceFilter =
  | "all"
  | "needs-review"
  | "unassigned"
  | "missing-source"
  | "conflicts"
  | "dormant"
  | "expired";

export type MemoryCardMergeDraftAction = {
  actionKey: string;
  label: string;
  command: string;
  doneMessage: string;
  input: {
    id: string;
    title: string;
    sources: string[];
    targets: string[];
  };
};

export type MemoryCardLifecycleAction = {
  actionKey: string;
  label: string;
  command: string;
  doneMessage: string;
  input: {
    id: string;
    tags: string[];
  };
};

export type MemoryGovernanceBatchAction = {
  actionKey: string;
  label: string;
  description: string;
  steps: MemoryGovernanceBatchStep[];
};

export type MemoryGovernanceBatchStep = {
  actionKey: string;
  command: string;
  doneMessage: string;
  args: Record<string, unknown>;
};

export function summarizeMemoryCardGovernance(
  card: MemoryCardRecord,
  assignment: ProjectAssignmentView | null,
  library: MemoryCardRecord[] = [],
): MemoryCardGovernance {
  const row = assignment?.target_matrix.rows.find((item) => item.memory_card_id === card.id);
  const assignedTargets = Object.entries(row?.targets ?? {})
    .filter(([, enabled]) => enabled)
    .map(([target]) => target);
  const hasSource = Boolean(card.evidence?.trim() || card.approved_from || card.extraction?.source_observations?.length);
  const warnings: string[] = [];
  const duplicates = likelyDuplicateCards(card, library);
  const lifecycle = lifecycleState(card);
  if (assignedTargets.length === 0) warnings.push("未分配给 Agent");
  if (!hasSource) warnings.push("缺少来源证据");
  if (lifecycle === "dormant") warnings.push("已休眠");
  if (lifecycle === "expired") warnings.push("已过期");
  for (const conflict of duplicates) {
    warnings.push(`可能与 ${conflict.card.title} 重复`);
  }

  return {
    assignedTargets,
    status: warnings.length === 0 ? "ready" : "needs-review",
    lifecycle,
    sourceLabel: sourceLabel(card, hasSource),
    warnings,
    mergeCount: card.merge_history?.length ?? 0,
    lineage: buildLineage(card),
    conflictDetails: duplicates.map((item) => ({
      id: item.card.id,
      title: item.card.title,
      similarityLabel: `${Math.round(item.score * 100)}%`,
    })),
    mergeDraftAction: duplicates.length > 0 ? buildMergeDraftAction(card, duplicates.map((item) => item.card), assignedTargets) : undefined,
    lifecycleAction: buildLifecycleAction(card, lifecycle),
    expireAction: lifecycle === "expired" ? undefined : buildExpireAction(card),
  };
}

export function buildMemoryGovernanceSummary(
  cards: MemoryCardRecord[],
  assignment: ProjectAssignmentView | null,
): MemoryGovernanceSummary {
  const governance = cards.map((card) => summarizeMemoryCardGovernance(card, assignment, cards));
  return {
    total: cards.length,
    assigned: governance.filter((item) => item.assignedTargets.length > 0).length,
    unassigned: governance.filter((item) => item.assignedTargets.length === 0).length,
    missingSource: governance.filter((item) => item.sourceLabel === "缺少来源").length,
    needsReview: governance.filter((item) => item.status === "needs-review").length,
    conflicts: governance.filter((item) => item.warnings.some((warning) => warning.includes("重复"))).length,
    dormant: governance.filter((item) => item.lifecycle === "dormant").length,
    expired: governance.filter((item) => item.lifecycle === "expired").length,
  };
}

export function filterMemoryCardsByGovernance(
  cards: MemoryCardRecord[],
  filter: MemoryGovernanceFilter,
  assignment: ProjectAssignmentView | null,
): MemoryCardRecord[] {
  if (filter === "all") return cards;
  return cards.filter((card) => {
    const governance = summarizeMemoryCardGovernance(card, assignment, cards);
    switch (filter) {
      case "needs-review":
        return governance.status === "needs-review";
      case "unassigned":
        return governance.assignedTargets.length === 0;
      case "missing-source":
        return governance.sourceLabel === "缺少来源";
      case "conflicts":
        return governance.conflictDetails.length > 0;
      case "dormant":
        return governance.lifecycle === "dormant";
      case "expired":
        return governance.lifecycle === "expired";
      default:
        return true;
    }
  });
}

export function buildMemoryGovernanceBatchActions(
  visibleCards: MemoryCardRecord[],
  activeFilter: MemoryGovernanceFilter,
  assignment: ProjectAssignmentView | null,
  library: MemoryCardRecord[] = visibleCards,
): MemoryGovernanceBatchAction[] {
  const actions: MemoryGovernanceBatchAction[] = [];
  const visibleGovernance = visibleCards.map((card) => ({
    card,
    governance: summarizeMemoryCardGovernance(card, assignment, library),
  }));

  const restoreSteps = visibleGovernance
    .filter(({ governance }) => governance.lifecycle !== "active" && governance.lifecycleAction)
    .map(({ governance }) => lifecycleStep(governance.lifecycleAction!));
  if ((activeFilter === "dormant" || activeFilter === "expired" || activeFilter === "needs-review") && restoreSteps.length > 1) {
    actions.push({
      actionKey: `批量恢复活跃-${activeFilter}`,
      label: "批量恢复活跃",
      description: `将当前筛选中的 ${restoreSteps.length} 张休眠/过期卡恢复为活跃。`,
      steps: restoreSteps,
    });
  }

  const mergeStepsBySignature = new Map<string, MemoryGovernanceBatchStep>();
  for (const { governance } of visibleGovernance) {
    const action = governance.mergeDraftAction;
    if (!action) continue;
    const signature = [...action.input.sources].sort().join("|");
    if (mergeStepsBySignature.has(signature)) continue;
    mergeStepsBySignature.set(signature, {
      actionKey: action.actionKey,
      command: action.command,
      doneMessage: action.doneMessage,
      args: { input: action.input },
    });
  }
  const mergeSteps = Array.from(mergeStepsBySignature.values());
  if ((activeFilter === "conflicts" || activeFilter === "needs-review") && mergeSteps.length > 1) {
    actions.push({
      actionKey: `批量生成合并草稿-${activeFilter}`,
      label: "批量生成合并草稿",
      description: `为当前筛选中的 ${mergeSteps.length} 组疑似重复卡生成审查草稿。`,
      steps: mergeSteps,
    });
  }

  return actions;
}

function sourceLabel(card: MemoryCardRecord, hasSource: boolean): string {
  if (!hasSource) return "缺少来源";
  if (card.approved_from) return `来自 ${card.approved_from}`;
  const sourceCount = card.extraction?.source_observations?.length ?? 0;
  if (sourceCount > 0) return `${sourceCount} 条来源观察`;
  return "有证据摘要";
}

function buildLineage(card: MemoryCardRecord): MemoryCardLineageItem[] {
  const lineage: MemoryCardLineageItem[] = [];
  if (card.approved_from) {
    lineage.push({ label: "批准来源", value: card.approved_from });
  }
  if (card.evidence?.trim()) {
    lineage.push({ label: "证据摘要", value: card.evidence.trim() });
  }
  for (const item of card.merge_history ?? []) {
    lineage.push({
      label: "合并来源",
      value: `${item.source_id} · ${item.action} · ${item.merged_at}`,
    });
  }
  return lineage;
}

function lifecycleState(card: MemoryCardRecord): MemoryCardGovernance["lifecycle"] {
  const tags = (card.tags ?? []).map((tag) => tag.toLowerCase().trim());
  if (tags.some((tag) => tag === "status:expired" || tag === "lifecycle:expired")) {
    return "expired";
  }
  if (tags.some((tag) => tag === "status:dormant" || tag === "lifecycle:dormant")) {
    return "dormant";
  }
  return "active";
}

function buildLifecycleAction(
  card: MemoryCardRecord,
  lifecycle: MemoryCardGovernance["lifecycle"],
): MemoryCardLifecycleAction {
  const currentTags = card.tags ?? [];
  const nonLifecycleTags = currentTags.filter((tag) => !isLifecycleTag(tag));
  if (lifecycle === "active") {
    return {
      actionKey: `标记休眠-${card.id}`,
      label: "标记休眠",
      command: "update_memory_card",
      doneMessage: "已标记为休眠。",
      input: { id: card.id, tags: [...nonLifecycleTags, "status:dormant"] },
    };
  }

  return {
    actionKey: `恢复活跃-${card.id}`,
    label: "恢复活跃",
    command: "update_memory_card",
    doneMessage: "已恢复为活跃。",
    input: { id: card.id, tags: nonLifecycleTags },
  };
}

function buildExpireAction(card: MemoryCardRecord): MemoryCardLifecycleAction {
  const nonLifecycleTags = (card.tags ?? []).filter((tag) => !isLifecycleTag(tag));
  return {
    actionKey: `标记过期-${card.id}`,
    label: "标记过期",
    command: "update_memory_card",
    doneMessage: "已标记为过期。",
    input: { id: card.id, tags: [...nonLifecycleTags, "status:expired"] },
  };
}

function lifecycleStep(action: MemoryCardLifecycleAction): MemoryGovernanceBatchStep {
  return {
    actionKey: action.actionKey,
    command: action.command,
    doneMessage: action.doneMessage,
    args: { id: action.input.id, input: { tags: action.input.tags } },
  };
}

function isLifecycleTag(tag: string): boolean {
  const normalized = tag.toLowerCase().trim();
  return (
    normalized === "status:dormant" ||
    normalized === "lifecycle:dormant" ||
    normalized === "status:expired" ||
    normalized === "lifecycle:expired"
  );
}

function likelyDuplicateCards(
  card: MemoryCardRecord,
  library: MemoryCardRecord[],
): Array<{ card: MemoryCardRecord; score: number }> {
  if (library.length < 2) return [];
  return library
    .filter((candidate) => candidate.id !== card.id)
    .map((candidate) => ({ card: candidate, score: duplicateScore(card, candidate) }))
    .filter((candidate) => candidate.score >= 0.26)
    .sort((left, right) => right.score - left.score)
    .slice(0, 2);
}

function buildMergeDraftAction(
  card: MemoryCardRecord,
  duplicates: MemoryCardRecord[],
  targets: string[],
): MemoryCardMergeDraftAction {
  const sources = [card.id, ...duplicates.map((item) => item.id)];
  const title = `合并：${card.title}`;
  const slug = sanitizeId(sources.join("-"));
  return {
    actionKey: `生成合并草稿-${card.id}`,
    label: "生成合并草稿",
    command: "fuse_memory_cards_to_draft",
    doneMessage: "已生成合并草稿，请在审查队列确认。",
    input: {
      id: `draft:fuse-${slug}`,
      title,
      sources,
      targets,
    },
  };
}

function duplicateScore(a: MemoryCardRecord, b: MemoryCardRecord): number {
  const left = bigrams(`${a.title} ${a.body}`);
  const right = bigrams(`${b.title} ${b.body}`);
  if (left.size === 0 || right.size === 0) return 0;

  let overlap = 0;
  for (const item of left) {
    if (right.has(item)) overlap += 1;
  }
  const union = left.size + right.size - overlap;
  return union === 0 ? 0 : overlap / union;
}

function bigrams(value: string): Set<string> {
  const normalized = value
    .toLowerCase()
    .replace(/[^\p{Letter}\p{Number}]+/gu, "")
    .trim();
  const grams = new Set<string>();
  for (let index = 0; index < normalized.length - 1; index += 1) {
    grams.add(normalized.slice(index, index + 2));
  }
  return grams;
}

function sanitizeId(value: string): string {
  return value
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, "-")
    .replace(/-+/g, "-")
    .replace(/^-|-$/g, "")
    .slice(0, 80);
}
