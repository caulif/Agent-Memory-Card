# Agent Memory Kernel / Memory-GC 构思文档

> 日期：2026-04-30  
> 目标：把一个由 Rust 构建、通过 `bunx` 分发的引擎，设计成面向 Claude Code / Codex 的本地 Memory Card 进化、可视化治理和规则编译系统。

## 0. 本轮增强后的关键结论

这轮讨论后，Agent Memory Kernel 的定位应该再往前推一步：它不是“修补现有规则文件”的小工具，而是一个面向 Claude Code 和 Codex 的“本地 Memory Card 进化引擎”。

### 0.0 v1 可用版架构重置

当前项目已经证明很多单点能力可行，但真正可用版不能继续横向堆功能。v1 的产品主线必须收敛为一个日常可重复工作流：

1. 选择本地 Claude Code / Codex 项目。
2. 立即看到轻量项目 Dashboard，而不是等待完整 snapshot。
3. 手动启动“整理历史与规则”，后台 Job 展示阶段、进度、日志和结果。
4. 得到少量高价值 Draft Memory Cards，默认只展示 Top 候选。
5. 用户编辑、合并、批准或拒绝 Draft。
6. 将已批准 Memory Card 分配给 Claude Code / Codex。
7. 编译生成 `CLAUDE.md` / `AGENTS.md` / skills artifacts，并运行本地验证。

因此 v1 架构从“功能模块集合”调整为：

```mermaid
flowchart TD
  UI["Tauri React UI"] --> IPC["Tauri IPC Commands"]
  IPC --> AppService["Application Service Layer"]
  AppService --> ReadModels["Lightweight Read Models"]
  AppService --> Jobs["Job Manager"]
  AppService --> Core["Rust Core Domain"]
  Core --> Observation["Observation Pipeline"]
  Core --> Synthesis["Candidate / Draft Synthesis"]
  Core --> Memory Card["Memory Card Domain"]
  Core --> Compiler["Compiler / Artifact Drift"]
  Core --> Policy["Policy / Audit"]
```

关键原则：

- UI 首屏只读轻量 Read Model，不能把完整 `ProjectSnapshot` 当成切项目主路径。
- 扫描、整理历史、AI 精炼、同步编译都必须是后台 Job。
- `Observation -> Candidate -> Draft -> Memory Card -> Assignment -> Artifact` 必须分层，不能把本地历史直接粗暴变成 Draft。
- Catalog / App Store 降级为次要能力，先把 Review Inbox 和 Assignment Matrix 做可靠。
- Canvas / 白板隐喻保留为后续可视化层，v1 默认界面以 Inbox、Library、Assignment Matrix 为主。

### 0.0.1 存储原则修正：File-native，而不是绝对 Zero DB

“零数据库”容易被误解为不允许索引和缓存。更准确的工程原则是：

> Agent Memory Kernel 使用 file-native local store，并且不要求任何外部数据库、向量库或后台服务作为必需依赖。

合理边界：

- 不强制 SQLite、Postgres、Vector DB。
- Source of Truth 仍然是 `.agent-kernel/` 和 `~/.agent-kernel/` 下可读、可 diff、可迁移的 YAML / Markdown / JSONL 文件。
- 允许维护结构化索引、read-model cache、job files、audit log，以保证 UI 足够快。
- 后续如果接 Mem0 / Graphiti / SQLite，只能作为可选 adapter，不能成为个人开发者使用 v1 的必需条件。

建议文件布局：

```text
~/.agent-kernel/
  projects.yml
  global-memory_cards/
  jobs/
  cache/

project/.agent-kernel/
  project.yml
  observations/
  candidates/
  drafts/
  memory_cards/
  assignments.yml
  indexes/
    observation-index.yml
    draft-index.yml
    memory_card-index.yml
  cache/
    dashboard.json
    search-index.json
  artifacts.lock.yml
  audit.jsonl
```

关键决策：

- Rust 内核：核心解析、分类、diff、build、Rule CI、文件操作都用 Rust，保证速度、单二进制分发和工程可靠性；`bunx agent-kernel` 是优先的跨平台安装与启动入口。
- 可视化优先：UI 不是后期锦上添花，而是建立信任的核心产品面。当前主入口从 egui 收敛到 Tauri 2 桌面架构：Rust 继续负责本地内核和文件系统能力，Web 前端负责现代交互和视觉表达；旧 egui app 仅保留为迁移期 fallback，旧 Web Canvas 保留为 legacy/dev 辅助入口。
- Native App 第一屏：安装后优先展示本机可发现的项目列表，用户选择某个项目后再进入 Project-centered Canvas / Inspector / App Store / Draft Inbox。第一体验不是“打开某个仓库再配置”，而是“先看到我的本地 Agent 工作区地图”。
- UI 双入口：Tauri 原生 app 第一屏是 Project Console + Canvas；同时提供 App Store/包管理器界面，用于浏览、安装、启用和更新 Skills/Memory Cards。Web `ui` 命令后续只作为调试和兼容入口。
- 编译产物思维：`CLAUDE.md`、`AGENTS.md` 默认视为 build artifacts，由 `~/.agent-kernel/memory_cards` 和项目 `.agent-kernel/project.yml` 全量编译生成；Cursor 等其他 Agent 保留 adapter 扩展接口，MVP 不进入默认目标。
- 首个入口：先从用户现有规则文件和 Skills 导入，后续再通过 MCP/session log 自动提炼对话中的 Draft Memory Cards。
- Skill 管理默认引用模式：第三方或已有 Skill 保持原位置，Agent Memory Kernel 建立索引、启用关系、导出关系和 overlay，不直接改源文件。
- Skill 分发默认 Mirror 模式：拖拽现有 Skill 到某个 Agent/项目时，不改源文件，而是复制/同步一份到目标 Agent 的 skills 目录，并记录 source path、hash 和同步状态。
- 项目配置采用声明式：`.agent-kernel/project.yml` 只描述“当前项目希望启用哪些规则、Memory Cards、Skills，以及它们分配给哪些 Agent”，build 负责生成具体产物。
- 对话提炼采用半自动 Draft Inbox：MCP/CLI/session log 主动发现值得沉淀的信息，但只生成草稿和推荐作用域，不自动启用。
- 隐私策略采用 Hybrid：扫描、索引、Mirror、build、Rule CI 编排默认本地执行；LLM 提炼和冲突判断通过可选 provider 完成，用户可选择云端模型或本地模型。
- v0.1 范围收紧：先不做半自动对话提炼，第一版只完成“扫描现有规则和 Skills -> Canvas 分配 -> Mirror/build preview”的闭环。
- v0.2 范围：完善 Mirror 信任闭环，区分 source updated / target drifted，提供 CLI/UI sync。
- v0.3 范围：落地 Owned Memory Card 存储、CLI add/list、声明式 project.yml include、按 Agent target 编译到指令文件。
- v0.4 范围：落地本地 Draft Inbox，不接 LLM，先支持 CLI add/list/approve 和 UI 展示。
- v0.5 范围：让 Draft Inbox 在 UI 中可操作，支持 approve/reject，并让 build preview 返回结构化 actions/warnings。
- v0.6 范围：实现本地启发式 extract，从文本/文件中提取高信号规则为 Draft Inbox，不接 LLM、不自动启用。
- v0.7 范围：把本地 extract 接入 Canvas UI，支持粘贴文本生成 Draft，并继续由用户 approve/reject。
- v0.8 范围：落地 Hybrid provider 配置地基，支持 provider init/show，默认 local-first，预留 openai-compatible。
- v0.9 范围：增强 extract 支持 `--provider local`、`--dry-run`，UI Extract 支持选择 Agent targets。
- v0.10 范围：加入本地 secret redaction，extract preview 和 Draft evidence 默认脱敏常见 token/key/password 形态。
- v0.11 范围：实现本地 Rule CI，读取 `.agent-kernel/tests/*.yml`，对生成的 Agent artifact 做 include/exclude 断言。
- v0.12 范围：把 Rule CI 接入 Canvas UI，提供 `/api/rule-tests` 和可视化 pass/fail 面板。
- v0.13 范围：实现 `review` 命令，聚合 Draft Inbox、Mirror Status、Rule CI、Build Preview，作为交互式 CLI 的前置地基。
- v0.14 范围：补齐发布/分发地基，Bun wrapper 优先加载预编译 Rust binary，本地开发 fallback 到 Cargo，并通过 tag workflow 打包跨平台 artifacts。
- v0.15 范围：让 `review` 变成可脚本化审查协议，支持 JSON 输出和 Draft approve/reject 决策入口，为后续 `git add -p` 式交互 CLI 铺路。
- v0.16 范围：把统一 review 协议接入 Canvas，提供 `/api/review` 和可视化 Review 摘要，让 CLI 与 UI 共用同一套审查数据模型。
- v0.17 范围：实现本地 Memory Card Catalog / App Store 地基，支持内置 packages、`.agent-kernel/catalog.yml` 覆盖、CLI install，以及 Canvas App Store 安装入口。
- v0.18 范围：增强 Catalog 安装状态反馈，CLI 和 Canvas 均显示 available/installed，避免 App Store 重复安装缺少信任提示。
- v0.19 范围：为 Catalog package 增加 provenance 元数据（version/source_url/tags），让未来 Registry 和安全审查能基于来源、版本和类别做信任判断。
- v0.20 范围：加入 Catalog 本地 trust gate，CLI/UI 均可验证 duplicate id、missing provenance、empty body、missing tags，安装前先建立信任反馈。
- v0.21 范围：加入 instruction artifact 预算警告，默认 32 KiB，借鉴 Codex `project_doc_max_bytes` 约束，提前发现 prompt bloat。
- v0.22 范围：探索通用 rules exporter，为未来 Cursor adapter 打接口地基；Cursor 不进入当前默认目标。
- v0.23 范围：加入 Agent target 启停控制，CLI 与 Canvas 都能切换 Agent enabled 状态，降低手改声明式 YAML 的门槛。
- v0.24 范围：加入 Memory Card target assignment，CLI/UI 都能把同一 Memory Card 分配给不同 Agent，强化 Project 层 multi-agent 配置体验。
- v0.25 范围：加入 Memory Card target matrix，CLI/UI 都能总览 Memory Card × Agent 分配关系，为后续拖拽连线和批量操作打底。
- v0.26 范围：探索 Cline-style 规则目录 exporter 与旧配置迁移；v0.30 后 Cline 从核心主线移除，作为后续插件式 adapter 备选。
- v0.27 范围：将 Canvas Inspector 中的 Memory Card target matrix 从文字摘要升级为可点击矩阵表，让用户能直接按 Memory Card × Agent 维度分配能力。
- v0.28 范围：加入 generated artifact drift 检测，基于 `project.lock.yml` 比对 `AGENTS.md`、`CLAUDE.md` 等编译产物是否被手改，为后续 Reverse Parse 生成 Draft 打基础。
- v0.29 范围：实现 Reverse Parse 的本地第一版，`import --artifacts` 通过重新渲染期望产物并提取用户新增行，把手改的 build artifact 转成 Draft Inbox 候选。
- v0.30 范围：重置 MVP 范围，默认只支持 Claude Code / Codex；Cursor 和 Cline 从默认配置与 Canvas 目标中移除，但保留通用 exporter/adapter 接口。
- v0.31 范围：增强 Memory Card 操作能力，支持把一个或多个 Memory Card 分配到某个项目或 Agent，合并多个 Memory Card，并把 Memory Card 作为生成补充追加进已有 mirrored Skill。
- v0.32 范围：启动 Observation Layer，支持把本地对话文件和 Claude Code / Codex 常见 JSONL session 目录导入 `.agent-kernel/observations`，先保存原始观察记录，后续再进行 Memory Card Synthesis。
- v0.33 范围：打通本地进化闭环第一版，`observe synthesize` 将 Observation 转成 Draft Inbox 候选，Canvas 也可以从 Observations 一键生成待审阅 Draft，但不会自动启用 Memory Card。
- v0.34 范围：增加 `observe evolve`，一条命令完成本地 Claude Code / Codex 会话导入与 Draft 合成，仍保持“只进 Draft Inbox，不自动启用”的信任边界。
- v0.35 范围：Observation 导入增加 ID 去重，同一会话重复导入会计入 skipped，避免每天重复 evolve 时制造虚假的新增数量。
- v0.36 范围：将 JavaScript 包装层从 Node/npm 叙事切到 Bun，目录改为 `bun/`，本地脚本使用 `bun` / `bun test`，发布打包使用 `bun pm pack`。
- v0.37 范围：本地提取器识别高置信 Bun 包管理偏好，把“从 npm/pnpm/yarn 改为 Bun”这类对话归一化为稳定 `project:prefer-bun` Draft，而不是生成口语化长标题。
- v0.38 范围：本地提取器识别 Fetch -> Axios 这类高频 HTTP 客户端偏好纠正，归一化为稳定 `project:use-axios` Draft。
- v0.39 范围：引入 Known Preference Registry，把 Bun、Axios、Vitest 等高置信偏好归一化改为表驱动，后续扩展更多 Memory Card 模板时只需增加条目。
- v0.40 范围：Observation Synthesis / Evolve 报告返回具体 Draft ID 列表，让本地进化过程能解释“生成了哪些候选”，而不是只显示数量。
- v0.41 范围：Observation dry-run 报告也返回候选 Draft ID，让 UI / CLI 在真正写入 Draft Inbox 前就能展示“将会生成哪些候选”。
- v0.42 范围：Known Preference Registry 支持项目级 `.agent-kernel/preference-registry.yml` 扩展，并且项目模板优先于内置模板，方便高级个人开发者覆盖默认偏好文案。
- v0.43 范围：增加 `preference list`，列出 built-in / project 来源的偏好模板，让用户能审计当前自动进化词表。
- v0.44 范围：增加 `preference init` / `preference validate`，让项目级偏好模板库可以初始化、校验，并在错误时以非零退出码接入脚本或 CI。
- v0.45 范围：增加 `preference test --text` 命中解释器，并提供 `docs/quickstart.md`，让用户能完整体验 Preference Registry -> Draft Inbox -> Memory Card -> Agent artifact 的闭环。
- v0.46 范围：增加全局 Project Registry 与本地原生桌面入口。`project scan/list/add` 维护 `~/.agent-kernel/projects.yml`，`agent-kernel app` 启动 Rust/egui 编译程序，展示本地项目、项目状态，并提供 Claude Code / Codex 历史对话整理到 Draft Inbox 的一键入口。
- v0.47 范围：将 Windows / macOS / Linux 支持变成工程约束。CI 在三大系统上跑 Rust 与 Bun wrapper 测试，release 输出 `win32-x64`、`linux-x64`、`darwin-x64`、`darwin-arm64` 四类预编译包，其他架构走 Cargo fallback。
- v0.48 范围：让原生 app 成为真正的 Review Inbox。选中项目后直接展示 Draft Inbox，支持 approve / reject，并复用 CLI review 协议刷新摘要，继续坚持“自动提炼只进 Draft，不静默启用”的信任边界。
- v0.49 范围：把 Memory Card Catalog / App Store 接入原生 app。选中项目后展示 Catalog Health、package provenance、安装状态，并支持把 catalog package 安装或分配给 Codex / Claude Code；重复分配时合并 targets，不覆盖已有 Agent 分配。
- v0.50 范围：将桌面第一屏产品语义收敛为“智能体记忆整理台”。UI 明确说明它不是聊天记录摘要器，而是过滤一次性任务，只保留稳定偏好、硬约束、工作流、项目约定、反复纠正和可补充到 Skill 的能力片段。
- v0.51 范围：项目发现读取 Claude Code / Codex 的本地历史索引。除了常规目录扫描，还解析 `~/.claude/history.jsonl`、`~/.claude/projects`、`~/.claude/sessions`、`~/.codex/history.jsonl`、`~/.codex/sessions` 中可发现的 `project` / `cwd`，让安装后更接近“看到我所有本地 Agent 项目”。
- v0.52 范围：Observation 导入过滤 system/base instructions、本地命令回显、token 计数和 session metadata 噪声，避免把 agent 自身提示词或终端输出误判成用户 Memory Card。
- v0.53 范围：Observation Synthesis 从逐条生成改为聚合式高价值提炼。默认最多生成 24 个候选，只保留命中 Known Preference Registry 或强 Memory Card 信号的内容，并在 UI 隐藏低置信度碎片，解决一千多个草稿无法审阅的问题。
- v0.54 范围：桌面提炼入口加入整理引擎选择，默认 Claude Code，保留 Codex 与本地过滤入口。Claude Code / Codex 通过非交互命令生成结构化 JSON 候选，失败或超时时回落到本地高价值过滤，避免 UI 卡死或因外部 CLI 不可用中断整理。
- v0.55 范围：加入首次启动静默增量整理。Tauri 桌面端启动后在后台处理已发现 Claude Code / Codex 项目的本地会话，首次全量生成 Draft Memory Cards，后续通过 `.agent-kernel/observation-index.yml` 记录 source path、hash、processed bytes，只读取新增或追加内容，避免同一批历史反复生成草稿。
- v0.56 范围：修正会话归属。Observation 导入会读取 JSONL 中的 `cwd` / `project`，只把某个会话导入它所属的项目，避免“当前选中项目”吃进所有 Claude/Codex 历史导致记忆污染。
- v0.57 范围：扩展“高价值”定义，不再只等同于长期偏好、硬约束、项目约定、流程和反复纠正。凡是对项目未来执行质量有明显改善的记录，例如 root cause、成功修复路径、架构决策、性能/卡顿处理、跨平台坑、测试策略、UI 可用性改进，也可以提炼成 Draft Memory Card。
- v0.50 范围：把 Memory Card × Agent target matrix 接入原生 app。选中项目后可以直接点击矩阵单元格给 Codex / Claude Code 分配或取消分配 Memory Card；同时防止通过 UI 产生空 targets，因为当前声明式语义中空 targets 表示“所有启用 Agent”。
- v0.51 范围：让 Draft Inbox 具备可解释性。自动提炼出来的 Draft 需要保存并展示 `confidence`、`matched_template`、`reason`，dry-run 与原生 app 都能说明“为什么建议固化这条 Memory Card”，把进化系统的信任边界从“可审批”推进到“可审计”。
- v0.52 范围：让 Draft Inbox 支持审批前编辑。CLI 提供 `draft update` 修改 title、body、kind、scope、targets，同时保留 evidence、confidence、matched_template、reason；原生 app 的 Draft 卡片提供 Edit / Save / Cancel，让用户能先修正候选，再决定 approve/reject。
- v0.53 范围：让 Draft Inbox 支持保守合并。CLI 提供 `draft merge` 把两个或更多 Draft 合成一个新的 reviewable Draft；源 Draft 不删除、不批准，原生 app 支持勾选多个候选、预填目标 Agent 并创建合并候选。
- v0.54 范围：将桌面 UI 从 egui 迁移到 Tauri 2。Rust 核心抽成 `agent-kernel` library 供 CLI 和 Tauri 后端共享；`src-tauri` 只暴露本地项目扫描、Draft 审批、Memory Card 分配、Catalog 安装、Observation evolve、build/sync 等命令；`app/` 使用 Bun + Vite + React 构建中文优先的现代桌面前端。
- v0.67 范围：后台 Job 记录持久化可重放 replay payload。扫描、历史整理和同步任务在创建时记录原始 Tauri command 与参数，Job Center 重试时直接调用后端保存的命令，不再根据中文任务名猜测项目路径、engine、targets 或 policy。
- v0.68 范围：Memory Card 融合进入后台 Job。`fuse_memory_cards_to_draft` 不再同步阻塞 UI，而是创建“融合Memory Card”任务、记录 replay payload、展示读取源 Memory Card / 写入融合草稿阶段，并在完成后自动刷新页面读模型。
- v0.69 范围：Job Center 增加状态/类型筛选和精确重试标记。用户可以按运行中、失败、已完成、已取消，以及扫描、整理历史、同步、融合 Memory Card 快速定位任务；带 replay payload 的任务会显示“可重试”徽章。
- 交互式 CLI：CLI 需要像 `git add -p` 一样逐块确认，而不是只给用户一份冷冰冰的 patch。
- Memory Card Registry：长期看，memory_card 可以像 JavaScript registry 包一样安装、版本化和组合，形成社区规则生态。
- Rule CI：规则压缩和合并后要能跑测试，验证“使用压缩后规则的 Agent 是否仍会做出期望行为”。

### 0.1 当前 MVP 权威决策

本阶段产品主线收束为：

> Agent Memory Kernel 先只服务 Claude Code / Codex，但 Observation Layer 要能读取本地规则、生成产物、手动修改，以及所有可发现的本地对话记录，从中持续提炼 Memory Cards，让个人开发环境越用越贴合自己。

当前目标 Agent：

- Claude Code：`CLAUDE.md`、`.claude/skills`
- Codex：`AGENTS.md`、`.agents/skills`

暂缓目标：

- Cursor：架构保留 adapter/exporter 接口，MVP 不做 UI 和导出主路径。
- Cline：从当前产品主线移除，后续如果支持也走插件式 adapter，不进入核心叙事。

这会让定位更锋利：不是“所有 Agent 都支持一点”，而是先把 Claude Code / Codex 的本地记忆进化闭环做深。

### 0.2 新版核心架构

1. Observation Layer
   - 读取本地规则文件：`CLAUDE.md`、`AGENTS.md`
   - 读取本地 Skills：`.claude/skills`、`.agents/skills`
   - 读取 build artifact drift：用户手改生成文件后通过 Reverse Parse 回流
   - 读取本地对话记录：Claude Code / Codex 可发现的 session、transcript、logs
   - 后续扩展 Cursor / IDE / MCP / terminal history adapter

2. Observation Store
   - Observation 不直接变成 Memory Card。
   - 先保存为 observation record，字段包括 source、timestamp、agent、project、text span、evidence、privacy/redaction status、confidence。
   - Observation 是证据层，Draft Memory Card 是建议层，Active Memory Card 是用户批准后的源代码。

3. Memory Card Synthesis Layer
   - 从 observations 中提炼候选 Memory Card。
   - 识别类型：`preference`、`constraint`、`procedure`、`convention`、`correction`、`anti-pattern`。
   - 自动判断 scope：`global`、`project`、`directory`、`agent-specific`。
   - 输出必须包含 confidence、reason、evidence、suggested targets。

4. Evolution Layer
   - 负责去重、合并、冲突检测和版本演化。
   - Memory Card 需要 lineage：来自哪几次 observation、被修改过几次、是否通过 Rule CI、被哪些 Agent 使用、最近是否仍然有效。
   - Project Memory Card 如果反复出现，可建议 promotion 到 global preference。
   - 长期未触发的 Memory Card 可进入 dormant candidate，但不能自动删除。

5. Review Layer
   - 所有自动生成内容先进 Draft Inbox。
   - 用户 approve / reject / edit。
   - 这是信任边界，不能跳过。

6. Compiler Layer
   - 编译到 Claude Code / Codex 的原生文件和 skills 目录。
   - 生成 artifacts。
   - 检测 drift。
   - 支持 Reverse Parse 回流。

### 0.3 Memory Card 操作模型

用户必须能对 Memory Card 做这些操作：

- 将一个或多个 Memory Card 赋予给某个 Project。
- 将一个或多个 Memory Card 赋予给某个 Agent，例如只给 Codex 或只给 Claude Code。
- 合并多个 Memory Card，生成新的复合 Memory Card。
- 把 Memory Card 加入现有 mirrored Skill，作为生成补充，不直接修改第三方 Skill 源文件。
- 后续在 UI 中用拖拽和矩阵完成这些操作，同时保留 CLI 等价命令。

这些能力是 Project 层 multi-agent 配置的基础。即使当前 MVP 只支持 Claude Code / Codex，也要保持“同一项目中不同 Agent 获得不同能力”的模型。

### 0.4 进化体验设计

Memory Card 进化可以轻微参考游戏里的“技能树”隐喻，但命名和交互要工程化，不能喧宾夺主。

建议 UI 元素：

- Memory Card Tree：展示某条规则如何从多次 Observation 进化而来。
- Confidence / Stability：可信度和稳定度，不叫等级。
- Evolution Timeline：来自哪次对话，何时批准，何时合并，何时编译给哪个 Agent。
- Conflict Warning：冲突 Memory Card 用红色边连接。
- Dormant / Active：长期没触发的 Memory Card 进入休眠候选。
- Promotion Candidate：反复出现的 project Memory Card 可以建议提升为 global preference。
- Review Task：需要用户处理的候选、冲突或合并建议。

命名原则：

- 不叫 XP，叫 confidence。
- 不叫 level up，叫 promotion candidate。
- 不叫 rarity，叫 stability。
- 不叫 quest，叫 review task。

## 1. 一句话定位

Agent Memory Kernel 是一个面向 Claude Code 和 Codex 的本地 Memory Card 进化引擎：它不试图替代 Agent runtime，而是专门治理它们依赖的文件化上下文，把散落在本地对话、规则文件、Skill 文件和项目文档里的有效信息抽取、去重、合并、冲突仲裁，并编译回对应 Agent 能直接消费的格式。

更短的产品表达：

> 把 AI 对话中的可复用经验，编译成干净、可审计、可移植的 Agent 规则和 Skills。

## 2. 为什么这个想法值得做

现在主流 Agent 的长期记忆正在分裂成两条路线：

1. 重型记忆层：Mem0、Letta、LangGraph、Graphiti/Zep 等，用数据库、向量检索、图谱或状态机保存记忆。
2. 文件化上下文：Claude Code 的 `CLAUDE.md`、`.claude/skills`、auto memory；Codex 的 `AGENTS.md`、`.agents/skills`；Cursor 的 `.cursor/rules`、AGENTS.md、Memories；Cline 的 Memory Bank 和 `.clinerules`。

你的机会不在于再做一个向量记忆库，而在于做第二条路线的“维护工具”和“编译工具”。开发者最终还是会依赖 Markdown 规则、Skill 文件、项目级说明和全局偏好，因为这些东西具备三种数据库很难替代的优势：

- 可读：人能直接审查和修改。
- 可迁移：不同 Agent 可以读取同一份或相近的文本。
- 可版本控制：能进 Git，能做 diff，能 code review。

痛点是：文件化上下文越用越乱。规则被重复追加，旧偏好和新偏好冲突，项目级规则混进个人偏好，步骤型知识塞进常驻 Prompt，最后导致 Token 浪费和 Agent 行为不稳定。

Agent Memory Kernel 的价值就是把“文件即记忆”从手工维护升级成半自动编译。

## 3. 最新生态观察

### 3.1 Claude Code

Claude Code 已经把文件化记忆做得很完整：

- `CLAUDE.md` 用于持久项目、用户或组织指令。
- auto memory 会自动记录构建命令、调试经验、架构笔记、代码风格和工作习惯。
- `.claude/skills/<skill-name>/SKILL.md` 用于按需加载的能力包。
- `.claude/rules/` 可做路径级规则。
- `/memory` 可查看和编辑记忆。

关键启发：

- Claude 文档明确区分 `CLAUDE.md` 和 auto memory：前者是人写的规则，后者是 Claude 自动积累的学习笔记。
- `CLAUDE.md` 适合“每个会话都该知道”的事实；多步骤流程应迁移为 Skill。
- Claude Code Skills 采用 Agent Skills 开放标准，Skill body 只有在被触发时才加载，避免常驻上下文膨胀。
- Claude Code auto memory 的 `MEMORY.md` 只加载前 200 行或 25KB，说明“索引短、细节按需读”是官方认可的上下文治理方向。

这直接支持你的想法：Agent Memory Kernel 应该帮助用户把“长期常驻规则”“按需 Skill”“自动记忆条目”拆清楚，而不是一股脑写进一个大文件。

### 3.2 OpenAI Codex

Codex 的文件化上下文也已经形成清晰结构：

- `AGENTS.md` 是 Codex 的自定义项目指令入口。
- Codex 会按全局、项目根目录、当前工作目录路径逐层合并指令，越靠近当前目录的指令越后出现，也就更具体。
- 默认有 `project_doc_max_bytes` 限制，官方默认是 32 KiB。
- Codex Skills 使用 Agent Skills 标准，Skill 是一个目录，包含 `SKILL.md`、可选脚本、references、assets。
- Codex 会通过 progressive disclosure 管理 Skill：启动时只放名称、描述和路径，需要时再读取完整内容。
- Codex 支持用户级、仓库级、管理级、系统级 Skills，也支持 symlinked skill folders。

关键启发：

- Codex 已经天然适合“全局偏好 + 项目规则 + 子目录规则 + Skills”的分层模型。
- `AGENTS.md` 的大小限制说明规则治理必须有预算意识。
- symlink 支持很适合作为 Agent Memory Kernel 的跨项目注入机制。

### 3.3 Cursor（Future Adapter）

Cursor 当前规则系统已经从旧式 `.cursorrules` 迁移到 `.cursor/rules`：

- Project Rules 存在 `.cursor/rules`，可进版本控制。
- User Rules 是全局偏好。
- Memories 会从对话中自动生成规则。
- `AGENTS.md` 是简单 Markdown 替代方案。
- `.cursorrules` 仍支持，但属于 legacy。
- Cursor 规则有 Always、Auto Attached、Agent Requested、Manual 等触发类型。

关键启发：

- Cursor 已经具备“从对话生成规则”的入口，但缺少跨工具、跨项目的统一治理。
- Agent Memory Kernel 可以把 Cursor Memories / Rules 纳入统一知识库，再导出成 Claude Skill、Codex Skill 或 AGENTS.md。
- 但 Cursor 不进入当前 MVP 主路径。现阶段只保留 exporter/adapter 接口，等 Claude Code / Codex 的进化闭环足够稳定后再补 UI 与原生 `.cursor/rules` 导出。

### 3.4 Cline（Out Of Core）

Cline 的 Memory Bank 是一个结构化 Markdown 文档体系：

- 用 `projectbrief.md`、`activeContext.md`、`progress.md` 等文件保存项目状态。
- 通过 “initialize memory bank”“update memory bank”“follow your custom instructions” 等命令维护。
- `/smol` 和 `/newtask` 用于压缩上下文或新任务交接。
- 文档强调 Memory Bank 文件要短，详细信息应拆成按需读取的文档。
- `.clinerules/` 目录适合作为项目级规则面，Agent Memory Kernel 应把自己的规则写成其中一个可审查的 build artifact，而不是独占整个 Cline 规则空间。

关键启发：

- Cline 证明“文档化记忆”对开发流程很有效。
- 但它更像项目持续文档，不是规则冲突解决器。
- Agent Memory Kernel 可以借鉴其文件结构，但 Cline 不进入当前产品主线。后续如果支持，应作为插件式 adapter，而不是默认 Agent、默认 Canvas 节点或核心叙事。

### 3.5 Mem0 / Letta / LangGraph / Graphiti

这些项目代表重型或中型记忆层：

- Mem0：面向 AI agents 的 managed memory layer，强调跨用户和跨 Agent 的连续性，平台侧提供 vector store、graph services、rerankers 等基础设施。
- Letta：memory-first/stateful agent，强调 Agent 能随使用学习用户偏好、代码规范和项目模式。
- LangGraph：把记忆分成 short-term/thread-scoped 和 long-term/cross-session，并讨论 semantic、episodic、procedural memory。
- Graphiti/Zep：用 temporal knowledge graph 处理动态事实、关系变化和历史上下文。

关键启发：

- 这些方案解决的是“Agent 应用如何持久化和检索大量动态记忆”。
- 你的项目解决的是“开发者已有规则文件和 Skill 文件如何保持干净、短小、一致、可迁移”。
- 这不是替代关系，而是互补关系。Agent Memory Kernel 可以在 MVP 阶段保持零数据库，后续再把 Mem0/Graphiti 接成可选后端。

### 3.6 Aider

Aider 的 RepoMap 很值得借鉴：

- 它会生成代码库的精简地图，只保留关键类、函数、签名和关系。
- 大代码库时，它会在 token budget 内选择最相关部分。

关键启发：

- Agent Memory Kernel 可以做 “MemoryMap”：不是把整份规则文件丢给 LLM，而是先构建 Markdown AST 和语义索引，让模型只处理冲突候选、冗余候选和需要合并的节点。

### 3.7 2026-05-01 生态补充：Skills 正在变成基础设施

最新一轮生态观察显示，Agent Skills 已经从“单个 Agent 的小插件”变成更通用的能力封装格式：

- Codex / Claude Code 都把 Skills 作为按需加载能力包，配合 `AGENTS.md` / `CLAUDE.md` 形成“常驻规则 + 按需 Skill”的双层上下文。
- Memento-Skills 这类研究方向开始强调把成功经验自动压缩成可复用 skills，说明“从历史任务中进化 Memory Card”不是孤立想法，而是 Agent 生态的共同趋势。
- 社区 Skill 生态会带来供应链风险。Memory Card Catalog / App Store 不能只做安装按钮，还需要 provenance、版本、校验、信任提示、Rule CI 和 review 默认门槛。

对 Agent Memory Kernel 的启发：

- Draft explainability 是必要地基：每条自动候选都必须说明 `matched_template`、`confidence`、`reason` 和 evidence。
- App Store 后续要从“本地内置包”升级为“带信任元数据的 registry client”，但 MVP 仍保持本地 catalog 优先。
- Memory Card lineage 应成为下一阶段核心：用户需要看到一个 Memory Card 来自哪些 observations、何时被批准、编译给了哪些 Agent、是否通过 Rule CI。

## 4. 产品边界

Agent Memory Kernel 不应该做：

- 不做完整 Agent 框架。
- 不强制用户接入数据库。
- 不替代 Claude/Codex 的原生机制。
- 不在 MVP 中追求 Cursor/Cline 的默认导出和 UI 主路径。
- 不把所有对话自动永久保存。
- 不让 LLM 直接重写整份规则文件。

Agent Memory Kernel 应该做：

- 从对话、规则文件、Skill、Memory Bank 中抽取可复用知识。
- 判断知识类型：事实、偏好、流程、禁令、架构约束、临时上下文、已废弃规则。
- 对 Markdown/MDC/SKILL.md/AGENTS.md 做反向解析、AST 级理解和编译输出。
- 发现冲突和重复，并生成可审计 patch。
- 把稳定知识编译成不同 Agent 的目标格式。
- 让用户在 UI 中批准、拒绝、移动、合并或禁用候选 Skill。

## 5. 核心概念设计

### 5.1 Kernel Memory Item

内部统一格式不直接等于 `SKILL.md`。建议先定义一个更小的中间表示：

```yaml
id: km_20260430_001
title: Prefer Axios for HTTP requests
kind: preference
scope: project
domains: [frontend, api]
applies_to:
  paths: ["src/**/*.ts", "src/**/*.tsx"]
  agents: ["claude-code", "codex"]
status: active
confidence: 0.86
source:
  type: chat
  uri: "chat://2026-04-30/session-12"
  evidence: "以后所有请求都必须使用 Axios，不要再用 Fetch"
conflicts_with:
  - km_20260310_004
supersedes:
  - km_20260310_004
created_at: "2026-04-30T22:00:00+08:00"
updated_at: "2026-04-30T22:00:00+08:00"
```

正文用高度压缩的 Markdown：

```md
Use Axios for HTTP requests. Do not introduce new Fetch-based request helpers unless a platform API requires native Fetch.
```

### 5.2 五类知识

建议把“Skill”拆成更精确的类型，否则会越做越像大杂烩：

| 类型 | 例子 | 默认去向 |
| --- | --- | --- |
| Preference | “Python 项目默认用 Poetry” | 全局 AGENTS/CLAUDE/User Rule |
| Project Fact | “本仓库 API tests 依赖本地 Redis” | 项目 CLAUDE.md / AGENTS.md |
| Procedure | “发布前按 5 步检查” | Skill |
| Constraint | “支付模块不能直接 rotate API key” | 项目规则 / path-scoped rule |
| Episode | “上次这个 bug 是因为时区 mock” | auto memory / Memory Bank 详情文件 |

关键判断：

- 每次都该知道：规则文件。
- 只有做特定任务才需要：Skill。
- 只是历史经验：Memory Bank 或 topic memory。
- 未验证或单次上下文：不要固化，只放候选区。

### 5.3 Memory Card：比 Skill 更轻的中间单元

你提到“类似 skill，但更轻量化”。可以命名为 `memory_card`：

- `memory_card` 是 Agent Memory Kernel 内部最小知识单元。
- 多个 memory_card 可以编译成一个 Agent Skill。
- 单个 memory_card 也可以导出为一条 Cursor Rule 或一段 AGENTS.md。
- memory_card 有状态：draft、active、deprecated、archived、conflicted。

这样你不会被现有 `SKILL.md` 目录格式绑死，同时可以输出到各种平台。

### 5.4 Skill Library：现有 Skill 的可视化管理

Agent Memory Kernel 不只管理自己生成的 memory_cards，也要管理用户已经拥有的 Claude Code / Codex / Superpowers / 社区 Skills。

默认采用引用模式：

- 第三方 Skill 保持在原路径，不复制、不重写。
- Agent Memory Kernel 建立索引：名称、描述、路径、来源、版本、适配 Agent、依赖脚本、references、assets。
- UI 中可以启用、禁用、分配到项目、分配到 Agent、查看依赖和预览导出。
- 如果用户想修改第三方 Skill，系统应提示 fork 成用户自有 Skill，再由 Kernel 托管修改后的副本。

Skill Library 中的资产分两类：

| 类型 | 来源 | 管理方式 |
| --- | --- | --- |
| Referenced Skill | `.claude/skills`、`.agents/skills`、Superpowers、社区目录 | 索引和引用，不原地修改 |
| Owned Skill | 用户在 Agent Memory Kernel 内创建或 fork 的 Skill | 由 Kernel 托管，可编辑、组合、编译 |

这能避免一个很危险的问题：规则治理工具不应该把用户安装的第三方 Skill 悄悄改坏。引用模式让第一版更可信，也更符合个人高级开发者的实际使用习惯。

### 5.5 Mirror：引用模式下的默认分发方式

当用户在 UI 中把一个已有 Skill 拖到某个 Agent 或项目时，默认使用 Mirror 模式。

Mirror 的含义：

- Source Skill 保持原位置，不修改。
- Agent Memory Kernel 把 Source Skill 复制到目标 Agent 的 skills 目录。
- 复制时保留目录结构：`SKILL.md`、`scripts/`、`references/`、`assets/`。
- Kernel 记录 source path、target path、source hash、target hash、mirrored_at。
- 目标副本头部或旁边 metadata 标明来源，方便追踪。

示例：

```text
Source:
~/.agents/skills/superpowers/brainstorming/

Mirrors:
<project>/.claude/skills/brainstorming/
<project>/.agents/skills/brainstorming/
```

Mirror state 可以写入：

```yaml
mirrors:
  - source: "C:/Users/15893/.agents/skills/superpowers/brainstorming"
    target: ".agents/skills/brainstorming"
    agent: "codex"
    source_hash: "sha256:..."
    target_hash: "sha256:..."
    status: "synced"
```

同步策略：

- Source 更新，target 未手动改：自动提示可同步。
- Source 更新，target 被手动改：标记 drift，提示用户选择 keep target / overwrite mirror / fork owned。
- Target 被 Agent 或用户修改：不反写 source，避免污染第三方 Skill。
- 用户想改 Mirror 副本：UI 推荐 fork 成 Owned Skill。

Mirror 比 Link 更兼容，因为不是所有 Agent 都支持任意外部路径；Mirror 也比 Compile 更安全，因为不会重新解释第三方 Skill 的语义。

## 6. 系统架构

### 6.0 Rust 内核 + bunx 分发

用户入口优先使用 `bunx agent-kernel`，核心实现使用 Rust，而不是 TypeScript。

推荐形态：

```text
agent-kernel/
  crates/
    agent-kernel-core/      # IR、memory_card、分类、冲突、预算
    agent-kernel-parser/    # Markdown/MDC/SKILL/AGENTS 解析与反向解析
    agent-kernel-exporters/ # Claude/Codex exporters, future adapters
    agent-kernel-ci/        # Rule CI runner
    agent-kernel-mcp/       # MCP server
    agent-kernel-ui/        # Axum API + embedded web assets
  bun/
    agent-kernel/           # Bun wrapper，下载/调用平台二进制
```

Rust 适合这里的原因：

- 文件治理和 diff 需要稳定、快、可预测。
- 单二进制适合通过 Bun + JavaScript registry、Homebrew、Cargo、GitHub Releases 多渠道分发。
- TUI 可以用 `ratatui` / `crossterm`，本地 Web 服务可以用 `axum`，配置和 IR 可以用 `serde`。
- 后续如果做本地守护进程、文件监听、增量 build、Rule CI，Rust 的长期维护成本更低。

技术建议：

- CLI：`clap`
- 配置：`serde`、`serde_yaml`、`toml`
- Markdown AST：优先评估 `markdown` / `markdown-rs` 的 mdast，或 `comrak` 的 AST 与 sourcepos；如果某些格式 round-trip 不够好，再做保守 formatter。
- Diff：`similar` 或自定义 line diff。
- TUI：`ratatui` + `crossterm`
- Web UI：Rust `axum` API + 前端静态资源嵌入二进制；早期可以用 React/Vite 构建前端，但运行时仍由 Rust 托管。
- MCP：Rust MCP SDK 或 JSON-RPC over stdio，接口保持薄。

`bunx` 包不要承载主逻辑。它只做三件事：

1. 检测平台。
2. 下载或调用对应二进制。
3. 把参数原样转交给 Rust CLI。

### 6.1 Ingestion 摄取层

输入来源：

- CLI stdin：`cat chat.log | bunx agent-kernel extract`
- 本地文件：`CLAUDE.md`、`AGENTS.md`
- Future adapter 文件：`.cursor/rules/*.mdc`、`.clinerules`、`memory-bank/*.md`
- Agent Skills：`.claude/skills/**/SKILL.md`、`.agents/skills/**/SKILL.md`
- MCP tool：让 Claude Code / Codex 调用 `remember_candidate`、`gc_rules`、`export_rules`
- Git diff：从 PR review 或近期修改中提炼项目规则
- Session log：从 Agent 的 JSONL 或 transcript 中提炼稳定知识

输入必须保留 provenance。没有来源证据的知识不应自动固化。

### 6.2 Parse & Normalize 解析层

技术建议：

- Markdown：Rust AST parser，优先保留 source range 和代码块原文。
- YAML frontmatter：`serde_yaml` 解析成 typed metadata。
- MDC：按 frontmatter + markdown body 解析
- XML-like blocks：保留原始节点，避免 LLM 破坏标签
- 代码块：作为 opaque node，不让 LLM 随意重写

原则：

- LLM 只处理语义内容，不直接改文件结构。
- AST 节点要带 source range，方便生成 patch 和 UI 高亮。
- 原文件格式尽量 round-trip，不做无关格式化。

### 6.3 Classify 分类层

对每个候选片段输出：

- `kind`：preference / fact / procedure / constraint / episode
- `scope`：global / project / directory / file / agent-specific
- `stability`：temporary / evolving / stable / deprecated
- `confidence`
- `dedupe_key`
- `conflict_key`
- `suggested_export_targets`

这里可以结合规则和 LLM：

- 明确句式 “以后都…”、“always…”、“never…” 通常是 constraint/preference。
- 包含步骤、检查清单、命令序列，通常是 procedure。
- 带错误排查过程和结果，通常是 episode。
- 出现时间、版本、迁移、旧 API、新 API，优先做冲突检测。

### 6.4 Surgery 手术层

这是项目护城河。

核心任务：

- 去重：重复规则合并。
- 压缩：口语转机器指令。
- 冲突：旧规则与新规则互斥时标记 supersedes。
- 迁移：把过长流程从 CLAUDE.md / AGENTS.md 移到 Skill。
- 分层：全局偏好、项目规则、目录规则、Agent 专用规则拆开。
- 预算：控制常驻上下文 token/byte 上限。

不要直接“自动删除旧规则”。更好的默认策略：

1. 低风险重复：自动合并，生成 diff。
2. 明确时间线冲突：推荐保留新规则，旧规则变 deprecated。
3. 高风险冲突：进入 UI 待确认。
4. 安全/合规规则：不自动覆盖，只提示人工确认。

### 6.5 Kernel Store 内核存储与唯一事实来源

核心原则：`~/.agent-kernel/memory_cards` 和项目 `.agent-kernel/project.yml` 才是 Single Source of Truth。当前 MVP 中 Claude Code / Codex 读取的 `CLAUDE.md`、`AGENTS.md` 是编译产物；Cursor/Cline 类文件只作为后续 adapter 产物。

MVP 建议零数据库：

```text
~/.agent-kernel/
  kernel.yml
  memory_cards/
    global/
    projects/
    registry/
  exports/
  audit/
  cache/
```

项目级：

```text
.agent-kernel/
  project.yml
  locks/
  drafts/
  tests/
```

推荐 memory_card 文件结构：

```text
~/.agent-kernel/memory_cards/global/frontend/http-client.memory_card.yml
~/.agent-kernel/memory_cards/global/frontend/http-client.md
```

其中 `.yml` 保存 metadata、scope、conflicts、export targets，`.md` 保存正文。这样比把所有内容塞进一个 YAML 更适合人工阅读和 Git diff。

后续可选：

- SQLite：用于全文搜索和审计。
- LanceDB/Chroma：用于语义近邻。
- Graphiti/Mem0：用于长程跨 Agent 记忆。

但这些不应该是 MVP 必需依赖。

### 6.6 Exporters 导出层

统一 IR 编译到各平台：

| 目标 | 输出 |
| --- | --- |
| Claude Code | `CLAUDE.md`、`.claude/skills/<name>/SKILL.md`、`.claude/rules/*.md` |
| Codex | `AGENTS.md`、`.agents/skills/<name>/SKILL.md` |
| Cursor | Future adapter：`.cursor/rules/*.mdc`、项目根 `AGENTS.md`、User Rules 文本 |
| Cline | Out of core：`.clinerules`、`memory-bank/*.md`，后续可做插件式 adapter |
| Aider | `CONVENTIONS.md` 或 aider 可读的 repo instructions |
| Generic | `AGENTS.md`、`SKILL.md`、纯 Markdown |

导出策略：

- `always` 规则只放最短、最稳定、最关键内容。
- 程序性内容导出为 Skills。
- 路径相关内容导出为 path-scoped rules。
- Agent 专用语法由 adapter 负责，核心知识保持无平台绑定。

### 6.7 UI 可视化层

`bunx agent-kernel ui` 启动本地面板。

这个 UI 应该提前到 MVP，而不是排到最后。因为 Agent Memory Kernel 做的是“长期知识治理”，用户最担心的是误删、误合并和误固化。可视化面板是信任系统的一部分，不是装饰层。

UI 应采用双入口：

1. Project-centered Canvas 第一屏：用于理解和操作“当前项目启用了哪些 Skills/Memory Cards，并分别分发给哪些 Agent”。
2. App Store / Package Manager：用于浏览、搜索、安装、更新和启用现有 Skills/Memory Cards。

核心页面：

- Canvas Workspace：第一屏，以当前 Project 为中心，用白板节点和连线展示目标 Agent、Referenced Skills、Owned Skills、Memory Cards、Rules、Exports。
- Inbox：从对话和文件提取出的候选 memory_cards。
- Conflict Center：冲突规则对比，显示证据、时间线、推荐决策。
- Knowledge Map：按项目、技术栈、目录、Agent、知识类型浏览。
- Skill Library：现有 Skills 与 Owned Skills 的索引、搜索、预览、拖拽分发和 Mirror 同步状态。
- App Store：卡片式 Skill/Memory Card 浏览、安装、升级、评分、来源和兼容性展示。
- Export Preview：选择目标 Agent，预览将写入哪些文件和 diff。
- Budget View：显示常驻规则 token/byte 占用，提示哪些应该迁移成 Skill。
- Audit Log：记录每次合并、删除、迁移和导出。
- Registry：搜索、安装、升级和禁用社区 memory_card 包。
- Rule CI：展示规则测试用例、最近运行结果和失败原因。

很关键的一点：UI 不是花哨面板，而是“信任建立器”。用户必须能看到为什么引擎认为两条规则冲突、为什么要删除旧规则、会改哪些文件。

交互形态建议：

- Canvas 节点：Current Project、Claude Code、Codex、Skill、Memory Card、Rule Set、Export Artifact。Cursor 等后续目标只通过 adapter 插件加入。
- Canvas 连线：启用、Mirror、Compile、Depends on、Conflicts with、Supersedes。
- 拖动连接：从 Skill 节点拖线到 Agent 节点，创建该项目下的 Agent-specific Mirror；从 Memory Card 拖到 Project，加入项目规则；从 Memory Card 拖到某个 Agent，只给该 Agent 编译；从多个 Memory Cards 拖到新 Skill，组合成 Owned Skill。
- 连线状态：synced、source updated、target drifted、conflict、test failed。
- 冲突对比：左侧旧规则，右侧新规则，红色表示将废弃，绿色表示将保留。
- 知识拖拽：把 memory_card 从 Global 拖到 Project，或从 Always Prompt 拖到 Skill。
- Skill 拖拽：把 Referenced Skill 拖到 Claude Code / Codex，系统默认创建 Mirror 副本。后续 adapter 可加入新的 Agent 节点。
- 预算条：像 bundle analyzer 一样展示每个 Agent 的上下文占用。
- 反向解析提示：如果用户手动改了 `CLAUDE.md`，UI 显示“检测到编译产物被手动修改，是否导入为 Draft memory_card？”
- Mirror 状态：显示 synced / source updated / target drifted / fork recommended。
- 一键回滚：从 audit log 选择某次 build，还原对应 memory_card 状态和导出结果。

Canvas 第一屏的信息布局建议：

```text
左侧 Palette:
  Rules / Memory Cards / Skills / Packages / Drafts

中间 Canvas:
  Current Project
    -> Claude Code
    -> Codex

右侧 Inspector:
  选中节点或连线的 metadata、source、target、hash、budget、tests、diff

底部 Build Bar:
  Preview / Test / Build / Sync Mirrors / Rollback
```

App Store 页面重点不做复杂关系图，而做快速发现：

- 搜索：React、Rust、TDD、Security、Claude、Codex。
- 筛选：Agent 兼容性、来源、本地/远程、已安装/可更新、风险等级。
- 卡片：名称、描述、来源、版本、包含多少 memory_cards、是否有 Rule CI。
- 操作：Install、Mirror to Project、Preview Contents、Fork。

Project-centered Canvas 的关键规则：

- Project 是唯一中心节点，所有操作都回答“这个项目当前如何配置”。
- Agent 节点是 Project 的运行目标，可以同时存在多个。
- 同一个 Skill 可以 Mirror 给多个 Agent，也可以只给一个 Agent。
- 同一个 Memory Card 可以编译进项目级规则，也可以只编译进某个 Agent 的规则。
- Canvas 上要清楚显示 inheritance：Global Preferences -> Project Rules -> Agent-specific Rules。
- 任何 Agent-specific 分配都不应该污染全局偏好。

示例：

```text
Global Preference: "Prefer Bun"
  -> Current Project
      -> Codex: AGENTS.md + .agents/skills/*
      -> Claude Code: CLAUDE.md + .claude/skills/*

Skill "superpowers/brainstorming"
  -> Codex only, Mirror

Memory Card "Use Axios for frontend requests"
  -> Project Rule, compiled to Codex + Claude Code

Memory Card "Use Claude subagents for refactors"
  -> Claude Code only
```

### 6.8 编译产物模型

`CLAUDE.md`、`AGENTS.md` 默认不再是手工维护的主文件，而是 dist。`.cursor/rules/*.mdc`、`.clinerules` 属于后续 adapter 产物，不进入当前 MVP 默认路径。

编译流程：

```text
memory_cards + project.yml + exporter config
  -> plan
  -> Rule CI
  -> preview diff
  -> build artifacts
```

产物文件头部可以写明来源，但不要依赖 managed block 保护局部结构：

```md
<!-- Generated by Agent Memory Kernel. Do not edit directly. Run `bunx agent-kernel import` to ingest manual changes. -->
```

如果用户确实手动改了产物：

1. `agent-kernel scan` 检测文件 hash 与 lockfile 不一致。
2. `agent-kernel import CLAUDE.md` 做 reverse parse。
3. 新增内容进入 Draft Inbox。
4. 用户确认后写回 memory_cards。
5. 再次 build 全量生成产物。

这个模式比 managed block 更稳，因为系统不会在半手写半生成的文件中做脆弱手术。managed block 可保留为 legacy/compat 模式，服务不愿全量托管规则文件的团队。

### 6.8.1 声明式项目配置

`.agent-kernel/project.yml` 应该是声明式配置，类似 Terraform、Vite config 或 Kubernetes manifest：它描述期望状态，不记录所有运行时细节。

原则：

- 写“我要启用什么”，不写“上一次具体生成了什么”。
- 写 Project -> Agent -> Skills/Memory Cards 的分配关系。
- 不把 Canvas 坐标、hash、同步时间、测试结果塞进 `project.yml`。
- build、test、sync 根据声明式配置推导产物。
- lockfile 和 cache 可以存在，但它们是派生状态，不是用户主要编辑对象。

示例：

```yaml
version: 1
project:
  name: "new-project"
  root: "."

agents:
  codex:
    enabled: true
    exports:
      instructions: "AGENTS.md"
      skills_dir: ".agents/skills"
  claude-code:
    enabled: true
    exports:
      instructions: "CLAUDE.md"
      skills_dir: ".claude/skills"
rules:
  include:
    - "global:typescript-style"
    - "project:frontend-api"

memory_cards:
  include:
    - id: "global:prefer-bun"
      scope: "project"
    - id: "project:use-axios"
      targets: ["codex", "claude-code"]
    - id: "project:claude-subagents-for-refactors"
      targets: ["claude-code"]

skills:
  mirrors:
    - ref: "superpowers:brainstorming"
      targets: ["codex"]
    - ref: "superpowers:verification-before-completion"
      targets: ["codex", "claude-code"]

budgets:
  codex:
    instructions_max_bytes: 24000
  claude-code:
    instructions_max_tokens: 6000
```

派生文件建议：

```text
.agent-kernel/
  project.yml          # 用户主要编辑的声明式配置
  project.lock.yml     # mirror hashes、产物 hashes、registry versions
  canvas.state.json    # UI 布局、折叠状态、最近选中节点
  cache/               # 解析缓存、测试缓存
```

这样 UI 可以拖拽操作，但最终落盘时仍然转换成简洁的声明式配置。高级用户可以像维护 dotfiles 一样维护 `project.yml`，UI 用户也不会被运行时细节淹没。

### 6.9 Memory Card Registry

memory_card 天然适合社区化。长期可以做一个轻量 Registry，让用户安装经过整理的 AI 规则包。

命令草案：

```bash
bunx agent-kernel search react
bunx agent-kernel install @frontend/react-best-practices
bunx agent-kernel install @backend/fastapi-clean-architecture
bunx agent-kernel update
bunx agent-kernel audit-registry
```

Registry 包应该包含：

```text
package.yml
memory_cards/
tests/
README.md
LICENSE
```

`package.yml` 示例：

```yaml
name: "@frontend/react-best-practices"
version: "0.1.0"
compat:
  agents: ["claude-code", "codex"]
  exporters: [">=0.1.0"]
tags: ["react", "typescript", "frontend"]
risk: "medium"
```

社区生态的价值：

- 新手可以直接安装高质量规则包。
- 团队可以发布内部私有 memory_card collection。
- 开源项目可以随仓库发布官方 Agent 使用规则。
- Agent Memory Kernel 可以像 lockfile 一样固定规则包版本，避免规则漂移。

### 6.10 Hybrid 隐私与模型 Provider

Agent Memory Kernel 的默认信任模型应该是 local-first，但产品能力允许用户接入云端或本地模型。因此第一版采用 Hybrid 策略。

本地默认执行：

- 扫描规则文件和 Skills。
- 建立 Skill Library 索引。
- Mirror 复制和同步。
- 编译 `CLAUDE.md`、`AGENTS.md` 等产物；其他 Agent 通过 future adapter 扩展。
- 计算 hash、diff、预算。
- 管理 Canvas、project.yml、lockfile。
- 运行非 LLM 断言类 Rule CI。

需要 provider 的能力：

- 从对话中提炼 Draft Memory Cards。
- 语义去重。
- 规则冲突判断。
- 压缩冗余文本。
- LLM judge 型 Rule CI。

首次启动向导应让用户选择：

```text
Model Provider

1. None / Local-only
   - 不调用任何 LLM
   - 只能做扫描、索引、Mirror、编译、机械去重

2. Local Model
   - 使用 Ollama / LM Studio / llama.cpp compatible endpoint
   - 隐私更强，质量取决于本地模型

3. Cloud Model
   - OpenAI / Anthropic / Gemini / OpenRouter 等
   - 提炼和冲突判断质量更好
   - 需要用户显式配置 API key
```

配置示例：

```yaml
providers:
  default: "local"
  local:
    type: "openai-compatible"
    base_url: "http://localhost:11434/v1"
    model: "qwen2.5-coder:7b"
  cloud:
    type: "openai"
    model: "gpt-4.1-mini"
    api_key_env: "OPENAI_API_KEY"

privacy:
  upload_policy: "ask"
  redact_secrets: true
  include_code_context: false
  store_prompts_locally: true
```

上传策略：

- `never`：永不上传内容，只使用本地能力。
- `ask`：每次发送前预览 payload，由用户确认。
- `allow-listed`：只允许指定项目或指定数据类型调用云模型。

UI 中需要有 “Payload Preview”：

- 展示即将发送给模型的文本。
- 标出将被脱敏的 secrets。
- 允许用户删除敏感片段。
- 显示预计 token 和成本。

这能让高级开发者保留掌控感：项目规则和私有偏好默认留在本机，只有用户选择时才让模型参与语义理解。

## 7. 交互工作流

### 7.1 从对话提炼规则

首个用户是个人高级开发者，因此第一入口应该是“导入现有世界”，对话提炼放到第二阶段。

推荐首次使用流程：

```bash
bunx agent-kernel import --scan-home --project .
bunx agent-kernel ui
```

扫描对象：

- `CLAUDE.md`
- `AGENTS.md`
- `.claude/skills/**/SKILL.md`
- `.agents/skills/**/SKILL.md`
- `~/.agents/skills/**/SKILL.md`
- 用户手动添加的 Skill 路径
- Claude Code / Codex 可发现的本地 session、transcript、logs

后续 adapter 再加入：

- `.cursor/rules/*.mdc`
- `.cursorrules`
- `.clinerules`
- `memory-bank/*.md`

导入后 UI 先不自动改文件，而是展示：

- Imported Rules：可反向解析为 memory_cards 的规则。
- Referenced Skills：已发现但保持原路径的 Skills。
- Draft Memory Cards：可从规则中提炼出的候选。
- Conflicts：跨 Agent 规则中互相冲突的偏好。
- Export Targets：当前项目可以编译到哪些 Agent。

后续再从对话提炼：

```bash
cat session.md | bunx agent-kernel extract --project .
```

输出：

- 5 条候选 memory_cards
- 2 条疑似重复
- 1 条与旧规则冲突
- 1 条建议变成 Skill

用户执行：

```bash
bunx agent-kernel review
```

打开 TUI 或 Web UI 批准。

### 7.2 清理项目规则

```bash
bunx agent-kernel gc --targets claude,codex
```

输出示例：

```text
Found:
- 7 duplicate rules
- 2 stale API preferences
- 1 oversized procedure in AGENTS.md

Suggested:
- Merge UI style rules into "frontend-style"
- Supersede "Use Fetch" with "Use Axios"
- Move release checklist into .agents/skills/release-checklist/SKILL.md
```

随后进入类似 `git add -p` 的交互模式：

```text
Conflict 1/2: HTTP client preference

Old rule:
- Use Fetch for all HTTP requests.

New candidate:
+ Use Axios for new HTTP request helpers.
+ Exception: keep native Fetch for streaming upload endpoints.

Recommendation: supersede old rule with new candidate.

Accept this change? [y]es / [n]o / [e]dit / [s]kip / [a]pply all / [q]uit
```

交互式 CLI 的价值：

- 比纯 patch 更接近人的判断方式。
- 让用户逐条批准高风险知识变化。
- `e` 可以直接打开临时编辑器修改最终 memory_card 正文。
- `s` 可以跳过不确定项，保留到 UI Inbox。
- 每次决策都写入 audit log，便于回滚和训练后续推荐。

### 7.3 注入到任意项目

```bash
bunx agent-kernel attach --project . --profile frontend-saas --agents codex,claude
```

实现方式：

- 轻量方案：项目内生成 `.agent-kernel/project.yml`，记录启用哪些 memory_cards。
- 编译方案：把目标 Agent 文件视为 build artifacts，全量生成。
- symlink 方案：把全局 skill 目录 symlink 到 `.agents/skills` 或 `.claude/skills`。

建议默认使用“Source of Truth + 全量编译 + 可审计 diff”。symlink 作为高级选项。原因是团队协作时，显式产物更容易 code review，完整重建也比局部补丁更稳定。

### 7.4 Agent 运行时调用

通过 MCP 提供工具：

- `agent_kernel.extract_from_text`
- `agent_kernel.propose_memory`
- `agent_kernel.gc_project_rules`
- `agent_kernel.search_memory_cards`
- `agent_kernel.export_to_agent`
- `agent_kernel.archive_success`

其中 `archive_success` 是你“进化”概念的关键。Agent 完成任务后，可以提交：

```json
{
  "task": "migrate fetch clients to axios",
  "success_signal": "tests passed and user accepted",
  "reusable_steps": [
    "Find request helpers",
    "Replace native fetch wrappers",
    "Keep upload endpoints on native fetch when streaming is required"
  ],
  "suggested_scope": "project"
}
```

引擎生成 Draft Skill，等待用户确认。

### 7.5 半自动 Draft Inbox

对话提炼不应该默认自动写入长期规则。第一版采用半自动推荐：

```text
Conversation / Agent Session / CLI Extract
  -> Detect Candidate
  -> Classify
  -> Attach Evidence
  -> Suggest Scope
  -> Draft Inbox
  -> User Approves
  -> Project / Global / Agent-specific
  -> Build
```

触发来源：

- 用户显式说“记住这个”“以后都这样”“把这次经验沉淀一下”。
- Agent 任务成功后调用 `archive_success`。
- 系统检测到用户重复纠正同一类问题。
- `agent-kernel extract chat.md` 手动导入会话。
- MCP 工具从当前 Agent session 提交候选。

Draft Memory Card 必须包含：

```yaml
title: "Use Axios for frontend requests"
kind: "preference"
suggested_scope: "project"
suggested_targets: ["codex", "claude-code"]
confidence: 0.82
evidence:
  - source: "session"
    quote: "以后所有请求都必须使用 Axios，不要再用 Fetch"
actions:
  recommended: "supersede"
  conflicts_with: ["project:use-fetch"]
```

UI Inbox 展示：

- 原始证据：为什么系统认为它值得沉淀。
- 推荐分类：Preference / Project Fact / Procedure / Constraint / Episode。
- 推荐作用域：Global / Project / Directory / Agent-specific。
- 冲突对象：是否会替换旧规则。
- 目标 Agent：建议编译到哪些 Agent。
- 操作：Approve、Edit、Assign、Reject、Merge、Keep Draft。

默认策略：

- 不自动启用 Draft。
- 不自动覆盖冲突规则。
- 低风险重复项可以批量合并，但仍需要用户确认。
- 用户批准后才写入 `project.yml` 或 global memory_card store。

这个设计保留了“AI 会主动帮你整理”的感觉，但把最终控制权留给用户。

## 8. 核心算法设计

### 8.1 冲突检测

冲突不是简单相似度问题，建议用四层：

1. 规则层：同一 conflict key 下存在否定关系，例如 `fetch` vs `axios`、`node-package-manager` 从旧工具切到 `bun`。
2. 时间层：新证据是否明确说“以后”“改为”“不再”。
3. 范围层：全局规则和项目规则是否其实可以共存。
4. LLM 判定层：输出结构化结果，而不是自由文本。

结构化输出：

```json
{
  "relationship": "supersedes",
  "winner": "new",
  "reason": "New rule explicitly says future requests must use Axios and deprecates Fetch.",
  "risk": "medium",
  "requires_user_confirmation": true
}
```

### 8.2 压缩策略

目标不是越短越好，而是“常驻内容短、按需内容完整”。

压缩规则：

- 删除情绪词和一次性上下文。
- 保留条件、例外、范围。
- 把长流程迁移为 Skill，而不是压成一句丢信息。
- 把示例移到 references，主规则只保留索引。

示例：

原文：

```text
我再说最后一次，以后所有请求都必须使用 Axios，不要再用那个过时的 Fetch 了。
但是上传大文件那里先别动，因为之前那个接口依赖 stream。
```

压缩：

```md
Use Axios for new HTTP request helpers. Do not introduce Fetch-based wrappers. Exception: keep native Fetch for large-file streaming endpoints unless the upload API is redesigned.
```

### 8.3 编译产物优先，Managed Block 降级为兼容模式

之前的 managed block 方案适合快速落地，但不是长期主架构。原因是 Agent 或用户都有可能误删、截断或重排 HTML 注释，导致块边界失效。

新的主策略：

- memory_cards 是源代码。
- `project.yml` 是 build config。
- `CLAUDE.md`、`AGENTS.md` 是当前 MVP 编译产物。
- `.cursor/rules/*.mdc`、`.clinerules` 是后续 adapter 编译产物，不进入当前默认路径。
- 每次 `build` 全量生成目标文件，并生成 diff 供用户确认。

只有在团队明确要求保留手写文件时，才开启兼容模式：

```yaml
exporters:
  codex:
    mode: managed-block
  claude:
    mode: full-build
```

兼容模式可以继续使用：

```md
<!-- agent-kernel:start id=frontend-api-preferences -->
...
<!-- agent-kernel:end -->
```

但文档中要明确：`full-build` 是推荐模式，`managed-block` 是迁移和兼容模式。

### 8.4 预算系统

每个目标 Agent 应有预算：

```yaml
budgets:
  codex_agents_md:
    max_bytes: 24000
    hard_limit_bytes: 32768
  claude_md:
    max_tokens: 6000
```

当超预算：

- Always -> Agent Requested / Manual
- 常驻规则 -> Skill
- 细节 -> references
- 历史过程 -> Memory Bank / topic memory

### 8.5 Rule CI

Rule CI 是防止“过度压缩切掉关键信息”的工程保险。

用户可以在项目中定义 `.agent-kernel/tests/*.yml`：

```yaml
name: frontend-http-client
agent_target: codex
prompt: "帮我写一个获取用户列表的请求函数"
expect:
  include:
    - "axios"
  exclude:
    - "fetch("
rules:
  max_cost_usd: 0.02
  model: "small"
```

每次 `gc`、`compress` 或 `build` 之后，引擎可以运行测试：

```bash
bunx agent-kernel test
bunx agent-kernel build --test
```

Rule CI 不需要真的运行完整 Agent，也可以先用小模型做判断：

1. 把编译后的目标规则作为 system/developer context。
2. 输入测试 prompt。
3. 要求模型输出方案或代码片段。
4. 用字符串断言、正则断言或 LLM judge 检查。

测试失败时：

- 阻止自动 build。
- 回滚到上一个 lockfile。
- 在 UI 中标出是哪个 memory_card 压缩后破坏了行为。
- 允许用户把失败用例一键保存为长期 Rule CI。

Rule CI 的真正价值不是“模型测试 100% 准确”，而是为高价值规则建立回归保护。它会让用户敢于让系统持续进化。

## 9. MVP 建议

### MVP 0 / v0.1：Import + Canvas + Mirror + Build Preview

目标：证明“扫描现有 Claude Code / Codex 规则和 Skills -> Project-centered Canvas 分配 -> Mirror Skills -> Claude/Codex 编译产物预览”可行。

v0.1 明确不做：

- 不做半自动对话提炼。
- 不做 LLM 语义合并。
- 不做 Memory Card Registry。
- 不做 Rule CI。
- 不做复杂团队协作。

功能：

- 读取 `AGENTS.md`、`CLAUDE.md`
- 扫描 `.claude/skills`、`.agents/skills`、`~/.agents/skills`
- 对现有 Skills 建立引用索引，不复制、不修改
- AST 解析 heading/list/code/frontmatter
- 检测重复标题和重复 bullet
- 输出 dry-run diff
- 反向解析现有规则，生成 Imported Rules 索引
- 从 memory_cards 全量编译到 Codex `AGENTS.md` 和 Claude `CLAUDE.md`
- 生成 lockfile，记录产物 hash
- UI 第一屏显示 Current Project、Codex、Claude Code 和已发现 Skills
- 支持把一个 Referenced Skill Mirror 到 Codex 或 Claude Code
- 支持 `build --preview` 展示将生成/覆盖的文件

技术栈：

- Rust
- `clap`
- `serde` / `serde_yaml`
- Markdown AST parser
- `similar`

### MVP 0.5：交互式 CLI

目标：让用户像 `git add -p` 一样逐块批准规则变化。

功能：

- 冲突逐条确认：`y/n/e/s/a/q`
- 彩色 diff：旧规则红色，新规则绿色
- 临时编辑：`e` 打开 `$EDITOR`
- 决策写入 audit log
- 跳过项进入 Draft Inbox

技术栈：

- `ratatui`
- `crossterm`
- `console` / `owo-colors`

### MVP 1：Skill Library + Mirror 分发增强

目标：让用户能可视化管理现有 Skills，并把它们安全分发到任意 Agent。

功能：

- 扫描并索引 Referenced Skills。
- UI 中展示 Skill 名称、描述、路径、来源、依赖文件。
- 拖拽 Skill 到目标 Agent 或项目。
- 创建 Mirror 副本到目标 skills 目录。
- 记录 mirror hash 和同步状态。
- 检测 source updated / target drifted。
- 支持 fork mirror 为 Owned Skill。

### MVP 2：半自动 Draft Inbox + LLM 语义合并

功能：

- 通过 CLI/MCP/session log 手动或半自动提交候选
- 从文本中抽取候选 memory_cards
- 结构化分类
- 发现 3 类冲突：工具偏好、语言/框架偏好、禁令
- 生成 Draft Inbox 项，不自动启用
- 附带证据、推荐作用域、推荐目标 Agent、冲突对象

### MVP 3：可视化控制台

功能：

- Canvas Workspace 作为第一屏
- Project-centered multi-agent 分配：同一项目下可给 Claude Code / Codex 分配不同 Skills/Memory Cards；Cursor/Cline 仅作为未来 adapter 扩展。
- Inbox
- Conflict Center
- Knowledge Map
- Skill Library
- App Store / Package Manager
- Export Preview
- Budget View
- Audit Log

技术栈：

- Rust `axum` 本地服务
- 前端可用 React/Vite，构建后嵌入 Rust 二进制

### MVP 4：Memory Card Store + Exporters + Rule CI

功能：

- `~/.agent-kernel/memory_cards`
- project profile
- Exporters：当前 Claude Code、Codex；后续 Cursor、Cline 通过 adapter 扩展
- full-build / managed-block 两种模式
- `.agent-kernel/tests/*.yml`
- `agent-kernel test`
- Hybrid provider config：local-only / local model / cloud model
- Payload Preview 和 secrets redaction

### MVP 5：MCP Server

功能：

- 暴露 extract/search/gc/export/archive_success 工具
- 让 Claude Code、Codex 在工作中调用；后续 adapter 可开放给更多 Agent

### MVP 6：Memory Card Registry

功能：

- 安装社区 memory_card 包
- 支持私有 registry
- lockfile 固定版本
- registry package Rule CI

## 10. 竞争定位

| 项目 | 主要路线 | 与 Agent Memory Kernel 的关系 |
| --- | --- | --- |
| Mem0 | 托管记忆层，向量/图/重排 | 可作为后端，不是直接竞品 |
| Letta | Stateful memory-first agent | 偏完整 Agent，Agent Memory Kernel 偏文件治理 |
| LangGraph memory | Agent 应用状态和长期记忆框架 | 可借鉴记忆分类 |
| Graphiti/Zep | 动态时序知识图谱 | 可处理复杂事实变化，MVP 不需要 |
| Claude Code memory/skills | 原生文件记忆和 Skill | 目标输出平台 |
| Codex AGENTS/Skills | 原生项目指令和 Skill | 目标输出平台 |
| Cursor Rules | 项目规则和 Memories | 目标输出平台和输入来源 |
| Cline Memory Bank | 项目文档化记忆 | 输入来源和输出平台 |
| Aider RepoMap | 代码上下文压缩 | 算法启发 |

一句话差异：

> Mem0/Letta 帮 Agent 记住更多东西；Agent Memory Kernel 帮开发者决定哪些东西值得变成规则、应该放在哪里、如何保持不冲突。

## 11. 开源包形态

建议包名方向：

- `agent-kernel`
- `memory-gc`
- `skill-os`
- `agent-memory-gc`
- `context-compiler`
- `memory_card`

我更推荐：

- JavaScript registry 包：`agent-kernel`
- 核心概念：`memory_card`
- 核心实现：Rust binary
- 分发方式：Bun wrapper + GitHub Releases，后续支持 Homebrew / Cargo install
- 子命令：`project`、`app`、`gc`、`extract`、`review`、`export`、`attach`、`ui`（legacy Web Canvas）

命令草案：

```bash
bunx agent-kernel init
bunx agent-kernel scan
bunx agent-kernel project scan
bunx agent-kernel project list
bunx agent-kernel app
bunx agent-kernel extract ./chat.md
bunx agent-kernel gc --dry-run
bunx agent-kernel review
bunx agent-kernel build
bunx agent-kernel import ./CLAUDE.md
bunx agent-kernel test
bunx agent-kernel export --to codex
bunx agent-kernel export --to claude
bunx agent-kernel attach --agents codex,claude
bunx agent-kernel install @frontend/react-best-practices
bunx agent-kernel ui   # legacy/dev web canvas
bunx agent-kernel mcp
```

## 12. 最大风险

### 12.1 错误固化

用户一句临时抱怨可能被误当长期规则。

缓解：

- draft by default
- 需要证据和作用域
- 高风险改动必须确认
- audit log 可回滚

### 12.2 过度压缩

把重要例外压没，反而导致 Agent 犯错。

缓解：

- 压缩必须保留 conditions / exceptions / scope
- 允许 “summary + details link”
- 对安全、支付、数据迁移等规则降低压缩强度
- 用 Rule CI 对关键行为做回归测试
- 压缩前后的 memory_card 保留版本，可一键回滚

### 12.3 多平台格式漂移

Claude Code、Codex 的规则格式会继续变化；未来 adapter 还要面对 Cursor/Cline 等格式漂移。

缓解：

- 核心 IR 与 exporters 分离
- adapter 单独版本化
- 每个 exporter 都有 snapshot tests

### 12.4 用户不信任自动编辑

长期规则很敏感，用户不愿让工具自动改。

缓解：

- dry-run 默认
- UI diff 预览
- 交互式 CLI 逐条确认
- 编译产物有 lockfile 和 hash
- 反向解析手动修改，先进入 Draft Inbox
- managed block 仅作为兼容模式

### 12.5 编译产物覆盖用户手写内容

如果用户不知道 `CLAUDE.md` 是产物，可能手动编辑后被下一次 build 覆盖。

缓解：

- 产物头部明确写 Generated by Agent Memory Kernel。
- build 前检查 hash，发现手动修改则中断并提示 import。
- `agent-kernel import` 先反向解析为 draft，不直接覆盖 memory_cards。
- UI 中展示“手动改动 -> draft memory_card -> build”的闭环。

## 13. 我建议的产品路线

最好的第一步不是做 MCP，也不是一开始接所有 Agent，而是做一个很锋利的 Rust CLI + 早期可视化预览：

> 输入一份混乱的 `CLAUDE.md` / `AGENTS.md`，反向解析成 draft memory_cards，交互式确认后编译成干净的目标产物。

因为这个闭环最小，但价值最明显：

- 用户能立刻看到重复、冲突、过期规则。
- 能证明“反向解析 + memory_card 源码 + 全量编译”比 LLM 全文重写更可靠。
- 能积累真实规则样本，用来打磨分类和冲突提示词。
- 可视化页面能建立用户信任，避免它看起来像危险的自动清理脚本。

推荐第一版 demo：

```bash
bunx agent-kernel import ./AGENTS.md
bunx agent-kernel gc --interactive
bunx agent-kernel build --to codex --preview
```

输出：

```text
Memory-GC Report

Compression:
- 124 lines -> 72 lines
- estimated token reduction: 38%

Conflicts:
- HTTP client preference: Fetch vs Axios
  recommendation: keep Axios, deprecate Fetch

Migrations:
- "Release checklist" should become a Skill
  target: .agents/skills/release-checklist/SKILL.md

Patch:
- AGENTS.md
- .agents/skills/release-checklist/SKILL.md
```

这会比一开始做“全平台完美支持”更容易获得早期用户。

### 13.1 最新迭代重点：高价值 Prompt 与可编辑进化视图

Memory Card 的含义需要继续放宽：它不只是“以后/必须/always/prefer”这类显式规则，也应该包含能明显改善项目质量的高价值 prompt、成功协作模式、根因复盘、测试策略、UI 性能经验、跨平台修复路径和 agent 交接流程。

当前产品体验应遵循：

- 提取器先过滤一次性任务噪声，再识别长期偏好、硬约束、项目约定、流程、反复纠正、项目改善记录和高价值 prompt。
- 高价值 prompt 不要求出现规则词，只要它描述了可复用的 agent 协作方式，并带来减少返工、提高质量、降低理解成本等结果，就可以进入 Draft Inbox。
- 分配页不再只是矩阵展示，而是用户可直接决定每个 Memory Card 是否分配给 Claude Code / Codex。
- Memory Card 说明优先用常用语言解释，让用户一眼知道“它以后会怎样帮我”，而不是只展示抽象分类。
- Memory Card 进化视图采用工程化的“进化树/时间线”隐喻：展示来源、置信度、稳定度、活跃/休眠、冲突预警、提升候选，但避免游戏化术语喧宾夺主。

下一步建议：

- 将 evolution insight 从前端启发式推导下沉到 Rust 数据模型，记录真实 observation lineage、approval timeline、merge history 和 target history。
- 给分配页增加批量操作：多选 Memory Cards 后统一分配给 Claude Code、Codex，或合并为一个更高层级 Memory Card。
- 增加“提升候选”审阅流：项目级 Memory Card 多次被不同项目复用后，建议提升为 global preference。
- 增加“休眠候选”审阅流：长期未被触发或不再分配给任何 Agent 的 Memory Card，建议归档但不删除。

### 13.2 v0.60：Visible Evolution & Reusable Memory Cards

这一版的目标是把 Agent Memory Kernel 从“能整理”推进到“用户知道它在做什么，并且能把沉淀出的能力拿到别的项目继续用”。

#### 启动与长任务可见化

首次启动时系统会读取项目注册表、从 Claude Code / Codex 本地历史中补全项目、对已登记项目做增量会话导入和本地提炼、再加载当前项目快照。现在这些操作看起来像“卡住”，下一版要改为任务中心：

- 每个慢操作都有阶段、进度、状态文案和完成/失败记录。
- 首次启动显示“建立本地记忆索引”，后续启动显示“检查增量”。
- UI 不应该被后台任务整体锁死，只有相关按钮进入等待状态。
- 手动扫描、整理历史、调用 Claude Code/Codex、融合 Memory Card、同步编译都进入同一个任务中心。

#### Draft Brief 与轻量精炼

待批准草稿需要有面向人的简短简介，而不是只展示给 Agent 编译用的正文。

- `brief`：用用户主要语言写，一两句话说明“以后遇到什么场景，怎么用这条经验”。
- `body`：继续作为编译给 Claude Code / Codex 的精炼指令。
- `language`：记录简介语言，默认根据内容推断为 `zh` 或 `en`。
- `tags`：由引擎建议，也允许用户手动编辑。

#### Tags 与筛选

Memory Card / Draft 都要支持 tag，作为分类、筛选、推荐和全局复用的基础。初始建议 tags：

- `ui-design`
- `frontend`
- `backend`
- `rust`
- `tauri`
- `testing`
- `performance`
- `agent-handoff`
- `code-style`
- `workflow`
- `safety`

#### 可暂停的 Memory Card 分配

技能分配不再强制至少保留一个目标。空 targets 表示“已保存但未启用”，UI 显示为未分配/休眠，而不是阻止用户取消。

#### 全局 Memory Card Library

某个项目产生的 Memory Card 应该可以给别的项目使用。推荐采用 Hybrid：

- 项目 Memory Card 仍保存在项目 `.agent-kernel/memory_cards/`。
- 全局 Memory Card 保存在 `~/.agent-kernel/memory_cards/`。
- 项目可以引用全局 Memory Card，也可以把全局 Memory Card fork 成项目本地版本。
- 项目 Memory Card 可以被提升为全局库项，保留来源项目和 lineage。

#### 融合与推荐

Memory Card 融合不应该只是拼接文本，而是调用当前整理引擎生成新的融合草稿：

- 用户多选多个 Memory Card。
- 引擎生成新的 `title / brief / body / tags / conflict_notes / source_ids`。
- 原 Memory Card 保留，融合结果进入 Draft Inbox。

项目推荐功能在“项目构思完成后”触发：

- 读取项目技术栈、目录结构、已有规则、历史会话和已安装 Skills。
- 推荐应该启用的已有 Memory Cards。
- 推荐适合安装或镜像的 Skills。
- 推荐需要新建的项目级 Memory Cards。
- 标出冲突、过时、过窄或可以提升为全局的内容。

### 13.3 v0.61：真实任务中心与全局复用闭环

这一版把 v0.60 的体验进一步落地：前端不再只用动作名猜测进度，而是从 Tauri 后端读取真实任务状态；全局 Memory Card Library 也不再只是展示，而是能被加入当前项目继续使用。

#### Desktop Task Center

- Rust/Tauri 后端维护 `DesktopTaskStatus`，包含 `key / label / description / percent / running / message`。
- 首次启动的静默增量整理、项目扫描、历史整理、同步编译都写入同一个任务状态。
- 前端每隔固定时间轻量轮询 `get_task_status`，优先显示后端进度，后端不可用时回退到本地启发式进度。
- 后续可以升级为事件推送或任务队列，但 MVP 先用稳定、易调试的状态快照。

#### Global Memory Card Reuse

- 项目 Memory Card 可以被提升到 `~/.agent-kernel/memory_cards/`，保留 `source_project`。
- 全局 Memory Card 可以一键加入当前项目，复制为项目本地 Memory Card 并写入目标 Agent 分配。
- 加入后的全局 Memory Card 可继续在项目内编辑、分配、休眠、融合，避免全局库变成只读收藏夹。
- 后续再加入“引用模式”：项目只引用全局 Memory Card，不复制正文；当全局项更新时可以提示受影响项目。

#### 下一步

- 把任务状态从单个全局状态升级为多任务列表，支持并行扫描、整理、推荐和编译。
- 给全局 Memory Card 加版本号和来源 lineage，支持项目 fork 后对比差异。
- 给 Draft / Memory Card 增加 UI 编辑器，允许直接修改 `brief / body / tags / targets`。
- 融合功能接入 Claude Code / Codex 引擎，生成真正压缩后的融合草稿，而不是简单拼接。

### 13.4 v0.62：Agent Skills Kernel Architecture

这一版把 Agent Memory Kernel 的定位从“桌面管理器”进一步收束为“Agent Skills 的本地内核”。后端是可被 Claude Code / Codex / CLI / 未来 MCP 调用的强类型治理接口；前端是高级个人开发者的驾驶舱，既能手动管理，也能把部分权限交给 AI。

#### 三层驱动模型

```text
Observation Store
  ↓
Rule Engine（确定性规则）
  ↓
AI Engine（语义精炼）
  ↓
Governance Layer（人/AI 决策）
  ↓
Memory Card Kernel Store
  ↓
Compiler / AI Interface / Human UI
```

底层规则驱动：

- 噪声过滤：过滤“继续优化”“帮我修一下”“再来一版”这类一次性对话。
- tag 分类：为 Draft / Memory Card 自动建议 `ui-design`、`testing`、`workflow`、`agent-handoff` 等标签。
- scope 推断：判断内容更适合 `global`、`project`、`directory` 还是 `agent-specific`。
- risk 分级：区分只读、生成草稿、修改内核存储、编译写文件等风险。
- 结构校验：检查 Memory Card schema、目标 Agent、token 预算和编译产物漂移。

中层 AI 驱动：

- 生成用户主要语言的 `brief`。
- 精炼 `body`，把对话式表达转成可编译指令。
- 语义融合多个 Memory Card，保留来源和冲突说明。
- 识别高价值 Prompt、项目改善记录、复用工作流。
- 给项目推荐应启用的 Memory Cards / Skills / 包。

上层治理驱动：

- `Manual`：只生成草稿，所有写入都由用户确认。
- `Assisted`：AI 给建议，用户审阅、编辑、批准。
- `Guarded Auto`：低风险自动执行，高风险进入 Draft Inbox。
- `Agent Managed`：AI 可以管理内核，但所有命令经过规则评估、审计记录和可回滚写入。

#### Kernel API

未来所有入口都应调用统一 Kernel API，而不是直接改 `.agent-kernel/*.yml`：

- `kernel.observe(...)`
- `kernel.plan_command(...)`
- `kernel.update_draft(...)`
- `kernel.approve_draft(...)`
- `kernel.merge_memory_cards(...)`
- `kernel.assign_memory_card(...)`
- `kernel.compile_project(...)`
- `kernel.explain_decision(...)`

Tauri UI、Bun CLI、未来 MCP Server 和 Claude Code/Codex 调用入口都应该复用这层 API。这样可以给 AI 完整能力，同时让权限、风险、审计和回滚保持一致。

#### 第一阶段实现

- 新增 `src/kernel/` 模块。
- 提供 `KernelPolicy`、`AutomationMode`、`KernelCommand`、`KernelDecision`。
- 提供确定性 `RuleAssessment`，用于 command 风险分级和文本分类。
- Tauri 暴露 `plan_kernel_command`，给 UI 或未来 AI 工具查看“这个动作是否可自动执行、为什么需要审阅”。
- 暂不替换所有旧命令，先作为兼容门面存在，后续再逐步让 Draft 编辑器、Memory Card 融合和 MCP 工具迁移到内核层。

### 13.5 v0.63：多角色自治迭代协议

从这一版开始，项目进入多角色协作模式。Leader 负责架构、整合和最终验证；PM、Rust Kernel Engineer、UI Engineer、QA/User Advocate 作为专门角色参与迭代。完整协作协议保存在 `docs/multi-agent-collaboration.md`。

#### 角色分工

- Leader / Kernel Architect：维护 Local-first、Rust core、Tauri UI、Bun tooling、Kernel API、编译产物模型和用户信任边界。
- PM / Developer Experience Strategist：定义高级个人开发者的体验、PRD、验收标准和隐私/信任要求。
- Rust Kernel Engineer：实现 Draft、Memory Card、Kernel Policy、Rule Engine、Compiler、Provider、Tauri command 等核心能力。
- UI / Desktop Experience Engineer：实现 Tauri + React 桌面体验；涉及视觉 polish 时优先调用 Claude Code，Codex 负责审核、集成和测试。
- QA / User Advocate：审查数据丢失、静默覆盖、重复处理、噪声草稿、UI 卡顿、跨平台路径和 Rule CI 风险。

#### 协作规则

- PM 先说明 What / Why / Acceptance Criteria。
- Leader 将需求转为数据结构、KernelCommand、Tauri IPC 和任务边界。
- Rust core 先实现强类型能力，UI 只通过 IPC 消费。
- QA 先列阻断风险，再给回归测试建议。
- 所有 AI 自动化必须经过 Kernel Policy，不允许绕过 `.agent-kernel` 源事实直接改生成产物。

#### 当前迭代目标

- Draft / Memory Card 可视化编辑器：允许编辑 `brief / body / tags / targets`，并支持清空 targets 作为休眠。
- Memory Card 语义融合：由 Claude Code / Codex 做精炼，但结果进入 Draft Inbox，不直接覆盖原 Memory Card。
- UI 接入 Kernel Policy：在执行高风险动作前展示决策原因和审阅要求。

### 13.6 v0.64：Kernel Policy 强制门禁与写入审计

这一版把 Kernel Policy 从“前端提示”升级为后端强制边界。Tauri UI、未来 CLI/MCP/AI 入口都不能绕过 Rust 内核直接写项目记忆。

已落地的核心约束：

- Tauri 写命令需要携带 confirmed policy；未确认时默认按 Manual policy 拦截。
- `approve_draft`、`reject_draft`、`update_draft`、`update_memory_card`、`set_memory_card_targets`、`merge/fuse`、`promote/install`、`evolve_project`、`import_artifact_drifts`、`sync_project` 等项目级 mutation 都先构造 `KernelCommand` 并经过 `KernelPolicy`。
- React UI 在用户触发写操作时传递 confirmed policy；编辑器仍保留“审查变更 -> 确认保存”的流程。
- 新增 `.agent-kernel/audit-log.jsonl`，记录 Tauri 项目 mutation 的 authorized / blocked 决策、风险等级、policy mode、原因和时间。
- `sync_project` 写 `AGENTS.md` / `CLAUDE.md` / rules 前会检查 `project.lock.yml`。如果生成产物被手动修改，会阻止覆盖并要求先走 artifact import / reverse parse。

下一步：

- confirmed policy 已升级为 command payload hash / decision token：plan 阶段签发 token，执行阶段校验 project + command + payload，并一次性消费，避免前端或 AI 入口复用“已确认”。
- Draft 批准已增加同 ID Memory Card 冲突保护：不会再直接覆盖已有 Memory Card，而是要求先审查或合并。
- Draft / Memory Card 编辑已增加 schema 防线：空标题/空正文、未知 kind/scope、未知 Agent target 会被后端拒绝。
- 在 UI 中展示 Audit Log，并基于 audit entry 提供回滚/checkpoint。
- 将启动静默整理的写入也接入 audit，保持“静默增量”和“可追踪”同时成立。

### 13.7 v0.65：非阻塞任务层与页面级 Read Models

这一版继续修复“功能能做但体验不够像真实产品”的根因：UI 不能被扫描、整理、同步这类长任务拖住，项目切换也不能每次都依赖完整 `ProjectSnapshot`。

已落地的架构推进：

- `DesktopTaskStore` 升级为 Job Manager v1：任务有 `job_id`、`stage`、`lifecycle`、进度、日志、开始/结束时间和结果摘要。
- Tauri 暴露 `get_job_history` 与 `cancel_job`，前端新增“任务中心”抽屉，展示后台任务历史、日志和取消请求。
- `scan_projects` 不再在 IPC 调用中同步递归扫描，而是立即返回 job ticket，扫描在线程中完成；完成后 UI 自动刷新项目列表。
- `sync_project` 不再阻塞按钮点击，而是先通过 Kernel Policy，再启动后台编译任务；完成后 UI 自动刷新当前项目快照。
- `evolve_project` 保持后台整理，并在任务完成后自动刷新 Dashboard / Snapshot，让新 Draft 出现在审阅区。
- Job History 写入 `~/.agent-kernel/jobs/history.jsonl`，重启后仍可恢复最近任务记录。
- Application Service 新增页面级读模型：`ProjectReviewInbox`、`ProjectMemory CardLibrary`、`ProjectAssignmentView`、`ProjectQualityView`。后续前端可以按页面加载，而不是切换项目时拉取全量 snapshot。

这一版确认的产品原则：

- 扫描、整理、同步、融合、推荐都应是后台 Job，前端只显示 job ticket 和进度。
- `ProjectSnapshot` 保留为兼容调试接口，不再作为长期 UI 主路径。
- Job History 采用本地 JSONL 持久化，符合 file-native 原则；后续可增加按项目过滤和日志清理策略。
- `cancel_job` 是安全检查点式取消，不承诺强杀外部 Agent。后续 Claude Code / Codex 深度精炼需要把子进程句柄纳入 Job Manager。

下一步大版本建议：

- 把 Draft、Memory Card、Assignment、Quality 页面切到对应 read model，进一步降低切项目和页面切换成本。
- 将 Memory Card 融合、项目推荐和 Claude Code / Codex 深度整理全部接入 Job Manager。
- 在 UI 中增加按项目/状态筛选 Job History，并支持清理过期日志。
- 增加“重试”能力：失败任务可以用同一 payload 重新排队，但仍要经过 Kernel Policy。

### 13.8 v0.66：页面级数据接入与可恢复任务操作

这一版把 v0.65 的后端能力真正接入到 Tauri React 前端，让 UI 从“完整项目快照驱动”过渡到“页面级读模型驱动”。

已落地：

- Draft Inbox 页面优先读取 `get_project_review_inbox`，只加载待审草稿。
- Memory Card Library / Catalog 页面优先读取 `get_project_memory_card_library`，共享项目 Memory Cards、全局 Memory Cards 和包状态。
- Assignment 页面优先读取 `get_project_assignment_view`，只加载目标 Agent 和 Memory Card target matrix。
- 右侧质量状态优先读取 `get_project_quality_view`，避免为了显示 Rule CI / warnings 拉取整个项目。
- 完整 `ProjectSnapshot` 仍作为兼容 fallback 和调试路径存在；后续可以逐页移除对它的主路径依赖。
- 任务中心增加失败/取消任务的“重试”入口，先支持扫描、整理历史和同步三类主任务。

这带来的体验变化：

- 切换项目后先加载 Dashboard，再按当前页面加载最小数据，完整 snapshot 退到后台。
- 用户进入草稿、技能库、分配矩阵时，不再必须等待 build preview、Rule CI、observations 等全部数据。
- 任务失败或被取消后，用户可以在任务中心继续操作，不需要回到工作台重新找按钮。

下一步建议：

- 给每个页面 read model 增加 loading / stale 标记，让 UI 明确显示“正在刷新本页数据”。
- 把 `ProjectSnapshot` 从页面 props 中继续下沉，只在 Settings / debug / export preview 中使用。
- Retry 需要从“按任务 key 重新执行”升级为“保存原始 command payload 后精确重放”，并仍通过 Kernel Policy。
- Job Center 增加按项目、状态和时间过滤，避免历史任务积累后难以阅读。

### 13.9 v0.67-v0.68：精确重试与融合任务后台化

这一轮把 Job Center 从“能看任务”推进到“能可靠恢复任务”。之前重试是根据中文任务名猜测入口，容易丢失项目路径、目标 Agent、整理引擎和 policy；这会让失败任务看似能重试，实际却可能重跑到错误上下文。

已落地：

- `DesktopTaskStatus` 增加 `replay` 字段，保存原始 Tauri command 和 args。
- Job history JSONL 持久化 replay payload，重启后仍能精确重试。
- `scan_projects`、`evolve_project`、`sync_project` 创建任务时写入 replay payload。
- 前端 Job Center 重试改为直接调用 `job.replay.command` 和 `job.replay.args`，不再根据 `job.key` 猜测。
- `fuse_memory_cards_to_draft` 改为后台 Job，立即返回 job ticket；任务中心展示读取源 Memory Card、写入融合草稿和完成/失败结果。
- 融合任务也保存 replay payload，并在完成后刷新 Dashboard、页面读模型和兼容 snapshot。
- Job Center 增加状态筛选、类型筛选和“可重试”标记，让任务历史积累后仍然能快速定位失败任务或确认哪些任务支持精确重放。

下一步：

- Job History 增加按项目、状态、任务类型过滤和清理。
- 所有外部 AI 精炼、推荐、批量融合都进入 Job Manager，并记录 engine、输入摘要、输出草稿 ID 和失败原因。
- Replay payload 后续应附带 project hash / command version，避免版本升级后盲目重放旧参数。

## 14. 参考资料

- Claude Code memory: https://code.claude.com/docs/en/memory
- Claude Code skills: https://code.claude.com/docs/en/skills
- OpenAI Codex AGENTS.md: https://developers.openai.com/codex/guides/agents-md
- OpenAI Codex Skills: https://developers.openai.com/codex/skills
- Agent Skills standard: https://agentskills.io/
- OpenAI Skills catalog: https://github.com/openai/skills
- Memento-Skills: https://arxiv.org/abs/2603.18743
- Dive into Claude Code: https://arxiv.org/abs/2604.14228
- Model Context Protocol: https://modelcontextprotocol.io/docs/getting-started/intro
- Cursor Rules: https://docs.cursor.com/context/rules
- Cline Memory Bank: https://docs.cline.bot/customization/memory-bank
- Mem0 docs: https://docs.mem0.ai/platform/overview
- Letta Code docs: https://docs.letta.com/letta-code
- LangGraph memory docs: https://docs.langchain.com/oss/javascript/langgraph/memory
- Graphiti docs: https://help.getzep.com/graphiti/getting-started/overview
- Aider RepoMap: https://aider.chat/docs/repomap.html
