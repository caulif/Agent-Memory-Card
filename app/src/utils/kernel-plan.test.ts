import { describe, expect, test } from "bun:test";
import { readModelRefreshPagesForMutation } from "../project-read-models";
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

  test("refreshes all project-facing read models after syncing agent files", () => {
    expect(readModelRefreshPagesForMutation("sync_project")).toEqual([
      "drafts",
      "memory-cards",
      "agents",
    ]);
  });
});
