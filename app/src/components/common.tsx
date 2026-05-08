import React from "react";
import { Check, Loader2 } from "lucide-react";
import { resolveTaskProgress, type DesktopTaskStatus } from "../ui-helpers";

/** 面板容器 */
export function Panel({
  title,
  subtitle,
  icon: Icon,
  children,
}: React.PropsWithChildren<{ title: string; subtitle?: string; icon?: React.ComponentType<{ size?: number }> }>) {
  return (
    <article className="panel">
      <div className="panel-title">
        <div>
          <h2>{title}</h2>
          {subtitle ? <p className="subtitle">{subtitle}</p> : null}
        </div>
        {Icon ? (
          <div className="panel-icon">
            <Icon size={17} />
          </div>
        ) : null}
      </div>
      {children}
    </article>
  );
}

/** 指标数字卡 */
export function Metric({ label, value }: { label: string; value: number }) {
  return (
    <div className="metric">
      <strong>{value}</strong>
      <span>{label}</span>
    </div>
  );
}

/** 状态列表 */
export function StatusList({ items, empty, tone }: { items: string[]; empty: string; tone?: "warning" }) {
  if (items.length === 0) {
    return <p className="empty">{empty}</p>;
  }

  return (
    <div className="status-list" role="list">
      {items.slice(0, 8).map((item) => (
        <p className={tone === "warning" ? "line warning" : "line"} key={item} role="listitem">
          {item}
        </p>
      ))}
    </div>
  );
}

/** 空状态占位 */
export function EmptyState({ title, description }: { title: string; description: string }) {
  return (
    <div className="empty-state" role="status">
      <h3>{title}</h3>
      <p>{description}</p>
    </div>
  );
}

type ActionButtonVariant = "primary" | "secondary" | "hero" | "danger" | "ghost";

const VARIANT_CLASS: Record<ActionButtonVariant, string> = {
  primary: "primary-action",
  secondary: "secondary-action",
  hero: "hero-button",
  danger: "danger-action",
  ghost: "ghost-action",
};

/** 操作按钮 */
export function ActionButton({
  variant,
  className,
  icon: Icon,
  label,
  busyLabel,
  busy,
  disabled,
  onClick,
  ariaLabel,
}: {
  variant?: ActionButtonVariant;
  className?: string;
  icon: React.ComponentType<{ size?: number; className?: string }>;
  label: string;
  busyLabel: string;
  busy: boolean;
  disabled: boolean;
  onClick: () => void | Promise<void>;
  ariaLabel?: string;
}) {
  const resolvedClass = className ?? VARIANT_CLASS[variant ?? "secondary"];
  return (
    <button
      className={resolvedClass}
      disabled={disabled || busy}
      onClick={() => void onClick()}
      aria-label={ariaLabel ?? label}
      aria-busy={busy}
    >
      {busy ? <Loader2 className="spin" size={14} /> : <Icon size={14} />}
      {busy ? busyLabel : label}
    </button>
  );
}

/** 任务进度条 */
export function TaskProgressBar({ pendingAction, message, backendStatus }: { pendingAction: string; message: string; backendStatus?: DesktopTaskStatus }) {
  const progress = resolveTaskProgress(pendingAction, backendStatus);
  const isBusy = pendingAction !== "" || Boolean(backendStatus?.running);
  const activeLabel = backendStatus?.label || pendingAction || backendStatus?.key || "后台任务";
  const visiblePercent = Math.max(0, Math.min(100, progress.percent));
  const isComplete = visiblePercent >= 100;
  const details = backendStatus?.details ?? [];
  const logs = backendStatus?.logs ?? [];
  const jobId = backendStatus?.job_id ?? "";
  const resultSummary = backendStatus?.result_summary ?? "";
  const startedAt = backendStatus?.started_at ?? "";

  return (
    <div className="progress-area">
      <div className="status-pill" title={message} role="status" aria-live="polite">
        {isBusy ? <Loader2 className="spin" size={13} /> : <Check size={13} />}
        <span>{isBusy ? `${activeLabel}中...` : message}</span>
      </div>
      {isBusy ? (
        <div className="progress-section" role="progressbar" aria-valuenow={isComplete ? 100 : visiblePercent} aria-valuemin={0} aria-valuemax={100} aria-label={progress.label}>
          <div className="progress-header">
            <span className="progress-label">{progress.label}</span>
            <span className="progress-percent">{isComplete ? "100%" : `${visiblePercent}%`}</span>
          </div>
          <div className="progress-track">
            <div
              className="progress-fill"
              style={{ width: `${isComplete ? 100 : visiblePercent}%` }}
            />
          </div>
          <p className="progress-description">{progress.description}</p>
          {jobId || startedAt || resultSummary ? (
            <div className="job-meta" aria-label="任务元数据">
              {jobId ? <span>任务 {jobId}</span> : null}
              {startedAt ? <span>开始 {startedAt}</span> : null}
              {resultSummary ? <span>结果 {resultSummary}</span> : null}
            </div>
          ) : null}
          {details.length > 0 ? (
            <div className="task-details" aria-label="后台活动明细">
              <div className="task-details-title">后台活动</div>
              {details.slice(-5).map((detail, index) => (
                <div className="task-detail-row" key={`${detail.label}-${detail.percent}-${index}`}>
                  <span className="task-detail-dot" />
                  <div>
                    <strong>
                      {detail.label}
                      <span>{detail.percent}%</span>
                    </strong>
                    <p>{detail.description}</p>
                  </div>
                </div>
              ))}
            </div>
          ) : null}
          {logs.length > details.length ? (
            <div className="task-details compact" aria-label="任务日志">
              <div className="task-details-title">任务日志</div>
              {logs.slice(-4).map((entry, index) => (
                <div className="task-detail-row" key={`${entry.timestamp}-${entry.stage}-${index}`}>
                  <span className="task-detail-dot" />
                  <div>
                    <strong>
                      {entry.stage}
                      <span>{entry.percent}%</span>
                    </strong>
                    <p>{entry.description}</p>
                  </div>
                </div>
              ))}
            </div>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}
