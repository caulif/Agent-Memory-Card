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

  if (skills.length === 0) {
    return (
      <EmptyState
        title="暂无 Skills 注册"
        description="系统会在后端自动检测本地 Skill 目录，自动解析其内置的工具集定义与作用域Scope。"
      />
    );
  }

  return (
    <div className="skills-workbench" style={{ display: "grid", gap: "24px" }}>
      {/* ===== 极简总览统计条 ===== */}
      <section className="skills-summary-strip" aria-label="Skills 状态摘要" style={{ gridTemplateColumns: "1fr 1fr", gap: "16px" }}>
        <div style={{ borderRadius: "16px", padding: "16px", background: "var(--color-surface)", border: "1px solid var(--color-border)" }}>
          <strong style={{ fontSize: "22px", fontFamily: "var(--font-mono)" }}>{skills.length}</strong>
          <span style={{ fontSize: "11px", color: "var(--color-text-dim)" }}>系统全局已载入的核心 Skills</span>
        </div>
        <div style={{ borderRadius: "16px", padding: "16px", background: "var(--color-surface)", border: "1px solid var(--color-border)" }}>
          <strong style={{ fontSize: "22px", fontFamily: "var(--font-mono)" }}>{skills.filter((skill) => skill.mirror_targets.length > 0).length}</strong>
          <span style={{ fontSize: "11px", color: "var(--color-text-dim)" }}>已完成镜像挂载 (Mirrored Decks)</span>
        </div>
      </section>

      {/* ===== 双栏极简技能辞典布局 ===== */}
      <div className="skills-grid" style={{ gridTemplateColumns: "280px minmax(0, 1fr)", gap: "28px" }}>
        {/* 左半边：技能目录选择区 */}
        <section className="skill-list" aria-label="Skill 列表" style={{ gap: "8px" }}>
          {skills.map((skill) => (
            <button
              key={skill.id}
              className={`skill-row ${activeSkill?.id === skill.id ? "active" : ""}`}
              type="button"
              onClick={() => setActiveSkillId(skill.id)}
              style={{
                borderRadius: "12px",
                padding: "12px 14px",
                border: activeSkill?.id === skill.id ? "1.5px solid var(--color-accent)" : "1px solid var(--color-border)",
                background: "var(--color-surface)",
                textAlign: "left"
              }}
            >
              <span style={{ fontWeight: activeSkill?.id === skill.id ? "700" : "600", fontSize: "13px" }}>{skill.name}</span>
              <small style={{ marginTop: "4px", display: "block" }}>{skill.source_kind} · {skill.mirror_targets.length || 0} 个挂载目标</small>
            </button>
          ))}
        </section>

        {/* 右半边：选定技能详情视窗（彻底剥离强词夺理的只读Card绑定与编译日志） */}
        {activeSkill ? (
          <section className="skill-detail" aria-label="Skill 详情" style={{ background: "var(--color-surface)", borderRadius: "20px", padding: "24px", border: "1px solid var(--color-border)", boxShadow: "var(--shadow-md)" }}>
            <div className="skill-detail-head" style={{ borderBottom: "1px dashed var(--color-border)", paddingBottom: "16px", marginBottom: "16px" }}>
              <div style={{ minWidth: 0, flex: 1 }}>
                <span className="tag" style={{ borderRadius: "6px", fontSize: "10.5px" }}>
                  SKILL MODULE · {activeSkill.source_kind}
                </span>
                <h2 style={{ fontSize: "18px", fontWeight: "750", marginTop: "8px" }}>{activeSkill.name}</h2>
                <p style={{ marginTop: "10px", lineHeight: "1.6", color: "var(--color-text-secondary)" }}>
                  {activeSkill.description || "该技能已全局向智能体授权，无特定约束条件限制。"}
                </p>
              </div>
            </div>

            <div className="skill-meta-grid" style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "12px" }}>
              <div style={{ padding: "12px", borderRadius: "10px", background: "var(--color-canvas)", border: "1px solid var(--color-border)" }}>
                <span style={{ fontSize: "10px", color: "var(--color-text-dim)" }}>技能映射路径</span>
                <strong style={{ fontSize: "12px", color: "var(--color-text-muted)", display: "block", marginTop: "4px", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                  {activeSkill.source_path}
                </strong>
              </div>
              <div style={{ padding: "12px", borderRadius: "10px", background: "var(--color-canvas)", border: "1px solid var(--color-border)" }}>
                <span style={{ fontSize: "10px", color: "var(--color-text-dim)" }}>镜像绑定 Agent 队列</span>
                <strong style={{ fontSize: "12px", color: "var(--color-text-primary)", display: "block", marginTop: "4px" }}>
                  {activeSkill.mirror_targets.length > 0 ? activeSkill.mirror_targets.join("、") : "未分配/隐身态"}
                </strong>
              </div>
            </div>

            {activeSkill.warnings.length > 0 ? (
              <div className="skill-warning-list" style={{ marginTop: "16px" }}>
                {activeSkill.warnings.map((warning) => (
                  <span key={warning} style={{ borderRadius: "8px", fontSize: "11px", color: "var(--color-warning-text)", background: "var(--color-warning-bg)", border: "1px solid var(--color-border)" }}>
                    <AlertTriangle size={12} style={{ marginRight: "4px" }} />
                    {warning}
                  </span>
                ))}
              </div>
            ) : (
              <div style={{ display: "flex", alignItems: "center", gap: "6px", marginTop: "24px", color: "var(--color-success)", fontSize: "12px", borderTop: "1px solid var(--color-border)", paddingTop: "16px" }}>
                <CheckCircle2 size={13} />
                <span>该技能契约处于完美状态。由终端底层核心直接托管，无需人为干涉治理。</span>
              </div>
            )}
          </section>
        ) : null}
      </div>
    </div>
  );
}
