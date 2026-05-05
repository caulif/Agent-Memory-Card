import { Metric } from "../common";
import { projectOverviewMetrics, type ProjectDashboard, type ProjectSnapshot } from "../../ui-helpers";

export function ProjectOverviewStrip({
  snapshot,
  dashboard,
  installedCount,
}: {
  snapshot: ProjectSnapshot | null;
  dashboard: ProjectDashboard | null;
  installedCount: number;
}) {
  const metrics = projectOverviewMetrics(snapshot, dashboard, installedCount);
  return (
    <section className="overview-strip" aria-label="项目概览">
      <Metric label="待审草稿" value={metrics.draftCount} />
      <Metric label="系统建议" value={metrics.candidateCount} />
      <Metric label="技能片段" value={metrics.skillletCount} />
      <Metric label="历史观察" value={metrics.observationCount} />
      <Metric label="已装包" value={metrics.installedCount} />
    </section>
  );
}
