import React from "react";
import { Check, Circle, GitBranch, Loader2 } from "lucide-react";
import { EmptyState, Panel } from "../common";
import {
  deriveSkillletEvolution,
  describeSkillletPlainly,
  formatAgent,
  nextSkillletTargets,
  type EvolutionInsight,
  type ProjectAction,
  type ProjectAssignmentView,
  type ProjectSkillletLibrary,
  type ProjectSnapshot,
  type PanelPageProps,
} from "../../ui-helpers";

export function Agents({
  snapshot,
  assignment,
  library,
  pendingAction,
  disabled,
  onAction,
}: PanelPageProps & { assignment: ProjectAssignmentView | null; library: ProjectSkillletLibrary | null }) {
  const agents = assignment?.target_matrix.agents ?? snapshot?.target_matrix.agents ?? [];
  const rows = assignment?.target_matrix.rows ?? snapshot?.target_matrix.rows ?? [];
  const skilllets = library?.skilllets ?? snapshot?.skilllets ?? [];
  const togglingKey = pendingAction.startsWith("切换-") ? pendingAction : "";
  const evolutionSnapshot = React.useMemo<ProjectSnapshot | null>(() => {
    if (snapshot) return snapshot;
    if (!assignment && !library) return null;
    return {
      project_path: assignment?.project_path ?? library?.project_path ?? "",
      drafts: [],
      skilllets,
      global_skilllets: library?.global_skilllets ?? [],
      observations: [],
      catalog_status: library?.catalog_status ?? { items: [] },
      target_matrix: assignment?.target_matrix ?? { agents, rows },
      rule_ci: { passed: 0, failed: 0 },
      build_preview: { actions: [], warnings: [] },
      status: { warnings: [] },
    };
  }, [agents, assignment, library, rows, skilllets, snapshot]);

  /** 解析正在切换中的 skilllet_id 和 agent */
  const togglingParts = togglingKey ? togglingKey.replace("切换-", "").split("|") : [];
  const togglingSkillletId = togglingParts[0] ?? "";
  const togglingAgent = togglingParts[1] ?? "";

  return (
    <div className="stack">
      <Panel title="当前项目 Skilllet 分配矩阵" subtitle="这里只分配当前项目已安装/已批准的 Skilllet 到目标智能体；全局库需先加入当前项目后才会出现在这里。" icon={GitBranch}>
        {rows.length === 0 ? (
          <EmptyState title="暂无分配数据" description="安装包或批准技能片段后，这里会显示目标智能体矩阵。" />
        ) : (
          <div className="matrix" style={{ gridTemplateColumns: `minmax(180px, 1fr) repeat(${Math.max(agents.length, 1)}, 100px)` }}>
            <div className="matrix-head">技能片段</div>
            {agents.map((agent) => (
              <div className="matrix-head" key={agent}>
                {formatAgent(agent)}
              </div>
            ))}
            {rows.flatMap((row) => {
              const currentTargets = agents.filter((a) => row.targets[a]);
              return [
                <div key={`${row.skilllet_id}-title`} className="matrix-title">
                  {row.title}
                  <span className="matrix-scope">{row.scope === "global" ? "全局来源" : "项目"}</span>
                </div>,
                ...agents.map((agent) => {
                  const isOn = row.targets[agent];
                  const isToggling = togglingSkillletId === row.skilllet_id && togglingAgent === agent;
                  const nextTargets = nextSkillletTargets(currentTargets, agent, !isOn);
                  return (
                    <button
                      key={`${row.skilllet_id}-${agent}`}
                      className={`matrix-cell interactive ${isOn ? "on" : "off"} ${isToggling ? "toggling" : ""}`}
                      disabled={disabled}
                      title={isOn ? `点击取消分配给 ${formatAgent(agent)}` : `点击分配给 ${formatAgent(agent)}`}
                      onClick={() =>
                        onAction(
                          `切换-${row.skilllet_id}|${agent}`,
                          `已更新"${row.title}"的目标智能体`,
                          "set_skilllet_targets",
                          { id: row.skilllet_id, targets: nextTargets },
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

      {/* 工程化进化视图 */}
      {skilllets.length > 0 && evolutionSnapshot ? (
        <Panel title="工程化技能进化视图" subtitle="只展示当前项目已安装或已批准的 Skilllet，帮助判断哪些真正会参与分配和编译。" icon={GitBranch}>
          <div className="evolution-grid">
            {skilllets.map((skilllet) => {
              const insight = deriveSkillletEvolution(skilllet, evolutionSnapshot);
              const targetRow = rows.find((row) => row.skilllet_id === skilllet.id);
              return <EvolutionCard key={skilllet.id} insight={insight} skilllet={skilllet} targetRow={targetRow} />;
            })}
          </div>
        </Panel>
      ) : (
        <Panel title="工程化技能进化视图" subtitle="当前项目还没有可分析的 Skilllet。" icon={GitBranch}>
          <EmptyState title="暂无进化数据" description="批准或加入当前项目 Skilllet 后，这里会基于分配目标、草稿和记录元数据生成进化视图。" />
        </Panel>
      )}
    </div>
  );
}

/** 技能进化卡片：进化树 + 置信度/稳定度 + 时间线 + 冲突预警 + 活跃状态 */
function EvolutionCard({
  insight,
  skilllet,
  targetRow,
}: {
  insight: EvolutionInsight;
  skilllet: import("../../ui-helpers").SkillletRecord;
  targetRow?: ProjectAssignmentView["target_matrix"]["rows"][number];
}) {
  const assignedAgents = targetRow
    ? Object.entries(targetRow.targets)
        .filter(([, enabled]) => enabled)
        .map(([agent]) => formatAgent(agent))
    : [];
  return (
    <article className="evolution-card">
      <div className="evolution-head">
        <h3>{insight.title}</h3>
        <div className="evolution-badges">
          <span className={`activity-badge ${insight.activity}`}>
            {insight.activity === "active" ? "活跃" : "未分配"}
          </span>
          {insight.promotion_candidate ? <span className="promotion-badge">↑ 提升候选</span> : null}
        </div>
      </div>

      <div className="evolution-summary">
        <span>{skilllet.scope === "global" ? "全局 Skilllet" : "项目 Skilllet"}</span>
        <span>{assignedAgents.length > 0 ? `已分配给 ${assignedAgents.join(" / ")}` : "尚未分配给智能体"}</span>
      </div>

      {insight.evolution_tree.length > 0 ? (
        <div className="evolution-source">
          <span className="evolution-label">来源</span>
          <p>{insight.evolution_tree.map((edge) => edge.label).join("；")}</p>
        </div>
      ) : null}

      {/* 置信度 & 稳定度 */}
      <div className="metrics-row">
        <div className="confidence-bar-wrap">
          <div className="bar-header">
            <span>置信度</span>
            <strong>{Math.round(insight.confidence * 100)}%</strong>
          </div>
          <div className="bar-track">
            <div className="bar-fill confidence" style={{ width: `${insight.confidence * 100}%` }} />
          </div>
        </div>
        <div className="confidence-bar-wrap">
          <div className="bar-header">
            <span>稳定度</span>
            <strong>{Math.round(insight.stability * 100)}%</strong>
          </div>
          <div className="bar-track">
            <div className="bar-fill stability" style={{ width: `${insight.stability * 100}%` }} />
          </div>
        </div>
      </div>

      {/* 时间线 */}
      <div className="evolution-timeline">
        <span className="evolution-label">时间线</span>
        {insight.timeline.map((entry, idx) => (
          <div className="timeline-entry" key={idx}>
            <span className="timeline-dot" />
            <time className="timeline-date">{entry.date}</time>
            <span className="timeline-event">{entry.event}</span>
          </div>
        ))}
      </div>

      {/* 冲突预警 */}
      {insight.conflicts.length > 0 ? (
        <div className="conflict-section">
          <span className="evolution-label conflict-label">冲突预警</span>
          {insight.conflicts.map((c, idx) => (
            <p className="conflict-warning" key={idx}>
              {c}
            </p>
          ))}
        </div>
      ) : null}

      {/* 一句话描述 */}
      <p className="evolution-description">{describeSkillletPlainly(skilllet)}</p>
    </article>
  );
}
