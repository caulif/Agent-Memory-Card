# 提炼核心设计

> 适用范围：`agent-kernel` 的提炼引擎本身——从对话证据到 MemoryCard 的核心算法与数据建模。
>
> 关于"快"看 [extraction-pipeline-optimization-plan.md](./extraction-pipeline-optimization-plan.md)。
>
> 关于"治理"看 [extraction-quality-improvement-plan.md](./extraction-quality-improvement-plan.md)。
>
> 本文只讲提炼本身。简单、优美、可实现。

---

## 一、一句话定义

**提炼 = 从对话中找到 `<When, What, Why>` 三元组，并为每条三元组留下真实的原文证人。**

提炼不是"总结对话"，不是"打分排序"，更不是"反复改写让 LLM 评分高"。它就是这一件事：找三元组、留证人。

## 二、三个核心概念

只用三个概念解释整条提炼链路。任何字段、任何 stage、任何规则都必须能映射回这三个概念之一。如果不能，它就不该存在。

### 概念一：Signal（信号）

一段对话里出现"这件事以后还会发生"的痕迹。

判断 signal 的唯一标尺：**未来性**。能不能用一句话补完"以后再遇到 ___ 这种情况，就 ___"。补不完 → 不是 signal。

例：
- "以后前端请求统一用 Axios" → 强 signal（未来性明确）
- "把这个 bug 修一下" → 不是 signal（一次性请求）
- "代码要写清晰" → 弱 signal（无场景指向）

实现上 signal 不是一个独立字段，而是 candidate 进入流水线的资格——不是 signal 的根本就不该被 harvest 出来。

### 概念二：Triplet（三元组）

每条 signal 必须能写成 `<When, What, Why>`：

| 槽 | 含义 | 一句话回答 |
|---|---|---|
| When | 触发条件 | 在什么场景下，这条规则被激活 |
| What | 动作约束 | 这条规则要求做什么 / 不做什么 |
| Why | 目的 | 这样做要达成什么 |

如果一段文本不能干净地填入这三个槽，它就不是合格的卡。

例（合格）：
- When：评估提炼质量或修改提炼逻辑时
- What：用真实历史会话做回归验证，不只用静态样例
- Why：让提炼规则接受真实数据的检验

例（不合格）：
- "代码要清晰" —— When 缺失（什么时候不需要清晰？）
- "用 Axios" —— Why 缺失（为什么是 Axios 不是 fetch？）
- "请测试这个 bug" —— What 是一次性指令而非约束

Triplet 是高质量卡片的**形状**。所有质量规则都从这个形状推出，不需要发明 8 个维度、12 维 score。

### 概念三：Witness（证人）

每张卡必须能指向**一段从原文字面引用的 quote**作为证人。Witness 是 signal 的合法性证据。

三条硬约束：

1. quote 必须在某条 observation body 中**字面包含**（子串匹配，不是语义近似）。
2. quote 长度 8-200 字。太短不构成证据，太长是抄段落。
3. quote 必须是 `user` 或 `assistant` 的真实发言，禁止 `ai-synthesis`（LLM 自己造的）。

如果找不到 witness，candidate 就不该成卡。这一条是硬门槛，**不商量**。

---

## 三、当前为什么不行（最短诊断）

把三个概念套到现状上，问题立刻显形：

- **Signal 没有显式**：现有 `score_breakdown` 12 维（`actionability`、`fragility_value`、`grounding`...），但没有"未来性"这一维。导致一次性请求（"可以不要每次推送都发 release 吗"）能被当 candidate 留下。
- **Triplet 没有结构化**：body 写成什么形状全看 LLM 心情。当前 archive 里的 body 有的是完整 When+What+Why，有的只有 What。没有 schema 强制。
- **Witness 是循环的**：`evidence_span.quote == body`，`role: ai-synthesis`，`source_observations: []`。证人是 LLM 自己写的卡——这等于没有证人。

**根本问题不是过滤器不够多，是没有形状约束**。

---

## 四、三阶段架构

只有三个 stage，分别对应**找 / 验 / 写**。每阶段一道门，不通过就丢，不修不补。

```
原始对话 / observation
        │
        ▼
┌─────────────────────┐
│  Stage 1: HARVEST   │   找 signal candidate（要召回多）
│  is_future_oriented?│
└─────────────────────┘
        │
        ▼
┌─────────────────────┐
│  Stage 2: WITNESS   │   绑真实 quote（要精确硬）
│  quote ⊂ obs.body ? │
└─────────────────────┘
        │
        ▼
┌─────────────────────┐
│  Stage 3: TRIPLET   │   写成 When/What/Why（要规整）
│  three slots filled?│
└─────────────────────┘
        │
        ▼
   MemoryCard 落盘
```

### Stage 1 · HARVEST（收割）

**目标**：从原始对话中圈出可能的 signal。**宁可多召回，让 Stage 2/3 拒**。

**实现**：单次 LLM 调用，输入是带 `--- obs:<id> ---` 标记的对话片段（已经在 `observation/chunked.rs` 中实现），输出 candidate 数组：

```json
[
  {
    "raw_text": "评估提炼质量时优先用真实历史会话做回归验证，不要只依赖静态样例",
    "future_oriented_reason": "明确指向未来评估场景，不是一次性请求",
    "source_observation_id": "obs:claude:abc123"
  }
]
```

**唯一的拒绝规则**：`future_oriented_reason` 不能为空。LLM 自己想不出"为什么这值得长期记"，就别返回。

**模型**：Haiku 4.5（廉价，召回为主）。

**不做的事**：
- 不做 12 维打分
- 不做"分类"（kind/scope 这一步无法可靠判断，留给 Stage 3）
- 不做 deduplication（留给后置）

### Stage 2 · WITNESS（取证）

**目标**：把每个 candidate 锚定到原文。**不通过 LLM**，纯字符串匹配。

**实现**（确定性 Rust 函数，无 LLM）：

```rust
fn find_witness(candidate: &HarvestCandidate, observations: &[ObservationRecord])
    -> Result<Witness, WitnessError>
{
    // 1. 取 candidate.raw_text 的核心子串（去标点、保留 8-200 字）
    let needle = extract_core_quote(&candidate.raw_text)?;

    // 2. 在 source_observation.body 中找完整子串
    let observation = observations
        .iter()
        .find(|o| o.id == candidate.source_observation_id)
        .ok_or(WitnessError::ObservationMissing)?;

    if !observation.body.contains(&needle) {
        return Err(WitnessError::QuoteNotInObservation);
    }

    // 3. 校验角色
    if observation.role == "ai-synthesis" {
        return Err(WitnessError::SyntheticRole);
    }

    Ok(Witness { quote: needle, observation_id: observation.id.clone(), role: observation.role.clone() })
}
```

**输出**：每个通过的 candidate 携带一个 `Witness { quote, observation_id, role }`。

**通过率预期**：60-80%。Stage 1 召回多，这里淘汰假信号。

**关键设计**：witness 是后取的，不是 LLM 生成的。这一点彻底解决"evidence_span.quote == body 的循环引用"问题。

### Stage 3 · TRIPLET（成卡）

**目标**：把 candidate + witness 重写成 `<When, What, Why>` 三元组，并按金标准模板渲染。

**实现**：单次 LLM 调用，强 JSON schema：

```json
{
  "when": "评估提炼质量或修改提炼逻辑时",
  "what": "用真实历史会话做回归验证，不只用静态样例",
  "why": "让提炼规则接受真实数据的检验",
  "kind": "procedure",
  "scope": "global",
  "activation": "always-on"
}
```

**模型**：Sonnet 4.6（精修，质量为主）。

**渲染**（确定性 Rust 函数，无 LLM）：

```rust
fn render_card(triplet: Triplet, witness: Witness) -> MemoryCard {
    MemoryCard {
        title: derive_title(&triplet.what),         // 从 what 派生，动宾结构
        body: format!("当{}，{}；目标是{}。", triplet.when, triplet.what, triplet.why),
        brief: format!("用于{}", triplet.why),       // 从 why 派生
        kind: triplet.kind,
        scope: triplet.scope,
        activation: triplet.activation,
        tags: derive_tags(&triplet),                // 从 triplet 派生（单一来源！）
        evidence: witness,
        // ...
    }
}
```

**关键设计**：所有派生字段（title/body/brief/tags）从 triplet 单向投影出来。**不再有"多个真实来源各说各话"**——这是当前最大的混乱来源。

**拒绝条件（确定性）**：

- triplet 三槽任一为空或重复（when == what 之类）
- when 没有"当...时"或"在...场景"句式
- body 长度不在 [50, 200] 中文字
- brief 与 body 的字符级 Jaccard 相似度 ≥ 0.5（防套娃）

任何拒绝原因 → 整张卡丢弃，**不再让 LLM 重写一次**。失败两次说明候选不合格。

---

## 五、Triplet 金标准模板

三个槽各自的形状约束。这就是"高质量记忆卡片该有的样子"——简单到能背下来。

### When 槽

- 必须以"当...时"或"在...时/下/中/场景"开头
- 描述场景，不描述对象（"当评估提炼质量时" ✓ ；"评估" ✗）
- 单一场景（不并列两个 when）

### What 槽

- 必须有一个动词宾语
- 必须是约束/动作（"用 X""不要 Y""先 X 再 Y"）
- 不能是疑问、不能是软建议（"建议"、"可以考虑"）

### Why 槽

- 一句话说"目的是..."
- 不重复 what 的内容
- 是抽象目的，不是具体步骤

### 渲染后的 body 标准句式

```
当 <When>，<What>；目标是 <Why>。
```

读起来像一句话。**这是唯一允许的 body 形状**。

### 反例（自动拒绝）

| body | 原因 |
|---|---|
| 代码要写清晰。 | when 缺失 |
| 用 Axios。 | when + why 缺失 |
| 当评估时用真实历史，同时要测试，另外注意性能。 | 多 what |
| 当评估时，评估，让评估更准。 | 三槽自指 |
| 不要每次推送都发 release。 | 是请求不是规则 |

---

## 六、模型与代码分工

### 模型分工（最少 LLM 调用）

| 阶段 | 是否用 LLM | 模型 | 输入大小 | 单调用成本 |
|---|---|---|---|---|
| Harvest | 是 | Haiku 4.5 | 1 chunk ≈ 8K tokens | 极低 |
| Witness | **否** | — | — | 0 |
| Triplet | 是 | Sonnet 4.6 | 1 candidate < 500 tokens | 低 |
| Render | **否** | — | — | 0 |

每张候选总计 LLM 调用 = 1 次 Haiku（在 chunk 内摊薄）+ 1 次 Sonnet。比当前的 5 ProviderRole 各调一次（Refine/Abstract/Update/Judge/synthesize）少 60% 以上。

### 代码模块分工

```
src/extract/
├── harvest.rs        # Stage 1，含 LLM prompt 模板
├── witness.rs        # Stage 2，纯函数，无外部依赖
├── triplet.rs        # Stage 3，含 LLM prompt 模板
├── render.rs         # MemoryCard 派生渲染（单一真实来源）
└── pipeline.rs       # 三 stage 串联，错误传播
```

每个文件 < 300 行。`pipeline.rs` 应该 < 100 行（只是串联）。

**与现状对比**：

| 现状 | 新设计 |
|---|---|
| `extract.rs` 942 行（混合多入口） | `pipeline.rs` < 100 行 |
| `quality_gate.rs` 848 行（17+ 启发函数） | 拆为 witness.rs（纯函数） + triplet.rs 的 schema 校验 |
| `extract/refine.rs` 279 行（关键词改写已删） | 替换为 `triplet.rs` |
| 12 维 `score_breakdown` | 不要了。Stage 1 单一布尔 + Stage 3 单一 schema 校验 |

把 17 个 `looks_like_*` 函数减到 0。把 12 维分数减到 0。把 5 个 ProviderRole 减到 2 个 stage。**减法即优雅**。

---

## 七、实施步骤（3 步走）

每步独立可上线，独立可回滚。

### Step 1：实现 Witness（最简单，先做）

- 新增 `src/extract/witness.rs` 与 `src/extract/witness_tests.rs`
- 接口 `find_witness(candidate, observations) -> Result<Witness, WitnessError>`
- 测试覆盖 5 种 error 场景 + happy path
- **不接入主流程**，只是模块就位

收益：拿到 witness 这把锁，后续两步才有合法性基石。

### Step 2：实现 Triplet 替代 refine

- 新增 `src/extract/triplet.rs` 与 `src/extract/render.rs`
- 把 `extract/refine.rs` 中走 LLM 的路径改为 triplet 模式
- 修改 prompt：要求模型直接返回 `{when, what, why, kind, scope, activation}` 而非 body 字符串
- `render.rs` 中拼装 body / brief / title / tags
- 老的 deterministic fallback（已经简化过）保留作为 LLM 不可用时的退化

收益：彻底解决"多真实来源"问题。所有派生字段从 triplet 单向投影。

### Step 3：重构 Harvest，扔掉 17 个启发函数

- 新增 `src/extract/harvest.rs`，单一 LLM 调用，输出 candidate 数组
- 删除 `quality_gate.rs` 中所有 `looks_like_*`（保留过去半年最稳定的 3-5 个作为 prompt few-shot 反例，不再写代码）
- 把 `extract.rs` 的 4 个入口收口为 `pipeline::run(input) -> Vec<MemoryCard>`

收益：`extract.rs` 从 942 行 → < 100 行。`quality_gate.rs` 从 848 行 → 0 行（删除）。

---

## 八、质量验证（最小集）

不需要复杂指标，三个数字就够：

| 指标 | 怎么测 | 目标 |
|---|---|---|
| Witness 通过率 | Stage 2 通过 / Stage 1 输出 | 60-80%（太低说明 Harvest 太松，太高说明 Harvest 太严） |
| Triplet 通过率 | Stage 3 通过 / Stage 2 输出 | 70-85% |
| 人工接受率 | 抽样 50 张通过卡，人工判定 | ≥ 80% |

每次改 prompt 跑一次三个数字。三个数字稳定，就发布。

---

## 九、不做什么

- 不做 12 维 score。一个布尔（is_signal）+ 一个 schema 校验已经足够。
- 不做"动态置信度阈值"。阈值固定。改了就通知所有人，不要静默调参。
- 不做"自适应 prompt"。prompt 改动是显式版本切换。
- 不做 knowledge graph、entity extraction、relationship modeling。本项目是项目级 ≤ 500 张卡，过度设计。
- 不做"记忆遗忘"。卡的进退由人审，不由算法。
- 不做"二次重试"。LLM 在 Stage 1 或 Stage 3 失败就丢，不要让模型多次涂改一张卡。
- 不在 Stage 3 之后再设新 stage。三个 stage 是上限。

---

## 十、与既有方案的关系

| 文档 | 解决什么 | 新设计如何对齐 |
|---|---|---|
| pipeline-optimization-plan.md | 性能（速度+成本） | 本设计 LLM 调用从 5 → 2，性能方案的 T2.1（多模型）/ T1.3（cache）天然适配 |
| quality-improvement-plan.md | 质量（gate + golden set） | 本设计的 Stage 2/3 取代了原 5 stage gate；金标准模板从原 8 维改为 3 槽，更具体 |
| 本设计 | 提炼本身 | 是前两份的"内核" |

如果只能做一件事，做这一份。前两份是性能与治理的延伸。

---

## 十一、一页总结（贴在显示器上）

```
提炼 = 找 Signal → 取 Witness → 写 Triplet

Signal  : 这事以后还会发生吗？
Witness : 这话原文哪里说的？
Triplet : 当____ 时，____ ；目标是 ____。

三道门，不修不补。
失败优于伪装。
减法即优雅。
```
