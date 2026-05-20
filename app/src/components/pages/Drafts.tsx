import React from "react";
import { ArrowUp, RefreshCw, X } from "lucide-react";
import { ActionButton, EmptyState, Panel } from "../common";
import {
  type CandidateRecord,
  sortCandidatesForInbox,
  translateKind,
  translateScope,
  type ProjectAction,
  type ProjectAssignmentView,
  type ProjectCandidateInbox,
  type ProjectMemoryCardLibrary,
  type ProjectReviewInbox,
  type ProjectSnapshot,
  type PanelPageProps,
} from "../../ui-helpers";

export function Drafts({
  snapshot,
  candidates,
  inbox,
  assignment,
  library,
  pendingAction,
  disabled,
  onAction,
  previewMode,
  projectPath,
  onRefresh,
}: PanelPageProps & {
  candidates: ProjectCandidateInbox | null;
  inbox: ProjectReviewInbox | null;
  assignment: ProjectAssignmentView | null;
  library: ProjectMemoryCardLibrary | null;
  previewMode: boolean;
  projectPath: string;
  onRefresh: () => void;
}) {
  const allCandidates = React.useMemo(() => sortCandidatesForInbox(candidates?.candidates ?? []), [candidates]);
  const visibleCandidates = allCandidates;

  const [selectedCandidateId, setSelectedCandidateId] = React.useState<string | null>(null);

  React.useEffect(() => {
    if (selectedCandidateId && allCandidates.some((candidate) => candidate.id === selectedCandidateId)) return;
    setSelectedCandidateId(allCandidates[0]?.id ?? null);
  }, [allCandidates, selectedCandidateId]);

  const selectedCandidate = selectedCandidateId
    ? allCandidates.find((candidate) => candidate.id === selectedCandidateId) ?? null
    : null;

  return (
    <div className="stack">
      {/* ===== 顶部极简指示行 ===== */}
      <Panel title="Suggestion Review" subtitle="审阅系统近期会话提炼出的建议规则，确认匹配真实历史证据后，一键吸纳转正。">
        <div className="engine-command-row">
          <ActionButton
            className="hero-button"
            icon={RefreshCw}
            label="从历史会话提炼建议"
            busyLabel="正在高精度整理中"
            busy={pendingAction === "整理历史"}
            disabled={disabled}
            onClick={() =>
              onAction("整理历史", "已开始整理分析建议", "evolve_project", {
                targets: ["codex", "claude-code"],
                dryRun: false,
                engine: "claude-code",
              })
            }
          />
        </div>
      </Panel>

      {/* ===== 极致两栏 triage 工作区 ===== */}
      <div className="drafts-grid" style={{ gridTemplateColumns: "310px minmax(0, 1fr)" }}>
        {/* 左栏：建议收件箱列表 */}
        <div className="drafts-column">
          <div className="drafts-column-head">
            <h3>待审建议收件箱 ({visibleCandidates.length})</h3>
          </div>

          {visibleCandidates.length === 0 ? (
            <EmptyState
              title="待审箱空空如也"
              description="先点击顶部提炼建议；机器会高精度从你最近的代码和对话中归纳偏好并排队等待核准。"
            />
          ) : (
            <div className="candidate-queue" style={{ maxHeight: "640px" }}>
              {visibleCandidates.map((candidate) => {
                const isActive = selectedCandidateId === candidate.id;
                const isWeak = candidate.confidence != null && candidate.confidence < 0.85;
                return (
                  <article
                    key={candidate.id}
                    className={`candidate-queue-item ${isActive ? "active" : ""} ${isWeak ? "needs-review" : ""}`}
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
                          <span className={`confidence-badge ${isWeak ? "review" : "safe"}`}>
                            {Math.round(candidate.confidence * 100)}% 置信
                          </span>
                        ) : null}
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

        {/* 右栏：沉浸式详阅与工作台 */}
        <div className="drafts-column drafts-detail">
          {selectedCandidate ? (
            <>
              <div className="drafts-column-head">
                <h3>建议卡片实证详查</h3>
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
            <div className="review-onboarding-card" style={{ padding: "32px", textAlign: "center" }}>
              <strong>请在左侧列表中点击选择一条规则</strong>
              <p style={{ marginTop: "14px" }}>为了给您最静心的开发体验，我们清空了多余的诊断看板。在此处，您只需专注干脆、快速地阅读证据并核准卡片：</p>
              <ul style={{ textAlign: "left", display: "inline-block", marginTop: "14px" }}>
                <li>阅读机器从最新对话或代码分析中归纳出的规则提议。</li>
                <li>确认下方的「引录真实对话历史片段」，排查这是否为一次性需求。</li>
                <li>直接选择底部的「吸收入库 / Absorb Into Library」或「忽略建议 / Ignore Draft」进行快速处置。</li>
              </ul>
            </div>
          )}
        </div>
      </div>

      {allCandidates.length === 0 ? (
        <EmptyState title="目前没有待审项目条例" description={'点击「从历史会话提炼建议」后，分析出的偏好、流程和卡片草案会自动排队在此。'} />
      ) : null}
    </div>
  );
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
  const candidateTags = candidate.tags ?? [];
  const candidateBrief =
    candidate.brief?.trim() ||
    `这条建议沉淀了"${candidate.title}"，批准后会直接进入 Memory Card。`;

  return (
    <div className="candidate-detail">
      {/* ===== 轻量化元数据标签 ===== */}
      <span className="tag" style={{ display: "inline-block", marginBottom: "12px" }}>
        Suggestion · {translateKind(candidate.kind)} · {translateScope(candidate.scope)}
        {confidence}
      </span>

      {/* ===== 建议卡片拟案内容 ===== */}
      <div className="proposed-card-content" style={{
        border: "1px solid var(--color-border-strong)",
        borderRadius: "var(--radius-lg)",
        padding: "20px",
        background: "var(--color-surface-raised)",
        boxShadow: "var(--shadow-sm)",
        marginBottom: "20px"
      }}>
        <h3 style={{ margin: "0 0 12px 0", fontSize: "16px", fontWeight: 700 }}>{candidate.title}</h3>
        {candidateBrief && (
          <p className="draft-brief" style={{ margin: "0 0 12px 0", fontStyle: "italic", color: "var(--color-text-secondary)" }}>
            {candidateBrief}
          </p>
        )}
        <p style={{ margin: 0, fontSize: "13.5px", lineHeight: "1.6", color: "var(--color-text-primary)" }}>{candidate.body}</p>
      </div>

      {/* ===== 实证历史与可追溯上下文 ===== */}
      <div className="plain-evidence-section" style={{
        marginTop: "20px",
        borderTop: "1px solid var(--color-border)",
        paddingTop: "20px"
      }}>
        <h4 style={{ margin: "0 0 10px 0", fontSize: "12px", fontWeight: 700, color: "var(--color-text-muted)", textTransform: "uppercase", letterSpacing: "0.05em" }}>
          引录真实会话与历史片段实证
        </h4>
        <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
          {candidate.evidence && (
            <blockquote style={{ margin: 0, paddingLeft: "12px", borderLeft: "3px solid var(--color-accent)", color: "var(--color-text-secondary)", fontSize: "13px", lineHeight: "1.5" }}>
              <strong>会话引录证据：</strong>{candidate.evidence}
            </blockquote>
          )}
          {candidate.reason && (
            <p style={{ margin: 0, fontSize: "12.5px", color: "var(--color-text-muted)", lineHeight: "1.5" }}>
              <strong>判断依据与推荐原因：</strong>{candidate.reason}
            </p>
          )}
          {candidate.matched_template && (
            <p style={{ margin: 0, fontSize: "12px", color: "var(--color-text-subtle)" }}>
              <strong>匹配特征模板：</strong><code>{candidate.matched_template}</code>
            </p>
          )}
          {candidate.extraction?.reason && (
            <p style={{ margin: 0, fontSize: "12.5px", color: "var(--color-text-muted)" }}>
              <strong>提炼引擎追溯：</strong>{candidate.extraction.reason}
            </p>
          )}
          {candidate.extraction?.suggested_action?.action === "merge_into_existing" && (
            <p style={{ margin: 0, fontSize: "12.5px", color: "var(--color-warning-text)", background: "var(--color-warning-bg)", padding: "8px 10px", borderRadius: "var(--radius-sm)", border: "1px solid rgba(212, 162, 59, 0.15)" }}>
              <strong>合并建议：</strong>已存在相近 Memory Card 标识 (<code>{candidate.extraction.suggested_action.target_record || candidate.extraction.suggested_action.record_id}</code>)
              {candidate.extraction.suggested_action.similarity != null && `，相似度约为 ${Math.round(candidate.extraction.suggested_action.similarity * 100)}%`}
            </p>
          )}
        </div>
      </div>

      {candidateTags.length > 0 && (
        <div className="tag-row" style={{ marginTop: "14px", display: "flex", flexWrap: "wrap", gap: "6px" }}>
          {candidateTags.map((tag) => (
            <span key={tag} className="tag-quiet" style={{ fontSize: "11px", color: "var(--color-text-muted)", background: "var(--color-canvas)", padding: "2px 6px", borderRadius: "var(--radius-sm)", border: "1px solid var(--color-border)" }}>{tag}</span>
          ))}
        </div>
      )}

      {/* ===== 底部操作动作 ===== */}
      <div className="candidate-detail-actions" style={{ display: "flex", gap: "10px", marginTop: "24px" }}>
        <div style={{ flex: 1 }}>
          <ActionButton
            className="danger-action"
            icon={X}
            label="忽略建议 / Ignore Draft"
            busyLabel="忽略中"
            busy={pendingAction === `隐藏-${candidate.id}`}
            disabled={disabled}
            onClick={() => onAction(`隐藏-${candidate.id}`, "已忽略建议", "hide_candidate", { id: candidate.id })}
          />
        </div>
        <div style={{ flex: 1 }}>
          <ActionButton
            icon={ArrowUp}
            label="吸收入库 / Absorb Into Library"
            busyLabel="入库中"
            busy={pendingAction === `批准-${candidate.id}`}
            disabled={disabled}
            onClick={() =>
              onAction(`批准-${candidate.id}`, "已批准为 Memory Card", "promote_candidate", { id: candidate.id })
            }
          />
        </div>
      </div>
    </div>
  );
}