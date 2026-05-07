import { describe, expect, test } from "bun:test";
import { filterCandidatesForInbox, projectOverviewMetrics } from "./candidates";
import type { CandidateRecord, ProjectDashboard, ProjectSnapshot } from "../types/domain";

function candidate(id: string, confidence: number | undefined, kind = "procedure"): CandidateRecord {
  return {
    id,
    title: id,
    body: "Durable candidate body",
    kind,
    scope: "project",
    evidence: "test",
    confidence,
    status: "candidate",
    targets: [],
  };
}

describe("candidate inbox utilities", () => {
  test("keeps high-confidence inbox results sorted before lower confidence items", () => {
    const records = [
      candidate("low", 0.4),
      candidate("high", 0.91),
      candidate("medium", 0.72),
    ];

    expect(filterCandidatesForInbox(records, "all").map((item) => item.id)).toEqual([
      "high",
      "medium",
      "low",
    ]);
    expect(filterCandidatesForInbox(records, "high").map((item) => item.id)).toEqual([
      "high",
      "medium",
    ]);
  });

  test("uses already-filtered snapshot drafts ahead of dashboard fallback counts", () => {
    const snapshot = {
      candidates: [],
      drafts: [{ id: "draft:visible" }],
      skilllets: [],
      observations: [],
    } as unknown as ProjectSnapshot;
    const dashboard = { draft_count: 99 } as ProjectDashboard;

    expect(projectOverviewMetrics(snapshot, dashboard, 0).draftCount).toBe(1);
    expect(projectOverviewMetrics(null, dashboard, 0).draftCount).toBe(99);
  });
});
