import React from "react";
import { ArrowUp, Bell, Check, Circle, Layers3, Pencil, RefreshCw, ShieldAlert, X } from "lucide-react";
import { ActionButton, EmptyState, Panel } from "../common";
import {
  buildEditFormFromDraft,
  type CandidateFilter,
  type CandidateRecord,
  describeDraftForReview,
  filterCandidatesForInbox,
  sortCandidatesForInbox,
  translateKind,
  translateScope,
  type ExtractionMetadata,
  type ProjectAction,
  type ProjectCandidateInbox,
  type ProjectReviewInbox,
  type ProjectSnapshot,
  type PanelPageProps,
} from "../../ui-helpers";
import { RecordEditor } from "./RecordEditor";

export function Drafts({
  snapshot,
  candidates,
  inbox,
  pendingAction,
  disabled,
  onAction,
  onBatchCandidateAction,
  previewMode,
  projectPath,
  synthesisEngine,
  onSynthesisEngineChange,
  onRefresh,
}: PanelPageProps & {
  candidates: ProjectCandidateInbox | null;
  inbox: ProjectReviewInbox | null;
  onBatchCandidateAction: (
    actionKey: string,
    doneMessage: string,
    command: "promote_candidate" | "hide_candidate" | "reject_candidate",
    ids: string[],
    extraArgs?: Record<string, unknown>,
  ) => Promise<void>;
  previewMode: boolean;
  projectPath: string;
  synthesisEngine: "claude-code" | "codex" | "local";
  onSynthesisEngineChange: (engine: "claude-code" | "codex" | "local") => void;
  onRefresh: () => void;
}) {
  const [candidateFilter, setCandidateFilter] = React.useState<CandidateFilter>("high");
  const [selectedCandidateIds, setSelectedCandidateIds] = React.useState<string[]>([]);
  const [batchRejectReason, setBatchRejectReason] = React.useState("");
  const allCandidates = React.useMemo(() => sortCandidatesForInbox(candidates?.candidates ?? []), [candidates]);
  const visibleCandidates = React.useMemo(
    () => filterCandidatesForInbox(allCandidates, candidateFilter),
    [allCandidates, candidateFilter],
  );
  const lowConfidenceCount = allCandidates.filter((candidate) => (candidate.confidence ?? 0) < 0.72).length;
  const sourceDrafts = inbox?.drafts ?? snapshot?.drafts ?? [];
  const drafts = visibleDrafts(sourceDrafts);
  const hiddenCount = Math.max(sourceDrafts.length - drafts.length, 0);
  const [editingId, setEditingId] = React.useState<string | null>(null);

  function startEdit(draft: (typeof drafts)[number]) {
    setEditingId(draft.id);
  }

  function cancelEdit() {
    setEditingId(null);
  }

  async function handleSaved() {
    setEditingId(null);
    onRefresh();
  }

  function toggleCandidateSelected(id: string) {
    setSelectedCandidateIds((current) => current.includes(id) ? current.filter((item) => item !== id) : [id, ...current]);
  }

  async function runBatch(command: "promote_candidate" | "hide_candidate" | "reject_candidate") {
    if (selectedCandidateIds.length === 0) return;
    const label = command === "promote_candidate" ? "批量批准" : command === "hide_candidate" ? "批量隐藏" : "批量拒绝";
    const reason = command === "reject_candidate" ? batchRejectReason.trim() || "用户批量拒绝候选。" : undefined;
    await onBatchCandidateAction(label, `${label}完成`, command, selectedCandidateIds, reason ? { reason } : {});
    setSelectedCandidateIds([]);
  }

  return (
    <div className="list">
      <Panel title="Inbox 工作台" subtitle="系统建议就是待审草稿；你批准后会直接进入 Skilllet，再分配并编译到目标智能体。" icon={Bell}>
        <div className="engine-row" aria-label="整理引擎">
          <span>整理引擎</span>
          <button className={synthesisEngine === "local" ? "active" : ""} onClick={() => onSynthesisEngineChange("local")}>
            本地极速
          </button>
          <button className={synthesisEngine === "claude-code" ? "active" : ""} onClick={() => onSynthesisEngineChange("claude-code")}>
            Claude Code
          </button>
          <button className={synthesisEngine === "codex" ? "active" : ""} onClick={() => onSynthesisEngineChange("codex")}>
            Codex
          </button>
        </div>
        <div className="command-strip">
          <ActionButton
            className="hero-button"
            icon={RefreshCw}
            label="提炼候选"
            busyLabel="正在整理"
            busy={pendingAction === "整理历史"}
            disabled={disabled}
            onClick={() =>
              onAction("整理历史", "已开始整理候选", "evolve_project", {
                targets: ["codex", "claude-code"],
                dryRun: false,
                engine: synthesisEngine,
              })
            }
          />
          <ActionButton
            className="secondary-action"
            icon={Layers3}
            label="编译产物"
            busyLabel="正在同步"
            busy={pendingAction === "同步"}
            disabled={disabled}
            onClick={() => onAction("同步", "已同步生成产物", "sync_project")}
          />
        </div>
      </Panel>

      {allCandidates.length > 0 ? (
        <Panel title="系统建议" subtitle="本地高价值过滤和少量 AI 精炼后的待审草稿；批准后直接成为 Skilllet。" icon={Bell}>
          <div className="filter-bar" aria-label="候选筛选">
            {[
              ["high", "高置信"],
              ["all", "全部"],
              ["low", `低置信 ${lowConfidenceCount}`],
              ["rule", "规则"],
              ["preference", "偏好"],
              ["procedure", "流程"],
              ["constraint", "约束"],
            ].map(([id, label]) => (
              <button
                key={id}
                className={candidateFilter === id ? "active" : ""}
                onClick={() => setCandidateFilter(id as CandidateFilter)}
              >
                {label}
              </button>
            ))}
          </div>
          <div className="batch-row">
            <span>{selectedCandidateIds.length > 0 ? `已选择 ${selectedCandidateIds.length} 条` : "选择候选后可批量处理"}</span>
            <input
              type="text"
              value={batchRejectReason}
              onChange={(event) => setBatchRejectReason(event.target.value)}
              placeholder="批量拒绝原因"
            />
            <button className="secondary-action" disabled={disabled || selectedCandidateIds.length === 0} onClick={() => void runBatch("promote_candidate")}>
              批量批准
            </button>
            <button className="secondary-action" disabled={disabled || selectedCandidateIds.length === 0} onClick={() => void runBatch("hide_candidate")}>
              批量隐藏
            </button>
            <button className="danger-action" disabled={disabled || selectedCandidateIds.length === 0} onClick={() => void runBatch("reject_candidate")}>
              批量拒绝
            </button>
          </div>
          <div className="list compact-list">
            {visibleCandidates.map((candidate) => (
              <CandidateCard
                key={candidate.id}
                candidate={candidate}
                selected={selectedCandidateIds.includes(candidate.id)}
                onSelectedChange={() => toggleCandidateSelected(candidate.id)}
                disabled={disabled}
                pendingAction={pendingAction}
                onAction={onAction}
              />
            ))}
          </div>
          {visibleCandidates.length === 0 ? <p className="empty">当前筛选下没有候选。</p> : null}
        </Panel>
      ) : null}
      {drafts.length === 0 ? (
        <EmptyState title="暂无高价值草稿" description="点击“提炼高价值 Skilllet”后，稳定偏好、硬约束、流程和可复用技能补充会出现在这里等待审核。" />
      ) : (
        <>
          {hiddenCount > 0 ? <p className="filter-note">已隐藏 {hiddenCount} 条低置信度或过碎草稿，避免审阅列表失控。</p> : null}
          {drafts.map((draft) => (
            <React.Fragment key={draft.id}>
            <article className="record">
              <div className="record-main">
                <span className="tag">
                  {translateKind(draft.kind)} · {translateScope(draft.scope)}
                  {draft.confidence ? ` · 置信度 ${Math.round(draft.confidence * 100)}%` : ""}
                </span>
                <p className="draft-brief">
                  <span className="brief-label">审阅摘要</span>
                  {describeDraftForReview(draft)}
                </p>
                <h3>{draft.title}</h3>
                <p>{draft.body}</p>
                {draft.tags && draft.tags.length > 0 ? (
                  <div className="tag-row">
                    {draft.tags.map((t) => (
                      <span key={t}>{t}</span>
                    ))}
                  </div>
                ) : null}
                <small>{draft.reason ?? draft.evidence}</small>
                {draft.extraction?.reason ? (
                  <p className="record-reason">
                    {draft.extraction.matched_signal ? `${draft.extraction.matched_signal} · ` : ""}
                    {draft.extraction.origin ? `${draft.extraction.origin} · ` : ""}
                    {draft.extraction.reason}
                  </p>
                ) : null}
                {draft.extraction?.classification ? (
                  <div className="tag-row classification-chips">
                    {draft.extraction.classification.signal ? (
                      <span className="chip chip-signal">{draft.extraction.classification.signal}</span>
                    ) : null}
                    {draft.extraction.classification.artifact_kind ? (
                      <span className="chip chip-artifact">{draft.extraction.classification.artifact_kind}</span>
                    ) : null}
                    {draft.extraction.classification.hardness ? (
                      <span className="chip chip-hardness">{draft.extraction.classification.hardness}</span>
                    ) : null}
                    {draft.extraction.classification.activation ? (
                      <span className="chip chip-activation">{draft.extraction.classification.activation}</span>
                    ) : null}
                  </div>
                ) : null}
              </div>
              <div className="record-actions">
                <button
                  className="secondary-action"
                  disabled={disabled}
                  onClick={() => startEdit(draft)}
                >
                  <Pencil size={15} />
                  编辑
                </button>
                <ActionButton
                  icon={Check}
                  label="批准"
                  busyLabel="批准中"
                  busy={pendingAction === `批准-${draft.id}`}
                  disabled={disabled}
                  onClick={() => onAction(`批准-${draft.id}`, "已批准草稿", "approve_draft", { id: draft.id })}
                />
                <ActionButton
                  className="danger-action"
                  icon={X}
                  label="删除"
                  busyLabel="删除中"
                  busy={pendingAction === `删除-${draft.id}`}
                  disabled={disabled}
                  onClick={() => onAction(`删除-${draft.id}`, "已删除草稿", "reject_draft", { id: draft.id })}
                />
              </div>
            </article>
            {editingId === draft.id ? (
              <RecordEditor
                recordType="draft"
                initialForm={buildEditFormFromDraft(draft)}
                extraction={draft.extraction}
                previewMode={previewMode}
                projectPath={projectPath}
                recordId={draft.id}
                onSaved={handleSaved}
                onCancel={cancelEdit}
              />
            ) : null}
            </React.Fragment>
          ))}
        </>
      )}
    </div>
  );
}

function CandidateCard({
  candidate,
  selected,
  onSelectedChange,
  disabled,
  pendingAction,
  onAction,
}: {
  candidate: CandidateRecord;
  selected: boolean;
  onSelectedChange: () => void;
  disabled: boolean;
  pendingAction: string;
  onAction: ProjectAction;
}) {
  const confidence = candidate.confidence ? ` · 置信度 ${Math.round(candidate.confidence * 100)}%` : "";
  const candidateTags = candidate.tags ?? [];
  const candidateBrief =
    candidate.brief?.trim() ||
    `这条系统建议沉淀了“${candidate.title}”，批准后会直接进入 Skilllet。`;
  const sourceText = [
    candidate.reason ? `原因：${candidate.reason}` : null,
    candidate.evidence ? `证据：${candidate.evidence}` : null,
    candidate.matched_template ? `模板：${candidate.matched_template}` : null,
  ]
    .filter(Boolean)
    .join(" · ");
  const [rejectReason, setRejectReason] = React.useState("");
  return (
    <article className="record candidate-record">
      <div className="record-main">
        <label className="select-row">
          <input type="checkbox" checked={selected} onChange={onSelectedChange} disabled={disabled} />
          <span>选择候选</span>
        </label>
        <span className="tag">
          候选 · {translateKind(candidate.kind)} · {translateScope(candidate.scope)}
          {confidence}
        </span>
        <h3>{candidate.title}</h3>
        <p className="draft-brief">
          <span className="brief-label">中文概述</span>
          {candidateBrief}
        </p>
        {candidateTags.length > 0 ? (
          <div className="tag-row" aria-label="候选标签">
            {candidateTags.map((tag) => (
              <span key={tag}>{tag}</span>
            ))}
          </div>
        ) : null}
        <p>{candidate.body}</p>
        <small>{sourceText}</small>
        {candidate.extraction?.reason ? (
          <p className="record-reason">
            {candidate.extraction.matched_signal ? `${candidate.extraction.matched_signal} · ` : ""}
            {candidate.extraction.origin ? `${candidate.extraction.origin} · ` : ""}
            {candidate.extraction.reason}
          </p>
        ) : null}
      </div>
      <div className="record-actions">
        <input
          className="inline-input"
          type="text"
          value={rejectReason}
          onChange={(event) => setRejectReason(event.target.value)}
          placeholder="拒绝原因"
          disabled={disabled}
        />
        <ActionButton
          icon={ArrowUp}
          label="批准"
          busyLabel="批准中"
          busy={pendingAction === `批准-${candidate.id}`}
          disabled={disabled}
          onClick={() =>
            onAction(`批准-${candidate.id}`, "已批准为 Skilllet", "promote_candidate", { id: candidate.id })
          }
        />
        <ActionButton
          className="secondary-action"
          icon={Circle}
          label="隐藏"
          busyLabel="隐藏中"
          busy={pendingAction === `隐藏-${candidate.id}`}
          disabled={disabled}
          onClick={() => onAction(`隐藏-${candidate.id}`, "已隐藏候选", "hide_candidate", { id: candidate.id })}
        />
        <ActionButton
          className="danger-action"
          icon={X}
          label="拒绝"
          busyLabel="拒绝中"
          busy={pendingAction === `拒绝-${candidate.id}`}
          disabled={disabled}
          onClick={() =>
            onAction(`拒绝-${candidate.id}`, "已拒绝候选", "reject_candidate", {
              id: candidate.id,
              reason: rejectReason.trim() || "用户从 Candidate Review 拒绝。",
            })
          }
        />
      </div>
    </article>
  );
}


function visibleDrafts(drafts: ProjectSnapshot["drafts"]) {
  return [...drafts]
    .filter((draft) => {
      if (draft.matched_template) return true;
      if ((draft.confidence ?? 0) >= 0.78) return true;
      return ["constraint", "preference", "procedure"].includes(draft.kind) && draft.body.length <= 240;
    })
    .sort((a, b) => (b.confidence ?? 0) - (a.confidence ?? 0))
    .slice(0, 60);
}
