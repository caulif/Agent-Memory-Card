import { Metric } from "../common";
import {
  buildWorkflowContractState,
  projectOverviewMetrics,
  type PageId,
  type ProjectDashboard,
  type ProjectSnapshot,
} from "../../ui-helpers";

export function ProjectOverviewStrip({
  snapshot,
  dashboard,
  installedCount,
  runningJobCount,
  onNavigate,
}: {
  snapshot: ProjectSnapshot | null;
  dashboard: ProjectDashboard | null;
  installedCount: number;
  runningJobCount: number;
  onNavigate: (page: PageId) => void;
}) {
  const metrics = projectOverviewMetrics(snapshot, dashboard, installedCount);
  const workflow = buildWorkflowContractState({
    hasProject: Boolean(snapshot ?? dashboard),
    dashboard,
    candidates: snapshot?.candidates,
    drafts: snapshot?.drafts,
    memoryCards: snapshot?.memory_cards,
    assignment: snapshot
      ? {
          project_path: snapshot.project_path,
          enabled_agents: snapshot.target_matrix.agents,
          target_matrix: snapshot.target_matrix,
        }
      : null,
    quality: snapshot
      ? {
          project_path: snapshot.project_path,
          rule_ci: snapshot.rule_ci,
          build_preview: snapshot.build_preview,
          status: snapshot.status,
        }
      : null,
    runningJobCount,
  });
  return (
    <section className="overview-strip" aria-label="项目概览">
      <button
        type="button"
        className="overview-next-action"
        onClick={() => workflow.primaryAction.targetPage ? onNavigate(workflow.primaryAction.targetPage) : undefined}
      >
        <span>下一步</span>
        <strong>{workflow.primaryAction.label}</strong>
        <small>{workflow.primaryAction.detail}</small>
      </button>
      {workflow.warnings.length > 0 ? (
        <div className="overview-warning-list" aria-label="工作流提醒">
          {workflow.warnings.slice(0, 3).map((warning) => (
            <button
              key={warning.id}
              type="button"
              className={`overview-warning ${warning.tone}`}
              onClick={() => onNavigate(warning.targetPage)}
            >
              <strong>{warning.label}</strong>
              <span>{warning.detail}</span>
            </button>
          ))}
        </div>
      ) : null}
      <Metric label="待审建议" value={metrics.draftCount + metrics.candidateCount} />
      <Metric label="Memory Cards" value={metrics.memory_cardCount} />
      <Metric label="历史观察" value={metrics.observationCount} />
      <Metric label="已装包" value={metrics.installedCount} />
    </section>
  );
}
