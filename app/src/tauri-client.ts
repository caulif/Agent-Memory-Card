import { invoke } from "@tauri-apps/api/core";
import type {
  CandidateRecord,
  DraftRecord,
  DesktopAppState,
  DesktopJobStart,
  DesktopTaskStatus,
  KernelPlanRequest,
  KernelPolicyPayload,
  ProjectAssignmentView,
  ProjectCandidateInbox,
  ProjectDashboard,
  ProjectQualityView,
  ProjectReviewInbox,
  ProjectSkillletLibrary,
  ProjectSnapshot,
  SkillletRecord,
} from "./ui-helpers";

export type ProjectMutationAck = {
  project_path: string;
};

export function getAppState() {
  return invoke<DesktopAppState>("get_app_state", {});
}

export function getProjectDashboard(projectPath: string) {
  return invoke<ProjectDashboard>("get_project_dashboard", { projectPath });
}

export function getProjectCandidateInbox(projectPath: string) {
  return invoke<ProjectCandidateInbox>("get_project_candidate_inbox", { projectPath });
}

export function getProjectReviewInbox(projectPath: string) {
  return invoke<ProjectReviewInbox>("get_project_review_inbox", { projectPath });
}

export function getProjectSkillletLibrary(projectPath: string) {
  return invoke<ProjectSkillletLibrary>("get_project_skilllet_library", { projectPath });
}

export function getProjectAssignmentView(projectPath: string) {
  return invoke<ProjectAssignmentView>("get_project_assignment_view", { projectPath });
}

export function getProjectQualityView(projectPath: string) {
  return invoke<ProjectQualityView>("get_project_quality_view", { projectPath });
}

export function getProjectSnapshot(projectPath: string) {
  return invoke<ProjectSnapshot>("get_project_snapshot", { projectPath });
}

export function planKernelCommand(input: { command: KernelPlanRequest["command"]; payload: KernelPlanRequest["payload"]; policy: KernelPolicyPayload; projectPath: string }) {
  const { projectPath, ...rest } = input;
  return invoke<unknown>("plan_kernel_command", { input: { ...rest, project_path: projectPath } });
}

export function runProjectMutation<T = ProjectMutationAck>(
  command: string,
  args: Record<string, unknown>,
) {
  return invoke<T>(command, args);
}

export function cancelJobCommand(jobId: string) {
  return invoke<DesktopTaskStatus>("cancel_job", { jobId });
}

export function retryJobCommand(command: string, args: Record<string, unknown>) {
  return invoke<DesktopJobStart>(command, args);
}

export function updateDraft(args: {
  projectPath: string;
  id: string;
  input: Record<string, unknown>;
  confirmedPolicy: KernelPolicyPayload;
  decisionToken?: string;
}) {
  return invoke<DraftRecord>("update_draft", args);
}

export function updateSkilllet(args: {
  projectPath: string;
  id: string;
  input: Record<string, unknown>;
  confirmedPolicy: KernelPolicyPayload;
  decisionToken?: string;
}) {
  return invoke<SkillletRecord>("update_skilllet", args);
}

export function promoteCandidate(args: {
  projectPath: string;
  id: string;
  confirmedPolicy: KernelPolicyPayload;
}) {
  return invoke<SkillletRecord>("promote_candidate", args);
}

export function hideCandidate(args: {
  projectPath: string;
  id: string;
  confirmedPolicy: KernelPolicyPayload;
}) {
  return invoke<CandidateRecord>("hide_candidate", args);
}

export function rejectCandidate(args: {
  projectPath: string;
  id: string;
  reason?: string;
  confirmedPolicy: KernelPolicyPayload;
}) {
  return invoke<CandidateRecord>("reject_candidate", args);
}

export function getTaskStatus() {
  return invoke<DesktopTaskStatus | null>("get_task_status", {});
}

export function getJobHistory() {
  return invoke<DesktopTaskStatus[]>("get_job_history", {});
}

export function scanProjectsJob() {
  return invoke<DesktopJobStart>("scan_projects", {
    roots: [],
    maxDepth: 5,
    confirmedPolicy: {
      mode: "agent-managed",
      auto_approve_min_confidence: 0.72,
      allow_file_writes: true,
      require_review_for_high_risk: false,
    },
  });
}
