import { describe, expect, test } from "bun:test";
import {
  buildMemoryGovernanceBatchActions,
  buildMemoryGovernanceSummary,
  filterMemoryCardsByGovernance,
  summarizeMemoryCardGovernance,
} from "./memory-governance";
import type { MemoryCardRecord, ProjectAssignmentView } from "../types/domain";

function card(overrides: Partial<MemoryCardRecord> = {}): MemoryCardRecord {
  return {
    id: "project:review-boundary",
    title: "保留人工审阅边界",
    body: "涉及高影响变更时保留人工审阅边界。",
    kind: "constraint",
    scope: "project",
    evidence: "用户明确要求保留人工判断。",
    approved_from: "candidate:review-boundary",
    merge_history: [],
    ...overrides,
  };
}

function assignment(targets: Record<string, boolean>): ProjectAssignmentView {
  return {
    project_path: ".",
    enabled_agents: ["codex", "claude-code"],
    target_matrix: {
      agents: ["codex", "claude-code"],
      rows: [{ memory_card_id: "project:review-boundary", title: "保留人工审阅边界", targets }],
    },
  };
}

describe("memory governance utilities", () => {
  test("summarizes healthy assigned cards", () => {
    expect(summarizeMemoryCardGovernance(card(), assignment({ codex: true, "claude-code": false }))).toEqual({
      assignedTargets: ["codex"],
      status: "ready",
      lifecycle: "active",
      sourceLabel: "来自 candidate:review-boundary",
      warnings: [],
      mergeCount: 0,
      conflictDetails: [],
      lineage: [
        { label: "批准来源", value: "candidate:review-boundary" },
        { label: "证据摘要", value: "用户明确要求保留人工判断。" },
      ],
      lifecycleAction: {
        actionKey: "标记休眠-project:review-boundary",
        command: "update_memory_card",
        doneMessage: "已标记为休眠。",
        input: { id: "project:review-boundary", tags: ["status:dormant"] },
        label: "标记休眠",
      },
      expireAction: {
        actionKey: "标记过期-project:review-boundary",
        command: "update_memory_card",
        doneMessage: "已标记为过期。",
        input: { id: "project:review-boundary", tags: ["status:expired"] },
        label: "标记过期",
      },
    });
  });

  test("flags cards without source evidence or assignments", () => {
    expect(
      summarizeMemoryCardGovernance(
        card({ evidence: null, approved_from: null }),
        assignment({ codex: false, "claude-code": false }),
      ),
    ).toMatchObject({
      assignedTargets: [],
      status: "needs-review",
      sourceLabel: "缺少来源",
      warnings: ["未分配给 Agent", "缺少来源证据"],
    });
  });

  test("builds library governance counts", () => {
    const cards = [
      card(),
      card({
        id: "project:no-source",
        title: "测试范围按风险决定",
        body: "改动风险越高，验证覆盖越广。",
        evidence: null,
        approved_from: null,
      }),
    ];
    const view = assignment({ codex: true, "claude-code": false });

    expect(buildMemoryGovernanceSummary(cards, view)).toEqual({
      total: 2,
      assigned: 1,
      unassigned: 1,
      missingSource: 1,
      needsReview: 1,
      conflicts: 0,
      dormant: 0,
      expired: 0,
    });
  });

  test("flags likely duplicate Memory Cards before the library drifts", () => {
    const cards = [
      card(),
      card({
        id: "project:human-review-boundary",
        title: "高风险变更保留人工审阅",
        body: "高影响写入和覆盖操作必须保留人工审阅边界。",
      }),
      card({
        id: "project:test-scope",
        title: "测试范围按风险决定",
        body: "改动风险越高，验证覆盖越广。",
      }),
    ];

    expect(summarizeMemoryCardGovernance(cards[0]!, assignment({ codex: true }), cards)).toMatchObject({
      status: "needs-review",
      warnings: ["可能与 高风险变更保留人工审阅 重复"],
      conflictDetails: [
        {
          id: "project:human-review-boundary",
          title: "高风险变更保留人工审阅",
          similarityLabel: expect.stringMatching(/^\d+%$/),
        },
      ],
      mergeDraftAction: {
        actionKey: "生成合并草稿-project:review-boundary",
        command: "fuse_memory_cards_to_draft",
        doneMessage: "已生成合并草稿，请在审查队列确认。",
        input: {
          id: "draft:fuse-project-review-boundary-project-human-review-boundary",
          title: "合并：保留人工审阅边界",
          sources: ["project:review-boundary", "project:human-review-boundary"],
          targets: ["codex"],
        },
        label: "生成合并草稿",
      },
    });
    const fullAssignment: ProjectAssignmentView = {
      project_path: ".",
      enabled_agents: ["codex"],
      target_matrix: {
        agents: ["codex"],
        rows: cards.map((item) => ({ memory_card_id: item.id, title: item.title, targets: { codex: true } })),
      },
    };

    expect(buildMemoryGovernanceSummary(cards, fullAssignment)).toMatchObject({
      conflicts: 2,
      needsReview: 2,
    });
  });

  test("surfaces merge lineage for review before future edits", () => {
    expect(
      summarizeMemoryCardGovernance(
        card({
          merge_history: [
            { source_id: "project:old-review-boundary", merged_at: "2026-05-01T00:00:00Z", action: "merge" },
          ],
        }),
        assignment({ codex: true }),
      ).lineage,
    ).toEqual([
      { label: "批准来源", value: "candidate:review-boundary" },
      { label: "证据摘要", value: "用户明确要求保留人工判断。" },
      { label: "合并来源", value: "project:old-review-boundary · merge · 2026-05-01T00:00:00Z" },
    ]);
  });

  test("recognizes dormant and expired lifecycle tags", () => {
    const cards = [
      card({ id: "project:dormant", tags: ["status:dormant"] }),
      card({ id: "project:expired", tags: ["lifecycle:expired"] }),
    ];

    const lifecycleAssignment: ProjectAssignmentView = {
      project_path: ".",
      enabled_agents: ["codex"],
      target_matrix: {
        agents: ["codex"],
        rows: cards.map((item) => ({ memory_card_id: item.id, title: item.title, targets: { codex: true } })),
      },
    };

    expect(summarizeMemoryCardGovernance(cards[0]!, lifecycleAssignment)).toMatchObject({
      lifecycle: "dormant",
      warnings: ["已休眠"],
      lifecycleAction: {
        actionKey: "恢复活跃-project:dormant",
        command: "update_memory_card",
        input: { id: "project:dormant", tags: [] },
        label: "恢复活跃",
      },
      expireAction: {
        actionKey: "标记过期-project:dormant",
        command: "update_memory_card",
        input: { id: "project:dormant", tags: ["status:expired"] },
        label: "标记过期",
      },
    });
    expect(summarizeMemoryCardGovernance(cards[1]!, lifecycleAssignment)).toMatchObject({
      lifecycle: "expired",
      warnings: ["已过期"],
      lifecycleAction: {
        actionKey: "恢复活跃-project:expired",
        command: "update_memory_card",
        input: { id: "project:expired", tags: [] },
        label: "恢复活跃",
      },
    });
    expect(buildMemoryGovernanceSummary(cards, lifecycleAssignment)).toMatchObject({
      dormant: 1,
      expired: 1,
      needsReview: 2,
    });
  });

  test("suggests a lifecycle update action for active cards", () => {
    expect(summarizeMemoryCardGovernance(card({ tags: ["workflow"] }), assignment({ codex: true }))).toMatchObject({
      lifecycle: "active",
      lifecycleAction: {
        actionKey: "标记休眠-project:review-boundary",
        command: "update_memory_card",
        input: { id: "project:review-boundary", tags: ["workflow", "status:dormant"] },
        label: "标记休眠",
      },
      expireAction: {
        actionKey: "标记过期-project:review-boundary",
        command: "update_memory_card",
        input: { id: "project:review-boundary", tags: ["workflow", "status:expired"] },
        label: "标记过期",
      },
    });
  });

  test("filters cards by governance issue for batch review", () => {
    const cards = [
      card(),
      card({ id: "project:no-source", title: "无来源", evidence: null, approved_from: null }),
      card({ id: "project:dormant", title: "休眠卡", tags: ["status:dormant"] }),
      card({ id: "project:expired", title: "过期卡", tags: ["status:expired"] }),
      card({
        id: "project:human-review-boundary",
        title: "高风险变更保留人工审阅",
        body: "高影响写入和覆盖操作必须保留人工审阅边界。",
      }),
    ];

    expect(filterMemoryCardsByGovernance(cards, "needs-review", assignment({ codex: true })).map((item) => item.id)).toEqual([
      "project:review-boundary",
      "project:no-source",
      "project:dormant",
      "project:expired",
      "project:human-review-boundary",
    ]);
    expect(filterMemoryCardsByGovernance(cards, "missing-source", assignment({ codex: true })).map((item) => item.id)).toEqual([
      "project:no-source",
    ]);
    expect(filterMemoryCardsByGovernance(cards, "dormant", assignment({ codex: true })).map((item) => item.id)).toEqual([
      "project:dormant",
    ]);
    expect(filterMemoryCardsByGovernance(cards, "expired", assignment({ codex: true })).map((item) => item.id)).toEqual([
      "project:expired",
    ]);
    expect(filterMemoryCardsByGovernance(cards, "conflicts", assignment({ codex: true })).map((item) => item.id)).toContain(
      "project:review-boundary",
    );
  });

  test("builds batch lifecycle actions for filtered dormant cards", () => {
    const cards = [
      card({ id: "project:dormant-a", title: "休眠 A", tags: ["status:dormant", "workflow"] }),
      card({ id: "project:dormant-b", title: "休眠 B", tags: ["lifecycle:dormant"] }),
      card({ id: "project:active", title: "活跃", tags: ["workflow"] }),
    ];
    const visible = filterMemoryCardsByGovernance(cards, "dormant", assignment({ codex: true }));

    expect(buildMemoryGovernanceBatchActions(visible, "dormant", assignment({ codex: true }), cards)).toEqual([
      {
        actionKey: "批量恢复活跃-dormant",
        label: "批量恢复活跃",
        description: "将当前筛选中的 2 张休眠/过期卡恢复为活跃。",
        steps: [
          {
            actionKey: "恢复活跃-project:dormant-a",
            command: "update_memory_card",
            doneMessage: "已恢复为活跃。",
            args: { id: "project:dormant-a", input: { tags: ["workflow"] } },
          },
          {
            actionKey: "恢复活跃-project:dormant-b",
            command: "update_memory_card",
            doneMessage: "已恢复为活跃。",
            args: { id: "project:dormant-b", input: { tags: [] } },
          },
        ],
      },
    ]);
  });

  test("builds deduped batch merge draft actions for conflict review", () => {
    const cards = [
      card(),
      card({
        id: "project:human-review-boundary",
        title: "高风险变更保留人工审阅",
        body: "高影响写入和覆盖操作必须保留人工审阅边界。",
      }),
      card({
        id: "project:test-scope",
        title: "测试范围按风险决定",
        body: "改动风险越高，验证覆盖越广。",
      }),
      card({
        id: "project:test-risk-scope",
        title: "按风险决定测试范围",
        body: "测试范围应该跟随改动风险调整。",
      }),
    ];
    const fullAssignment: ProjectAssignmentView = {
      project_path: ".",
      enabled_agents: ["codex"],
      target_matrix: {
        agents: ["codex"],
        rows: cards.map((item) => ({ memory_card_id: item.id, title: item.title, targets: { codex: true } })),
      },
    };
    const visible = filterMemoryCardsByGovernance(cards, "conflicts", fullAssignment);
    const actions = buildMemoryGovernanceBatchActions(visible, "conflicts", fullAssignment, cards);

    expect(actions).toHaveLength(1);
    expect(actions[0]!.label).toBe("批量生成合并草稿");
    expect(actions[0]!.steps.length).toBeGreaterThanOrEqual(2);
    expect(actions[0]!.steps[0]).toMatchObject({
      command: "fuse_memory_cards_to_draft",
      args: {
        input: {
          sources: expect.arrayContaining(["project:review-boundary", "project:human-review-boundary"]),
          targets: ["codex"],
        },
      },
    });
  });
});
