# Agent Memory Kernel

[简体中文](README.md) | [English](README.en.md)

> 面向 Claude Code / Codex 用户的本地优先 Memory Card 工作台：从真实对话、项目规则和 Skills 中提炼高价值内容，经过人工审查后同步回各工具原生 memory 格式。

Agent Memory Kernel 不是聊天记录总结器，也不是替你偷偷写规则的自动记忆系统。它更像一个“Agent memory 编译器”：把散落在本地对话、`AGENTS.md`、`CLAUDE.md`、Skills 和项目约定里的长期知识，整理成可审查、可编辑、可分配、可重新编译的 Memory Cards。

默认文档语言为中文。English documentation is available in [README.en.md](README.en.md).

## 适合谁

- 你长期使用 Claude Code / Codex 做真实项目。
- 你的 `AGENTS.md`、`CLAUDE.md` 或 Skills 开始变长、重复、冲突。
- 你经常在对话里告诉 Agent “以后都这样做”，但这些经验很难沉淀。
- 你希望 memory 是本地、可审查、可 diff、可迁移的文件，而不是黑盒状态。
- 你想把常驻规则和按需 Skill 分开，让 Agent 少读无关上下文。

## 它解决什么问题

重度使用 Agent 后，高价值信息通常会散落在很多地方：

- 用户偏好：例如“前端请求统一使用 Axios”。
- 项目约束：例如“发布前必须跑完整质量门禁”。
- 协作流程：例如“收到 code review 后先验证反馈，再修改代码”。
- 反复纠正：例如“不要在没有确认的情况下合并、发布或覆盖生成产物”。
- Skill 素材：例如稳定的调试流程、模板、脚本和检查清单。

Agent Memory Kernel 会先把这些内容提炼成待审 Suggestion，再由你确认是否批准为长期 Memory Card。批准后的 Memory Cards 可以进入 Claude Code / Codex 的 Agent Loadout，并编译成它们能直接识别的文件结构。

## 核心流程

```text
Observation -> Suggestion -> Memory Card -> Agent Loadout -> Artifact
```

- `Observation`：从本地对话、规则文件或项目资料中导入的证据。
- `Suggestion`：系统发现并进入人工审查区的待审建议，内部可能由 Candidate / Draft 两层实现。
- `Memory Card`：人工批准后的长期记忆源。
- `Agent Loadout`：把 Memory Card 分配给目标 Agent。
- `Artifact`：编译生成给 Claude Code / Codex 使用的原生文件。

生成产物按激活方式拆分：

- 常驻偏好和约束编译到 `AGENTS.md` / `CLAUDE.md`。
- 流程、模板和可复用工作流编译到 `.agents/skills/<name>/SKILL.md` / `.claude/skills/<name>/SKILL.md`。
- Hook 型 Memory Cards 可以编译到 Claude Code 项目 hooks。

## 主要功能

Agent Memory Kernel 的主要入口是 Tauri 桌面工作台。你可以在 UI 里完成核心闭环：

- 项目列表：扫描、注册和打开多个本地 Claude Code / Codex 项目。
- Suggestion Review：审查自动提炼出的建议，查看证据、风险、置信度、动作和 Artifact 影响。
- Memory Library：管理已经批准的 Memory Cards，持续维护长期 agent knowledge。
- Agent Loadout：把 Memory Cards 分配给 Codex、Claude Code 或两者。
- Catalog：安装可复用的 Memory Card packages。
- Observation Evolve：从本地 Claude Code / Codex 历史中整理高价值建议，先进入 Suggestion Review。
- Build / Sync：预览并生成 `AGENTS.md`、`CLAUDE.md` 和 Skill 文件夹。

CLI 目前主要作为源码开发和内部验证入口，暂不作为公开 release 产物发布。

## 安装与运行

当前项目处于 pre-1.0 阶段。普通 Windows 用户优先下载 GitHub Releases 中的桌面安装包；开发者可以从源码运行。

安装包用户打开应用后，先进入 `Settings` 查看首跑清单。安装包正常使用不需要 Rust、Bun 或 Tauri；如果 Provider 测试失败，设置页会给出 API key、Base URL、模型名或代理相关的下一步动作。

从源码运行需要：

- Rust 1.85+
- Bun 1.3+
- Claude Code CLI，可选但推荐，因为默认 provider 是 Claude Code

安装依赖并构建前端：

```bash
bun install
bun install --cwd app
bun run --cwd app build
```

运行桌面工作台：

```bash
bun run app:dev
```

启动后，桌面工作台会先展示本地项目列表。选择项目后，你可以在同一个界面里审查 Suggestion、批准 Memory Cards、调整 Agent Loadout、整理本地历史并执行 Build / Sync。

确认 CLI 可用：

```bash
cargo run -- --version
```

构建 Windows 桌面安装包：

```bash
bun run tauri:build
```

安装产物会输出到：

```text
src-tauri/target/release/bundle/
```

## 快速开始

推荐先使用桌面 UI 体验完整流程。CLI 放在后面的开发者参考里。

### 1. 打开桌面工作台

```bash
bun run app:dev
```

安装包用户直接从开始菜单打开应用；源码开发者使用上面的命令。

首次进入后：

1. 在 `Settings` 确认首跑清单和 Provider 状态。
2. 扫描或添加本地 Claude Code / Codex 项目。
3. 选择项目进入工作台，查看下一步动作、Suggestion、Memory Card、artifact drift 和同步状态。

### 2. 整理本地历史

在 UI 中触发 Observation / Evolve 流程。Agent Memory Kernel 会读取本地 Claude Code / Codex 历史和项目上下文，提炼可能长期有用的候选内容。

这些建议不会自动写入 `AGENTS.md` 或 `CLAUDE.md`，而是先进入 Suggestion Review。

### 3. 审查 Suggestion Review

在 Suggestion Review 中逐条检查建议：

- 是否来自真实上下文。
- 是否值得长期保存。
- 应该分配给 Codex、Claude Code，还是两者。
- 应该作为常驻规则，还是按需 Skill。

确认后批准为 Memory Card；不合适的建议可以拒绝或继续编辑。

### 4. 管理 Memory Cards 和 Agent Loadout

进入 Memory Library / Agent Loadout：

- 查看已批准 Memory Cards。
- 调整目标 Agent。
- 控制哪些内容进入 `AGENTS.md`、`CLAUDE.md` 或 Skill 文件夹。

### 5. 预览并同步生成产物

在 UI 中先执行 Build Preview，确认将要写入的原生文件，再执行 Sync。

默认生成目标：

- `codex` -> `AGENTS.md` 和 `.agents/skills`
- `claude-code` -> `CLAUDE.md` 和 `.claude/skills`

同步后查看 Rule CI、Last Sync checkpoint、失败 next action，并复制 reload prompt 让当前 Agent 会话重新读取最新生成文件。

## 开发者 CLI

```bash
cargo run -- import --scan-home --project .
cargo run -- project add --path .
```

默认 provider 是 `claude-cli`。如果你只想快速测试本地确定性提取，不想调用 Claude Code，可以显式传入 `--provider local`：

```bash
cargo run -- extract --text "以后前端请求统一使用 Axios，不要再用 Fetch。" --target codex --provider local --dry-run --project .
```

少量常用自动化命令：

```bash
cargo run -- observe evolve --project . --target codex --target claude-code --dry-run
cargo run -- build --preview --project .
cargo run -- sync --project .
cargo run -- mcp --project .
```

`mcp` 命令提供最小 stdio MCP endpoint，包含 `list_drafts`、`approve_draft` 和 `build_artifacts` 等工具，方便 Claude Code 或其他 MCP 客户端接入本地 Draft Inbox。

## 文档

- [功能介绍](docs/feature-overview.md)
- [可体验教程](docs/quickstart.md)
- [平台支持](docs/platform-support.md)
- [迭代 Backlog](docs/iteration-backlog.md)
- [相关项目研究](docs/research/related-projects.md)

## 隐私与信任边界

Agent Memory Kernel 是本地优先工具。它会在你触发导入、扫描或 evolve 时读取本地项目和本地 Agent 历史，并把提炼结果放进 Draft Inbox。

默认原则是：

- 自动化负责发现候选。
- 人类负责批准长期记忆。
- Memory Cards 是可读、可编辑、可审查的文件化源数据。
- 生成给 Agent 的文件是 build artifacts，可以重新生成和检查差异。

## 开发

运行测试和检查：

```bash
cargo fmt --check
cargo clippy --quiet -- -D warnings
bun run --cwd app build
```

Tauri 后端检查：

```bash
cargo clippy --manifest-path src-tauri/Cargo.toml --quiet -- -D warnings
cargo build --manifest-path src-tauri/Cargo.toml --release
```

## Contributing

欢迎提交 issue、讨论使用场景、补充文档、修复 bug 或改进适配器。提交 PR 时请尽量做到：

- 说明这个改动解决了什么用户问题。
- 保持改动范围清晰，避免把无关重构混在一起。
- 为核心行为补充验证步骤；测试文件保留在本地开发环境，不上传到公开仓库。
- UI 改动请附上截图或录屏说明。
- 涉及隐私、provider、文件写入或 artifact 同步的改动，请说明信任边界和失败处理。

更多细节见 [CONTRIBUTING.md](CONTRIBUTING.md)。

## 状态

Agent Memory Kernel 仍在早期快速迭代中。当前重点是把 Claude Code / Codex 的本地对话和规则文件沉淀为高质量、可审查、可长期复用的 Memory Cards。

欢迎试用、提 issue，也欢迎分享你真实的 Agent memory 维护痛点。

## License

MIT

