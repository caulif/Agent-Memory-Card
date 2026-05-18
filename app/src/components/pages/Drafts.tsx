import React from "react";
import { ArrowUp, Check, Circle, Pencil, RefreshCw, X } from "lucide-react";
import { ActionButton, EmptyState, Panel } from "../common";
import {
  buildEditFormFromDraft,
  buildEditFormFromCandidate,
  buildReviewClosureState,
  type CandidateRecord,
  describeDraftForReview,
  routeLabel,
  summarizeArtifactPreview,
  summarizeCandidateEvidence,
  summarizeDraftEvidence,
  sortCandidatesForInbox,
  translateKind,
  translateScope,
  type ProjectAction,
  type ProjectAssignmentView,
  type ProjectCandidateInbox,
  type ProjectMemoryCardLibrary,
  type ProjectQualityView,
  type ProjectReviewInbox,
  type ProjectSnapshot,
  type PanelPageProps,
} from "../../ui-helpers";
import { ArtifactPreviewStrip } from "../review/ArtifactPreviewStrip";
import { RecordEditor } from "./RecordEditor";

export function Drafts({
  snapshot,
  candidates,
  inbox,
  assignment,
  library,
  quality,
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
  assignment: ProjectAssignmentView | null;
  library: ProjectMemoryCardLibrary | null;
  quality: ProjectQualityView | null;
  onBatchCandidateAction: (
    actionKey: string,
    doneMessage: string,
    command: "promote_candidate" | "hide_candidate" | "reject_candidate",
    ids: string[],
    extraArgs?: Record<string, unknown>,
  ) => Promise<void>;
  previewMode: boolean;
  projectPath: string;
  synthesisEngine: "claude-code" | "codex" | "local" | "llm";
  onSynthesisEngineChange: (engine: "claude-code" | "codex" | "local" | "llm") => void;
  onRefresh: () => void;
}) {
  const [selectedCandidateIds, setSelectedCandidateIds] = React.useState<string[]>([]);
  const [batchRejectReason, setBatchRejectReason] = React.useState("");
  const allCandidates = React.useMemo(() => sortCandidatesForInbox(candidates?.candidates ?? []), [candidates]);
  const visibleCandidates = allCandidates;
  const lowConfidenceCount = allCandidates.filter((candidate) => (candidate.confidence ?? 0) < 0.72).length;
  const sourceDrafts = inbox?.drafts ?? snapshot?.drafts ?? [];
  const drafts = visibleDrafts(sourceDrafts);
  const hiddenCount = Math.max(sourceDrafts.length - drafts.length, 0);
  const memoryCards = library?.memory_cards ?? snapshot?.memory_cards ?? [];
  const closureState = buildReviewClosureState({
    candidates: allCandidates,
    drafts,
    memoryCards,
    assignment,
    quality,
  });
  const artifactPreview = summarizeArtifactPreview(quality);
  const [editingId, setEditingId] = React.useState<string | null>(null);
  const [selectedCandidateId, setSelectedCandidateId] = React.useState<string | null>(null);

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

  const selectedCandidate = selectedCandidateId
    ? allCandidates.find((candidate) => candidate.id === selectedCandidateId) ?? null
    : null;

  return (
    <div className="stack">
      {/* ===== 顶部指标行 ===== */}
      <Panel title="Draft Review" subtitle="Inbox 工作台 — 系统建议就是待审草稿；批准后直接进入 Memory Card，再分配并编译到目标智能体。">
        <div className="engine-command-row">
          <div className="engine-row" aria-label="整理引擎">
            <button className={synthesisEngine === "claude-code" ? "active" : ""} onClick={() => onSynthesisEngineChange("claude-code")}>
              Claude Code
            </button>
            <button className={synthesisEngine === "codex" ? "active" : ""} onClick={() => onSynthesisEngineChange("codex")}>
              Codex
            </button>
            <button className={synthesisEngine === "llm" ? "active" : ""} onClick={() => onSynthesisEngineChange("llm")}>
              LLM
            </button>
          </div>
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
        </div>
        <div className="review-closure-strip" aria-label="审查闭环状态">
          <div className="review-closure-next">
            <span>下一步</span>
            <strong>{closureState.nextStep.label}</strong>
            <small>{closureState.nextStep.detail}</small>
          </div>
          <div className="review-closure-metrics">
            <span>{closureState.candidateCount} 候选</span>
            <span>{closureState.draftCount} 草稿</span>
            <span>{closureState.unassignedCount} 未分配</span>
            <span>{closureState.driftWarningCount} Drift</span>
            <span>{closureState.buildActionCount} 写入动作</span>
          </div>
        </div>
        <ArtifactPreviewStrip
          preview={artifactPreview}
          disabled={disabled}
          pendingAction={pendingAction}
          onAction={onAction}
        />
      </Panel>

      {/* ===== 三栏布局：候选队列 / 详情 / 质量状态 ===== */}
      <div className="drafts-grid">
        {/* 左栏：候选队列 */}
        <div className="drafts-column">
          <div className="drafts-column-head">
            <h3>候选队列</h3>
          </div>

          {visibleCandidates.length === 0 ? (
            <p className="empty" style={{ padding: "20px 0", textAlign: "center" }}>
              暂无候选。
            </p>
          ) : (
            <div className="candidate-queue">
              {visibleCandidates.map((candidate) => {
                const isSelected = selectedCandidateIds.includes(candidate.id);
                const isActive = selectedCandidateId === candidate.id;
                return (
                <article
                  key={candidate.id}
                  className={`candidate-queue-item ${isActive ? "active" : ""} ${isSelected ? "selected" : ""}`}
                >
                  <div className="candidate-queue-head">
                    <button
                      className="candidate-open-target"
                      aria-pressed={isActive}
                      onClick={() => setSelectedCandidateId(candidate.id === selectedCandidateId ? null : candidate.id)}
                    >
                      <strong>{candidate.title}</strong>
                    </button>
                    <div className="candidate-queue-head-right">
                      {candidate.confidence != null ? (
                        <span className="confidence-badge">
                          {Math.round(candidate.confidence * 100)}%
                        </span>
                      ) : null}
                      <button
                        className={`candidate-select-toggle ${isSelected ? "checked" : ""}`}
                        aria-label={isSelected ? "取消选择" : "选择候选"}
                        aria-pressed={isSelected}
                        onClick={(event) => { event.stopPropagation(); toggleCandidateSelected(candidate.id); }}
                      >
                        {isSelected ? <Check size={12} /> : <Circle size={12} />}
                      </button>
                    </div>
                  </div>
                  <button
                    className="candidate-open-copy"
                    onClick={() => setSelectedCandidateId(candidate.id === selectedCandidateId ? null : candidate.id)}
                  >
                    {candidate.brief ?? candidate.body.slice(0, 80)}
                  </button>
                </article>
                );
              })}
            </div>
          )}
        </div>

        {/* 中栏：选中候选详情/审阅 */}
        <div className="drafts-column drafts-detail">
          {selectedCandidate ? (
            <>
              <div className="drafts-column-head">
                <h3>审阅详情</h3>
              </div>
              <CandidateDetail
                candidate={selectedCandidate}
                disabled={disabled}
                pendingAction={pendingAction}
                onAction={onAction}
                previewMode={previewMode}
                projectPath={projectPath}
                onRefresh={onRefresh}
              />
            </>
          ) : (
            <div className="drafts-column-head">
              <h3>审阅详情</h3>
              <EmptyState title="选择一个候选" description="从左侧候选队列中选择一条建议来查看详情和操作。" />
            </div>
          )}
        </div>

        {/* 右栏：质量状态 */}
        <div className="drafts-column">
          <div className="drafts-column-head">
            <h3>质量状态</h3>
          </div>
          <div className="drafts-quality-card">
            <div className="quality-ring">
              <svg width="72" height="72" viewBox="0 0 72 72">
                <circle cx="36" cy="36" r="30" fill="none" stroke="var(--color-border)" strokeWidth="5" />
                <circle
                  cx="36"
                  cy="36"
                  r="30"
                  fill="none"
                  stroke="var(--color-accent)"
                  strokeWidth="5"
                  strokeDasharray={`${(allCandidates.length > 0 ? drafts.length / Math.max(allCandidates.length, 1) : 0) * 188.5} 188.5`}
                  strokeLinecap="round"
                  transform="rotate(-90 36 36)"
                />
              </svg>
              <div className="quality-ring-text">
                <strong>{drafts.length}</strong>
                <span>草稿</span>
              </div>
            </div>
            <div className="quality-bars">
              <div className="quality-bar-row">
                <span>候选</span>
                <div className="quality-bar-track">
                  <div className="quality-bar-fill" style={{ width: `${Math.min(100, allCandidates.length * 20)}%` }} />
                </div>
                <strong>{allCandidates.length}</strong>
              </div>
              <div className="quality-bar-row">
                <span>草稿</span>
                <div className="quality-bar-track">
                  <div className="quality-bar-fill" style={{ width: `${Math.min(100, drafts.length * 25)}%`, background: "var(--color-accent)" }} />
                </div>
                <strong>{drafts.length}</strong>
              </div>
              <div className="quality-bar-row">
                <span>低置信</span>
                <div className="quality-bar-track">
                  <div className="quality-bar-fill" style={{ width: `${Math.min(100, lowConfidenceCount * 25)}%`, background: "#d4a23b" }} />
                </div>
                <strong>{lowConfidenceCount}</strong>
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* 草稿列表 */}
      {drafts.length > 0 ? (
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
                  <div className="classification-chips">
                    {draft.extraction.classification.signal ? (
                      <span className="chip chip-signal">{draft.extraction.classification.signal}</span>
                    ) : null}
                    {draft.extraction.classification.artifact_kind ? (
                      <span className="chip chip-artifact">
                        {routeLabel(draft.extraction.classification.artifact_kind)}
                      </span>
                    ) : null}
                    {draft.extraction.classification.hardness ? (
                      <span className="chip chip-hardness">{draft.extraction.classification.hardness}</span>
                    ) : null}
                    {draft.extraction.classification.activation ? (
                      <span className="chip chip-activation">{draft.extraction.classification.activation}</span>
                    ) : null}
                  </div>
                ) : null}
                {draft.extraction?.suggested_action ? (
                  <div className="classification-chips">
                    <span className="chip chip-artifact">
                      {routeLabel(draft.extraction.suggested_action.route)}
                    </span>
                    <span className="chip chip-activation">
                      {compileLabel(draft.extraction.suggested_action.compile_enabled)}
                    </span>
                    <span className="chip chip-signal">
                      {actionLabel(draft.extraction.suggested_action.action)}
                    </span>
                  </div>
                ) : null}
                <EvidencePanel summary={summarizeDraftEvidence(draft)} />
              </div>
              <div className="record-actions">
                <button
                  className="secondary-action"
                  disabled={disabled}
                  onClick={() => startEdit(draft)}
                >
                  <Pencil size={14} />
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
      ) : allCandidates.length === 0 ? (
        <EmptyState title="暂无高价值草稿" description={'点击「提炼候选」后，稳定偏好、硬约束、流程和可复用技能补充会出现在这里等待审核。'} />
      ) : null}

      {/* 底部批量操作栏 */}
      {selectedCandidateIds.length > 0 ? (
        <div className="batch-bar-sticky">
          <div className="batch-row" style={{ margin: 0 }}>
            <span>已选择 {selectedCandidateIds.length} 条候选</span>
            <input
              type="text"
              value={batchRejectReason}
              onChange={(event) => setBatchRejectReason(event.target.value)}
              placeholder="批量拒绝原因"
            />
            <button className="primary-action" disabled={disabled || selectedCandidateIds.length === 0} onClick={() => void runBatch("promote_candidate")}>
              批量批准
            </button>
            <button className="secondary-action" disabled={disabled || selectedCandidateIds.length === 0} onClick={() => void runBatch("hide_candidate")}>
              批量隐藏
            </button>
            <button className="danger-action" disabled={disabled || selectedCandidateIds.length === 0} onClick={() => void runBatch("reject_candidate")}>
              批量拒绝
            </button>
          </div>
        </div>
      ) : null}
    </div>
  );
}

/** 候选详情卡片 */
function CandidateDetail({
  candidate,
  disabled,
  pendingAction,
  onAction,
  previewMode,
  projectPath,
  onRefresh,
}: {
  candidate: CandidateRecord;
  disabled: boolean;
  pendingAction: string;
  onAction: ProjectAction;
  previewMode: boolean;
  projectPath: string;
  onRefresh: () => void;
}) {
  const confidence = candidate.confidence ? ` · 置信度 ${Math.round(candidate.confidence * 100)}%` : "";
  const evidenceSummary = summarizeCandidateEvidence(candidate);
  const candidateTags = candidate.tags ?? [];
  const candidateBrief =
    candidate.brief?.trim() ||
    `这条系统建议沉淀了"${candidate.title}"，批准后会直接进入 Memory Card。`;
  const suggestedAction = candidate.extraction?.suggested_action;
  const mergeTarget =
    suggestedAction?.action === "merge_into_existing"
      ? suggestedAction.target_record || suggestedAction.record_id
      : null;
  const sourceText = [
    candidate.reason ? `原因：${candidate.reason}` : null,
    candidate.evidence ? `证据：${candidate.evidence}` : null,
    candidate.matched_template ? `模板：${candidate.matched_template}` : null,
  ]
    .filter(Boolean)
    .join(" · ");
  const [rejectReason, setRejectReason] = React.useState("");
  const [editing, setEditing] = React.useState(false);

  async function handleSaved() {
    setEditing(false);
    onRefresh();
  }

  return (
    <div className="candidate-detail">
      <span className="tag">
        候选 · {translateKind(candidate.kind)} · {translateScope(candidate.scope)}
        {confidence}
      </span>
      <h3>{candidate.title}</h3>
      <p className="draft-brief">
        <span className="brief-label">概述</span>
        {candidateBrief}
      </p>
      {candidateTags.length > 0 ? (
        <div className="tag-row">
          {candidateTags.map((tag) => (
            <span key={tag}>{tag}</span>
          ))}
        </div>
      ) : null}
      {candidate.extraction?.classification || suggestedAction ? (
        <div className="classification-chips">
          {candidate.extraction?.classification?.artifact_kind ? (
            <span className="chip chip-artifact">{routeLabel(candidate.extraction.classification.artifact_kind)}</span>
          ) : null}
          {candidate.extraction?.classification?.hardness ? (
            <span className="chip chip-hardness">{candidate.extraction.classification.hardness}</span>
          ) : null}
          {suggestedAction ? (
            <span className="chip chip-activation">{compileLabel(suggestedAction.compile_enabled)}</span>
          ) : null}
          {suggestedAction ? (
            <span className="chip chip-signal">{actionLabel(suggestedAction.action)}</span>
          ) : null}
        </div>
      ) : null}
      <p>{candidate.body}</p>
      {mergeTarget ? (
        <p className="record-reason">
          合并建议 · 已存在相近 Memory Card：{mergeTarget}
          {suggestedAction?.similarity ? ` · 相似度 ${Math.round(suggestedAction.similarity * 100)}%` : ""}
        </p>
      ) : null}
      <small>{sourceText}</small>
      {candidate.extraction?.reason ? (
        <p className="record-reason">
          {candidate.extraction.matched_signal ? `${candidate.extraction.matched_signal} · ` : ""}
          {candidate.extraction.origin ? `${candidate.extraction.origin} · ` : ""}
          {candidate.extraction.reason}
        </p>
      ) : null}
      <EvidencePanel summary={evidenceSummary} />

      <div className="candidate-detail-actions">
        <input
          className="inline-input"
          type="text"
          value={rejectReason}
          onChange={(event) => setRejectReason(event.target.value)}
          placeholder="拒绝原因"
          disabled={disabled}
          style={{ flex: 1 }}
        />
        <ActionButton
          className="secondary-action"
          icon={Pencil}
          label="编辑"
          busyLabel="编辑中"
          busy={false}
          disabled={disabled}
          onClick={() => setEditing((current) => !current)}
        />
        <ActionButton
          icon={ArrowUp}
          label="批准"
          busyLabel="批准中"
          busy={pendingAction === `批准-${candidate.id}`}
          disabled={disabled}
          onClick={() =>
            onAction(`批准-${candidate.id}`, "已批准为 Memory Card", "promote_candidate", { id: candidate.id })
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
      {editing ? (
        <RecordEditor
          recordType="candidate"
          initialForm={buildEditFormFromCandidate(candidate)}
          extraction={candidate.extraction}
          previewMode={previewMode}
          projectPath={projectPath}
          recordId={candidate.id}
          onSaved={handleSaved}
          onCancel={() => setEditing(false)}
        />
      ) : null}
    </div>
  );
}

function EvidencePanel({ summary }: { summary: ReturnType<typeof summarizeCandidateEvidence> }) {
  return (
    <div className={`evidence-panel ${summary.status === "weak" ? "weak" : ""}`}>
      <div className="evidence-panel-head">
        <strong>{summary.status === "grounded" ? "证据可追溯" : "证据需复核"}</strong>
        <span>{summary.sourceCount} 来源</span>
        <span>{summary.confidenceLabel}</span>
        <span>{summary.routeLabel}</span>
        <span>{summary.compileLabel}</span>
      </div>
      {summary.evidencePreview ? <p>{summary.evidencePreview}</p> : null}
      {summary.warnings.length > 0 ? (
        <div className="evidence-warnings">
          {summary.warnings.map((warning) => (
            <span key={warning}>{warning}</span>
          ))}
        </div>
      ) : null}
    </div>
  );
}

function compileLabel(enabled?: boolean | null): string {
  return enabled ? "会编译" : "不编译";
}

function actionLabel(action?: string | null): string {
  switch (action) {
    case "merge_into_existing":
      return "合并建议";
    case "new_candidate":
      return "新 Memory Card";
    default:
      return "待审";
  }
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
