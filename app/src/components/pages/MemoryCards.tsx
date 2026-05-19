import React from "react";
import { Loader2, Pencil, Trash2 } from "lucide-react";
import { EmptyState } from "../common";
import {
  buildEditFormFromMemoryCard,
  buildMemoryGovernanceBatchActions,
  buildMemoryGovernanceSummary,
  filterMemoryCardsByGovernance,
  filterRecordsByTag,
  summarizeMemoryCardGovernance,
  translateKind,
  translateScope,
  type MemoryGovernanceFilter,
  type ProjectMemoryCardLibrary,
  type ProjectAssignmentView,
  type PanelPageProps,
} from "../../ui-helpers";
import { MemoryGovernanceBatchBar } from "../memory/MemoryGovernanceBatchBar";
import { MemoryGovernancePanel } from "../memory/MemoryGovernancePanel";
import { RecordEditor } from "./RecordEditor";

export function MemoryCards({
  snapshot,
  library,
  assignment,
  pendingAction,
  disabled,
  onAction,
  previewMode,
  projectPath,
  onRefresh,
}: PanelPageProps & {
  library: ProjectMemoryCardLibrary | null;
  assignment: ProjectAssignmentView | null;
  previewMode: boolean;
  projectPath: string;
  onRefresh: () => void;
}) {
  const [activeTag, setActiveTag] = React.useState("all");
  const [activeGovernance, setActiveGovernance] = React.useState<MemoryGovernanceFilter>("all");
  const [editingId, setEditingId] = React.useState<string | null>(null);
  const [batchAction, setBatchAction] = React.useState<string | null>(null);

  function startEdit(memoryCard: import("../../ui-helpers").MemoryCardRecord) {
    setEditingId(memoryCard.id);
  }

  function cancelEdit() {
    setEditingId(null);
  }

  async function handleSaved() {
    setEditingId(null);
    onRefresh();
  }

  function deleteMemoryCard(memoryCard: import("../../ui-helpers").MemoryCardRecord) {
    void onAction(
      `删除-${memoryCard.id}`,
      `已删除"${memoryCard.title}"`,
      "delete_memory_card",
      { id: memoryCard.id },
    );
  }

  async function runGovernanceBatch(action: import("../../ui-helpers").MemoryGovernanceBatchAction) {
    setBatchAction(action.actionKey);
    try {
      for (const step of action.steps) {
        await onAction(step.actionKey, step.doneMessage, step.command, step.args);
      }
    } finally {
      setBatchAction(null);
      onRefresh();
    }
  }

  const rawProjectMemoryCards = library?.memory_cards ?? snapshot?.memory_cards ?? [];
  const projectMemoryCards = React.useMemo(
    () => dedupeMemoryCards(rawProjectMemoryCards, projectPath),
    [rawProjectMemoryCards, projectPath],
  );
  const allMemoryCards = projectMemoryCards;
  const governanceSummary = React.useMemo(
    () => buildMemoryGovernanceSummary(allMemoryCards, assignment),
    [allMemoryCards, assignment],
  );

  const allTags = React.useMemo(() => {
    const tagSet = new Set<string>();
    for (const memoryCard of allMemoryCards) {
      for (const tag of memoryCard.tags ?? []) {
        tagSet.add(tag);
      }
    }
    return Array.from(tagSet).sort();
  }, [allMemoryCards]);

  const tagFiltered = React.useMemo(
    () => filterRecordsByTag(allMemoryCards, activeTag),
    [allMemoryCards, activeTag],
  );
  const filtered = React.useMemo(
    () => filterMemoryCardsByGovernance(tagFiltered, activeGovernance, assignment),
    [activeGovernance, assignment, tagFiltered],
  );
  const filteredProject = React.useMemo(
    () => filterMemoryCardsByGovernance(filterRecordsByTag(projectMemoryCards, activeTag), activeGovernance, assignment),
    [activeGovernance, assignment, projectMemoryCards, activeTag],
  );
  const batchActions = React.useMemo(
    () => buildMemoryGovernanceBatchActions(filteredProject, activeGovernance, assignment, allMemoryCards),
    [activeGovernance, allMemoryCards, assignment, filteredProject],
  );
  const activeFilterLabel = governanceFiltersLabel(activeGovernance);
  const priorityIssue = pickGovernancePriority(governanceSummary);
  const governanceFilters: Array<{ id: MemoryGovernanceFilter; label: string }> = [
    { id: "all", label: "全部治理" },
    { id: "needs-review", label: "需复核" },
    { id: "conflicts", label: "疑似重复" },
    { id: "unassigned", label: "未分配" },
    { id: "missing-source", label: "缺来源" },
    { id: "dormant", label: "休眠" },
    { id: "expired", label: "过期" },
  ];

  return (
    <div className="list">
      {allMemoryCards.length === 0 ? (
        <EmptyState title="暂无项目 Memory Card" description="批准建议后会先进入这里，之后再由你手动配置到 Agent Loadout。" />
      ) : (
        <>
          <section className="governance-strip" aria-label="Memory Card 治理状态">
            <div>
              <strong>{governanceSummary.total}</strong>
              <span>总卡片</span>
            </div>
            <div>
              <strong>{governanceSummary.assigned}</strong>
              <span>已分配</span>
            </div>
            <div>
              <strong>{governanceSummary.unassigned}</strong>
              <span>未分配</span>
            </div>
            <div>
              <strong>{governanceSummary.missingSource}</strong>
              <span>缺来源</span>
            </div>
            <div>
              <strong>{governanceSummary.needsReview}</strong>
              <span>需复核</span>
            </div>
            <div>
              <strong>{governanceSummary.conflicts}</strong>
              <span>疑似重复</span>
            </div>
            <div>
              <strong>{governanceSummary.dormant}</strong>
              <span>休眠</span>
            </div>
            <div>
              <strong>{governanceSummary.expired}</strong>
              <span>过期</span>
            </div>
          </section>

          <section className={`memory-priority-card ${priorityIssue.tone}`} aria-label="Memory Card 下一步">
            <div>
              <span>下一步</span>
              <strong>{priorityIssue.label}</strong>
              <p>{priorityIssue.detail}</p>
            </div>
            <button
              className="secondary-action"
              type="button"
              onClick={() => setActiveGovernance(priorityIssue.filter)}
            >
              查看相关卡片
            </button>
          </section>

          {allTags.length > 0 ? (
            <nav className="tag-filter" aria-label="按标签筛选 Memory Card">
              <button
                className={activeTag === "all" ? "active" : ""}
                onClick={() => setActiveTag("all")}
              >
                全部
              </button>
              {allTags.map((tag) => (
                <button
                  key={tag}
                  className={activeTag === tag ? "active" : ""}
                  onClick={() => setActiveTag(tag)}
                >
                  {tag}
                </button>
              ))}
            </nav>
          ) : null}
          <nav className="tag-filter" aria-label="按治理状态筛选 Memory Card">
            {governanceFilters.map((filter) => (
              <button
                key={filter.id}
                className={activeGovernance === filter.id ? "active" : ""}
                onClick={() => setActiveGovernance(filter.id)}
              >
                {filter.label}
              </button>
            ))}
          </nav>
          <div className="filter-summary-row">
            <span>
              当前显示 {filteredProject.length}/{projectMemoryCards.length} 张项目卡片
              {activeGovernance !== "all" ? ` · ${activeFilterLabel}` : ""}
              {activeTag !== "all" ? ` · 标签 ${activeTag}` : ""}
            </span>
            {(activeGovernance !== "all" || activeTag !== "all") ? (
              <button
                className="ghost-action"
                type="button"
                onClick={() => {
                  setActiveGovernance("all");
                  setActiveTag("all");
                }}
              >
                清除筛选
              </button>
            ) : null}
          </div>

          <MemoryGovernanceBatchBar
            actions={batchActions}
            disabled={disabled}
            pendingAction={pendingAction}
            batchAction={batchAction}
            onRunBatch={runGovernanceBatch}
          />

          {filtered.length === 0 ? (
            <EmptyState
              title="没有匹配的 Memory Card"
              description="当前筛选条件没有结果。清除筛选后可查看全部项目卡片，或先批准新的 Suggestion。"
            />
          ) : null}

          {filteredProject.length > 0 ? (
            <section className="memory_card-section">
              <div className="section-label">项目 Memory Cards</div>
              {filteredProject.map((memoryCard) => {
                const deleting = pendingAction === `删除-${memoryCard.id}`;
                const governance = summarizeMemoryCardGovernance(memoryCard, assignment, allMemoryCards);
                return (
                  <React.Fragment key={memoryCard.id}>
                    <article className="record compact">
                      <div className="record-main">
                        <span className="tag">
                          {translateKind(memoryCard.kind)} · {translateScope(memoryCard.scope)}
                        </span>
                        {memoryCard.brief ? <p className="draft-brief">{memoryCard.brief}</p> : null}
                        <h3>{memoryCard.title}</h3>
                        <p>{memoryCard.body}</p>
                        {memoryCard.tags && memoryCard.tags.length > 0 ? (
                          <div className="tag-row">
                            {memoryCard.tags.map((tag) => (
                              <span key={tag}>{tag}</span>
                            ))}
                          </div>
                        ) : null}
                        <MemoryGovernancePanel
                          governance={governance}
                          disabled={disabled}
                          pendingAction={pendingAction}
                          onAction={onAction}
                        />
                      </div>
                      <div className="record-actions">
                        <button
                          className="secondary-action"
                          disabled={disabled || deleting}
                          onClick={() => startEdit(memoryCard)}
                        >
                          <Pencil size={15} />
                          编辑
                        </button>
                        <button
                          className="danger-action"
                          disabled={disabled || deleting}
                          onClick={() => deleteMemoryCard(memoryCard)}
                        >
                          {deleting ? <Loader2 className="spin" size={15} /> : <Trash2 size={15} />}
                          删除
                        </button>
                      </div>
                    </article>
                    {editingId === memoryCard.id ? (
                      <RecordEditor
                        recordType="memory_card"
                        initialForm={buildEditFormFromMemoryCard(memoryCard)}
                        previewMode={previewMode}
                        projectPath={projectPath}
                        recordId={memoryCard.id}
                        onSaved={handleSaved}
                        onCancel={cancelEdit}
                      />
                    ) : null}
                  </React.Fragment>
                );
              })}
            </section>
          ) : null}
        </>
      )}
    </div>
  );
}

function governanceFiltersLabel(filter: MemoryGovernanceFilter): string {
  switch (filter) {
    case "needs-review":
      return "需复核";
    case "conflicts":
      return "疑似重复";
    case "unassigned":
      return "未分配";
    case "missing-source":
      return "缺来源";
    case "dormant":
      return "休眠";
    case "expired":
      return "过期";
    default:
      return "全部治理";
  }
}

function pickGovernancePriority(summary: ReturnType<typeof buildMemoryGovernanceSummary>): {
  label: string;
  detail: string;
  filter: MemoryGovernanceFilter;
  tone: "ready" | "attention" | "blocked";
} {
  if (summary.missingSource > 0) {
    return {
      label: "补齐来源证据",
      detail: `${summary.missingSource} 张卡片缺少可追溯来源，建议先复核再同步到 Agent 文件。`,
      filter: "missing-source",
      tone: "blocked",
    };
  }
  if (summary.conflicts > 0) {
    return {
      label: "处理疑似重复",
      detail: `${summary.conflicts} 张卡片可能表达相近规则，先合并能降低 Agent 读取噪音。`,
      filter: "conflicts",
      tone: "attention",
    };
  }
  if (summary.unassigned > 0) {
    return {
      label: "配置 Agent Loadout",
      detail: `${summary.unassigned} 张卡片还没有分配给 Codex 或 Claude Code。`,
      filter: "unassigned",
      tone: "attention",
    };
  }
  if (summary.needsReview > 0) {
    return {
      label: "复核不稳定卡片",
      detail: `${summary.needsReview} 张卡片需要人工确认是否仍然有效。`,
      filter: "needs-review",
      tone: "attention",
    };
  }
  return {
    label: "治理状态良好",
    detail: "当前项目卡片没有明显治理阻塞，可以继续分配 Loadout 或预览 Artifact。",
    filter: "all",
    tone: "ready",
  };
}

function dedupeMemoryCards(
  memoryCards: import("../../ui-helpers").MemoryCardRecord[],
  projectPath: string,
): import("../../ui-helpers").MemoryCardRecord[] {
  const entries: import("../../ui-helpers").MemoryCardRecord[] = [];
  const indexByKey = new Map<string, number>();

  for (const memoryCard of memoryCards) {
    const key = logicalMemoryCardKey(memoryCard);
    const existingIndex = indexByKey.get(key);
    if (existingIndex == null) {
      indexByKey.set(key, entries.length);
      entries.push(memoryCard);
      continue;
    }

    const current = entries[existingIndex]!;
    if (!isProjectMemoryCard(current, projectPath) && isProjectMemoryCard(memoryCard, projectPath)) {
      entries[existingIndex] = memoryCard;
    }
  }

  return entries;
}

function logicalMemoryCardKey(memoryCard: import("../../ui-helpers").MemoryCardRecord): string {
  const id = memoryCard.id
    .toLowerCase()
    .replace(/^(project|global):/, "")
    .trim();
  const title = normalizeMemoryCardText(memoryCard.title);
  const bodyPrefix = normalizeMemoryCardText(memoryCard.body).slice(0, 120);
  return `${id || title}|${title}|${bodyPrefix}`;
}

function normalizeMemoryCardText(value: string): string {
  return value.toLowerCase().replace(/\s+/g, " ").trim();
}

function isProjectMemoryCard(memoryCard: import("../../ui-helpers").MemoryCardRecord, projectPath: string): boolean {
  return memoryCard.scope === "project" || memoryCard.source_project === projectPath || memoryCard.id.startsWith("project:");
}
