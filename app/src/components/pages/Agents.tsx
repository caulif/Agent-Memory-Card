import React from "react";
import { Check, Circle, GitBranch, Loader2, MessageSquareText, Save, Trash2 } from "lucide-react";
import { ActionButton, EmptyState, Panel } from "../common";
import {
  formatAgent,
  nextMemoryCardTargets,
  translateKind,
  type ProjectAssignmentView,
  type ProjectMemoryCardLibrary,
  type ProjectQualityView,
  type ProjectSnapshot,
  type PanelPageProps,
  type RegisteredProject,
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
  quality: ProjectQualityView | null;
  projects: RegisteredProject[];
}) {
  const agents = assignment?.target_matrix.agents ?? snapshot?.target_matrix.agents ?? [];
  const rows = assignment?.target_matrix.rows ?? snapshot?.target_matrix.rows ?? [];
  const memory_cards = library?.memory_cards ?? snapshot?.memory_cards ?? [];
  const globalMemoryCards = library?.global_memory_cards ?? snapshot?.global_memory_cards ?? [];
  const togglingKey = pendingAction.startsWith("切换-") ? pendingAction : "";
  const [dragOverAgent, setDragOverAgent] = React.useState<string | null>(null);
  const [hasUnwrittenChanges, setHasUnwrittenChanges] = React.useState(false);
  const [reloadPromptStatus, setReloadPromptStatus] = React.useState("");
  const projectLabel = React.useMemo(() => {
    const path = assignment?.project_path ?? snapshot?.project_path ?? "";
    return path.split(/[\\/]/).filter(Boolean).pop() || "当前项目";
  }, [assignment?.project_path, snapshot?.project_path]);
  const projectPath = assignment?.project_path ?? snapshot?.project_path ?? "";
  const allAvailable = React.useMemo(
    () => dedupeAvailableMemoryCards([...memory_cards, ...globalMemoryCards], projectPath),
    [globalMemoryCards, projectPath, memory_cards],
  );
  const assignmentDisabled = disabled;
  const syncBusy = pendingAction === "写入-Agent-文件";
  const clearAllBusy = pendingAction === "清空-全部分配";

  React.useEffect(() => {
    setHasUnwrittenChanges(false);
    setReloadPromptStatus("");
  }, [projectPath]);

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

  async function runAssignmentAction(
    actionKey: string,
    doneMessage: string,
    command: string,
    args: Record<string, unknown>,
  ) {
    await onAction(actionKey, doneMessage, command, args);
    setHasUnwrittenChanges(true);
    setReloadPromptStatus("");
  }

  function assignMemoryCardToAgent(memory_cardId: string, agent: string) {
    const row = rows.find((item) => item.memory_card_id === memory_cardId);
    const memory_card = allAvailable.find((item) => item.id === memory_cardId);
    const title = row?.title ?? memory_card?.title ?? memory_cardId;
    const currentTargets = row ? agents.filter((a) => row.targets[a]) : [];
    const nextTargets = nextMemoryCardTargets(currentTargets, agent, true);
    void runAssignmentAction(
      `切换-${memory_cardId}|${agent}`,
      `已更新"${title}"的目标智能体`,
      "set_memory_card_targets",
      { id: memory_cardId, targets: nextTargets },
    );
  }

  function clearAgentAssignments(agent: string) {
    void runAssignmentAction(
      `清空-${agent}`,
      `已清空 ${formatAgent(agent)} 的分配`,
      "clear_memory_card_targets",
      { agent },
    );
  }

  function clearAllAssignments() {
    void runAssignmentAction(
      "清空-全部分配",
      "已清空本项目全部分配",
      "clear_memory_card_targets",
      { agent: null },
    );
  }

  async function syncAgentFiles() {
    await onAction(
      "写入-Agent-文件",
      "已写入 Agent 文件。新会话会自动生效；当前会话可使用重读提示。",
      "sync_project",
      {},
    );
    setHasUnwrittenChanges(false);
  }

  async function copyReloadPrompt() {
    const prompt = `请重新读取本项目的 AGENTS.md / CLAUDE.md 以及 .agents/.claude skills，并在当前会话中遵循最新 Enabled Memory Cards。项目路径：${projectPath}`;
    try {
      await navigator.clipboard.writeText(prompt);
      setReloadPromptStatus("已复制重读提示");
    } catch {
      setReloadPromptStatus(prompt);
    }
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

  return (
    <div className="stack">
      {/* ===== Loadout 极简大盘头部 ===== */}
      <Panel title="Memory Card Loadout" subtitle="将左侧建议池的 Memory Card 挂载分配至右侧对应的目标智能体并一键同步写入。">
        {agents.length === 0 ? (
          <EmptyState title="暂无目标智能体" description="当前项目未检测到 Codex 或 Claude Code 配置。" />
        ) : (
          <>
            <div className="target-scope-row">
              <div className="target-scope-label">
                <strong>当前项目大纲</strong>
                <span>{projectLabel}</span>
              </div>
              <div className="target-scope-current" style={{ marginLeft: "auto", marginRight: "24px" }}>
                <strong style={{ color: hasUnwrittenChanges ? "var(--color-accent)" : "var(--color-success)" }}>
                  {hasUnwrittenChanges ? "● 存在未保存的分配改动" : "● 分配产物已完美保持同步"}
                </strong>
              </div>
              <div className="assignment-toolbar">
                <ActionButton
                  icon={Save}
                  label="写入 Agent 文件生效"
                  busyLabel="写入中"
                  busy={syncBusy}
                  disabled={disabled || syncBusy}
                  onClick={syncAgentFiles}
                />
                <ActionButton
                  icon={MessageSquareText}
                  label="一键复制重读提示"
                  busyLabel="准备中"
                  busy={false}
                  disabled={!projectPath}
                  onClick={copyReloadPrompt}
                />
                <ActionButton
                  variant="danger"
                  icon={Trash2}
                  label="清空本项目全部分配"
                  busyLabel="清空中"
                  busy={clearAllBusy}
                  disabled={assignmentDisabled || clearAllBusy || rows.length === 0}
                  onClick={clearAllAssignments}
                />
              </div>
            </div>
            {reloadPromptStatus ? <div className="target-apply-note" style={{ borderRadius: "12px" }}>{reloadPromptStatus}</div> : null}
          </>
        )}
      </Panel>

      {/* ===== 双栏极简左右互锁分配区 ===== */}
      <div className="drafts-grid" style={{ gridTemplateColumns: "1.1fr 0.9fr", gap: "28px" }}>
        {/* 左栏：可用 Memory Cards 卡池 */}
        <div className="skill-pool" style={{ borderRadius: "20px", padding: "24px" }}>
          <div className="skill-pool-head" style={{ marginBottom: "16px" }}>
            <h3 style={{ fontSize: "14px", fontWeight: "750" }}>可用 Memory Card 规则池</h3>
            <span style={{ fontSize: "11px", color: "var(--color-text-dim)" }}>
              你可以直接选择规则卡片拖动到右侧目标 Agent 中进行快速绑定装填。
            </span>
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
                    <h4 style={{ fontSize: "10.5px", color: "var(--color-text-dim)", textTransform: "uppercase" }}>
                      {kind === "other" ? "其他" : translateKind(kind)}
                    </h4>
                    <div className="pool-items" style={{ display: "flex", flexWrap: "wrap", gap: "6px", marginTop: "6px" }}>
                      {items.map((s) => {
                        const equipped = isEquippedAnywhere(s.id);
                        return (
                          <div
                            key={s.id}
                            role="button"
                            tabIndex={assignmentDisabled ? -1 : 0}
                            className={`pool-item ${equipped ? "equipped" : ""}`}
                            draggable={!assignmentDisabled}
                            onDragStart={(event) => startMemoryCardDrag(event, s)}
                            onDragEnd={() => setDragOverAgent(null)}
                            aria-disabled={assignmentDisabled}
                            title={s.title}
                            onKeyDown={(event) => {
                              if (assignmentDisabled || event.key !== "Enter" || agents.length === 0) return;
                              assignMemoryCardToAgent(s.id, agents[0]!);
                            }}
                            style={{
                              borderRadius: "8px",
                              padding: "6px 12px",
                              border: equipped ? "1.5px solid var(--color-success)" : "1px dashed var(--color-border)",
                              boxShadow: "var(--shadow-sm)"
                            }}
                          >
                            {equipped ? <Check size={12} className="pool-check" style={{ color: "var(--color-success)" }} /> : <Circle size={12} />}
                            <span className="pool-title" style={{ fontWeight: equipped ? "650" : "500" }}>{s.title}</span>
                            <span className="pool-source" style={{ borderRadius: "4px" }}>{sourceScopeLabel(s)}</span>
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
          {agents.map((agent) => {
            const equipped = getEquippedFor(agent);
            const clearAgentBusy = pendingAction === `清空-${agent}`;
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
                  boxShadow: "var(--shadow-md)"
                }}
              >
                <h3 style={{ display: "flex", alignItems: "center", borderBottom: "1px dashed var(--color-border)", paddingBottom: "12px", marginBottom: "12px" }}>
                  <span className="loadout-dot" style={{ background: "var(--color-accent-ring)" }} />
                  {formatAgent(agent)}
                  <span className="loadout-count" style={{ borderRadius: "8px", fontSize: "11px", fontWeight: "700" }}>
                    已分配 {equipped.length} 条记忆规则
                  </span>
                  <button
                    className="icon-action"
                    disabled={assignmentDisabled || clearAgentBusy || equipped.length === 0}
                    title={`清空 ${formatAgent(agent)} 的全部分配`}
                    aria-label={`清空 ${formatAgent(agent)} 的全部分配`}
                    onClick={() => clearAgentAssignments(agent)}
                    style={{ marginLeft: "12px", width: "28px", height: "28px", borderRadius: "8px" }}
                  >
                    {clearAgentBusy ? <Loader2 className="spin" size={12} /> : <Trash2 size={12} />}
                  </button>
                </h3>
                {equipped.length === 0 ? (
                  <p className="empty" style={{ textAlign: "center", padding: "24px 0", color: "var(--color-text-dim)" }}>
                    暂无分配 Memory Card，可从左边拖入或点击绑定。
                  </p>
                ) : (
                  <div style={{ display: "grid", gap: "6px" }}>
                    {equipped.map((row) => (
                      <button
                        key={`${row.memory_card_id}-${agent}`}
                        className="equipped-item"
                        draggable={!assignmentDisabled}
                        disabled={assignmentDisabled}
                        title={`点击取消分配给 ${formatAgent(agent)}`}
                        onDragStart={(event) => startMemoryCardDrag(event, { id: row.memory_card_id, title: row.title, scope: row.scope ?? "project" })}
                        onClick={() => {
                          const currentTargets = agents.filter((a) => row.targets[a]);
                          const nextTargets = nextMemoryCardTargets(currentTargets, agent, false);
                          void runAssignmentAction(
                            `切换-${row.memory_card_id}|${agent}`,
                            `已更新"${row.title}"的目标智能体`,
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
                          alignItems: "center"
                        }}
                      >
                        {togglingMemoryCardId === row.memory_card_id && togglingAgent === agent ? (
                          <Loader2 className="spin" size={12} />
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

function sourceScopeLabel(memory_card: MemoryCardRecord): string {
  return memory_card.scope === "global" || memory_card.id.startsWith("global:") ? "来源：全局" : "来源：项目";
}
