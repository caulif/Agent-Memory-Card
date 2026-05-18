# 提炼链路最终方案与 Roadmap

> 这是收口文档。前 5 份方案（pipeline-optimization / quality-improvement / core-design / quality-hardening / design-reality-check）都是阶段性思考，本文档是结论。
>
> **核心视角转变**：从"过滤管道"（filter pipeline）转为"压缩管道"（compaction pipeline）。
>
> **灵感来源**：
> - Anthropic Claude Code 的[五层压缩流水线](https://arxiv.org/abs/2604.14228)（cheapest first，lazy degradation）
> - RAG 领域的[层级 chunking + cluster 检索](https://arxiv.org/html/2507.09935v1)（把分散表达聚合成有意义单元）
>
> 不照搬，借思想。

---

## 一、根本视角转变：过滤 vs 压缩

我之前的所有方案都是这个形态：

```
candidate → gate1 → gate2 → gate3 → gate4 → gate5 → 卡片
              ↓ reject     ↓ reject       ↓ reject
              丢弃          丢弃           丢弃
```

每个 gate 做"接受 or 拒绝"二分判断。问题：

- 单 quote 必须字面匹配 → 误杀
- 单一句式 → 装不下真实形态
- LLM 从单 quote 推断 → 容易 hallucination
- 一次性请求和反复偏好被同等对待

正确视角应该是：

```
原始对话 → 去噪 → 截断 → 聚类 → 归纳 → 结晶 → 卡片
            ↓        ↓       ↓        ↓        ↓
          无损     有损但  无损     有损但   有损但
                  结构化  聚合     压缩     规范
```

每层是**渐进压缩 + 信息聚合**，不是 reject/accept。Claude Code 五层压缩的精髓正在这里：每层处理不同压力，cheapest first，lazy degradation。

应用到提炼：**用户的偏好往往分散在多次对话里口语化表达，单 quote 推规则注定脆弱；只有先聚类再归纳才能让多个证据共同支撑一条卡**。

---

## 二、五层提炼流水线

```
┌─────────────────────────────────────────────────────────┐
│  Layer 1 · STRIP        确定性 · 无损 · 微秒级           │
│  去噪：剥离 system reminder、tool 回显、空消息、重复     │
└──────────────┬──────────────────────────────────────────┘
               ▼
┌─────────────────────────────────────────────────────────┐
│  Layer 2 · TRUNCATE     确定性 · 头尾保留 · 微秒级       │
│  长消息保留头部（场景）和尾部（结论），中间折叠          │
└──────────────┬──────────────────────────────────────────┘
               ▼
┌─────────────────────────────────────────────────────────┐
│  Layer 3 · CLUSTER      embedding · 无损 · 毫秒级        │
│  按语义相似聚类相关消息（关键步骤）                      │
│  「分散偏好」在这一层被合并为「证据集合」                │
└──────────────┬──────────────────────────────────────────┘
               ▼
┌─────────────────────────────────────────────────────────┐
│  Layer 4 · INDUCE       LLM Haiku · 多→一 · 秒级         │
│  每个 cluster → 1 个 candidate                          │
│  evidence 是 quote 集合（不是单 quote）                  │
└──────────────┬──────────────────────────────────────────┘
               ▼
┌─────────────────────────────────────────────────────────┐
│  Layer 5 · CRYSTALLIZE  LLM Sonnet + schema · 秒级       │
│  规范化为 MemoryCard，单一真实来源                       │
└─────────────────────────────────────────────────────────┘
```

每层独立可测、可禁用、可单独 dry-run。

### Layer 1 · STRIP（去噪）

**输入**：原始 jsonl session 文件
**输出**：去噪后的 user/assistant 真实表达列表

去掉的内容：

- `<system-reminder>` 系统注入
- tool_use / tool_result 回显（保留人类可读的总结部分，去掉机械输出）
- 空消息、纯空白、纯标点
- 完全重复的消息（同一句话被复制多次）
- "你是什么模型"、"你好"等无信息问候

实现：纯 Rust 函数 + 正则。无 LLM。

**对比 Claude Code 第 1 层**：Claude Code 是替换为指针；我们是直接丢弃（提炼场景下指针没用）。

### Layer 2 · TRUNCATE（截断）

**输入**：长消息（> 500 字符）
**输出**：保留头部 + 尾部，中间折叠

例：用户粘贴的 5000 字代码错误 → 保留前 500 字（错误类型）+ 尾 500 字（堆栈触发点）+ 标记 `[省略 4000 字]`。

**对比 Claude Code 第 2-3 层**：完全照搬思想。

### Layer 3 · CLUSTER（聚类）—— 这是核心创新

**输入**：清理后的消息片段（来自任意多个 session）
**输出**：按语义相似度分组的 cluster 列表

实现：

1. 每条消息计算 embedding（用 fastembed，已有）
2. 相似度 ≥ 0.65 的消息归同一 cluster（阈值偏宽，召回优先）
3. 单消息 cluster 也保留（但标记 `recurrence: 1`，confidence 衰减）

为什么这一层是核心：

- 用户在 session A 说"以后用 axios"，在 session B 说"我喜欢 axios，比 fetch 好"，在 session C 说"还是 axios 吧"——三句话分散，单看任何一句都不够强。**聚类后看到 3 条证据，就是强偏好**。
- 自然内嵌 recurrence count（cluster 大小）
- 自然解决"分散表达"问题
- 后续 LLM 看到 cluster 全部成员，归纳质量远高于看单 quote

**这是从五层压缩 + RAG cluster 思想合成的关键设计**。

### Layer 4 · INDUCE（归纳）

**输入**：每个 cluster（含 N 条相关消息）
**输出**：每个 cluster → 0 或 1 个 candidate

LLM 任务：

```
看下面这 N 条来自不同时间的消息（按时序排列）。
判断它们是否共同表达一个长期可复用的规则。
- 是 → 输出 candidate（含 title / when / what / why，不强制完整）
- 否 → 输出 null
```

输入示例：

```
cluster_id: c_42
recurrence: 4
messages:
  [session a4c2, 2026-04-12]: "以后所有前端请求都用 axios 吧"
  [session b608, 2026-04-15]: "记得用 axios 不要用 fetch"
  [session 5ecd, 2026-05-02]: "axios 的拦截器更好用"
  [session 0ee8, 2026-05-08]: "再次确认用 axios"
```

输出：

```json
{
  "title": "前端 HTTP 请求统一用 axios",
  "when": "在前端项目中发起 HTTP 请求时",
  "what": "使用 axios 而非 fetch",
  "why": "拦截器更好用",
  "evidence_quotes": [...],
  "evidence_observations": ["a4c2:...", "b608:...", "5ecd:...", "0ee8:..."]
}
```

模型：Haiku（廉价）。每个 cluster 1 次调用。

**关键约束**：

- evidence_quotes 必须是 cluster 内消息的字面片段（确定性可校验）
- when / what / why 由 LLM 从多 quote 归纳，可以有 quote 没说出的部分（隐含 why），因为是从多条证据归纳，不是从单 quote 推断
- candidate 数 = cluster 数（粗筛后的）

### Layer 5 · CRYSTALLIZE（结晶）

**输入**：candidate
**输出**：规范化 MemoryCard 或 reject

LLM 任务：

```
把下面 candidate 规范化为一张 MemoryCard。
- title: 8-30 字简体中文，动宾结构
- body: 50-200 字，规范句式（见模板）
- brief: 15-60 字，以"用于"开头，禁止重复 body
- kind / scope / activation: 从 classification 单向投影（schema 强制）
- tags: 派生自 kind/scope/activation
不要新增 evidence，沿用 candidate 的 evidence_quotes。
```

模型：Sonnet（精修）。

**单一真实来源原则**：所有派生字段（title/body/brief/tags）从 LLM 输出 + 派生规则计算，**不允许多处独立写入**。这一条彻底解决"activation 三处不一致"的根本缺陷。

确定性 reject 条件：

- title / body / brief 字符数越界
- brief 与 body 字符级 Jaccard ≥ 0.5
- 任何派生字段间不一致

reject 不重试，直接进 Draft Inbox 让人决定。

---

## 三、最小化数据契约

```yaml
# 顶层（11 字段，全必填）
schema_version: 2
id: <scope>:<slug>
title: ...                           # 8-30 字
body: ...                            # 50-200 字
brief: 用于...                       # 15-60 字，禁止重复 body
kind: preference | constraint | procedure
scope: global | project
activation: always-on | skill | manual | path-glob
tags:                                # 派生自 kind/scope/activation
  - kind:<kind>
  - scope:<scope>
  - activation:<activation>
  - <domain-tag>...
language: zh-CN | en | ...
created_at / updated_at: ISO8601

# evidence（5 字段，强约束）
evidence:
  cluster_id: c_42                   # Layer 3 产物
  recurrence: 4                      # cluster 大小，自然 confidence 信号
  source_observations: [...]         # 至少 1 条真实 obs id
  quotes:                            # 至少 1 条，每条字面来自原文
    - observation_id: a4c2:...
      text: "以后所有前端请求都用 axios 吧"
      role: user
  origin: induced                    # 标明是从多 quote 归纳

# extraction（3 字段，可观测性）
extraction:
  pipeline_version: 1
  layer_trace:                       # 每层耗时与决策
    - {layer: strip, ms: 12, kept: 850/1024}
    - {layer: cluster, ms: 340, cluster_size: 4}
    - {layer: induce, ms: 1820, model: haiku}
    - {layer: crystallize, ms: 2410, model: sonnet}
  rejected_at: null                  # 若被拒，标注哪一层
```

**核心简化**：

- 删掉 12 维 score_breakdown
- 删掉 classification（合并到顶层 kind/scope/activation）
- 删掉 suggested_action（compile_enabled 由 activation 派生）
- evidence 从单 quote 扩展为 quote 列表（原 reality-check 的关键修正）
- 加 layer_trace（可观测性，照搬 Claude Code 思想）

---

## 四、为什么这次方案能解决之前的问题

| 之前缺陷 | 解决机制 |
|---|---|
| activation 三处不一致 | Layer 5 单一真实来源 + tags 派生 |
| evidence 是 LLM 自造 | Layer 4 evidence_quotes 必须来自 cluster 内字面片段 |
| 单 quote 字面匹配误杀 | Layer 3 聚类后多 quote 共同支撑，单条字面性丧失也无所谓 |
| 单一句式装不下真实形态 | Layer 5 schema 不规定 body 句式，只校验长度+jaccard |
| Why 必填逼出 hallucination | Layer 4 从多 quote 归纳，多条证据自然支撑 why；Layer 5 不强制 why 字段独立 |
| 一次性请求被收录 | Layer 3 cluster size = 1 时 confidence 自动衰减；Layer 4 提示 LLM 重点关注 ≥ 2 的 cluster |
| 反复纠正没被当强信号 | recurrence = cluster size，天然权重 |

**沙盘推演**：之前 4 张人工合格 archive 卡用新方案能否通过？

- 卡 A "保留人工审阅边界"：在历史中至少出现 1 次 cluster → Layer 4 从该 cluster 归纳 → Layer 5 规范化 → 通过
- 卡 B "优先用真实历史"：同上
- 卡 C "从用户视角"：同上
- 卡 D "按改动风险"：同上（不再被 shape 卡）

**误杀率从 100% 降到接近 0%**。

---

## 五、Roadmap：接下来 5 周做什么

每周一个 deliverable，每周可独立验证收益。

### Week 1 · Layer 1 + Layer 2（去噪 + 截断）

**做什么**：

- 新建 `src/extract/strip.rs`，实现 jsonl → 清理后消息列表
- 新建 `src/extract/truncate.rs`，实现头尾保留逻辑
- 接入 CLI：`cargo run -- extract pipeline --layer strip --input <session.jsonl>`

**验证**：

- 跑一个真实 session，输出消息数应大幅减少（估计 -60% 噪声）
- 单元测试：固定 fixture 跑两次结果一致

**前置依赖**：无

### Week 2 · Layer 3（聚类，最大杠杆）

**做什么**：

- 新建 `src/extract/cluster.rs`，复用 `extract/embedding.rs` 的 fastembed
- 实现简单的 union-find / hierarchical 聚类（cosine ≥ 0.65 归一组）
- 输出 cluster 列表（每个含 messages + recurrence count）
- 接入 CLI：`extract pipeline --layer cluster`

**验证**：

- 跑全 103 session，观察 cluster 分布
- 人工检查 top-10 大 cluster：是否真的代表用户偏好
- 这一步**最能反映方案是否成立**——cluster 质量决定后面所有

**前置依赖**：Week 1

### Week 3 · Layer 4（归纳）

**做什么**：

- 新建 `src/extract/induce.rs`
- LLM prompt 模板放到 `prompts/induce.md`
- 走 Haiku，每个 cluster 1 次调用
- 强 JSON schema 输出 candidate

**验证**：

- 跑 cluster ≥ 3 的所有 cluster
- 人工抽样 20 个 candidate，看是否真实归纳了用户偏好
- 这一步可以引入第一份**真实可批准的卡**

**前置依赖**：Week 2

### Week 4 · Layer 5（结晶）+ 数据契约迁移

**做什么**：

- 新建 `src/extract/crystallize.rs`
- LLM prompt 模板 `prompts/crystallize.md`
- 走 Sonnet，强 schema
- 写 schema migration `cargo run -- memory-card migrate --to-schema 2`
- 把现有 archive 卡 migrate 到新 schema（标 origin: legacy）

**验证**：

- 端到端跑：原始 session → 5 张完整 MemoryCard
- 跟现有 archive 卡对比，用新方案能否产出更高质量的卡
- 跑 migration，旧卡能正确转换

**前置依赖**：Week 3

### Week 5 · 黄金集 + 收口

**做什么**：

- 人工标注 30 张 ground truth（参考 `~/.claude/CLAUDE.md` + `MEMORY.md` + 历史 user 原话）
- 写 `tests/golden_set_regression.rs`
- 把旧的 5 stage gate（`quality_gate.rs` 848 行）整体废弃
- 把 `extract.rs` 4 个入口收口为 `pipeline::run()`
- 更新 README，明确新提炼模型

**验证**：

- 黄金集 positive recall ≥ 80%（5 层流水线在 cluster 充足时应能达到）
- 黄金集 negative precision ≥ 90%
- `extract.rs` 行数从 942 → < 200
- `quality_gate.rs` 从 848 → 0（删除）

---

## 六、与之前 5 份方案文档的处置

| 文档 | 处置 | 理由 |
|---|---|---|
| extraction-pipeline-optimization-plan.md | **保留** | 性能优化与本方案正交。Plan 0（metrics）/ Plan 1（cache）/ Plan 2（多模型路由）都直接服务本方案 |
| extraction-quality-improvement-plan.md | **作废** | 8 维 score 与 5 stage gate 思路被本方案的渐进压缩替代 |
| extraction-core-design.md | **作废** | Signal/Witness/Triplet 三概念骨架被五层流水线吸收，但具体设计已 reality-check 证伪 |
| extraction-quality-hardening.md | **作废** | 是 core-design 的补丁。core-design 作废它也跟着作废 |
| extraction-design-reality-check.md | **保留** | 是诚实的反向论证，下次设计任何方案前应该先读这份避免重蹈 |

可以直接：

```bash
# 在 docs/ 下加一个 archived 子目录
mkdir docs/archived
mv docs/extraction-quality-improvement-plan.md docs/archived/
mv docs/extraction-core-design.md docs/archived/
mv docs/extraction-quality-hardening.md docs/archived/
```

保留 reality-check 在主目录作为反思记录。

---

## 七、不做什么（边界）

- **不做 knowledge graph entity extraction**：聚类已经够用，KG 是 RAG 重型方案。
- **不做向量库（LanceDB / Qdrant）**：500 卡规模内存 cosine 即可。
- **不做 Critic Stage**：研究证明 LLM judge 召回低，加一层不实质提升。把质量交给"多 quote 归纳"自然解决。
- **不做 reflection 多轮重写**：Layer 5 失败直接进 Draft Inbox，不让 LLM 反复改。
- **不做自动 REPLACE/UPDATE/DELETE**：参考 Mem0 的退路，新卡冲突时进 Draft Inbox 让人决定。
- **不引入新的存储格式**：仍是 yml + jsonl，不引入 SQLite / vector db。

---

## 八、对工程承诺的兑现

用户原话："我希望我的项目是可以作为基础设施的工程规划强度，要实用"。

本方案在工程层面给出的承诺：

| 承诺 | 怎么兑现 |
|---|---|
| 可观测 | 每张卡的 `extraction.layer_trace` 记录每层耗时与决策 |
| 可 replay | cluster_id + pipeline_version 唯一标识，可重放 |
| 可演化 | schema_version + migration 命令，向后兼容 |
| 可组装 | 5 层独立 trait，可禁用、可换实现、可单测 |
| cheapest first | 前 3 层无 LLM，调 LLM 前已大幅压缩 |
| 失败可见 | rejected_at 字段说明卡在哪层；audit-log 记录决策 |
| 单一真实来源 | activation/kind/scope 严格从 LLM 输出投影，不允许多处写入 |

---

## 九、一句话总结

**之前的方案是"过滤管道"，每层判 reject/accept；新方案是"压缩管道"，每层信息密度递增、cluster 让分散偏好聚合、多 quote 共同支撑一条卡——这才是 Claude Code 五层压缩思想真正的应用，而不是搬具体的 5 层。**

---

## 参考资料

- [Dive into Claude Code: Five-Layer Compaction Pipeline](https://arxiv.org/abs/2604.14228)（核心灵感）
- [VILA-Lab/Dive-into-Claude-Code GitHub](https://github.com/VILA-Lab/Dive-into-Claude-Code)
- [Hierarchical Text Segmentation Chunking for RAG](https://arxiv.org/html/2507.09935v1)（Layer 3 聚类思想）
- [HiChunk Auto-Merge Retrieval](https://arxiv.org/pdf/2509.11552)
- [NEXUSSUM Hierarchical Multi-Agent Summarization](https://aclanthology.org/2025.acl-long.500.pdf)
- [Mem0 ADD-only & Quality Probes](https://mem0.ai/blog/state-of-ai-agent-memory-2026)
