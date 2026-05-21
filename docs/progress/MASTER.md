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

---

## 7. 2026-05-20 自主试用与提炼质量复核记录
*   **Intent**: 按真实用户路径复查四页是否“一秒看懂，一眼即操作”，并重点验证提炼结果是否能转化为可用 Skill 补强。
*   **User Trial Findings**:
    *   Drafts / MemoryCards / Agents / Skills 四页在桌面和移动端均无横向溢出、无浏览器 console error。
    *   Skills 初版点击“纳入此 Skill”后只更新顶部提示，建议项没有从“可采纳”移动到“已纳入”，用户无法确认动作完成。
    *   Skills 详情头部在桌面端横向挤压，长 Skill 名会过早断行，影响辞典阅读感。
    *   GitHub PR #35 Windows CI 失败并非 UI 编译问题，而是被源码引用的 Rust 外置测试文件被 `.gitignore` 忽略，导致 CI checkout 后 `cargo fmt --check` 找不到 `src/build/tests.rs`。
    *   追踪测试文件后，CI 继续暴露历史 Rust clippy debt；本轮按机械方式收口 lint，不改变提炼语义。
    *   完整 Tauri 测试在 Windows CI 上暴露 8.3 短路径差异：`runneradmin` 与 `RUNNER~1` 表示同一临时目录但字符串断言失败。
*   **Production Changes**:
    *   `Skills.tsx`: 增加轻量本地乐观合并，点击推荐 Memory Card 后立即把条目移动到“已纳入上下文”，保持 Skills Directory 的单页心智但补全使用闭环。
    *   `project-detail.css`: 将 Skill 详情头部改为纵向 grid，避免标签、标题、描述互相挤压。
    *   `.gitignore`: 精确白名单入库被 Rust 模块引用的测试文件，恢复 CI 可复现性。
    *   Rust core: 派生默认值、去除不必要 clone、替换连续 replace 与 Option::map(unit)，让 CI 的 `cargo clippy -- -D warnings` 可通过。
    *   Tauri tests: 将项目路径断言改为规范化路径比较，兼容 Windows CI 的短路径展开。
*   **Extraction Quality Evidence**:
    *   `cargo test --test extract_quality_v2 --quiet` -> PASS, 21 passed / 1 ignored.
    *   `cargo test --test golden_set_regression --quiet` -> PASS.
    *   Positive dry-run: “Memory Card 能补强常用 Skill，要在 Skills 页面给出一键纳入路径” 被归类为 `workflow_skill`, `activation=skill`, `target:skill-dir`。
    *   Negative dry-run: “关掉窗口/今天不继续改” 一次性操作未生成 draft。
*   **Verification**:
    *   `bun run --cwd app verify-ui` -> PASS
    *   `bun test --cwd app ./src/utils/review-workbench.test.ts ./src/utils/kernel-plan.test.ts` -> PASS
    *   `bun run --cwd app build` -> PASS
    *   `cargo fmt --check` -> PASS
    *   `cargo clippy -- -D warnings` -> PASS
    *   `cargo test --manifest-path Cargo.toml --lib` -> PASS, 303 passed.
    *   `cargo test --manifest-path src-tauri/Cargo.toml -- --nocapture` -> PASS, 44 passed.
    *   Playwright user trial report: `output/playwright/ui-trial-report.json`

---

## 8. 2026-05-20 真实 Skill Registry 与成熟卡片闭环修正
*   **Issue**: GitHub #36 tracks the follow-up from real user trial.
*   **User Findings Addressed**:
    *   Drafts 页之前没有可见 LLM 引擎选项，且主按钮硬编码 `claude-code`。
    *   Skills 页只读取旧 `.agent-kernel/skill-index.yml`，没有像 Codex 一样刷新真实本地 Skills 的入口。
    *   Memory Card 审阅/挂载应展示成熟规则，重复或相似内容应合并，而不是让用户批准粗糙候选和重复卡片。
*   **Production Changes**:
    *   `Drafts.tsx`: 增加极简提炼引擎选择器，支持 `LLM Provider / Claude Code / Codex / Local`，主提炼动作使用用户选择的 engine。
    *   `main.tsx`: 项目切换时读取 Provider 配置；若第三方 Provider 已启用，默认切到 `llm`。
    *   `scanner.rs`: home scan 扩展到 Codex superpowers 与 Codex plugin cache；本项目 `import --scan-home` 实测索引 57 个 Skills。
    *   `Skills.tsx`: 增加“扫描真实 Skills”；Skill read model 显示扫描时间与来源计数，保留字典心智。
    *   Skill 挂载：新增 `manual` 直接挂载与 `auto` LLM/确定性融合 supplement；构建时写入融合后的 `AGENT_KERNEL_MEMORY_CARDS.md` 内容。
    *   Candidate 批准：当提炼动作标记 `merge_into_existing` 时，批准会更新既有 Memory Card，不再新增重复卡。
*   **Verification**:
    *   `bun run --cwd app verify-ui` -> PASS
    *   `bun test --cwd app ./src/utils/kernel-plan.test.ts` -> PASS
    *   `bun run --cwd app build` -> PASS
    *   `cargo fmt --check` -> PASS
    *   `cargo clippy -- -D warnings` -> PASS
    *   `cargo test --manifest-path Cargo.toml --lib` -> PASS, 304 passed.
    *   `cargo test --manifest-path src-tauri/Cargo.toml -- --nocapture` -> PASS, 45 passed.
    *   `cargo test --test extract_quality_v2 --quiet` -> PASS, 21 passed / 1 ignored.
    *   `cargo test --test golden_set_regression --quiet` -> PASS, 3 passed.
    *   `agent-kernel import --project . --scan-home` -> indexed 57 Skills, including 6 plugin and 28 superpowers entries.
    *   Playwright preview trial -> no console errors, no desktop/mobile horizontal overflow; Drafts engine options and Skills scan/manual/auto-fuse controls visible.

---

## 9. 2026-05-20 口语候选成熟化与项目级 Skill 装填修正
*   **Trigger**: 用户截图反馈已吸纳 Memory Card 仍像口语摘录，MemoryCards 布局看不完整，Skills 未区分项目级/全局，融合动作缺少取消，Agents 拖动不可用，Settings 信息冗余。
*   **Production Changes**:
    *   `candidate.rs`: 候选批准前新增成熟 Memory Card 审核改写关卡；存在 Provider 配置时调用 refine LLM 输出成熟 `title/body/brief/tags`，无 Provider 时用确定性 `触发 / 动作 / 边界` 兜底，避免口语候选直接落库。
    *   `Drafts.tsx`: Review Inbox 展示成熟卡片拟案，用户审核时看到的是可执行规则而不是原始聊天摘录。
    *   `MemoryCards.tsx`: Library 卡片改为两列自适应读本布局，隐藏聊天式 brief，正文完整换行展示，并标记需要精修的旧卡。
    *   `Skills.tsx`: 增加项目级 / 全局 Skills 分层；Memory Card 只融合到项目级 Skill；推荐项先选择优化方式，再显示直接纳入 / LLM 融合 / 取消。
    *   `Agents.tsx`: Loadout 只面向项目级 Memory Card；新增点选卡片再装入智能体的鼠标路径，拖拽仅作为辅助；修复移动端两栏被内联宽度顶住的问题。
    *   `Settings.tsx`: 收束为提炼引擎与外观主题，运行环境、扫描根和 checklist 默认折叠到高级诊断。
    *   `app_service.rs`: 自动 Skill 融合提示词与兜底输出改为更接近 SKILL.md 的 Use when / Instructions / Boundaries 风格。
*   **Verification**:
    *   `bun run --cwd app verify-ui` -> PASS
    *   `bun run --cwd app build` -> PASS
    *   `bun test --cwd app ./src/utils/kernel-plan.test.ts` -> PASS
    *   `cargo fmt --check` -> PASS
    *   `cargo clippy -- -D warnings` -> PASS
    *   `cargo test --manifest-path Cargo.toml --lib` -> PASS, 304 passed.
    *   `cargo test --manifest-path src-tauri/Cargo.toml -- --nocapture` -> PASS, 45 passed.
    *   Playwright preview trial -> no console errors; Drafts 成熟拟案、MemoryCards 读本布局、Skills 项目/全局分层与取消路径、Agents 点选装填、Settings 高级诊断折叠均可见；桌面 5 页与移动 Agents 均无横向溢出。

---

## 10. 2026-05-20 价值导向 Memory Card Synthesis 实施记录
*   **Issue**: GitHub #38 tracks the implementation.
*   **Spec**:
    *   `docs/analysis/memory-card-synthesis-agent-design.md`
    *   `docs/plan/memory-card-synthesis-implementation-plan.md`
*   **Goal**: 按 “No Card Is A Success / Value Delta / Skill-targeted Memory Card” 设计，把候选成熟化从聊天摘要升级为能说明目标、缺口和未来行为改进的 Memory Card synthesis。
*   **Current Status**:
    *   [x] 设计文档完成并收束产品概念为 Memory Card。
    *   [x] GitHub issue #38 创建。
    *   [x] Phase 1: 元数据契约。
    *   [x] Phase 2: 价值导向 synthesis。
    *   [x] Phase 3: Drafts / Skills review UX。
    *   [x] Phase 4: 验证、提交、推送与 issue 更新。
*   **Implementation Notes**:
    *   `ExtractionMetadata` 新增 `card_function / value_claim / value_delta / target_context / synthesis_trace`，保持旧 YAML 默认兼容。
    *   候选批准前成熟化改为 value-directed prompt；确定性 fallback 也会生成 Value Delta 与 trace。
    *   Merge 候选批准后会把新 synthesis metadata 写回既有 Memory Card，避免重复新增。
    *   Drafts 显示 Value Delta、目标上下文和 compact trace；Skills 的 Memory Card mini preview 显示 Skill-targeted Before / After。
    *   参照 `earendil-works/pi` 的 agent-core 思路，当前先落状态/trace/stop-reason 兼容契约，后续 runtime 不手搓裸 loop。
*   **Verification So Far**:
    *   `cargo test --manifest-path Cargo.toml candidate::tests --lib` -> PASS, 9 passed.
    *   `cargo test --manifest-path Cargo.toml --lib` -> PASS, 306 passed.
    *   `cargo fmt --check` -> PASS.
    *   `cargo clippy -- -D warnings` -> PASS.
    *   `cargo test --manifest-path src-tauri/Cargo.toml -- --nocapture` -> PASS, 45 passed.
    *   `cargo test --test extract_quality_v2 --quiet` -> PASS, 21 passed / 1 ignored.
    *   `cargo test --test golden_set_regression --quiet` -> PASS, 3 passed.
    *   `bun run --cwd app verify-ui` -> PASS.
    *   `bun test --cwd app ./src/utils/review-workbench.test.ts ./src/utils/kernel-plan.test.ts` -> PASS, 28 passed.
    *   `bun run --cwd app build` -> PASS.
    *   Playwright smoke on `http://127.0.0.1:1420` -> no console errors, no horizontal overflow; Skill-targeted Before / After visible.
    *   PR #35 Windows CI for commit `a2bab81` -> PASS, 2/2 jobs.
*   **Agent Runtime Constraint Update**:
    *   `docs/analysis/memory-card-synthesis-agent-design.md` now records `earendil-works/pi` as the mature runtime reference for future agent work.
    *   `docs/plan/memory-card-synthesis-implementation-plan.md` now requires an explicit build-or-adapt decision before implementing a full synthesis agent runtime.
    *   Future runtime work should embed/wrap `@earendil-works/pi-agent-core` where practical, or port its stateful session, context transform, typed read-only tools, hooks, event stream, stop-condition, trace, and progressive Skill-loading contracts.

---

## 11. 2026-05-20 Tool-backed Memory Card Synthesis Runtime Foundation
*   **Issue**: GitHub #39 tracks the continuation from metadata-only synthesis to a production runtime foundation.
*   **Goal**: 让 Memory Card synthesis 在批准前先具备全局视野：读取相关 observations、既有 Memory Cards、项目/全局 Skills 和 writing guide，输出可审阅 trace、stop reason、Value Delta 与目标上下文。
*   **Production Changes**:
    *   `src/synthesis_agent.rs`: 新增 Rust core synthesis runtime foundation，包含 `SynthesisReview`、typed read-only tool events、context pack、proposal、stop reason 和 compact trace。
    *   `src/candidate.rs`: 候选成熟化前运行 synthesis review；Provider prompt 接收 `synthesis_context`；无 Provider 时 deterministic fallback 也使用同一份 value metadata。
    *   `src/candidate.rs` / `src-tauri/src/app_service.rs`: Review Inbox 读取候选时即做只读 synthesis preview enrichment，让用户批准前就能看到成熟 Memory Card 预览、Value Delta、目标上下文和 trace。
    *   `src/synthesis_agent.rs` / `Drafts.tsx`: runtime 显式区分 `already_covered / merge_card / skill_targeted_card / workflow_card / needs_human`；已覆盖候选在 UI 中走“No Card Is A Success”归档路径，而不是诱导新增重复卡。
    *   `src/candidate.rs`: 若 runtime 识别近重复项目 Memory Card，批准时自动走 `merge_into_existing`；若识别已充分覆盖，则保留为 review-only 归档建议。
    *   `src/lib.rs`: 暴露 `synthesis_agent` 模块供后续 Tauri/CLI/UI read model 复用。
*   **Implemented Tools**:
    *   `search_observations`
    *   `search_memory_cards`
    *   `search_skills`
    *   `read_writing_guide`
    *   `find_memory_duplicates`
    *   `compare_with_skill`
*   **Boundaries**:
    *   runtime 只读，不写入文件、不改 project config。
    *   项目级 Skills 可作为目标；全局 Skills 仅作参考。
    *   真正 provider tool-call loop、Web search/read 和 embedding search 暂未混入核心写路径，后续复用同一 `SynthesisReview` 合约替换 planner。
*   **Verification So Far**:
    *   `cargo test --manifest-path Cargo.toml synthesis_agent --lib` -> PASS, 4 passed.
    *   `cargo test --manifest-path Cargo.toml candidate::tests::visible_candidate_inbox --lib` -> PASS, 2 passed.
    *   `cargo test --manifest-path Cargo.toml --lib` -> PASS, 312 passed.
    *   `cargo test --manifest-path src-tauri/Cargo.toml -- --nocapture` -> PASS, 45 passed.
    *   `cargo test --test extract_quality_v2 --quiet` -> PASS, 21 passed / 1 ignored.
    *   `cargo test --test golden_set_regression --quiet` -> PASS, 3 passed.
    *   `cargo fmt --check` -> PASS.
    *   `cargo clippy -- -D warnings` -> PASS. Local run emitted rustc incremental-cache corruption warnings; they were non-lint warnings and did not fail clippy.
    *   `bun run --cwd app verify-ui` -> PASS.
    *   `bun test --cwd app ./src/utils/review-workbench.test.ts ./src/utils/kernel-plan.test.ts` -> PASS, 28 passed.
    *   `bun run --cwd app build` -> PASS.

---

## 12. 2026-05-21 CI 与用户路径质量守门补强
*   **Trigger**: PR #35 最新 head 的两条 Windows CI run 因 `push` 与 `pull_request` 双触发并行，且重型 Tauri 阶段无显式 timeout，长期停留在 `in_progress`，导致 PR 状态无法稳定收口。
*   **Reference Check**:
    *   GitHub Actions 官方 workflow syntax 支持 `concurrency.cancel-in-progress`，可取消同组旧 run。
    *   GitHub Actions 官方 workflow syntax 支持 job/step 级 `timeout-minutes`，可防止长时间 pending。
    *   Anthropic agent guidance 再次确认：确定性、可评估的质量门应优先用 workflow；开放式探索才交给 agent。
*   **Production Changes**:
    *   `.github/workflows/ci.yml`: 增加 workflow concurrency group，按分支/PR head 自动取消旧 run。
    *   `.github/workflows/ci.yml`: 为 Windows job、release build、Tauri build/test、CLI smoke 等步骤设置明确 timeout。
    *   `.github/workflows/ci.yml`: 将本轮核心质量门纳入 CI：Rust lib tests、`extract_quality_v2`、`golden_set_regression`、`verify-ui`、前端 utility tests 与 app build。
    *   `main.tsx` / `useProjectSelection.ts`: 修复浏览器预览模式进入时的 read-model 竞态，直接加载 demo read models，不再短暂调用 Tauri `invoke` 并显示“页面数据加载失败”。
*   **Goal**: CI 不再无限悬挂，并且能覆盖 Memory Card synthesis 这条生产力主线，而不只是编译 smoke。
*   **User Trial Evidence**:
    *   Playwright CLI 打开 `http://127.0.0.1:1420` 后，`Suggestion Review` 与“浏览器演示模式”可见。
    *   页面正文不再出现“页面数据加载失败”。
    *   `documentElement.scrollWidth <= clientWidth`，无横向溢出。
    *   剩余 console error 为 `/favicon.ico` 404，属于静态资源噪音，未影响 UI 功能路径。

---

## 13. 2026-05-21 首次打开价值展示优化
*   **Intent**: 让用户第一次在浏览器预览或演示模式打开项目时，直接看到 Agent Memory Kernel 的核心价值，而不是空的 Review Inbox。
*   **Design Rationale**:
    *   GitHub Actions 质量门继续承担可评估 workflow；开放式价值判断仍由 synthesis runtime 提供。
    *   Anthropic agent guidance 支持从简单、可组合 workflow 开始，只在需要开放探索时引入 agent。
    *   UX 空状态/演示状态应承担 onboarding 职责：展示真实工作结果，而不是只告诉用户“先去点击按钮”。
*   **Production Changes**:
    *   `app/src/demo/demo-data.ts`: 浏览器预览候选现在包含一条 `skill_targeted_card` 和一条 `already_covered`，直接展示 Value Delta、目标上下文、trace、No Card Is A Success 和合并/归档判断。
    *   `app/scripts/verify-ui-focus-contract.mjs`: 增加 demo read-model guardrail，要求预览数据保留 synthesis decisions，避免回退为空 Inbox。
    *   `app/src/components/pages/Drafts.tsx`: 候选兜底预览改为成熟 Memory Card 形态，使用“目标 / 适用场景 / 执行方式 / 边界 / 验收”，删除“用于把……”这种内部加工口吻。
    *   `app/scripts/verify-ui-focus-contract.mjs`: 增加 Review Inbox 成熟度 guardrail，要求候选兜底预览保留规范 Memory Card 字段，并禁止退回内部加工文案。
    *   `tests/extract_quality_v2.rs`: fake LLM provider 显式使用 UTF-8 stdout，修复 Windows GitHub Actions 上中文 JSON 输出触发 Python `UnicodeEncodeError` 的 CI 失败；同时收窄 refine 响应分支，只有明确的 `FinalMemoryRefinement` / `"candidates"` prompt 才返回 `{ items: [...] }`，避免 extract 阶段误拿 refine JSON shape。
*   **Verification**:
    *   `bun run --cwd app verify-ui` -> PASS.
    *   `bun test --cwd app ./src/utils/review-workbench.test.ts ./src/utils/kernel-plan.test.ts` -> PASS, 28 passed.
    *   `bun run --cwd app build` -> PASS.
    *   `cargo test --test extract_quality_v2 --quiet` -> PASS, 21 passed / 1 ignored.
    *   `cargo test --test extract_quality_v2 llm_provider_filters_and_rewrites_candidate_into_memory_card_shape -- --nocapture` -> PASS.
    *   `cargo fmt --check` -> PASS.
    *   `git diff --check` -> PASS.
    *   Browser trial on `http://127.0.0.1:1420`: Review Inbox 显示 `待审建议收件箱 (2)`、`LLM Provider`、`项目级 Skill 补强`、`Value Delta`；候选正文显示成熟 Memory Card shape；无“页面数据加载失败”；无横向溢出；不再出现“用于把”模板腔。

---

## 14. 2026-05-21 Synthesis Runtime 全局视野质量补强
*   **Issue**: GitHub #39 continuation.
*   **GitHub Status**: #39 closed after PR #35 pull_request CI passed for commit `c726aa9`; provider loop, Web/read, and embedding search remain separate follow-up work.
*   **Intent**: 让只读 synthesis runtime 不只看当前候选附近的局部文本，而能解释它读过哪些更宽的历史 observations，并识别重复工作流失败模式。
*   **Production Changes**:
    *   `src/synthesis_agent.rs`: 将 observation context 分为 direct evidence 与 broader history；新增 `search_global_history` trace event。
    *   `src/synthesis_agent.rs`: `SynthesisContextPack` 新增 `related_observations` 与 `workflow_failures`，用于给 provider prompt / Review Inbox 提供更可解释的全局视野。
    *   `src/synthesis_agent.rs`: 复用既有 `observation::failure_flow`，新增 `summarize_workflow_failures` trace event，提炼 review-iterate loop、质量纠偏、scope boundary 等失败信号。
    *   `src/synthesis_agent.rs`: 无直接证据且无更宽本地上下文时保持 `needs_human`，避免凭空高置信生成 Memory Card。
    *   `src/synthesis_agent/tests.rs`: 将 runtime tests 拆出，保持核心文件低于 1000 行，并新增同源历史、失败模式摘要、无上下文保守停止测试。
    *   `.gitignore`: 白名单追踪 `src/synthesis_agent/tests.rs`，避免 CI checkout 丢失被 Rust 模块引用的测试文件。
*   **Verification So Far**:
    *   `cargo fmt --check` -> PASS.
    *   `cargo test --manifest-path Cargo.toml synthesis_agent --lib` -> PASS, 7 passed.
    *   `cargo test --manifest-path Cargo.toml --lib` -> PASS, 315 passed.
    *   `cargo test --test extract_quality_v2 --quiet` -> PASS, 21 passed / 1 ignored.
    *   `cargo test --test golden_set_regression --quiet` -> PASS, 3 passed.
    *   `cargo clippy -- -D warnings` -> PASS.
    *   `bun run --cwd app verify-ui` -> PASS.
    *   `bun test --cwd app ./src/utils/review-workbench.test.ts ./src/utils/kernel-plan.test.ts` -> PASS, 28 passed.
    *   `bun run --cwd app build` -> PASS.
    *   `cargo test --manifest-path src-tauri/Cargo.toml -- --nocapture` -> PASS, 45 passed.
    *   `git diff --check` -> PASS.
