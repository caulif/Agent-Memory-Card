import type { DraftRecord, MemoryCardRecord, ProjectSnapshot, EvolutionInsight, ProjectCandidateInbox } from "../types/domain";
import { translateKind, translateScope } from "./formatting";

export function describeMemoryCardPlainly(memory_card: MemoryCardRecord): string {
  const kindLabel = translateKind(memory_card.kind);
  const scopeLabel = translateScope(memory_card.scope);
  return `这是一个${scopeLabel}级别的${kindLabel}：${memory_card.title}。${memory_card.body} 以后遇到类似任务，可以直接复用该片段，无需重新描述。`;
}

export function describeDraftForReview(draft: DraftRecord): string {
  if (draft.brief?.trim()) {
    return draft.brief.trim();
  }

  const kindLabel = translateKind(draft.kind);
  const scopeLabel = translateScope(draft.scope);
  const content = summarizeDraftContent(draft.body || draft.title);
  return clampChineseBrief(`这条草稿沉淀了${scopeLabel}级${kindLabel}：${content}`);
}

function summarizeDraftContent(input: string): string {
  let text = input
    .replace(/\s+/g, " ")
    .replace(/\bfrontend\b/gi, "前端")
    .replace(/\bbackend\b/gi, "后端")
    .replace(/\bHTTP requests?\b/g, "HTTP 请求")
    .replace(/\binstead of\b/gi, "替代")
    .replace(/\bUse\b/g, "使用")
    .replace(/\bPrefer\b/g, "优先使用")
    .replace(/\bAvoid\b/g, "避免")
    .trim();
  if (!/[。.!?！？]$/.test(text)) {
    text = `${text}。`;
  }
  return text;
}

function clampChineseBrief(input: string): string {
  if (input.length <= 90) return input;
  return `${input.slice(0, 87).trimEnd()}...`;
}

const evolutionTimestamps = [
  "2026-04-28",
  "2026-04-29",
  "2026-04-30",
  "2026-05-01",
  "2026-05-02",
];

export function deriveMemoryCardEvolution(
  memory_card: MemoryCardRecord,
  snapshot: ProjectSnapshot,
): EvolutionInsight {
  const targetRow = snapshot.target_matrix.rows.find((r) => r.memory_card_id === memory_card.id);
  const agentCount = targetRow
    ? Object.values(targetRow.targets).filter(Boolean).length
    : 0;

  const relatedDrafts = snapshot.drafts.filter((d) =>
    d.targets?.some((t) => t === "codex" || t === "claude-code"),
  );
  const draftCount = relatedDrafts.length;

  const confidence = Math.min(0.95, +(0.48 + draftCount * 0.07 + agentCount * 0.08).toFixed(2));

  const stability = Math.min(
    0.98,
    +(0.42 + (memory_card.scope === "global" ? 0.22 : memory_card.scope === "project" ? 0.14 : 0.04) + agentCount * 0.06).toFixed(2),
  );

  const activity: "active" | "dormant" = draftCount > 0 || agentCount >= 2 ? "active" : "dormant";

  const conflicts: string[] = [];
  if (agentCount >= 2 && memory_card.scope === "project") {
    conflicts.push("多智能体共享项目级片段，需确认目标智能体对该约束或流程理解一致");
  }
  if (memory_card.kind === "constraint" && agentCount <= 1) {
    conflicts.push("硬约束仅分配给单个智能体，建议扩展覆盖范围以避免约束遗漏");
  }
  if (memory_card.kind === "procedure" && memory_card.scope === "agent") {
    conflicts.push("智能体级流程片段可能与其他智能体的同名流程冲突，需检查一致性");
  }

  const promotion_candidate =
    memory_card.scope === "project" && agentCount >= 2 && confidence > 0.6;

  const timeline: Array<{ date: string; event: string }> = [];
  const baseDateIdx = Math.min(
    evolutionTimestamps.length - 1,
    Math.max(0, Math.floor(draftCount / 2)),
  );
  timeline.push({
    date: evolutionTimestamps[baseDateIdx]!,
    event: `技能片段"${memory_card.title}"被提取并注册`,
  });
  if (draftCount > 0) {
    timeline.push({
      date: evolutionTimestamps[Math.min(baseDateIdx + 1, evolutionTimestamps.length - 1)]!,
      event: `关联 ${draftCount} 条相关草稿进入审核队列`,
    });
  }
  timeline.push({
    date: evolutionTimestamps[evolutionTimestamps.length - 1]!,
    event:
      agentCount >= 2
        ? `已分配给 ${agentCount} 个智能体协同使用`
        : agentCount === 1
          ? "当前分配给单个智能体"
          : "尚未分配目标智能体",
  });

  const evolution_tree: EvolutionInsight["evolution_tree"] = [];
  if (memory_card.scope === "project" || memory_card.scope === "global") {
    evolution_tree.push({
      from: "原始会话任务",
      to: memory_card.title,
      label: memory_card.scope === "global" ? "提炼为全局级片段" : "提炼为项目级片段",
    });
  } else {
    evolution_tree.push({
      from: "智能体特定任务",
      to: memory_card.title,
      label: "沉淀为智能体级片段",
    });
  }

  if (agentCount >= 2) {
    const last = evolution_tree[evolution_tree.length - 1]!;
    evolution_tree.push({
      from: last.to,
      to: `${memory_card.title} (多智能体共享)`,
      label: "扩展为多智能体共享",
    });
  }

  if (promotion_candidate) {
    const last = evolution_tree[evolution_tree.length - 1]!;
    evolution_tree.push({
      from: last.to,
      to: `${memory_card.title} (全局提升)`,
      label: "候选提升为全局片段",
    });
  }

  return {
    memory_card_id: memory_card.id,
    title: memory_card.title,
    confidence,
    stability,
    timeline,
    activity,
    conflicts,
    promotion_candidate,
    evolution_tree,
  };
}

export function nextMemoryCardTargets(current: string[], agent: string, add: boolean): string[] {
  if (add) {
    if (current.includes(agent)) return [...current];
    return [agent, ...current];
  }
  return current.filter((a) => a !== agent);
}

export function filterRecordsByTag<T extends DraftRecord | MemoryCardRecord>(
  records: T[],
  tag: string,
): T[] {
  if (!tag || tag === "all") return records;
  return records.filter((item) => (item.tags ?? []).includes(tag));
}

export function removeDraftFromSnapshot(snapshot: ProjectSnapshot, draftId: string): ProjectSnapshot {
  return {
    ...snapshot,
    drafts: snapshot.drafts.filter((draft) => draft.id !== draftId),
  };
}

export function removeCandidateFromInbox(
  inbox: ProjectCandidateInbox | null,
  candidateId: string,
): ProjectCandidateInbox | null {
  if (!inbox) return inbox;
  return {
    ...inbox,
    candidates: inbox.candidates.filter((candidate) => candidate.id !== candidateId),
  };
}
