# Agent Memory Kernel 快速开始

这份教程围绕桌面日常闭环写：选择项目 -> 整理本地历史 -> 审查 Suggestion -> 批准 Memory Card -> 配置 Agent Loadout -> 预览 Artifact -> 同步并验证。

## 1. 安装并打开

Windows 用户优先使用 GitHub Releases 里的桌面安装包。安装包用户不需要 Rust、Bun 或 Tauri。

从源码运行才需要开发工具链：

```bash
bun install
bun install --cwd app
bun run app:dev
```

打开应用后进入 `Settings`，先看首跑清单：

- installer mode：Rust / Bun 会显示为不相关。
- dev mode：Rust / Bun 缺失会显示为需要处理。
- Provider：可以保存 OpenAI-compatible / Anthropic-compatible 配置，并点击 `测试 Provider`。

如果当前网络需要代理，请在启动应用的同一个终端设置：

```powershell
$env:HTTP_PROXY="http://127.0.0.1:7897"
$env:HTTPS_PROXY="http://127.0.0.1:7897"
$env:ALL_PROXY="http://127.0.0.1:7897"
```

## 2. 选择一个项目

在项目列表里扫描或添加本地项目。建议第一次选择一个真实但风险低的项目，因为 Agent Memory Kernel 的价值来自真实规则、真实对话和真实 artifact diff。

首次进入项目后，先看顶部状态：

- Suggestion Review 是否有待审建议。
- Memory Library 是否已有 Memory Cards。
- Agent Loadout 是否覆盖 Codex / Claude Code。
- Artifact Preview 是否有 drift 或未同步内容。

## 3. 整理本地历史

在桌面 UI 里触发 `Observation Evolve`。它会读取本机 Claude Code / Codex 历史和项目上下文，生成可审查的 Suggestion。

这个步骤不会直接改写 `AGENTS.md` 或 `CLAUDE.md`。自动化只负责发现候选，长期记忆必须由你批准。

Provider 不可用时，回到 `Settings` 点击 `测试 Provider`。常见 next action 包括：

- 设置 API key 环境变量或在 SK 输入框保存 key。
- 检查 Base URL、协议和模型名。
- 设置代理并重启应用进程。
- 切回本地/CLI 引擎做确定性诊断。

## 4. 审查 Suggestion

进入 `Suggestion Review`，优先看证据和下一步动作，而不是只看标题。

批准前确认：

- 它是否来自真实上下文。
- 它是否值得长期保存。
- 它应该给 Codex、Claude Code，还是两者。
- 它应该是常驻规则，还是按需 Skill。
- 它会影响哪些 Artifact。

不合适的 Suggestion 可以拒绝。拒绝会进入反馈学习材料，但不会自动修改黄金集或长期 Memory Card。

## 5. 批准为 Memory Card

确认后批准 Suggestion。批准后的 Memory Card 是长期源数据，可以继续编辑、合并、安装到项目或分配给 Agent。

产品里使用这些概念：

- `Suggestion`：系统提出的待审建议。
- `Memory Card`：人工批准后的长期记忆单元。
- `Agent Loadout`：某个项目给 Codex / Claude Code 装配了哪些 Memory Card。
- `Artifact`：最终生成的 `AGENTS.md`、`CLAUDE.md` 或 Skill 文件。

## 6. 配置 Agent Loadout

进入 `Agent Loadout`，把 Memory Cards 分配给目标 Agent。

默认生成目标：

- `codex` -> `AGENTS.md` 和 `.agents/skills`
- `claude-code` -> `CLAUDE.md` 和 `.claude/skills`

常驻偏好和约束会进入 `AGENTS.md` / `CLAUDE.md`。流程、模板和可复用工作流会进入 `.agents/skills/<name>/SKILL.md` 或 `.claude/skills/<name>/SKILL.md`。

## 7. 预览并同步 Artifact

先执行 Build Preview，检查将要写入的文件和 diff。确认没有意外 drift 后再 Sync。

同步后查看验证结果：

- Rule CI pass/fail。
- 失败时的 next action。
- Last Sync checkpoint 和回滚提示。
- 复制 reload prompt，让当前 Agent 会话重新读取最新 `AGENTS.md` / `CLAUDE.md` / Skills。

手动改过生成文件时，先用 artifact drift 操作逐文件 import / keep / discard，不要直接覆盖掉有价值的人工修改。

## 隐私与安全边界

Agent Memory Kernel 是本地优先工具：

- 只有在你触发扫描、导入或 evolve 时才读取本地项目和本地 Agent 历史。
- Provider 调用只在你启用并使用远程/CLI Provider 时发生。
- API key 不写入项目配置；设置页的 SK 输入只写入当前桌面进程环境。
- 自动化不会静默批准长期记忆。
- 生成给 Agent 的文件是可预览、可 diff、可重新生成的 build artifacts。

## 开发者 CLI 参考

CLI 目前是源码开发和内部验证入口，不作为公开 release 产物发布。

确认 CLI 可用：

```bash
cargo run -- --version
```

导入项目并查看状态：

```bash
cargo run -- import --scan-home --project .
cargo run -- project add --path .
cargo run -- status --project .
```

本地确定性提取诊断：

```bash
cargo run -- extract --text "以后前端请求统一使用 Axios，不要再用 Fetch。" --target codex --provider local --dry-run --project .
```

常用开发命令：

```bash
cargo run -- observe evolve --target codex --target claude-code --dry-run --project .
cargo run -- build --preview --project .
cargo run -- build --project .
cargo run -- test-rules --project .
cargo run -- mcp --project .
```

`mcp` 提供最小 stdio MCP endpoint，包括 `list_drafts`、`approve_draft` 和 `build_artifacts`，用于让支持 MCP 的客户端接入本地审查闭环。
