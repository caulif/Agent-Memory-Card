import { Loader2 } from "lucide-react";
import type { MemoryGovernanceBatchAction } from "../../utils/memory-governance";

export function MemoryGovernanceBatchBar({
  actions,
  disabled,
  pendingAction,
  batchAction,
  onRunBatch,
}: {
  actions: MemoryGovernanceBatchAction[];
  disabled: boolean;
  pendingAction: string;
  batchAction: string | null;
  onRunBatch: (action: MemoryGovernanceBatchAction) => Promise<void>;
}) {
  if (actions.length === 0) return null;

  return (
    <section className="governance-batch" aria-label="批量治理动作">
      {actions.map((action) => {
        const running = batchAction === action.actionKey || pendingAction.startsWith(action.label);
        return (
          <button
            key={action.actionKey}
            className="memory-governance-action"
            disabled={disabled || Boolean(batchAction) || running}
            title={action.description}
            onClick={() => void onRunBatch(action)}
          >
            {running ? <Loader2 className="spin" size={15} /> : null}
            {action.label} · {action.steps.length}
          </button>
        );
      })}
    </section>
  );
}
