import React from "react";
import { AlertTriangle, CheckCircle2, GitBranch, Loader2, Plus, Sparkles } from "lucide-react";
import { EmptyState } from "../common";
import {
  translateKind,
  translateScope,
  type MemoryCardRecord,
  type PanelPageProps,
  type ProjectMemoryCardLibrary,
  type ProjectQualityView,
  type ProjectSkillLibrary,
  type ProjectSkillView,
} from "../../ui-helpers";

export function Skills({
  skillLibrary,
  memoryLibrary,
  quality,
  pendingAction,
  disabled,
  onAction,
}: PanelPageProps & {
  skillLibrary: ProjectSkillLibrary | null;
  memoryLibrary: ProjectMemoryCardLibrary | null;
  quality: ProjectQualityView | null;
}) {
  const skills = skillLibrary?.skills ?? [];
  const [activeSkillId, setActiveSkillId] = React.useState("");
  const activeSkill = React.useMemo(
    () => skills.find((skill) => skill.id === activeSkillId) ?? skills[0] ?? null,
    [activeSkillId, skills],
  );
  const attachableCards = React.useMemo(() => {
    if (!activeSkill || !memoryLibrary) return [];
    const linked = new Set(activeSkill.linked_memory_cards.map((card) => card.id));
    const recommended = new Set(activeSkill.recommended_memory_cards.map((card) => card.id));
    return memoryLibrary.memory_cards
      .filter((card) => !linked.has(card.id))
      .sort((a, b) => Number(recommended.has(b.id)) - Number(recommended.has(a.id)) || a.title.localeCompare(b.title));
  }, [activeSkill, memoryLibrary]);

  if (skills.length === 0) {
    return (
      <EmptyState
        title="暂无 Skills"
        description="先导入项目或扫描 Skill 目录，系统会读取 SKILL.md 并生成 Skill Library。"
      />
    );
  }

  return (
    <div className="skills-workbench">
      <section className="skills-summary-strip" aria-label="Skills 摘要">
        <div>
          <strong>{skills.length}</strong>
          <span>Skills</span>
        </div>
        <div>
          <strong>{skills.filter((skill) => skill.mirror_targets.length > 0).length}</strong>
          <span>已镜像</span>
        </div>
        <div>
          <strong>{skills.reduce((total, skill) => total + skill.linked_memory_cards.length, 0)}</strong>
          <span>已嵌入卡片</span>
        </div>
        <div>
          <strong>{skills.reduce((total, skill) => total + skill.recommended_memory_cards.length, 0)}</strong>
          <span>推荐关联</span>
        </div>
      </section>

      <div className="skills-grid">
        <section className="skill-list" aria-label="Skill 列表">
          {skills.map((skill) => (
            <button
              key={skill.id}
              className={`skill-row ${activeSkill?.id === skill.id ? "active" : ""}`}
              type="button"
              onClick={() => setActiveSkillId(skill.id)}
            >
              <span>{skill.name}</span>
              <small>{skill.source_kind} · {skill.mirror_targets.length || 0} targets</small>
            </button>
          ))}
        </section>

        {activeSkill ? (
          <section className="skill-detail" aria-label="Skill 详情">
            <div className="skill-detail-head">
              <div>
                <span className="tag">Skill · {activeSkill.source_kind}</span>
                <h2>{activeSkill.name}</h2>
                <p>{activeSkill.description || "暂无描述。"}</p>
              </div>
              <button
                className="secondary-action"
                disabled={disabled || pendingAction === "同步"}
                type="button"
                onClick={() => void onAction("同步", "已开始同步 Agent 文件。", "sync_project")}
              >
                {pendingAction === "同步" ? <Loader2 className="spin" size={15} /> : <GitBranch size={15} />}
                同步
              </button>
            </div>

            <div className="skill-meta-grid">
              <div>
                <span>来源路径</span>
                <strong>{activeSkill.source_path}</strong>
              </div>
              <div>
                <span>镜像目标</span>
                <strong>{activeSkill.mirror_targets.length > 0 ? activeSkill.mirror_targets.join("、") : "未镜像"}</strong>
              </div>
              <div>
                <span>补充文件</span>
                <strong>AGENT_KERNEL_MEMORY_CARDS.md</strong>
              </div>
              <div>
                <span>预览状态</span>
                <strong>{(quality?.build_preview.warnings.length ?? 0) > 0 ? "有提示" : "可预览"}</strong>
              </div>
            </div>

            {activeSkill.warnings.length > 0 ? (
              <div className="skill-warning-list">
                {activeSkill.warnings.map((warning) => (
                  <span key={warning}>
                    <AlertTriangle size={14} />
                    {warning}
                  </span>
                ))}
              </div>
            ) : null}

            <SkillCardSection
              title="已嵌入 Memory Cards"
              empty="这个 Skill 还没有补充卡片。"
              cards={activeSkill.linked_memory_cards}
              skill={activeSkill}
              pendingAction={pendingAction}
              disabled={true}
              onAttach={onAction}
            />

            <SkillCardSection
              title="推荐嵌入"
              empty="暂无推荐。procedure、workflow、template 和 supplement 类型会优先出现在这里。"
              cards={attachableCards}
              skill={activeSkill}
              pendingAction={pendingAction}
              disabled={disabled}
              onAttach={onAction}
            />
          </section>
        ) : null}
      </div>
    </div>
  );
}

function SkillCardSection({
  title,
  empty,
  cards,
  skill,
  pendingAction,
  disabled,
  onAttach,
}: {
  title: string;
  empty: string;
  cards: MemoryCardRecord[];
  skill: ProjectSkillView;
  pendingAction: string;
  disabled: boolean;
  onAttach: PanelPageProps["onAction"];
}) {
  return (
    <section className="skill-card-section">
      <div className="section-label">{title}</div>
      {cards.length === 0 ? <p className="muted-line">{empty}</p> : null}
      {cards.map((card) => {
        const actionKey = `嵌入-${skill.id}-${card.id}`;
        const attaching = pendingAction === actionKey;
        const alreadyLinked = skill.linked_memory_cards.some((item) => item.id === card.id);
        return (
          <article className="skill-memory-card" key={card.id}>
            <div>
              <span className="tag">
                {translateKind(card.kind)} · {translateScope(card.scope)}
              </span>
              <h3>{card.title}</h3>
              <p>{card.brief || card.body}</p>
            </div>
            {alreadyLinked ? (
              <span className="linked-pill">
                <CheckCircle2 size={14} />
                已嵌入
              </span>
            ) : (
              <button
                className="secondary-action"
                type="button"
                disabled={disabled || attaching}
                onClick={() =>
                  void onAttach(actionKey, `已将"${card.title}"嵌入"${skill.name}"`, "attach_memory_card_to_skill", {
                    memoryCardId: card.id,
                    skillId: skill.id,
                  })
                }
              >
                {attaching ? <Loader2 className="spin" size={15} /> : <Plus size={15} />}
                嵌入
              </button>
            )}
          </article>
        );
      })}
      {title === "推荐嵌入" ? (
        <p className="skill-hint">
          <Sparkles size={14} />
          嵌入后不会改写源 SKILL.md；同步时会生成补充文件并进入 Artifact 预览。
        </p>
      ) : null}
    </section>
  );
}
