# 提炼质量自审与补强

> 这是 [extraction-core-design.md](./extraction-core-design.md) 的质量自审报告与补强方案。
>
> 触发原因：用户对原方案的"高质量是必须项"提出质疑。我做完自审后，承认原方案有 5 处质量缺口，需要 5 项补强。补强后能给出明确的质量保证。
>
> 本文档不推翻原设计的三概念骨架（Signal / Witness / Triplet），只在骨架内打补丁。

---

## 一、自审：原方案的 5 个真实缺口

诚实地说，原方案在"高质量"维度上有 5 个我没解决的问题。

### 缺口 1：Witness 子串匹配太脆

原方案：`observation.body.contains(quote)` 字面子串匹配。

**真实问题**：

- 中文用户原话常含全角/半角、中英标点混排：`"以后用 Axios，不要 fetch"` vs `"以后用Axios,不要fetch"` 同一句话，子串匹配过不去。
- 多空白字符（中英混排）。
- 用户原话口语化（"咱们以后就用 axios 吧"），LLM 提的 quote 是规范化版本（"以后用 Axios"），字面匹配失败。

**后果**：合格的 candidate 在 Stage 2 被误杀，召回率低。

### 缺口 2：Harvest 拒绝门槛太低

原方案：LLM 输出 `future_oriented_reason` 不为空就放行。

**真实问题**：LLM 编 reason 几乎零成本。"明确指向未来评估场景"这种句子 Haiku 可以无穷生成，根本不构成门槛。

**后果**：Stage 1 召回过宽，把负担甩给 Stage 2/3 拒。但 Stage 2 是字面匹配（被缺口 1 干扰），Stage 3 又只校验形态。**整条链路没有"是否真的是未来规则"的硬判断点。**

### 缺口 3：Triplet 句式过于刚性

原方案：body 必须是 `当 X 时，Y；目标是 Z`。

**真实问题**：不是所有合格规则都能塞进这一种句式。

- 配置类偏好：`项目内统一用 Axios，禁止 fetch` —— 没有 When，硬加显得拗口。
- 流程类规则：`改 prompt 时先跑黄金集，再 commit，再发 PR` —— 三步流程被压成一个 What。
- 例外条款：`默认用 Axios；若调用方在 Node 进程中改用 undici` —— 有主从两条规则。

强行单一句式 → LLM 要么不合格（被 Stage 3 拒），要么硬塞（语义损失）。**召回率会显著下降。**

### 缺口 4：没有 Triplet 与 Witness 的语义一致性校验

原方案：Triplet 通过 schema 就放行；只验"形状"，不验"内容是否能从 Witness 推出"。

**真实问题**：LLM 完全可能从 quote `"以后前端用 Axios"` 推出 triplet `当后端调用外部 API 时，使用 reqwest`。schema 通过、quote 真实、卡却**胡说八道**。这是 hallucination 的典型形态，在 quote 短、上下文模糊时经常发生。

**后果**：质量上限是 LLM 改写时的语义理解准确率，没有第二道关卡。

### 缺口 5：缺少冲突与重复处理

原方案：Stage 4 提了一句"cosine ≥ 0.95 拒绝"，没展开。

**真实问题**：

- 新卡与现有卡**矛盾**怎么办？（`"用 Axios"` vs 现有 `"用 fetch"`）
- 同一规则被多次提炼，每次措辞略不同，cosine 都 < 0.95（比如 0.85）怎么办？
- recurrence（用户三次纠正同一件事）应该比单次提及置信度更高，但当前没有这个信号。

**后果**：库会逐渐积累矛盾卡 + 准重复卡。

---

## 二、行业怎么做（参照而不照搬）

我搜索了 2026 年最新工业实践与论文，关键发现：

### Mem0（最新版本）

- **Single-pass extraction**：只调一次 LLM 做 extract，不再 UPDATE/DELETE 多轮（避免 sycophantic reflection 与 self-consistency trap）
- **Hybrid retrieval**：semantic + keyword + entity 三路召回（不只是 cosine）
- **Agent-generated facts 一等公民**：assistant 的发言也算证据
- **质量探针公开**：stale facts / unresolved contradictions / scope leakage / retrieval drift 四个具体探针，每个 > 0.7 才算健康

### KARMA / 多 Agent 验证

每个 candidate triplet 由 LLM verifier 在入库前评估。验证不通过的整条丢弃。但作者强调：**verifier 必须用 closed-world prompt**（"只用提供的证据，不要外部知识"），否则 verifier 会自己幻觉。

### Generator-Critic 模式

两次 LLM 调用：generator 出结果，critic 单独评估。Critic 用专属评估准则提示，**不是开放式"找问题"**。

### LLM-as-Judge 的硬限制（重要）

研究表明：LLM 作为 judge 时，识别一致内容 > 95% 准确，但识别**不一致内容**只有 30-60% 召回。意味着：

> **Critic 倾向于通过，不倾向于拒绝。**

所以 critic 不能是质量唯一防线，必须搭配确定性检查。

### 关键警告：Self-consistency trap

EMNLP 2025 研究：LLM 反复 reflection 可能 sycophantic（自我认同），多轮反思反而把对的改错。**多次重写不增加质量，单次精修 + 单次校验比多轮 reflection 更稳**。

---

## 三、5 项补强（不推翻原方案，只补漏）

每项补强对应一个缺口。每项的补强成本和质量收益独立可验证。

### 补强 1：Witness 改为"规范化子串"匹配（解缺口 1）

不引入语义近似，仍是确定性。规范化包括：

```text
- 全角/半角统一（中文标点 → 半角；半角字母 → 保留）
- 多空白合并为单空白
- 行首尾空白裁剪
- 大小写不归一（变量名/技术名词大小写有意义）
```

匹配规则升级为：

```rust
fn find_witness(quote, observation) -> Result<Witness> {
    let quote_n = normalize(quote);
    let body_n = normalize(&observation.body);
    if !body_n.contains(&quote_n) {
        return Err(QuoteNotFound);
    }
    // 取原文中对应的非规范化片段作为最终 quote（保真度）
    let original_span = locate_original_span(&observation.body, &quote_n)?;
    Ok(Witness { quote: original_span, ... })
}
```

**保真原则**：返回的 quote 是**原文未归一化的片段**，不是规范化后的版本。这样落盘的 evidence 还是原话。

**成本**：~30 行 Rust + 单元测试（~20 case 覆盖各种标点/空白组合）。一天工作量。

**收益**：召回率从估计 60% 提升到估计 80%+。

### 补强 2：Harvest 输出强结构化拒绝标准（解缺口 2）

Harvest 的 LLM prompt 改为强 JSON schema，每个 candidate 必须填以下字段：

```json
{
  "raw_text": "评估提炼质量时优先用真实历史会话",
  "source_observation_id": "obs:claude:abc",
  "future_test": {
    "next_situation": "下次评估提炼或修改 extract 代码时",
    "expected_behavior": "找历史会话跑回归而不是用 fixture",
    "passes": true
  },
  "rejection_reasons_considered": {
    "is_one_off_request": false,
    "is_implementation_status": false,
    "is_personal_statement": false
  }
}
```

`future_test` 是关键：让 LLM **具体说出**未来什么场景下、应该有什么行为。空泛的 reason 无法填这个 schema。

每个 `rejection_reasons_considered` 字段必须显式给 false（如果是 true 则该 candidate 自动 drop）。这是用 schema 强制 LLM 走 checklist，而不是依赖它自由发挥。

**成本**：仅 prompt 调整 + 解析逻辑。半天。

**收益**：Stage 1 假阳性下降，把"未来性"从 0/1 升级为可审计的两段陈述。

### 补强 3：Triplet 三种合法形态（解缺口 3）

不再单一句式。Triplet 升级为带 `shape` 字段的三选一：

```json
{ "shape": "conditional", "when": "...", "what": "...", "why": "..." }
{ "shape": "preference",  "what": "...", "why": "...", "scope_hint": "..." }
{ "shape": "procedure",   "trigger": "...", "steps": ["...", "...", "..."], "why": "..." }
```

#### shape: conditional（默认）

体现"在 X 时做 Y，目的是 Z"。原方案的句式。

渲染：`当 <when>，<what>；目标是 <why>。`

#### shape: preference

体现"在 X 域内偏好 A 而非 B"。配置类规则用这种。

渲染：`<scope_hint> 中，<what>；目标是 <why>。`

例：`项目前端代码中，统一使用 Axios 而非 fetch；目标是统一错误处理与拦截器。`

#### shape: procedure

体现"X 触发时按顺序做 ABC"。流程类规则用这种。

渲染：

```text
<trigger> 时，依次：
1. <step1>
2. <step2>
3. <step3>
目标是 <why>。
```

**强约束**：

- shape 必填，由 LLM 在 Triplet 阶段决定
- 每种 shape 有独立的字段必填表
- 不允许第四种形态
- 同一张卡只能是一种 shape，禁止混合

**成本**：JSON schema + 渲染函数 ~150 行。两天。

**收益**：召回率提升（之前被刚性句式逼掉的合格规则能进来）；表达力提升。

### 补强 4：Witness-Triplet Critic（解缺口 4）

Stage 3 之后插入一道 Critic 校验。**只问一个问题，不开放评价**：

```
[Closed-world prompt to Haiku]

请你只基于下面的 quote，判断后面的卡片内容是否能从 quote 直接推出。
不要使用任何 quote 之外的知识。

Quote: "<witness.quote>"

卡片内容：
- when/trigger: "..."
- what/steps: "..."
- why: "..."

请输出 JSON：
{
  "entails": true | false,
  "missing_or_extra": "如果不能直接推出，说明 quote 没说清的部分"
}
```

**只信 entails: false 的判断（拒绝），不信 entails: true 的判断（不放行）。**

为什么这样设计：研究表明 LLM judge 识别"不一致"召回率只 30-60%。所以 critic 说"过"不等于真的过；但 critic 说"不过"已经是较强信号了。

**关键是"closed-world"指令**：明确告诉模型不要用 quote 之外的知识。不加这一条 critic 等于摆设。

**双 critic？不需要**。研究表明 multi-critic 把成本从 2x 推到 5x，质量提升边际递减。一个 closed-world critic 足够。

**成本**：每张卡 1 次 Haiku 调用。延迟可接受。

**收益**：Triplet 与 Witness 语义不一致的卡被拦下。这是当前**完全空缺**的防线。

### 补强 5：Recurrence + Conflict 两件事一起做（解缺口 5）

不引入 knowledge graph，不引入 LLM debate。两个轻量步骤：

#### Recurrence（在 Harvest 之后）

每个 candidate 计算 `recurrence_count` = 该 signal 在 observation 全集中独立出现的次数。

实现：用 candidate 的 raw_text 做 cosine（embedding 已经有了）+ 关键词重叠（jaccard）；阈值 ≥ 0.7 视为同一 signal 在多处出现。

`recurrence_count >= 3` 的 candidate：自动获得 0.1 confidence 加分；标记 `recurrence_evidence` 数组列出所有 obs id。

意义：用户重复纠正同一件事，比单次说一次的卡更可信。这是 Mem0 没做但本项目可以做的（Mem0 是 ADD-only，每次都新增；本项目是审查后入库，需要 confidence 信号区分）。

#### Conflict（在 Stage 4 写盘前）

新卡 `card_new` 与现有卡 `card_existing` 做 cosine：

| cosine | 处置 |
|---|---|
| ≥ 0.95 | 视为重复，drop `card_new` |
| 0.7 - 0.95 | **暂存为 `merge_candidate`，不直接写盘**，进入人工 Draft Inbox 让用户决定 ACCEPT / REPLACE / MERGE |
| < 0.7 | 直接写盘 |

注意：**不让 LLM 自动决定 REPLACE/MERGE**。Mem0 最新版本回退到 ADD-only 也是因为自动 UPDATE/DELETE 在长程上不靠谱。本项目卡数小、人工审查在 Draft Inbox 里本来就是流程的一部分，把决定权留给人。

冲突卡的 `merge_candidate` 状态在 UI 中显著标示，Draft Inbox 给 4 个按钮：ACCEPT both / REPLACE old / MERGE / REJECT new。

**成本**：cosine 比对复用现有 embedding；Draft Inbox UI 改动 1-2 天。

**收益**：库长期不会积累矛盾卡；recurrence 信号让"用户多次提到的"卡有更高 confidence。

---

## 四、补强后的 Pipeline

```
                              ┌─────────────────────────────┐
                              │  HARVEST  (LLM Haiku)        │
                              │  · 强 JSON schema (补强 2)   │
                              │  · 输出 future_test 显式断言 │
                              └──────────────┬───────────────┘
                                             ▼
                              ┌─────────────────────────────┐
                              │  RECURRENCE (确定性,补强 5) │
                              │  · embedding cosine + jaccard│
                              │  · 计 recurrence_count       │
                              └──────────────┬───────────────┘
                                             ▼
                              ┌─────────────────────────────┐
                              │  WITNESS  (确定性,补强 1)   │
                              │  · 规范化子串匹配           │
                              │  · 返回原文未归一化片段     │
                              └──────────────┬───────────────┘
                                             ▼
                              ┌─────────────────────────────┐
                              │  TRIPLET  (LLM Sonnet)       │
                              │  · 三种 shape (补强 3)       │
                              │  · 强 JSON schema            │
                              └──────────────┬───────────────┘
                                             ▼
                              ┌─────────────────────────────┐
                              │  CRITIC  (LLM Haiku,补强 4) │
                              │  · closed-world entailment  │
                              │  · 只信 entails: false      │
                              └──────────────┬───────────────┘
                                             ▼
                              ┌─────────────────────────────┐
                              │  CONFLICT (确定性,补强 5)   │
                              │  · cosine 与现有卡比对      │
                              │  · 0.7-0.95 暂存为 candidate│
                              └──────────────┬───────────────┘
                                             ▼
                                       MemoryCard 落盘
                                       或进入 Draft Inbox
```

LLM 调用数：3 次（Harvest / Triplet / Critic），仍然比当前 5 ProviderRole 少。

确定性步骤：3 个（Recurrence / Witness / Conflict），是质量的硬骨架。

---

## 五、质量保证（具体到可测）

借鉴 Mem0 的探针思路，给每张落盘的卡定义 4 个**确定性可测**的健康指标：

| 指标 | 定义 | 健康阈值 |
|---|---|---|
| Witness coverage | 卡数中能成功 locate 原文 quote 的比例 | 100%（确定性步骤，硬约束） |
| Triplet shape conformance | 卡数中通过 shape schema 的比例 | 100%（schema 校验，硬约束） |
| Critic-pass rate | Critic 投票 entails: true 的比例 | ≥ 90%（统计探针） |
| Conflict-free rate | 入库后未被新卡触发 cosine 0.95+ 冲突的比例 | ≥ 95%（30 天滚动） |

**前两条 100% 由确定性代码保证**，不达标就是 bug。后两条是抽样探针，掉到阈值下要排查 prompt。

### 端到端质量保证陈述（可以写进 README）

> 经过 Stage 1 至 Stage 5 的卡，满足以下硬约束：
>
> 1. **每张卡都有真实 quote 锚定原文**（Witness 是确定性子串匹配）
> 2. **每张卡形态属于三种合法 shape 之一**（Schema 校验）
> 3. **每张卡的 Triplet 内容能从 quote 通过 closed-world Critic 直接推出**（≥ 90% 抽样验证）
> 4. **每张卡入库时不与现有卡冲突**（cosine 0.95+ 直接 drop，0.7-0.95 进人工审查）

这是机器层面能给出的"高质量"保证。剩余的语义级判断由 Draft Inbox 的人工审查兜底——这本来就是项目的设计原则（"人类负责批准长期记忆"）。

---

## 六、不做什么（坚持边界）

- **不做 multi-critic**：研究表明边际收益小、成本高 5x。
- **不做 reflection 多轮重写**：sycophantic reflection 风险，可能把对的改错。Stage 3 失败两次直接 drop。
- **不做 LLM 自动 REPLACE/UPDATE/DELETE**：Mem0 最新版本回退到 ADD-only 已经证明这条路在长程不稳。冲突卡进 Draft Inbox 让人决定。
- **不做 knowledge graph entity extraction**：本项目 ≤ 500 张卡，过度设计。
- **不做"自适应阈值"**：所有阈值固定（0.95 / 0.7 / 90% / 95%），调整必须改代码并通知所有人。
- **不引入 NLI 模型**（DeBERTa-v3-MNLI 等）：研究表明在领域任务上不如 LLM-as-judge，且引入 ONNX 模型会膨胀依赖。

---

## 七、与原 extraction-core-design.md 的关系

原文档是骨架，本文档是骨架内的补丁。建议处理方式：

| 选项 | 操作 | 推荐 |
|---|---|---|
| A. 保留两份 | extraction-core-design.md 不动，本文档作为补丁存在 | 否 |
| B. 合并 | 把 5 项补强直接写回 extraction-core-design.md，本文档作为变更记录归档 | **是** |

合并后的 extraction-core-design.md 结构会是：

```
1. 一句话定义
2. 三个核心概念（Signal / Witness / Triplet）
3. 当前为什么不行
4. 五阶段架构（HARVEST / RECURRENCE / WITNESS / TRIPLET / CRITIC + CONFLICT）  ← 升级
5. Triplet 三种 shape（补强 3）  ← 替换原"金标准模板"
6. 模型与代码分工（3 LLM 调用）
7. 实施步骤  ← 调整
8. 质量保证（4 个探针 + 端到端陈述）  ← 替换原"3 个数字"
9. 不做什么
```

我推荐做合并（选项 B）。但合并是 destructive 操作，需要你点头才动。本文档先独立存在，让你决定。

---

## 八、实施成本估计

| 补强 | Rust 改动 | LLM 调整 | 工期 | 风险 |
|---|---|---|---|---|
| 1. Witness 规范化 | 30 行 + 测试 | 无 | 1 天 | 低 |
| 2. Harvest 强 schema | 50 行 | prompt 重写 | 1 天 | 低 |
| 3. Triplet 三 shape | 150 行 + 渲染 | prompt 重写 | 2 天 | 中（验证三 shape 都要 fixture） |
| 4. Critic 步骤 | 80 行 | 新 prompt | 1 天 | 低 |
| 5. Recurrence + Conflict | 100 行 + UI | 无 | 2 天 + UI 1 天 | 中（UI 改动） |

总计：约 8 天 Rust + UI。每项独立可上线。

**单独上线建议顺序**：1 → 2 → 4 → 3 → 5。补强 1 是其他所有的基石；补强 4 性价比最高（拦截所有 hallucination triplet）；补强 3 最有不确定性，放后面。

---

## 九、最终回答用户的问题

> "这个方案的提取质量有保证吗？"

**原方案**：保证不足。三概念骨架是对的，但 Witness 太脆、Harvest 太松、Triplet 太刚性、缺 Critic、缺冲突处理。

**补强后**：能给出明确保证：
- Witness 100%（确定性）
- Shape 100%（schema 校验）
- Critic-pass ≥ 90%（统计探针）
- Conflict-free ≥ 95%（30 天滚动）
- 兜底：Draft Inbox 人工审查

补强成本 8 天工期，3 次 LLM 调用，比当前 5 ProviderRole 反而更省。

> "是必须项。"

接受。补强 1/4 是必做（Witness 规范化 + Critic）；补强 2/3/5 强烈建议；不做就坦率告诉用户"质量上限会卡在 LLM 单次输出的可靠性"。

---

## 参考资料

- [Mem0 ADD-only Architecture & Quality Probes](https://mem0.ai/blog/state-of-ai-agent-memory-2026)
- [KARMA Multi-Agent KG Enrichment with Verifier](https://openreview.net/pdf?id=k0wyi4cOGy)
- [Generator-Critic Pattern (Zylos Research 2026)](https://zylos.ai/research/2026-03-06-ai-agent-reflection-self-evaluation-patterns)
- [LLM-as-Judge Evaluation Guide (Confident AI)](https://www.confident-ai.com/blog/llm-evaluation-metrics-everything-you-need-for-llm-evaluation)
- [CaseFacts Closed-world LLM Verification](https://arxiv.org/html/2601.17230v1)
- [LLM Evaluators High Precision Low Recall on Inconsistencies](https://eugeneyan.com/writing/llm-evaluators/)
