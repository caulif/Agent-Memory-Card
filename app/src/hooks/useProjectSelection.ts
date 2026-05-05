import React from "react";
import { getAppState } from "../tauri-client";
import {
  createDemoAppState,
  isTauriRuntimeUnavailable,
  type DesktopAppState,
  type PageId,
} from "../ui-helpers";

export function useProjectSelection({
  page,
  previewMode,
  setPreviewMode,
  selectedProjectRef,
  setMessage,
  onProjectActivated,
  onProjectCleared,
}: {
  page: PageId;
  previewMode: boolean;
  setPreviewMode: (value: boolean) => void;
  selectedProjectRef: React.MutableRefObject<string>;
  setMessage: (message: string) => void;
  onProjectActivated: (projectPath: string, page: PageId) => void;
  onProjectCleared: () => void;
}) {
  const [state, setState] = React.useState<DesktopAppState | null>(null);
  const [selectedProject, setSelectedProject] = React.useState("");
  const [appStateError, setAppStateError] = React.useState("");

  React.useEffect(() => {
    selectedProjectRef.current = selectedProject;
  }, [selectedProject, selectedProjectRef]);

  const enterPreviewMode = React.useCallback((preferredProjectPath = "") => {
    const demoState = createDemoAppState();
    const first = preferredProjectPath || demoState.registry.projects[0]?.path || "";
    setPreviewMode(true);
    setAppStateError("");
    setState(demoState);
    setSelectedProject(first);
    selectedProjectRef.current = first;
    if (first) {
      onProjectActivated(first, page);
    } else {
      onProjectCleared();
    }
  }, [onProjectActivated, onProjectCleared, page, selectedProjectRef, setPreviewMode]);

  const loadAppStateFromBackend = React.useCallback(async () => {
    const next = await getAppState();
    setState(next);
    const current = selectedProjectRef.current;
    const stillExists = current && next.registry.projects.some((project) => project.path === current);
    const projectPath = stillExists ? current : "";
    setSelectedProject(projectPath);
    selectedProjectRef.current = projectPath;
    if (projectPath) {
      onProjectActivated(projectPath, page);
    } else {
      onProjectCleared();
    }
    return next;
  }, [onProjectActivated, onProjectCleared, page, selectedProjectRef]);

  const refreshAppState = React.useCallback(async () => {
    setAppStateError("");
    if (previewMode) {
      enterPreviewMode(selectedProjectRef.current);
      return "预览模式：已刷新演示项目索引。";
    }
    try {
      await loadAppStateFromBackend();
      return "项目索引已更新";
    } catch (error) {
      if (isTauriRuntimeUnavailable(error)) {
        enterPreviewMode();
        return "已进入浏览器预览模式，真实文件操作需要通过 bun run app:dev 启动。";
      }
      const message = error instanceof Error ? error.message : String(error);
      setAppStateError(message);
      throw error;
    }
  }, [enterPreviewMode, loadAppStateFromBackend, previewMode, selectedProjectRef]);

  const reloadAppStateFromBackend = React.useCallback(async () => {
    if (previewMode) return;
    try {
      await loadAppStateFromBackend();
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setMessage(`项目索引刷新失败：${message}`);
    }
  }, [loadAppStateFromBackend, previewMode, setMessage]);

  const chooseProject = React.useCallback((path: string) => {
    if (path === selectedProject) return;
    setSelectedProject(path);
    selectedProjectRef.current = path;
    setMessage("项目已切换，正在加载轻量 Inbox 工作台。");
    onProjectActivated(path, page);
  }, [onProjectActivated, page, selectedProject, selectedProjectRef, setMessage]);

  return {
    state,
    setState,
    selectedProject,
    setSelectedProject,
    appStateError,
    setAppStateError,
    enterPreviewMode,
    refreshAppState,
    reloadAppStateFromBackend,
    chooseProject,
  };
}
