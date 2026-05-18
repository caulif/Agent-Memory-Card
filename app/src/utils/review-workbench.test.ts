import { describe, expect, test } from "bun:test";
import { driftFileActionKey } from "../components/review/ArtifactDriftGroup";
import {
  buildWorkflowContractState,
  buildReviewClosureState,
  paginateArtifactBlockingTargets,
  summarizeArtifactPreview,
  summarizeCandidateEvidence,
  summarizeDraftEvidence,
} from "./review-workbench";
import type { CandidateRecord, DraftRecord, ProjectAssignmentView, ProjectQualityView } from "../types/domain";

function candidate(overrides: Partial<CandidateRecord> = {}): CandidateRecord {
  return {
    id: "candidate:review-boundary",
    title: "保留人工审阅边界",
    body: "涉及高影响变更时，区分自动化建议和需要人工确认的部分。",
    kind: "constraint",
    scope: "global",
    evidence: "用户强调不要越过人工审阅边界。",
    confidence: 0.88,
    status: "candidate",
    targets: ["codex"],
    source_observations: ["obs-1", "obs-2"],
    extraction: {
      source_observations: ["obs-2", "obs-3"],
      suggested_action: {
        action: "new_candidate",
        route: "always_on_rule",
        compile_enabled: true,
      },
    },
    ...overrides,
  };
}

function draft(overrides: Partial<DraftRecord> = {}): DraftRecord {
  return {
    id: "draft:quality",
    title: "候选质量优先于数量",
    body: "提炼候选时优先保证候选质量，而不是追求数量。",
    kind: "preference",
    scope: "project",
    evidence: "候选质量优先于数量。",
    confidence: 0.81,
    status: "draft",
    targets: ["codex"],
    extraction: {
      source_observations: ["obs-9"],
      suggested_action: {
        action: "new_candidate",
        route: "review_only",
        compile_enabled: false,
      },
    },
    ...overrides,
  };
}

describe("review workbench utilities", () => {
  test("defines product terms that hide Candidate and Draft implementation details", () => {
    const state = buildWorkflowContractState({ hasProject: false });

    expect(state.productTerms).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ implementation: "Candidate", product: "Suggestion" }),
        expect.objectContaining({ implementation: "Draft", product: "Suggestion" }),
        expect.objectContaining({ implementation: "Assignment Matrix", product: "Agent Loadout" }),
        expect.objectContaining({ implementation: "Build Preview / Drift", product: "Artifact Preview" }),
      ]),
    );
  });

  test("computes the canonical first-run workflow action", () => {
    expect(buildWorkflowContractState({ hasProject: false }).primaryAction).toEqual({
      id: "select-project",
      label: "选择项目",
      detail: "先扫描或打开一个本地 Claude Code / Codex 项目。",
    });

    expect(
      buildWorkflowContractState({
        hasProject: true,
        dashboard: {
          project_path: "C:/demo",
          candidate_count: 0,
          draft_count: 0,
          memory_card_count: 0,
          observation_count: 0,
          global_memory_card_count: 0,
          enabled_agents: ["codex"],
          warning_count: 0,
        },
      }).primaryAction,
    ).toMatchObject({
      id: "run-evolution",
      label: "提炼建议",
      targetPage: "drafts",
    });
  });

  test("walks reviewed suggestions toward loadout and artifact sync", () => {
    const assignment = {
      target_matrix: {
        agents: ["codex"],
        rows: [{ memory_card_id: "project:quality", title: "质量", targets: { codex: false } }],
      },
    } as ProjectAssignmentView;
    const assigned = {
      target_matrix: {
        agents: ["codex"],
        rows: [{ memory_card_id: "project:quality", title: "质量", targets: { codex: true } }],
      },
    } as ProjectAssignmentView;
    const readyQuality = {
      build_preview: { actions: ["Update AGENTS.md"], warnings: [] },
      status: { warnings: [] },
    } as ProjectQualityView;

    expect(
      buildWorkflowContractState({
        hasProject: true,
        candidates: [candidate()],
      }).primaryAction,
    ).toMatchObject({ id: "review-suggestions", targetPage: "drafts" });
    expect(
      buildWorkflowContractState({
        hasProject: true,
        candidates: [candidate()],
      }).warnings,
    ).toEqual([
      {
        id: "pending-suggestions",
        label: "1 条待审建议",
        detail: "先确认来源证据，再批准为 Memory Card。",
        targetPage: "drafts",
        tone: "info",
      },
    ]);

    expect(
      buildWorkflowContractState({
        hasProject: true,
        candidates: [],
        drafts: [],
        memoryCards: [{ id: "project:quality" }],
        assignment,
      }).primaryAction,
    ).toMatchObject({ id: "assign-loadout", targetPage: "agents" });

    expect(
      buildWorkflowContractState({
        hasProject: true,
        candidates: [],
        drafts: [],
        memoryCards: [{ id: "project:quality" }],
        assignment: assigned,
        quality: readyQuality,
      }).primaryAction,
    ).toMatchObject({ id: "preview-artifacts", targetPage: "agents" });
    expect(
      buildWorkflowContractState({
        hasProject: true,
        candidates: [],
        drafts: [],
        memoryCards: [{ id: "project:quality" }],
        assignment: assigned,
        quality: readyQuality,
      }).warnings,
    ).toEqual([
      {
        id: "artifact-preview",
        label: "1 个 Artifact 动作",
        detail: "同步前检查 AGENTS.md / CLAUDE.md / Skill diff。",
        targetPage: "agents",
        tone: "info",
      },
    ]);
  });

  test("treats artifact drift as a blocking workflow state", () => {
    const assigned = {
      target_matrix: {
        agents: ["codex"],
        rows: [{ memory_card_id: "project:quality", title: "质量", targets: { codex: true } }],
      },
    } as ProjectAssignmentView;
    const driftedQuality = {
      build_preview: {
        actions: ["Update AGENTS.md"],
        warnings: [],
        artifact_previews: [
          {
            agent: "codex",
            kind: "codex:instructions",
            path: "AGENTS.md",
            status: "drifted",
            current_hash: "manual",
            expected_hash: "expected",
            diff_preview: ["- manual", "+ generated"],
          },
        ],
      },
      status: { warnings: [] },
    } as ProjectQualityView;

    expect(
      buildWorkflowContractState({
        hasProject: true,
        memoryCards: [{ id: "project:quality" }],
        assignment: assigned,
        quality: driftedQuality,
      }).primaryAction,
    ).toMatchObject({ id: "resolve-artifact-drift", label: "处理 Artifact Drift", targetPage: "drafts" });
    expect(
      buildWorkflowContractState({
        hasProject: true,
        memoryCards: [{ id: "project:quality" }],
        assignment: assigned,
        quality: driftedQuality,
      }).warnings[0],
    ).toMatchObject({ id: "artifact-drift", targetPage: "drafts", tone: "blocked" });
  });

  test("summarizes candidate grounding from both top-level and extraction sources", () => {
    expect(summarizeCandidateEvidence(candidate())).toEqual({
      sourceCount: 3,
      quoteCount: 0,
      confidenceLabel: "88%",
      recurrenceLabel: "复现证据 3 来源",
      riskLabel: "低风险",
      riskTone: "safe",
      routeLabel: "写入规则",
      compileLabel: "会编译",
      actionLabel: "新 Memory Card",
      artifactImpactLabel: "写入规则，会编译",
      status: "grounded",
      warnings: [],
      evidencePreview: "用户强调不要越过人工审阅边界。",
    });
  });

  test("flags weak candidate grounding before review actions", () => {
    expect(
      summarizeCandidateEvidence(
        candidate({
          evidence: "",
          confidence: 0.42,
          source_observations: [],
          extraction: { source_observations: [] },
        }),
      ),
    ).toMatchObject({
      sourceCount: 0,
      quoteCount: 0,
      confidenceLabel: "42%",
      recurrenceLabel: "无来源",
      riskLabel: "高风险",
      riskTone: "blocked",
      status: "weak",
      warnings: ["缺少来源观察", "缺少证据摘要", "置信度偏低"],
    });
  });

  test("summarizes draft grounding and compile route", () => {
    expect(summarizeDraftEvidence(draft())).toEqual({
      sourceCount: 1,
      quoteCount: 0,
      confidenceLabel: "81%",
      recurrenceLabel: "单次证据",
      riskLabel: "需复核",
      riskTone: "review",
      routeLabel: "仅审阅",
      compileLabel: "不编译",
      actionLabel: "新 Memory Card",
      artifactImpactLabel: "仅审阅，不会写入 Agent 文件",
      status: "grounded",
      warnings: [],
      evidencePreview: "候选质量优先于数量。",
    });
  });

  test("surfaces risk, recurrence, action, and artifact impact from evidence metadata", () => {
    expect(
      summarizeCandidateEvidence(
        candidate({
          extraction: {
            source_observations: ["obs-1"],
            evidence_bundle: {
              source_observation_ids: ["obs-1"],
              quotes: [
                { observation_id: "obs-1", text: "以后都这样做。" },
                { observation_id: "obs-1", text: "别再自动覆盖。" },
              ],
              validity: "weak",
              source_trust: "user_feedback",
            },
            suggested_action: {
              action: "merge_into_existing",
              route: "workflow_skill",
              compile_enabled: true,
            },
          },
        }),
      ),
    ).toMatchObject({
      sourceCount: 2,
      quoteCount: 2,
      recurrenceLabel: "复现证据 2 来源",
      riskLabel: "合并需复核",
      riskTone: "review",
      actionLabel: "合并到现有 Memory Card",
      artifactImpactLabel: "生成 Skill 草稿，会编译",
      warnings: ["证据较弱"],
    });
  });

  test("points the dashboard to the next closure step", () => {
    const assignment = {
      target_matrix: {
        agents: ["codex"],
        rows: [{ memory_card_id: "project:quality", title: "质量", targets: { codex: false } }],
      },
    } as ProjectAssignmentView;
    const quality = {
      build_preview: { actions: ["Update AGENTS.md"], warnings: [] },
      status: { warnings: ["artifact drifted"] },
    } as ProjectQualityView;

    expect(
      buildReviewClosureState({
        candidates: [candidate()],
        drafts: [],
        memoryCards: [],
        assignment: null,
        quality: null,
      }).nextStep,
    ).toEqual({ id: "review-candidates", label: "审查建议", detail: "先处理 1 条系统建议。" });

    expect(
      buildReviewClosureState({
        candidates: [],
        drafts: [draft()],
        memoryCards: [],
        assignment: null,
        quality: null,
      }).nextStep.id,
    ).toBe("review-drafts");

    expect(
      buildReviewClosureState({
        candidates: [],
        drafts: [],
        memoryCards: [{ id: "project:quality" }],
        assignment,
        quality: null,
      }).nextStep.id,
    ).toBe("assign-cards");

    expect(
      buildReviewClosureState({
        candidates: [],
        drafts: [],
        memoryCards: [{ id: "project:quality" }],
        assignment,
        quality,
      }).nextStep.id,
    ).toBe("resolve-drift");
  });

  test("summarizes artifact preview actions and drift blockers", () => {
    expect(
      summarizeArtifactPreview({
        build_preview: {
          actions: ["Update AGENTS.md", "Write .agents/skills/review/SKILL.md"],
          warnings: ["Skill output will change"],
        },
        status: { warnings: ["artifact drifted at AGENTS.md"] },
      } as ProjectQualityView),
    ).toEqual({
      status: "blocked",
      actionCount: 2,
      warningCount: 2,
      actions: ["Update AGENTS.md", "Write .agents/skills/review/SKILL.md"],
      warnings: ["artifact drifted at AGENTS.md", "Skill output will change"],
      blockingTargets: [],
      lastSync: null,
      headline: "写入前需处理 Drift",
      detail: "2 个生成动作已计算，但 1 个 drift 警告会阻止安全写入。",
      recoveryAction: {
        actionKey: "导入 Drift",
        command: "import_artifact_drifts",
        description: "把检测到的手动改动转成待审建议，保留人工判断。",
        doneMessage: "已把手动改动导入为待审建议。",
        label: "导入为草稿",
        tone: "safe",
      },
      recoveryActions: [
        {
          actionKey: "导入 Drift",
          command: "import_artifact_drifts",
          description: "把检测到的手动改动转成待审建议，保留人工判断。",
          doneMessage: "已把手动改动导入为待审建议。",
          label: "导入为草稿",
          tone: "safe",
        },
        {
          actionKey: "保留 Drift",
          command: "keep_artifact_drifts",
          description: "接受当前文件内容，并更新 artifact lock，不改写手动内容。",
          doneMessage: "已接受当前生成文件的手动改动。",
          label: "保留手改",
          tone: "safe",
        },
        {
          actionKey: "丢弃 Drift",
          command: "discard_artifact_drifts",
          confirmMessage: "这会丢弃当前 drift 文件里的手动改动，并恢复为 Memory Card 生成内容。确定继续吗？",
          description: "用 Memory Card 生成内容覆盖 drift 文件，会丢弃当前手动改动。",
          doneMessage: "已恢复为 Memory Card 生成的文件内容。",
          label: "丢弃手改",
          tone: "destructive",
        },
      ],
      targets: [
        { kind: "artifact", label: "Update AGENTS.md", path: "AGENTS.md" },
        { kind: "artifact", label: "Write .agents/skills/review/SKILL.md", path: ".agents/skills/review/SKILL.md" },
      ],
    });

    expect(
      summarizeArtifactPreview({
        build_preview: { actions: ["Update CLAUDE.md"], warnings: [] },
        status: { warnings: [] },
      } as ProjectQualityView).status,
    ).toBe("ready");

    expect(summarizeArtifactPreview(null).status).toBe("empty");
  });

  test("surfaces last sync checkpoint for rollback guidance", () => {
    const summary = summarizeArtifactPreview({
      build_preview: { actions: ["Update AGENTS.md"], warnings: [] },
      status: {
        warnings: [],
        last_sync: {
          id: "sync-20260518T010203Z",
          created_at: "2026-05-18T01:02:03Z",
          artifact_count: 1,
          artifacts: [{ path: "AGENTS.md", kind: "codex:instructions", hash: "sha256:abc" }],
          memory_card_ids: ["project:quality"],
          rollback_instructions: ["Review hashes before changing files."],
        },
      },
    } as ProjectQualityView);

    expect(summary.lastSync).toMatchObject({
      id: "sync-20260518T010203Z",
      artifact_count: 1,
      memory_card_ids: ["project:quality"],
    });
  });

  test("groups structured drift targets for blocked artifact review", () => {
    expect(
      summarizeArtifactPreview({
        build_preview: {
          actions: ["Would write AGENTS.md", "Would write CLAUDE.md"],
          warnings: [],
          artifact_previews: [
            {
              agent: "codex",
              kind: "codex:instructions",
              path: "AGENTS.md",
              status: "drifted",
              current_hash: "sha256:manual",
              expected_hash: "sha256:expected",
              diff_preview: ["- manual edit", "+ generated content"],
              diff_lines: ["- manual edit", "+ generated content"],
              diff_truncated: false,
            },
            {
              agent: "claude-code",
              kind: "claude:instructions",
              path: "CLAUDE.md",
              status: "unmanaged",
              current_hash: null,
              expected_hash: "sha256:expected",
              diff_preview: ["+ generated file"],
              diff_lines: ["+ generated file"],
              diff_truncated: true,
            },
          ],
        },
        status: { warnings: [] },
      } as ProjectQualityView),
    ).toMatchObject({
      status: "blocked",
      blockingTargets: [
        {
          kind: "artifact",
          path: "AGENTS.md",
          status: "drifted",
          diffPreview: ["- manual edit", "+ generated content"],
          diffTruncated: false,
        },
        {
          kind: "artifact",
          path: "CLAUDE.md",
          status: "unmanaged",
          diffPreview: ["+ generated file"],
          diffTruncated: true,
        },
      ],
    });
  });

  test("paginates blocking drift targets for large artifact previews", () => {
    const targets = Array.from({ length: 13 }, (_, index) => ({
      kind: "artifact" as const,
      label: `drifted · file-${index}.md`,
      path: `file-${index}.md`,
      status: "drifted",
    }));

    expect(paginateArtifactBlockingTargets(targets, 0, 5)).toMatchObject({
      page: 0,
      pageSize: 5,
      total: 13,
      totalPages: 3,
      hasPrevious: false,
      hasNext: true,
      items: targets.slice(0, 5),
    });
    expect(paginateArtifactBlockingTargets(targets, 2, 5)).toMatchObject({
      page: 2,
      hasPrevious: true,
      hasNext: false,
      items: targets.slice(10, 13),
    });
    expect(paginateArtifactBlockingTargets(targets, 99, 5).page).toBe(2);
    expect(paginateArtifactBlockingTargets(targets, -1, 5).page).toBe(0);
  });

  test("scopes artifact drift pending state to a single file operation", () => {
    expect(driftFileActionKey("AGENTS.md", "import")).toBe("drift:import:AGENTS.md");
    expect(driftFileActionKey("AGENTS.md", "discard")).not.toBe(
      driftFileActionKey("CLAUDE.md", "discard"),
    );
  });

  test("prefers structured artifact preview rows over action parsing", () => {
    expect(
      summarizeArtifactPreview({
        build_preview: {
          actions: ["Would write AGENTS.md"],
          warnings: [],
          artifact_previews: [
            {
              agent: "codex",
              kind: "codex:instructions",
              path: "AGENTS.md",
              status: "create",
              current_hash: null,
              expected_hash: "sha256:abc",
              diff_preview: ["+ ### Keep Review Boundary", "+ Require human confirmation."],
              diff_lines: ["+ ### Keep Review Boundary", "+ Require human confirmation.", "+ Write safely."],
              diff_truncated: false,
            },
          ],
        },
        status: { warnings: [] },
      } as ProjectQualityView),
    ).toMatchObject({
      status: "ready",
      actionCount: 1,
      headline: "可预览写入",
      targets: [
        {
          kind: "artifact",
          label: "create · AGENTS.md",
          path: "AGENTS.md",
          status: "create",
          agent: "codex",
          diffPreview: ["+ ### Keep Review Boundary", "+ Require human confirmation."],
          diffLines: ["+ ### Keep Review Boundary", "+ Require human confirmation.", "+ Write safely."],
          diffTruncated: false,
        },
      ],
    });
  });
});
