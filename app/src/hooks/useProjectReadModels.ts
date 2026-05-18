import React from "react";
import {
  loadProjectReadModelsFromTauri,
  type ProjectReadModels,
} from "../project-read-models";
import { getProjectDashboard } from "../tauri-client";
import {
  createDemoProjectAssignmentView,
  createDemoProjectCandidateInbox,
  createDemoProjectDashboard,
  createDemoProjectEvalRunView,
  createDemoProjectQualityView,
  createDemoProjectReviewInbox,
  createDemoProjectMemoryCardLibrary,
  type PageId,
  type ProjectAssignmentView,
  type ProjectCandidateInbox,
  type ProjectDashboard,
  type ProjectEvalRunView,
  type ProjectQualityView,
  type ProjectReviewInbox,
  type ProjectMemoryCardLibrary,
} from "../ui-helpers";

export function useProjectReadModels({
  page,
  previewMode,
  selectedProjectRef,
  setMessage,
}: {
  page: PageId;
  previewMode: boolean;
  selectedProjectRef: React.MutableRefObject<string>;
  setMessage: (message: string) => void;
}) {
  const [dashboard, setDashboard] = React.useState<ProjectDashboard | null>(null);
  const [candidateInbox, setCandidateInbox] = React.useState<ProjectCandidateInbox | null>(null);
  const [reviewInbox, setReviewInbox] = React.useState<ProjectReviewInbox | null>(null);
  const [memory_cardLibrary, setMemoryCardLibrary] = React.useState<ProjectMemoryCardLibrary | null>(null);
  const [assignmentView, setAssignmentView] = React.useState<ProjectAssignmentView | null>(null);
  const [qualityView, setQualityView] = React.useState<ProjectQualityView | null>(null);
  const [evalRunView, setEvalRunView] = React.useState<ProjectEvalRunView | null>(null);
  const dashboardRequestRef = React.useRef(0);
  const readModelRequestRef = React.useRef(0);

  const clearProjectReadModels = React.useCallback(() => {
    setCandidateInbox(null);
    setReviewInbox(null);
    setMemoryCardLibrary(null);
    setAssignmentView(null);
    setQualityView(null);
    setEvalRunView(null);
  }, []);

  const applyReadModels = React.useCallback((models: ProjectReadModels) => {
    if (models.candidates) setCandidateInbox(models.candidates);
    if (models.inbox) setReviewInbox(models.inbox);
    if (models.library) setMemoryCardLibrary(models.library);
    if (models.assignment) setAssignmentView(models.assignment);
    if (models.quality) setQualityView(models.quality);
    if (models.evalRun) setEvalRunView(models.evalRun);
  }, []);

  const hydrateReadModelsFromDemo = React.useCallback((projectPath: string, targetPage: PageId = page) => {
    if (targetPage === "drafts" || targetPage === "settings") {
      setCandidateInbox(createDemoProjectCandidateInbox(projectPath));
      setReviewInbox(createDemoProjectReviewInbox(projectPath));
      setMemoryCardLibrary(createDemoProjectMemoryCardLibrary(projectPath));
      setAssignmentView(createDemoProjectAssignmentView(projectPath));
      setQualityView(createDemoProjectQualityView(projectPath));
      setEvalRunView(createDemoProjectEvalRunView(projectPath));
    }
    if (targetPage === "memory-cards" || targetPage === "agents" || targetPage === "settings") {
      setMemoryCardLibrary(createDemoProjectMemoryCardLibrary(projectPath));
    }
    if (targetPage === "agents" || targetPage === "settings") {
      setAssignmentView(createDemoProjectAssignmentView(projectPath));
    }
    if (targetPage === "settings") {
      setQualityView(createDemoProjectQualityView(projectPath));
      setEvalRunView(createDemoProjectEvalRunView(projectPath));
    }
  }, [page]);

  const loadDashboard = React.useCallback(async (projectPath: string) => {
    const requestId = ++dashboardRequestRef.current;
    if (previewMode) {
      if (requestId === dashboardRequestRef.current && projectPath === selectedProjectRef.current) {
        setDashboard(createDemoProjectDashboard(projectPath));
      }
      return;
    }

    try {
      const next = await getProjectDashboard(projectPath);
      if (requestId === dashboardRequestRef.current && projectPath === selectedProjectRef.current) {
        setDashboard(next);
      }
    } catch (error) {
      if (requestId === dashboardRequestRef.current && projectPath === selectedProjectRef.current) {
        const message = error instanceof Error ? error.message : String(error);
        setMessage(`概览加载失败：${message}`);
      }
    }
  }, [previewMode, selectedProjectRef, setMessage]);

  const loadReadModelsForPage = React.useCallback(async (projectPath: string, targetPage: PageId) => {
    const requestId = ++readModelRequestRef.current;
    if (previewMode) {
      if (requestId === readModelRequestRef.current && projectPath === selectedProjectRef.current) {
        hydrateReadModelsFromDemo(projectPath, targetPage);
      }
      return;
    }

    try {
      const models = await loadProjectReadModelsFromTauri(projectPath, targetPage);
      if (requestId === readModelRequestRef.current && projectPath === selectedProjectRef.current) {
        applyReadModels(models);
      }
    } catch (error) {
      if (requestId === readModelRequestRef.current && projectPath === selectedProjectRef.current) {
        const message = error instanceof Error ? error.message : String(error);
        setMessage(`页面数据加载失败：${message}`);
      }
    }
  }, [applyReadModels, hydrateReadModelsFromDemo, previewMode, selectedProjectRef, setMessage]);

  return {
    dashboard,
    setDashboard,
    candidateInbox,
    setCandidateInbox,
    reviewInbox,
    setReviewInbox,
    memory_cardLibrary,
    setMemoryCardLibrary,
    assignmentView,
    setAssignmentView,
    qualityView,
    setQualityView,
    evalRunView,
    setEvalRunView,
    clearProjectReadModels,
    hydrateReadModelsFromDemo,
    loadDashboard,
    loadReadModelsForPage,
  };
}
