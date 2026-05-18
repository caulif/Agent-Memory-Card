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
        <EmptyState title="暂无项目技能片段" description="批准草稿后会先进入这里，之后再由你手动分配给 Agent。" />
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

          {allTags.length > 0 ? (
            <nav className="tag-filter" aria-label="按标签筛选技能片段">
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

          <MemoryGovernanceBatchBar
            actions={batchActions}
            disabled={disabled}
            pendingAction={pendingAction}
            batchAction={batchAction}
            onRunBatch={runGovernanceBatch}
          />

          {filtered.length === 0 ? (
            <p className="filter-note">当前标签筛选条件下暂无匹配的技能片段。</p>
          ) : null}

          {filteredProject.length > 0 ? (
            <section className="memory_card-section">
              <div className="section-label">项目技能片段</div>
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
