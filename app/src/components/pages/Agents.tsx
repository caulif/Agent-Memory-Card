import React from "react";
import { Check, Circle, Loader2, Trash2 } from "lucide-react";
import { EmptyState } from "../common";
import {
  formatAgent,
  nextMemoryCardTargets,
  translateKind,
  type ProjectAssignmentView,
  type ProjectMemoryCardLibrary,
  type PanelPageProps,
  type MemoryCardRecord,
} from "../../ui-helpers";

export function Agents({
  snapshot,
  assignment,
  library,
  pendingAction,
  disabled,
  onAction,
}: PanelPageProps & {
  assignment: ProjectAssignmentView | null;
  library: ProjectMemoryCardLibrary | null;
}) {
  const agents = assignment?.target_matrix.agents ?? snapshot?.target_matrix.agents ?? [];
  const rows = assignment?.target_matrix.rows ?? snapshot?.target_matrix.rows ?? [];
  const memory_cards = library?.memory_cards ?? snapshot?.memory_cards ?? [];
  const togglingKey = pendingAction.startsWith("切换-") ? pendingAction : "";
  const [dragOverAgent, setDragOverAgent] = React.useState<string | null>(null);
  const [selectedMemoryCardId, setSelectedMemoryCardId] = React.useState<string | null>(null);

  const projectPath = assignment?.project_path ?? snapshot?.project_path ?? "";
  const allAvailable = React.useMemo(
    () => dedupeAvailableMemoryCards(memory_cards, projectPath).filter((card) => isProjectSource(card, projectPath)),
    [projectPath, memory_cards],
  );

  const assignmentDisabled = disabled;

  /** 解析正在切换中的 memory_card_id 和 agent */
  const togglingParts = togglingKey ? togglingKey.replace("切换-", "").split("|") : [];
  const togglingMemoryCardId = togglingParts[0] ?? "";
  const togglingAgent = togglingParts[1] ?? "";

  /** 根据 agent 获取已分配的 memory_card */
  function getEquippedFor(agent: string) {
    return rows.filter((row) => row.targets[agent]);
  }

  /** 根据 kind 分组 memory_cards */
  const poolByKind = React.useMemo(() => {
    const map: Record<string, typeof allAvailable> = {};
    for (const s of allAvailable) {
      const kind = s.kind ?? "other";
      if (!map[kind]) map[kind] = [];
      map[kind].push(s);
    }
    return map;
  }, [allAvailable]);

  /** 判断 memory_card 是否已分配给任意 agent */
  function isEquippedAnywhere(memory_cardId: string) {
    return rows.some((row) => row.memory_card_id === memory_cardId && Object.values(row.targets).some(Boolean));
  }

  function startMemoryCardDrag(event: React.DragEvent<HTMLElement>, memory_card: Pick<MemoryCardRecord, "id" | "title" | "scope">) {
    event.dataTransfer.effectAllowed = "copyMove";
    event.dataTransfer.setData("application/x-memory_card-id", memory_card.id);
    event.dataTransfer.setData("application/x-memory-card-id", memory_card.id);
    event.dataTransfer.setData("text/plain", memory_card.id);
  }

  function assignMemoryCardToAgent(memory_cardId: string, agent: string) {
    const row = rows.find((item) => item.memory_card_id === memory_cardId);
    const memory_card = allAvailable.find((item) => item.id === memory_cardId);
    const title = row?.title ?? memory_card?.title ?? memory_cardId;
    const currentTargets = row ? agents.filter((a) => row.targets[a]) : [];
    const nextTargets = nextMemoryCardTargets(currentTargets, agent, true);
    void onAction(
      `切换-${memory_cardId}|${agent}`,
      `已挂载"${title}"到智能体`,
      "set_memory_card_targets",
      { id: memory_cardId, targets: nextTargets },
    );
    setSelectedMemoryCardId(null);
  }

  function clearAgentAssignments(agent: string) {
    void onAction(
      `清空-${agent}`,
      `已卸载 ${formatAgent(agent)} 挂载的全部卡片`,
      "clear_memory_card_targets",
      { agent },
    );
  }

  function handleDrop(event: React.DragEvent<HTMLElement>, agent: string) {
    event.preventDefault();
    const memory_cardId =
      event.dataTransfer.getData("application/x-memory_card-id") ||
      event.dataTransfer.getData("application/x-memory-card-id") ||
      event.dataTransfer.getData("text/plain");
    setDragOverAgent(null);
    if (!memory_cardId || assignmentDisabled) return;
    assignMemoryCardToAgent(memory_cardId, agent);
  }

  const selectedMemoryCard = allAvailable.find((card) => card.id === selectedMemoryCardId) ?? null;

  return (
    <div className="stack">
      {/* ===== Loadout 极简大盘头部 ===== */}
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "20px" }}>
        <div>
          <h2 style={{ margin: 0, fontSize: "20px", fontWeight: "bold" }}>Memory Card Loadout</h2>
          <p style={{ margin: "4px 0 0 0", color: "var(--color-text-secondary)", fontSize: "13px" }}>
            只装配当前项目的 Memory Card。先点选左侧卡片，再点右侧智能体槽位装入。
          </p>
        </div>
        <div style={{ borderRadius: "8px", border: "1px solid var(--color-border)", padding: "6px 12px", background: "var(--color-surface)", display: "flex", alignItems: "center", gap: "8px" }}>
          <span style={{ width: "8px", height: "8px", borderRadius: "50%", background: "var(--color-success)" }}></span>
          <span style={{ fontSize: "11px", fontWeight: "600", color: "var(--color-text-secondary)" }}>
            分配配置状态正常
          </span>
        </div>
      </div>

      {agents.length === 0 ? (
        <EmptyState title="暂无目标智能体" description="当前项目未检测到 Codex 或 Claude Code 配置。" />
      ) : (
        /* ===== 双栏极简左右互锁分配区 ===== */
        <div className="drafts-grid agents-loadout-shell" style={{ gap: "28px" }}>
          {/* 左栏：可用 Memory Cards 卡池 */}
          <div className="skill-pool" style={{ borderRadius: "20px", padding: "24px", background: "var(--color-surface)", border: "1px solid var(--color-border)" }}>
            <div className="skill-pool-head" style={{ marginBottom: "16px" }}>
              <h3 style={{ fontSize: "14px", fontWeight: "750", margin: 0 }}>可用 Memory Card 规则池</h3>
              <p style={{ fontSize: "11px", color: "var(--color-text-dim)", margin: "4px 0 0 0" }}>
                点击选择卡片；也可以拖动到右侧目标智能体槽位中。
              </p>
            </div>
            {allAvailable.length === 0 ? (
              <EmptyState title="暂无可用 Memory Card" description="请先去「建议审阅」提取和批准一些条例卡片。" />
            ) : (
              <div className="pool-groups">
                {(["rule", "procedure", "constraint", "preference", "other"] as const).map((kind) => {
                  const items = kind === "other"
                    ? allAvailable.filter((item) => !["rule", "procedure", "constraint", "preference"].includes(item.kind))
                    : poolByKind[kind];
                  if (!items || items.length === 0) return null;
                  return (
                    <div className="pool-group" key={kind} style={{ marginTop: "12px" }}>
                      <h4 style={{ fontSize: "10.5px", color: "var(--color-text-dim)", textTransform: "uppercase", margin: "12px 0 6px 0" }}>
                        {kind === "other" ? "其他" : translateKind(kind)}
                      </h4>
                      <div className="pool-items" style={{ display: "flex", flexWrap: "wrap", gap: "6px" }}>
                        {items.map((s) => {
                          const equipped = isEquippedAnywhere(s.id);
                          return (
                            <div
                              key={s.id}
                              role="button"
                              tabIndex={assignmentDisabled ? -1 : 0}
                              className={`pool-item ${equipped ? "equipped" : ""} ${selectedMemoryCardId === s.id ? "selected" : ""}`}
                              draggable={!assignmentDisabled}
                              aria-pressed={selectedMemoryCardId === s.id}
                              onDragStart={(event) => startMemoryCardDrag(event, s)}
                              onDragEnd={() => setDragOverAgent(null)}
                              onClick={() => {
                                if (assignmentDisabled) return;
                                setSelectedMemoryCardId((current) => current === s.id ? null : s.id);
                              }}
                              aria-disabled={assignmentDisabled}
                              title={s.title}
                              onKeyDown={(event) => {
                                if (assignmentDisabled || event.key !== "Enter") return;
                                setSelectedMemoryCardId((current) => current === s.id ? null : s.id);
                              }}
                              style={{
                                borderRadius: "8px",
                                padding: "6px 12px",
                                border: equipped ? "1.5px solid var(--color-success)" : "1px dashed var(--color-border)",
                                boxShadow: "var(--shadow-sm)",
                              }}
                            >
                              {equipped ? <Check size={12} className="pool-check" style={{ color: "var(--color-success)" }} /> : <Circle size={12} />}
                              <span className="pool-title" style={{ fontWeight: equipped ? "650" : "500" }}>{s.title}</span>
                            </div>
                          );
                        })}
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </div>

          {/* 右栏：智能体槽位分配清单 */}
          <div className="loadout-grid" style={{ display: "grid", gridTemplateColumns: "1fr", gap: "16px" }}>
            {selectedMemoryCard ? (
              <div className="loadout-selection">
                <strong>已选择</strong>
                <span>{selectedMemoryCard.title}</span>
                <button type="button" className="ghost-action" onClick={() => setSelectedMemoryCardId(null)}>
                  取消选择
                </button>
              </div>
            ) : null}
            {agents.map((agent) => {
              const equipped = getEquippedFor(agent);
              const clearAgentBusy = pendingAction === `清空-${agent}`;
              const selectedAlreadyEquipped = selectedMemoryCardId
                ? equipped.some((row) => row.memory_card_id === selectedMemoryCardId)
                : false;
              return (
                <div
                  className={`loadout-column ${dragOverAgent === agent ? "drag-over" : ""}`}
                  key={agent}
                  onDragOver={(event) => {
                    if (assignmentDisabled) return;
                    event.preventDefault();
                    event.dataTransfer.dropEffect = "copy";
                    setDragOverAgent(agent);
                  }}
                  onDragEnter={(event) => {
                    if (assignmentDisabled) return;
                    event.preventDefault();
                    setDragOverAgent(agent);
                  }}
                  onDragLeave={() => setDragOverAgent((current) => (current === agent ? null : current))}
                  onDrop={(event) => handleDrop(event, agent)}
                  style={{
                    borderRadius: "20px",
                    padding: "20px",
                    border: dragOverAgent === agent ? "1.5px solid var(--color-accent)" : "1px solid var(--color-border)",
                    boxShadow: "var(--shadow-md)",
                    background: "var(--color-surface)",
                  }}
                >
                  <h3 style={{ display: "flex", alignItems: "center", borderBottom: "1px dashed var(--color-border)", paddingBottom: "12px", marginBottom: "12px", marginTop: 0 }}>
                    <span className="loadout-dot" style={{ background: "var(--color-accent-ring)" }} />
                    {formatAgent(agent)}
                    <span className="loadout-count" style={{ borderRadius: "8px", fontSize: "11px", fontWeight: "700", marginLeft: "8px" }}>
                      已装配 {equipped.length} 条规范
                    </span>
                    {selectedMemoryCardId ? (
                      <button
                        className="secondary-action compact"
                        disabled={assignmentDisabled || selectedAlreadyEquipped}
                        title={selectedAlreadyEquipped ? "该卡片已在此智能体中" : `装入 ${formatAgent(agent)}`}
                        onClick={() => assignMemoryCardToAgent(selectedMemoryCardId, agent)}
                        style={{ marginLeft: "auto" }}
                      >
                        装入已选
                      </button>
                    ) : null}
                    <button
                      className="icon-action"
                      disabled={assignmentDisabled || clearAgentBusy || equipped.length === 0}
                      title={`卸载 ${formatAgent(agent)} 的全部卡片`}
                      aria-label={`卸载 ${formatAgent(agent)} 的全部卡片`}
                      onClick={() => clearAgentAssignments(agent)}
                      style={{ marginLeft: selectedMemoryCardId ? "0" : "auto", width: "28px", height: "28px", borderRadius: "8px", cursor: "pointer" }}
                    >
                      {clearAgentBusy ? <Loader2 className="spin" size={12} /> : <Trash2 size={12} />}
                    </button>
                  </h3>
                  {equipped.length === 0 ? (
                    <p className="empty" style={{ textAlign: "center", padding: "24px 0", color: "var(--color-text-dim)", margin: 0 }}>
                      {selectedMemoryCard ? "点击上方“装入已选”即可挂载到此智能体。" : "暂无分配 Memory Card，先从左边点选一张项目卡片。"}
                    </p>
                  ) : (
                    <div style={{ display: "grid", gap: "6px" }}>
                      {equipped.map((row) => (
                        <button
                          key={`${row.memory_card_id}-${agent}`}
                          className="equipped-item"
                          draggable={!assignmentDisabled}
                          disabled={assignmentDisabled}
                          title={`点击卸载装载`}
                          onDragStart={(event) => startMemoryCardDrag(event, { id: row.memory_card_id, title: row.title, scope: row.scope ?? "project" })}
                          onClick={() => {
                            const currentTargets = agents.filter((a) => row.targets[a]);
                            const nextTargets = nextMemoryCardTargets(currentTargets, agent, false);
                            void onAction(
                              `切换-${row.memory_card_id}|${agent}`,
                              `已更新挂载信息`,
                              "set_memory_card_targets",
                              { id: row.memory_card_id, targets: nextTargets },
                            );
                          }}
                          style={{
                            borderRadius: "10px",
                            background: "var(--color-canvas)",
                            padding: "10px 14px",
                            border: "1px solid var(--color-border)",
                            display: "flex",
                            alignItems: "center",
                            width: "100%",
                            cursor: "pointer",
                            textAlign: "left",
                          }}
                        >
                          {togglingMemoryCardId === row.memory_card_id && togglingAgent === agent ? (
                            <Loader2 className="spin" size={12} style={{ marginRight: "8px" }} />
                          ) : (
                            <Check size={12} className="equip-check" style={{ color: "var(--color-success)", marginRight: "8px" }} />
                          )}
                          <span className="equip-name" style={{ fontWeight: "600", fontSize: "12.5px" }}>{row.title}</span>
                        </button>
                      ))}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}

function dedupeAvailableMemoryCards(memory_cards: MemoryCardRecord[], projectPath: string): MemoryCardRecord[] {
  const entries: MemoryCardRecord[] = [];
  const indexByKey = new Map<string, number>();

  for (const memory_card of memory_cards) {
    const key = logicalMemoryCardKey(memory_card);
    const existingIndex = indexByKey.get(key);
    if (existingIndex == null) {
      indexByKey.set(key, entries.length);
      entries.push(memory_card);
      continue;
    }

    const current = entries[existingIndex]!;
    if (!isProjectSource(current, projectPath) && isProjectSource(memory_card, projectPath)) {
      entries[existingIndex] = memory_card;
    }
  }

  return entries;
}

function logicalMemoryCardKey(memory_card: MemoryCardRecord): string {
  const id = memory_card.id.toLowerCase().replace(/^(project|global):/, "").trim();
  const title = normalizeMemoryCardText(memory_card.title);
  const bodyPrefix = normalizeMemoryCardText(memory_card.body).slice(0, 120);
  return `${id || title}|${title}|${bodyPrefix}`;
}

function normalizeMemoryCardText(value: string): string {
  return value.toLowerCase().replace(/\s+/g, " ").trim();
}

function isProjectSource(memory_card: MemoryCardRecord, projectPath: string): boolean {
  return memory_card.scope === "project" || memory_card.source_project === projectPath || memory_card.id.startsWith("project:");
}
