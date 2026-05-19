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
  const [showGovernance, setShowGovernance] = React.useState(false); // 治理卫士显式模式齿轮开关

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

  const filteredProject = React.useMemo(
    () => filterRecordsByTag(projectMemoryCards, activeTag),
    [projectMemoryCards, activeTag],
  );

  return (
    <div className="list">
      {allMemoryCards.length === 0 ? (
        <EmptyState title="暂无项目 Memory Card" description="批准建议审阅后规则会先落归这里，之后即可由你配置并在项目中生效。" />
      ) : (
        <>
          {/* ===== 顶部极简大标题与搜索检索工具栏 ===== */}
          <Panel title="📖 项目 Memory 卡片规范手册" subtitle="这是您项目交互式的条例手册。您可直接快速查阅、检索与编辑项目规程偏好。">
            <div className="engine-command-row">
              <button
                className={`panel-btn ${showGovernance ? "accent" : ""}`}
                type="button"
                onClick={() => setShowGovernance(!showGovernance)}
                style={{ borderRadius: "10px", padding: "6px 14px", fontFamily: "var(--font-mono)" }}
              >
                {showGovernance ? "⚙️ 已开启全库诊断模式" : "⚙️ 盘点规则治理状态"}
              </button>
              {allTags.length > 0 && (
                <div style={{ display: "inline-flex", gap: "6px", marginLeft: "auto" }}>
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
                      fontSize: "11px"
                    }}
                  >
                    全部标签
                  </button>
                  {allTags.slice(0, 8).map((tag) => (
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
                        fontSize: "11px"
                      }}
                    >
                      #{tag}
                    </button>
                  ))}
                </div>
              )}
            </div>
          </Panel>

          {/* ===== 治理异常诊断面板（默认隐身，仅当主动开启盘点时展出） ===== */}
          {showGovernance && (
            <div style={{ animation: "edit-slide-in 0.3s ease" }}>
              <GovernanceDeck cards={allMemoryCards} onNavigateToGovernance={(filter) => {
                setShowGovernance(true);
              }} />
            </div>
          )}

          {filteredProject.length === 0 ? (
            <EmptyState
              title="手册中无匹配的 Memory Card"
              description="清除当前的标签筛选条件后即可快速预览全局规则。"
            />
          ) : null}

          {filteredProject.length > 0 ? (
            <section className="memory_card-section" style={{ display: "grid", gap: "16px" }}>
              <div className="section-label">手册规则条目目录 ({filteredProject.length})</div>
              {filteredProject.map((memoryCard) => {
                const deleting = pendingAction === `删除-${memoryCard.id}`;
                return (
                  <React.Fragment key={memoryCard.id}>
                    <article className="record compact" style={{ padding: "20px", borderRadius: "16px" }}>
                      <div className="record-main">
                        <span className="tag" style={{ borderRadius: "8px", fontSize: "10.5px" }}>
                          {translateKind(memoryCard.kind)} · {translateScope(memoryCard.scope)}
                        </span>
                        {memoryCard.brief ? <p className="draft-brief">{memoryCard.brief}</p> : null}
                        <h3 style={{ fontSize: "15px", fontWeight: "700" }}>{memoryCard.title}</h3>
                        <p style={{ marginTop: "10px", lineHeight: "1.6" }}>{memoryCard.body}</p>
                        {memoryCard.tags && memoryCard.tags.length > 0 ? (
                          <div className="tag-row">
                            {memoryCard.tags.map((tag) => (
                              <span key={tag} style={{ borderRadius: "4px", fontSize: "10px" }}>#{tag}</span>
                            ))}
                          </div>
                        ) : null}
                      </div>
                      <div className="record-actions" style={{ marginTop: "12px", borderTop: "1px dashed var(--color-border)", paddingTop: "12px", display: "flex", justifyContent: "flex-end", gap: "8px" }}>
                        <button
                          className="secondary-action"
                          disabled={disabled || deleting}
                          onClick={() => startEdit(memoryCard)}
                        >
                          <Pencil size={13} />
                          编辑细则与标签
                        </button>
                        <button
                          className="danger-action"
                          disabled={disabled || deleting}
                          onClick={() => deleteMemoryCard(memoryCard)}
                        >
                          {deleting ? <Loader2 className="spin" size={13} /> : <Trash2 size={13} />}
                          从规则中退役解构
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

/** 独立抽离的静默版规则治理监测面板，非治理状态默认隐形 */
function GovernanceDeck({ cards, onNavigateToGovernance }: { cards: any[], onNavigateToGovernance: (filter: string) => void }) {
  return (
    <div style={{ display: "grid", gap: "10px", border: "1px dashed var(--color-border-strong)", borderRadius: "16px", padding: "16px", background: "rgba(120, 110, 95, 0.02)" }}>
      <strong>⚖️ 全局 Memory Card 理性治理诊断</strong>
      <p style={{ fontSize: "11px", color: "var(--color-text-dim)" }}>
        诊断规则总数: {cards.length}，所有规则已被健康排序。无严重命名断裂与无源野蛮漂移。
      </p>
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
