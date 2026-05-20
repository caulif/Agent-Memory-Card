import React from "react";
import { CheckCircle2, Link2, RefreshCw, X } from "lucide-react";
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
  const [selectedScope, setSelectedScope] = React.useState<"project" | "global">("project");
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
    for (const s of visibleSkills.filter((skill) => skillScope(skill) === selectedScope)) {
      if (s.source_kind) list.add(s.source_kind);
    }
    return ["all", ...Array.from(list)];
  }, [selectedScope, visibleSkills]);

  const filteredSkills = React.useMemo(() => {
    const query = searchQuery.toLowerCase();
    return visibleSkills.filter((skill) => {
      if (skillScope(skill) !== selectedScope) return false;
      const nameMatch = skill.name?.toLowerCase().includes(query);
      const descMatch = skill.description?.toLowerCase().includes(query);
      const cardMatch = [...skill.linked_memory_cards, ...skill.recommended_memory_cards].some((card) =>
        `${card.title} ${card.brief ?? ""} ${card.body}`.toLowerCase().includes(query),
      );
      const kindMatch = selectedKind === "all" || skill.source_kind === selectedKind;
      return (nameMatch || descMatch || cardMatch) && kindMatch;
    });
  }, [visibleSkills, searchQuery, selectedKind, selectedScope]);

  const projectSkillCount = React.useMemo(() => visibleSkills.filter((skill) => skillScope(skill) === "project").length, [visibleSkills]);
  const globalSkillCount = visibleSkills.length - projectSkillCount;

  const activeSkill = React.useMemo(
    () => filteredSkills.find((skill) => skill.id === activeSkillId) ?? filteredSkills[0] ?? null,
    [activeSkillId, filteredSkills],
  );
  const activeSkillIsProject = activeSkill ? skillScope(activeSkill) === "project" : false;

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
            先看项目级 Skills，再查全局来源；Memory Card 只融合到当前项目可治理的 Skill。
          </p>
          {skillLibrary ? (
            <p className="skills-source-line">
              扫描时间 {skillLibrary.generated_at || "未记录"} · {formatSourceCounts(skillLibrary.source_counts)}
            </p>
          ) : null}
        </div>
        <div className="skills-filter-row">
          <div className="skill-scope-tabs" role="tablist" aria-label="Skill 来源范围">
            <button
              className={selectedScope === "project" ? "active" : ""}
              type="button"
              role="tab"
              aria-selected={selectedScope === "project"}
              onClick={() => {
                setSelectedScope("project");
                setSelectedKind("all");
              }}
            >
              项目级 {projectSkillCount}
            </button>
            <button
              className={selectedScope === "global" ? "active" : ""}
              type="button"
              role="tab"
              aria-selected={selectedScope === "global"}
              onClick={() => {
                setSelectedScope("global");
                setSelectedKind("all");
              }}
            >
              全局 {globalSkillCount}
            </button>
          </div>
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
            {selectedScope === "project" ? "项目级 Skills" : "全局 Skills"} ({filteredSkills.length})
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
                  {skillScopeLabel(skill)} · {sourceKindLabel(skill.source_kind)} · {skill.mirror_targets?.length ? `${skill.mirror_targets.length} 个镜像目标` : "未镜像"}
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
                <span style={{ fontSize: "11px", color: "var(--color-text-dim)" }}>来源层级</span>
                <strong style={{ fontSize: "12px", color: "var(--color-text-primary)", display: "block", marginTop: "4px" }}>
                  {skillScopeLabel(activeSkill)} / {sourceKindLabel(activeSkill.source_kind)}
                </strong>
              </div>
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
              allowAttach={activeSkillIsProject}
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
                  : activeSkillIsProject
                    ? "该项目级 Skill 可按上方建议逐步补强上下文。"
                    : "该全局 Skill 仅作字典查阅；需要补强时请先在项目中声明对应 Skill。"}
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
  allowAttach,
  disabled,
  actionState,
  dispatchAction,
  onAttached,
}: {
  skillId: string;
  linkedCards: MemoryCardRecord[];
  recommendedCards: MemoryCardRecord[];
  allowAttach: boolean;
  disabled: boolean;
  actionState: string;
  dispatchAction: ProjectAction;
  onAttached: (cardId: string) => void;
}) {
  const visibleLinked = linkedCards.slice(0, 3);
  const visibleRecommended = recommendedCards.slice(0, 4);
  const [activeCardId, setActiveCardId] = React.useState<string | null>(null);

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
            {allowAttach
              ? "来自已批准的 Memory Card，先选择优化方式，再纳入当前项目 Skill。"
              : "全局 Skill 暂不直接写入项目补充；这里仅显示潜在匹配项。"}
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
              const fusionKey = `融合-${skillId}|${card.id}`;
              const choosing = activeCardId === card.id;
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
                  {allowAttach ? (
                    choosing ? (
                      <div className="skill-fusion-actions">
                        <ActionButton
                          variant="secondary"
                          icon={Link2}
                          label="直接纳入"
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
                            setActiveCardId(null);
                          }}
                        />
                        <ActionButton
                          variant="primary"
                          icon={Link2}
                          label="LLM 融合"
                          busyLabel="融合中"
                          busy={actionState === fusionKey}
                          disabled={disabled}
                          onClick={async () => {
                            await dispatchAction(fusionKey, "已自动融合进 Skill 上下文", "attach_memory_card_to_skill", {
                              skillId,
                              memoryCardId: card.id,
                              fusionMode: "auto",
                            });
                            onAttached(card.id);
                            setActiveCardId(null);
                          }}
                        />
                        <button
                          type="button"
                          className="ghost-action"
                          disabled={disabled || actionState === key || actionState === fusionKey}
                          onClick={() => setActiveCardId(null)}
                        >
                          <X size={14} />
                          取消
                        </button>
                      </div>
                    ) : (
                      <button
                        type="button"
                        className="secondary-action"
                        disabled={disabled}
                        onClick={() => setActiveCardId(card.id)}
                      >
                        <Link2 size={14} />
                        选择优化方式
                      </button>
                    )
                  ) : null}
                </div>
              );
            })}
          </div>
        )}
      </section>
    </div>
  );
}

function skillScope(skill: { id: string; source_kind: string; source_path: string }): "project" | "global" {
  return skill.source_kind === "project" || skill.id.startsWith("project:") ? "project" : "global";
}

function skillScopeLabel(skill: { id: string; source_kind: string; source_path: string }) {
  return skillScope(skill) === "project" ? "项目级 Skill" : "全局 Skill";
}

function sourceKindLabel(sourceKind: string) {
  const labels: Record<string, string> = {
    project: "项目目录",
    home: "用户目录",
    referenced: "全局引用",
    "codex-home": "Codex 用户目录",
    "codex-superpower": "Codex Superpower",
    superpowers: "Codex Superpower",
    "codex-plugin": "Codex 插件",
    plugin: "Codex 插件",
    "claude-home": "Claude Code 用户目录",
  };
  return labels[sourceKind] ?? sourceKind;
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
