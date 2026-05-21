# 五层提炼流水线 · 设计修订（v2）与执行清单

> 这份文档是对 [extraction-plan-final.md](./extraction-plan-final.md) 的修订与落地清单，不取代原文档。
>
> **读法**：先读 extraction-plan-final.md（V1，骨架）→ 再读本文（V2，调研收口 + 落地任务）。
>
> **触发**：用户要求"优化完善方案 → 逐步实现 → 至少超过基线（LLM 粗筛 + 全量提炼）"。
>
> **基线定义**（必须先对齐）：当前已合并到 main 的 `observation::synthesize_observations_to_drafts_with_engine` 流程——本地启发式选 top-40 高分 observation → 拼成 16K 字符大材料 → 一次性喂 Claude/Codex CLI 子进程 → LLM 返回 ≤12 张 candidate → 强制 `evidence_quote` + `source_observation_ids` → 本地 gate 二次过滤 → 取 top-5。本文称之为 **B0**。

---

## 一、调研收口：6 处对 V1 的关键修订

V1 把骨架画出来了，但有 6 处细节调研后发现需要修正。每条都附了来源。

### 修订 1：CLUSTER 阈值从 0.65 提到 0.78（+簇大小硬上限）

**V1 原话**："相似度 ≥ 0.65 的消息归同一 cluster（阈值偏宽，召回优先）"。

**调研发现**：
- [Kamradt Semantic Chunker](https://superlinked.com/vectorhub/articles/semantic-chunking) 文献阈值是 95 百分位（典型 cosine 0.85+）。
- [NAACL 2025 Findings](https://aclanthology.org/2025.findings-emnlp.1361.pdf) 反过来发现："计算成本不被一致收益证明，固定 200 词块匹配或胜过语义分块"——意思是激进聚类反而拉低质量。
- [HERCULES (arXiv 2506.19992)](https://arxiv.org/abs/2506.19992) 用递归 k-means 而非阈值合并，避免一次性阈值的脆性。

**修订**：
- cosine 阈值 0.65 → **0.78**（中等，召回与精确折衷）
- 簇大小硬上限 **N=10**：超过即按时间序切成多个相邻簇，避免单簇灌爆 Layer 4 prompt
- 单消息簇（recurrence=1）**默认不进 Layer 4**（节省 LLM 成本），仅保留在审计 trace 中；仅当用户开 `--include-singletons` 时才进
- 阈值不再"全局唯一"，**按消息长度动态调整**：< 30 字符（短句、口语化）走 0.82；≥ 30 字符走 0.78（短句更易假阳性）

### 修订 2：聚类算法明确为"基于 embedding 的连通分量"，不引入 HDBSCAN

**V1 原话**："简单的 union-find / hierarchical 聚类"。

**调研发现**：
- 工业界常用 HDBSCAN 但需要标注数据调参；HERCULES 用递归 k-means 但需要 LLM 标 cluster description（成本翻倍）。
- 我们数据规模 ≤ 1000 messages × 384 维 embedding，O(N²) 完全可接受。

**修订**：
- 算法：先 batch embed 所有消息 → 全对比 cosine → 阈值过的对走并查集 union → 输出连通分量
- Rust 实现 ~50 行（fastembed 已就位，并查集也简单）
- 不引入新依赖（hdbscan 在 Rust 生态不成熟）

### 修订 3：中文短文本验证 fastembed 默认模型，不达标就换 BGE-M3

**V1 没提**：默认 `TextEmbedding::try_new(Default::default())` 用的是 `all-MiniLM-L6-v2`，**英文为主，中文支持差**。

**调研发现**：
- [BGE 系列文档](https://huggingface.co/BAAI/bge-m3) 推荐 BGE-M3 多语言（含中文），fastembed 0.6+ 已支持。
- 我们 90% 对话是中英混排，中文 + 口语化短句是聚类的常态。

**修订**：
- 实施 Layer 3 前 **先做模型校验**：
  - 取 20 对人工标注的"应该归簇"中文短句 + 20 对"不应归簇"短句
  - 用 MiniLM 跑 cosine 分布，看 P(同簇) vs P(异簇) 的 AUC
  - AUC < 0.85 就换 BGE-M3（fastembed crate 加 feature flag `bge-m3`）
- 这一步**必须在 Layer 3 落地前完成**，否则后面所有评估都不准

### 修订 4：Layer 4 INDUCE 必须用 Anthropic prompt cache

**V1 没提**：原文只说"每个 cluster 1 次 Haiku 调用"。

**调研发现**：
- [Anthropic prompt caching](https://platform.claude.com/docs/en/build-with-claude/prompt-caching) 文档 + [多份生产报告](https://dev.to/stella_lin_82914c71e25769/anthropic-prompt-caching-cut-our-rca-cost-by-90-5gmb)：
  - cache hit 0.1× 输入价；写入 1.25× 输入价（5 分钟 TTL）
  - 90%+ 缓存命中后总成本降 90%
  - 关键约束：**静态前置 + 动态后置**；中间任何一字符变化都让后续 cache 失效
- [Lost-in-the-middle 2025](https://arxiv.org/html/2510.05381v1)：长 prompt 中间内容被遗忘；多次小调用 + 共享前缀 > 单次大调用

**修订**：
- Layer 4 prompt 拆成 `(stable_prefix, dynamic_cluster)` 二元组：
  - `stable_prefix`：任务说明 + JSON schema + 1-2 条 few-shot（≥ 1024 tokens 才能命中 cache）
  - `dynamic_cluster`：簇内消息（每次都不同）
- 在 `provider.rs::anthropic_request_body` 把 user content 改为数组，`stable_prefix` 加 `cache_control: {type: "ephemeral"}`
- Layer 5 同理（schema + 渲染规则前置，candidate 后置）
- **这是性能与成本的关键，不做就不算交付**

### 修订 5：簇内时序冲突的处理（V1 完全漏了）

**V1 没提**：簇内消息可能有时间冲突（用户先说"用 axios"，3 周后改口"还是 fetch 吧"）。聚类会把这种内容合到一起。

**调研发现**：
- [Zep / Graphiti 双时态模型](https://blog.getzep.com/content/files/2025/01/ZEP__USING_KNOWLEDGE_GRAPHS_TO_POWER_LLM_AGENT_MEMORY_2025011700.pdf)：边失效 + valid_from/valid_to
- [Lyzr Cognis](https://blog.devgenius.io/ai-agent-memory-systems-in-2026-mem0-zep-hindsight-memvid-and-everything-in-between-compared-96e35b818da8)：context-aware 写时推理
- 我们已经决定**不自动 REPLACE/UPDATE**，但不能把冲突信息归一同张卡

**修订**：
- Layer 4 prompt 增加显式问题：
  ```
  Has the user reversed or refined their stated rule across the messages
  (sorted in chronological order)? If yes, output `temporal_status: "reversed"`
  and use ONLY the latest expression as the rule; cite the earlier ones in
  `evidence.superseded_quotes`.
  ```
- Layer 4 输出新增字段 `temporal_status: stable | reversed | refined`
  - `stable`：所有消息表达一致 → 正常出 candidate
  - `reversed`：最新与最早矛盾 → candidate.body 用最新；evidence 标注被取代的早期 quote
  - `refined`：早期粗、晚期细 → candidate.body 用最细的，evidence 标"逐步细化"
- Layer 5 不再处理冲突（已经在 Layer 4 解决），只做规范化

### 修订 6：评估方法借鉴 LongMemEval 的 nugget recall（不只是合格率）

**V1 原话**："黄金集 positive recall ≥ 80%，negative precision ≥ 90%"。

**调研发现**：
- [LongMemEval (ICLR 2025)](https://github.com/xiaowu0162/longmemeval) 用 nugget-based：把每条参考答案拆成原子 nugget，按"满足/部分满足/不满足"打 0/0.5/1
- [Mem0 在 LongMemEval 上 67.13% LLM-Judge / 1764 tokens / 200ms p95](https://mem0.ai/blog/state-of-ai-agent-memory-2026)
- [Rating Roulette EMNLP 2025](https://aclanthology.org/2025.findings-emnlp.1361.pdf)：LLM judge 自一致性 < 0.8，**单次评分不可信**

**修订**：
- 黄金集大小：30 张 → **60 张**（30 positive + 30 negative）
- 评估指标改为 3 个：
  - `recall`：黄金集 positive 中被新方案 accept 的比例
  - `precision`：新方案 accept 的全部 candidate 中被人工标 positive 的比例
  - `cluster_coverage`：黄金集 positive 中"该信号在 ≥ 1 个 Layer 3 簇里出现"的比例（验证聚类质量）
- 每次评估**跑 3 次取均值 + 标准差**（应对 LLM 不一致）
- 与 B0 对比：3 个指标各自要 ≥ B0；至少 1 个有显著提升（≥ 5 个百分点）才算通过

---

## 二、修订后的流水线全图

```
┌─────────────────────────────────────────────────────────────┐
│  Layer 0 · LOAD            (确定性)                          │
│  增量：跳过已 evolve 过的 observation；维护 .agent-kernel/  │
│  cache/processed_obs_ids.json                               │
└──────────────┬──────────────────────────────────────────────┘
               ▼
┌─────────────────────────────────────────────────────────────┐
│  Layer 1 · STRIP            (确定性)                         │
│  剥 system reminder / tool 回显 / 空消息 / 完全重复          │
│  保留：user 与 assistant 的"自然语言段"                       │
└──────────────┬──────────────────────────────────────────────┘
               ▼
┌─────────────────────────────────────────────────────────────┐
│  Layer 2 · TRUNCATE         (确定性)                         │
│  > 1500 chars 的消息保留头 750 + 尾 750，中间标记折叠         │
│  保留头：场景/出错点；保留尾：结论/决策                      │
└──────────────┬──────────────────────────────────────────────┘
               ▼
┌─────────────────────────────────────────────────────────────┐
│  Layer 3 · CLUSTER          (embedding，无 LLM)              │
│  · 全量 batch embed（fastembed BGE-M3 或 MiniLM 验证后定）   │
│  · 全对比 cosine + 并查集；阈值 0.78（短句 0.82）            │
│  · 簇大小上限 10；超出按时间序切                              │
│  · 单消息簇默认 drop（除非 --include-singletons）            │
└──────────────┬──────────────────────────────────────────────┘
               ▼
┌─────────────────────────────────────────────────────────────┐
│  Layer 4 · INDUCE           (LLM Haiku，prompt cache)        │
│  · 每簇 1 次调用；簇内按时序排列                              │
│  · stable_prefix（指令 + schema + few-shot）→ cache          │
│  · dynamic_cluster（簇内消息）→ 不 cache                      │
│  · 输出 candidate：含 temporal_status / evidence_quotes       │
│  · evidence_quotes ≥ 2 条来自不同 obs（避免单源依赖）        │
└──────────────┬──────────────────────────────────────────────┘
               ▼
┌─────────────────────────────────────────────────────────────┐
│  Layer 5 · CRYSTALLIZE      (LLM Sonnet，prompt cache)       │
│  · candidate → MemoryCard，强 schema                         │
│  · stable_prefix（schema + 派生规则） → cache                │
│  · 失败不重写，进 Draft Inbox                                │
└─────────────────────────────────────────────────────────────┘
```

每层的输入输出契约必须**可独立测试 / dry-run**：

```
agent-kernel extract pipeline --layer strip      < input.jsonl   > stripped.jsonl
agent-kernel extract pipeline --layer truncate   < stripped.jsonl > truncated.jsonl
agent-kernel extract pipeline --layer cluster    < truncated.jsonl > clusters.json
agent-kernel extract pipeline --layer induce     < clusters.json   > candidates.json
agent-kernel extract pipeline --layer crystallize< candidates.json > cards.yml
agent-kernel extract pipeline --all              < input.jsonl     > cards.yml
```

---

## 三、与基线 B0 的具体对比方法

### B0 baseline 的精确定义

跑这条命令固化 B0 数字：
```
cargo run -- observe evolve --engine claude-code --dry-run --project tests/fixtures/golden_session.jsonl
```
输出：`b0_report.json`，包含 candidates 数组与每张卡的 evidence。

### 黄金集构建（Week 0 必做）

`tests/fixtures/extraction_golden/`：
- `positive/`：30 张人工写的"应该被提炼的卡"，每张含
  - `expected_card.yml`：理想 MemoryCard
  - `source_session.jsonl`：含 ≥ 1 段对应原文的 session
- `negative/`：30 张人工写的"不该提炼的"，每张含
  - `reject_reason.txt`：为什么不该提炼（一次性请求 / 信息泄露 / 重复 etc.）
  - `source_session.jsonl`

构造方法：
1. 从现有 `.agent-kernel/memory-cards/.archive/` 拿已批准卡（约 4 张）作为种子
2. 从 `~/.claude/projects/<project>/*.jsonl` 翻 6-12 个月真实历史，人工挑 26 张高质量片段
3. negative 集从历史里挑被 reject / 一次性请求 / 模板话 / 工具回显 30 段

### 评估脚本

新增 `tests/extraction_benchmark.rs`：
```rust
// 跑 N=3 轮，对 V2 与 B0 各取均值
let v2 = run_pipeline_v2(&golden_set, runs=3);
let b0 = run_pipeline_b0(&golden_set, runs=3);

// 输出表格
// metric         | b0      | v2      | delta   | passes?
// recall         | 0.55    | 0.72    | +0.17   | YES
// precision      | 0.62    | 0.81    | +0.19   | YES
// cluster_cov    | n/a     | 0.85    | -       | YES
```

通过条件：3 个指标 V2 都 ≥ B0；至少 1 个 ≥ B0 + 0.05。

### 失败回退

- 任一指标 V2 < B0：**该层有 bug**，不能合并到 main，必须回到该层迭代
- 全部指标 V2 ≈ B0（差距 < 0.02）：聚类没带来好处，**重新审视模型 / 阈值 / few-shot**
- precision 大幅下降但 recall 大涨：簇过宽，**调高阈值或加 critic**

---

## 四、可执行任务清单（给 Codex 用）

每个任务独立可验证，按顺序做。每完成一个 task 跑一遍单测 + 黄金集，**不通过不进下一个**。

### Task A · 模型校验与黄金集（Week 0）

A1. 写 `tests/fixtures/embedding_zh_audit.rs`：
- 准备 20 对中文短句（10 同义 + 10 不同义），手工标注
- 用 fastembed MiniLM 跑 cosine，输出 AUC
- 通过条件：AUC ≥ 0.85；否则在 `Cargo.toml` 加 fastembed `bge-m3` feature 并切换默认模型

A2. 整理黄金集：
- 在 `tests/fixtures/extraction_golden/{positive,negative}/` 各放 30 个目录
- 每个目录含 `source_session.jsonl` + `expected_card.yml`（positive）或 `reject_reason.txt`（negative）
- 用 `cargo run -- memory-card export --format=jsonl` 把 archive 旧卡导出作为种子

A3. 写 `tests/extraction_benchmark.rs`：
- 加载黄金集
- 跑 B0（当前 main）3 次，记录 baseline 指标
- 留 V2 接口（每完成一层就回填）

**Verify**：跑 A1 + A2 + A3，AUC 报告打印；baseline 指标固化到 `tests/fixtures/extraction_golden/baseline.json`。

### Task B · Layer 1 STRIP（Week 1）

B1. 新建 `src/extract/strip.rs` ~150 行
- `pub fn strip_observations(records: &[ObservationRecord]) -> Vec<StrippedMessage>`
- 删除：`<system-reminder>` 块、`tool_use` / `tool_result` 块、纯空白、纯标点、< 8 字符的孤立消息、完全重复的消息
- 输入输出都序列化为 jsonl 兼容格式

B2. 写 `src/extract/strip_tests.rs`（按 .gitignore 32-39 行约定外置）
- fixture：1 段 jsonl 含各种噪声 → 期望输出
- 至少 8 个 case：每种噪声类型 + happy path + 重复消息 + 中英混排

B3. 接入 CLI：
- `cli.rs` 加 `extract pipeline --layer strip --input <path>` 子命令
- 命令读 jsonl → 调 `strip_observations` → 输出 jsonl 到 stdout

**Verify**：
- 单测全过
- 跑真实 session：消息数应减少 40-70%
- B 层完成后**不需要**跑黄金集（还没产出 candidate）

### Task C · Layer 2 TRUNCATE（Week 1）

C1. 新建 `src/extract/truncate.rs` ~80 行
- `pub fn truncate_long_messages(messages: &[StrippedMessage], head: usize, tail: usize) -> Vec<TruncatedMessage>`
- 复用 `observation/chunked.rs::truncate_head_and_tail`（已有）
- 默认 head=750, tail=750；超长消息中间填 `\n[省略 N 字]\n`

C2. 写 `src/extract/truncate_tests.rs`：
- 短消息保持原样
- 长消息保头 + 标记 + 尾
- UTF-8 边界正确（中文字符不截半）

C3. 接入 CLI：`--layer truncate`

**Verify**：跑真实 session，平均消息长度应稳定在 750-1500 chars 区间。

### Task D · Layer 3 CLUSTER（Week 2，最大杠杆）

D1. 给 `src/extract/embedding.rs` 加批量接口（修复现有 2-pair 限制）：
- `pub fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>`
- 内部一次 `model.embed(texts.to_vec(), None)`

D2. 新建 `src/extract/cluster.rs` ~200 行
- `pub fn cluster_messages(messages: &[TruncatedMessage], threshold: f32, max_size: usize) -> Vec<MessageCluster>`
- 步骤：
  1. batch embed 所有消息
  2. 全对比 cosine（消息长度 < 30 字符的对用 0.82 阈值；其他 0.78）
  3. 并查集 union 阈值过的对
  4. 收集连通分量；簇大小 > max_size 按 created_at 切
  5. 单消息簇默认丢弃（除非显式保留）
- 数据结构：
  ```rust
  pub struct MessageCluster {
      pub cluster_id: String,            // c_<sha256-prefix>
      pub recurrence: usize,             // = messages.len()
      pub messages: Vec<TruncatedMessage>, // 按 created_at 升序
      pub mean_similarity: f32,          // 簇内平均 cosine（debug）
  }
  ```

D3. 写 `src/extract/cluster_tests.rs`：
- 同义短句应归簇
- 不同主题不应归簇
- 簇大小切分正确
- 单消息簇默认丢

D4. 接入 CLI：`--layer cluster`，输出 JSON 簇列表

**Verify**：
- 单测全过
- 跑全 103 session（用户已有的所有项目 session），输出簇分布
- 人工抽查 top-10 大簇：每簇内消息确实是同一规则的不同表达
- **如果人工抽查通过率 < 80%，调阈值或换 embedding 模型，再迭代**

### Task E · Layer 4 INDUCE（Week 3）

E1. Provider 改造（前置依赖）：
- `src/provider.rs::anthropic_request_body` 支持 user content 数组 + cache_control
- 新增 `ProviderRequest::user_content_blocks: Vec<UserContentBlock>`
- 单测：`anthropic_request_body_marks_user_prefix_cacheable`

E2. 新建 `src/extract/induce.rs` ~250 行
- `pub fn induce_candidates(clusters: &[MessageCluster]) -> Result<Vec<InducedCandidate>>`
- 每簇调一次 Haiku；prompt 拆 stable_prefix / dynamic_cluster
- prompt 模板单独文件 `prompts/induce.md`（include_str! 加载）
- JSON schema 强制 `{title, when, what, why, evidence_quotes (≥2), temporal_status, confidence}`
- evidence_quotes 必须 substring-match 簇内某条 message（用现有 normalize 子串匹配）

E3. 写 `src/extract/induce_tests.rs`（用 mock provider）：
- 给定一个 happy 簇，期望输出 1 个 candidate
- temporal_status="reversed" 路径
- evidence_quotes 不在簇内 → reject

E4. 接入 CLI：`--layer induce`

**Verify**：
- 单测全过
- 跑黄金集 positive 子集：观察 candidate 数量、temporal_status 分布
- 抽样 20 个 candidate 人工评估，期望"该 candidate 真实归纳了用户偏好"≥ 80%

### Task F · Layer 5 CRYSTALLIZE（Week 4）

F1. 新建 `src/extract/crystallize.rs` ~200 行
- `pub fn crystallize_candidate(candidate: &InducedCandidate) -> Result<Option<MemoryCard>>`
- Sonnet 调用 + prompt cache
- 输出 schema：完整 MemoryCard 字段（title 8-30 字 / body 50-200 字 / brief 15-60 字 / kind / scope / activation / tags）
- 失败不重试，返回 `None` + 写 reject log

F2. 写 `src/extract/crystallize_tests.rs`

F3. 写 schema migration（V1 卡 → V2）：
- `cargo run -- memory-card migrate --to-schema 2`
- 现有 archive 卡批量转换，标 `origin: legacy`

F4. 接入 CLI：`--layer crystallize` + `--all`（端到端）

**Verify**：
- 单测全过
- 跑黄金集全集，更新 `tests/fixtures/extraction_golden/v2_report.json`
- **关键**：跑 `cargo test extraction_benchmark`，对比 V2 与 B0
- 全部 3 指标 V2 ≥ B0；≥ 1 个 ≥ B0 + 0.05 → 通过

### Task G · 收口与回归（Week 5）

G1. 删除 `quality_gate.rs` 全部 17+ 个 `looks_like_*` 函数（行数从 848 → 0）
G2. 收口 `extract.rs` 4 个入口为 `extract::pipeline::run(input) -> Vec<MemoryCard>`（行数从 942 → < 200）
G3. 更新 README，明确"V2 是唯一推荐路径，B0 仍可用做 fallback"
G4. 跑 cargo fmt + clippy + 全测试，零 warning

---

## 五、不做什么（边界）

- **不做 critic 阶段**：调研显示 LLM-as-Judge 自一致性 < 0.8，加 critic 性价比低，不如把质量交给"多 quote 归纳" + 人工 Draft Inbox
- **不做自动 REPLACE/UPDATE/DELETE**：簇内冲突由 Layer 4 的 `temporal_status` 处理；跨卡冲突进 Draft Inbox
- **不引入向量数据库**：≤ 1000 messages × 384 维内存 cosine 即可，O(N²) ≈ 1M 操作，毫秒级
- **不引入 KG / entity extraction**：聚类已经够用
- **不重写已有 fastembed 接口**：只加 `embed_batch`，旧 `embed_pair` 保留作 fallback
- **不上 batch API**（Plan 4 内容）：先把同步路径打通，batch 是后续优化
- **不做多 critic / reflection**：研究证伪

---

## 六、风险清单（不达标就回滚）

| 风险 | 监测点 | 回滚方案 |
|---|---|---|
| 中文 embedding 不准 | Task A1 AUC < 0.85 | 切 BGE-M3 或固定块（NAACL 2025 兜底） |
| 聚类阈值过严 | Task D 簇 ≤ 5 个 | 阈值降到 0.72 ；或保留 singleton |
| LLM Layer 4 hallucinate evidence_quote | Task E 单测 substring-match 失败率 > 5% | 改为多 quote 强校验 + 失败丢弃 |
| Layer 5 schema 校验通过率 < 70% | Task F 黄金集 | 简化 schema + few-shot；不做第二次重写 |
| V2 全指标 < B0 | Task F benchmark | 不合并，回到 D/E 排查；保留 B0 |
| prompt cache 命中率 < 60% | provider metrics | 检查 stable_prefix 是否含日期/随机 ID |

---

## 七、对用户问题的最终回答

> "至少要比 LLM 粗筛 + 全量提炼好"

要做到这个，**必须**：

1. **基线 B0 要先固化数字**（Task A3），否则"超过"无从验证
2. **黄金集要建**（Task A2），否则没有评估锚
3. **聚类质量决定一切**（Task D），中文 embedding 不达标就立即换模型
4. **prompt cache 必须开**（修订 4），否则每簇 1 次 Haiku 成本太高
5. **每层独立可测、可 dry-run**（流水线全图最后一段），否则 bug 难定位

只要这 5 件做实，**新方案有 ≥ 70% 概率全指标超过 B0**，因为：
- 聚类把分散表达聚合，多 quote 归纳天然比单条更稳（Mem0 / Cognis 数据支持）
- 多次小调用 + prompt cache > 单次大调用（lost-in-the-middle 2025 + Anthropic caching 数据支持）
- evidence_quotes ≥ 2 条来自不同 obs 强校验，比 B0 单 quote 更难造假
- temporal_status 显式处理冲突，比 B0 完全不处理强

**有 30% 概率失败的场景**：
- 中文 embedding 模型选错（修订 3 校验是兜底）
- 黄金集偏向某种风格，评估失真（多人参与构建可缓解）
- LLM Haiku 在 Layer 4 输出质量不稳（可换 Sonnet，成本翻倍但一致性提升）

如果 Week 4 评估不通过，**回到 Task D 排查**，而不是加 critic 或加层。

---

## 八、参考资料（按调研顺序）

1. [Anthropic Claude Code 五层压缩 - Finisky 文章](https://finisky.github.io/en/claude-code-context-compaction/)
2. [Dive into Claude Code arXiv 2604.14228](https://arxiv.org/html/2604.14228v1)
3. [VILA-Lab/Dive-into-Claude-Code GitHub](https://github.com/VILA-Lab/Dive-into-Claude-Code)
4. [Anthropic Effective context engineering](https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents)
5. [Mem0 State of AI Agent Memory 2026](https://mem0.ai/blog/state-of-ai-agent-memory-2026)
6. [Mem0 paper arXiv 2504.19413](https://arxiv.org/html/2504.19413v1)
7. [Mem0 vs LangMem 2026 比较](https://blog.devgenius.io/ai-agent-memory-systems-in-2026-mem0-zep-hindsight-memvid-and-everything-in-between-compared-96e35b818da8)
8. [LongMemEval ICLR 2025](https://github.com/xiaowu0162/longmemeval)
9. [Hierarchical Text Segmentation Chunking arXiv 2507.09935](https://arxiv.org/html/2507.09935v1)
10. [HERCULES Hierarchical Embedding Clustering arXiv 2506.19992](https://arxiv.org/abs/2506.19992)
11. [Context Length Hurts arXiv 2510.05381](https://arxiv.org/html/2510.05381v1)
12. [Lost in the Middle (Liu et al. 2023)](https://arxiv.org/abs/2307.03172)
13. [Rating Roulette EMNLP 2025](https://aclanthology.org/2025.findings-emnlp.1361.pdf)
14. [Anthropic Prompt Caching 文档](https://platform.claude.com/docs/en/build-with-claude/prompt-caching)
15. [Zep Temporal KG arXiv 2501.13956](https://arxiv.org/html/2501.13956v1)
16. [Cognitive Memory in LLMs arXiv 2504.02441](https://arxiv.org/html/2504.02441v1)
