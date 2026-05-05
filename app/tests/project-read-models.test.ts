import { describe, expect, test } from "bun:test";
import { readModelCommandsForPage } from "../src/project-read-models";

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
    expect(readModelCommandsForPage("skilllets")).toEqual(["get_project_skilllet_library"]);
    expect(readModelCommandsForPage("catalog")).toEqual(["get_project_skilllet_library"]);
    expect(readModelCommandsForPage("agents")).toEqual([
      "get_project_assignment_view",
      "get_project_skilllet_library",
    ]);
  });
});
