import {
  getProjectAssignmentView,
  getProjectCandidateInbox,
  getProjectQualityView,
  getProjectReviewInbox,
  getProjectMemoryCardLibrary,
} from "./tauri-client";
import type {
  PageId,
  ProjectAssignmentView,
  ProjectCandidateInbox,
  ProjectQualityView,
  ProjectReviewInbox,
  ProjectMemoryCardLibrary,
} from "./ui-helpers";

export type ProjectReadModelCommand =
  | "get_project_candidate_inbox"
  | "get_project_review_inbox"
  | "get_project_memory_card_library"
  | "get_project_assignment_view"
  | "get_project_quality_view";

export type ProjectReadModels = {
  candidates?: ProjectCandidateInbox;
  inbox?: ProjectReviewInbox;
  library?: ProjectMemoryCardLibrary;
  assignment?: ProjectAssignmentView;
  quality?: ProjectQualityView;
};

export function readModelCommandsForPage(page: PageId): ProjectReadModelCommand[] {
  if (page === "drafts") {
    return [
      "get_project_candidate_inbox",
      "get_project_review_inbox",
      "get_project_memory_card_library",
      "get_project_assignment_view",
      "get_project_quality_view",
    ];
  }
  if (page === "memory-cards") {
    return ["get_project_memory_card_library"];
  }
  if (page === "agents") {
    return ["get_project_assignment_view", "get_project_memory_card_library"];
  }
  if (page === "settings") {
    return [
      "get_project_candidate_inbox",
      "get_project_review_inbox",
      "get_project_memory_card_library",
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
    return ["drafts", "memory-cards", "agents"];
  }
  if (["fuse_memory_cards_to_draft"].includes(command)) {
    return ["drafts", "memory-cards", "agents"];
  }
  if (["promote_candidate"].includes(command)) {
    return ["drafts", "memory-cards", "agents"];
  }
  if (["update_memory_card", "delete_memory_card", "promote_memory_card_to_global", "install_global_memory_card_to_project", "merge_memory_cards"].includes(command)) {
    return ["memory-cards", "agents"];
  }
  if (["set_agent_enabled", "set_memory_card_targets", "clear_memory_card_targets"].includes(command)) {
    return ["agents"];
  }
  if (["install_catalog_package"].includes(command)) {
    return ["memory-cards", "agents"];
  }
  if ([
    "import_project",
    "import_artifact_drifts",
    "import_artifact_drift_path",
    "keep_artifact_drifts",
    "keep_artifact_drift_path",
    "discard_artifact_drifts",
    "discard_artifact_drift_path",
    "sync_project",
    "clear_project_history",
  ].includes(command)) {
    return ["drafts", "memory-cards", "agents"];
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
      if (command === "get_project_memory_card_library") {
        models.library = await getProjectMemoryCardLibrary(projectPath);
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
