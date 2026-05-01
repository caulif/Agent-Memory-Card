# Agent-Kernel / Memory-GC 构思文档

> 日期：2026-04-30  
> 目标：把一个由 Rust 构建、通过 `bunx` 分发的引擎，设计成面向 Claude Code / Codex 的本地 Skilllet 进化、可视化治理和规则编译系统。

## 0. 本轮增强后的关键结论

这轮讨论后，Agent-Kernel 的定位应该再往前推一步：它不是“修补现有规则文件”的小工具，而是一个面向 Claude Code 和 Codex 的“本地 Skilllet 进化引擎”。

关键决策：

- Rust 内核：核心解析、分类、diff、build、Rule CI、文件操作都用 Rust，保证速度、单二进制分发和工程可靠性；`bunx agent-kernel` 是优先的跨平台安装与启动入口。
- 可视化优先：UI 不是后期锦上添花，而是建立信任的核心产品面。用户需要在图形界面里审查冲突、确认压缩、拖拽分发 skilllets。
- UI 双入口：第一屏采用以 Project 为中心的白板/Canvas，展示当前项目、多个 Agent、Skill、Skilllet 的关系和拖拽连接；同时提供 App Store/包管理器界面，用于浏览、安装、启用和更新 Skills/Skilllets。
- 编译产物思维：`CLAUDE.md`、`AGENTS.md` 默认视为 build artifacts，由 `~/.agent-kernel/skilllets` 和项目 `.agent-kernel/project.yml` 全量编译生成；Cursor 等其他 Agent 保留 adapter 扩展接口，MVP 不进入默认目标。
- 首个入口：先从用户现有规则文件和 Skills 导入，后续再通过 MCP/session log 自动提炼对话中的 Draft Skilllets。
- Skill 管理默认引用模式：第三方或已有 Skill 保持原位置，Agent-Kernel 建立索引、启用关系、导出关系和 overlay，不直接改源文件。
- Skill 分发默认 Mirror 模式：拖拽现有 Skill 到某个 Agent/项目时，不改源文件，而是复制/同步一份到目标 Agent 的 skills 目录，并记录 source path、hash 和同步状态。
- 项目配置采用声明式：`.agent-kernel/project.yml` 只描述“当前项目希望启用哪些规则、Skilllets、Skills，以及它们分配给哪些 Agent”，build 负责生成具体产物。
- 对话提炼采用半自动 Draft Inbox：MCP/CLI/session log 主动发现值得沉淀的信息，但只生成草稿和推荐作用域，不自动启用。
- 隐私策略采用 Hybrid：扫描、索引、Mirror、build、Rule CI 编排默认本地执行；LLM 提炼和冲突判断通过可选 provider 完成，用户可选择云端模型或本地模型。
- v0.1 范围收紧：先不做半自动对话提炼，第一版只完成“扫描现有规则和 Skills -> Canvas 分配 -> Mirror/build preview”的闭环。
- v0.2 范围：完善 Mirror 信任闭环，区分 source updated / target drifted，提供 CLI/UI sync。
- v0.3 范围：落地 Owned Skilllet 存储、CLI add/list、声明式 project.yml include、按 Agent target 编译到指令文件。
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
- v0.17 范围：实现本地 Skilllet Catalog / App Store 地基，支持内置 packages、`.agent-kernel/catalog.yml` 覆盖、CLI install，以及 Canvas App Store 安装入口。
- v0.18 范围：增强 Catalog 安装状态反馈，CLI 和 Canvas 均显示 available/installed，避免 App Store 重复安装缺少信任提示。
- v0.19 范围：为 Catalog package 增加 provenance 元数据（version/source_url/tags），让未来 Registry 和安全审查能基于来源、版本和类别做信任判断。
- v0.20 范围：加入 Catalog 本地 trust gate，CLI/UI 均可验证 duplicate id、missing provenance、empty body、missing tags，安装前先建立信任反馈。
- v0.21 范围：加入 instruction artifact 预算警告，默认 32 KiB，借鉴 Codex `project_doc_max_bytes` 约束，提前发现 prompt bloat。
- v0.22 范围：探索通用 rules exporter，为未来 Cursor adapter 打接口地基；Cursor 不进入当前默认目标。
- v0.23 范围：加入 Agent target 启停控制，CLI 与 Canvas 都能切换 Agent enabled 状态，降低手改声明式 YAML 的门槛。
- v0.24 范围：加入 Skilllet target assignment，CLI/UI 都能把同一 Skilllet 分配给不同 Agent，强化 Project 层 multi-agent 配置体验。
- v0.25 范围：加入 Skilllet target matrix，CLI/UI 都能总览 Skilllet × Agent 分配关系，为后续拖拽连线和批量操作打底。
- v0.26 范围：探索 Cline-style 规则目录 exporter 与旧配置迁移；v0.30 后 Cline 从核心主线移除，作为后续插件式 adapter 备选。
- v0.27 范围：将 Canvas Inspector 中的 Skilllet target matrix 从文字摘要升级为可点击矩阵表，让用户能直接按 Skilllet × Agent 维度分配能力。
- v0.28 范围：加入 generated artifact drift 检测，基于 `project.lock.yml` 比对 `AGENTS.md`、`CLAUDE.md` 等编译产物是否被手改，为后续 Reverse Parse 生成 Draft 打基础。
- v0.29 范围：实现 Reverse Parse 的本地第一版，`import --artifacts` 通过重新渲染期望产物并提取用户新增行，把手改的 build artifact 转成 Draft Inbox 候选。
- v0.30 范围：重置 MVP 范围，默认只支持 Claude Code / Codex；Cursor 和 Cline 从默认配置与 Canvas 目标中移除，但保留通用 exporter/adapter 接口。
- v0.31 范围：增强 Skilllet 操作能力，支持把一个或多个 Skilllet 分配到某个项目或 Agent，合并多个 Skilllet，并把 Skilllet 作为生成补充追加进已有 mirrored Skill。
- v0.32 范围：启动 Observation Layer，支持把本地对话文件和 Claude Code / Codex 常见 JSONL session 目录导入 `.agent-kernel/observations`，先保存原始观察记录，后续再进行 Skilllet Synthesis。
- v0.33 范围：打通本地进化闭环第一版，`observe synthesize` 将 Observation 转成 Draft Inbox 候选，Canvas 也可以从 Observations 一键生成待审阅 Draft，但不会自动启用 Skilllet。
- v0.34 范围：增加 `observe evolve`，一条命令完成本地 Claude Code / Codex 会话导入与 Draft 合成，仍保持“只进 Draft Inbox，不自动启用”的信任边界。
- v0.35 范围：Observation 导入增加 ID 去重，同一会话重复导入会计入 skipped，避免每天重复 evolve 时制造虚假的新增数量。
- v0.36 范围：将 JavaScript 包装层从 Node/npm 叙事切到 Bun，目录改为 `bun/`，本地脚本使用 `bun` / `bun test`，发布打包使用 `bun pm pack`。
- v0.37 范围：本地提取器识别高置信 Bun 包管理偏好，把“从 npm/pnpm/yarn 改为 Bun”这类对话归一化为稳定 `project:prefer-bun` Draft，而不是生成口语化长标题。
- v0.38 范围：本地提取器识别 Fetch -> Axios 这类高频 HTTP 客户端偏好纠正，归一化为稳定 `project:use-axios` Draft。
- v0.39 范围：引入 Known Preference Registry，把 Bun、Axios、Vitest 等高置信偏好归一化改为表驱动，后续扩展更多 Skilllet 模板时只需增加条目。
- v0.40 范围：Observation Synthesis / Evolve 报告返回具体 Draft ID 列表，让本地进化过程能解释“生成了哪些候选”，而不是只显示数量。
- 交互式 CLI：CLI 需要像 `git add -p` 一样逐块确认，而不是只给用户一份冷冰冰的 patch。
- Skilllet Registry：长期看，skilllet 可以像 JavaScript registry 包一样安装、版本化和组合，形成社区规则生态。
- Rule CI：规则压缩和合并后要能跑测试，验证“使用压缩后规则的 Agent 是否仍会做出期望行为”。

### 0.1 当前 MVP 权威决策

本阶段产品主线收束为：

> Agent-Kernel 先只服务 Claude Code / Codex，但 Observation Layer 要能读取本地规则、生成产物、手动修改，以及所有可发现的本地对话记录，从中持续提炼 Skilllets，让个人开发环境越用越贴合自己。

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
   - Observation 不直接变成 Skilllet。
   - 先保存为 observation record，字段包括 source、timestamp、agent、project、text span、evidence、privacy/redaction status、confidence。
   - Observation 是证据层，Draft Skilllet 是建议层，Active Skilllet 是用户批准后的源代码。

3. Skilllet Synthesis Layer
   - 从 observations 中提炼候选 Skilllet。
   - 识别类型：`preference`、`constraint`、`procedure`、`convention`、`correction`、`anti-pattern`。
   - 自动判断 scope：`global`、`project`、`directory`、`agent-specific`。
   - 输出必须包含 confidence、reason、evidence、suggested targets。

4. Evolution Layer
   - 负责去重、合并、冲突检测和版本演化。
   - Skilllet 需要 lineage：来自哪几次 observation、被修改过几次、是否通过 Rule CI、被哪些 Agent 使用、最近是否仍然有效。
   - Project Skilllet 如果反复出现，可建议 promotion 到 global preference。
   - 长期未触发的 Skilllet 可进入 dormant candidate，但不能自动删除。

5. Review Layer
   - 所有自动生成内容先进 Draft Inbox。
   - 用户 approve / reject / edit。
   - 这是信任边界，不能跳过。

6. Compiler Layer
   - 编译到 Claude Code / Codex 的原生文件和 skills 目录。
   - 生成 artifacts。
   - 检测 drift。
   - 支持 Reverse Parse 回流。

### 0.3 Skilllet 操作模型

用户必须能对 Skilllet 做这些操作：

- 将一个或多个 Skilllet 赋予给某个 Project。
- 将一个或多个 Skilllet 赋予给某个 Agent，例如只给 Codex 或只给 Claude Code。
- 合并多个 Skilllet，生成新的复合 Skilllet。
- 把 Skilllet 加入现有 mirrored Skill，作为生成补充，不直接修改第三方 Skill 源文件。
- 后续在 UI 中用拖拽和矩阵完成这些操作，同时保留 CLI 等价命令。

这些能力是 Project 层 multi-agent 配置的基础。即使当前 MVP 只支持 Claude Code / Codex，也要保持“同一项目中不同 Agent 获得不同能力”的模型。

### 0.4 进化体验设计

Skilllet 进化可以轻微参考游戏里的“技能树”隐喻，但命名和交互要工程化，不能喧宾夺主。

建议 UI 元素：

- Skilllet Tree：展示某条规则如何从多次 Observation 进化而来。
- Confidence / Stability：可信度和稳定度，不叫等级。
- Evolution Timeline：来自哪次对话，何时批准，何时合并，何时编译给哪个 Agent。
- Conflict Warning：冲突 Skilllet 用红色边连接。
- Dormant / Active：长期没触发的 Skilllet 进入休眠候选。
- Promotion Candidate：反复出现的 project Skilllet 可以建议提升为 global preference。
- Review Task：需要用户处理的候选、冲突或合并建议。

命名原则：

- 不叫 XP，叫 confidence。
- 不叫 level up，叫 promotion candidate。
- 不叫 rarity，叫 stability。
- 不叫 quest，叫 review task。

## 1. 一句话定位

Agent-Kernel 是一个面向 Claude Code 和 Codex 的本地 Skilllet 进化引擎：它不试图替代 Agent runtime，而是专门治理它们依赖的文件化上下文，把散落在本地对话、规则文件、Skill 文件和项目文档里的有效信息抽取、去重、合并、冲突仲裁，并编译回对应 Agent 能直接消费的格式。

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

Agent-Kernel 的价值就是把“文件即记忆”从手工维护升级成半自动编译。

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

这直接支持你的想法：Agent-Kernel 应该帮助用户把“长期常驻规则”“按需 Skill”“自动记忆条目”拆清楚，而不是一股脑写进一个大文件。

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
- symlink 支持很适合作为 Agent-Kernel 的跨项目注入机制。

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
- Agent-Kernel 可以把 Cursor Memories / Rules 纳入统一知识库，再导出成 Claude Skill、Codex Skill 或 AGENTS.md。
- 但 Cursor 不进入当前 MVP 主路径。现阶段只保留 exporter/adapter 接口，等 Claude Code / Codex 的进化闭环足够稳定后再补 UI 与原生 `.cursor/rules` 导出。

### 3.4 Cline（Out Of Core）

Cline 的 Memory Bank 是一个结构化 Markdown 文档体系：

- 用 `projectbrief.md`、`activeContext.md`、`progress.md` 等文件保存项目状态。
- 通过 “initialize memory bank”“update memory bank”“follow your custom instructions” 等命令维护。
- `/smol` 和 `/newtask` 用于压缩上下文或新任务交接。
- 文档强调 Memory Bank 文件要短，详细信息应拆成按需读取的文档。
- `.clinerules/` 目录适合作为项目级规则面，Agent-Kernel 应把自己的规则写成其中一个可审查的 build artifact，而不是独占整个 Cline 规则空间。

关键启发：

- Cline 证明“文档化记忆”对开发流程很有效。
- 但它更像项目持续文档，不是规则冲突解决器。
- Agent-Kernel 可以借鉴其文件结构，但 Cline 不进入当前产品主线。后续如果支持，应作为插件式 adapter，而不是默认 Agent、默认 Canvas 节点或核心叙事。

### 3.5 Mem0 / Letta / LangGraph / Graphiti

这些项目代表重型或中型记忆层：

- Mem0：面向 AI agents 的 managed memory layer，强调跨用户和跨 Agent 的连续性，平台侧提供 vector store、graph services、rerankers 等基础设施。
- Letta：memory-first/stateful agent，强调 Agent 能随使用学习用户偏好、代码规范和项目模式。
- LangGraph：把记忆分成 short-term/thread-scoped 和 long-term/cross-session，并讨论 semantic、episodic、procedural memory。
- Graphiti/Zep：用 temporal knowledge graph 处理动态事实、关系变化和历史上下文。

关键启发：

- 这些方案解决的是“Agent 应用如何持久化和检索大量动态记忆”。
- 你的项目解决的是“开发者已有规则文件和 Skill 文件如何保持干净、短小、一致、可迁移”。
- 这不是替代关系，而是互补关系。Agent-Kernel 可以在 MVP 阶段保持零数据库，后续再把 Mem0/Graphiti 接成可选后端。

### 3.6 Aider

Aider 的 RepoMap 很值得借鉴：

- 它会生成代码库的精简地图，只保留关键类、函数、签名和关系。
- 大代码库时，它会在 token budget 内选择最相关部分。

关键启发：

- Agent-Kernel 可以做 “MemoryMap”：不是把整份规则文件丢给 LLM，而是先构建 Markdown AST 和语义索引，让模型只处理冲突候选、冗余候选和需要合并的节点。

## 4. 产品边界

Agent-Kernel 不应该做：

- 不做完整 Agent 框架。
- 不强制用户接入数据库。
- 不替代 Claude/Codex 的原生机制。
- 不在 MVP 中追求 Cursor/Cline 的默认导出和 UI 主路径。
- 不把所有对话自动永久保存。
- 不让 LLM 直接重写整份规则文件。

Agent-Kernel 应该做：

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

### 5.3 Skilllet：比 Skill 更轻的中间单元

你提到“类似 skill，但更轻量化”。可以命名为 `skilllet`：

- `skilllet` 是 Agent-Kernel 内部最小知识单元。
- 多个 skilllet 可以编译成一个 Agent Skill。
- 单个 skilllet 也可以导出为一条 Cursor Rule 或一段 AGENTS.md。
- skilllet 有状态：draft、active、deprecated、archived、conflicted。

这样你不会被现有 `SKILL.md` 目录格式绑死，同时可以输出到各种平台。

### 5.4 Skill Library：现有 Skill 的可视化管理

Agent-Kernel 不只管理自己生成的 skilllets，也要管理用户已经拥有的 Claude Code / Codex / Superpowers / 社区 Skills。

默认采用引用模式：

- 第三方 Skill 保持在原路径，不复制、不重写。
- Agent-Kernel 建立索引：名称、描述、路径、来源、版本、适配 Agent、依赖脚本、references、assets。
- UI 中可以启用、禁用、分配到项目、分配到 Agent、查看依赖和预览导出。
- 如果用户想修改第三方 Skill，系统应提示 fork 成用户自有 Skill，再由 Kernel 托管修改后的副本。

Skill Library 中的资产分两类：

| 类型 | 来源 | 管理方式 |
| --- | --- | --- |
| Referenced Skill | `.claude/skills`、`.agents/skills`、Superpowers、社区目录 | 索引和引用，不原地修改 |
| Owned Skill | 用户在 Agent-Kernel 内创建或 fork 的 Skill | 由 Kernel 托管，可编辑、组合、编译 |

这能避免一个很危险的问题：规则治理工具不应该把用户安装的第三方 Skill 悄悄改坏。引用模式让第一版更可信，也更符合个人高级开发者的实际使用习惯。

### 5.5 Mirror：引用模式下的默认分发方式

当用户在 UI 中把一个已有 Skill 拖到某个 Agent 或项目时，默认使用 Mirror 模式。

Mirror 的含义：

- Source Skill 保持原位置，不修改。
- Agent-Kernel 把 Source Skill 复制到目标 Agent 的 skills 目录。
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
    agent-kernel-core/      # IR、skilllet、分类、冲突、预算
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

核心原则：`~/.agent-kernel/skilllets` 和项目 `.agent-kernel/project.yml` 才是 Single Source of Truth。当前 MVP 中 Claude Code / Codex 读取的 `CLAUDE.md`、`AGENTS.md` 是编译产物；Cursor/Cline 类文件只作为后续 adapter 产物。

MVP 建议零数据库：

```text
~/.agent-kernel/
  kernel.yml
  skilllets/
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

推荐 skilllet 文件结构：

```text
~/.agent-kernel/skilllets/global/frontend/http-client.skilllet.yml
~/.agent-kernel/skilllets/global/frontend/http-client.md
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

这个 UI 应该提前到 MVP，而不是排到最后。因为 Agent-Kernel 做的是“长期知识治理”，用户最担心的是误删、误合并和误固化。可视化面板是信任系统的一部分，不是装饰层。

UI 应采用双入口：

1. Project-centered Canvas 第一屏：用于理解和操作“当前项目启用了哪些 Skills/Skilllets，并分别分发给哪些 Agent”。
2. App Store / Package Manager：用于浏览、搜索、安装、更新和启用现有 Skills/Skilllets。

核心页面：

- Canvas Workspace：第一屏，以当前 Project 为中心，用白板节点和连线展示目标 Agent、Referenced Skills、Owned Skills、Skilllets、Rules、Exports。
- Inbox：从对话和文件提取出的候选 skilllets。
- Conflict Center：冲突规则对比，显示证据、时间线、推荐决策。
- Knowledge Map：按项目、技术栈、目录、Agent、知识类型浏览。
- Skill Library：现有 Skills 与 Owned Skills 的索引、搜索、预览、拖拽分发和 Mirror 同步状态。
- App Store：卡片式 Skill/Skilllet 浏览、安装、升级、评分、来源和兼容性展示。
- Export Preview：选择目标 Agent，预览将写入哪些文件和 diff。
- Budget View：显示常驻规则 token/byte 占用，提示哪些应该迁移成 Skill。
- Audit Log：记录每次合并、删除、迁移和导出。
- Registry：搜索、安装、升级和禁用社区 skilllet 包。
- Rule CI：展示规则测试用例、最近运行结果和失败原因。

很关键的一点：UI 不是花哨面板，而是“信任建立器”。用户必须能看到为什么引擎认为两条规则冲突、为什么要删除旧规则、会改哪些文件。

交互形态建议：

- Canvas 节点：Current Project、Claude Code、Codex、Skill、Skilllet、Rule Set、Export Artifact。Cursor 等后续目标只通过 adapter 插件加入。
- Canvas 连线：启用、Mirror、Compile、Depends on、Conflicts with、Supersedes。
- 拖动连接：从 Skill 节点拖线到 Agent 节点，创建该项目下的 Agent-specific Mirror；从 Skilllet 拖到 Project，加入项目规则；从 Skilllet 拖到某个 Agent，只给该 Agent 编译；从多个 Skilllets 拖到新 Skill，组合成 Owned Skill。
- 连线状态：synced、source updated、target drifted、conflict、test failed。
- 冲突对比：左侧旧规则，右侧新规则，红色表示将废弃，绿色表示将保留。
- 知识拖拽：把 skilllet 从 Global 拖到 Project，或从 Always Prompt 拖到 Skill。
- Skill 拖拽：把 Referenced Skill 拖到 Claude Code / Codex，系统默认创建 Mirror 副本。后续 adapter 可加入新的 Agent 节点。
- 预算条：像 bundle analyzer 一样展示每个 Agent 的上下文占用。
- 反向解析提示：如果用户手动改了 `CLAUDE.md`，UI 显示“检测到编译产物被手动修改，是否导入为 Draft skilllet？”
- Mirror 状态：显示 synced / source updated / target drifted / fork recommended。
- 一键回滚：从 audit log 选择某次 build，还原对应 skilllet 状态和导出结果。

Canvas 第一屏的信息布局建议：

```text
左侧 Palette:
  Rules / Skilllets / Skills / Packages / Drafts

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
- 卡片：名称、描述、来源、版本、包含多少 skilllets、是否有 Rule CI。
- 操作：Install、Mirror to Project、Preview Contents、Fork。

Project-centered Canvas 的关键规则：

- Project 是唯一中心节点，所有操作都回答“这个项目当前如何配置”。
- Agent 节点是 Project 的运行目标，可以同时存在多个。
- 同一个 Skill 可以 Mirror 给多个 Agent，也可以只给一个 Agent。
- 同一个 Skilllet 可以编译进项目级规则，也可以只编译进某个 Agent 的规则。
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

Skilllet "Use Axios for frontend requests"
  -> Project Rule, compiled to Codex + Claude Code

Skilllet "Use Claude subagents for refactors"
  -> Claude Code only
```

### 6.8 编译产物模型

`CLAUDE.md`、`AGENTS.md` 默认不再是手工维护的主文件，而是 dist。`.cursor/rules/*.mdc`、`.clinerules` 属于后续 adapter 产物，不进入当前 MVP 默认路径。

编译流程：

```text
skilllets + project.yml + exporter config
  -> plan
  -> Rule CI
  -> preview diff
  -> build artifacts
```

产物文件头部可以写明来源，但不要依赖 managed block 保护局部结构：

```md
<!-- Generated by Agent-Kernel. Do not edit directly. Run `bunx agent-kernel import` to ingest manual changes. -->
```

如果用户确实手动改了产物：

1. `agent-kernel scan` 检测文件 hash 与 lockfile 不一致。
2. `agent-kernel import CLAUDE.md` 做 reverse parse。
3. 新增内容进入 Draft Inbox。
4. 用户确认后写回 skilllets。
5. 再次 build 全量生成产物。

这个模式比 managed block 更稳，因为系统不会在半手写半生成的文件中做脆弱手术。managed block 可保留为 legacy/compat 模式，服务不愿全量托管规则文件的团队。

### 6.8.1 声明式项目配置

`.agent-kernel/project.yml` 应该是声明式配置，类似 Terraform、Vite config 或 Kubernetes manifest：它描述期望状态，不记录所有运行时细节。

原则：

- 写“我要启用什么”，不写“上一次具体生成了什么”。
- 写 Project -> Agent -> Skills/Skilllets 的分配关系。
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

skilllets:
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

### 6.9 Skilllet Registry

skilllet 天然适合社区化。长期可以做一个轻量 Registry，让用户安装经过整理的 AI 规则包。

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
skilllets/
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
- 团队可以发布内部私有 skilllet collection。
- 开源项目可以随仓库发布官方 Agent 使用规则。
- Agent-Kernel 可以像 lockfile 一样固定规则包版本，避免规则漂移。

### 6.10 Hybrid 隐私与模型 Provider

Agent-Kernel 的默认信任模型应该是 local-first，但产品能力允许用户接入云端或本地模型。因此第一版采用 Hybrid 策略。

本地默认执行：

- 扫描规则文件和 Skills。
- 建立 Skill Library 索引。
- Mirror 复制和同步。
- 编译 `CLAUDE.md`、`AGENTS.md` 等产物；其他 Agent 通过 future adapter 扩展。
- 计算 hash、diff、预算。
- 管理 Canvas、project.yml、lockfile。
- 运行非 LLM 断言类 Rule CI。

需要 provider 的能力：

- 从对话中提炼 Draft Skilllets。
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

- Imported Rules：可反向解析为 skilllets 的规则。
- Referenced Skills：已发现但保持原路径的 Skills。
- Draft Skilllets：可从规则中提炼出的候选。
- Conflicts：跨 Agent 规则中互相冲突的偏好。
- Export Targets：当前项目可以编译到哪些 Agent。

后续再从对话提炼：

```bash
cat session.md | bunx agent-kernel extract --project .
```

输出：

- 5 条候选 skilllets
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
- `e` 可以直接打开临时编辑器修改最终 skilllet 正文。
- `s` 可以跳过不确定项，保留到 UI Inbox。
- 每次决策都写入 audit log，便于回滚和训练后续推荐。

### 7.3 注入到任意项目

```bash
bunx agent-kernel attach --project . --profile frontend-saas --agents codex,claude
```

实现方式：

- 轻量方案：项目内生成 `.agent-kernel/project.yml`，记录启用哪些 skilllets。
- 编译方案：把目标 Agent 文件视为 build artifacts，全量生成。
- symlink 方案：把全局 skill 目录 symlink 到 `.agents/skills` 或 `.claude/skills`。

建议默认使用“Source of Truth + 全量编译 + 可审计 diff”。symlink 作为高级选项。原因是团队协作时，显式产物更容易 code review，完整重建也比局部补丁更稳定。

### 7.4 Agent 运行时调用

通过 MCP 提供工具：

- `agent_kernel.extract_from_text`
- `agent_kernel.propose_memory`
- `agent_kernel.gc_project_rules`
- `agent_kernel.search_skilllets`
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

Draft Skilllet 必须包含：

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
- 用户批准后才写入 `project.yml` 或 global skilllet store。

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

- skilllets 是源代码。
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
- 在 UI 中标出是哪个 skilllet 压缩后破坏了行为。
- 允许用户把失败用例一键保存为长期 Rule CI。

Rule CI 的真正价值不是“模型测试 100% 准确”，而是为高价值规则建立回归保护。它会让用户敢于让系统持续进化。

## 9. MVP 建议

### MVP 0 / v0.1：Import + Canvas + Mirror + Build Preview

目标：证明“扫描现有 Claude Code / Codex 规则和 Skills -> Project-centered Canvas 分配 -> Mirror Skills -> Claude/Codex 编译产物预览”可行。

v0.1 明确不做：

- 不做半自动对话提炼。
- 不做 LLM 语义合并。
- 不做 Skilllet Registry。
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
- 从 skilllets 全量编译到 Codex `AGENTS.md` 和 Claude `CLAUDE.md`
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
- 从文本中抽取候选 skilllets
- 结构化分类
- 发现 3 类冲突：工具偏好、语言/框架偏好、禁令
- 生成 Draft Inbox 项，不自动启用
- 附带证据、推荐作用域、推荐目标 Agent、冲突对象

### MVP 3：可视化控制台

功能：

- Canvas Workspace 作为第一屏
- Project-centered multi-agent 分配：同一项目下可给 Claude Code / Codex 分配不同 Skills/Skilllets；Cursor/Cline 仅作为未来 adapter 扩展。
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

### MVP 4：Skilllet Store + Exporters + Rule CI

功能：

- `~/.agent-kernel/skilllets`
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

### MVP 6：Skilllet Registry

功能：

- 安装社区 skilllet 包
- 支持私有 registry
- lockfile 固定版本
- registry package Rule CI

## 10. 竞争定位

| 项目 | 主要路线 | 与 Agent-Kernel 的关系 |
| --- | --- | --- |
| Mem0 | 托管记忆层，向量/图/重排 | 可作为后端，不是直接竞品 |
| Letta | Stateful memory-first agent | 偏完整 Agent，Agent-Kernel 偏文件治理 |
| LangGraph memory | Agent 应用状态和长期记忆框架 | 可借鉴记忆分类 |
| Graphiti/Zep | 动态时序知识图谱 | 可处理复杂事实变化，MVP 不需要 |
| Claude Code memory/skills | 原生文件记忆和 Skill | 目标输出平台 |
| Codex AGENTS/Skills | 原生项目指令和 Skill | 目标输出平台 |
| Cursor Rules | 项目规则和 Memories | 目标输出平台和输入来源 |
| Cline Memory Bank | 项目文档化记忆 | 输入来源和输出平台 |
| Aider RepoMap | 代码上下文压缩 | 算法启发 |

一句话差异：

> Mem0/Letta 帮 Agent 记住更多东西；Agent-Kernel 帮开发者决定哪些东西值得变成规则、应该放在哪里、如何保持不冲突。

## 11. 开源包形态

建议包名方向：

- `agent-kernel`
- `memory-gc`
- `skill-os`
- `agent-memory-gc`
- `context-compiler`
- `skilllet`

我更推荐：

- JavaScript registry 包：`agent-kernel`
- 核心概念：`skilllet`
- 核心实现：Rust binary
- 分发方式：Bun wrapper + GitHub Releases，后续支持 Homebrew / Cargo install
- 子命令：`gc`、`extract`、`review`、`export`、`attach`、`ui`

命令草案：

```bash
bunx agent-kernel init
bunx agent-kernel scan
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
bunx agent-kernel ui
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
- 压缩前后的 skilllet 保留版本，可一键回滚

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

- 产物头部明确写 Generated by Agent-Kernel。
- build 前检查 hash，发现手动修改则中断并提示 import。
- `agent-kernel import` 先反向解析为 draft，不直接覆盖 skilllets。
- UI 中展示“手动改动 -> draft skilllet -> build”的闭环。

## 13. 我建议的产品路线

最好的第一步不是做 MCP，也不是一开始接所有 Agent，而是做一个很锋利的 Rust CLI + 早期可视化预览：

> 输入一份混乱的 `CLAUDE.md` / `AGENTS.md`，反向解析成 draft skilllets，交互式确认后编译成干净的目标产物。

因为这个闭环最小，但价值最明显：

- 用户能立刻看到重复、冲突、过期规则。
- 能证明“反向解析 + skilllet 源码 + 全量编译”比 LLM 全文重写更可靠。
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

## 14. 参考资料

- Claude Code memory: https://code.claude.com/docs/en/memory
- Claude Code skills: https://code.claude.com/docs/en/skills
- OpenAI Codex AGENTS.md: https://developers.openai.com/codex/guides/agents-md
- OpenAI Codex Skills: https://developers.openai.com/codex/skills
- Agent Skills standard: https://agentskills.io/
- Model Context Protocol: https://modelcontextprotocol.io/docs/getting-started/intro
- Cursor Rules: https://docs.cursor.com/context/rules
- Cline Memory Bank: https://docs.cline.bot/customization/memory-bank
- Mem0 docs: https://docs.mem0.ai/platform/overview
- Letta Code docs: https://docs.letta.com/letta-code
- LangGraph memory docs: https://docs.langchain.com/oss/javascript/langgraph/memory
- Graphiti docs: https://help.getzep.com/graphiti/getting-started/overview
- Aider RepoMap: https://aider.chat/docs/repomap.html
