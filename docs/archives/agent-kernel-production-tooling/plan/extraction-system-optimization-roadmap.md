# 记忆卡片生成质量优化规划

> 日期：2026-05-18  
> 范围：聚焦提高 LLM 归纳候选与 Memory Card 成品文本质量。  
> 不包含：Review Inbox UI、Artifact Sync、Dashboard、Onboarding、GitHub Project 拆分、泛模块重构。

## 1. 目标

把当前提炼链路从“能产出候选”提升到“稳定产出可批准的高质量 Memory Card”：

```text
证据簇
  -> 高质量输入包
  -> LLM 归纳 InducedCandidate
  -> 候选质量判定
  -> 确定性或 LLM 辅助生成 Memory Card
  -> 本地文本质量校验
  -> 本地私有评估
```

核心目标：

- 卡片正文更像可执行的 agent memory，而不是机械摘要。
- title、body、brief、scope、kind、lifecycle 语义一致。
- LLM 不编造证据，也不把证据里的低层细节误写成长期规则。
- 对“好卡 / 差卡”的判断可测试、可回归、可对比。
- 允许确定性拼装继续保底，但为高价值候选提供受控的 LLM rewrite 路径。
- 真实对话数据只用于本地效果评估，不进入 Git/GitHub，也不进入生产链路。

## 1.1 隐私与数据边界

这个优化必须明确区分三类数据：

| 数据类型 | 用途 | 是否进入 Git | 是否进入生产链路 |
|:--|:--|:--|:--|
| 合成 fixture | 单元测试、CI、公开回归 | 可以 | 可以作为测试输入 |
| 脱敏 fixture | 本地或仓库级测试，确认边界行为 | 只有确认脱敏后才可以 | 可以作为测试输入 |
| 真实对话 / 本地历史 | 只用于本地效果评估和人工观察 | 禁止 | 禁止 |

硬规则：

- 不把 `C:\obsidian`、本地 Codex/Claude 历史、真实项目对话原文写入 Git。
- 不把真实对话样例上传到 GitHub Issue、PR、Project、Golden Set fixture。
- 本地真实历史评估只输出聚合指标和人工观察，不输出原文 quote。
- 链路优化不能依赖真实对话作为运行时参考；真实数据只能用于离线验证“效果好不好”。
- 如果需要保留失败样例进入仓库，必须改写成合成样例或彻底脱敏样例。

## 2. 当前问题

### 2.1 归纳候选质量问题

- `when/what/why` 容易变成三段模板话，而不是清楚描述触发条件、动作和边界。
- `why` 有时是 LLM 合理化出来的，不一定被 evidence 支撑。
- `scope` 会受项目名、文件名、产品词影响，导致通用方法论被误判为 project，或项目工作流被误判为 global。
- 同一证据簇如果包含低层细节和长期方法，LLM 仍可能抓错抽象层级。
- `confidence` 对质量帮助有限，不能直接说明“为什么值得批准”。

### 2.2 成品卡片质量问题

- 当前 Layer 5 是确定性拼装，稳定但容易机械：`当 X，Y；目标是 Z。`
- 好的候选被模板压扁后，可能不够自然、不够具体、不够像可长期执行的规则。
- `brief` 从 `why` 派生，容易抽象、重复或信息不足。
- 没有单独的“Memory Card 文本质量评分器”来衡量可执行性、清晰度、证据支撑和适用边界。

## 3. 设计原则

- **Evidence-bound generation**：LLM 只能从 evidence 归纳，不允许新增事实。
- **Abstraction control**：明确要求从低层事实抽象到工作流/偏好/约束，但不能抽象到空泛鸡汤。
- **Candidate before card**：先产出可审的候选，再生成最终卡，避免一步到位不可诊断。
- **Deterministic floor, LLM ceiling**：确定性拼装保证下限；LLM rewrite 只用于提升表达质量，并受 schema 与校验约束。
- **Quality rubric first**：先定义高质量卡片标准，再改 prompt 和代码。

## 4. 高质量 Memory Card 标准

### 4.1 好卡标准

一张好卡必须满足：

- **可触发**：明确什么时候使用，不是泛泛原则。
- **可执行**：agent 知道下一次应该做什么或避免什么。
- **有边界**：知道适用范围、例外或不要过度应用的地方。
- **有证据**：至少一条 evidence quote 能支撑核心动作。
- **抽象层级正确**：不沉淀临时实现细节，也不把具体经验抽成空话。
- **语言自然**：正文像人认可的长期规则，不像拼接模板。
- **字段一致**：title、body、brief、kind、scope、memory_tier、lifecycle 不互相打架。

### 4.2 坏卡类型

- 证据不足：quote 只能证明发生过，不足以证明偏好或规则。
- 抽象过度：把“这次修 bug 要跑测试”写成“所有任务都要完整回归”。
- 抽象不足：把“DaoFocus 境界阈值”写成长期 Memory Card。
- 模板味重：正文只是 `当 X，Y；目标是 Z`，但 X/Y/Z 都空泛。
- 字段错配：body 是 workflow，kind 却是 preference；项目规则却 scope=global。
- assistant 自嗨：证据主要来自 assistant 总结或建议，用户没有确认。

## 5. 目标生成链路

```text
1. BuildGenerationPacket
   输入：cluster messages + lane + source trust + existing similar cards
   输出：给 LLM 的高信号上下文包

2. InduceCandidate
   LLM 只做规则归纳：
   title / when / what / why / kind / scope / evidence_quotes / abstraction_level / boundary

3. CandidateQualityGate
   本地检查候选是否证据支撑、抽象层级正确、字段一致。

4. RenderCard
   默认确定性生成 body / brief / tags。

5. OptionalRewrite
   仅对高价值候选调用 LLM rewrite，改善自然度和可执行性。

6. CardQualityGate
   本地检查成品卡文本质量、字段一致性、重复和 evidence grounding。

7. LocalPrivateEval
   可选离线评估，只读取本机真实历史，输出聚合指标；不写入仓库。
```

## 5. 数据契约

### 5.1 GenerationPacket

```yaml
cluster_id: string
lane: project_workflow | memory_governance | engineering_method | collaboration_preference
recurrence: number
source_trust_summary:
  primary: user_direct | user_feedback | assistant_summary | tool_output | artifact
  has_user_confirmation: boolean
messages:
  - observation_id: string
    role: user | assistant | tool
    text: string
    created_at: string
nearby_existing_cards:
  - id: string
    title: string
    body: string
generation_goal: add | update | merge_candidate | reject_if_weak
```

### 5.2 InducedCandidate v2

```yaml
title: string
when: string
what: string
why: string | null
boundary: string
kind: preference | constraint | procedure | policy
scope: global | project
memory_tier: project_rule | cross_project_principle | collaboration_preference
abstraction_level: too_low | good | too_high
evidence_quotes:
  - observation_id: string
    text: string
support_level: strong | medium | weak
reject_reason: string | null
```

新增字段重点：

- `boundary`：解决正文没有边界的问题。
- `abstraction_level`：让 LLM 自报抽象层级，并由本地校验复核。
- `support_level`：区分 evidence 能强支撑还是只是弱相关。
- `memory_tier`：提前让 LLM 判定层级，后续和 scope 一起校验。

### 5.3 CardQualityReport

```yaml
passed: boolean
scores:
  clarity: 0-5
  actionability: 0-5
  evidence_grounding: 0-5
  abstraction_fit: 0-5
  boundary_quality: 0-5
  field_consistency: 0-5
failures:
  - weak_evidence
  - vague_action
  - missing_boundary
  - abstraction_too_low
  - abstraction_too_high
  - field_mismatch
  - brief_duplicates_body
  - unsupported_why
```

## 6. 生成质量优化任务

### G1. 重写 LLM 输入包

当前 prompt 直接给 cluster messages。下一步改成 `GenerationPacket`，把 lane、source trust、是否有用户确认、已有相似卡、生成目标一起给模型。

验收：

- LLM 能看见这条候选为什么进入生成阶段。
- prompt 明确“从证据中抽象一层，但不要抽象过头”。
- 有相似卡时，模型必须选择 add / update / merge_candidate / reject_if_weak。

### G2. 升级 induce prompt 到质量 rubric 驱动

prompt 从“接受标准/拒绝标准”升级为“质量 rubric + 输出前自检”。

新增要求：

- `when` 必须是可触发场景，不是“在项目中”这种空话。
- `what` 必须是下一次 agent 能执行的动作。
- `why` 可以为 null；证据不足时不能编。
- `boundary` 必填，说明不要在哪些场景过度应用。
- `abstraction_level` 必填，低层细节和空泛原则都要拒绝。
- 输出前自检：证据是否支撑 what、why、boundary。

验收：

- Golden Set 中好卡的 `when/what/boundary` 更具体。
- `unsupported_why` 明显下降。
- 低层产品细节不再被包装成项目规则。

### G3. 增加候选质量门

在 crystallize 之前增加 `CandidateQualityGate`，避免差候选进入卡片生成。

校验：

- `when` 是否过泛。
- `what` 是否含具体动作。
- `why` 是否被 evidence 支撑；不支撑则清空或拒绝。
- `boundary` 是否存在。
- `abstraction_level == good`。
- `support_level != weak`。
- `scope` 与 `memory_tier` 是否一致。

验收：

- 质量失败有明确 failure category。
- 差候选不会被确定性模板“洗白”为看似合格的卡。

### G4. 改造 Memory Card body 生成

当前 deterministic render 可以保留，但模板需要从单句式升级为按卡片形态渲染：

- `procedure`：步骤/流程型。
- `constraint`：禁止/必须型。
- `preference`：偏好/风格型。
- `policy`：分情况判断型。

正文结构建议：

```text
触发场景 + 应采取动作 + 边界/例外 + 目标
```

不是所有卡都必须写成 `当 X，Y；目标是 Z。`

验收：

- 好候选渲染出的 body 更自然。
- `boundary` 能进入正文。
- `brief` 不再只从 why 机械派生，而是总结用途。

### G5. 引入可选 LLM rewrite

只对高价值候选启用 LLM rewrite，提高最终卡片可读性。rewrite 不能改 evidence、scope、kind、memory_tier，只能改 title/body/brief 的表达。

启用条件：

- CandidateQualityGate 通过。
- evidence support strong 或 medium。
- 非 duplicate。
- 默认只在 provider 可用时运行；失败则回退 deterministic render。

rewrite prompt 必须包含：

- 原 candidate。
- deterministic draft card。
- evidence quotes。
- 禁止改动字段列表。
- 好卡 rubric。
- 输出 JSON schema。

验收：

- rewrite 后 CardQualityReport 分数必须高于 deterministic draft。
- 如果 rewrite 降低 evidence grounding 或 field consistency，自动回退 deterministic draft。

### G6. 建立卡片质量评分器

新增 `CardQualityReport`，用于比较 prompt 和 rewrite 前后质量。

建议先用确定性规则 + 小型 rubric 实现，不急着引入 LLM judge：

- clarity：是否有过泛词。
- actionability：是否包含明确动作动词。
- evidence_grounding：核心动作是否能由 quote 支撑。
- abstraction_fit：是否过低或过高。
- boundary_quality：是否有边界。
- field_consistency：kind/scope/memory_tier/lifecycle 是否一致。

验收：

- 每张生成卡都有质量报告。
- Golden Set 能比较 old prompt / new prompt / rewrite 的质量分。
- 低分项能回到具体失败类别。

## 7. Prompt 改造要点

### 7.1 Induce prompt

保留现有接受/拒绝标准，但新增：

- 好卡 rubric。
- 抽象层级说明。
- `boundary` 字段。
- `why` 可为空，禁止硬编目的。
- `support_level`。
- 输出前自检清单。

### 7.2 Rewrite prompt

新增可选 prompt，不替代 induce：

```text
你是 Memory Card 文本编辑器。
只能改 title/body/brief 的表达质量。
不得改 kind/scope/memory_tier/lifecycle/evidence。
不得新增 evidence 中没有的事实。
目标是让 card 更清楚、可执行、有边界、自然。
如果 deterministic draft 已经更好，返回 keep_original。
```

## 8. 评测与回归

### 8.1 数据分层

评测分两层：

```text
committable tests
  使用合成 / 脱敏 fixture
  目标：防止明显回归，可以跑 CI，可以进 Git

local private eval
  使用本机真实历史
  目标：评估本地效果，只输出指标和人工结论，不上传、不提交
```

Golden Set 在本文中只指 **可提交的合成/脱敏测试集**。真实对话评估不叫 Golden Set，统一叫 `local private eval`。

### 必备指标

- candidate accept precision
- evidence validity
- unsupported why rate
- missing boundary rate
- abstraction mismatch rate
- field mismatch rate
- deterministic card quality score
- rewrite win rate
- rewrite regression rate
- provider parse failure rate
- provider semantic failure rate
- approved-card similarity to local human edited version（只在本地私有评估里统计）

### Golden Set 分组

```text
good-cards/
  合成或脱敏的高质量 Memory Card

bad-generated-cards/
  合成或脱敏的坏卡：证据弱、抽象错、模板味重、字段错配

rewrite-pairs/
  合成或脱敏的生成前 / 编辑后 pair，用于评估 rewrite

provider-failures/
  malformed JSON、弱证据、语义错配、空 source ids

abstraction-boundary/
  低层细节 vs 合适工作流 vs 过度抽象
```

真实对话数据不能被复制到这些目录。若真实评估中发现失败模式，只能把模式改写为合成 case 后再提交。

### Local Private Eval

本地私有评估可以读取真实历史，但必须满足：

- 输出路径默认在 `.agent-kernel/private-evals/` 或 `docs/runs/private/`，并加入 `.gitignore`。
- 报告默认只保存聚合指标、失败类别计数、候选 id/hash，不保存原文。
- 如需人工查看原文，只在本地临时页面或终端中显示，不写入可提交文件。
- 生成的失败样例如果要进入 Golden Set，必须先人工改写成合成/脱敏版本。

### 验收线

- evidence validity >= 95%
- unsupported why rate <= 5%
- missing boundary rate <= 10%
- abstraction mismatch rate <= 10%
- rewrite regression rate <= 5%
- rewrite win rate >= 30% on high-value candidates
- 人工编辑后相似度逐轮提高

## 9. 实施顺序

### Phase 1: 定义质量 rubric 和失败分类

- 写 `CardQualityReport` 数据结构。
- 标注 20-30 张合成或脱敏 good/bad generated card 样例。
- 把失败分成 weak_evidence、unsupported_why、missing_boundary、abstraction_too_low、abstraction_too_high、field_mismatch、template_smell。

### Phase 2: 升级 InducedCandidate

- 增加 `boundary`、`support_level`、`abstraction_level`、`memory_tier`。
- 更新 induce prompt。
- 更新 parser 和本地校验。
- 补 provider semantic failure tests。

### Phase 3: 改造确定性 render

- body 模板纳入 boundary。
- policy/case-analysis 支持分情况表达。
- brief 从“why 复述”改为用途摘要。
- 对旧 Golden Set 跑回归。

### Phase 4: 可选 LLM rewrite

- 新增 rewrite prompt。
- 增加 rewrite 输出校验。
- A/B 比较 deterministic vs rewrite。
- rewrite 失败或降分自动回退。

### Phase 5: 人工编辑回流

- 本地记录人工编辑前后 pair，但默认写入私有评估目录。
- 可提交测试只使用合成或脱敏 rewrite pair。
- 每次 prompt 改动可以本地比较与人工编辑版的距离，但报告不得包含真实原文。

## 10. 与现有 GitHub Issue 的关系

当前已有规划里最相关的是：

- `#6 Capture real provider evidence validity`：承载 evidence grounding 与 provider semantic failure。
- `#15 Split extraction pipeline contracts`：承载 InducedCandidate v2、CardQualityReport、rewrite contract。
- `#7 Build a rejection-learning loop`：只承载脱敏/合成后的坏卡模式回流；真实对话拒绝记录不得上传。

如果新增 Issue，建议只新增一个更聚焦的：

```text
Improve generated Memory Card quality with candidate rubric and optional rewrite
```

## 11. 最小成功标准

一轮优化完成后，应该能回答：

- 这张卡为什么是好卡？
- 它的 when/what/why/boundary 是否都被证据支撑？
- 它有没有抽象过度或抽象不足？
- deterministic render 和 LLM rewrite 哪个更好，为什么？
- prompt 改动后 unsupported why、missing boundary、template smell 是否下降？
- 本地人工编辑后的好卡能否以脱敏/合成模式反哺下一轮生成？

如果这些问题都能用报告和测试回答，生成质量优化才算进入可持续状态。

## 12. 已落地的首个实现切片

2026-05-18 已先落地一个不读取真实数据、不会改变生产生成结果的本地质量评分基础：

- 在 `src/extract/quality.rs` 增加 `CardQualityInput`、`CardQualityReport`、`CardQualityScores`、`CardQualityFailure`。
- 增加 `quality_report_for_card` 与 `quality_report_for_crystallized_card`，后续可直接接入 deterministic render、rewrite A/B 或 eval report。
- 首批失败类别覆盖：`weak_evidence`、`vague_action`、`missing_boundary`、`abstraction_too_low`、`abstraction_too_high`、`field_mismatch`、`brief_duplicates_body`、`unsupported_why`、`template_smell`。
- `InducedCandidate` 已兼容新增 `boundary`、`memory_tier`、`abstraction_level`、`support_level`，旧 provider JSON 缺这些字段时仍可解析。
- `prompts/induce.md` 已加入质量 rubric、自检、边界字段、支撑强度和抽象层级要求。
- deterministic render 已能在候选带 `boundary` 时写入正文边界句。
- 测试只使用合成隐私边界样例，不包含真实对话原文。
- 未新增 GitHub Issue；后续实现继续复用 `#6`、`#7`、`#15`，如需单独追踪再创建一个聚焦 issue。

## 13. 质量复盘：过滤“生成流程提醒式”坏卡

2026-05-18 的人工抽样发现一类不该进入 Memory Card 的坏卡：它描述的是当前优化链路的验收动作，而不是长期可复用的用户偏好。例如合成同型句“修改卡片改写器时，必须抽样审阅最终卡片，不要只看质量分。”这类话不能沉淀为 Memory Card，因为它只是在约束本地评估方式；如果进入记忆，会把一次开发流程提醒误编译成未来所有任务的 agent 行为。

本轮优化：

- 在 `quality_gate` 和 `memory_gate` 增加生成质量验收语识别：同时命中“生成/卡片/改写器”等生成表面、“抽样/最终卡片/质量分/指标”等评估表面，以及“必须/不要/should”等指令语气时，标记为 `memory-pipeline-meta` 并过滤。
- 在 `CardQualityFailure` 增加 `PipelineMeta`，让成品卡质量评分也能拒绝这类卡，避免只依赖前置过滤。
- 回归样例使用合成同型句，不使用真实对话原文；真实失败模式只被抽象成可提交 fixture。
- 人工抽样继续检查最终候选文本：真实历史回归、人工审阅边界各自应收敛成 1 张可审卡，而不是多张近义卡。
- 在 `dedupe_candidates` 增加窄语义域合并，只针对明确主题如 `real-history`、`review-boundary`、`candidate-quality`、`self-correction` 跨 scope/tier 合并，降低最终候选重复污染。

当前验证结论：

- 合成生成质量验收语 dry-run 输出 `No drafts created`。
- 抽样“真实历史回归”从 4 张候选收敛为 1 张；“审阅边界”从 2 张候选收敛为 1 张。
- Golden Set 指标保持 positive recall 100%、negative precision 100%、one-off false positive 0%、card quality baseline 100%。
- `duplicate cluster risk` 仍为 27%，但它衡量的是输入聚类拆分，不等同于最终候选重复；后续应新增 final draft duplicate risk 或 candidate merge rate，避免误读。
