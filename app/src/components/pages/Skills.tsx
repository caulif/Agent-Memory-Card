import React from "react";
import { CheckCircle2, Link2, RefreshCw } from "lucide-react";
import { ActionButton, EmptyState } from "../common";
import {
  type MemoryCardRecord,
  type ProjectAction,
  type ProjectSkillLibrary,
} from "../../ui-helpers";

export function Skills({
  skillLibrary,
  actionState,
  disabled,
  dispatchAction,
}: {
  skillLibrary: ProjectSkillLibrary | null;
  actionState: string;
  disabled: boolean;
  dispatchAction: ProjectAction;
}) {
  const skills = skillLibrary?.skills ?? [];
  const [activeSkillId, setActiveSkillId] = React.useState("");
  const [searchQuery, setSearchQuery] = React.useState("");
  const [selectedKind, setSelectedKind] = React.useState("all");
  const [locallyAttachedCards, setLocallyAttachedCards] = React.useState<Record<string, string[]>>({});

  const visibleSkills = React.useMemo(() => {
    return skills.map((skill) => {
      const localIds = locallyAttachedCards[skill.id] ?? [];
      if (localIds.length === 0) return skill;

      const linkedIds = new Set(skill.linked_memory_cards.map((card) => card.id));
      const locallyLinked = skill.recommended_memory_cards.filter((card) => localIds.includes(card.id) && !linkedIds.has(card.id));
      if (locallyLinked.length === 0) return skill;

      return {
        ...skill,
        linked_memory_cards: [...skill.linked_memory_cards, ...locallyLinked],
        recommended_memory_cards: skill.recommended_memory_cards.filter((card) => !localIds.includes(card.id)),
      };
    });
  }, [locallyAttachedCards, skills]);

  const kinds = React.useMemo(() => {
    const list = new Set<string>();
    for (const s of visibleSkills) {
      if (s.source_kind) list.add(s.source_kind);
    }
    return ["all", ...Array.from(list)];
  }, [visibleSkills]);

  const filteredSkills = React.useMemo(() => {
    const query = searchQuery.toLowerCase();
    return visibleSkills.filter((skill) => {
      const nameMatch = skill.name?.toLowerCase().includes(query);
      const descMatch = skill.description?.toLowerCase().includes(query);
      const cardMatch = [...skill.linked_memory_cards, ...skill.recommended_memory_cards].some((card) =>
        `${card.title} ${card.brief ?? ""} ${card.body}`.toLowerCase().includes(query),
      );
      const kindMatch = selectedKind === "all" || skill.source_kind === selectedKind;
      return (nameMatch || descMatch || cardMatch) && kindMatch;
    });
  }, [visibleSkills, searchQuery, selectedKind]);

  const activeSkill = React.useMemo(
    () => filteredSkills.find((skill) => skill.id === activeSkillId) ?? filteredSkills[0] ?? null,
    [activeSkillId, filteredSkills],
  );

  if (skills.length === 0) {
    return (
      <div className="skills-workbench">
        <EmptyState
          title="暂无 Skills 注册"
          description="点击下方扫描真实 Skills，系统会读取当前项目、Codex、Claude Code、superpowers 和插件缓存中的 SKILL.md。"
        />
        <div className="empty-state-actions">
          <ActionButton
            variant="primary"
            icon={RefreshCw}
            label="扫描真实 Skills"
            busyLabel="扫描中"
            busy={actionState === "扫描 Skills"}
            disabled={disabled}
            onClick={() => dispatchAction("扫描 Skills", "已刷新真实 Skill Registry", "import_project", { scanHome: true })}
          />
        </div>
      </div>
    );
  }

  return (
    <div className="skills-workbench" style={{ display: "grid", gap: "20px" }}>
      {/* ===== 极简总览统计与检索过滤栏 ===== */}
      <div className="skills-toolbar" style={{ marginBottom: "8px" }}>
        <div className="skills-toolbar-copy">
          <h2 style={{ margin: 0, fontSize: "20px", fontWeight: "bold" }}>项目技能字典</h2>
          <p style={{ margin: "4px 0 0 0", color: "var(--color-text-secondary)", fontSize: "13px" }}>
            查阅当前注册生效的工具集契约规范，这是智能体在当前项目被授予的专属拓展本领。
          </p>
          {skillLibrary ? (
            <p className="skills-source-line">
              扫描时间 {skillLibrary.generated_at || "未记录"} · {formatSourceCounts(skillLibrary.source_counts)}
            </p>
          ) : null}
        </div>
        <div className="skills-filter-row">
          <ActionButton
            variant="secondary"
            icon={RefreshCw}
            label="扫描真实 Skills"
            busyLabel="扫描中"
            busy={actionState === "扫描 Skills"}
            disabled={disabled}
            onClick={() => dispatchAction("扫描 Skills", "已刷新真实 Skill Registry", "import_project", { scanHome: true })}
          />
          {kinds.length > 2 ? (
            <select
              value={selectedKind}
              onChange={(e) => setSelectedKind(e.target.value)}
              style={{
                padding: "8px 12px",
                borderRadius: "10px",
                border: "1px solid var(--color-border)",
                background: "var(--color-surface)",
                fontSize: "12px",
              }}
            >
              {kinds.map((k) => (
                <option key={k} value={k}>
                  {k === "all" ? "全部类型" : k}
                </option>
              ))}
            </select>
          ) : null}
          <input
            type="text"
            placeholder="检索核心技能编码或描述..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            style={{
              padding: "8px 14px",
              borderRadius: "10px",
              border: "1px solid var(--color-border)",
              background: "var(--color-surface)",
              fontSize: "12px",
              width: "min(240px, 100%)",
            }}
          />
        </div>
      </div>

      {/* ===== 双栏极简技能辞典布局 ===== */}
      <div className="skills-grid" style={{ display: "grid", gap: "28px" }}>
        {/* 左半边：技能目录选择区 */}
        <section className="skill-list" aria-label="Skill 列表" style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
          <div style={{ fontSize: "11px", fontWeight: "bold", textTransform: "uppercase", color: "var(--color-text-dim)", marginBottom: "4px" }}>
            已注册的核心技能 ({filteredSkills.length})
          </div>
          <div style={{ display: "flex", flexDirection: "column", gap: "8px", maxHeight: "600px", overflowY: "auto" }}>
            {filteredSkills.map((skill) => (
              <button
                key={skill.id}
                className={`skill-row ${activeSkill?.id === skill.id ? "active" : ""}`}
                type="button"
                onClick={() => setActiveSkillId(skill.id)}
                style={{
                  borderRadius: "12px",
                  padding: "12px 14px",
                  border: activeSkill?.id === skill.id ? "1.5px solid var(--color-accent)" : "1px solid var(--color-border)",
                  background: activeSkill?.id === skill.id ? "rgba(120, 110, 95, 0.03)" : "var(--color-surface)",
                  textAlign: "left",
                  cursor: "pointer",
                  width: "100%",
                }}
              >
                <span style={{ fontWeight: activeSkill?.id === skill.id ? "700" : "600", fontSize: "13px", display: "block" }}>{skill.name}</span>
                <small style={{ marginTop: "4px", display: "block", color: "var(--color-text-dim)", fontSize: "11px" }}>
                  类型: {skill.source_kind} | 核心工具集成: {skill.mirror_targets?.length ? `${skill.mirror_targets.length} 处` : "内置"}
                </small>
              </button>
            ))}
          </div>
        </section>

        {/* 右半边：选定技能详情视窗 */}
        {activeSkill ? (
          <section
            className="skill-detail"
            aria-label="Skill 详情"
            style={{
              background: "var(--color-surface)",
              borderRadius: "20px",
              padding: "24px",
              border: "1px solid var(--color-border)",
              boxShadow: "var(--shadow-md)",
              display: "flex",
              flexDirection: "column",
              gap: "20px",
            }}
          >
            <div className="skill-detail-head" style={{ borderBottom: "1px dashed var(--color-border)", paddingBottom: "16px" }}>
              <span className="tag" style={{ borderRadius: "6px", fontSize: "10px", textTransform: "uppercase", padding: "3px 8px" }}>
                SKILL MODULE · {activeSkill.source_kind}
              </span>
              <h2 style={{ fontSize: "18px", fontWeight: "750", marginTop: "8px", margin: "8px 0 0 0" }}>{activeSkill.name}</h2>
              <p style={{ marginTop: "10px", lineHeight: "1.6", color: "var(--color-text-secondary)", fontSize: "13px", margin: "10px 0 0 0" }}>
                {activeSkill.description || "该技能已全局向系统核心和智能体授权，无特定约束条件限制。"}
              </p>
            </div>

            <div className="skill-meta-grid" style={{ display: "grid", gap: "16px" }}>
              <div style={{ padding: "12px", borderRadius: "10px", background: "var(--color-canvas)", border: "1px solid var(--color-border)" }}>
                <span style={{ fontSize: "11px", color: "var(--color-text-dim)" }}>技能映射路径</span>
                <strong
                  title={activeSkill.source_path}
                  style={{
                    fontSize: "12px",
                    color: "var(--color-text-muted)",
                    display: "block",
                    marginTop: "4px",
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                    whiteSpace: "nowrap",
                    fontFamily: "var(--font-mono)",
                  }}
                >
                  {activeSkill.source_path}
                </strong>
              </div>
              <div style={{ padding: "12px", borderRadius: "10px", background: "var(--color-canvas)", border: "1px solid var(--color-border)" }}>
                <span style={{ fontSize: "11px", color: "var(--color-text-dim)" }}>可用工具定义</span>
                <strong style={{ fontSize: "12px", color: "var(--color-text-primary)", display: "block", marginTop: "4px" }}>
                  {activeSkill.mirror_targets && activeSkill.mirror_targets.length > 0
                    ? activeSkill.mirror_targets.join(" 、 ")
                    : "由核心协议直接宿主挂载"}
                </strong>
              </div>
            </div>

            <SkillOptimizationPanel
              skillId={activeSkill.id}
              linkedCards={activeSkill.linked_memory_cards}
              recommendedCards={activeSkill.recommended_memory_cards}
              disabled={disabled}
              actionState={actionState}
              dispatchAction={dispatchAction}
              onAttached={(cardId) => {
                setLocallyAttachedCards((current) => ({
                  ...current,
                  [activeSkill.id]: Array.from(new Set([...(current[activeSkill.id] ?? []), cardId])),
                }));
              }}
            />

            <div
              style={{
                display: "flex",
                alignItems: "center",
                gap: "8px",
                color: "var(--color-success)",
                fontSize: "12px",
                borderTop: "1px solid var(--color-border)",
                paddingTop: "16px",
                marginTop: "12px",
              }}
            >
              <span
                style={{
                  width: "6px",
                  height: "6px",
                  borderRadius: "50%",
                  background: activeSkill.warnings.length > 0 ? "var(--color-warning)" : "var(--color-success)",
                }}
              ></span>
              <span>
                {activeSkill.warnings.length > 0
                  ? activeSkill.warnings.slice(0, 2).join("；")
                  : "该技能契约处于稳定运行状态，可按上方建议逐步补强上下文。"}
              </span>
            </div>
          </section>
        ) : (
          <div style={{ padding: "40px", border: "1px dashed var(--color-border)", borderRadius: "20px", display: "flex", justifyContent: "center", alignItems: "center" }}>
            <span style={{ color: "var(--color-text-dim)" }}>无匹配的技能定义，请清除筛选条件</span>
          </div>
        )}
      </div>
    </div>
  );
}

function SkillOptimizationPanel({
  skillId,
  linkedCards,
  recommendedCards,
  disabled,
  actionState,
  dispatchAction,
  onAttached,
}: {
  skillId: string;
  linkedCards: MemoryCardRecord[];
  recommendedCards: MemoryCardRecord[];
  disabled: boolean;
  actionState: string;
  dispatchAction: ProjectAction;
  onAttached: (cardId: string) => void;
}) {
  const visibleLinked = linkedCards.slice(0, 3);
  const visibleRecommended = recommendedCards.slice(0, 4);

  return (
    <div
      className="skill-optimization-grid"
      style={{
        borderTop: "1px solid var(--color-border)",
        paddingTop: "18px",
        display: "grid",
        gap: "16px",
      }}
    >
      <section aria-label="已纳入 Skill 的上下文" style={{ display: "grid", gap: "10px" }}>
        <div>
          <strong style={{ fontSize: "12px" }}>已纳入上下文</strong>
          <p style={{ margin: "4px 0 0", color: "var(--color-text-dim)", fontSize: "11.5px", lineHeight: 1.5 }}>
            这些 Memory Card 会作为 Skill 的补充说明参与后续镜像与同步。
          </p>
        </div>
        {visibleLinked.length === 0 ? (
          <p style={{ margin: 0, color: "var(--color-text-muted)", fontSize: "12px", lineHeight: 1.55 }}>
            暂无专属补充。右侧建议若符合此 Skill 的使用边界，可以直接纳入。
          </p>
        ) : (
          <div style={{ display: "grid", gap: "8px" }}>
            {visibleLinked.map((card) => (
              <MemoryCardMini key={card.id} card={card} tone="linked" />
            ))}
          </div>
        )}
      </section>

      <section aria-label="可纳入 Skill 的优化建议" style={{ display: "grid", gap: "10px" }}>
        <div>
          <strong style={{ fontSize: "12px" }}>可采纳的优化建议</strong>
          <p style={{ margin: "4px 0 0", color: "var(--color-text-dim)", fontSize: "11.5px", lineHeight: 1.5 }}>
            来自已批准的 Memory Card，优先展示适合作为 Skill 流程补充的条目。
          </p>
        </div>
        {visibleRecommended.length === 0 ? (
          <p style={{ margin: 0, color: "var(--color-text-muted)", fontSize: "12px", lineHeight: 1.55 }}>
            当前没有新的建议。继续在 Review Inbox 吸纳流程型建议后，这里会自动出现可补强项。
          </p>
        ) : (
          <div style={{ display: "grid", gap: "8px" }}>
            {visibleRecommended.map((card) => {
              const key = `补强-${skillId}|${card.id}`;
              return (
                <div
                  key={card.id}
                  style={{
                    border: "1px solid var(--color-border)",
                    borderRadius: "12px",
                    padding: "12px",
                    background: "var(--color-canvas)",
                    display: "grid",
                    gap: "10px",
                  }}
                >
                  <MemoryCardMini card={card} tone="recommended" />
                  <ActionButton
                    variant="secondary"
                    icon={Link2}
                    label="纳入此 Skill"
                    busyLabel="正在纳入"
                    busy={actionState === key}
                    disabled={disabled}
                    onClick={async () => {
                      await dispatchAction(key, "已纳入 Skill 上下文", "attach_memory_card_to_skill", {
                        skillId,
                        memoryCardId: card.id,
                        fusionMode: "manual",
                      });
                      onAttached(card.id);
                    }}
                  />
                  <ActionButton
                    variant="primary"
                    icon={Link2}
                    label="自动融合"
                    busyLabel="融合中"
                    busy={actionState === `融合-${skillId}|${card.id}`}
                    disabled={disabled}
                    onClick={async () => {
                      await dispatchAction(`融合-${skillId}|${card.id}`, "已自动融合进 Skill 上下文", "attach_memory_card_to_skill", {
                        skillId,
                        memoryCardId: card.id,
                        fusionMode: "auto",
                      });
                      onAttached(card.id);
                    }}
                  />
                </div>
              );
            })}
          </div>
        )}
      </section>
    </div>
  );
}

function formatSourceCounts(sourceCounts: Record<string, number>) {
  const entries = Object.entries(sourceCounts ?? {});
  if (entries.length === 0) return "未发现来源";
  return entries
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([kind, count]) => `${kind} ${count}`)
    .join(" / ");
}

function MemoryCardMini({ card, tone }: { card: MemoryCardRecord; tone: "linked" | "recommended" }) {
  return (
    <article
      style={{
        border: "1px solid var(--color-border)",
        borderRadius: "10px",
        padding: "10px 12px",
        background: tone === "linked" ? "var(--color-surface-raised)" : "var(--color-surface)",
      }}
    >
      <div style={{ display: "flex", gap: "7px", alignItems: "center" }}>
        {tone === "linked" ? <CheckCircle2 size={13} color="var(--color-success)" /> : <Link2 size={13} color="var(--color-text-muted)" />}
        <strong style={{ fontSize: "12.5px", lineHeight: 1.35 }}>{card.title}</strong>
      </div>
      <p style={{ margin: "6px 0 0", color: "var(--color-text-secondary)", fontSize: "11.5px", lineHeight: 1.5 }}>
        {card.brief || card.body.slice(0, 96)}
      </p>
    </article>
  );
}
