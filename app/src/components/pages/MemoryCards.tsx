import React from "react";
import { Loader2, Pencil, Trash2 } from "lucide-react";
import { EmptyState } from "../common";
import {
  buildEditFormFromMemoryCard,
  filterRecordsByTag,
  translateKind,
  translateScope,
  type ProjectMemoryCardLibrary,
  type ProjectAssignmentView,
  type PanelPageProps,
  type MemoryCardRecord,
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
  assignment: ProjectAssignmentView | null;
  previewMode: boolean;
  projectPath: string;
  onRefresh: () => void;
}) {
  const [activeTag, setActiveTag] = React.useState("all");
  const [editingId, setEditingId] = React.useState<string | null>(null);
  const [showGovernance, setShowGovernance] = React.useState(false);

  function startEdit(memoryCard: MemoryCardRecord) {
    setEditingId(memoryCard.id);
  }

  function cancelEdit() {
    setEditingId(null);
  }

  async function handleSaved() {
    setEditingId(null);
    onRefresh();
  }

  function deleteMemoryCard(memoryCard: MemoryCardRecord) {
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

  const filteredProject = React.useMemo(
    () => filterRecordsByTag(projectMemoryCards, activeTag),
    [projectMemoryCards, activeTag],
  );

  // Group by Constraint / Procedure / Preference using card.kind
  const groupedCards = React.useMemo(() => {
    const groups: {
      constraints: MemoryCardRecord[];
      procedures: MemoryCardRecord[];
      preferences: MemoryCardRecord[];
      others: MemoryCardRecord[];
    } = {
      constraints: [],
      procedures: [],
      preferences: [],
      others: [],
    };

    for (const card of filteredProject) {
      if (card.kind === "constraint") {
        groups.constraints.push(card);
      } else if (card.kind === "procedure") {
        groups.procedures.push(card);
      } else if (card.kind === "preference") {
        groups.preferences.push(card);
      } else {
        groups.others.push(card);
      }
    }
    return groups;
  }, [filteredProject]);

  return (
    <div className="list">
      {allMemoryCards.length === 0 ? (
        <EmptyState title="暂无项目 Memory Card" description="批准建议审阅后规则会先落归这里，之后即可由你配置并在项目中生效。" />
      ) : (
        <>
          {/* ===== 顶部极简大标题与搜索检索工具栏 ===== */}
          <div style={{ display: "flex", flexDirection: "column", gap: "16px", marginBottom: "24px" }}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start" }}>
              <div>
                <h2 style={{ margin: 0, fontSize: "20px", fontWeight: "bold" }}>项目 Memory 卡片规范手册</h2>
                <p style={{ margin: "4px 0 0 0", color: "var(--color-text-secondary)", fontSize: "13px" }}>
                  这是您项目交互式的条例手册。您可直接快速查阅、检索与编辑项目规程偏好。
                </p>
              </div>
              <button
                className={`panel-btn ${showGovernance ? "accent" : ""}`}
                type="button"
                onClick={() => setShowGovernance(!showGovernance)}
                style={{
                  borderRadius: "10px",
                  padding: "6px 14px",
                  fontFamily: "var(--font-mono)",
                  cursor: "pointer",
                  border: "1px solid var(--color-border)",
                  background: "var(--color-surface)",
                  fontSize: "12px",
                }}
              >
                {showGovernance ? "已开启静默诊断" : "盘点诊断"}
              </button>
            </div>

            <div style={{ display: "flex", gap: "6px", overflowX: "auto", paddingBottom: "4px" }}>
              <button
                className={`filter-btn ${activeTag === "all" ? "active" : ""}`}
                onClick={() => setActiveTag("all")}
                style={{
                  border: "none",
                  background: activeTag === "all" ? "var(--color-text-primary)" : "rgba(120, 110, 95, 0.05)",
                  color: activeTag === "all" ? "#fff" : "var(--color-text-secondary)",
                  borderRadius: "6px",
                  padding: "4px 10px",
                  cursor: "pointer",
                  fontSize: "11px",
                }}
              >
                全部标签
              </button>
              {allTags.slice(0, 12).map((tag) => (
                <button
                  key={tag}
                  className={activeTag === tag ? "active" : ""}
                  onClick={() => setActiveTag(tag)}
                  style={{
                    border: "none",
                    background: activeTag === tag ? "var(--color-text-primary)" : "rgba(120, 110, 95, 0.05)",
                    color: activeTag === tag ? "#fff" : "var(--color-text-secondary)",
                    borderRadius: "6px",
                    padding: "4px 10px",
                    cursor: "pointer",
                    fontSize: "11px",
                  }}
                >
                  #{tag}
                </button>
              ))}
            </div>
          </div>

          {/* ===== 极简诊断小块 (仅在开启显式盘点时渲染，完全排除被禁用的治理面板) ===== */}
          {showGovernance && (
            <div
              style={{
                borderRadius: "16px",
                border: "1px dashed var(--color-border)",
                padding: "16px",
                marginBottom: "24px",
                background: "rgba(120, 110, 95, 0.02)",
                animation: "edit-slide-in 0.3s ease",
              }}
            >
              <strong style={{ fontSize: "13px", display: "block" }}>全局 Memory Card 简易监测</strong>
              <p style={{ margin: "6px 0 0 0", fontSize: "12px", color: "var(--color-text-dim)" }}>
                所有规则条目已完成轻量级挂载。当前诊断规则总数: {allMemoryCards.length}。规则状态结构处于理性完好态。
              </p>
            </div>
          )}

          {filteredProject.length === 0 ? (
            <EmptyState
              title="手册中无匹配的 Memory Card"
              description="清除当前的标签筛选条件后即可快速预览全局规则。"
            />
          ) : (
            <div style={{ display: "flex", flexDirection: "column", gap: "28px" }}>
              {/* Group 1: Constraints */}
              {groupedCards.constraints.length > 0 && (
                <section>
                  <div className="section-label" style={{ marginBottom: "12px", fontWeight: "bold", fontSize: "12px", letterSpacing: "0.05em", color: "var(--color-text-dim)", textTransform: "uppercase" }}>
                    约束规范 / Constraints ({groupedCards.constraints.length})
                  </div>
                  <div style={{ display: "grid", gap: "16px" }}>
                    {groupedCards.constraints.map((card) => (
                      <CardItem
                        key={card.id}
                        card={card}
                        disabled={disabled}
                        deleting={pendingAction === `删除-${card.id}`}
                        editingId={editingId}
                        previewMode={previewMode}
                        projectPath={projectPath}
                        startEdit={startEdit}
                        deleteMemoryCard={deleteMemoryCard}
                        onSaved={handleSaved}
                        cancelEdit={cancelEdit}
                      />
                    ))}
                  </div>
                </section>
              )}

              {/* Group 2: Procedures */}
              {groupedCards.procedures.length > 0 && (
                <section>
                  <div className="section-label" style={{ marginBottom: "12px", fontWeight: "bold", fontSize: "12px", letterSpacing: "0.05em", color: "var(--color-text-dim)", textTransform: "uppercase" }}>
                    流程规程 / Procedures ({groupedCards.procedures.length})
                  </div>
                  <div style={{ display: "grid", gap: "16px" }}>
                    {groupedCards.procedures.map((card) => (
                      <CardItem
                        key={card.id}
                        card={card}
                        disabled={disabled}
                        deleting={pendingAction === `删除-${card.id}`}
                        editingId={editingId}
                        previewMode={previewMode}
                        projectPath={projectPath}
                        startEdit={startEdit}
                        deleteMemoryCard={deleteMemoryCard}
                        onSaved={handleSaved}
                        cancelEdit={cancelEdit}
                      />
                    ))}
                  </div>
                </section>
              )}

              {/* Group 3: Preferences */}
              {groupedCards.preferences.length > 0 && (
                <section>
                  <div className="section-label" style={{ marginBottom: "12px", fontWeight: "bold", fontSize: "12px", letterSpacing: "0.05em", color: "var(--color-text-dim)", textTransform: "uppercase" }}>
                    偏好习惯 / Preferences ({groupedCards.preferences.length})
                  </div>
                  <div style={{ display: "grid", gap: "16px" }}>
                    {groupedCards.preferences.map((card) => (
                      <CardItem
                        key={card.id}
                        card={card}
                        disabled={disabled}
                        deleting={pendingAction === `删除-${card.id}`}
                        editingId={editingId}
                        previewMode={previewMode}
                        projectPath={projectPath}
                        startEdit={startEdit}
                        deleteMemoryCard={deleteMemoryCard}
                        onSaved={handleSaved}
                        cancelEdit={cancelEdit}
                      />
                    ))}
                  </div>
                </section>
              )}

              {/* Group 4: Others */}
              {groupedCards.others.length > 0 && (
                <section>
                  <div className="section-label" style={{ marginBottom: "12px", fontWeight: "bold", fontSize: "12px", letterSpacing: "0.05em", color: "var(--color-text-dim)", textTransform: "uppercase" }}>
                    其他条目 / Others ({groupedCards.others.length})
                  </div>
                  <div style={{ display: "grid", gap: "16px" }}>
                    {groupedCards.others.map((card) => (
                      <CardItem
                        key={card.id}
                        card={card}
                        disabled={disabled}
                        deleting={pendingAction === `删除-${card.id}`}
                        editingId={editingId}
                        previewMode={previewMode}
                        projectPath={projectPath}
                        startEdit={startEdit}
                        deleteMemoryCard={deleteMemoryCard}
                        onSaved={handleSaved}
                        cancelEdit={cancelEdit}
                      />
                    ))}
                  </div>
                </section>
              )}
            </div>
          )}
        </>
      )}
    </div>
  );
}

function CardItem({
  card,
  disabled,
  deleting,
  editingId,
  previewMode,
  projectPath,
  startEdit,
  deleteMemoryCard,
  onSaved,
  cancelEdit,
}: {
  card: MemoryCardRecord;
  disabled: boolean;
  deleting: boolean;
  editingId: string | null;
  previewMode: boolean;
  projectPath: string;
  startEdit: (card: MemoryCardRecord) => void;
  deleteMemoryCard: (card: MemoryCardRecord) => void;
  onSaved: () => Promise<void>;
  cancelEdit: () => void;
}) {
  return (
    <React.Fragment>
      <article className="record compact" style={{ padding: "20px", borderRadius: "16px" }}>
        <div className="record-main">
          <span className="tag" style={{ borderRadius: "8px", fontSize: "10.5px" }}>
            {translateKind(card.kind)} · {translateScope(card.scope)}
          </span>
          {card.brief ? <p className="draft-brief">{card.brief}</p> : null}
          <h3 style={{ fontSize: "15px", fontWeight: "700" }}>{card.title}</h3>
          <p style={{ marginTop: "10px", lineHeight: "1.6" }}>{card.body}</p>
          {card.tags && card.tags.length > 0 ? (
            <div className="tag-row">
              {card.tags.map((tag) => (
                <span key={tag} style={{ borderRadius: "4px", fontSize: "10px" }}>#{tag}</span>
              ))}
            </div>
          ) : null}
        </div>
        <div className="record-actions" style={{ marginTop: "12px", borderTop: "1px dashed var(--color-border)", paddingTop: "12px", display: "flex", justifyContent: "flex-end", gap: "8px" }}>
          <button
            className="secondary-action"
            disabled={disabled || deleting}
            onClick={() => startEdit(card)}
          >
            <Pencil size={13} />
            编辑细则与标签
          </button>
          <button
            className="danger-action"
            disabled={disabled || deleting}
            onClick={() => deleteMemoryCard(card)}
          >
            {deleting ? <Loader2 className="spin" size={13} /> : <Trash2 size={13} />}
            从规则中退役解构
          </button>
        </div>
      </article>
      {editingId === card.id ? (
        <RecordEditor
          recordType="memory_card"
          initialForm={buildEditFormFromMemoryCard(card)}
          previewMode={previewMode}
          projectPath={projectPath}
          recordId={card.id}
          onSaved={onSaved}
          onCancel={cancelEdit}
        />
      ) : null}
    </React.Fragment>
  );
}

function dedupeMemoryCards(
  memoryCards: MemoryCardRecord[],
  projectPath: string,
): MemoryCardRecord[] {
  const entries: MemoryCardRecord[] = [];
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

function logicalMemoryCardKey(memoryCard: MemoryCardRecord): string {
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

function isProjectMemoryCard(memoryCard: MemoryCardRecord, projectPath: string): boolean {
  return memoryCard.scope === "project" || memoryCard.source_project === projectPath || memoryCard.id.startsWith("project:");
}
