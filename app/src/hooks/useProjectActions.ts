import React from "react";
import { readModelRefreshPagesForMutation } from "../project-read-models";
import { planKernelCommand, runProjectMutation } from "../tauri-client";
import {
  buildKernelPlanForInvoke,
  confirmedAgentManagedPolicy,
  formatJobStartMessage,
  getPreviewActionMessage,
  isDesktopJobStart,
  normalizePlanReviewResult,
  removeCandidateFromInbox,
  type DesktopJobStart,
  type PageId,
  type ProjectCandidateInbox,
  type ProjectDashboard,
  type ProjectReviewInbox,
} from "../ui-helpers";

export function useProjectActions({
  selectedProject,
  selectedProjectRef,
  page,
  previewMode,
  setMessage,
  setPendingAction,
  setCandidateInbox,
  setReviewInbox,
  setDashboard,
  loadDashboard,
  loadReadModelsForPage,
}: {
  selectedProject: string;
  selectedProjectRef: React.MutableRefObject<string>;
  page: PageId;
  previewMode: boolean;
  setMessage: (message: string) => void;
  setPendingAction: (action: string) => void;
  setCandidateInbox: React.Dispatch<React.SetStateAction<ProjectCandidateInbox | null>>;
  setReviewInbox: React.Dispatch<React.SetStateAction<ProjectReviewInbox | null>>;
  setDashboard: React.Dispatch<React.SetStateAction<ProjectDashboard | null>>;
  loadDashboard: (projectPath: string) => Promise<void>;
  loadReadModelsForPage: (projectPath: string, page: PageId) => Promise<void>;
}) {
  const refreshAfterMutation = React.useCallback(async (projectPath: string, command: string) => {
    if (projectPath !== selectedProjectRef.current) return;
    const pages = readModelRefreshPagesForMutation(command);
    const orderedPages = pages.includes(page)
      ? [page, ...pages.filter((item) => item !== page)]
      : pages.length > 0
        ? pages
        : [page];
    for (const targetPage of orderedPages) {
      await loadReadModelsForPage(projectPath, targetPage);
    }
    await loadDashboard(projectPath);
  }, [loadDashboard, loadReadModelsForPage, page, selectedProjectRef]);

  const runTask = React.useCallback(async (
    actionKey: string,
    doneMessage: string,
    task: () => Promise<string | void>,
    onError?: (message: string) => void,
  ) => {
    setPendingAction(actionKey);
    try {
      const taskMessage = await task();
      setMessage(taskMessage ?? doneMessage);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setMessage(message);
      onError?.(message);
    } finally {
      setPendingAction("");
    }
  }, [setMessage, setPendingAction]);

  const projectAction = React.useCallback(async (
    actionKey: string,
    doneMessage: string,
    command: string,
    args: Record<string, unknown> = {},
  ) => {
    if (!selectedProject) return;
    const actionProjectPath = selectedProject;
    const draftDecisionId =
      (command === "approve_draft" || command === "reject_draft") && typeof args.id === "string"
        ? (args.id as string)
        : "";
    const candidateDecisionId =
      (command === "promote_candidate" || command === "hide_candidate" || command === "reject_candidate") &&
      typeof args.id === "string"
        ? (args.id as string)
        : "";

    const applyCandidateOptimism = () => {
      if (!candidateDecisionId) return;
      setCandidateInbox((current) => removeCandidateFromInbox(current, candidateDecisionId));
      setDashboard((current) => {
        if (!current) return current;
        return {
          ...current,
          candidate_count: Math.max(current.candidate_count - 1, 0),
          memory_card_count: command === "promote_candidate" ? current.memory_card_count + 1 : current.memory_card_count,
        };
      });
    };

    const applyDraftOptimism = () => {
      if (!draftDecisionId) return;
      setReviewInbox((current) =>
        current ? { ...current, drafts: current.drafts.filter((draft) => draft.id !== draftDecisionId) } : current,
      );
      setDashboard((current) =>
        current ? { ...current, draft_count: Math.max(current.draft_count - 1, 0) } : current,
      );
    };

    if (previewMode) {
      applyCandidateOptimism();
      applyDraftOptimism();
      setMessage(getPreviewActionMessage(actionKey));
      return;
    }

    applyCandidateOptimism();
    applyDraftOptimism();

    await runTask(actionKey, doneMessage, async () => {
      const policy = confirmedAgentManagedPolicy();
      const plan = buildKernelPlanForInvoke(command, args);
      const decisionToken = plan
        ? normalizePlanReviewResult(await planKernelCommand({
            command: plan.command,
            policy,
            projectPath: actionProjectPath,
            payload: plan.payload,
          })).decisionToken
        : undefined;
      const result = await runProjectMutation<DesktopJobStart | unknown>(command, {
        projectPath: actionProjectPath,
        confirmedPolicy: policy,
        decisionToken,
        ...args,
      });
      if (isDesktopJobStart(result)) {
        return formatJobStartMessage(result);
      }
      if (command === "evolve_project") {
        return "已开始后台整理。你可以继续删除、编辑或切换页面。";
      }
      // 所有会改变 memory_card/draft/assignment 数据的命令成功执行后刷新读模型
      const mutationCommands = [
        "promote_candidate", "hide_candidate", "reject_candidate", "gc_candidates",
        "update_candidate",
        "approve_draft", "reject_draft",
        "set_memory_card_targets", "clear_memory_card_targets", "update_memory_card", "delete_memory_card",
        "set_agent_enabled", "merge_drafts", "merge_memory_cards",
        "fuse_memory_cards_to_draft",
        "attach_memory_card_to_skill",
        "promote_memory_card_to_global", "install_global_memory_card_to_project",
        "import_project",
        "sync_project",
        "import_artifact_drifts",
        "import_artifact_drift_path",
        "keep_artifact_drifts",
        "keep_artifact_drift_path",
        "discard_artifact_drifts",
        "discard_artifact_drift_path",
        "clear_project_history",
      ];
      if (mutationCommands.includes(command)) {
        void refreshAfterMutation(actionProjectPath, command);
      }
    }, () => {
      void refreshAfterMutation(actionProjectPath, command);
    });
  }, [
    previewMode,
    refreshAfterMutation,
    runTask,
    selectedProject,
    setCandidateInbox,
    setDashboard,
    setMessage,
    setReviewInbox,
  ]);

  const batchCandidateAction = React.useCallback(async (
    actionKey: string,
    doneMessage: string,
    command: "promote_candidate" | "hide_candidate" | "reject_candidate",
    ids: string[],
    extraArgs: Record<string, unknown> = {},
  ) => {
    if (!selectedProject || ids.length === 0) return;
    const projectPath = selectedProject;
    await runTask(actionKey, doneMessage, async () => {
      const policy = confirmedAgentManagedPolicy();
      for (const id of ids) {
        const mutationArgs = { id, ...extraArgs };
        const plan = buildKernelPlanForInvoke(command, mutationArgs);
        const decisionToken = plan
          ? normalizePlanReviewResult(await planKernelCommand({
              command: plan.command,
              policy,
              projectPath,
              payload: plan.payload,
            })).decisionToken
          : undefined;
        await runProjectMutation(command, {
          projectPath,
          id,
          confirmedPolicy: policy,
          decisionToken,
          ...extraArgs,
        });
      }
      await refreshAfterMutation(projectPath, command);
    }, () => {
      void refreshAfterMutation(projectPath, command);
    });
  }, [refreshAfterMutation, runTask, selectedProject]);

  return {
    runTask,
    projectAction,
    batchCandidateAction,
  };
}
