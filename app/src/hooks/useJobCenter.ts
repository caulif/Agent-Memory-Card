import React from "react";
import {
  cancelJobCommand,
  getJobHistory,
  getTaskStatus,
  retryJobCommand,
} from "../tauri-client";
import {
  formatJobStartMessage,
  type DesktopTaskStatus,
  type PageId,
} from "../ui-helpers";

export function useJobCenter({
  previewMode,
  selectedProjectRef,
  page,
  setMessage,
  reloadAppStateFromBackend,
  loadDashboard,
  loadReadModelsForPage,
}: {
  previewMode: boolean;
  selectedProjectRef: React.MutableRefObject<string>;
  page: PageId;
  setMessage: (message: string) => void;
  reloadAppStateFromBackend: () => void | Promise<void>;
  loadDashboard: (projectPath: string) => void | Promise<void>;
  loadReadModelsForPage: (projectPath: string, page: PageId) => void | Promise<void>;
}) {
  const [backendTaskStatus, setBackendTaskStatus] = React.useState<DesktopTaskStatus | undefined>(undefined);
  const [jobHistory, setJobHistory] = React.useState<DesktopTaskStatus[]>([]);
  const [jobCenterOpen, setJobCenterOpen] = React.useState(false);
  const handledJobIdsRef = React.useRef(new Set<string>());

  React.useEffect(() => {
    if (previewMode) {
      setBackendTaskStatus(undefined);
      setJobHistory([]);
      handledJobIdsRef.current.clear();
      return;
    }
    let cancelled = false;
    const refreshTaskStatus = () => {
      Promise.all([getTaskStatus(), getJobHistory()])
        .then(([status, history]) => {
          if (!cancelled) {
            setBackendTaskStatus(status ?? undefined);
            setJobHistory(history);
          }
        })
        .catch(() => {
          if (!cancelled) {
            setBackendTaskStatus(undefined);
            setJobHistory([]);
          }
        });
    };
    refreshTaskStatus();
    const timer = setInterval(refreshTaskStatus, 2000);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [previewMode]);

  React.useEffect(() => {
    if (previewMode) return;
    const completedJobs = jobHistory.filter(
      (job) => job.job_id && ["completed", "failed", "cancelled"].includes(job.lifecycle ?? ""),
    );
    for (const job of completedJobs) {
      const jobId = job.job_id;
      if (!jobId || handledJobIdsRef.current.has(jobId)) continue;
      handledJobIdsRef.current.add(jobId);
      if (job.lifecycle === "completed") {
        setMessage(job.result_summary || job.message || `${job.key}已完成`);
      }
      if (job.lifecycle === "completed" && job.key === "扫描") {
        void reloadAppStateFromBackend();
      }
      if (job.lifecycle === "completed" && (job.key === "整理历史" || job.key === "同步" || job.key === "融合Skilllet")) {
        const projectPath = selectedProjectRef.current;
        if (projectPath) {
          void loadDashboard(projectPath);
          if (job.key === "融合Skilllet") {
            void loadReadModelsForPage(projectPath, "settings");
          } else {
            void loadReadModelsForPage(projectPath, page);
          }
        }
      }
    }
  }, [jobHistory, loadDashboard, loadReadModelsForPage, page, previewMode, reloadAppStateFromBackend, selectedProjectRef, setMessage]);

  const cancelJob = React.useCallback(async (jobId: string) => {
    if (!jobId || previewMode) return;
    try {
      const next = await cancelJobCommand(jobId);
      setBackendTaskStatus(next);
      const history = await getJobHistory();
      setJobHistory(history);
      setMessage(`已请求取消任务 ${jobId}`);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setMessage(`取消任务失败：${message}`);
    }
  }, [previewMode, setMessage]);

  const retryJob = React.useCallback(async (job: DesktopTaskStatus) => {
    if (previewMode) return;
    if (!job.replay?.command) {
      setMessage(`该任务缺少可重放参数，请从原入口重新执行：${job.key || "未知任务"}`);
      return;
    }
    try {
      setMessage(`正在重试任务：${job.key || job.replay.command}`);
      const result = await retryJobCommand(job.replay.command, job.replay.args ?? {});
      setMessage(formatJobStartMessage(result));
      const history = await getJobHistory();
      setJobHistory(history);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setMessage(`重试失败：${message}`);
    }
  }, [previewMode, setMessage]);

  return {
    backendTaskStatus,
    setBackendTaskStatus,
    jobHistory,
    setJobHistory,
    jobCenterOpen,
    setJobCenterOpen,
    cancelJob,
    retryJob,
  };
}
