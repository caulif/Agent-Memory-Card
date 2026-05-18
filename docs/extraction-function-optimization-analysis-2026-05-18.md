# 提炼功能全面优化方案

> 日期：2026-05-18  
> 目标：结合本项目现状、`C:\obsidian` 中工程设计 / 产品逻辑 / Agent / Memory 相关笔记，以及外部成熟项目和资料，分析当前提炼功能为什么仍然“不够满意”，并给出可落地的改进路线。  
> 结论：不要再把问题理解成“prompt 不够强”。真正的问题是提炼系统还没有形成稳定的 memory harness：数据源、证据、候选、审查、写入、评测和反馈回流还没有完全闭环。

## 1. 当前判断

这个项目的定位仍然是对的：它不是聊天总结器，而是本地优先的 Agent Memory 编译器。

```text
真实对话 / 项目规则 / Skills / 反馈
  -> Observation
  -> Suggestion
  -> Review
  -> Memory Card
  -> Assignment
  -> Artifact
  -> Eval / Feedback
```

但现在的实现还没有稳定地让用户相信三件事：

1. 系统真的找到了“长期有用”的东西，而不是找到了“看起来像规则”的句子。
2. 每条候选都有足够证据、上下文和生命周期判断。
3. 用户批准 / 拒绝以后，这些反馈会反过来让下一轮提炼变好。

我认为“不满意”的核心原因不是单个页面或单个 prompt，而是四个断点：

- 真实项目召回断点：Provider 路径证据更干净，但 top-k 和聚类会偏向工程方法论，容易漏 DaoFocus 这类产品规则。
- 证据契约断点：pipeline-v2 已经有 `evidence_quotes` 校验，但旧 LLM extraction path 仍多处写 `source_observations: Vec::new()`，导致候选 lineage 不一致。
- 审查体验断点：Candidate / Draft / Memory Card 的内部概念仍然暴露给用户，用户看到的是“列表”，不是“证据 -> 判断 -> 影响 -> 操作”的审查案件。
- 评测回流断点：Golden Set 已可用，但还没有持续吸收真实批准、真实拒绝、provider 失败和历史误判。

## 2. 来自 Obsidian 的设计约束

我读了这些笔记：`Claude Code Memory 相关问题`、`Workflow vs Agent`、`工具响应设计`、`工程验证闭环`、`Agent Evals`、`Coding Agent Harness`、`CLAUDE.md 设计原则`、`个人数据与长期记忆`、`结构化输出`。

这些笔记对本项目的约束很明确。

### 2.1 Workflow-first

固定路径能解决的问题优先 workflow，LLM 只做局部判断。提炼主链路应该是可解释流水线，不是黑盒 Agent。

当前项目已经朝这个方向做了 `STRIP -> TRUNCATE -> CLUSTER -> INDUCE -> CRYSTALLIZE`，但 UI 和评测还没有把每层结果变成用户可见的决策依据。

### 2.2 验证闭环优先

工程验证闭环强调：动手前和交付后都要回答“怎么证明真的对了”。对应提炼功能，不能只说生成了 8 条候选，而要证明：

- 哪些候选来自真实 observation。
- 哪些被拒绝，为什么拒绝。
- 哪些进入 Memory Card 后确实影响了 `AGENTS.md` / `CLAUDE.md` / Skill。
- 哪些用户反馈被写回 Golden Set。

### 2.3 工具响应要服务下一步决策

工具响应不是日志倾倒，而是高信号、结构化、可分页、可操作。提炼报告和 UI 应默认展示：

- `decision`: accept / reject / review-only / merge / expire
- `evidence`: quote、source、context before/after
- `risk`: scope risk、one-off risk、duplicate risk、drift risk
- `next_actions`: edit、approve、merge、reject、add-to-golden

### 2.4 Memory 要分层

个人长期记忆笔记强调短期、长期、偏好、项目记忆、任务进度需要分层。当前 Memory Card 同时承载偏好、流程、产品规则、工程经验、临时阶段约束，分类还不够锋利。

## 3. Hermes 设计经验可迁移点

我额外搜集了 `C:\obsidian\AI与Agent\项目实践\hermes-ai-daily` 里的 Hermes 设计材料，包括 `Hermes AI 日报自动化`、`Hermes 功能与触发方式`、`Hermes Agent 能力地图与个人用法讨论`、`AI日报与每日想法工作流`、`AI 研究助理工作流`、`interest-profile.md`、日报 QA 报告和多版 candidate 迭代。

Hermes 和 Agent Memory Kernel 表面上一个做信息日报，一个做 memory 提炼，但底层问题很像：都是从大量噪声输入中筛出少量高价值内容，并持续根据用户反馈调优。

### 3.1 固定结构不是死板，而是降低审查成本

Hermes 日报固定栏目顺序：

```text
质量说明 -> 可信度分级 -> 昨日新闻 -> 近期深读
-> AI 产品与软硬件观察 -> 工程底蕴 -> 待核验线索
-> 可连接到的已有笔记 -> 兴趣画像更新 -> AI 整理区 -> 今日日报建议
```

这给本项目一个很直接的启发：Review Inbox 也应该固定“审查案件结构”，而不是每条候选随意展示字段。

建议 Memory Suggestion 固定结构：

```text
质量说明 -> 证据可信度 -> 候选正文 -> 来源上下文
-> 重复/冲突 -> 生命周期 -> Artifact 影响 -> 建议动作 -> 用户反馈
```

这样用户每次审查都走同一条心智路径，系统也更容易做 QA。

### 3.2 可信度分级应进入 evidence model

Hermes 把来源分成 A / B / C：

- A：官方博客、官方文档、第一手工程文章。
- B：高质量个人博客、社区项目、arXiv 等需要进一步判断的来源。
- C：媒体转述、社区讨论、产品首页、无法确认来源。

对应到 Memory 提炼，建议把 evidence 分级：

- A：用户直接表达的长期偏好、明确纠正、明确批准。
- B：用户在任务 brief 中隐含的偏好、反复出现的操作选择、人工编辑后的 artifact drift。
- C：assistant 总结、工具输出、生成文件、当前状态事实。

只有 A/B 可以默认进入 Review Inbox；C 只能作为辅助上下文，不应单独支撑 Memory Card。

### 3.3 `待核验线索` 是比 reject 更好的中间层

Hermes 不把社区讨论、日期不稳、来源不明的内容直接丢掉，而是放入 `待核验线索`。这比二元 accept/reject 更贴近真实工作流。

本项目也需要类似层级：

```text
accept-ready      可直接进入审查批准
needs-evidence    方向可能对，但证据不足
needs-context     有指代丢失或上下文不完整
needs-merge       可能重复或冲突
expirable         阶段性有效
reject            明确不应记忆
```

这会减少“有点价值但不能长期固化”的内容被错误丢弃或错误批准。

### 3.4 seen-urls 去重可以迁移成 seen-memory-signatures

Hermes 用 `seen-urls.txt` 防止日报重复收录同一 URL，并区分“同 URL 不收录”和“同主题新文章可收录”。

Memory Kernel 可以借鉴为：

```text
.agent-kernel/seen-memory-signatures.jsonl
```

每条记录保存：

- normalized body hash
- evidence quote hash
- source observation id
- approved/rejected/edit/merged outcome
- related card id
- timestamp

这样可以做到：

- 同 evidence + 同结论：抑制重复候选。
- 同主题 + 新证据：标为 update / refine，而不是新卡。
- 曾被用户拒绝：再次出现时降低优先级或要求新证据。

### 3.5 QA 脚本比“感觉好不好”更可靠

Hermes 的 `score-ai-daily.ps1` 会检查结构缺失、泛链接、风险数字、重复 URL、批注块数量，并给出 100 分制评分。2026-05-17 日报 QA 已能输出 `96/100` 和具体风险项。

Memory Kernel 应该有类似 `score-extraction-run`：

```text
agent-kernel eval score-run --project <path> --run <run-id>
```

建议检查项：

- evidence validity
- empty source observations
- C 级证据单独支撑候选
- duplicate signatures
- scope risk
- lifecycle risk
- missing artifact impact
- provider parse failures
- rejected reason coverage

低于阈值时，不应该鼓励用户批准，而应提示先修提炼策略或补证据。

### 3.6 `今日日报建议` 是轻量 feedback-to-eval 入口

Hermes 把对下一份日报的反馈固定写到 `今日日报建议`，下一次生成前优先读取。这是很轻量但有效的反馈闭环。

Memory Kernel 可以增加：

```text
.agent-kernel/review-feedback.md
```

或 UI 内的“本次提炼反馈”区。用户可以写：

- 这次太多工程规则，漏了产品规则。
- 不要把阶段性“不要加功能”固化。
- 以后 DaoFocus 要优先抓数值、交互、体验规则。
- 这些候选太像 assistant 总结，证据不够。

下一次 extraction 前先读取这些反馈，并自动转成 Golden Set 候选或 lane 权重调整。

### 3.7 兴趣画像可以迁移为 extraction profile

Hermes 的 `interest-profile.md` 记录搜索偏好、来源权重、时间窗口、升权/降权变化。Memory Kernel 需要类似的项目级 extraction profile：

```yaml
project_profile:
  high_value_lanes:
    - product_rule
    - interaction_rule
    - engineering_method
  low_value_lanes:
    - current_task_status
    - generated_artifact
  domain_terms:
    - DaoFocus
    - 灵石
    - 境界
    - 修炼
    - 拖动
  evidence_policy:
    min_trust: B
    require_user_direct_for_always_on: true
```

这个 profile 应由用户审查反馈、批准卡和项目资料共同更新，而不是全靠硬编码关键词。

## 4. 外部资料校准

外部资料给出的方向和你的笔记一致：

- Anthropic 的 [Building effective agents](https://www.anthropic.com/engineering/building-effective-agents) 明确区分 workflow 和 agent，并建议先用简单可组合模式，只有需要时才增加复杂度。
- GitHub 的 [Validating agentic behavior when correct is not deterministic](https://github.blog/ai-and-ml/generative-ai/validating-agentic-behavior-when-correct-isnt-deterministic/) 强调不要相信 agent 自评，要看真实执行轨迹里是否经过必要里程碑。
- LangChain 的 [Memory overview](https://docs.langchain.com/oss/python/concepts/memory) 把长期记忆拆成 semantic / episodic / procedural，也区分 hot path 与 background memory 更新。
- Mem0 的 [Add Memory](https://docs.mem0.ai/core-concepts/memory-operations/add)、[Update Memory](https://docs.mem0.ai/core-concepts/memory-operations/update)、[Delete Memory](https://docs.mem0.ai/core-concepts/memory-operations/delete) 把 add/search/update/delete 和冲突处理作为一等 memory operation。
- OpenAI Agents SDK 的 [Guardrails](https://openai.github.io/openai-agents-python/guardrails/) 把 input / output / tool guardrails 放进 workflow 边界；这对应本项目的提炼前过滤、provider 输出校验和写入前 policy。
- OpenAI 的 [Structured Outputs](https://platform.openai.com/docs/guides/structured-outputs) 说明 JSON mode 只保证合法 JSON，Structured Outputs 才强调 schema adherence；本项目 provider 适配应优先使用支持 schema 的 provider 能力，而不是只做 lenient JSON 解析。

这些资料共同指向一个结论：本项目应该成为“提炼、审查、治理、评测”的 harness，而不是把希望压在单次 LLM 归纳上。

## 5. 现状证据

我本轮跑了当前 Golden Set：

```text
positive recall: 100% (17/17)
negative precision: 100% (28/28)
one-off false positive: 0% (0/28)
duplicate cluster risk: 29% (5/17)
evidence validity: 88% (15/17)
```

这说明当前下限已经比早期强很多，但它仍然不是“满意”的充分条件：

- 正例只有 17 条，且手写样例比例高。
- evidence validity 还没到 95% 以上。
- duplicate cluster risk 仍有 29%。
- provider evidence validity 是可选路径，不是默认 gate。
- Golden Set 目前主要验证 strip / cluster / evidence 机制，不足以代表真实 DaoFocus 产品规则召回。

代码层面也能看到边界压力：

```text
src/eval.rs                 1264 lines
src/provider.rs             1134 lines
src/candidate.rs             959 lines
src/observation.rs           927 lines
src/extract/induce.rs        921 lines
src/extract.rs               902 lines
app/src/components/pages/Drafts.tsx 629 lines
```

大文件本身不是罪，但在 agent 协作场景里，它会让每次改提炼逻辑的认知负担变高，也会让“局部重构 + 回归验证”更难。

## 6. 不完善点清单

### 6.1 输入层：Observation 还不够像“案件材料”

现在 observation 更像被抽出来的文本片段。对 memory 提炼来说，它还缺少：

- 会话级 thread id 和 turn range。
- 用户消息 / assistant 消息 / tool 结果的可靠角色标注。
- 上下文窗口：quote 前后各 1-3 条消息。
- 项目实体识别：DaoFocus、Agent Kernel、Obsidian、当前 workspace。
- 来源信任等级：用户直接指令 > 用户反馈 > assistant 总结 > 工具输出 > 生成 artifact。

没有这些，系统很难判断“这是长期规则、阶段性约束、当前任务、还是 assistant 自嗨”。

Hermes 的启发是：输入不是一堆文本，而是一组带来源等级、时间窗口、是否已见过、是否待核验的材料。Observation 也应该至少携带 source trust、thread context 和 seen signature。

### 6.2 召回层：BatchTopK 容易偏向工程方法论

当前 pipeline 默认 `BatchTopK { evidence_top_k: 8, max_cards: 5 }`。这对减少成本有效，但会造成一个真实问题：高信号工程词更容易被选中，产品细节规则可能被挤掉。

DaoFocus 历史提炼已经暴露过这个问题：provider 最终更容易产出“代码变更后做质量审查”这类 global 工程卡，而 DaoFocus 的经济系统、修炼难度、拖动入口等产品规则主要在本地 candidates 里出现。

建议改成“分泳道 top-k”，而不是全局 top-k：

- product-rule lane：产品机制、数值、交互、视觉、体验。
- engineering-method lane：验证、测试、review、边界、工具。
- collaboration-preference lane：自主性、反馈格式、是否提问。
- memory-governance lane：提炼、证据、Golden Set、artifact drift。
- temporary/task lane：阶段性规则，只能进 expirable，不进 always-on。

每个 lane 有自己的 top-k、阈值和拒绝规则。

这和 Hermes 的栏目化来源策略一致：昨日新闻、近期深读、产品观察、工程底蕴分开筛选，避免单一 ranking 把某类内容挤掉。Memory 提炼也不应该让 engineering-method lane 抢掉 product-rule lane 的预算。

### 6.3 证据层：新旧路径证据契约不统一

pipeline-v2 的 `induce.rs` 已经要求 `evidence_quotes` 必须能回到簇内 observation，这是正确方向。

但旧的 `extract_llm_text_to_drafts` 路径仍然多处创建候选时传 `source_observations: Vec::new()`。这会导致：

- UI 有时只能展示 evidence 字符串，不能打开真实来源。
- Memory Card verify 会报 lineage 弱。
- Golden Set 通过的证据约束不能覆盖所有用户实际使用入口。

建议建立统一 `EvidenceBundle`：

```text
EvidenceBundle
  source_observation_ids: [id]
  quotes: [{ observation_id, text, role, created_at }]
  context: [{ observation_id, before, after }]
  source_trust: user_direct | user_feedback | assistant_summary | tool_output | artifact
  validity: valid | weak | invalid
```

所有 Candidate / Draft / Memory Card 都只接受这个结构，不再让各路径自由拼 `evidence: String`。

### 6.4 归纳层：when/what/why 模板仍有损真实语义

之前 reality check 已经证明 schema-first 会误杀真实合格卡。现在虽然已经改成 evidence quote 软约束，但 `Crystallize` 仍把候选渲染成固定句式：

```text
当 X 时，Y；目标是 Z。
在 X 时，必须 Y；目的是 Z。
在 X 时，偏好 Y；原因是 Z。
```

这带来两个问题：

- 真实产品规则常常是 case-analysis，比如“小改快测，大改重测”，不适合单 when/what。
- why 经常是隐含的，强制写 why 容易制造听起来合理但未被证据支持的解释。

建议把 Memory Card body shape 拆成几类：

- `conditional_rule`: 单条件规则。
- `case_policy`: 多条件分支。
- `preference`: 偏好或风格。
- `workflow`: 步骤型流程。
- `negative_boundary`: 不要做什么。
- `product_fact`: 项目产品规则。
- `review_principle`: 审查/验证原则。

并增加 `body_origin: inferred | quoted | edited`，让用户知道正文是归纳、引用还是人工编辑。

### 6.5 拒绝层：负例还缺真实“失败故事”

现在 negatives 覆盖了一次性任务、临时路径、provider malformed JSON 等，但还缺几类真实失败：

- 指代丢失：例如 “A + B，再用 C”。
- assistant 复述用户偏好后被误当成用户偏好。
- 当前项目事实被误判为长期规则。
- 已过期产品方向被继续固化。
- 同一条偏好在 global / project 两边各生成一条。
- provider 输出字段合法但语义错配。
- 低质量 evidence quote 正确可回溯，但不足以支持 body。

这些应该来自真实 rejected candidates 和用户手动修改，而不是继续手写。

Hermes 的 candidate 迭代也说明，失败样例要保留。低分日报、QA 报告、遗留旧结构、泛链接、风险数字都被记录下来。本项目也应该保留 provider 失败输出、用户拒绝理由和候选改写前后对照，而不是只留下最终 Memory Card。

### 6.6 生命周期层：缺少 task-scoped / expirable 一等状态

现在已有 dormant / expired 标签，但提炼时还没有把阶段性规则天然路由到 expirable。

例如“这轮先不要加额外功能”很有价值，但它不是长期 memory。正确结果不是 reject，也不是 always-on，而是：

```text
scope: project
lifecycle: expirable
expires_when: current_goal_complete | after_date | after_sync
compile_route: review_only
```

这类记忆如果没有一等模型，要么被拒绝丢掉，要么污染长期库。

### 6.7 审查层：用户看到的不是“证据案件”

用户真正要审的是：

- 这条建议说什么？
- 证据来自哪里？
- 有没有上下文丢失？
- 是长期、阶段性、还是一次性？
- 是否和已有卡重复或冲突？
- 批准后会影响哪些 Agent 和哪些 artifact？
- 如果我编辑，系统如何保留 lineage？

现在 UI 已经有 Evidence summary、candidate edit、artifact diff、Memory governance，但还不够“案件化”。建议把 Review Inbox 里的每条建议变成四栏：

```text
Suggestion
Evidence
Decision
Impact
```

### 6.8 反馈层：批准 / 拒绝没有自动变成训练资产

这是最关键的增长飞轮缺口。

用户在 UI 里做的每次操作都应该写入 eval corpus：

- approve -> positive exemplar
- reject + reason -> negative exemplar
- edit -> before/after rewrite pair
- merge -> duplicate/merge exemplar
- expire -> lifecycle exemplar
- import drift -> human-authored memory exemplar

没有这个回流，Golden Set 永远靠人工维护，质量提升会慢。

Hermes 的反馈闭环已经证明轻量入口有效：当天日报末尾的建议会影响下一份日报。Memory Kernel 也应该把每次 Review Session 末尾变成“下次提炼建议”，再把这些建议汇总进 extraction profile 和 Golden Set。

## 7. 推荐目标架构

建议把提炼功能重构为 6 个明确模块。

```text
observation/
  ingest, normalize, thread, project attribution, source trust

extraction/
  strip, chunk, lane routing, cluster, induce, verify evidence

review/
  suggestion model, decision workflow, edit diff, reject reasons

memory/
  card schema, lifecycle, merge, conflict, lineage

artifact/
  assignment, preview, sync, drift resolution, rollback

eval/
  golden set, real-project runs, provider trace, feedback corpus
```

借鉴 Hermes 后，建议再补两个横切模块：

```text
profile/
  project extraction profile, lane weights, domain terms, trust policy

qa/
  run scoring, structural checks, duplicate signatures, review readiness
```

核心对象也建议改名收口：

- `Suggestion`: 用户看到的待审建议。内部可来自 Candidate 或 Draft，但 UI 不再暴露两层。
- `EvidenceBundle`: 所有证据的唯一结构。
- `MemoryCard`: 人工批准后的长期资产。
- `MemoryWriteIntent`: add / update / merge / supersede / expire / reject / review-only。
- `ReviewDecision`: approve / edit / reject / merge / mark-expirable。
- `ArtifactImpact`: 将影响的 agent、文件、diff 和风险。
- `ExtractionProfile`: 项目级提炼画像，记录 lane 权重、领域词、证据策略、最近用户反馈。
- `RunQAReport`: 每次提炼运行的结构化质量报告，类似 Hermes 日报 QA。

## 8. 具体优化路线

### Phase 0：建立真实基线

目标：先承认当前真实效果。

- 对 `C:\DaoFocus`、本项目、`C:\obsidian` 各跑一次 provider pipeline。
- 每个项目人工标 20 条：应批准、应拒绝、应合并、应过期。
- 产出 `docs/runs/extraction-baseline-YYYYMMDD.md`。
- 指标包括：product-rule recall、engineering-rule recall、evidence validity、duplicate rate、scope accuracy、lifecycle accuracy、approval rate。

验收：

- 不再只看 Golden Set 45 条小样本。
- 能回答“DaoFocus 产品规则为什么召回少”。

### Phase 1：统一证据契约

目标：所有路径都产生同一种证据结构。

- 新增 `EvidenceBundle`。
- pipeline-v2 和 legacy LLM extraction 都必须填 source ids。
- `source_observations: Vec::new()` 只允许出现在测试 fixture 或明确的 `evidence_validity=weak` 分支。
- UI Evidence Panel 支持打开 source、quote、上下文窗口。

验收：

- 新候选 95% 以上有有效 source observation。
- Golden Set evidence validity 从 88% 提到 95%+。

### Phase 2：分泳道召回

目标：解决 provider 召回偏工程、漏产品规则的问题。

- STRIP 后先做 lane routing。
- 每个 lane 单独 cluster 和 top-k。
- product-rule lane 增加项目实体和 PRD 词典：数值、交互、UI、特效、体验、境界、灵石、拖动等。
- temporary lane 不直接 reject，进入 expirable review。
- 引入 `ExtractionProfile`，像 Hermes `interest-profile.md` 一样记录项目高价值 lane、低价值 lane、领域词和来源信任策略。

验收：

- DaoFocus 至少召回经济/修炼数值、拖动入口、自主但克制这三类规则。
- provider 路径不再只产出 global 工程流程卡。

### Phase 3：Review Inbox 案件化

目标：用户能看懂为什么批准或拒绝。

- Candidate / Draft 在 UI 统一为 Suggestion。
- 每条 Suggestion 显示证据、风险、冲突、生命周期、影响范围。
- 编辑候选时显示 original / edited diff。
- 拒绝必须选择 reason，reason 回流 eval corpus。
- 批准前展示 ArtifactImpact。
- 增加 `needs-evidence`、`needs-context`、`needs-merge`、`expirable` 等中间状态，借鉴 Hermes `待核验线索`，避免二元 accept/reject 误伤。

验收：

- 用户能在一屏内完成“证据确认 -> 编辑 -> 批准/拒绝”。
- 每次操作都生成可回放审计记录。

### Phase 4：Memory Card 治理升级

目标：防止长期库变脏。

- 做专门 Merge Review 页面：源卡差异、保留字段、合并后预览、影响 Agent。
- 增加 supersede / expire / dormant / reactivate 的明确命令。
- 支持 task-scoped / expirable memory。
- 重复和冲突检测不只看相似度，还看 scope、activation、targets、lifecycle。

验收：

- 重复卡不会直接堆进库。
- 阶段性规则不会污染 always-on。
- 每张卡都能解释 lineage 和当前影响面。

### Phase 5：Feedback-to-Golden 自动回流

目标：让用户审查成为质量飞轮。

- `.agent-kernel/eval-corpus/` 保存 review event。
- CLI 增加 `eval harvest-feedback`，把真实 approve/reject/edit/merge 转成候选 golden case。
- UI 提供“加入 Golden Set”按钮。
- Golden Set 拆分为 `core`、`project-dao-focus`、`provider-failures`、`review-failures`。
- 增加“本次提炼反馈”入口，作用类似 Hermes `今日日报建议`：下一次提炼前优先读取，并可转成 lane 权重或 Golden Set 候选。

验收：

- Golden Set 从 45 条增长到 80-120 条，其中 60% 以上来自真实历史。
- 每次 provider 失败、用户拒绝、用户编辑，都能形成回归样例。

### Phase 5.5：Run QA 与 seen signatures

目标：把每次提炼运行变成可评分、可追踪、可去重的工程产物。

- 新增 `.agent-kernel/seen-memory-signatures.jsonl`，记录已见过的 evidence/body/outcome。
- 新增 `agent-kernel eval score-run`，输出类似 Hermes QA 的结构化报告。
- QA 报告检查 evidence validity、duplicate signatures、scope risk、lifecycle risk、missing artifact impact、provider failures。
- UI 在 Review Inbox 顶部显示 run score，不达标时建议先处理证据或重复问题。

验收：

- 每次提炼有 `RunQAReport`。
- 重复候选能解释“同 evidence 重复”还是“同主题新证据”。
- 用户能看到这次提炼是否值得审，而不是直接面对一堆候选。

### Phase 6：模块边界继续拆

目标：让项目适合长期 agent 协作。

优先拆：

- `src/eval.rs` -> `eval/golden.rs`、`eval/report.rs`、`eval/provider.rs`、`eval/feedback.rs`
- `src/extract/induce.rs` -> `induce/request.rs`、`induce/parse.rs`、`induce/evidence.rs`、`induce/batch.rs`
- `src/extract.rs` -> legacy path 收口，避免与 pipeline-v2 并行漂移
- `app/src/components/pages/Drafts.tsx` -> `features/review-inbox`

验收：

- 每个模块有清晰 owner 和测试入口。
- 新增一种 review action 不需要同时改 6 个大文件。

## 9. 我建议先做的三件事

如果只选最近一轮，我建议按这个顺序：

1. 统一证据契约：解决 lineage 不一致，这是信任基础。
2. 分泳道召回：解决长期项目工作流和全局通用 memory 被低层项目细节挤掉的问题，这是效果基础。
3. 审查反馈回流 Golden Set：解决“越用越好”的发动机。

这三件事比继续打磨按钮或继续加 prompt 更关键。

结合 Hermes，我会把第 2 步进一步明确为“分泳道召回 + ExtractionProfile”，把第 3 步明确为“Review feedback -> Golden Set / profile / seen signatures”。

## 10. 不建议做的事

- 不建议上来换成重型向量数据库或知识图谱。当前问题不是存储不够高级，而是证据和反馈闭环不够稳。
- 不建议让 AI 自动写入 / 删除 / 合并长期记忆。这个项目的差异化是人工审查边界。
- 不建议继续堆独立 gate。应该把 gate 整理成输入 guardrail、provider output guardrail、review policy、artifact write policy 四类。
- 不建议让 Catalog / Pack 成为主线。提炼质量未稳定前，分发生态会放大脏记忆。
- 不建议让 extraction profile 自动静默更新 always-on 规则。可以自动提出画像变化建议，但要像 Hermes `interest-profile.md` 一样保留变化说明和证据。

## 11. 最终验收标准

我建议以后用这组标准判断“效果终于达标”：

- 对 DaoFocus 全历史，provider 路径能召回至少 4 条高质量候选，重点是项目工作流或全局通用 memory，而不是灵石、境界、拖动、教程等低层产品细节。
- 新候选 evidence validity >= 95%。
- 用户批准率 >= 40%，拒绝样例能自动回流 Golden Set。
- duplicate rate <= 10%。
- scope accuracy >= 90%。
- task-scoped / expirable 规则不会进入 always-on artifact。
- 每张 Memory Card 都能打开 lineage、source quotes、merge history 和 artifact impact。
- Golden Set 至少 80 条，真实历史来源占多数。
- 每次提炼改动都有 real-project eval report，不靠感觉判断。
- 每次提炼运行都有 RunQAReport，低于 85 分不进入默认审查流。
- `seen-memory-signatures` 能防止同 evidence 重复刷屏，同时允许同主题新证据触发 update/refine。
- ExtractionProfile 能解释为什么 DaoFocus 优先召回项目工作流，而本项目优先召回 memory governance / engineering method。

## 12. 一句话结论

当前提炼功能已经越过“玩具可用线”，但还没到“真实重度用户放心依赖线”。下一步不要追求更玄的 Agent，而要借鉴 Hermes 的稳定做法，把它做成一条工程化的 memory extraction production line：固定审查结构、来源可信度、分层召回、严格证据、待核验中间层、seen signatures 去重、人工审查、生命周期治理、artifact impact、反馈回流、可复现 QA。

## 13. 2026-05-18 实施回填

本轮已经把方案里最影响信任下限的部分落到代码里：

- 统一证据契约：新增 `EvidenceBundle` / `SourceTrust` / `EvidenceValidity`，pipeline 保存 Draft 时写入统一 evidence bundle，前端 evidence summary 会读取 bundle、source ids、quote trust 和 validity。
- 分泳道召回：BatchTopK 不再只做全局排序，增加 project-workflow / memory-governance / engineering-method / collaboration-preference / temporary-task lane 预算；DaoFocus 的长期工作流不会被低层产品细节或泛泛工程方法论完全挤掉。
- Golden Set 深化：`tests/golden/golden_set.yml` 从 45 条扩展到 60 条，新增 DaoFocus 项目工作流、review feedback、ExtractionProfile、RunQA、seen signatures，以及 provider 语义错配、过期方向、弱 quote、assistant 复述等失败样例。
- Run QA：新增 `RunQAReport` 和 `agent-kernel eval --score-run <pipeline.json>`，检查 evidence missing/invalid、provider failures、duplicate signatures、scope risk、lifecycle risk、artifact impact hint，并可写入 seen signatures。
- seen signatures：新增 `.agent-kernel/seen-memory-signatures.jsonl` 写入函数，记录 body/evidence hash、source observation ids、cluster id 和 outcome。
- Review feedback：`ReviewDecision` 支持带 reason 的拒绝，并写入 `.agent-kernel/eval-corpus/review-events.jsonl`，让批准/拒绝成为后续 harvest 的真实资产。
- 逐文件 drift：后端、Tauri 命令、kernel plan 和 UI 都支持单个 drift 文件导入、保留、丢弃，不再只能全局三动作。
- 模块边界：拆出 `candidate/evidence.rs`、`eval/golden.rs`、`eval/qa.rs`，并把大文件内联测试迁移到独立测试模块，使 repo source line rule 重新通过。

本轮验证结果：

```text
cargo test                                      PASS
cargo check --manifest-path src-tauri/Cargo.toml PASS
bun run --cwd app build                         PASS
bun test app/src/utils/kernel-plan.test.ts app/src/utils/review-workbench.test.ts app/src/utils/memory-governance.test.ts PASS
agent-kernel eval --golden-set --project .      PASS
```

当前 Golden Set 指标：

```text
positive recall: 100% (22/22)
negative precision: 100% (38/38)
one-off false positive: 0% (0/38)
duplicate cluster risk: 23% (5/22)
evidence validity: 91% (20/22)
```

仍建议继续做的下一层：

- 把 `EvidenceBundle.context` 接到真实 thread turn 前后文，而不是只保存 quote。
- 把 `ReviewDecision` 的 edit / merge / expire 事件也写入 eval corpus，并提供 `eval harvest-feedback` 命令生成候选 Golden cases。
- 把 ExtractionProfile 从硬编码 lane markers 升级为项目文件，例如 `.agent-kernel/extraction-profile.yml`。
- 把 RunQAReport 接入 Review Inbox 顶部，让低分 run 先提示处理证据和重复问题。

## 14. 2026-05-18 真实 provider 校准

本轮真实运行纠正了一个重要判断：我原先把 DaoFocus 的“经济/境界/拖动/教程”等当成 project memory 召回目标，这是错的。你明确指出这些质量不行，真正想要的是长期可用的项目工作流 memory，或者全局通用 memory。

因此最新方向已经改成：

- 产品细节、数值阈值、具体 UI 入口、教程文案、存储 key、枚举名，只能作为 evidence 背景，不直接沉淀为 Memory Card。
- DaoFocus 这类项目应优先沉淀：TDD、Bun-only、只读审查、提交前测试/构建、review 后 merge、写入范围和协作边界。
- 当前项目应优先沉淀：小改快测大改重测、真实历史回归、设计前澄清、候选质量优先、人工审阅边界、Golden Set / evidence / feedback 回流治理。

对应代码改动：

- `prompts/induce.md`：把“产品规则一等记忆”改成“项目工作流规则一等记忆”，明确低层产品/实现细节应拒绝。
- `src/extract/pipeline.rs`：把 product-rule lane 调整为 project-workflow lane。
- `src/eval/qa.rs`：RunQA 不再要求 product_rule recall，改为检查 project_workflow / memory_governance / engineering_method。
- `src/extract/crystallize.rs`：新增 deterministic guard，拒绝境界阈值、STORAGE_KEY、拖动入口、教程、枚举等低层项目细节；同时保留 DaoFocus TDD/review/Bun 这类项目工作流。
- `src/extract/induce.rs`：batch provider 不再依赖 Anthropic structured output beta，改为 prompt JSON 契约 + 本地 parser，并把 batch token budget 提到 4096。
- `src/provider.rs`：HTTP provider 默认超时从 30s 提到 120s，并支持 `AGENT_KERNEL_HTTP_TIMEOUT_SECS` 覆盖，避免第三方 provider 长响应被提前断开。

最新真实运行结果：

```text
DaoFocus run: C:\DaoFocus\docs\runs\pipeline-20260518T044642.json
observations_in: 101
clusters_in: 33
induce_calls: 1
crystallize_accepted: 4
accepted cards:
- 实行测试驱动开发
- 仅使用 Bun 开发
- 以只读方式审查代码
- 提交前运行测试和构建
```

```text
Current project run: C:\Users\15893\Documents\New project\docs\runs\pipeline-20260518T045005.json
observations_in: 99
clusters_in: 22
crystallize_accepted: 5
accepted cards:
- 小改快测大改重测策略
- 重视真实历史回归
- 设计阶段先澄清再规划
- 生成规则列表时确保覆盖多类全局规则
- 保留审阅边界，先 review 再 merge
```

这两组产出比“灵石/境界/拖动”更接近真正可用的 memory。剩余问题是：第三方 Anthropic-compatible provider 仍偶发返回 no text，导致 batch fallback；RunQA 的 memory_governance 期望也偏粗，需要结合项目 profile 再校准。
