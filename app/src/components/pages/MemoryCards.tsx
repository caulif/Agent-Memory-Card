import React from "react";
import { Loader2, Pencil, Trash2 } from "lucide-react";
import { EmptyState } from "../common";
import {
  buildEditFormFromMemoryCard,
  filterRecordsByTag,
  translateKind,
  translateScope,
  type ProjectMemoryCardLibrary,
  type PanelPageProps,
} from "../../ui-helpers";
import { RecordEditor } from "./RecordEditor";

export function MemoryCards({
  snapshot,
  library,
  pendingAction,
  disabled,
  onAction,
  previewMode,
  projectPath,
  onRefresh,
}: PanelPageProps & {
  library: ProjectMemoryCardLibrary | null;
  previewMode: boolean;
  projectPath: string;
  onRefresh: () => void;
}) {
  const [activeTag, setActiveTag] = React.useState("all");
  const [editingId, setEditingId] = React.useState<string | null>(null);

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

  const rawProjectMemoryCards = library?.memory_cards ?? snapshot?.memory_cards ?? [];
  const projectMemoryCards = React.useMemo(
    () => dedupeMemoryCards(rawProjectMemoryCards, projectPath),
    [rawProjectMemoryCards, projectPath],
  );
  const allMemoryCards = projectMemoryCards;

  const allTags = React.useMemo(() => {
    const tagSet = new Set<string>();
    for (const memoryCard of allMemoryCards) {
      for (const tag of memoryCard.tags ?? []) {
        tagSet.add(tag);
      }
    }
    return Array.from(tagSet).sort();
  }, [allMemoryCards]);

  const filtered = React.useMemo(
    () => filterRecordsByTag(allMemoryCards, activeTag),
    [allMemoryCards, activeTag],
  );
  const filteredProject = React.useMemo(
    () => filterRecordsByTag(projectMemoryCards, activeTag),
    [projectMemoryCards, activeTag],
  );

  return (
    <div className="list">
      {allMemoryCards.length === 0 ? (
        <EmptyState title="暂无项目技能片段" description="批准草稿后会先进入这里，之后再由你手动分配给 Agent。" />
      ) : (
        <>
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

          {filtered.length === 0 ? (
            <p className="filter-note">当前标签筛选条件下暂无匹配的技能片段。</p>
          ) : null}

          {filteredProject.length > 0 ? (
            <section className="memory_card-section">
              <div className="section-label">项目技能片段</div>
              {filteredProject.map((memoryCard) => {
                const deleting = pendingAction === `删除-${memoryCard.id}`;
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
