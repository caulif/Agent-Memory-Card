# MASTER CONTROL INDEX - JAPANDI REDESIGN

## 1. 物理重构元状态
*   **Project Name**: Japandi Warm Minimalist & Singular Functionality Focus (白瓷冰川日系侘寂与单页单功能重设计)
*   **Tracking Mode**: `LOCAL_ONLY`
*   **Active Branch**: `style/warm-minimalist-redesign`
*   **Repository status**: `Operational // Clean and Validated`
*   **Supervisor Skill Installed**: `.agents/skills/spec-driven-develop/`

---

## 2. 里程碑与阶段完成进度追踪
*   [x] **Phase 1: 物理样式参数重构与暖黏土高光调优 (3/3 tasks)**
    *   *MileStone 1*: 常见底色、顺滑大曲线圆角与散射投影 CSS 变量落地 -> **[Completed]**
*   [x] **Phase 2: React 页面组件大减法重构与交互解耦 (3/3 tasks)**
    *   *MileStone 2*: 全页面编译与 0 阻断 Vite 运行集成真机校准 -> **[Completed]**

---

## 3. 已物理实施的最终编译修补历史 (HMR Telemetry Logs)
1.  **🐞 Error #01 (Agents.tsx JSX Close Match)**:
    *   *诊断*: Equipped 列表映射中，按钮开始标签 `<button>` 内部闭合为了 `</div>`。
    *   *物理修复*: 将闭合标签修正为 `</button>`。Oxc 编译恢复。
2.  **🐞 Error #02 (Skills.tsx Inline Style Hyphen)**:
    *   *诊断*: `style={{ ... }}` JSX Inline styles 中错塞原生 `white-space` 键名导致 Vite transform failed。
    *   *物理修复*: 驼峰式改写为 `whiteSpace: "nowrap"`。Vite 阻断解除。

---

## 4. 后续调试与交付成果查阅
*   **视觉与交互原型主入口**: `app/preview_redesign.html`
*   **技术规约全套卷卷**: 
    1.  `docs/analysis/project-overview.md` (物理技术栈结构)
    2.  `docs/analysis/module-inventory.md` (S.U.P.E.R compliance scoring 审计)
    3.  `docs/analysis/risk-assessment.md` (风险拦截与编译热点)
    4.  `docs/plan/task-breakdown.md` (优先执行路线)
    5.  `docs/plan/dependency-graph.md` (Mermaid lanes 网格)
    6.  `docs/plan/milestones.md` (MS 标定成果)

---

## 5. 2026-05-20 页面专注重构续跑记录
*   **Scope**: Drafts / MemoryCards / Agents / Skills 四大核心页继续按“每页只做一件事”减法重构。
*   **Prototype**: 已由 Claude Code 重写 `app/preview_redesign.html` 为四页高保真原型，并经 Playwright 截图审查。
*   **Production Changes**:
    *   `Drafts.tsx`: 收束为 Review Inbox，两栏候选/证据审阅，移除 eval、批量、编辑表单、诊断芯片和已审流。
    *   `MemoryCards.tsx`: 收束为 Library Index，治理信息隐藏到轻量诊断开关。
    *   `Agents.tsx`: 收束为 Loadout Slots，保留卡池与智能体接收槽，移除矩阵/重读/同步大动作噪音。
    *   `Skills.tsx`: 收束为 Skills Directory，移除 Memory Card 只读绑定与构建同步杂讯。
    *   `main.tsx`: 对四个专注页隐藏 ProjectOverviewStrip 与右侧 inspector。
*   **Guardrail**: 新增 `app/scripts/verify-ui-focus-contract.mjs`，并挂载 `bun run --cwd app verify-ui`。
*   **Verification**:
    *   `bun run --cwd app verify-ui` -> PASS
    *   `bun run --cwd app build` -> PASS
    *   Playwright production screenshots captured under `output/playwright/production-*-final.png`
*   **GitHub Evidence**:
    *   Issue #32 commented with Drafts implementation evidence.
    *   Issue #33 commented with MemoryCards implementation evidence.
    *   Issue #34 commented with Agents / Skills implementation evidence.

---

## 6. 2026-05-20 Skills 赋能闭环复审记录
*   **Intent**: 复查“提炼结果是否能真正赋能 Skills”，避免 Skills Directory 退化为只读字典。
*   **Finding**: 后端读模型已提供 `linked_memory_cards` / `recommended_memory_cards`，且 Tauri 已支持 `attach_memory_card_to_skill`；前端上一轮减法后没有给用户一条从已批准 Memory Card 到常用 Skill 补强的轻量操作路径。
*   **Production Changes**:
    *   `Skills.tsx`: 在所选 Skill 详情中新增“已纳入上下文 / 可采纳的优化建议”，保留字典检索心智，同时允许一键把推荐 Memory Card 纳入 Skill supplement。
    *   `main.tsx`: 向 Skills Directory 传入轻量 action dispatcher，不恢复旧的质量看板、同步动作或只读大面板。
    *   `project-detail.css`: 修复 Skills 页面桌面/移动端响应式布局，移动端无横向溢出。
*   **Verification**:
    *   `bun run --cwd app verify-ui` -> PASS
    *   `bun test --cwd app ./src/utils/review-workbench.test.ts ./src/utils/kernel-plan.test.ts` -> PASS
    *   `bun run --cwd app build` -> PASS
    *   `cargo test --manifest-path src-tauri/Cargo.toml skill` -> PASS
    *   Playwright desktop/mobile checks show Skills loop visible, no console errors, no horizontal overflow.
