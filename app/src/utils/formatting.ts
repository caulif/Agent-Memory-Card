import type { DesktopJobStart } from "../types/domain";

const agentLabels: Record<string, string> = {
  codex: "Codex",
  "claude-code": "Claude Code",
};

const kindLabels: Record<string, string> = {
  rule: "规则",
  memory_card: "Memory Card",
  observation: "观察",
  package: "条目",
  preference: "偏好",
  constraint: "约束",
  procedure: "流程",
  convention: "约定",
};

const scopeLabels: Record<string, string> = {
  project: "项目",
  global: "全局",
  agent: "智能体",
};

export function formatAgent(agent: string) {
  return agentLabels[agent] ?? agent;
}

export function formatAgents(agents: string[]) {
  if (agents.length === 0) return "未配置智能体";
  return agents.map(formatAgent).join(" / ");
}

export function translateKind(kind: string) {
  return kindLabels[kind] ?? kind;
}

export function translateScope(scope: string) {
  return scopeLabels[scope] ?? scope;
}

export function isChineseLabel(label: string) {
  return /\p{Script=Han}/u.test(label);
}

export function formatJobLifecycle(lifecycle?: string): string {
  const labels: Record<string, string> = {
    idle: "就绪",
    running: "运行中",
    cancelling: "取消中",
    completed: "已完成",
    failed: "失败",
    cancelled: "已取消",
  };
  return labels[lifecycle ?? ""] ?? "未知";
}

export function formatJobStartMessage(job: DesktopJobStart): string {
  if (!job.accepted) {
    return `${job.key}未能加入后台：${job.message}`;
  }
  return `${job.key}已加入后台：${job.message}（任务 ${job.job_id}）`;
}
