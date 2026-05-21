import React from "react";
import { AlertTriangle, Check, CloudCog, Loader2, Monitor, Moon, RefreshCw, Save, Sun, TestTube2 } from "lucide-react";
import { Panel } from "../common";
import { getCustomProviderConfig, getSetupChecklist, planKernelCommand, saveCustomProviderConfig, testProviderStatus } from "../../tauri-client";
import {
  buildKernelPlanForInvoke,
  confirmedAgentManagedPolicy,
  formatAgent,
  normalizePlanReviewResult,
  type CustomProviderConfig,
  type DesktopAppState,
  type ProviderStatusReport,
  type ProjectSnapshot,
  type SetupChecklistItem,
  type SetupChecklistReport,
} from "../../ui-helpers";

const THEME_OPTIONS: { key: "light" | "dark" | "system"; label: string; icon: React.ComponentType<{ size?: number }> }[] = [
  { key: "light", label: "浅色", icon: Sun },
  { key: "dark", label: "暗色", icon: Moon },
  { key: "system", label: "跟随系统", icon: Monitor },
];

export function Settings({
  state,
  snapshot,
  projectPath,
  previewMode,
  synthesisEngine,
  theme,
  onThemeChange,
  onSynthesisEngineChange,
}: {
  state: DesktopAppState | null;
  snapshot: ProjectSnapshot | null;
  projectPath: string;
  previewMode: boolean;
  synthesisEngine: "claude-code" | "codex" | "local" | "llm";
  theme: "light" | "dark" | "system";
  onThemeChange: (theme: "light" | "dark" | "system") => void;
  onSynthesisEngineChange: (engine: "claude-code" | "codex" | "local" | "llm") => void;
}) {
  const [providerConfig, setProviderConfig] = React.useState<CustomProviderConfig | null>(null);
  const [providerKey, setProviderKey] = React.useState("");
  const [providerSaving, setProviderSaving] = React.useState(false);
  const [providerTesting, setProviderTesting] = React.useState(false);
  const [providerMessage, setProviderMessage] = React.useState("");
  const [setupChecklist, setSetupChecklist] = React.useState<SetupChecklistReport | null>(null);
  const [providerStatus, setProviderStatus] = React.useState<ProviderStatusReport | null>(null);
  const [showDiagnostics, setShowDiagnostics] = React.useState(false);
  const providerDisabledReason = !projectPath
    ? "请先在左上角选择一个项目；Provider 配置会保存到该项目。"
    : !providerConfig
      ? "正在读取 Provider 配置。"
      : "";

  React.useEffect(() => {
    if (!projectPath || previewMode) {
      setProviderConfig({
        enabled: false,
        protocol: "openai-compatible",
        base_url: "https://api.openai.com/v1",
        model: "gpt-4.1-mini",
        api_key_env: "OPENAI_API_KEY",
      });
      return;
    }
    let cancelled = false;
    void Promise.all([getCustomProviderConfig(projectPath), getSetupChecklist(projectPath), testProviderStatus(projectPath, false)])
      .then(([config, checklist, status]) => {
        if (!cancelled) {
          setProviderConfig(config);
          setSetupChecklist(checklist);
          setProviderStatus(status);
        }
      })
      .catch((error) => {
        if (!cancelled) setProviderMessage(error instanceof Error ? error.message : String(error));
      });
    return () => {
      cancelled = true;
    };
  }, [previewMode, projectPath]);

  function updateProvider(field: keyof CustomProviderConfig, value: string | boolean) {
    setProviderConfig((current) => current ? { ...current, [field]: value } : current);
    setProviderMessage("");
  }

  async function saveProvider() {
    if (!projectPath || !providerConfig) return;
    setProviderSaving(true);
    setProviderMessage("");
    try {
      const input = { ...providerConfig, api_key: providerKey.trim() || undefined };
      if (previewMode) {
        await new Promise((resolve) => setTimeout(resolve, 300));
        setProviderKey("");
        setProviderMessage("预览模式：已模拟保存第三方 Provider。");
        return;
      }
      const policy = confirmedAgentManagedPolicy();
      const plan = buildKernelPlanForInvoke("save_custom_provider_config", { input });
      const decisionToken = plan
        ? normalizePlanReviewResult(await planKernelCommand({
            command: plan.command,
            policy,
            projectPath,
            payload: plan.payload,
          })).decisionToken
        : undefined;
      const saved = await saveCustomProviderConfig({
        projectPath,
        input,
        confirmedPolicy: policy,
        decisionToken,
      });
      setProviderConfig(saved);
      setProviderStatus(await testProviderStatus(projectPath, false));
      setSetupChecklist(await getSetupChecklist(projectPath));
      if (saved.enabled) onSynthesisEngineChange("llm");
      setProviderKey("");
      setProviderMessage("第三方 Provider 已保存，并已切换默认整理引擎为 LLM；SK 仅写入当前桌面进程环境。");
    } catch (error) {
      setProviderMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setProviderSaving(false);
    }
  }

  async function testProvider() {
    if (!projectPath || providerTesting) return;
    setProviderTesting(true);
    setProviderMessage("");
    try {
      if (previewMode) {
        await new Promise((resolve) => setTimeout(resolve, 300));
        setProviderStatus({
          status: "pass",
          provider: "custom-api",
          protocol: providerConfig?.protocol ?? "openai-compatible",
          base_url: providerConfig?.base_url ?? "https://api.openai.com/v1",
          model: providerConfig?.model ?? "gpt-4.1-mini",
          api_key_env: providerConfig?.api_key_env ?? "OPENAI_API_KEY",
          proxy: null,
          checks: [{ label: "Live request", status: "pass", detail: "预览模式：Provider 测试通过。" }],
          next_actions: [],
        });
        setProviderMessage("预览模式：Provider 测试通过。");
        return;
      }
      const report = await testProviderStatus(projectPath, true);
      setProviderStatus(report);
      setProviderMessage(report.status === "pass" ? "Provider 测试通过。" : "Provider 需要处理下面的下一步。");
    } catch (error) {
      setProviderMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setProviderTesting(false);
    }
  }

  return (
    <div className="stack">
      <Panel title="提炼引擎" subtitle="用于把候选内容改写成成熟 Memory Card，并在 Skill 融合时做最终精修。" icon={CloudCog}>
        <div className="settings-form">
          <div className="settings-summary-line">
            <span>当前项目：{projectPath || snapshot?.project_path || "未选择"}</span>
            <strong>默认引擎：{synthesisEngine === "llm" ? "LLM Provider" : formatAgent(synthesisEngine)}</strong>
          </div>
          <label className="toggle-row">
            <input
              type="checkbox"
              checked={providerConfig?.enabled ?? false}
              onChange={(event) => updateProvider("enabled", event.target.checked)}
              disabled={!providerConfig}
            />
            <span>启用第三方 Provider 作为默认提炼引擎</span>
          </label>
          {providerDisabledReason ? <p className="settings-hint">{providerDisabledReason}</p> : null}
          <div className="settings-grid">
            <label>
              <span>协议</span>
              <select
                value={providerConfig?.protocol ?? "openai-compatible"}
                onChange={(event) => updateProvider("protocol", event.target.value)}
                disabled={!providerConfig}
              >
                <option value="openai-compatible">OpenAI-compatible</option>
                <option value="anthropic-compatible">Anthropic-compatible</option>
              </select>
            </label>
            <label>
              <span>Base URL</span>
              <input
                type="url"
                value={providerConfig?.base_url ?? ""}
                onChange={(event) => updateProvider("base_url", event.target.value)}
                placeholder={providerConfig?.protocol === "anthropic-compatible" ? "https://api.deepseek.com/anthropic" : "https://api.deepseek.com"}
              />
            </label>
            <label>
              <span>模型</span>
              <input
                type="text"
                value={providerConfig?.model ?? ""}
                onChange={(event) => updateProvider("model", event.target.value)}
                placeholder="deepseek-chat"
              />
            </label>
            <label>
              <span>SK 环境变量名</span>
              <input
                type="text"
                value={providerConfig?.api_key_env ?? ""}
                onChange={(event) => updateProvider("api_key_env", event.target.value)}
                placeholder="OPENAI_API_KEY"
              />
            </label>
            <label>
              <span>SK（仅本次运行）</span>
              <input
                type="password"
                value={providerKey}
                onChange={(event) => setProviderKey(event.target.value)}
                placeholder="留空则沿用现有环境变量"
                autoComplete="off"
              />
            </label>
          </div>
          <div className="settings-actions">
            <button
              className="primary-action"
              disabled={Boolean(providerDisabledReason) || providerSaving}
              onClick={() => void saveProvider()}
            >
              {providerSaving ? <Loader2 className="spin" size={15} /> : providerMessage.startsWith("第三方") ? <Check size={15} /> : <Save size={15} />}
              {providerSaving ? "保存中" : "保存 Provider"}
            </button>
            <button className="secondary-action" disabled={!projectPath || providerTesting} onClick={() => void testProvider()}>
              {providerTesting ? <Loader2 className="spin" size={15} /> : <TestTube2 size={15} />}
              {providerTesting ? "测试中" : "测试 Provider"}
            </button>
            <button className="ghost-action" type="button" onClick={() => setShowDiagnostics((current) => !current)}>
              <RefreshCw size={15} />
              {showDiagnostics ? "收起诊断" : "高级诊断"}
            </button>
            {providerMessage ? <span>{providerMessage}</span> : null}
          </div>
          {providerStatus && (showDiagnostics || providerStatus.status !== "pass") ? (
            <div className={`provider-status ${providerStatus.status}`}>
              <div className="provider-status-head">
                <strong>{providerStatus.provider}</strong>
                <span>{providerStatus.protocol} / {providerStatus.model}</span>
              </div>
              <div className="setup-checklist compact">
                {providerStatus.checks.map((item) => (
                  <ChecklistRow key={`${item.label}:${item.detail}`} item={item} />
                ))}
              </div>
              {providerStatus.next_actions.length > 0 ? (
                <div className="provider-next-actions">
                  {providerStatus.next_actions.map((action) => <p key={action}>{action}</p>)}
                </div>
              ) : null}
            </div>
          ) : null}
          {showDiagnostics ? (
            <div className="settings-diagnostics">
              <div className="settings-diagnostic-meta">
                <span>主目录：{state?.home ?? "读取中"}</span>
                <span>扫描根：{(state?.scan_roots ?? []).length} 个</span>
              </div>
              <div className="setup-checklist compact">
                {(setupChecklist?.items ?? fallbackChecklist(projectPath, previewMode)).map((item) => (
                  <ChecklistRow key={item.label} item={item} />
                ))}
              </div>
            </div>
          ) : null}
        </div>
      </Panel>

      <Panel title="外观主题" subtitle="切换浅色/暗色或跟随系统自动切换。" icon={Monitor}>
        <div className="theme-switcher" role="radiogroup" aria-label="主题切换">
          {THEME_OPTIONS.map((option) => (
            <button
              key={option.key}
              className={theme === option.key ? "active" : ""}
              role="radio"
              aria-checked={theme === option.key}
              onClick={() => onThemeChange(option.key)}
            >
              <option.icon size={15} />
              {option.label}
            </button>
          ))}
        </div>
      </Panel>

    </div>
  );
}

function ChecklistRow({ item }: { item: SetupChecklistItem }) {
  const Icon = item.status === "pass" ? Check : AlertTriangle;
  return (
    <div className={`setup-check-row ${item.status}`}>
      <Icon size={15} />
      <div>
        <strong>{item.label}</strong>
        <p>{item.detail}</p>
        {item.next_action ? <span>{item.next_action}</span> : null}
      </div>
    </div>
  );
}

function fallbackChecklist(projectPath: string, previewMode: boolean): SetupChecklistItem[] {
  if (!projectPath && !previewMode) {
    return [{
      label: "Project",
      status: "warn",
      detail: "先选择一个项目，设置页会显示首跑检查和 Provider 诊断。",
      next_action: "从左侧项目列表选择或扫描一个本地项目。",
    }];
  }
  return [{
    label: "Runtime",
    status: "warn",
    detail: "正在读取首跑检查。",
    next_action: "稍等片刻，或重新选择项目刷新状态。",
  }];
}
