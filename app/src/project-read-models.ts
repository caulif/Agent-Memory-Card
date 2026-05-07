import {
  getProjectAssignmentView,
  getProjectCandidateInbox,
  getProjectQualityView,
  getProjectReviewInbox,
  getProjectSkillletLibrary,
} from "./tauri-client";
import type {
  PageId,
  ProjectAssignmentView,
  ProjectCandidateInbox,
  ProjectQualityView,
  ProjectReviewInbox,
  ProjectSkillletLibrary,
} from "./ui-helpers";

export type ProjectReadModelCommand =
  | "get_project_candidate_inbox"
  | "get_project_review_inbox"
  | "get_project_skilllet_library"
  | "get_project_assignment_view"
  | "get_project_quality_view";

export type ProjectReadModels = {
  candidates?: ProjectCandidateInbox;
  inbox?: ProjectReviewInbox;
  library?: ProjectSkillletLibrary;
  assignment?: ProjectAssignmentView;
  quality?: ProjectQualityView;
};

export function readModelCommandsForPage(page: PageId): ProjectReadModelCommand[] {
  if (page === "drafts") {
    return ["get_project_candidate_inbox", "get_project_review_inbox"];
  }
  if (page === "skilllets") {
    return ["get_project_skilllet_library"];
  }
  if (page === "agents") {
    return ["get_project_assignment_view", "get_project_skilllet_library"];
  }
  if (page === "settings") {
    return [
      "get_project_candidate_inbox",
      "get_project_review_inbox",
      "get_project_skilllet_library",
      "get_project_assignment_view",
      "get_project_quality_view",
    ];
  }
  return ["get_project_candidate_inbox", "get_project_review_inbox"];
}

export function readModelRefreshPagesForMutation(command: string): PageId[] {
  if (["hide_candidate", "reject_candidate", "gc_candidates", "reject_draft", "update_draft", "merge_drafts"].includes(command)) {
    return ["drafts"];
  }
  if (["approve_draft"].includes(command)) {
    return ["drafts", "skilllets", "agents"];
  }
  if (["fuse_skilllets_to_draft"].includes(command)) {
    return ["drafts", "skilllets", "agents"];
  }
  if (["promote_candidate"].includes(command)) {
    return ["drafts", "skilllets", "agents"];
  }
  if (["update_skilllet", "promote_skilllet_to_global", "install_global_skilllet_to_project", "merge_skilllets"].includes(command)) {
    return ["skilllets", "agents"];
  }
  if (["set_agent_enabled", "set_skilllet_targets"].includes(command)) {
    return ["agents"];
  }
  if (["install_catalog_package"].includes(command)) {
    return ["skilllets", "agents"];
  }
  if (["import_project", "import_artifact_drifts", "sync_project"].includes(command)) {
    return ["drafts", "skilllets", "agents"];
  }
  return ["drafts"];
}

export async function loadProjectReadModelsFromTauri(projectPath: string, page: PageId): Promise<ProjectReadModels> {
  const commands = readModelCommandsForPage(page);
  const models: ProjectReadModels = {};
  await Promise.all(
    commands.map(async (command) => {
      if (command === "get_project_candidate_inbox") {
        models.candidates = await getProjectCandidateInbox(projectPath);
      }
      if (command === "get_project_review_inbox") {
        models.inbox = await getProjectReviewInbox(projectPath);
      }
      if (command === "get_project_skilllet_library") {
        models.library = await getProjectSkillletLibrary(projectPath);
      }
      if (command === "get_project_assignment_view") {
        models.assignment = await getProjectAssignmentView(projectPath);
      }
      if (command === "get_project_quality_view") {
        models.quality = await getProjectQualityView(projectPath);
      }
    }),
  );
  return models;
}
