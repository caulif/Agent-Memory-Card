# Agent-Kernel 可体验教程

这份教程帮助你在一个本地项目里体验 Agent-Kernel 的核心闭环：导入现有规则，配置偏好模板，从一句对话提取 Draft Skilllet，批准为 Skilllet，再编译到 Codex / Claude Code 的原生规则文件。

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

如果想直接体验本地编译 UI，可以先扫描项目并打开原生桌面 app：

```bash
cargo run -- project scan --root . --max-depth 4
cargo run -- app --scan-root .
```

`app` 是 Rust/egui 编译出来的本地桌面程序，不是 Web UI。它会读取 `~/.agent-kernel/projects.yml`，在第一屏展示本地项目列表；选择项目后可以查看 Review 摘要、处理 Draft Inbox、安装 Catalog Skilllets，并一键把 Claude Code / Codex 本地历史对话整理进 Draft Inbox。

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

- `title` 会变成 Draft / Skilllet 标题，并决定稳定 ID，例如 `Use Playwright` -> `project:use-playwright`
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
Agent-Kernel preference test

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

## 4. 从对话提取 Draft Skilllet

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

也可以打开原生 app，在选中的项目页直接查看 `Draft Inbox`，点击 `Approve` 或 `Reject`。

## 5. 批准 Draft 为 Skilllet

批准候选：

```bash
cargo run -- draft approve --id project:use-axios --project .
```

查看 Skilllet：

```bash
cargo run -- skilllet list --project .
cargo run -- skilllet matrix --project .
```

如果想把某个 Skilllet 分配给多个 Agent：

```bash
cargo run -- skilllet targets --id project:use-axios --target codex --target claude-code --project .
```

## 5.1 从 Catalog 安装 Skilllet

查看内置 Catalog：

```bash
cargo run -- catalog list --project .
cargo run -- catalog validate --project .
```

安装一个内置 package：

```bash
cargo run -- catalog install --id core:rust-quality-gate --target codex --project .
```

也可以在原生 app 的 `Skilllet Catalog` 区块里点击 `Install to Codex` 或 `Install to Claude Code`。如果同一个 package 已经安装，再分配给另一个 Agent 会合并 targets，不会覆盖之前的分配。

安装后可以用矩阵检查：

```bash
cargo run -- skilllet matrix --project .
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

这个流程仍然不会自动启用 Skilllet。所有自动提炼的内容都会先进 Draft Inbox，由你批准。

同一个动作也可以在原生 app 里完成：选择项目后点击 `Preview Conversation Evolution` 先预览，再点击 `Evolve Conversations to Drafts` 写入 Draft Inbox。生成后可以在同一页的 `Draft Inbox` 里批准或拒绝。

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
cargo run -- draft list --project .
cargo run -- draft approve --id project:use-axios --project .
cargo run -- build --preview --project .
cargo run -- build --project .
cargo run -- test-rules --project .
```

## 9. 当前边界

当前版本已经适合体验本地 Skilllet 进化闭环，但还有几个刻意保守的边界：

- 不会静默启用自动提炼内容，必须先进入 Draft Inbox
- LLM provider 仍是配置地基，本地提取器是主路径
- Cursor / Cline 不在当前 MVP 主线
- 原生 app 已经能浏览项目、Review、触发对话进化，并处理 Draft Inbox；更细的 Canvas 拖拽、App Store、Preference Registry 编辑体验仍会继续补强
- 原生 app 已经接入 Skilllet Catalog；后续还需要把 Skilllet target matrix 和更细的包详情编辑体验做得更顺手

下一步最值得补的是原生 Skilllet target matrix，以及给 Draft 增加 `matched_template` / `confidence` / `reason` 字段，让审查体验更像一个可解释的进化系统。
