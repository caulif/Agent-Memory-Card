import { RefreshCw } from "lucide-react";
import type { ProjectAction } from "../../types/domain";
import type { ArtifactPreviewSummary } from "../../utils/review-workbench";
import { ArtifactDriftGroup } from "./ArtifactDriftGroup";

export function ArtifactPreviewStrip({
  preview,
  disabled,
  pendingAction,
  onAction,
}: {
  preview: ArtifactPreviewSummary;
  disabled: boolean;
  pendingAction: string;
  onAction: ProjectAction;
}) {
  return (
    <div className={`artifact-preview-strip ${preview.status}`}>
      <div className="artifact-preview-copy">
        <span>Artifact Preview</span>
        <strong>{preview.headline}</strong>
        <small>{preview.detail}</small>
      </div>
      <div className="artifact-preview-list">
        {(preview.recoveryActions ?? (preview.recoveryAction ? [preview.recoveryAction] : [])).map((action) => (
          <button
            key={action.actionKey}
            className={`artifact-preview-action ${action.tone === "destructive" ? "danger" : ""}`}
            disabled={disabled || pendingAction === action.actionKey}
            title={action.description}
            onClick={() =>
              {
                if (action.confirmMessage && !window.confirm(action.confirmMessage)) return;
                void onAction(
                  action.actionKey,
                  action.doneMessage,
                  action.command,
                );
              }
            }
          >
            <RefreshCw size={13} />
            {action.label}
          </button>
        ))}
        {preview.targets.slice(0, 3).map((target) => (
          <span key={`${target.kind}:${target.path}`} title={target.diffPreview?.join("\n") || target.path}>
            {target.status ? `${target.status} · ` : ""}
            {target.kind === "skill" ? "Skill" : "File"} · {target.path}
            {target.diffPreview?.[0] ? <small>{target.diffPreview[0]}</small> : null}
          </span>
        ))}
        {preview.warnings.slice(0, 2).map((warning) => (
          <span className="warning" key={warning}>
            {warning}
          </span>
        ))}
        {preview.actions.length + preview.warnings.length > 5 ? (
          <span>还有 {preview.actions.length + preview.warnings.length - 5} 项</span>
        ) : null}
      </div>
      <ArtifactDriftGroup
        targets={preview.blockingTargets}
        disabled={disabled}
        pendingAction={pendingAction}
        onAction={onAction}
      />
      {preview.lastSync ? (
        <div className="artifact-sync-checkpoint">
          <div>
            <span>Last Sync</span>
            <strong>{preview.lastSync.artifact_count} artifacts · {preview.lastSync.memory_card_ids.length} Memory Cards</strong>
            <small>{formatSyncTime(preview.lastSync.created_at)}</small>
          </div>
          <details>
            <summary>Rollback guidance</summary>
            <ul>
              {preview.lastSync.rollback_instructions.map((step) => (
                <li key={step}>{step}</li>
              ))}
            </ul>
          </details>
        </div>
      ) : null}
      {preview.targets.some((target) => target.diffLines?.length) ? (
        <div className="artifact-preview-diffs">
          {preview.targets
            .filter((target) => target.diffLines?.length)
            .slice(0, 2)
            .map((target) => (
              <details key={`diff:${target.path}`}>
                <summary>
                  查看差异 · {target.path}
                  {target.diffTruncated ? "（已截断）" : ""}
                </summary>
                <pre>{(target.diffLines ?? []).join("\n")}</pre>
              </details>
            ))}
        </div>
      ) : null}
    </div>
  );
}

function formatSyncTime(timestamp: string) {
  const date = new Date(timestamp);
  if (Number.isNaN(date.getTime())) return timestamp;
  return date.toLocaleString();
}
