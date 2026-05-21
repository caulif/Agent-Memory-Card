# Task Decomposition Blueprint

## 1. 物理重构概览
*   **Total Phases**: 3
*   **Total Tasks**: 6
*   **Estimated Total Effort**: Large (L)
*   **Globally S.U.P.E.R Focus**: S (Single Purpose — 一页一事), P (Ports over Implementation — 接口清爽)

## 2. 重构执行分解

### Phase 1: 物理样式参数重构与高光调优 (CSS Surface Styling)
*   **prerequisite**: 无
*   **S.U.P.E.R Focus**: R (Replaceable Parts — 全局变量替换)

| # | 任务标题 | 优先级 | 工作量 | 依赖项 | Lane | 核心驱动 | 验收标准 |
|:--|:---|:---|:---|:---|:---|:---|:---|
| 1 | `styles.css` 明亮燕麦色变量注入 | P0 | S | — | A | R | 画布底色变为 `#FAF8F5` |
| 2 | `themes.css` 生泥炭高级陶灰暗色洗练 | P1 | S | — | B | R | 自动黑暗模式变为泥灰 `#1C1917` |
| 3 | `project-detail.css` 宣纸会话比对与黏土高亮 | P1 | M | 1 | A | S | 证据舱高亮背景变为超低对比暖橙 |

---

### Phase 2: React 交互逻辑大减法重构与解耦 (React Singularity Focus)
*   **prerequisite**: Phase 1
*   **S.U.P.E.R Focus**: S (Single Purpose), P (Ports)

| # | 任务标题 | 优先级 | 工作量 | 依赖项 | Lane | 核心驱动 | 验收标准 |
|:--|:---|:---|:---|:---|:---|:---|:---|
| 4 | `Drafts.tsx` 页面已审流与 ML 看板清洗 | P0 | M | 3 | A | S | 极其清爽的左收件、右详查 triage 双栏布局 |
| 5 | `MemoryCards.tsx` 分组手册与齿轮治理开发 | P0 | M | 4 | A | S | 默认静止手册，展开治理隐形显示 |
| 6 | `Agents.tsx` & `Skills.tsx` 冲突矩阵及只读卡片清除 | P0 | L | 5 | B | P | 废除二维矩阵，仅余高内聚技能目录与极简拖配 |
