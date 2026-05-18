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
  type ProjectEvalMetricView,
  type ProjectEvalRunView,
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
  evalRun,
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
  evalRun: ProjectEvalRunView | null;
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
    const label = command === "promote_candidate" ? "批量批准建议" : command === "hide_candidate" ? "批量隐藏建议" : "批量拒绝建议";
    const reason = command === "reject_candidate" ? batchRejectReason.trim() || "用户批量拒绝建议。" : undefined;
    await onBatchCandidateAction(label, `${label}完成`, command, selectedCandidateIds, reason ? { reason } : {});
    setSelectedCandidateIds([]);
  }

  const selectedCandidate = selectedCandidateId
    ? allCandidates.find((candidate) => candidate.id === selectedCandidateId) ?? null
    : null;

  return (
    <div className="stack">
      {/* ===== 顶部指标行 ===== */}
      <Panel title="Suggestion Review" subtitle="审阅系统建议，确认真实证据后批准为 Memory Card，再配置 Loadout 并预览 Artifact。">
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
            label="提炼建议"
            busyLabel="正在整理"
            busy={pendingAction === "整理历史"}
            disabled={disabled}
            onClick={() =>
              onAction("整理历史", "已开始整理建议", "evolve_project", {
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
            <span>{closureState.candidateCount + closureState.draftCount} 建议</span>
            <span>{closureState.unassignedCount} 未分配</span>
            <span>{closureState.driftWarningCount} Drift</span>
            <span>{closureState.buildActionCount} Artifact 动作</span>
          </div>
        </div>
        <ArtifactPreviewStrip
          preview={artifactPreview}
          disabled={disabled}
          pendingAction={pendingAction}
          onAction={onAction}
        />
      </Panel>

      {/* ===== 三栏布局：建议队列 / 详情 / 质量状态 ===== */}
      <div className="drafts-grid">
        {/* 左栏：建议队列 */}
        <div className="drafts-column">
          <div className="drafts-column-head">
            <h3>建议队列</h3>
          </div>

          {visibleCandidates.length === 0 ? (
            <p className="empty" style={{ padding: "20px 0", textAlign: "center" }}>
              暂无建议。
            </p>
          ) : (
            <div className="candidate-queue">
              {visibleCandidates.map((candidate) => {
                const isSelected = selectedCandidateIds.includes(candidate.id);
                const isActive = selectedCandidateId === candidate.id;
                const evidence = summarizeCandidateEvidence(candidate);
                return (
                <article
                  key={candidate.id}
                  className={`candidate-queue-item ${isActive ? "active" : ""} ${isSelected ? "selected" : ""} ${evidence.riskTone !== "safe" ? "needs-review" : ""}`}
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
                        <span className={`confidence-badge ${evidence.riskTone}`}>
                          {Math.round(candidate.confidence * 100)}%
                        </span>
                      ) : null}
                      <button
                        className={`candidate-select-toggle ${isSelected ? "checked" : ""}`}
                        aria-label={isSelected ? "取消选择" : "选择建议"}
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
                  <div className="candidate-queue-evidence">
                    <span>{evidence.recurrenceLabel}</span>
                    <span>{evidence.riskLabel}</span>
                  </div>
                </article>
                );
              })}
            </div>
          )}
        </div>

        {/* 中栏：选中建议详情/审阅 */}
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
              <EmptyState title="选择一个建议" description="从左侧建议队列中选择一条建议来查看证据、风险和操作。" />
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
                <span>已审建议</span>
              </div>
            </div>
            <div className="quality-bars">
              <div className="quality-bar-row">
                <span>建议</span>
                <div className="quality-bar-track">
                  <div className="quality-bar-fill" style={{ width: `${Math.min(100, allCandidates.length * 20)}%` }} />
                </div>
                <strong>{allCandidates.length}</strong>
              </div>
              <div className="quality-bar-row">
                <span>已审建议</span>
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
          <EvalRunPanel evalRun={evalRun} />
        </div>
      </div>

      {/* 已审建议列表 */}
      {drafts.length > 0 ? (
        <>
          {hiddenCount > 0 ? <p className="filter-note">已隐藏 {hiddenCount} 条低置信度或过碎建议，避免审阅列表失控。</p> : null}
          {drafts.map((draft) => (
            <React.Fragment key={draft.id}>
            <article className="record">
              <div className="record-main">
                <span className="tag">
                  {translateKind(draft.kind)} · {translateScope(draft.scope)}
                  {draft.confidence ? ` · 置信度 ${Math.round(draft.confidence * 100)}%` : ""}
                </span>
                <p className="draft-brief">
                    <span className="brief-label">建议摘要</span>
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
                  label="批准为 Memory Card"
                  busyLabel="批准中"
                  busy={pendingAction === `批准-${draft.id}`}
                  disabled={disabled}
                  onClick={() => onAction(`批准-${draft.id}`, "已批准为 Memory Card", "approve_draft", { id: draft.id })}
                />
                <ActionButton
                  className="danger-action"
                  icon={X}
                  label="拒绝"
                  busyLabel="删除中"
                  busy={pendingAction === `删除-${draft.id}`}
                  disabled={disabled}
                  onClick={() => onAction(`删除-${draft.id}`, "已拒绝建议", "reject_draft", { id: draft.id })}
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
        <EmptyState title="暂无高价值建议" description={'点击「提炼建议」后，稳定偏好、硬约束、流程和可复用 Skill 补充会出现在这里等待审核。'} />
      ) : null}

      {/* 底部批量操作栏 */}
      {selectedCandidateIds.length > 0 ? (
        <div className="batch-bar-sticky">
          <div className="batch-row" style={{ margin: 0 }}>
            <span>已选择 {selectedCandidateIds.length} 条建议</span>
            <input
              type="text"
              value={batchRejectReason}
              onChange={(event) => setBatchRejectReason(event.target.value)}
              placeholder="批量拒绝原因"
            />
            <button className="primary-action" disabled={disabled || selectedCandidateIds.length === 0} onClick={() => void runBatch("promote_candidate")}>
              批量批准建议
            </button>
            <button className="secondary-action" disabled={disabled || selectedCandidateIds.length === 0} onClick={() => void runBatch("hide_candidate")}>
              批量隐藏建议
            </button>
            <button className="danger-action" disabled={disabled || selectedCandidateIds.length === 0} onClick={() => void runBatch("reject_candidate")}>
              批量拒绝建议
            </button>
          </div>
        </div>
      ) : null}
    </div>
  );
}

function EvalRunPanel({ evalRun }: { evalRun: ProjectEvalRunView | null }) {
  const metrics = [
    evalRun?.recall,
    evalRun?.precision,
    evalRun?.one_off_false_positive,
    evalRun?.duplicate_cluster_risk,
    evalRun?.evidence_validity,
    evalRun?.provider_evidence_validity,
  ].filter(Boolean) as ProjectEvalMetricView[];

  if (!evalRun || evalRun.status === "missing") {
    return (
      <div className="eval-run-card missing">
        <div className="eval-run-head">
          <span>Eval Run</span>
          <strong>未建立基线</strong>
        </div>
        <p>{evalRun?.recommendations[0] ?? "运行 golden-set eval 后，这里会显示召回、精确率和证据有效性。"}</p>
      </div>
    );
  }

  return (
    <div className={`eval-run-card ${evalRun.status}`}>
      <div className="eval-run-head">
        <span>Eval Run</span>
        <strong>{evalRun.status === "passing" ? "通过" : "需要关注"}</strong>
      </div>
      <div className="eval-run-meta">
        <span>{evalRun.provider ?? "unknown provider"}</span>
        <span>v{evalRun.pipeline_version ?? "?"}</span>
        <span>{formatEvalTimestamp(evalRun.timestamp)}</span>
      </div>
      <div className="eval-metric-list">
        {metrics.map((metric) => (
          <div key={metric.label} className={`eval-metric-row ${metric.status}`}>
            <span>{evalMetricLabel(metric.label)}</span>
            <strong>{formatEvalPercent(metric.percent)}</strong>
            <small>{metric.count}/{metric.total}</small>
          </div>
        ))}
      </div>
      {evalRun.recommendations.length > 0 ? (
        <div className="eval-recommendations">
          {evalRun.recommendations.slice(0, 2).map((recommendation) => (
            <p key={recommendation}>{recommendation}</p>
          ))}
        </div>
      ) : null}
    </div>
  );
}

function formatEvalPercent(percent?: number | null) {
  return percent == null ? "-" : `${Math.round(percent)}%`;
}

function formatEvalTimestamp(timestamp?: string | null) {
  if (!timestamp) return "未记录时间";
  const parsed = new Date(timestamp);
  if (Number.isNaN(parsed.getTime())) return timestamp;
  return parsed.toLocaleString();
}

function evalMetricLabel(label: string) {
  const labels: Record<string, string> = {
    Recall: "召回",
    Precision: "精确",
    "One-off false positives": "一次性误报",
    "Duplicate risk": "重复风险",
    "Evidence validity": "证据有效",
    "Provider evidence validity": "Provider 证据",
  };
  return labels[label] ?? label;
}

/** 建议详情卡片 */
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
    `这条建议沉淀了"${candidate.title}"，批准后会直接进入 Memory Card。`;
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
        Suggestion · {translateKind(candidate.kind)} · {translateScope(candidate.scope)}
        {confidence}
      </span>
      <h3>{candidate.title}</h3>
      <p className="draft-brief">
        <span className="brief-label">概述</span>
        {candidateBrief}
      </p>
      <div className="review-step-stack" aria-label="审阅步骤">
        <section className="review-step">
          <span>1</span>
          <div>
            <strong>证据与风险</strong>
            <p>先确认这条建议是否来自真实、可追溯、足够稳定的上下文。</p>
          </div>
        </section>
        <EvidencePanel summary={evidenceSummary} />
        <section className="review-step">
          <span>2</span>
          <div>
            <strong>动作与 Artifact 影响</strong>
            <p>{evidenceSummary.actionLabel} · {evidenceSummary.artifactImpactLabel}</p>
          </div>
        </section>
      </div>
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
      <div className="candidate-detail-actions">
        <div className="review-action-hint">
          <strong>3 · 人工决定</strong>
          <span>批准前请先看完证据、风险和 Artifact 影响。</span>
        </div>
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
          onClick={() => onAction(`隐藏-${candidate.id}`, "已隐藏建议", "hide_candidate", { id: candidate.id })}
        />
        <ActionButton
          className="danger-action"
          icon={X}
          label="拒绝"
          busyLabel="拒绝中"
          busy={pendingAction === `拒绝-${candidate.id}`}
          disabled={disabled}
          onClick={() =>
            onAction(`拒绝-${candidate.id}`, "已拒绝建议", "reject_candidate", {
              id: candidate.id,
              reason: rejectReason.trim() || "用户从 Suggestion Review 拒绝。",
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
