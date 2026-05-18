import React from "react";
import { Check, CloudCog, Loader2, Monitor, Moon, RefreshCw, Save, Sun } from "lucide-react";
import { Panel, StatusList } from "../common";
import { getCustomProviderConfig, planKernelCommand, saveCustomProviderConfig } from "../../tauri-client";
import {
  buildKernelPlanForInvoke,
  confirmedAgentManagedPolicy,
  formatAgent,
  normalizePlanReviewResult,
  type CustomProviderConfig,
  type DesktopAppState,
  type ProjectSnapshot,
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
  const [providerMessage, setProviderMessage] = React.useState("");
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
    void getCustomProviderConfig(projectPath)
      .then((config) => {
        if (!cancelled) setProviderConfig(config);
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
      if (saved.enabled) onSynthesisEngineChange("llm");
      setProviderKey("");
      setProviderMessage("第三方 Provider 已保存，并已切换默认整理引擎为 LLM；SK 仅写入当前桌面进程环境。");
    } catch (error) {
      setProviderMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setProviderSaving(false);
    }
  }

  return (
    <div className="stack">
      <Panel title="运行环境" subtitle="所有读取和生成都在本机完成。" icon={CloudCog}>
        <StatusList
          items={[`主目录：${state?.home ?? "读取中"}`, `当前项目：${projectPath || snapshot?.project_path || "未选择"}`, `默认整理引擎：${synthesisEngine === "llm" ? "LLM" : formatAgent(synthesisEngine)}`]}
          empty="暂无环境信息。"
        />
      </Panel>

      <Panel title="第三方 Provider" subtitle="用于提炼、抽象、评审和精修候选的 OpenAI-compatible API。" icon={CloudCog}>
        <div className="settings-form">
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
            {providerMessage ? <span>{providerMessage}</span> : null}
          </div>
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

      <Panel title="扫描来源" subtitle="除了常用目录，还会读取 Claude Code / Codex 的本地历史索引来发现项目。" icon={RefreshCw}>
        <StatusList
          items={[...(state?.scan_roots ?? []), "~/.claude/history.jsonl", "~/.claude/projects", "~/.codex/history.jsonl", "~/.codex/sessions"]}
          empty="暂无扫描目录。"
        />
      </Panel>
    </div>
  );
}
