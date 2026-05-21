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
  type SynthesisTraceEntry,
  type PanelPageProps,
} from "../../ui-helpers";

type SynthesisEngine = "claude-code" | "codex" | "local" | "llm";

const ENGINE_OPTIONS: Array<{ value: SynthesisEngine; label: string; hint: string }> = [
  { value: "llm", label: "LLM Provider", hint: "使用 Settings 中的 Provider 做抽取、审核与最终转写" },
  { value: "claude-code", label: "Claude Code", hint: "调用本地 Claude Code CLI；失败时后端会回退" },
  { value: "codex", label: "Codex", hint: "调用本地 Codex CLI 做候选整理" },
  { value: "local", label: "Local", hint: "只用本地规则，速度快但不会做 LLM 转写" },
];

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
  synthesisEngine,
  onSynthesisEngineChange,
  onRefresh,
}: PanelPageProps & {
  candidates: ProjectCandidateInbox | null;
  inbox: ProjectReviewInbox | null;
  assignment: ProjectAssignmentView | null;
  library: ProjectMemoryCardLibrary | null;
  previewMode: boolean;
  projectPath: string;
  synthesisEngine: SynthesisEngine;
  onSynthesisEngineChange: (engine: SynthesisEngine) => void;
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
          <label className="engine-picker" title={ENGINE_OPTIONS.find((item) => item.value === synthesisEngine)?.hint}>
            <span>提炼引擎</span>
            <select
              value={synthesisEngine}
              onChange={(event) => onSynthesisEngineChange(event.target.value as SynthesisEngine)}
            >
              {ENGINE_OPTIONS.map((option) => (
                <option key={option.value} value={option.value}>{option.label}</option>
              ))}
            </select>
          </label>
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
                engine: synthesisEngine,
              })
            }
          />
        </div>
      </Panel>

      {/* ===== 极致两栏 triage 工作区 ===== */}
      <div className="drafts-grid">
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
                const preview = matureCandidatePreview(candidate);
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
                        <strong>{preview.title}</strong>
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
                      {preview.brief}
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
  const preview = matureCandidatePreview(candidate);

  return (
    <div className="candidate-detail">
      {/* ===== 轻量化元数据标签 ===== */}
      <span className="tag" style={{ display: "inline-block", marginBottom: "12px" }}>
        Suggestion · {translateKind(candidate.kind)} · {translateScope(candidate.scope)}
        {confidence}
      </span>

      <ReviewOutcomePanel candidate={candidate} />

      {/* ===== 建议卡片拟案内容 ===== */}
      <div className="proposed-card-content" style={{
        border: "1px solid var(--color-border-strong)",
        borderRadius: "var(--radius-lg)",
        padding: "20px",
        background: "var(--color-surface-raised)",
        boxShadow: "var(--shadow-sm)",
        marginBottom: "20px"
      }}>
        <h3 style={{ margin: "0 0 12px 0", fontSize: "16px", fontWeight: 700 }}>{preview.title}</h3>
        {preview.brief && (
          <p className="draft-brief" style={{ margin: "0 0 12px 0", fontStyle: "italic", color: "var(--color-text-secondary)" }}>
            {preview.brief}
          </p>
        )}
        <p style={{ margin: 0, fontSize: "13.5px", lineHeight: "1.6", color: "var(--color-text-primary)", whiteSpace: "pre-wrap" }}>{preview.body}</p>
      </div>

      <ValueSynthesisPanel candidate={candidate} />

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
              。批准后会更新既有卡片，不会新增重复项。
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
          {candidate.extraction?.synthesis_action === "already_covered" ? (
            <ActionButton
              icon={X}
              label="标记已覆盖 / No New Card"
              busyLabel="归档中"
              busy={pendingAction === `已覆盖-${candidate.id}`}
              disabled={disabled}
              onClick={() =>
                onAction(`已覆盖-${candidate.id}`, "已标记为既有规则覆盖", "hide_candidate", { id: candidate.id })
              }
            />
          ) : (
            <ActionButton
              icon={ArrowUp}
              label={
                candidate.extraction?.suggested_action?.action === "merge_into_existing"
                  ? "合并更新 / Merge Card"
                  : "吸收入库 / Absorb Into Library"
              }
              busyLabel="入库中"
              busy={pendingAction === `批准-${candidate.id}`}
              disabled={disabled}
              onClick={() =>
                onAction(`批准-${candidate.id}`, "已批准为 Memory Card", "promote_candidate", { id: candidate.id })
              }
            />
          )}
        </div>
      </div>
    </div>
  );
}

function ReviewOutcomePanel({ candidate }: { candidate: CandidateRecord }) {
  const extraction = candidate.extraction;
  const action = extraction?.synthesis_action ?? extraction?.suggested_action?.action;
  const stopReason = extraction?.synthesis_stop_reason;
  if (!action && !stopReason) return null;
  const copy = reviewOutcomeCopy(action, stopReason);
  return (
    <section
      style={{
        border: `1px solid ${copy.tone === "warning" ? "rgba(212, 162, 59, 0.32)" : "var(--color-border)"}`,
        borderRadius: "var(--radius-md)",
        padding: "12px 14px",
        background: copy.tone === "warning" ? "var(--color-warning-bg)" : "var(--color-surface)",
        marginBottom: "16px",
      }}
    >
      <div style={{ display: "flex", justifyContent: "space-between", gap: "10px", alignItems: "center" }}>
        <strong style={{ fontSize: "13px", color: "var(--color-text-primary)" }}>{copy.title}</strong>
        <span className="tag-quiet">{stopReasonLabel(stopReason)}</span>
      </div>
      <p style={{ margin: "8px 0 0", fontSize: "12.5px", lineHeight: 1.5, color: "var(--color-text-secondary)" }}>
        {copy.body}
      </p>
    </section>
  );
}

function ValueSynthesisPanel({ candidate }: { candidate: CandidateRecord }) {
  const extraction = candidate.extraction;
  const delta = extraction?.value_delta;
  const trace = extraction?.synthesis_trace ?? [];
  const hasValue =
    Boolean(extraction?.value_claim) ||
    Boolean(delta?.existing_behavior) ||
    Boolean(delta?.missing_part) ||
    Boolean(delta?.new_behavior) ||
    Boolean(delta?.why_not_duplicate);
  if (!hasValue) return null;
  const cardFunction = memoryCardFunctionLabel(extraction?.card_function);
  return (
    <section
      style={{
        border: "1px solid var(--color-border)",
        borderRadius: "var(--radius-md)",
        padding: "14px 16px",
        background: "var(--color-surface)",
        marginBottom: "18px",
      }}
    >
      <div style={{ display: "flex", justifyContent: "space-between", gap: "12px", alignItems: "center", marginBottom: "10px" }}>
        <h4 style={{ margin: 0, fontSize: "12px", fontWeight: 800, color: "var(--color-text-muted)", textTransform: "uppercase", letterSpacing: "0.05em" }}>
          Value Delta
        </h4>
        <span className="tag-quiet">{cardFunction}</span>
      </div>
      {extraction?.value_claim && (
        <p style={{ margin: "0 0 12px", fontSize: "13px", lineHeight: 1.55, color: "var(--color-text-primary)" }}>
          <strong>价值主张：</strong>{extraction.value_claim}
        </p>
      )}
      {delta && (
        <div style={{ display: "grid", gap: "8px" }}>
          <ValueDeltaRow label="已有能力" value={delta.existing_behavior} />
          <ValueDeltaRow label="缺口" value={delta.missing_part} />
          <ValueDeltaRow label="新行为" value={delta.new_behavior} />
          <ValueDeltaRow label="非重复理由" value={delta.why_not_duplicate} />
        </div>
      )}
      {extraction?.target_context?.why_this_target && (
        <p style={{ margin: "12px 0 0", fontSize: "12px", lineHeight: 1.5, color: "var(--color-text-secondary)" }}>
          <strong>目标：</strong>{targetContextLabel(extraction.target_context.target_type)}
          {extraction.target_context.target_id ? ` · ${extraction.target_context.target_id}` : ""} — {extraction.target_context.why_this_target}
        </p>
      )}
      <SynthesisTracePanel trace={trace} />
    </section>
  );
}

function SynthesisTracePanel({ trace }: { trace: SynthesisTraceEntry[] }) {
  const groups = groupTraceEntries(trace);
  if (groups.length === 0) return null;
  return (
    <div
      aria-label="Synthesis Trace"
      style={{
        borderTop: "1px solid var(--color-border)",
        marginTop: "14px",
        paddingTop: "14px",
      }}
    >
      <div style={{ display: "flex", justifyContent: "space-between", gap: "10px", alignItems: "center", marginBottom: "10px" }}>
        <strong style={{ fontSize: "12px", color: "var(--color-text-muted)", textTransform: "uppercase", letterSpacing: "0.05em" }}>
          Synthesis Trace
        </strong>
        <span className="tag-quiet">只读证据链</span>
      </div>
      <div style={{ display: "grid", gap: "8px" }}>
        {groups.map((group) => (
          <article
            key={group.key}
            style={{
              display: "grid",
              gridTemplateColumns: "minmax(92px, 120px) minmax(0, 1fr)",
              gap: "10px",
              padding: "9px 10px",
              border: "1px solid var(--color-border)",
              borderRadius: "var(--radius-sm)",
              background: "var(--color-canvas)",
            }}
          >
            <div>
              <strong style={{ display: "block", fontSize: "12px", color: "var(--color-text-primary)" }}>{group.title}</strong>
              <span style={{ display: "block", marginTop: "3px", fontSize: "11px", lineHeight: 1.35, color: "var(--color-text-subtle)" }}>
                {group.helper}
              </span>
            </div>
            <div style={{ minWidth: 0 }}>
              {group.entries.map((entry) => (
                <div key={`${group.key}-${entry.step}-${entry.summary}`} style={{ marginBottom: "6px" }}>
                  <p style={{ margin: 0, fontSize: "12px", lineHeight: 1.45, color: "var(--color-text-secondary)" }}>
                    <span style={{ color: "var(--color-text-muted)", fontWeight: 700 }}>{traceStepLabel(entry.step)}：</span>
                    {entry.summary}
                  </p>
                  {entry.item_ids && entry.item_ids.length > 0 ? (
                    <div style={{ display: "flex", flexWrap: "wrap", gap: "4px", marginTop: "5px" }}>
                      {entry.item_ids.slice(0, 4).map((itemId) => (
                        <code
                          key={itemId}
                          style={{
                            fontSize: "10.5px",
                            color: "var(--color-text-muted)",
                            background: "var(--color-surface)",
                            border: "1px solid var(--color-border)",
                            borderRadius: "var(--radius-sm)",
                            padding: "1px 5px",
                            maxWidth: "100%",
                            overflowWrap: "anywhere",
                          }}
                        >
                          {itemId}
                        </code>
                      ))}
                      {entry.item_ids.length > 4 ? (
                        <span className="tag-quiet">+{entry.item_ids.length - 4}</span>
                      ) : null}
                    </div>
                  ) : null}
                </div>
              ))}
            </div>
          </article>
        ))}
      </div>
    </div>
  );
}

function ValueDeltaRow({ label, value }: { label: string; value?: string }) {
  if (!value) return null;
  return (
    <p style={{ margin: 0, fontSize: "12.5px", lineHeight: 1.5, color: "var(--color-text-secondary)" }}>
      <strong>{label}：</strong>{value}
    </p>
  );
}

type TraceGroup = {
  key: string;
  title: string;
  helper: string;
  steps: string[];
  entries: SynthesisTraceEntry[];
};

function groupTraceEntries(trace: SynthesisTraceEntry[]): TraceGroup[] {
  const definitions: Array<Omit<TraceGroup, "entries">> = [
    {
      key: "direct-evidence",
      title: "直接证据",
      helper: "候选绑定的历史片段",
      steps: ["search_observations", "filter"],
    },
    {
      key: "memory-comparison",
      title: "规则库对照",
      helper: "查重、覆盖或合并",
      steps: ["search_memory_cards", "find_memory_duplicates", "duplicate_check"],
    },
    {
      key: "skill-comparison",
      title: "Skill 对照",
      helper: "目标 Skill 与缺口",
      steps: ["search_skills", "compare_with_skill", "evaluate_skill_usefulness"],
    },
    {
      key: "decision",
      title: "审阅决策",
      helper: "转写规范与停止原因",
      steps: ["value_delta", "read_writing_guide", "rewrite", "stop"],
    },
  ];
  return definitions
    .map((definition) => ({
      ...definition,
      entries: trace.filter((entry) => definition.steps.includes(entry.step)).slice(0, 3),
    }))
    .filter((group) => group.entries.length > 0);
}

function memoryCardFunctionLabel(value?: string | null) {
  const labels: Record<string, string> = {
    library: "Library Memory Card",
    skill_targeted: "Skill-targeted Memory Card",
    workflow: "Workflow Memory Card",
    merge: "Merge Memory Card",
  };
  return value ? labels[value] ?? value : "Memory Card";
}

function targetContextLabel(value?: string) {
  const labels: Record<string, string> = {
    project_skill: "项目级 Skill",
    workflow: "项目工作流",
    memory_card: "既有 Memory Card",
    global_reference: "全局参考",
  };
  return value ? labels[value] ?? value : "未指定";
}

function traceStepLabel(value: string) {
  const labels: Record<string, string> = {
    filter: "已过滤",
    value_delta: "已比较价值差异",
    duplicate_check: "已查重",
    rewrite: "已转写",
    search_observations: "已读直接证据",
    search_memory_cards: "已查规则库",
    search_skills: "已查 Skills",
    read_writing_guide: "已读写作规范",
    find_memory_duplicates: "已判重",
    compare_with_skill: "已比对 Skill",
    evaluate_skill_usefulness: "已评估 Skill 有用性",
    stop: "已停止",
  };
  return labels[value] ?? value;
}

function reviewOutcomeCopy(action?: string | null, stopReason?: string | null) {
  if (action === "already_covered" || stopReason === "already_covered") {
    return {
      tone: "warning",
      title: "No Card Is A Success",
      body: "runtime 判断该候选已经被既有 Memory Card 覆盖。推荐归档候选，保持规则库和 Skill 上下文轻盈。",
    };
  }
  if (action === "merge_card" || action === "merge_into_existing" || stopReason === "merge_target_found") {
    return {
      tone: "neutral",
      title: "建议合并更新",
      body: "这条反馈有价值，但更适合更新既有 Memory Card。批准后会走合并路径，不会新增重复卡片。",
    };
  }
  if (action === "skill_targeted_card" || stopReason === "skill_gap_found") {
    return {
      tone: "neutral",
      title: "项目级 Skill 补强",
      body: "这张 Memory Card 的价值在于补上项目级 Skill 的触发、动作或边界，后续可在 Skills 页面挂载或融合。",
    };
  }
  if (stopReason === "needs_human") {
    return {
      tone: "warning",
      title: "需要人工判断",
      body: "证据或目标上下文还不够稳定。请优先看 Value Delta 和真实引录，再决定忽略还是吸收入库。",
    };
  }
  return {
    tone: "neutral",
    title: "可审阅 Memory Card",
    body: "runtime 已完成历史、规则库、Skills 和写作规范的只读检查；请根据价值增量决定是否吸收。",
  };
}

function stopReasonLabel(value?: string | null) {
  const labels: Record<string, string> = {
    already_covered: "已覆盖",
    merge_target_found: "合并目标",
    skill_gap_found: "Skill 缺口",
    workflow_gap_found: "工作流缺口",
    new_card_grounded: "可新增",
    needs_human: "需人工判断",
  };
  return value ? labels[value] ?? value : "已审阅";
}

function matureCandidatePreview(candidate: CandidateRecord) {
  const language = inferCandidateLanguage(candidate);
  const title = cleanCandidateTitle(candidate.title, candidate.body);
  const body = looksStructuredCandidateBody(candidate.body)
    ? candidate.body.trim()
    : renderCandidatePreviewBody(candidate, language);
  const actionLine = extractActionLine(body);
  return {
    title,
    body,
    brief: language === "zh"
      ? `长期规则：${actionLine || title}`
      : `Long-term rule: ${actionLine || title}`,
  };
}

function renderCandidatePreviewBody(candidate: CandidateRecord, language: "zh" | "en") {
  const source = cleanChattyText(candidate.body);
  const [trigger, rest] = source
    .split(/，|,/, 2)
    .map((part) => part.trim());
  const action = rest || source;
  if (language === "zh") {
    return [
      `目标：让“${cleanCandidateTitle(candidate.title, candidate.body)}”成为可复用、可审阅、可挂载的 Memory Card。`,
      `适用场景：${ensureZhWhen(trigger || candidate.title)}`,
      `执行方式：${stripSentenceEnd(action)}`,
      "边界：仅在真实历史证据支持、且能长期改善项目工作流或 Skill 行为时吸收入库；一次性任务、重复内容和临时偏好应忽略。",
      "验收：批准后应能独立说明触发条件、执行动作、适用边界和相对既有 Memory Card 的新增价值。",
    ].join("\n\n");
  }
  return [
    `Purpose: Make "${cleanCandidateTitle(candidate.title, candidate.body)}" a reusable, reviewable, attachable Memory Card.`,
    `When: ${trigger || candidate.title}`,
    `Do: ${stripSentenceEnd(action)}`,
    "Boundary: Approve only when evidence supports a durable improvement to project workflow or Skill behavior; ignore one-off task chatter, duplicates, and temporary preferences.",
    "Acceptance: The approved card should state its trigger, action, boundary, and incremental value over existing Memory Cards.",
  ].join("\n\n");
}

function extractActionLine(body: string) {
  const actionLine = body
    .split("\n")
    .find((line) =>
      line.includes("执行方式")
      || line.includes("动作")
      || line.toLowerCase().startsWith("do:")
    );
  return actionLine
    ?.replace("执行方式：", "")
    .replace("动作：", "")
    .replace("Do:", "")
    .trim();
}

function ensureZhWhen(value: string) {
  const trimmed = stripSentenceEnd(cleanChattyText(value)).replace(/^当/, "").replace(/^在/, "").replace(/时$/, "");
  return trimmed ? `当${trimmed}时` : "当处理相关项目任务时";
}

function stripSentenceEnd(value: string) {
  return value.trim().trimEnd().replace(/[。.;；]+$/, "");
}

function cleanCandidateTitle(title: string, body: string) {
  const source = cleanChattyText(title || body)
    .split(/[：:；;。，,]/)[0]
    ?.trim() || "项目规则";
  return source.replace(/^\/goal\s*/, "").replace(/^当/, "").replace(/^在/, "").replace(/时$/, "").slice(0, 36);
}

function cleanChattyText(value: string) {
  return value
    .replace(/\/goal/g, "")
    .replace(/这条候选建议沉淀了/g, "")
    .replace(/[“”"]/g, "")
    .replace(/\s+/g, " ")
    .trim();
}

function inferCandidateLanguage(candidate: CandidateRecord): "zh" | "en" {
  const text = `${candidate.title}\n${candidate.body}\n${candidate.brief ?? ""}`;
  return /[\u4e00-\u9fff]/.test(text) ? "zh" : "en";
}

function looksStructuredCandidateBody(body: string) {
  const lower = body.toLowerCase();
  return (lower.includes("when") && lower.includes("do") && lower.includes("boundary"))
    || (body.includes("触发") && body.includes("动作") && body.includes("边界"))
    || (body.includes("目标") && body.includes("适用场景") && body.includes("执行方式") && body.includes("边界"));
}
