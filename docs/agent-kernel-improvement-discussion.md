# Agent Memory Kernel 改进交流稿

> 日期：2026-05-15  
> 目的：先把我对项目需求、设计错位和改进方向的判断写清楚，作为和你继续讨论的材料。  
> 结论先行：我建议先做一次“产品闭环重构”，再做算法继续加法。现在最核心的问题不是不够 agentic，而是用户从真实历史到放心写入 `AGENTS.md` / `CLAUDE.md` 的路径还不够短、不够可验证、不够让人放心。

## 1. 我理解的项目本质

这个项目不是普通的“AI 记忆库”，也不是聊天记录总结器。它更像一个本地优先的 Agent Memory 编译器：

```text
真实协作历史 / 项目规则 / Skills
  -> Observation
  -> Candidate
  -> Draft
  -> Memory Card
  -> Assignment
  -> AGENTS.md / CLAUDE.md / agent skills
```

它真正卖点不是“自动记住更多”，而是：

- 把散落在对话、规则文件、技能和纠错里的长期知识变成可审查资产。
- 让用户决定什么能进入长期记忆，而不是让模型偷偷写 memory。
- 把中立 Memory Card 编译到 Claude Code、Codex 等工具的原生 memory 表面。
- 保护本地隐私、文件可 diff、可回滚、可迁移。

这和你 Obsidian 里的工程笔记高度一致：工具设计要结构化、错误要可操作、写操作默认需要确认、Agent Evals 要看真实环境和 trace，不只看最终输出。

## 2. 现在效果不达预期的根因

我认为主要不是某一个页面或某一个 prompt 坏了，而是项目有三层错位。

### 2.1 产品闭环错位

文档里写的 v1 工作流很对：

1. 选择本地项目。
2. 看轻量 dashboard。
3. 运行 evolution job。
4. 审查少量高价值 Draft。
5. 编辑、合并、批准或拒绝。
6. 分配 Memory Cards。
7. 编译并验证 artifact。

但从代码和 UI 看，当前体验容易变成“有很多概念和按钮，但用户不知道哪一步证明它真的有效”。比如 Draft、Candidate、Memory Card、Loadout、Quality、Job、Catalog 都存在，但第一屏没有足够强地回答：

- 这次从真实历史里发现了什么？
- 每条候选为什么可信？
- 它会写到哪里？
- 写入前后有什么 diff？
- 如果错了怎么撤回？
- 这次提炼质量比上次好吗？

用户的主观感受会是：系统很复杂，但不能快速建立信任。

### 2.2 算法闭环错位

项目已经朝五层提炼流水线推进：

```text
STRIP -> TRUNCATE -> CLUSTER -> INDUCE -> CRYSTALLIZE
```

这个方向比早期“多级过滤 gate”合理，因为真实偏好往往分散在多轮对话里，需要先聚合证据再归纳。但现在还缺一个更硬的验收闭环：每次改 prompt、阈值、聚类或 crystallize 规则，应该能回答：

- 黄金集 positive recall 多少？
- negative precision 多少？
- 新增候选里有多少是一次性请求误收？
- 有多少 evidence quote 真的能回溯到 observation？
- 审查后用户批准率是多少？

没有这个闭环，算法越做越复杂，质量仍然靠感觉判断。

### 2.3 工程边界错位

项目原则写得很清楚：Rust core 是业务核心，Tauri UI 是控制面，所有 AI 自动化走 `KernelCommand` 和 `KernelPolicy`。但代码规模已经开始出现“概念正确、文件承载过重”的信号：

- `app/src/styles.css` 和 `app/src/styles/project-detail.css` 都超过 1200 行。
- `src/observation.rs`、`src/memory_card.rs`、`src/candidate.rs`、`src/provider.rs`、`src-tauri/src/app_service.rs` 都接近或超过 900 行。
- 提炼相关逻辑分布在 `extract.rs`、`extract/*`、`eval.rs`、prompt、测试和 archived docs 中，演进历史较重。

这不一定马上导致 bug，但会降低 Agent 协作效率：文件太大时，模型更容易漏上下文，也更难做小步验证。

## 3. 从你的 Obsidian 笔记抽出的设计原则

我建议把这些原则直接升级为项目改进的验收标准。

### 3.1 Workflow 优先，Agent 其次

你的 `Workflow vs Agent` 笔记里有一个关键判断：能用固定 workflow 稳定解决的问题，优先用 workflow；只有路径开放、需要探索和持续反馈时，才引入 Agent。

本项目的 v1 主链路其实应该是 workflow：

```text
导入 -> 去噪 -> 聚类 -> 归纳 -> 证据校验 -> 人工审查 -> 编译预览 -> 写入
```

Agent/LLM 只负责其中的“归纳”和“重写”局部，不应该让用户感觉整个产品是一个会自由行动的黑盒 Agent。

### 3.2 工具响应要服务下一步决策

你的 `工具响应设计` 笔记强调高信号、低噪声、可分页、可切换详细度、可操作错误。对应到本项目：

- Candidate 列表默认不应该只展示 title/body/confidence，还要展示“证据数量、来源类型、风险、推荐动作”。
- Job 结果不应该只说成功/失败，还要给“产生了几条候选、拒绝了几条、主要拒绝原因、下一步建议”。
- Drift 错误不应该只阻止 sync，还要直接提供“导入 drift 为 draft / 查看 diff / 放弃手改”的路径。

### 3.3 Agent Evals 必须连接真实环境

你的 `Agent Evals` 和 `Trace Grading` 笔记强调：结果、工具选择、参数、轨迹、成本、安全边界都要看。对应到本项目，提炼质量评估不能只看 LLM 输出漂不漂亮，而要看：

- 是否读到了关键 observation。
- evidence 是否可回溯。
- 是否把一次性请求误判为长期规则。
- 是否尊重人工确认边界。
- 是否生成了可用 artifact，且不会覆盖用户手改。

### 3.4 Golden Set 是持续优化发动机

`Golden Set` 笔记虽然还短，但方向很关键。这个项目必须有自己的黄金集，否则“质量提升”没有度量。黄金集应来自：

- 人工已批准的高质量卡。
- 被用户拒绝的坏候选。
- 历史失败案例。
- 高风险边界案例，例如发布、覆盖、自动合并、隐私路径。

## 4. 外部参考给出的校准

这些参考不应该被照搬，但能帮助校准方向。

- Anthropic 的 Building Effective Agents 强调 workflow 和 agent 的区别：固定路径用 workflow，开放问题才用 agent。这个项目的主体验应该是可信 workflow，而不是炫耀自主 Agent。
- OpenAI 的 Codex agent loop 强调上下文、工具、观察、验证、终止和交付。对应本项目，Memory Card 生成也需要可观察 trace 和完成判断。
- LangChain / LangGraph 的 long-term memory 把长期记忆作为跨会话 JSON 文档存储，并区分短期状态和长期存储。对应本项目，`.agent-kernel/` 做 source of truth 是对的。
- Mem0 把 memory operations 作为一等概念，至少有 add/search/list/update/delete 这类显式操作。对应本项目，不应该只有“生成卡片”，还要有明确的 memory 生命周期和治理动作。
- OpenAI Agents SDK guardrails 把护栏作为一等机制。对应本项目，`KernelPolicy` 是正确方向，但还要让 UI 直接可见。

参考链接：

- [Anthropic: Building Effective Agents](https://www.anthropic.com/index/building-effective-agents)
- [OpenAI: Unrolling the Codex Agent Loop](https://openai.com/index/unrolling-the-codex-agent-loop)
- [LangChain: Long-term Memory](https://docs.langchain.com/oss/python/langchain/long-term-memory)
- [Mem0: Memory Operations](https://docs.mem0.ai/core-concepts/memory-operations)
- [OpenAI Agents SDK Guardrails](https://openai.github.io/openai-agents-python/guardrails/)

## 5. 我建议的改进方向

### 方向 A：先把 v1 日常闭环做硬

这是我最推荐的主线。

目标不是增加新能力，而是让用户每天打开后能完成一个稳定闭环：

```text
选择项目
  -> 运行提炼
  -> 看到少量高价值候选
  -> 点开证据和原因
  -> 批准/拒绝/编辑
  -> 分配给 Agent
  -> 预览 diff
  -> 写入
  -> 得到验证报告
```

关键改动：

- 合并 Candidate 和 Draft 的心智模型。用户看到的就是“待审建议”，内部可以继续保留两层，但 UI 不要让用户承担概念复杂度。
- 每条候选必须有 Evidence Panel：source observation、quote、recurrence、origin、risk、为什么推荐。
- Build Preview 变成写入前必经步骤，展示 `AGENTS.md` / `CLAUDE.md` / skills 的 diff。
- Job Center 的结果要能回到对应候选、日志和失败原因。
- Dashboard 改成“下一步工作台”：当前最该做什么、为什么、点哪里。

验收标准：

- 用户从空项目到写入第一张 Memory Card，不需要理解内部术语。
- 每条被批准的卡都能看到至少一条真实 evidence。
- 任何写入前都能看到目标 artifact diff。
- sync 遇到 drift 时不会让用户卡死，有明确恢复路径。

### 方向 B：建立提炼质量评测系统

这是第二优先级，但它决定后续算法是否越做越好。

建议保留当前五层流水线方向，但把“评测”提升为一等入口：

```text
cargo run -- eval --project . --golden-set local
```

评测输出应该包括：

- positive recall
- negative precision
- evidence validity
- duplicate rate
- one-off false positive rate
- approval simulation 或真实批准率
- prompt / pipeline version

并在 UI 里至少展示最近一次评测摘要。

验收标准：

- 每次修改 prompt 或 pipeline 都能跑同一套黄金集。
- 评测结果写入 `docs/runs/eval-*` 或 `.agent-kernel/evals/`。
- 低质量候选不是凭感觉说“差”，而能归因到 evidence、durability、scope、format、risk 等维度。

### 方向 C：把 Memory Card 治理做成核心产品

现在项目名称里有 Memory Card，但 UI 和模型里还带着很多旧术语，比如 skilllet、技能片段、candidate、draft。建议产品语言收口：

- `Memory Card`：最终长期记忆单元。
- `Suggestion`：系统提出的待审建议。
- `Loadout`：某个项目给某个 Agent 装配了哪些卡。
- `Artifact`：编译生成给 Agent 的文件。
- `Pack`：可安装的卡片集合。

治理能力建议优先做：

- 合并相似卡。
- 标记过期/休眠。
- 冲突检测。
- 来源证据浏览。
- 按 agent / scope / activation / risk 过滤。
- 回滚最近一次 sync。

验收标准：

- 用户能解释“这张卡为什么存在、来自哪里、给了谁、写到了哪里、最后一次何时生效”。

### 方向 D：工程重构，降低 Agent 协作成本

建议不是大爆炸重写，而是围绕产品闭环拆模块。

后端可以按边界收口：

```text
src/
  observation/        # 导入、去噪、session index
  extraction/         # strip/truncate/cluster/induce/crystallize
  review/             # suggestion/draft 审查动作
  memory_card/        # card store、merge、verify、lifecycle
  assignment/         # target matrix、agent loadout
  artifact/           # build preview、diff、sync、drift import
  eval/               # golden set、real-project eval、judge
  kernel/             # policy、audit、confirmation
```

前端可以按工作流重排：

```text
app/src/features/
  project-dashboard/
  review-inbox/
  memory-library/
  agent-loadout/
  artifact-preview/
  job-center/
  evals/
```

这不是为了“目录漂亮”，而是让每个模块都能回答三件事：它做什么、依赖什么、如何测试。

## 6. 我不建议继续投入的方向

### 6.1 不建议继续堆复杂 gate

早期文档已经自我反驳过：schema-first 的多级 gate 会误杀真实合格卡。现在应坚持 data-first：真实样本、黄金集、证据校验、少量稳定规则。

### 6.2 不建议 v1 做知识图谱或重型向量库

项目当前规模下，本地文件 + fastembed + 内存聚类足够。引入数据库、图谱或外部服务会稀释“本地优先、可 diff、可迁移”的优势。

### 6.3 不建议让 AI 自动更新/删除长期记忆

新增候选可以自动提出，但 UPDATE / MERGE / DELETE / SYNC 这类高影响动作默认应进入审查。Mem0 那类自动 memory 操作适合服务端产品，但这个项目的信任卖点恰好是人工边界。

### 6.4 不建议把 Catalog/App Store 放在主线

Catalog 有想象力，但 v1 最该证明的是“我能从你的真实历史里提炼出可信卡片，并安全写回 Agent”。这个没打穿前，包生态会显得飘。

## 7. 一个可执行的重构路线

### Phase 1：产品闭环修复，两周

目标：让用户能稳定完成一次“提炼 -> 审查 -> 写入 -> 验证”。

- Review Inbox 统一心智模型：把 Candidate/Draft 视觉上合成一个待审队列。
- 增加 Evidence Panel。
- 增加 Artifact Diff Preview。
- Job 完成后给结构化 summary。
- Dashboard 改成下一步导向。

### Phase 2：质量评测闭环，两周

目标：以后每次改提炼逻辑都有数字。

- 建 30-60 条黄金集。
- eval 输出 precision/recall/evidence validity/duplicate/one-off false positive。
- 把当前真实项目跑一次 baseline。
- 评测摘要进入 docs 或 UI。

### Phase 3：Memory Card 治理，两到三周

目标：让长期库不变脏。

- 合并、休眠、过期、冲突检测。
- 卡片 lineage 页面。
- loadout 覆盖率和未分配卡提醒。
- 最近一次 sync 可回滚。

### Phase 4：模块化重构，穿插进行

目标：让代码库适合长期 Agent 协作。

- 拆大文件，不改行为。
- 后端按 source/review/card/assignment/artifact/eval/kernel 分边界。
- 前端按 feature 分区。
- 保留现有测试，先迁移再新增。

## 8. 我心里的 v1 验收标准

我建议用这组标准判断“效果达标”：

1. 从真实历史提炼 5-10 条候选，其中用户愿意批准至少 2 条。
2. 每条建议都有真实 evidence quote，且能打开来源。
3. 用户能在写入前看到 artifact diff。
4. 手动改过 `AGENTS.md` / `CLAUDE.md` 时，sync 不覆盖，能导入 drift。
5. 提炼质量有 eval 报告，不再靠感觉争论。
6. 一个新用户 10 分钟内能完成第一张卡的安全写入。
7. 一个重度用户能解释自己的 memory library：哪些常驻、哪些按需 skill、哪些休眠。

## 9. 我想和你确认的关键问题

下面这些问题会决定是“局部改进”还是“重构产品主线”。

1. 你现在最失望的是哪一类效果？
   - 提炼出来的卡质量差。
   - UI 不好用，不知道怎么完成闭环。
   - 写入/同步不可信。
   - 项目概念太多，心智负担大。
   - 代码工程上越来越难维护。

2. 你期待的 v1 是个人自用工具，还是可以给其他 Claude Code / Codex 重度用户安装的产品？

3. 你更想先看到哪种改进？
   - 一次端到端可用体验。
   - 提炼质量明显提升。
   - UI/交互焕然一新。
   - 工程架构重构。

4. 你能接受“默认不自动写入，所有长期记忆必须审查”的保守策略吗？我认为这是项目差异化优势，但会牺牲一点自动化爽感。

## 10. 我的建议结论

我建议不要马上推倒重写，也不要继续在现有体验上堆算法。更好的路径是：

```text
先重构用户闭环
  -> 再用黄金集约束提炼质量
  -> 再做 Memory Card 治理
  -> 最后按边界拆工程模块
```

如果只选一个突破口，我会选 Review Inbox：

它应该成为项目的核心舞台。用户打开项目后，不是看到一堆抽象指标，而是看到“系统从真实历史里发现了这些可能长期有用的经验，每条都有证据、风险、推荐动作，你可以批准、编辑、合并、拒绝，然后预览写入结果”。这个体验成立了，Agent Memory Kernel 才真正成立。

## 11. 2026-05-15 本轮落地记录

这轮我按你给的顺序先打穿“用户闭环”，再把黄金集、Memory Card 治理和模块边界接进来。核心判断没有变：项目应该像一个可审查的 memory compiler，而不是一个偷偷替用户写长期记忆的 agent。

### 11.1 用户闭环

已把 Review Inbox 往“下一步工作台”推进：

- Drafts 页面同时读取候选、草稿、Memory Card library、assignment view 和 quality view。
- 顶部显示下一步：审查候选、审查草稿、分配卡片、处理 drift、预览写入或重新提炼。
- 候选/草稿显示 evidence 概览：来源数量、置信度、route、compile 状态和弱证据警告。
- Artifact Preview 不再只显示字符串动作，而是优先读取后端结构化 artifact rows，展示 create/update/drifted、目标文件和 diff 摘要。
- Artifact Preview 现在同时返回完整 diff lines 和截断标记；Review 页面可以展开查看目标文件差异，不再只依赖短摘要。
- 当 Artifact Preview 进入 blocked/drift 状态时，页面提供三种恢复动作：`import_artifact_drifts` 导入为草稿、`keep_artifact_drifts` 保留当前手改并更新 artifact lock、`discard_artifact_drifts` 丢弃手改并恢复生成内容。三者都走 kernel policy，避免 drift 卡住时只剩一条路。
- `discard_artifact_drifts` 在 UI 上被标记为 destructive action，并带明确确认文案：“会丢弃当前 drift 文件里的手动改动，并恢复为 Memory Card 生成内容”。这把高风险路径和“导入为草稿/保留手改”区分开。
- structured artifact preview 现在会把 `drifted` / `unmanaged` targets 单独归入“阻塞写入的 Drift 文件”组，显示目标路径、首条 diff 摘要、截断状态，并提供上一页/下一页分页，避免大 drift 集合只散落在普通 target chips 里。

这一步解决的是“用户不知道下一步该相信什么”的问题。

### 11.2 Golden Set 约束质量

已把黄金集从文档愿望推进到可执行 gate：

- 新增 `cargo run -- eval --golden-set --project .`。
- Golden Set regression 现在覆盖 positive recall 和 negative precision。
- 当前 focused 验证里，黄金集 positive recall 为 100% (17/17)，negative precision 为 100% (28/28)。
- Golden Set eval report 现在显式输出 one-off false positive 指标；当前为 0% (0/28)。
- Golden Set eval report 现在显式输出 duplicate cluster risk，用 positive case 被拆成多簇来近似提示重复候选风险；当前为 29% (5/17)。
- Golden Set eval report 现在显式输出 evidence validity，用 deterministic induce stub 跑到最终 crystallized card，再验证 `evidence_quotes` 是否能回到原始 observation；当前为 88% (15/17)，剩余弱项集中在真实历史回归和人工审阅边界这两类 archived case。
- Golden Set 已扩到 17 条 positives / 28 条 negatives，新增覆盖“provider evidence quote 必须可回溯”“不能把绿色测试当完成代理信号”“file-scoped drift resolution”“Memory Card merge review”“前端 feature 边界”等正例，以及 provider timeout、malformed JSON、当前指标事实、单次导航/安装/文件路径等负例。
- Golden Set 的最小规模 gate 也从早期 4/4 提升为 17 positives / 28 negatives，避免以后质量基线被无意缩回玩具样例。
- `cluster` 语义锚点显式覆盖人工审阅边界、规则写入控制、测试范围、git 高风险确认和输出风格。
- `cluster` 语义锚点新增 artifact diff preview 主题，避免“AGENTS.md / CLAUDE.md 写入前看完整差异”这类同义表达在无 embedding 环境下被拆散。
- `cluster` 语义锚点新增 workflow-first 与 evidence-before-confidence 主题，承接你 Obsidian 里的工程品味：固定路径优先 workflow，结论必须跟着验证证据走。
- `cluster` 语义锚点新增 provider evidence grounding、file-scoped drift resolution、Memory Card merge review 三类主题，让新 Golden Set 正例可以在无 embedding 环境下稳定聚类。
- Golden Set CLI 现在支持真实 provider-induced evidence 验证：默认 `eval --golden-set` 仍是 deterministic；如果传 `--provider <name>`，会额外跑真实 provider INDUCE，并报告 `provider evidence validity`，用于检查 provider 输出的 `evidence_quotes` 是否真的能回到原始 observation。

这一步解决的是“提炼质量不能只靠感觉”的问题。

### 11.3 Memory Card 治理

已把 Memory Card Library 从静态列表推进到治理面板：

- Memory Cards 页面展示总卡片、已分配、未分配、缺来源、需复核。
- 每张卡展示治理状态、来源标签、已分配 targets、合并次数和 warning。
- Memory Cards 页面现在会提示疑似重复卡片，用保守的本地相似度启发式防止长期库慢慢堆出同义规则。
- 疑似重复卡现在可以直接生成“合并草稿”，走 `fuse_memory_cards_to_draft`，先进入审查队列，而不是直接删除源卡或自动合并。
- 每张卡现在可以展开“来源链路”，查看批准来源、证据摘要和合并历史，避免未来编辑时只看最终文本、不知道它为什么存在。
- Memory Card 治理现在识别 `status:dormant` / `lifecycle:dormant` 和 `status:expired` / `lifecycle:expired` 标签，并在 summary 和单卡治理状态里显示休眠/过期。
- Memory Card 治理面板现在提供“标记休眠 / 标记过期 / 恢复活跃”操作，通过 `update_memory_card` 更新 lifecycle tags，并走 Kernel plan 与 read model refresh。
- 疑似重复卡现在有“冲突解释”展开项，显示相似卡 id、标题和本地相似度分数，不再只给一句 warning。
- Memory Cards 页面新增治理维度筛选：全部治理、需复核、疑似重复、未分配、缺来源、休眠、过期。用户可以先从一类治理问题进入批量处理，而不是在长列表里逐张找异常。
- Memory Card 治理新增保守批量动作：在休眠/过期筛选下可批量恢复活跃，在疑似重复筛选下可批量生成合并草稿；每一步仍逐张走 `update_memory_card` 或 `fuse_memory_cards_to_draft` 的 Kernel policy，不做静默合并或静默删除。
- 单卡 lifecycle 操作已补齐 `update_memory_card` 的 `id` 参数，避免 UI 只传 tags 而无法把治理变更绑定到具体 Memory Card。
- 编辑、删除、分配矩阵、sync 都走现有 mutation/read model refresh 闭环，避免 UI 显示和真实状态脱节。

下一步更值得做的是把合并草稿变成更清晰的 lineage 视图，并把批量动作执行结果做成更细的逐项反馈。

### 11.4 工程模块边界

本轮没有做大爆炸重构，只拆了一个已经影响闭环的边界：

- `src/build.rs` 继续负责 build/sync 主流程。
- `src/build/artifact_preview.rs` 负责结构化 artifact preview、hash、status 和 diff 摘要。
- `src/build/artifact_drift.rs` 负责 artifact drift 的导入、保留、丢弃和新增行提取，`build.rs` 不再承载这些恢复路径的细节。
- `src/memory_card/target_matrix.rs` 负责 Memory Card 到 Agent 的 assignment read model，`memory_card.rs` 不再同时承载目标矩阵渲染和卡片写路径。
- `src/memory_card/merge.rs` 负责 Memory Card 合并写路径，治理动作开始从基础 card CRUD 中分离出来。
- `src/memory_card/global.rs` 负责 Memory Card 的 promote-to-global 与 install-global-to-project，跨项目分发路径不再挤在本地卡片 CRUD 里。
- `src/memory_card/edit.rs` 负责 Memory Card 编辑、review update 和 lifecycle tag 更新所走的 `update_memory_card` 路径，治理状态变更和基础存储加载进一步分离。
- 前端新增 `review-workbench.ts`、`memory-governance.ts` 这类纯函数 read-model 层，页面组件只消费结果。
- 前端 Artifact Preview UI 已从 Drafts 页面抽到 `app/src/components/review/ArtifactPreviewStrip.tsx`，Review 页面只负责编排 read model 和动作回调。
- 前端 Artifact Drift 阻塞文件分页视图已抽到 `app/src/components/review/ArtifactDriftGroup.tsx`，Artifact Preview 主组件不再承载 drift 分页细节。
- 前端 Memory Card 治理 UI 已从 MemoryCards 页面抽到 `app/src/components/memory/MemoryGovernancePanel.tsx`，Library 页面只负责列表、筛选和编辑状态。
- 前端批量治理按钮已抽到 `app/src/components/memory/MemoryGovernanceBatchBar.tsx`，批量 action 的生成逻辑保留在 `memory-governance.ts` 纯函数里，页面不再直接拼装批量步骤。

这个方向后续可以继续扩展成 `artifact/`、`assignment/`、`review/`、`eval/` 更清晰的边界。

### 11.5 外部资料校准

我本轮重新看了几类外部资料，和你的 Obsidian 笔记结论一致：

- [OpenAI: Unrolling the Codex Agent Loop](https://openai.com/index/unrolling-the-codex-agent-loop) 强调 agent loop 是用户、模型和工具之间的可执行环境，不是长聊天；这支持我们把 Memory Card 提炼做成 observe -> act -> verify 的闭环。
- [Anthropic: Writing effective tools for AI agents](https://www.anthropic.com/engineering/writing-tools-for-agents) 强调工具质量要用 eval 检查，工具响应要帮助 agent 理解下一步；这支持结构化 artifact preview，而不是继续解析自然语言 action。
- [Anthropic: Demystifying evals for AI agents](https://www.anthropic.com/engineering/demystifying-evals-for-ai-agents) 强调 eval 要围绕真实工作流的“不破坏、按要求做、做得好”；这支持 Golden Set gate 和后续 evidence validity。
- [Anthropic: Building Effective Agents](https://www.anthropic.com/engineering/building-effective-agents) 区分 workflow 和 agent；本项目 v1 主链路应继续保持 workflow-first，LLM 只承担局部归纳和重写。

### 11.6 仍未完成的硬问题

我不建议现在宣布整个目标完成，剩下这些才是下一轮最有价值的工作：

1. Artifact Preview 已有可展开 diff viewer，但还缺更完整的分组、语法高亮和大文件分页。
2. Drift 恢复路径已经有“导入为 Draft / 保留手改 / 丢弃手改”三条路，discard 也有 destructive tone 和二次确认，blocked preview 会单独分组并分页展示 drift 文件；后续主要是做逐文件操作。
3. Memory Card 治理已有疑似重复提示、冲突解释、合并草稿入口、来源链路、休眠/过期识别、lifecycle 操作、治理状态筛选和保守批量动作；后续主要是补更完整的合并审查体验和逐项执行反馈。
4. Golden Set 已扩到 45 条，并加入 one-off false positive、duplicate cluster risk、deterministic evidence validity 与可选 provider evidence validity 指标；但真实 provider 路径还需要在本地配置 provider 后跑实测，并继续补更多历史失败样例。
5. 模块拆分还只是局部开始，后端 `build` 已拆出 artifact preview / artifact drift，`memory_card` 已拆出 target matrix / merge / global / edit；但 `extract` 和前端 feature 边界仍可继续收口。
