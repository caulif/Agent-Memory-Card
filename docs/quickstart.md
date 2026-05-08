# Agent Memory Kernel 可体验教程

这份教程帮助你在一个本地项目里体验 Agent Memory Kernel 的核心闭环：导入现有规则，配置偏好模板，从一句对话提取 Draft Memory Card，批准为 Memory Card，再编译到 Codex / Claude Code 的原生规则文件。

## 0. 准备

在仓库根目录运行：

```bash
cargo run -- --version
bun run test
```

如果只想体验命令，不想跑全量测试，可以先确认 CLI 可用：

```bash
cargo run -- preference list --project .
```

如果想直接体验桌面 UI，可以打开当前 Tauri 工作台；启动后先读取项目索引，不会自动扫描所有目录，点击“扫描本地项目”才会执行扫描：

```bash
bun run app:dev
```

Tauri 桌面工作台会读取 `~/.agent-kernel/projects.yml`，在第一屏展示本地项目列表；选择项目后可以查看 Review 摘要、处理带置信度和原因解释的 Draft Inbox、安装 Catalog Memory Cards、调整 Memory Card × Agent 分配矩阵，并手动触发 Claude Code / Codex 本地历史对话整理进 Draft Inbox。

## 1. 初始化项目状态

导入当前项目已有规则、Skills 和配置：

```bash
cargo run -- import --scan-home --project .
```

把当前项目加入全局项目列表：

```bash
cargo run -- project add --path .
cargo run -- project list
```

查看当前 Agent 目标：

```bash
cargo run -- agent list --project .
```

当前 MVP 默认服务：

- `codex` -> `AGENTS.md` 和 `.agents/skills`
- `claude-code` -> `CLAUDE.md` 和 `.claude/skills`

## 2. 初始化偏好模板库

项目级偏好模板放在 `.agent-kernel/preference-registry.yml`。

```bash
cargo run -- preference init --project .
cargo run -- preference validate --project .
cargo run -- preference list --project .
```

如果文件已经存在，`preference init` 不会覆盖。

你可以编辑 `.agent-kernel/preference-registry.yml`，加入自己的高置信偏好模板。例如：

```yaml
preferences:
  - title: Use Playwright
    body: Use Playwright for browser automation tests.
    required:
      - playwright
    context:
      - cypress
      - browser automation
      - 浏览器自动化
```

规则含义：

- `title` 会变成 Draft / Memory Card 标题，并决定稳定 ID，例如 `Use Playwright` -> `project:use-playwright`
- `body` 是最终写入 Agent 规则的标准正文
- `required` 必须全部命中
- `context` 至少命中一个；为空时会被 `preference validate` 警告，因为可能匹配太宽

## 3. 调试一句话会命中什么模板

先用内置模板体验：

```bash
cargo run -- preference test --text "以后前端请求统一使用 Axios，不要再用 Fetch。" --project .
```

你应该看到类似输出：

```text
Agent Memory Kernel preference test

Matches:
- project:use-axios [built-in] Use Axios: Use Axios for frontend HTTP requests.
  required: axios
  context: fetch, 前端请求, 请求
```

再测试项目级 Playwright 模板：

```bash
cargo run -- preference test --text "以后浏览器自动化测试统一使用 Playwright，不要再用 Cypress。" --project .
```

这个命令不会写文件，只用于调试模板命中。

## 4. 从对话提取 Draft Memory Card

先 dry-run：

```bash
cargo run -- extract --text "以后前端请求统一使用 Axios，不要再用 Fetch。" --target codex --dry-run --project .
```

确认候选后，真正写入 Draft Inbox：

```bash
cargo run -- extract --text "以后前端请求统一使用 Axios，不要再用 Fetch。" --target codex --project .
```

查看待审查 Draft：

```bash
cargo run -- draft list --project .
```

如果候选内容需要微调，可以在批准前更新它：

```bash
cargo run -- draft update --id project:use-axios --title "Use Axios" --body "Use Axios for frontend HTTP requests." --target codex --target claude-code --project .
```

也可以打开 Tauri 桌面工作台，在选中的项目页直接查看 `Draft Inbox`，点击“编辑”修改标题、类型、作用域、正文和目标 Agent，再点击“保存变更”。保存只更新 Draft，不会自动批准或启用；确认无误后再点击“批准”，不想保留则点击“删除”。

如果多个候选其实是同一组项目偏好，可以先合并为一个新的待审 Draft：

```bash
cargo run -- draft merge --id project:frontend-defaults --title "Frontend Defaults" --source project:use-axios --source project:prefer-bun --target codex --target claude-code --project .
```

`draft merge` 至少需要两个 source。它只创建一个新的 Draft，不会删除源 Draft，也不会自动批准。Tauri 桌面工作台的 `Draft Inbox` 也可以勾选多个候选，点击批量操作后继续审阅生成结果。

高置信模板生成的 Draft 会带上可解释信息：

- `confidence`：当前本地规则对这个候选的信心，例如 `92%`
- `matched_template`：命中的模板来源，例如 `built-in:Use Axios`
- `reason`：具体命中了哪些 required/context marker

## 5. 批准 Draft 为 Memory Card

批准候选：

```bash
cargo run -- draft approve --id project:use-axios --project .
```

查看 Memory Card：

```bash
cargo run -- memory_card list --project .
cargo run -- memory_card matrix --project .
```

如果想把某个 Memory Card 分配给多个 Agent：

```bash
cargo run -- memory_card targets --id project:use-axios --target codex --target claude-code --project .
```

同样可以在 Tauri 桌面工作台的“分配”页面点击矩阵单元格切换分配。注意：当前声明式配置里空 targets 表示“所有启用 Agent”，所以工作台不允许通过矩阵关掉最后一个 target；要完全移除某个 Memory Card，后续会提供专门的 remove/disable 操作。

## 5.1 从 Catalog 安装 Memory Card

查看内置 Catalog：

```bash
cargo run -- catalog list --project .
cargo run -- catalog validate --project .
```

安装一个内置 package：

```bash
cargo run -- catalog install --id core:rust-quality-gate --target codex --project .
```

也可以在 Tauri 桌面工作台的“包管理”页面安装 Catalog Memory Card。如果同一个 package 已经安装，再分配给另一个 Agent 会合并 targets，不会覆盖之前的分配。

安装后可以用矩阵检查：

```bash
cargo run -- memory_card matrix --project .
```

## 6. 预览并编译到 Agent 规则文件

先预览：

```bash
cargo run -- build --preview --project .
```

确认后写入：

```bash
cargo run -- build --project .
```

检查生成状态：

```bash
cargo run -- status --project .
cargo run -- test-rules --project .
```

生成产物包括：

- `AGENTS.md`
- `CLAUDE.md`
- `.agents/skills`
- `.claude/skills`

编译器会按 Memory Card 的 activation 分流：`preference` / `constraint` 默认进入 always-on 指令文件，`procedure` / `template` / `workflow` 默认生成标准 Agent Skill 目录。生成的 Skill 形态如下：

```text
.claude/skills/frontend-workflow/
  SKILL.md
  references/
    project-frontend-workflow.md
```

`SKILL.md` 包含 `name` 和 `description` frontmatter，详细规则放在 `references/`，这样 Claude Code / Codex 可以按需加载，而不是把所有流程都塞进启动上下文。

如果某条 Memory Card 需要编译成 Claude Code hook，可以把它的 `activation` 设为 `hook`，并在 `tags` 里声明事件，例如：

```yaml
activation: hook
tags:
  - hook:event:session-end
  - hook:matcher:git commit
body: cargo clippy --quiet -- -D warnings
```

这类 Memory Card 会写入 `.claude/settings.local.json`，并保留该文件里不属于 Agent Memory Kernel 的其他配置键。

这些是编译产物。手动改了以后，可以用下面命令回流成 Draft：

```bash
cargo run -- import --artifacts --project .
```

## 7. 从本地对话进化

扫描 Claude Code / Codex 本地会话并合成 Draft：

```bash
cargo run -- observe evolve --target codex --target claude-code --dry-run --project .
```

确认后写入 Draft Inbox：

```bash
cargo run -- observe evolve --target codex --target claude-code --project .
```

这个流程仍然不会自动启用 Memory Card。所有自动提炼的内容都会先进 Draft Inbox，由你批准。

## 7.1 通过 MCP 接入外部 Agent

如果想让支持 MCP 的客户端直接调用 Agent Memory Kernel，可以在项目根目录启动：

```bash
cargo run -- mcp --project .
```

当前最小工具集包括：

- `list_drafts`
- `approve_draft`
- `build_artifacts`

这样 Claude Code 一类客户端可以直接列出 Draft Inbox、批准 Draft，并触发一次 build / preview。

同一个动作也可以在 Tauri 桌面工作台里完成：选择项目后在“审阅”页点击“提炼候选”写入 Draft Inbox。生成后可以在同一页批准、编辑、隐藏或拒绝候选。

## 8. 推荐体验顺序

第一次体验建议按这个顺序：

```bash
cargo run -- preference validate --project .
cargo run -- project scan --root . --max-depth 4
cargo run -- project list
cargo run -- preference list --project .
cargo run -- preference test --text "以后前端请求统一使用 Axios，不要再用 Fetch。" --project .
cargo run -- extract --text "以后前端请求统一使用 Axios，不要再用 Fetch。" --target codex --dry-run --project .
cargo run -- extract --text "以后前端请求统一使用 Axios，不要再用 Fetch。" --target codex --project .
cargo run -- extract --text "以后 JS 包管理统一使用 Bun。" --target codex --project .
cargo run -- draft list --project .
cargo run -- draft update --id project:use-axios --target codex --target claude-code --project .
cargo run -- draft merge --id project:frontend-defaults --title "Frontend Defaults" --source project:use-axios --source project:prefer-bun --target codex --target claude-code --project .
cargo run -- draft approve --id project:frontend-defaults --project .
cargo run -- build --preview --project .
cargo run -- build --project .
cargo run -- test-rules --project .
```

## 9. 当前边界

当前版本已经适合体验本地 Memory Card 进化闭环，但还有几个刻意保守的边界：

- 不会静默启用自动提炼内容，必须先进入 Draft Inbox
- LLM provider 仍是配置地基，本地提取器是主路径
- Cursor / Cline 不在当前 MVP 主线
- Tauri 桌面工作台已经能浏览项目、Review、触发对话进化，并处理 Draft Inbox；更细的 Canvas 拖拽、App Store、Preference Registry 编辑体验仍会继续补强
- Tauri 桌面工作台已经接入 Memory Card Catalog、Memory Card target matrix、Draft explainability、Draft inline editing 和 Draft merge；后续还需要把包详情编辑、Memory Card remove/disable、lineage 展示做得更顺手

下一步最值得补的是更完整的 Memory Card lineage，让审查体验继续靠近一个可解释的进化系统。
