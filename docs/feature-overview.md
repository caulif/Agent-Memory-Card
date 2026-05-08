# Agent Memory Kernel 功能介绍

Agent Memory Kernel 是一个面向 Claude Code / Codex 重度用户的本地优先 Agent Memory Kernel。它从真实协作对话、项目规则和已有 Skills 中发现高质量内容，先沉淀为可审查的 Memory Cards，再按需同步到 Claude Code / Codex 能识别的原生 memory 表面。

它的目标不是替 AI 自动记住一切，而是帮助用户把长期有价值的经验、偏好、约束、流程和判断标准整理出来，经过人工确认后成为可长期复用的项目资产。

## 为什么需要它

重度使用 Claude Code / Codex 后，项目里常见的问题是：

- `AGENTS.md`、`CLAUDE.md` 越写越长，规则重复、冲突、过时。
- 高质量经验散落在历史对话里，下次遇到类似任务时很难复用。
- 一些流程适合做成 Skill，但用户没有精力手动整理。
- AI 自动记忆不透明，用户不知道它保存了什么，也不好审查。
- 不同 Agent 的 memory 格式不一致，迁移和同步成本高。

Agent Memory Kernel 把这些问题收束成一个本地审查流程：先从真实输入中提取候选内容，再让用户编辑、合并、批准或拒绝，最后编译成各工具原生格式。

## 核心概念

### Memory Card

Memory Card 是 Agent Memory Kernel 对外推荐使用的产品概念。它表示一条经过整理、可以长期复用的 Agent 记忆。

一张 Memory Card 可以是：

- 用户偏好：例如“前端请求统一使用 Axios”。
- 项目约束：例如“发布前必须跑完整质量门禁”。
- 协作流程：例如“接收 code review 后先验证反馈，再修改代码”。
- 模板片段：例如 PR 描述、发布说明、测试报告格式。
- 反复纠正：例如“不要在没有确认的情况下替用户合并或发布”。
- 经验总结：例如某类调试、构建、迁移问题的稳定处理方式。

在当前代码实现里，Memory Card 对应内部的 `SkillletRecord`。`Skilllet` 可以继续作为开发者术语存在，但对外更建议使用 Memory Card。

### Draft Inbox

Draft Inbox 是所有自动提取内容进入长期记忆前的审查区。

Agent Memory Kernel 不会把候选内容直接写入 `AGENTS.md` 或 `CLAUDE.md`。它会先生成 Draft，让用户检查内容是否真实、是否值得长期保存、应该分配给哪个 Agent，以及应该作为常驻规则还是按需 Skill。

### 原生 memory 表面

Agent Memory Kernel 自己保存中立格式的 Memory Cards，然后再编译到 Claude Code / Codex 各自能识别的文件结构：

- Claude Code：`CLAUDE.md`、`.claude/skills`
- Codex：`AGENTS.md`、`.agents/skills`

轻量、常驻的偏好和约束会进入 `CLAUDE.md` / `AGENTS.md`。流程型、模板型、工作流型内容会进入 `.claude/skills` / `.agents/skills`，让 Agent 在需要时按需加载。

## 主要功能

### 1. 本地项目扫描

Agent Memory Kernel 可以扫描本地项目，发现项目中的 Agent 规则、Skills、配置和可管理的 memory 文件。

它的项目注册表保存在本地，适合管理多个 Claude Code / Codex 项目。

### 2. 对话内容提炼

Agent Memory Kernel 可以从本地 Claude Code / Codex 协作历史中提炼候选记忆。

提炼目标不是总结聊天记录，而是寻找可复用的高价值内容，例如：

- 反复出现的用户偏好
- 明确的项目约定
- 有迁移价值的工作流
- 用户纠正过的错误行为
- 可以沉淀成 Skill 的操作流程

提炼结果会进入 Draft Inbox，而不是自动启用。

### 3. Draft 审查、编辑和合并

用户可以在 Draft Inbox 中处理候选内容：

- 查看候选标题、正文、置信度和来源说明
- 编辑标题、正文、类型、作用域、标签和目标 Agent
- 删除无价值候选
- 合并多个相近候选
- 批准为长期 Memory Card

这一步是 Agent Memory Kernel 的核心信任边界：自动化负责发现候选，人类负责决定什么应该成为长期记忆。

### 4. Memory Library

批准后的 Draft 会进入 Memory Library，成为项目或全局可复用的 Memory Card。

Memory Card 会保留：

- 标题和正文
- 类型和作用域
- 标签和语言
- 目标 Agent
- 来源证据
- 创建和更新时间
- 合并历史

这让长期记忆不再只是散落的 Markdown 片段，而是可管理、可审查、可演化的结构化资产。

### 5. Agent 分配矩阵

同一张 Memory Card 可以分配给不同 Agent：

- 只给 Claude Code
- 只给 Codex
- 同时给 Claude Code 和 Codex
- 暂时不分配，作为休眠记忆保留

这个设计适合多 Agent 工作流。用户可以把同一条经验同步到多个工具，也可以根据 Agent 能力和格式差异做选择。

### 6. 编译到原生格式

Agent Memory Kernel 会根据 Memory Card 的类型和激活方式生成不同产物：

- `preference`、`constraint`、`convention`、`correction` 等轻量记忆会编译成常驻规则。
- `procedure`、`template`、`workflow` 等流程型记忆会编译成 Agent Skill。
- Claude Code 的 hook 型记忆可以编译到 `.claude/settings.local.json`。

默认输出目标是：

```text
Claude Code:
  CLAUDE.md
  .claude/skills/

Codex:
  AGENTS.md
  .agents/skills/
```

这些都是各工具能直接识别的原生文件结构，所以用户不需要在 Agent Memory Kernel 运行时常驻后台服务。

### 7. Drift 检测和回流

Agent Memory Kernel 会记录生成产物的 hash。如果用户手动修改了生成的 `CLAUDE.md`、`AGENTS.md` 或相关 artifact，Agent Memory Kernel 可以检测到 drift。

这可以避免同步时静默覆盖用户手改内容。用户可以先把手动改动导入 Draft Inbox，再决定是否沉淀为 Memory Card。

### 8. 桌面工作台

项目包含 Tauri 桌面工作台，用于完成更直观的审查流程：

- 浏览本地项目
- 查看项目概览
- 触发扫描和对话进化任务
- 处理 Draft Inbox
- 编辑 Memory Card
- 管理 Agent 分配
- 查看后台任务状态

如果只想快速体验，也可以使用 CLI 完成同样的核心流程。

### 9. CLI 和 MCP

Agent Memory Kernel 提供 CLI，适合脚本化和自动化：

```bash
cargo run -- project scan --root . --max-depth 4
cargo run -- import --project .
cargo run -- observe evolve --project . --target codex --target claude-code
cargo run -- draft list --project .
cargo run -- skilllet list --project .
cargo run -- build --preview --project .
cargo run -- sync --project .
```

项目也提供最小 MCP endpoint，让支持 MCP 的客户端可以列出 Draft、批准 Draft、触发 artifact build。

## 典型使用流程

一个完整闭环通常是：

1. 注册或扫描本地项目。
2. 导入已有 `AGENTS.md`、`CLAUDE.md` 和 Skills。
3. 从本地 Claude Code / Codex 历史中提炼候选记忆。
4. 在 Draft Inbox 审查、编辑、合并或拒绝候选。
5. 批准高质量候选为 Memory Card。
6. 分配 Memory Card 到 Claude Code、Codex 或两者。
7. 预览将要写入的原生 artifact。
8. 同步到 `AGENTS.md`、`CLAUDE.md` 或 Agent Skills。
9. 后续继续从真实协作中迭代记忆库。

这个流程强调“真实输入 -> 人类审查 -> 长期复用”，而不是自动把所有上下文塞进长期记忆。

## 适合谁使用

Agent Memory Kernel 适合：

- 高频使用 Claude Code / Codex 的开发者。
- 同时维护多个 AI coding 项目的人。
- 已经开始维护 `AGENTS.md`、`CLAUDE.md` 或 Skills，但感觉越来越乱的人。
- 希望把协作经验沉淀成团队资产的人。
- 对自动 memory 不放心，希望保留人工审查边界的人。
- 想探索 Agent workflow、memory governance、skills 生态的人。

它暂时不太适合：

- 只偶尔使用 AI 编程工具的人。
- 不想管理本地文件和命令行的人。
- 期待开箱即用云端同步和账号系统的人。
- 想要完全自动长期记忆、不做人工审查的人。

## 项目实际价值

Agent Memory Kernel 的实际价值不在于“又做了一个记忆库”，而在于它管理的是 Agent memory 的生命周期：

- 从真实对话中发现候选。
- 用 Draft Inbox 保留审查边界。
- 用 Memory Card 表达长期知识单元。
- 用 assignment matrix 管理不同 Agent 的使用范围。
- 用 compiler 输出各工具原生格式。
- 用 drift detection 防止覆盖用户手动修改。

这让 memory 从一堆散乱文件，变成可以持续维护和演化的本地资产。

## 当前边界

当前版本适合早期体验和开发者试用，但仍应视为 Alpha / Preview：

- 桌面安装包发布链路还需要进一步验证。
- 当前主线聚焦 Claude Code 和 Codex。
- Cursor、Cline、OpenCode、Goose 等 exporter 仍属于后续方向。
- 自动提炼质量需要继续通过真实历史验证。
- 外部 AI synthesis / fusion 仍保持保守策略。
- 全局 memory 写入应该默认关闭，避免误改用户级配置。

这些边界是刻意保守的。Agent Memory Kernel 的默认原则是先保护用户信任，再追求自动化便利。

## 后续方向

未来可以继续扩展：

- 更清晰的 Memory Card 分类和生命周期管理
- 更强的来源证据和 lineage 展示
- Memory Card 过期、休眠、冲突检测
- Cursor / Cline / OpenCode / Goose exporter
- 全局 memory library 导入导出
- 团队共享 memory pack
- 更完整的 Rule CI 和安全检查
- 更好的 release packaging 和安装体验

## 一句话介绍

Agent Memory Kernel turns Claude Code and Codex conversations into reviewable Memory Cards, then syncs approved knowledge back into each agent's native memory surfaces.

中文可以说：

Agent Memory Kernel 从 Claude Code / Codex 的真实协作过程中提炼高质量记忆卡片，经过人工审查后同步回各工具原生 memory 格式，让长期经验可以被持续复用。
