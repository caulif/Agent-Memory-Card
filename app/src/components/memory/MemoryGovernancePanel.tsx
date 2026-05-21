import React from "react";
import type { ProjectAction } from "../../types/domain";
import type { MemoryCardGovernance } from "../../utils/memory-governance";

export function MemoryGovernancePanel({
  governance,
  disabled,
  pendingAction,
  onAction,
}: {
  governance: MemoryCardGovernance;
  disabled: boolean;
  pendingAction: string;
  onAction: ProjectAction;
}) {
  return (
    <div className={`memory-governance ${governance.status === "needs-review" ? "needs-review" : ""}`}>
      <div className="memory-governance-head">
        <strong>{governance.status === "ready" ? "治理就绪" : "需要复核"}</strong>
        <span>{governance.sourceLabel}</span>
        <span>{governance.assignedTargets.length > 0 ? governance.assignedTargets.join(", ") : "未分配"}</span>
        <span>{governance.mergeCount} 次合并</span>
        {governance.lifecycle !== "active" ? (
          <span>{governance.lifecycle === "dormant" ? "休眠" : "过期"}</span>
        ) : null}
      </div>
      {governance.warnings.length > 0 ? (
        <div className="evidence-warnings">
          {governance.warnings.map((warning) => (
            <span key={warning}>{warning}</span>
          ))}
        </div>
      ) : null}
      {governance.lineage.length > 0 ? (
        <details className="memory-lineage">
          <summary>来源链路</summary>
          <dl>
            {governance.lineage.map((item) => (
              <React.Fragment key={`${item.label}:${item.value}`}>
                <dt>{item.label}</dt>
                <dd>{item.value}</dd>
              </React.Fragment>
            ))}
          </dl>
        </details>
      ) : null}
      {governance.conflictDetails.length > 0 ? (
        <details className="memory-lineage">
          <summary>冲突解释</summary>
          <dl>
            {governance.conflictDetails.map((item) => (
              <React.Fragment key={item.id}>
                <dt>{item.similarityLabel}</dt>
                <dd>
                  {item.title} · {item.id}
                </dd>
              </React.Fragment>
            ))}
          </dl>
        </details>
      ) : null}
      {governance.mergeDraftAction ? (
        <button
          className="memory-governance-action"
          disabled={disabled || pendingAction === governance.mergeDraftAction.actionKey}
          onClick={() =>
            void onAction(
              governance.mergeDraftAction!.actionKey,
              governance.mergeDraftAction!.doneMessage,
              governance.mergeDraftAction!.command,
              { input: governance.mergeDraftAction!.input },
            )
          }
        >
          {governance.mergeDraftAction.label}
        </button>
      ) : null}
      {governance.lifecycleAction ? (
        <button
          className="memory-governance-action"
          disabled={disabled || pendingAction === governance.lifecycleAction.actionKey}
          onClick={() =>
            void onAction(
              governance.lifecycleAction!.actionKey,
              governance.lifecycleAction!.doneMessage,
              governance.lifecycleAction!.command,
              {
                id: governance.lifecycleAction!.input.id,
                input: { tags: governance.lifecycleAction!.input.tags },
              },
            )
          }
        >
          {governance.lifecycleAction.label}
        </button>
      ) : null}
      {governance.expireAction ? (
        <button
          className="memory-governance-action"
          disabled={disabled || pendingAction === governance.expireAction.actionKey}
          onClick={() =>
            void onAction(
              governance.expireAction!.actionKey,
              governance.expireAction!.doneMessage,
              governance.expireAction!.command,
              {
                id: governance.expireAction!.input.id,
                input: { tags: governance.expireAction!.input.tags },
              },
            )
          }
        >
          {governance.expireAction.label}
        </button>
      ) : null}
    </div>
  );
}
