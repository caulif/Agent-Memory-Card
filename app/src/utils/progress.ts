import type { TaskProgress, DesktopTaskStatus } from "../types/domain";

export function taskProgressForAction(actionKey: string): TaskProgress {
  if (!actionKey) {
    return { label: "就绪", percent: 100, description: "当前没有正在执行的后台任务" };
  }

  const progressMap: Record<string, TaskProgress> = {
    "启动": {
      label: "建立本地记忆索引",
      percent: 25,
      description: "正在扫描桌面、文档和代码仓库目录，建立项目索引...",
    },
    "扫描": {
      label: "扫描本地项目",
      percent: 40,
      description: "正在递归扫描根目录，发现 AGENTS.md / CLAUDE.md 等项目标记...",
    },
    "整理历史": {
      label: "提炼高价值 Skilllet",
      percent: 55,
      description: "正在读取 Claude Code / Codex 历史对话，过滤一次性任务，保留稳定偏好和硬约束...",
    },
    "同步": {
      label: "同步生成产物",
      percent: 70,
      description: "正在将已批准的技能片段写入目标智能体的配置文件...",
    },
    "融合Skilllet": {
      label: "生成融合草稿",
      percent: 68,
      description: "正在把多个 Skilllet 合并为一个待审草稿，原片段会保留不变...",
    },
    "切换项目": {
      label: "切换项目快照",
      percent: 35,
      description: "正在加载目标项目的草稿、技能片段和分配矩阵...",
    },
  };

  return (
    progressMap[actionKey] ?? {
      label: actionKey,
      percent: 30,
      description: `正在执行：${actionKey}...`,
    }
  );
}

export function resolveTaskProgress(actionKey: string, backendStatus?: DesktopTaskStatus): TaskProgress {
  if (backendStatus?.running) {
    return {
      label: backendStatus.label,
      percent: backendStatus.percent,
      description: backendStatus.description,
    };
  }
  return taskProgressForAction(actionKey);
}
