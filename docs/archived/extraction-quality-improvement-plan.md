# 记忆卡片提取质量改进方案

> 适用范围：`agent-kernel` Rust 后端从 candidate 进入到最终 MemoryCard 写盘的完整质量管控链路。
>
> 与本文档姊妹的性能向方案在 [extraction-pipeline-optimization-plan.md](./extraction-pipeline-optimization-plan.md)。性能方案管"快"，本方案管"好"。两者独立交付，互不阻塞。
>
> 角色约定：本文档由 Claude 起草，Codex / 人工实施时参考。Claude 不写实现代码。

---

## 一、现状证据：真实样本暴露的质量缺陷

读取 `.agent-kernel/memory-cards/.archive/2026-05-08/` 下两张已批准卡片 + `.agent-kernel/drafts/artifact/claude-code/sha256/170d7.yml` 当前 draft，发现 5 类系统性问题。

### 缺陷 1：关键字段三处不一致（"一团浆糊"的源头）

`.archive/2026-05-08/保留人工审阅边界.yml`：

| 字段路径 | 值 |
|---|---|
| 顶层 `activation` | `always-on` |
| `extraction.classification.activation` | `skill` |
| `extraction.classification.tags` | `activation:skill` |
| 顶层 `tags` | `activation:skill` |
| `extraction.suggested_action.compile_enabled` | `false`（暗示 SKILL，不是 always-on）|

同一张卡的"激活方式"在 5 个地方各说各话。下游 `build.rs` 读哪个就编出哪种产物，等于结果不可预测。这正是上一轮 verify 命令要解的问题，但**根源不在校验，而在数据没有单一真实来源**。

### 缺陷 2：溯源是空的或循环的

两张已批准卡片都是 `source_observations: []`，意味着无法回溯到任何真实对话。`evidence_span.quote` 等于 `body` 自己（不是从原文抄证据，是 LLM 把自己输出当 quote 自我合法化）。

`memory_card_verify.rs` 已经能检测，但**当前是事后审计，不是入库前门禁**。

### 缺陷 3：分类理由是模板套娃英文

`extraction.classification.rationale: "Classified as procedure for workflow_skill with medium hardness."` 这种句子是变量拼接而来，看起来"专业"但没真分析。同样地 `extraction.reason: "High-value durable procedure signal with reusable project impact."` 也是固定句式。

### 缺陷 4：brief 重复 body

draft `170d7.yml` 的 brief 写的是 `"这条草稿记录了 'Manual artifact changes for claude-code'：### 保留人工审阅边界\n\n当结果涉及候选筛选..."` —— 把 body 整段抄一遍加个前缀。这种 brief 对人和模型都没有信息增量。

### 缺陷 5：质量门禁是"打地鼠"

`src/extract/quality_gate.rs` 长 848 行，模式如下：

- 17+ 个 `looks_like_*` / `contains_*` 启发函数（`looks_like_meta_discussion` / `looks_like_temporary_task_constraint` / `looks_like_package_manager_preference` ...）
- 每个针对一类历史失败样本写关键词黑名单
- 测试函数 30+，名字都是 `rejects_*_artifacts`

这是反应式开发：发现一种坏样本就加一个 fn。**新失败模式无法预防**，而且单文件已经接近 1000 行红线。

---

## 二、何为高质量记忆卡片（标准定义）

综合 Anthropic SKILL.md 规范、Mem0 ADD/UPDATE/DELETE/NOOP 模式、LangMem 三类记忆、Letta 的可编辑记忆块、shareAI-lab/learn-claude-code 的两层 Progressive Disclosure 思维，定义本项目的"高质量"为 8 个维度：

### Q1. Provenance（可溯源）

- `extraction.source_observations` 至少 1 个真实存在的 obs id
- `extraction.evidence_span.quote` 在某条 source_observation 的 body 中**字面包含**
- `extraction.evidence_span.role` 必须是 `user` 或 `assistant`，禁止 `ai-synthesis` 这种自我合成

### Q2. Atomicity（原子性）

- 一张卡只表达一条规则。表现为：body 不含 `\n\n`（段落分隔），不含编号列表（"1." "2."），不含"另外""同时"等并列连接
- 长度上限：body ≤ 200 字（中文）或 200 词（英文）

### Q3. Durability（持久性）

- 排除一次性请求："请帮我做 X"、"修一下这个 bug"
- 排除会随时间过期的事实：含具体日期、版本号、人名职位
- 必须能在未来 6 个月内仍然适用

### Q4. Triggerability（可触发）

- body 必须以"当 X 时"或"在 X 场景下"开头，明确 WHEN
- 或者使用 `trigger_description` 字段单独说明 WHEN
- 禁止纯陈述句无触发条件

### Q5. Actionability（可执行）

- body 必须有一个动词宾语短句说明 DO 什么
- 禁止"应该注意 X""请考虑 Y"这种软建议
- 必须能转译成 agent 的具体行为

### Q6. Reusability（可复用）

- `scope: global` 卡必须不依赖具体项目代码或文件路径
- `scope: project` 卡必须明确说出这是哪个项目特有
- 不能"两边都沾"

### Q7. Specificity（具体性）

- 禁止泛泛的最佳实践重述（"代码要写得清晰"、"测试要全面"）
- 必须包含本场景特有的约束、阈值、对象或边界

### Q8. Consistency（一致性）

- `kind` / `scope` / `activation` 三个字段从单一真实来源（`extraction.classification`）派生
- `tags` 中的 `activation:*` `kind:*` 必须与顶层字段一致
- `compile_enabled` 与 `activation` 的映射关系固定（always-on/path-glob → true，skill/manual → false）

### 反例对照

| 反例 | 失败维度 | 改进 |
|---|---|---|
| `代码要写得清晰` | Q5 Q7 | 改成 `当函数超过 50 行时，把内层逻辑抽到独立私有函数；目标是单函数可在一屏内读完。` |
| `请帮我修这个登录 bug` | Q3 Q4 | 直接拒绝，一次性请求 |
| `每次都要测试` | Q5 Q7 | 改成 `当改动涉及 src/extract/ 下的 gate 或 classify 时，必须在 .agent-kernel/observations/ 中找出至少 3 条真实历史 fixture 跑回归。` |

---

## 三、工程设计：Quality 子系统模块化架构

把当前散落在 `quality_gate.rs` / `classify.rs` / `refine.rs` / `memory_card_verify.rs` 的质量逻辑收到一个新的 `src/quality/` 模块，遵循单一真实来源、强 schema、stage 化处理三原则。

### 目标目录结构

```
src/quality/
├── mod.rs               # 公开入口，定义 QualityPipeline trait
├── schema.rs            # MemoryCard 的强 schema 定义与 invariant 校验
├── dimensions.rs        # 8 个质量维度的常量与权重
├── gate.rs              # 多 stage 质量门（替代 quality_gate.rs）
├── rubric.rs            # LLM rubric 评分（按 8 维度）
├── normalizer.rs        # 规范化转写流程
├── golden_template.rs   # 金标准模板（title/body/brief 形态）
├── consistency.rs       # 字段间 invariant 检查（合并 verify 逻辑）
└── tests/
    ├── golden_set.rs    # 黄金集回归测试
    └── reject_cases.rs  # 历史失败样本集合
```

### 与现有模块的关系

| 现有 | 处置 |
|---|---|
| `src/extract/quality_gate.rs` (848 行) | 拆分：通用 `looks_like_*` 移到 `quality/gate.rs` 做 stage-2，其他逻辑废弃 |
| `src/extract/classify.rs` (508 行) | 保留分类逻辑本身，但所有"质量过滤"代码移走 |
| `src/extract/refine.rs` (279 行) | 保留 LLM 重写调用骨架，"是否拒绝"决策权交给 `quality/gate.rs` |
| `src/memory_card_verify.rs` (383 行) | 把核心校验逻辑移到 `quality/consistency.rs` 复用，verify 命令改为薄包装 |

收益：`extract.rs` 不再 942 行膨胀，质量逻辑有唯一入口便于演进。

---

## 四、数据契约：MemoryCard 强 Schema 与单一真实来源

### 单一真实来源原则

`kind` / `scope` / `activation` / `compile_enabled` / 主要 `tags` **只能从一处派生**：`extraction.classification`。其他位置都是从这里"投影"出来的视图，不是独立写入。

#### 计算规则（必须固化在代码里）

```text
top.activation     := normalize(classification.activation)
top.kind           := classification.signal      （或固定 mapping 表）
top.scope          := classification.scope_hint  （或推导）
suggested_action.compile_enabled := match top.activation {
    "always-on" | "path-glob" => true,
    "skill" | "manual"        => false,
    _                          => false
}
top.tags ⊇ {
    format!("activation:{}", top.activation),
    format!("kind:{}", top.kind),
    format!("scope:{}", top.scope),
}
```

任何地方读到 `top.activation` 与 `classification.activation` 不一致 → 直接拒绝，不"修补"。

### Schema 必填字段清单

```yaml
# 顶层（11 必填）
schema_version: u32              # 必须 = 当前版本
id: String                       # 格式 <scope>:<slug>
title: String                    # 8-30 字（中文按字符算）
kind: enum                       # 派生自 classification
scope: enum                      # 派生自 classification
body: String                     # 50-200 字
brief: String                    # 15-60 字，必须不与 body 任何子串重合
tags: Vec<String>                # 至少含 activation:* kind:* scope:* 三类
language: String                 # 必填，禁止留空
activation: enum                 # 派生自 classification
created_at / updated_at: ISO8601

# extraction（10 必填，去掉了"可选"）
extraction.origin: enum          # ai-synthesis / heuristic / manual
extraction.matched_signal: String
extraction.reason: String        # 禁止固定句式（见后述检测）
extraction.source_observations: Vec<String>  # 长度 ≥ 1
extraction.evidence_span.role: enum {user, assistant}  # 禁止 ai-synthesis
extraction.evidence_span.quote: String        # 8-200 字，必须在 source_observation body 中字面出现
extraction.evidence_span.observation_id: String  # 必填，必须在 source_observations 中
extraction.classification.signal: enum
extraction.classification.activation: enum
extraction.classification.rationale: String  # 禁止模板拼接（见后述）
```

### Invariant 检查清单

固化为 `quality/consistency.rs::check_invariants(card) -> Result<(), Vec<Violation>>`：

1. `top.activation == normalize(classification.activation)`
2. `quote` 在某 `source_observations` 的真实 body 中字面包含
3. `evidence_span.observation_id` ∈ `source_observations`
4. `tags` 集合 ⊇ `{activation:X, kind:Y, scope:Z}`
5. `brief` 与 `body` 的 token-level Jaccard 相似度 < 0.5（防重复抄写）
6. `body` 字符数在 [50, 200]，`title` 在 [8, 30]，`brief` 在 [15, 60]
7. `body` 不含 `\n\n` 也不以 `1.` / `- ` / `* ` 开头（保原子性）
8. `classification.rationale` 不匹配模板正则 `^Classified as \w+ for \w+ with \w+ hardness\.?$`
9. `extraction.reason` 不匹配模板正则 `^High-value durable \w+ signal with reusable project impact\.?$`

---

## 五、质量门禁：5 阶段 Filter Pipeline

替代当前打地鼠式 `quality_gate.rs`。每张候选必须依次通过 5 个 stage，任意阶段拒绝即丢弃，**不进入转写**。

### Stage 0：Structural Validation（确定性检查）

- 检查 schema 必填字段都在
- 检查所有 invariant（上述 9 条）
- 失败 → reject reason `structural:<具体违反的 invariant>`
- 不调 LLM，毫秒级

### Stage 1：Provenance Validation（溯源检查）

- 在 `.agent-kernel/observations/` 中查找 `evidence_span.observation_id`
- 校验 `quote` 是否在该 observation 的 body 中
- 校验 `evidence_span.role` 不是 `ai-synthesis`
- 失败 → reject reason `provenance:<missing-obs|quote-not-found|fake-role>`
- 不调 LLM

### Stage 2：Heuristic Filter（启发式黑名单）

- 保留现有 `looks_like_*` 中真正稳定的模式：
  - `looks_like_meta_discussion`（讨论实现进度而非可复用规则）
  - `looks_like_temporary_task_constraint`（一次性任务边界）
  - `looks_like_generated_instruction_artifact`（从产物文件反向倒灌）
  - `looks_like_package_manager_preference`（项目政策已排除）
- 弃用容易误伤的：基于关键字"以后""每次"判可复用性
- 失败 → reject reason `heuristic:<规则名>`

### Stage 3：LLM Rubric Scoring（按 8 维度评分）

- 用 Haiku 4.5（性能向方案 T2.1 落地后的便宜模型路由）
- 输入：candidate 的 title/body/brief + source observation 摘要
- 输出强 JSON：
  ```json
  {
    "scores": {
      "provenance": 0..1,
      "atomicity": 0..1,
      "durability": 0..1,
      "triggerability": 0..1,
      "actionability": 0..1,
      "reusability": 0..1,
      "specificity": 0..1,
      "consistency": 0..1
    },
    "weakest_dimension": "specificity",
    "improvement_hint": "body 缺少具体阈值或对象"
  }
  ```
- 加权和 < 0.65 → reject reason `rubric:weak:<dimension>`
- 任意单维度 < 0.4 → reject reason `rubric:floor:<dimension>`

### Stage 4：Duplicate / Conflict Detection

- 用 Plan 1 T1.1 落地的 batch embedding 与现有 cards 全量 cosine
- ≥ 0.95 → reject `duplicate`（已经存在等价卡）
- 0.7 ≤ x < 0.95 → 标记为 `merge_candidate`，进入 Mem0 风格 ADD/UPDATE/DELETE/NOOP 决策（用 LLM 仲裁）
- < 0.7 → 通过

### Reject 原因可观测

每条 reject 必须落 `.agent-kernel/audit-log.jsonl` 一条结构化记录：

```json
{
  "ts": "2026-05-09T...",
  "candidate_id": "...",
  "reject_stage": 0..4,
  "reject_reason": "structural:quote-not-found",
  "candidate_preview": "...",  // 前 80 字
  "scores": {...}              // 若 stage 3 触发
}
```

UI Draft Inbox 可以读 audit-log 反向 trace："为什么这条被丢了"，方便 prompt 调优。

---

## 六、规范化转写：金标准模板 + 3 步流程

通过 5 stage 门禁后才进入转写。转写不是"修补缺陷"，而是"按金标准重写"。

### 金标准模板

#### title 模板

- 8-30 字简体中文（或 8-15 词英文）
- 动宾结构：`<动词>+<宾语>`，例 `保留人工审阅边界`、`优先用真实历史验证提炼逻辑`
- 禁止：疑问句、命令句、英文混搭、emoji、标点
- 禁止：以"如何"/"应该"/"请"开头

#### body 模板（强结构）

```text
当 <触发条件> 时，<动作>；目标是 <为什么>。
```

或

```text
在 <场景> 下，<约束>。<例外>。
```

字数：50-200 字。

例（合格）：
> 当评估提炼质量或修改提炼逻辑时，优先使用真实历史会话做回归验证；不要只依赖静态样例。

例（不合格，太短无目标）：
> 用真实历史。

例（不合格，多事项混在一起）：
> 当评估提炼质量时用真实历史；同时要写测试；另外注意性能。

#### brief 模板

- 15-60 字，一句话说"这卡是用来干嘛的"
- 必须以"用于"开头
- 禁止重复 body 任何片段

例：
> 用于让提炼规则接受真实历史回归检验。

#### tags 模板

至少 3 个、至多 8 个，必含：

- `activation:<value>`（来自 classification）
- `kind:<value>`
- `scope:<value>`

可选：`domain:*`、`hardness:*`、特定领域 tag（`memory-extraction` / `regression` 等）。

禁止：泛泛的 `important`、`high-value`、空格、中文标点。

### 转写 3 步流程

#### Step A：Selection Gate（最后一次确认要不要转写）

- 已通过 5 stage gate，但额外检查 LLM rubric 总分是否 ≥ 0.75
- 0.65 ≤ x < 0.75 之间 → 不转写但保留为 draft（让人工决定）
- ≥ 0.75 → 进入 Step B

#### Step B：Normalize Rewrite

- 用 Sonnet 4.6（性能方案 T2.1 之后）
- prompt：金标准模板 + 当前 candidate + improvement_hint（来自 rubric）
- 强 JSON schema：返回 `{title, body, brief, tags}` 四字段
- 输出限制：max_tokens = 512（足够）

#### Step C：Post-Rewrite Verification

把转写结果再过一遍：

1. 重新跑 Stage 0（structural）
2. 重新跑 Stage 1（provenance，转写后 source_observations / evidence_span 不能丢）
3. 重新计算 brief 与 body 的 Jaccard < 0.5
4. 任一失败 → 整张卡 reject，不写盘，不"再修"一次

**关键：失败优于伪装**。如果 LLM 转写不达标就丢弃，不能再调一次让它"再试试"。两次就放弃。

---

## 七、测试基线：黄金集 + 失败案例库

### 黄金集（Golden Set）

人工标注 50-100 张真实卡，分两类：

- `golden_set/positive/*.yml`：人类认为高质量的真实卡（30-50 张）
- `golden_set/negative/*.yml`：必须被拒绝的真实坏样本（30-50 张），按 reject 原因分子目录

存储在 `tests/golden_set/`（已 gitignore），不进公开仓库。

### 回归指标

每次质量代码改动后，跑 `cargo test --test golden_set_regression` 输出：

| 指标 | 目标 |
|---|---|
| Positive recall | ≥ 0.90（90% 高质量卡能通过） |
| Negative precision | ≥ 0.95（95% 坏样本被准确拒绝） |
| Per-stage attribution | 每条 reject 是哪个 stage 触发的清晰可见 |
| Rewrite acceptance | 转写后人工再审通过率 ≥ 0.85 |

### 失败案例库累积

每次发现新的失败模式：

1. 用真实样本（脱敏）加入 `golden_set/negative/<reason>/`
2. 写一条对应的 stage 规则或 rubric prompt 调整
3. 跑回归确认 negative precision 不降
4. 不加进单元测试的负样本不算"修了"

---

## 八、独立 Plan 列表

把上述设计拆成 5 个互相独立、可串行交付的 Plan。每个 Plan 完成后单独可上线、单独可回滚。

### Plan A：Schema 强化与 Invariant 落地（最重要的前置）

- 涉及：`src/quality/schema.rs`（新）、`src/quality/consistency.rs`（新）、`src/memory_card.rs`、`src/candidate.rs`
- 任务：
  1. 定义强 schema struct，所有"可选字段"在质量路径上变必填（`extraction.source_observations` 长度 ≥ 1 等）
  2. 实现 `check_invariants(card) -> Result<(), Vec<Violation>>`，9 条 invariant 全部覆盖
  3. 计算规则：`activation` 等派生字段从 `classification` 单向投影，不再各处独立写入
  4. 把 `memory_card_verify.rs` 的核心逻辑迁过来共用
  5. 黄金集 negative：构造现有 archive 中那两张 activation 不一致的卡，断言被 invariant 拒绝
- 风险：旧卡 violation 太多。缓解：写一次 migration `cargo run -- memory-card migrate-schema`，把现有 archive 里能修的字段对齐，无法修复的标记 `legacy: true` 跳过校验
- 完成标准：所有新写入的卡都过 9 条 invariant；旧卡有 migration 路径

### Plan B：5 Stage Quality Gate Pipeline

- 前置：Plan A
- 涉及：`src/quality/gate.rs`（新）、`src/extract/quality_gate.rs`（拆解）、`src/extract.rs`（接入新 gate）
- 任务：
  1. 实现 5 stage trait `QualityStage::evaluate(candidate) -> StageDecision`
  2. Stage 0/1（结构 + 溯源）从 Plan A 直接复用
  3. Stage 2 把现有 `quality_gate.rs` 的稳定启发式迁移过来，去掉容易误伤的
  4. Stage 3 LLM rubric：先用 prompt-only 实现，对接性能方案 T2.1 的 Haiku 路由（Plan A/B 不强依赖性能方案，但落地后能省钱）
  5. Stage 4 用现有 `SemanticDeduper`（性能方案 T1.1 的 batch 接口若已落地优先用）
  6. 接入点：`extract::extract_high_value_text_to_drafts` 在生成 candidate 后改调 `QualityPipeline::run()`
  7. 每个 reject 写 audit-log 结构化记录
- 验证：黄金集 positive recall ≥ 0.90, negative precision ≥ 0.95
- 风险：Stage 3 LLM 调用成本上升。缓解：rubric 是必要成本，用 Haiku 已经很便宜；且只对通过 Stage 0-2 的候选才调，不浪费

### Plan C：金标准模板与规范化转写

- 前置：Plan B（必须通过 gate 才进 normalize）
- 涉及：`src/quality/golden_template.rs`（新）、`src/quality/normalizer.rs`（新）、`src/extract/refine.rs`（重构调用）
- 任务：
  1. 把金标准模板 4 段（title/body/brief/tags）固化成 prompt 模板，外置到 `prompts/normalize.md`
  2. 实现 `normalize_rewrite(candidate, rubric_result) -> Result<MemoryCard>`
  3. 强 JSON schema 校验返回结果
  4. Step C 后置 verification：重跑 Stage 0/1，brief vs body 相似度 < 0.5
  5. 失败两次直接放弃（不能无限重试）
  6. 现有 `extract/refine.rs` 中的 deterministic fallback 改为：normalize 失败时不留兜底卡，直接 reject
- 验证：rewrite 后人工抽样接受率 ≥ 0.85
- 风险：LLM 输出不稳定。缓解：max_tokens=512 + 强 JSON schema + 重试上限 = 2

### Plan D：黄金集与失败案例库

- 前置：Plan A 落地（schema 稳定后才能写 fixture）
- 涉及：`tests/golden_set/`（新）、`tests/golden_set_regression.rs`（新）
- 任务：
  1. 人工标注 30 张高质量 + 30 张失败样本（用现有 `.archive/` 与 audit-log 当来源）
  2. 失败样本按 reject 原因分子目录：`structural/` `provenance/` `heuristic/` `rubric/` `duplicate/`
  3. 写 `golden_set_regression` 集成测试：跑全集，输出 4 个指标
  4. 在 README / CONTRIBUTING.md 加入"添加新失败案例"章节，固化 SOP
- 验证：跑测试有完整指标输出
- 风险：人工标注成本。缓解：先 30+30 起步，用真实 archive 而不是手编

### Plan E：Audit-log 与可观测性

- 前置：Plan B
- 涉及：`src/quality/audit.rs`（新）、`src/observation.rs`（埋点）、CLI `quality-report` 子命令
- 任务：
  1. 每次 reject 写一条 jsonl 到 `.agent-kernel/audit-log.jsonl`（已有此文件，只是格式扩展）
  2. 字段固化：`ts / candidate_id / reject_stage / reject_reason / scores / candidate_preview`
  3. CLI 子命令 `cargo run -- quality-report --since 7d`：聚合最近 7 天的 reject 分布、各维度得分分布、rubric 准确率
  4. UI Draft Inbox 增加"最近被拒原因"面板
- 收益：所有质量改动都有数字支撑，不再凭感觉

---

## 九、不要做的事

- **不要再加更多 `looks_like_*` 启发函数**：当前已有 17+ 个，再加只会让 `quality_gate.rs` 更不可维护。新失败模式应该走 rubric prompt 调优或加入黄金集回归。
- **不要让 LLM 自己生成 evidence**：`evidence_span.role: ai-synthesis` 是错误源头。证据必须从真实 observation body 来。
- **不要给 brief 加"这条草稿记录了..."这种前缀**：让 LLM 直接重写，不要套娃。
- **不要把转写结果"再修一次"**：失败两次就 reject。LLM 反复改不出合格卡，说明候选本身不够格。
- **不要把质量门禁改成"软警告"**：只要触发硬约束（invariant、provenance、rubric floor）就拒。允许"打折通过"会让黄金集回归立刻劣化。
- **不要按时间衰减**：本项目是手动审查驱动的，不需要 Mem0/Letta 那种自动遗忘。卡的生命周期由人决定。

---

## 十、与性能方案的协作矩阵

| 性能方案任务 | 对质量方案的支撑 |
|---|---|
| Plan 0 metrics | 给 quality 提供 baseline 指标（rubric 调用量、reject 率） |
| T1.1 batch embedding | Stage 4 dedup 必需 |
| T1.3 prompt cache 扩边界 | Stage 3 rubric 与 Step B normalize 都要复用 system prompt，cache 命中后省 90% |
| T2.1 多模型路由 | Stage 3 用 Haiku，Step B 用 Sonnet，分开后成本可控 |
| T2.3 强结构化输出 | Stage 3 rubric 与 Step B normalize 的 JSON schema 强制 |
| T3.4 observation 级缓存 | 没改的 observation 不重算 rubric |

性能方案 Plan 0 + Plan 1 + Plan 2 是质量方案的"基础设施"。建议两条线交错推进：性能方案 Plan 0 先行（建立 metrics），然后质量方案 Plan A（schema），然后性能方案 Plan 1（embedding cache + batch），然后质量方案 Plan B（5 stage gate）。

---

## 参考资料

- [Anthropic Skill Authoring Best Practices](https://platform.claude.com/docs/en/agents-and-tools/agent-skills/best-practices)
- [Claude Code Skills Complete Guide](https://duet.so/guides/claude-code-skills-complete-guide)
- [shareAI-lab/learn-claude-code: s05 Skill Loading 两层加载设计](https://github.com/shareAI-lab/learn-claude-code/blob/main/docs/zh/s05-skill-loading.md)
- [shareAI-lab/learn-claude-code: agent-builder skill](https://github.com/shareAI-lab/learn-claude-code/blob/main/skills/agent-builder/SKILL.md)
- [Mem0 ADD/UPDATE/DELETE/NOOP 模式](https://mem0.ai/blog/graph-memory-solutions-ai-agents)
- [LangMem 多类型记忆（semantic/episodic/procedural）](https://atlan.com/know/best-ai-agent-memory-frameworks-2026/)
- [Letta（前 MemGPT）的可编辑记忆块](https://atlan.com/know/best-ai-agent-memory-frameworks-2026/)
