import { describe, expect, test } from "bun:test";
import { readModelRefreshPagesForMutation } from "../project-read-models";
import { readModelCommandsForPage } from "../project-read-models";
import { buildKernelPlanForInvoke } from "./kernel-plan";

describe("kernel mutation planning", () => {
  test("plans clear Memory Card target mutations and refreshes assignment views", () => {
    expect(buildKernelPlanForInvoke("clear_memory_card_targets", { agent: "codex" })).toEqual({
      command: { type: "clear-memory-card-targets", agent: "codex" },
      payload: { agent: "codex" },
    });

    expect(readModelRefreshPagesForMutation("clear_memory_card_targets")).toEqual(["agents"]);
  });

  test("plans full project history clearing and refreshes all project views", () => {
    expect(buildKernelPlanForInvoke("clear_project_history")).toEqual({
      command: { type: "clear-project-history" },
      payload: {},
    });

    expect(readModelRefreshPagesForMutation("clear_project_history")).toEqual(["drafts", "memory-cards", "agents"]);
  });

  test("plans Memory Card deletion and refreshes library plus assignments", () => {
    expect(buildKernelPlanForInvoke("delete_memory_card", { id: "project:use-axios" })).toEqual({
      command: { type: "delete-memory-card", id: "project:use-axios" },
      payload: { id: "project:use-axios" },
    });

    expect(readModelRefreshPagesForMutation("delete_memory_card")).toEqual(["memory-cards", "agents"]);
  });

  test("plans Memory Card lifecycle tag updates", () => {
    const input = { tags: ["status:dormant"] };

    expect(buildKernelPlanForInvoke("update_memory_card", { id: "project:review-boundary", input })).toEqual({
      command: { type: "update-memory-card", id: "project:review-boundary" },
      payload: { id: "project:review-boundary", input },
    });

    expect(readModelRefreshPagesForMutation("update_memory_card")).toEqual(["memory-cards", "agents"]);
  });

  test("refreshes all project-facing read models after syncing agent files", () => {
    expect(readModelRefreshPagesForMutation("sync_project")).toEqual([
      "drafts",
      "memory-cards",
      "agents",
    ]);
  });

  test("plans artifact drift import and refreshes review-facing views", () => {
    expect(buildKernelPlanForInvoke("import_artifact_drifts")).toEqual({
      command: { type: "import-artifact-drifts" },
      payload: {},
    });

    expect(readModelRefreshPagesForMutation("import_artifact_drifts")).toEqual([
      "drafts",
      "memory-cards",
      "agents",
    ]);
  });

  test("plans explicit artifact drift keep and discard resolutions", () => {
    expect(buildKernelPlanForInvoke("keep_artifact_drifts")).toEqual({
      command: { type: "keep-artifact-drifts" },
      payload: {},
    });
    expect(buildKernelPlanForInvoke("discard_artifact_drifts")).toEqual({
      command: { type: "discard-artifact-drifts" },
      payload: {},
    });

    expect(readModelRefreshPagesForMutation("keep_artifact_drifts")).toEqual([
      "drafts",
      "memory-cards",
      "agents",
    ]);
    expect(readModelRefreshPagesForMutation("discard_artifact_drifts")).toEqual([
      "drafts",
      "memory-cards",
      "agents",
    ]);
  });

  test("plans file-scoped artifact drift resolutions", () => {
    expect(buildKernelPlanForInvoke("import_artifact_drift_path", { artifactPath: "AGENTS.md" })).toEqual({
      command: { type: "import-artifact-drifts" },
      payload: { artifact_path: "AGENTS.md" },
    });
    expect(buildKernelPlanForInvoke("keep_artifact_drift_path", { artifactPath: "AGENTS.md" })).toEqual({
      command: { type: "keep-artifact-drifts" },
      payload: { artifact_path: "AGENTS.md" },
    });
    expect(buildKernelPlanForInvoke("discard_artifact_drift_path", { artifactPath: "AGENTS.md" })).toEqual({
      command: { type: "discard-artifact-drifts" },
      payload: { artifact_path: "AGENTS.md" },
    });
    expect(readModelRefreshPagesForMutation("import_artifact_drift_path")).toEqual([
      "drafts",
      "memory-cards",
      "agents",
    ]);
  });

  test("plans Memory Card fusion as a reviewable draft", () => {
    const input = {
      id: "draft:fuse-review",
      title: "合并：审阅边界",
      sources: ["project:a", "project:b"],
      targets: ["codex"],
    };

    expect(buildKernelPlanForInvoke("fuse_memory_cards_to_draft", { input })).toEqual({
      command: { type: "fuse-memory-cards", ids: ["project:a", "project:b"], engine: "local" },
      payload: { input },
    });

    expect(readModelRefreshPagesForMutation("fuse_memory_cards_to_draft")).toEqual([
      "drafts",
      "memory-cards",
      "agents",
    ]);
  });

  test("loads closure read models on the review page", () => {
    expect(readModelCommandsForPage("drafts")).toEqual([
      "get_project_candidate_inbox",
      "get_project_review_inbox",
      "get_project_memory_card_library",
      "get_project_assignment_view",
      "get_project_quality_view",
    ]);
  });
});
