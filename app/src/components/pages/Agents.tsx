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
  quality,
  pendingAction,
  disabled,
  onAction,
  projects,
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
  const verification = quality?.build_preview.verification ?? null;
  const ruleCi = verification?.rule_ci ?? quality?.rule_ci ?? snapshot?.rule_ci ?? null;

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
    const prompt = verification?.reload_prompt ?? `请重新读取本项目的 AGENTS.md / CLAUDE.md 以及 .agents/.claude skills，并在当前会话中遵循最新 Enabled Memory Cards。项目路径：${projectPath}`;
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

  /** 计算覆盖率 */
  const totalMemoryCards = allAvailable.length;
  const assignedMemoryCards = rows.filter((row) => Object.values(row.targets).some(Boolean)).length;
  const coveragePercent = totalMemoryCards > 0 ? Math.round((assignedMemoryCards / totalMemoryCards) * 100) : 0;
  const unassignedCount = totalMemoryCards - assignedMemoryCards;

  return (
    <div className="stack">
      {/* ===== Loadout 标题 ===== */}
      <Panel title="Memory Card Loadout" subtitle="为当前项目配置记忆卡；分配后写入 Agent 文件才会更新生成产物。" icon={GitBranch}>
        {agents.length === 0 ? (
          <EmptyState title="暂无目标智能体" description="当前项目未检测到 Codex 或 Claude Code 配置。" />
        ) : (
          <>
            <div className="target-scope-row">
              <div className="target-scope-label">
                <strong>当前项目</strong>
                <span>{projectLabel}</span>
              </div>
              <div className="target-scope-current">
                <strong>{hasUnwrittenChanges ? "有未写入变更" : "生成产物待检查"}</strong>
                <span>{projectPath}</span>
              </div>
              <div className="assignment-toolbar">
                <ActionButton
                  icon={Save}
                  label="写入 Agent 文件"
                  busyLabel="写入中"
                  busy={syncBusy}
                  disabled={disabled || syncBusy}
                  onClick={syncAgentFiles}
                />
                <ActionButton
                  icon={MessageSquareText}
                  label="提示重读"
                  busyLabel="准备中"
                  busy={false}
                  disabled={!projectPath}
                  onClick={copyReloadPrompt}
                />
                <ActionButton
                  variant="danger"
                  icon={Trash2}
                  label="清空本项目分配"
                  busyLabel="清空中"
                  busy={clearAllBusy}
                  disabled={assignmentDisabled || clearAllBusy || rows.length === 0}
                  onClick={clearAllAssignments}
                />
              </div>
            </div>
            {reloadPromptStatus ? <div className="target-apply-note">{reloadPromptStatus}</div> : null}
            <div className={`sync-verification-card ${verification?.status ?? "pending"}`}>
              <div>
                <span>Sync Verification</span>
                <strong>
                  Rule CI {ruleCi?.passed ?? 0} passed / {ruleCi?.failed ?? 0} failed
                </strong>
              </div>
              {(verification?.next_actions ?? ["写入 Agent 文件后，这里会显示验证结果和下一步。"]).slice(0, 2).map((action) => (
                <p key={action}>{action}</p>
              ))}
            </div>
            <div className="loadout-grid">
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
                  >
                    <h3>
                      <span className="loadout-dot" />
                      {formatAgent(agent)}
                      <span className="loadout-count">
                        {equipped.length} 已装备
                      </span>
                      <button
                        className="icon-action"
                        disabled={assignmentDisabled || clearAgentBusy || equipped.length === 0}
                        title={`清空 ${formatAgent(agent)} 的全部分配`}
                        aria-label={`清空 ${formatAgent(agent)} 的全部分配`}
                        onClick={() => clearAgentAssignments(agent)}
                      >
                        {clearAgentBusy ? <Loader2 className="spin" size={13} /> : <Trash2 size={13} />}
                      </button>
                    </h3>
                    {equipped.length === 0 ? (
                      <p className="empty" style={{ textAlign: "center", padding: "12px 0" }}>
                        暂无装备的 Memory Card
                      </p>
                    ) : (
                      equipped.map((row) => (
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
                        >
                          {togglingMemoryCardId === row.memory_card_id && togglingAgent === agent ? (
                            <Loader2 className="spin" size={13} />
                          ) : (
                            <Check size={13} className="equip-check" />
                          )}
                          <span className="equip-name">{row.title}</span>
                          <span className="equip-kind">
                            {row.scope === "global" ? "全局" : "项目"}
                          </span>
                        </button>
                      ))
                    )}
                  </div>
                );
              })}
            </div>
          </>
        )}
      </Panel>

      {/* ===== Memory Card 池 ===== */}
      <div className="skill-pool">
        <div className="skill-pool-head">
          <h3>可用 Memory Cards</h3>
          <span>拖到上方智能体完成 Loadout 配置</span>
        </div>
        {allAvailable.length === 0 ? (
          <EmptyState title="暂无可用 Memory Card" description="批准建议后，这里会显示可配置到 Loadout 的 Memory Card。" />
        ) : (
          <div className="pool-groups">
            {(["rule", "procedure", "constraint", "preference", "other"] as const).map((kind) => {
              const items = kind === "other"
                ? allAvailable.filter((item) => !["rule", "procedure", "constraint", "preference"].includes(item.kind))
                : poolByKind[kind];
              if (!items || items.length === 0) return null;
              return (
                <div className="pool-group" key={kind}>
                  <h4>{kind === "other" ? "其他" : translateKind(kind)}</h4>
                  <div className="pool-items">
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
                        >
                          {equipped ? <Check size={12} className="pool-check" /> : <Circle size={12} />}
                          <span className="pool-title">{s.title}</span>
                          <span className="pool-source">{sourceScopeLabel(s)}</span>
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

      {/* ===== 分配矩阵（原有功能保留） ===== */}
      <Panel title="Loadout Matrix" subtitle="精细管理每个 Memory Card 到每个智能体的分配关系。" icon={GitBranch}>
        {rows.length === 0 ? (
          <EmptyState title="暂无 Loadout 数据" description="安装包或批准 Memory Card 后，这里会显示目标智能体矩阵。" />
        ) : (
          <div className="matrix" style={{ gridTemplateColumns: `minmax(180px, 1fr) repeat(${Math.max(agents.length, 1)}, 100px)` }}>
            <div className="matrix-head">Memory Card</div>
            {agents.map((agent) => (
              <div className="matrix-head" key={agent}>
                {formatAgent(agent)}
              </div>
            ))}
            {rows.flatMap((row) => {
              const currentTargets = agents.filter((a) => row.targets[a]);
              return [
                <div key={`${row.memory_card_id}-title`} className="matrix-title">
                  {row.title}
                  <span className="matrix-scope">{row.scope === "global" ? "全局来源" : "项目"}</span>
                </div>,
                ...agents.map((agent) => {
                  const isOn = row.targets[agent];
                  const isToggling = togglingMemoryCardId === row.memory_card_id && togglingAgent === agent;
                  const nextTargets = nextMemoryCardTargets(currentTargets, agent, !isOn);
                  return (
                    <button
                      key={`${row.memory_card_id}-${agent}`}
                      className={`matrix-cell interactive ${isOn ? "on" : "off"} ${isToggling ? "toggling" : ""}`}
                      disabled={assignmentDisabled}
                      title={isOn ? `点击取消分配给 ${formatAgent(agent)}` : `点击分配给 ${formatAgent(agent)}`}
                      onClick={() =>
                        runAssignmentAction(
                          `切换-${row.memory_card_id}|${agent}`,
                          `已更新"${row.title}"的目标智能体`,
                          "set_memory_card_targets",
                          { id: row.memory_card_id, targets: nextTargets },
                        )
                      }
                    >
                      {isToggling ? (
                        <Loader2 className="spin" size={14} />
                      ) : isOn ? (
                        <Check size={15} />
                      ) : (
                        <Circle size={13} />
                      )}
                    </button>
                  );
                }),
              ];
            })}
          </div>
        )}
      </Panel>
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
