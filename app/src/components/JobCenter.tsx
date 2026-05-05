import React from "react";
import { RefreshCw, X } from "lucide-react";
import { formatJobLifecycle, type DesktopTaskStatus } from "../ui-helpers";
import { EmptyState } from "./common";
export function JobCenter({
  jobs,
  currentJob,
  onCancel,
  onRetry,
  onClose,
}: {
  jobs: DesktopTaskStatus[];
  currentJob?: DesktopTaskStatus;
  onCancel: (jobId: string) => void | Promise<void>;
  onRetry: (job: DesktopTaskStatus) => void | Promise<void>;
  onClose: () => void;
}) {
  const [statusFilter, setStatusFilter] = React.useState("all");
  const [typeFilter, setTypeFilter] = React.useState("all");

  const allJobs = jobs.length > 0 ? jobs : currentJob?.job_id ? [currentJob] : [];

  const filteredJobs = React.useMemo(() => {
    return allJobs.filter((job) => {
      if (statusFilter !== "all" && job.lifecycle !== statusFilter) return false;
      if (typeFilter !== "all" && job.key !== typeFilter) return false;
      return true;
    });
  }, [allJobs, statusFilter, typeFilter]);

  const statusOptions = [
    { value: "all", label: "全部" },
    { value: "running", label: "运行中" },
    { value: "failed", label: "失败" },
    { value: "completed", label: "已完成" },
    { value: "cancelled", label: "已取消" },
  ];

  const typeOptions = [
    { value: "all", label: "全部" },
    { value: "扫描", label: "扫描" },
    { value: "整理历史", label: "整理历史" },
    { value: "同步", label: "同步" },
    { value: "融合Skilllet", label: "融合Skilllet" },
  ];

  return (
    <div className="job-center-backdrop" role="presentation" onClick={onClose}>
      <aside className="job-center" aria-label="任务中心" onClick={(event) => event.stopPropagation()}>
        <div className="job-center-head">
          <div>
            <p className="eyebrow">后台任务</p>
            <h2>任务中心</h2>
          </div>
          <button className="icon-button" aria-label="关闭任务中心" onClick={onClose}>
            <X size={16} />
          </button>
        </div>

        {/* 状态筛选 */}
        <nav className="job-filter" aria-label="按状态筛选">
          {statusOptions.map((opt) => (
            <button
              key={opt.value}
              className={statusFilter === opt.value ? "active" : ""}
              onClick={() => setStatusFilter(opt.value)}
            >
              {opt.label}
            </button>
          ))}
        </nav>

        {/* 类型筛选 */}
        <nav className="job-filter" aria-label="按类型筛选">
          {typeOptions.map((opt) => (
            <button
              key={opt.value}
              className={typeFilter === opt.value ? "active" : ""}
              onClick={() => setTypeFilter(opt.value)}
            >
              {opt.label}
            </button>
          ))}
        </nav>

        {filteredJobs.length === 0 ? (
          <EmptyState
            title={allJobs.length === 0 ? "暂无后台任务" : "暂无匹配任务"}
            description={
              allJobs.length === 0
                ? "扫描、整理历史或同步产物后，任务进度和日志会出现在这里。"
                : "当前筛选条件下没有匹配的任务记录，请调整筛选条件。"
            }
          />
        ) : (
          <div className="job-list">
            {filteredJobs.map((job) => {
              const logs = job.logs ?? [];
              const canCancel = job.running && !job.cancel_requested && job.job_id;
              const canRetry = !job.running && ["failed", "cancelled"].includes(job.lifecycle ?? "");
              return (
                <article className={`job-card ${job.lifecycle ?? "idle"}`} key={job.job_id || job.key}>
                  <div className="job-card-head">
                    <div>
                      <strong>{job.key || "后台任务"}</strong>
                      <span>{job.stage || job.label}</span>
                    </div>
                    <div className="job-card-badges">
                      {job.replay?.command ? (
                        <span className="retry-badge" title="该任务记录了完整执行参数，支持精确重试">
                          <RefreshCw size={10} />
                          可重试
                        </span>
                      ) : null}
                      <span className="job-lifecycle">{formatJobLifecycle(job.lifecycle)}</span>
                    </div>
                  </div>
                  <p>{job.result_summary || job.description || job.message}</p>
                  <div className="progress-track">
                    <div className="progress-fill" style={{ width: `${Math.max(0, Math.min(100, job.percent))}%` }} />
                  </div>
                  <div className="job-card-meta">
                    {job.job_id ? <span>{job.job_id}</span> : null}
                    {job.started_at ? <span>开始 {job.started_at}</span> : null}
                    {job.finished_at ? <span>结束 {job.finished_at}</span> : null}
                  </div>
                  {logs.length > 0 ? (
                    <div className="job-log-list">
                      {logs.slice(-5).map((entry, index) => (
                        <div className="job-log-row" key={`${job.job_id}-${entry.timestamp}-${index}`}>
                          <span>{entry.percent}%</span>
                          <div>
                            <strong>{entry.stage}</strong>
                            <p>{entry.description}</p>
                          </div>
                        </div>
                      ))}
                    </div>
                  ) : null}
                  {canCancel || canRetry ? (
                    <div className="job-actions">
                      {canCancel ? (
                        <button className="secondary-action cancel-job" onClick={() => void onCancel(job.job_id!)}>
                          <X size={14} />
                          请求取消
                        </button>
                      ) : null}
                      {canRetry ? (
                        <button className="secondary-action" onClick={() => void onRetry(job)}>
                          <RefreshCw size={14} />
                          重试
                        </button>
                      ) : null}
                    </div>
                  ) : null}
                </article>
              );
            })}
          </div>
        )}
      </aside>
    </div>
  );
}

