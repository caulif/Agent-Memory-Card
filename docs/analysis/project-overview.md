# Project Overview - UI & UX Overhaul

## 1. 物理技术架构与技术栈
本项目是一个基于 **Tauri (Rust) + React (TypeScript) + Vite** 构建的前端桌面工作台应用。
*   **前端核心**: React 18, Vite (以 Oxc 代码变换插件 `plugin:vite:oxc` 加速打包构建)。
*   **交互设计**: Lucide-React 图标库, postcss 进行全局/局部 CSS 面板与高层主题变量控制。
*   **入口文件**: `app/index.html` -> `/src/main.tsx`。

## 2. 目录布局与结构体系
*   `app/src/main.tsx`: 桌面工作台整体 App 与状态控制枢纽。
*   `app/src/components/project-pages.tsx`: 页面组件懒分发及骨架导出模块。
*   `app/src/components/pages/`:
    *   `Drafts.tsx`: 提炼建议 Triaging triage 页面。
    *   `MemoryCards.tsx`: 项目已批准 Memory Card 手册管理区。
    *   `Agents.tsx`: 记忆条例在智能体间（Codex, Claude Code）的主装配网格面板。
    *   `Skills.tsx`: 系统已注册技能接口只读查询辞典。
*   `app/src/styles/`: 全套 PostCSS 样式层定义：
    *   `themes.css`: 全局自适应/手动亮暗颜色变量控制区。
    *   `styles.css`: 侧边栏、控制大带、按钮体系及主内容池公共样式。
    *   `project-detail.css`: 面板内部、提取高亮、证据舱以及插槽的局部偏置样式。
