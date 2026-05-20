import React from "react";
import { Check, Loader2, ShieldAlert, ShieldCheck, X } from "lucide-react";
import { planKernelCommand, updateCandidate, updateDraft, updateMemoryCard } from "../../tauri-client";
import {
  buildKernelPlanForEditor,
  confirmedAgentManagedPolicy,
  EDITABLE_AGENTS,
  formatAgent,
  getPreviewPlanResult,
  KIND_OPTIONS,
  manualReviewPolicy,
  normalizePlanReviewResult,
  parseCommaTags,
  SCOPE_OPTIONS,
  translateKind,
  translateScope,
  type EditFormData,
  type ExtractionMetadata,
  type PlanReviewResult,
} from "../../ui-helpers";

type RecordEditorProps = {
  recordType: "draft" | "candidate" | "memory_card";
  initialForm: EditFormData;
  extraction?: ExtractionMetadata;
  previewMode: boolean;
  projectPath: string;
  recordId: string;
  onSaved: () => void;
  onCancel: () => void;
};

/** 通用记录编辑器：支持草稿和 Memory Card 的字段编辑，含审查和保存流程 */
export function RecordEditor({ recordType, initialForm, extraction, previewMode, projectPath, recordId, onSaved, onCancel }: RecordEditorProps) {
  const [form, setForm] = React.useState<EditFormData>(initialForm);
  const [planResult, setPlanResult] = React.useState<PlanReviewResult | null>(null);
  const [reviewing, setReviewing] = React.useState(false);
  const [saving, setSaving] = React.useState(false);
  const [confirmed, setConfirmed] = React.useState(false);
  const [message, setMessage] = React.useState("");

  const isRejected = planResult?.disposition === "reject";
  const needsConfirm = (planResult?.reviewRequired && !confirmed) ?? false;
  const canSave = planResult !== null && !isRejected && !needsConfirm;

  function updateField(field: keyof EditFormData, value: string | string[]) {
    setForm((prev) => ({ ...prev, [field]: value }));
    setConfirmed(false);
    setPlanResult(null);
  }

  function buildMutationInput() {
    const tags = parseCommaTags(form.tagsInput);
    if (recordType === "draft" || recordType === "candidate") {
      return { title: form.title, brief: form.brief || undefined, body: form.body, kind: form.kind, scope: form.scope, tags, targets: form.targets };
    }
    return { title: form.title, brief: form.brief || undefined, body: form.body, kind: form.kind, scope: form.scope, tags };
  }

  /** 调用 plan_kernel_command 获取审查结果 */
  async function handleReview() {
    setReviewing(true);
    setMessage("");
    try {
      if (previewMode) {
        await new Promise((r) => setTimeout(r, 400));
        const result = getPreviewPlanResult(form.kind);
        setPlanResult(result);
        setMessage("预览模式：已模拟审查，不会调用后端。");
      } else {
        const mutationInput = buildMutationInput();
        const plan = buildKernelPlanForEditor(recordType, recordId, mutationInput);
        const result = await planKernelCommand({
          command: plan.command,
          policy: manualReviewPolicy(),
          projectPath,
          payload: plan.payload,
        });
        setPlanResult(normalizePlanReviewResult(result));
      }
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : String(error);
      const lower = errorMessage.toLowerCase();
      const isPolicyRejection =
        lower.includes("review") ||
        lower.includes("manual") ||
        lower.includes("decision token") ||
        lower.includes("confirmation failed") ||
        lower.includes("policy blocked");
      setPlanResult({
        risk: "unknown",
        disposition: "reject",
        reason: errorMessage,
        reviewRequired: isPolicyRejection,
        requires_human_review: isPolicyRejection,
      });
    } finally {
      setReviewing(false);
      setConfirmed(false);
    }
  }

  /** 保存变更：草稿调用 update_draft，Memory Card 调用 update_memory_card */
  async function handleSave() {
    setSaving(true);
    setMessage("");
    try {
      const mutationInput = buildMutationInput();
      if (previewMode) {
        setMessage("预览模式：已模拟保存变更，不会写入文件。");
        setPlanResult(null);
        setSaving(false);
        onSaved();
        return;
      }

      if (recordType === "draft") {
        await updateDraft({
          projectPath,
          id: recordId,
          input: mutationInput,
          confirmedPolicy: confirmedAgentManagedPolicy(),
          decisionToken: planResult?.decisionToken,
        });
      } else if (recordType === "candidate") {
        await updateCandidate({
          projectPath,
          id: recordId,
          input: mutationInput,
          confirmedPolicy: confirmedAgentManagedPolicy(),
          decisionToken: planResult?.decisionToken,
        });
      } else {
        await updateMemoryCard({
          projectPath,
          id: recordId,
          input: mutationInput,
          confirmedPolicy: confirmedAgentManagedPolicy(),
          decisionToken: planResult?.decisionToken,
        });
      }
      setPlanResult(null);
      setSaving(false);
      onSaved();
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : String(error);
      setMessage(errorMessage);
      setSaving(false);
    }
  }

  const recordLabel = recordType === "draft" ? "草稿" : recordType === "candidate" ? "候选" : "Memory Card";

  return (
    <article className="edit-panel">
      <div className="edit-header">
        <h3>编辑{recordLabel}</h3>
        <button className="ghost-action" onClick={onCancel} disabled={saving}>
          <X size={15} />
          取消
        </button>
      </div>

      <div className="edit-fields">
        <label>
          <span>标题</span>
          <input
            type="text"
            value={form.title}
            onChange={(e) => updateField("title", e.target.value)}
            placeholder="输入标题"
          />
        </label>

        <label>
          <span>简要说明</span>
          <input
            type="text"
            value={form.brief}
            onChange={(e) => updateField("brief", e.target.value)}
            placeholder="简短描述该记录的用途"
          />
        </label>

        <label>
          <span>内容</span>
          <textarea
            value={form.body}
            onChange={(e) => updateField("body", e.target.value)}
            rows={4}
            placeholder="输入详细内容"
          />
        </label>

        <div className="edit-row">
          <label>
            <span>类型</span>
            <select value={form.kind} onChange={(e) => updateField("kind", e.target.value)}>
              {KIND_OPTIONS.map((k) => (
                <option key={k} value={k}>{translateKind(k)}</option>
              ))}
            </select>
          </label>

          <label>
            <span>范围</span>
            <select value={form.scope} onChange={(e) => updateField("scope", e.target.value)}>
              {SCOPE_OPTIONS.map((s) => (
                <option key={s} value={s}>{translateScope(s)}</option>
              ))}
            </select>
          </label>
        </div>

        <label>
          <span>标签（逗号分隔）</span>
          <input
            type="text"
            value={form.tagsInput}
            onChange={(e) => updateField("tagsInput", e.target.value)}
            placeholder="例如：ui-design, security"
          />
        </label>

        {recordType === "draft" || recordType === "candidate" ? (
          <fieldset className="edit-targets">
            <legend>目标智能体</legend>
            {EDITABLE_AGENTS.map((agent) => (
              <label key={agent} className="target-checkbox">
                <input
                  type="checkbox"
                  checked={form.targets.includes(agent)}
                  onChange={(e) => {
                    const next = e.target.checked
                      ? [...form.targets, agent]
                      : form.targets.filter((a) => a !== agent);
                    updateField("targets", next);
                  }}
                />
                {formatAgent(agent)}
              </label>
            ))}
            {form.targets.length === 0 ? (
              <span className="target-hint">休眠 / 未启用</span>
            ) : null}
          </fieldset>
        ) : null}
      </div>

      {extraction ? (
        <section className="editor-section">
          <h3>提取依据</h3>
          <p>{extraction.reason || "本条记录来自高价值提取流程。"}</p>
          <dl>
            <dt>来源</dt>
            <dd>{extraction.origin || "unknown"}</dd>
            <dt>信号</dt>
            <dd>{extraction.matched_signal || "unknown"}</dd>
            <dt>相似记录</dt>
            <dd>{extraction.similar_record || "无"}</dd>
            {extraction.classification ? (
              <>
                <dt>信号类型</dt>
                <dd>{extraction.classification.signal || "unknown"}</dd>
                <dt>产物类型</dt>
                <dd>{extraction.classification.artifact_kind || "unknown"}</dd>
                <dt>硬度</dt>
                <dd>{extraction.classification.hardness || "unknown"}</dd>
                <dt>激活方式</dt>
                <dd>{extraction.classification.activation || "unknown"}</dd>
              </>
            ) : null}
          </dl>
        </section>
      ) : null}

      <div className="edit-actions">
        <button
          className="secondary-action"
          disabled={reviewing || saving}
          onClick={() => void handleReview()}
        >
          {reviewing ? <Loader2 className="spin" size={15} /> : <ShieldAlert size={15} />}
          {reviewing ? "审查中..." : "审查变更"}
        </button>
        {needsConfirm ? (
          <button
            className="primary-action hero-button"
            disabled={saving}
            onClick={() => {
              setConfirmed(true);
              void handleSave();
            }}
          >
            <ShieldCheck size={15} />
            确认保存
          </button>
        ) : (
          <button
            className="primary-action hero-button"
            disabled={!canSave || saving}
            onClick={() => void handleSave()}
          >
            {saving ? <Loader2 className="spin" size={15} /> : <Check size={15} />}
            {saving ? "保存中..." : "保存"}
          </button>
        )}
      </div>

      {planResult ? (
        <div className={`plan-result ${planResult.disposition}`}>
          <div className="plan-result-header">
            <span className="plan-risk">风险等级：{planResult.risk}</span>
            <span className="plan-disposition">
              处置：
              {planResult.disposition === "execute"
                ? "通过"
                : planResult.disposition === "reject"
                  ? "拒绝"
                  : planResult.disposition === "draft-only"
                    ? "仅生成草稿"
                    : "需复核"}
            </span>
          </div>
          <p>{planResult.reason}</p>
          {planResult.reviewRequired ? <p className="plan-review-hint">该变更需要人工复核，请点击"确认保存"以继续。</p> : null}
        </div>
      ) : null}

      {message ? <p className="edit-message">{message}</p> : null}
    </article>
  );
}
