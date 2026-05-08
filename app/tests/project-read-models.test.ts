import { describe, expect, test } from "bun:test";
import { readModelCommandsForPage, readModelRefreshPagesForMutation } from "../src/project-read-models";
import { buildKernelPlanForInvoke, PAGES } from "../src/ui-helpers";

describe("project read model loading strategy", () => {
  test("loads inbox without quality or full snapshot commands", () => {
    expect(readModelCommandsForPage("drafts")).toEqual([
      "get_project_candidate_inbox",
      "get_project_review_inbox",
    ]);
  });

  test("keeps quality checks out of the default inbox and moves them to settings", () => {
    expect(readModelCommandsForPage("drafts")).not.toContain("get_project_quality_view");
    expect(readModelCommandsForPage("settings")).toContain("get_project_quality_view");
  });

  test("loads only the read models each secondary page needs", () => {
    expect(readModelCommandsForPage("memory-cards")).toEqual(["get_project_memory_card_library"]);
    expect(readModelCommandsForPage("agents")).toEqual([
      "get_project_assignment_view",
      "get_project_memory_card_library",
    ]);
  });

  test("does not expose the catalog as a top-level page", () => {
    expect(PAGES.map((page) => page.id)).toEqual(["drafts", "memory-cards", "agents", "settings"]);
  });

  test("refreshes draft inbox after candidate gc", () => {
    expect(readModelRefreshPagesForMutation("gc_candidates")).toEqual(["drafts"]);
  });

  test("refreshes draft, memory_card, and assignment models after memory_card fusion", () => {
    expect(readModelRefreshPagesForMutation("fuse_memory_cards_to_draft")).toEqual([
      "drafts",
      "memory-cards",
      "agents",
    ]);
  });

  test("refreshes draft, memory_card, and assignment models after draft approval", () => {
    expect(readModelRefreshPagesForMutation("approve_draft")).toEqual([
      "drafts",
      "memory-cards",
      "agents",
    ]);
  });

  test("plans candidate gc as a low-risk kernel command", () => {
    expect(buildKernelPlanForInvoke("gc_candidates")).toEqual({
      command: { type: "gc-candidates" },
      payload: {},
    });
  });
});
