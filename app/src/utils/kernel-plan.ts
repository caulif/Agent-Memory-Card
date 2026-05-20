import type { KernelPlanRequest, KernelPolicyPayload, PlanReviewResult } from "../types/domain";

export function confirmedAgentManagedPolicy(): KernelPolicyPayload {
  return {
    mode: "agent-managed",
    auto_approve_min_confidence: 0.72,
    allow_file_writes: true,
    require_review_for_high_risk: false,
  };
}

export function manualReviewPolicy(): KernelPolicyPayload {
  return {
    mode: "manual",
    auto_approve_min_confidence: 1,
    allow_file_writes: false,
    require_review_for_high_risk: true,
  };
}

export function buildKernelPlanForInvoke(commandName: string, args: Record<string, unknown> = {}): KernelPlanRequest | null {
  if (commandName === "approve_draft") {
    return { command: { type: "approve-draft", id: args.id }, payload: { id: args.id } };
  }
  if (commandName === "reject_draft") {
    return { command: { type: "reject-draft", id: args.id }, payload: { id: args.id } };
  }
  if (commandName === "promote_candidate") {
    return { command: { type: "promote-candidate", id: args.id }, payload: { id: args.id } };
  }
  if (commandName === "hide_candidate") {
    return { command: { type: "hide-candidate", id: args.id }, payload: { id: args.id } };
  }
  if (commandName === "reject_candidate") {
    return { command: { type: "reject-candidate", id: args.id }, payload: { id: args.id, reason: args.reason ?? null } };
  }
  if (commandName === "update_candidate") {
    return {
      command: { type: "update-candidate", id: args.id },
      payload: { id: args.id, input: args.input ?? {} },
    };
  }
  if (commandName === "gc_candidates") {
    return { command: { type: "gc-candidates" }, payload: {} };
  }
  if (commandName === "sync_project") {
    return { command: { type: "compile-project", dry_run: false }, payload: { dry_run: false } };
  }
  if (commandName === "import_artifact_drifts") {
    return { command: { type: "import-artifact-drifts" }, payload: {} };
  }
  if (commandName === "import_artifact_drift_path") {
    return { command: { type: "import-artifact-drifts" }, payload: { artifact_path: args.artifactPath } };
  }
  if (commandName === "keep_artifact_drifts") {
    return { command: { type: "keep-artifact-drifts" }, payload: {} };
  }
  if (commandName === "keep_artifact_drift_path") {
    return { command: { type: "keep-artifact-drifts" }, payload: { artifact_path: args.artifactPath } };
  }
  if (commandName === "discard_artifact_drifts") {
    return { command: { type: "discard-artifact-drifts" }, payload: {} };
  }
  if (commandName === "discard_artifact_drift_path") {
    return { command: { type: "discard-artifact-drifts" }, payload: { artifact_path: args.artifactPath } };
  }
  if (commandName === "save_custom_provider_config") {
    return { command: { type: "configure-provider" }, payload: { input: args.input ?? {} } };
  }
  if (commandName === "import_project") {
    const scanHome = Boolean(args.scanHome);
    return { command: { type: "import-project", scan_home: scanHome }, payload: { scan_home: scanHome } };
  }
  if (commandName === "set_memory_card_targets") {
    return {
      command: { type: "assign-memory-card", id: args.id, targets: args.targets ?? [] },
      payload: { id: args.id, targets: args.targets ?? [] },
    };
  }
  if (commandName === "delete_memory_card") {
    return {
      command: { type: "delete-memory-card", id: args.id },
      payload: { id: args.id },
    };
  }
  if (commandName === "update_memory_card") {
    return {
      command: { type: "update-memory-card", id: args.id },
      payload: { id: args.id, input: args.input ?? {} },
    };
  }
  if (commandName === "clear_memory_card_targets") {
    return {
      command: { type: "clear-memory-card-targets", agent: args.agent ?? null },
      payload: { agent: args.agent ?? null },
    };
  }
  if (commandName === "clear_project_history") {
    return { command: { type: "clear-project-history" }, payload: {} };
  }
  if (commandName === "promote_memory_card_to_global") {
    return { command: { type: "promote-memory-card-to-global", id: args.id }, payload: { id: args.id } };
  }
  if (commandName === "install_global_memory_card_to_project") {
    return {
      command: { type: "install-global-memory-card", id: args.id, targets: args.targets ?? [] },
      payload: { id: args.id, targets: args.targets ?? [] },
    };
  }
  if (commandName === "install_catalog_package") {
    return {
      command: { type: "install-catalog-package", id: args.packageId, targets: args.targets ?? [] },
      payload: { package_id: args.packageId, targets: args.targets ?? [] },
    };
  }
  if (commandName === "attach_memory_card_to_skill") {
    return {
      command: { type: "attach-memory-card-to-skill", memory_card_id: args.memoryCardId, skill_id: args.skillId },
      payload: { memory_card_id: args.memoryCardId, skill_id: args.skillId, fusion_mode: args.fusionMode ?? null },
    };
  }
  if (commandName === "fuse_memory_cards_to_draft") {
    const input = (args.input ?? {}) as Record<string, unknown>;
    return {
      command: { type: "fuse-memory-cards", ids: input.sources ?? [], engine: "local" },
      payload: { input },
    };
  }
  if (commandName === "evolve_project") {
    const dryRun = Boolean(args.dryRun);
    const engine = typeof args.engine === "string" ? args.engine : "local";
    return {
      command: { type: "evolve-project", dry_run: dryRun, engine },
      payload: { targets: args.targets ?? [], dry_run: dryRun, engine },
    };
  }
  return null;
}

export function buildKernelPlanForEditor(recordType: "draft" | "candidate" | "memory_card", recordId: string, input: Record<string, unknown>): KernelPlanRequest {
  if (recordType === "draft") {
    return {
      command: { type: "update-draft", id: recordId },
      payload: { id: recordId, input },
    };
  }
  if (recordType === "candidate") {
    return {
      command: { type: "update-candidate", id: recordId },
      payload: { id: recordId, input },
    };
  }
  return {
    command: { type: "update-memory-card", id: recordId },
    payload: { id: recordId, input },
  };
}

export function normalizePlanReviewResult(raw: unknown): PlanReviewResult {
  const value = (raw ?? {}) as Record<string, unknown>;
  const reviewRequired =
    typeof value.reviewRequired === "boolean"
      ? value.reviewRequired
      : typeof value.requires_human_review === "boolean"
        ? value.requires_human_review
        : false;

  return {
    risk: typeof value.risk === "string" ? value.risk : "unknown",
    disposition: typeof value.disposition === "string" ? value.disposition : "reject",
    reason: typeof value.reason === "string" ? value.reason : "Kernel Policy 未返回原因。",
    reviewRequired,
    requires_human_review: reviewRequired,
    decisionToken: typeof value.decision_token === "string" ? value.decision_token : typeof value.decisionToken === "string" ? value.decisionToken : undefined,
  };
}

export function getPreviewPlanResult(kind: string): PlanReviewResult {
  if (kind === "constraint") {
    return {
      risk: "medium",
      disposition: "review-required",
      reason: "预览模式：修改硬约束需人工复核后保存。",
      reviewRequired: true,
      requires_human_review: true,
      decisionToken: "preview-token",
    };
  }
  return {
    risk: "low",
    disposition: "execute",
    reason: "预览模式：变更风险较低，可直接保存。",
    reviewRequired: false,
    requires_human_review: false,
    decisionToken: "preview-token",
  };
}

export function getPreviewActionMessage(actionKey: string) {
  if (actionKey.startsWith("批准") || actionKey.startsWith("删除")) {
    return "预览模式：已从草稿列表移除演示项，不会写入文件。";
  }

  if (actionKey.startsWith("安装") || actionKey === "同步" || actionKey === "整理历史" || actionKey === "导入 Drift") {
    return "预览模式：已模拟完成操作，不会写入文件。";
  }

  if (actionKey === "扫描" || actionKey === "刷新" || actionKey === "切换项目") {
    return "预览模式：正在使用静态示例数据，真实扫描需要通过 bun run app:dev 启动。";
  }

  return "预览模式：该操作已作为本地演示处理，不会写入文件。";
}

export function isTauriRuntimeUnavailable(error: unknown) {
  const rawMessage = error instanceof Error ? error.message : String(error);
  const normalized = rawMessage.toLowerCase();

  return (
    normalized.includes("__tauri") ||
    normalized.includes("tauri") ||
    normalized.includes("get_app_state") ||
    normalized.includes("reading 'invoke'") ||
    normalized.includes('reading "invoke"')
  );
}
