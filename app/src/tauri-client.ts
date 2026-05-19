import { invoke } from "@tauri-apps/api/core";
import type {
  CandidateRecord,
  DraftRecord,
  DesktopAppState,
  DesktopJobStart,
  DesktopTaskStatus,
  KernelPlanRequest,
  KernelPolicyPayload,
  CustomProviderConfig,
  ProviderStatusReport,
  ProjectAssignmentView,
  ProjectCandidateInbox,
  ProjectDashboard,
  ProjectEvalRunView,
  ProjectQualityView,
  ProjectReviewInbox,
  ProjectMemoryCardLibrary,
  ProjectSkillLibrary,
  ProjectSnapshot,
  MemoryCardRecord,
  SetupChecklistReport,
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

export function getProjectMemoryCardLibrary(projectPath: string) {
  return invoke<ProjectMemoryCardLibrary>("get_project_memory_card_library", { projectPath });
}

export function getProjectSkillLibrary(projectPath: string) {
  return invoke<ProjectSkillLibrary>("get_project_skill_library", { projectPath });
}

export function getProjectAssignmentView(projectPath: string) {
  return invoke<ProjectAssignmentView>("get_project_assignment_view", { projectPath });
}

export function getProjectQualityView(projectPath: string) {
  return invoke<ProjectQualityView>("get_project_quality_view", { projectPath });
}

export function getProjectEvalRun(projectPath: string) {
  return invoke<ProjectEvalRunView>("get_project_eval_run", { projectPath });
}

export function getCustomProviderConfig(projectPath: string) {
  return invoke<CustomProviderConfig>("get_custom_provider_config", { projectPath });
}

export function saveCustomProviderConfig(args: {
  projectPath: string;
  input: CustomProviderConfig;
  confirmedPolicy: KernelPolicyPayload;
  decisionToken?: string;
}) {
  return invoke<CustomProviderConfig>("save_custom_provider_config", args);
}

export function getSetupChecklist(projectPath: string) {
  return invoke<SetupChecklistReport>("get_setup_checklist", { projectPath });
}

export function testProviderStatus(projectPath: string, liveRequest = true) {
  return invoke<ProviderStatusReport>("test_provider_status", { projectPath, liveRequest });
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

export function clearProjectHistory(args: {
  projectPath: string;
  confirmedPolicy: KernelPolicyPayload;
  decisionToken?: string;
}) {
  return invoke<ProjectMutationAck>("clear_project_history", args);
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

export function updateCandidate(args: {
  projectPath: string;
  id: string;
  input: Record<string, unknown>;
  confirmedPolicy: KernelPolicyPayload;
  decisionToken?: string;
}) {
  return invoke<CandidateRecord>("update_candidate", args);
}

export function updateMemoryCard(args: {
  projectPath: string;
  id: string;
  input: Record<string, unknown>;
  confirmedPolicy: KernelPolicyPayload;
  decisionToken?: string;
}) {
  return invoke<MemoryCardRecord>("update_memory_card", args);
}

export function promoteCandidate(args: {
  projectPath: string;
  id: string;
  confirmedPolicy: KernelPolicyPayload;
}) {
  return invoke<MemoryCardRecord>("promote_candidate", args);
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
