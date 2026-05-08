import React from "react";
import { CloudCog, Monitor, Moon, RefreshCw, Sun } from "lucide-react";
import { Panel, StatusList } from "../common";
import {
  formatAgent,
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
  synthesisEngine,
  theme,
  onThemeChange,
}: {
  state: DesktopAppState | null;
  snapshot: ProjectSnapshot | null;
  projectPath: string;
  previewMode: boolean;
  synthesisEngine: "claude-code" | "codex" | "local" | "llm";
  theme: "light" | "dark" | "system";
  onThemeChange: (theme: "light" | "dark" | "system") => void;
}) {
  return (
    <div className="stack">
      <Panel title="运行环境" subtitle="所有读取和生成都在本机完成。" icon={CloudCog}>
        <StatusList
          items={[`主目录：${state?.home ?? "读取中"}`, `当前项目：${snapshot?.project_path ?? "未选择"}`, `默认整理引擎：${synthesisEngine === "llm" ? "LLM" : formatAgent(synthesisEngine)}`]}
          empty="暂无环境信息。"
        />
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
