# 记忆卡片提炼链路优化方案

> 适用范围：`agent-kernel` Rust 后端，从 `cargo run -- observe evolve` / `cargo run -- extract` 进入的 Memory Card 提炼全链路。
>
> 角色约定：本文档由 Claude 起草，作为规划文档供 Codex / 人工实施时参考。Claude 不直接写实现代码，每个 Plan 内的「任务拆解」都是给实现者的工作清单。

## 现状快照（动手前对齐）

提炼链路当前的关键瓶颈：

| # | 瓶颈 | 证据 | 影响 |
|---|---|---|---|
| 1 | LLM 调用全串行 | 全项目 `tokio::spawn` / `join_all` / `FuturesUnordered` / `Semaphore` 0 命中 | N 个 candidate 处理时间 = N × 单次延迟 |
| 2 | embedding 一次只算 2 个 | `src/extract/embedding.rs:47-62` 每次 lock model 后只 `embed(vec![left, right])` | M 对相似度比较 = 2M 次推理 |
| 3 | 无 embedding 持久化缓存 | `.agent-kernel/` 下无 cache 目录 | 同 body 在多个 stage 反复 embed |
| 4 | 无 observation 级结果缓存 | 同上 | 重跑 evolve 全部从头算 |
| 5 | 多 role 各发一次 API | `draft.rs:687`、`extract/llm.rs:317`、`extract/llm.rs:410`、`extract/abstract.rs:86`、`extract/refine.rs:95` 五处独立调用 | 单候选 RTT × N |
| 6 | chunked synthesis 串行 | `synthesis_material_chunks` 切 K 个 chunk 但外层 for 循环 | K × 60s timeout 串行 |
| 7 | prompt cache 仅覆盖 system | `provider.rs:548-561` 只给 system 加 `cache_control` | cache 命中率上限被压死 |
| 8 | 统一用 Opus | `~/.claude/settings.json` 把 Sonnet/Haiku/Opus 全部映射到 `claude-opus-4-7[1m]` | 简单任务也走最贵模型 |

## 全局约束

- 任何改动必须保留 `cargo fmt --check` 和 `cargo clippy --quiet -- -D warnings` 通过。
- 提交前禁止把 `~/.claude/settings.json` 中的 API key 写进任何项目内文件、日志或测试 fixture。
- 所有新文件优先放在已有子模块目录（`src/extract/`、`src/observation/`、`src/provider/`），避免再让 `src/` 顶层膨胀。
- 触碰测试时遵循 `.gitignore` 第 32-39 行的"测试不上传公开仓库"约定（用 `#[path]` 引外部 `*_tests.rs` 文件，而不是把 `mod tests` 内联进主文件）。

---

## Plan 0：度量基线

**为什么放在最前面**：所有后续 Plan 都需要"改之前的数字"才能证明收益。先把指标埋好，再改代码。

### 目标

在 `.agent-kernel/metrics.jsonl` 持续输出端到端 / 子阶段 / Provider / Embedding / Dedup 五类指标，可通过 CLI 子命令 `agent-kernel metrics show` 汇总。

### 前置依赖

无。这个 Plan 是其它所有 Plan 的前置。

### 涉及文件

- 新增：`src/metrics.rs`（核心模块，预计 < 200 行）
- 修改：`src/lib.rs`（注册 `pub mod metrics;`）
- 修改：`src/observation.rs`（在 `evolve_local_conversations_with_engine` 入口/出口埋点）
- 修改：`src/extract.rs`（在 `extract_text_to_drafts` 等入口埋点）
- 修改：`src/provider.rs`（`call_provider_for_role` 出口埋 `provider.call_count` / `cache_read_input_tokens` / `cache_creation_input_tokens`）
- 修改：`src/extract/embedding.rs`（`embed_pair` 内埋 `embedding.compute_count`）
- 修改：`src/extract/dedupe.rs` 或调用 `SemanticDeduper` 的位置（埋 `dedup.skipped_by_embedding`）
- 修改：`src/cli.rs`（新增 `metrics show` 子命令）

### 任务拆解

1. 设计 `MetricEvent` 结构：`{ timestamp, kind: String, data: serde_json::Value }`，全部以 jsonl 追加方式写入 `.agent-kernel/metrics.jsonl`。
2. 实现 `metrics::record(event_kind: &str, data: serde_json::Value)`，内部用 `OnceLock<Mutex<File>>` 保证多线程下顺序追加。
3. 在 6 个埋点位置调用 `metrics::record`：
   - `evolve.start` / `evolve.end`（带 duration_ms、candidates_in、candidates_out、source）
   - `provider.call`（带 role、provider_name、input_tokens、output_tokens、cache_read_input_tokens、cache_creation_input_tokens、duration_ms）
   - `embedding.compute`（带 batch_size、duration_ms）
   - `embedding.cache_hit` / `embedding.cache_miss`（Plan 1 落地后才会有 hit）
   - `dedup.skip`（带 reason: "cosine"|"jaccard"|"llm-judge"）
   - `extract.candidate_emit`（带 stage、kind、scope、confidence）
4. CLI 增加 `metrics show --project . --since 24h`，汇总输出 p50/p95 latency、cache hit rate、provider call count by role。

### 验证方式

- 跑一次 `observe evolve --dry-run --project .`，`.agent-kernel/metrics.jsonl` 至少出现 `evolve.start`、`evolve.end`、若干条 `provider.call`、`embedding.compute`。
- `metrics show` 能正确解析并打印汇总。
- 单元测试：`metrics::record` 在并发写入下不丢条目（spawn 100 线程各 record 100 条，最后行数 = 10000）。

### 风险与回滚

- 风险：埋点 IO 拖慢主流程。缓解：record 路径走 `try_lock`，失败直接丢弃（指标允许有损）。
- 回滚：把所有 `metrics::record` 调用注释掉，删除 `src/metrics.rs`，文件变化集中且可逆。

---

## Plan 1：缓存与批量化（Quick Wins）

**对应原 T1 + T2 + T4**

**为什么放第一**：改动小、风险低、收益直接体现在 latency 和成本上。3 个任务可以串行实施，每个落地后立刻看 `metrics.jsonl` 确认收益。

### 目标

- embedding 单次批量从 2 提到「按 candidate × existing_card 一次算完」。
- 同 body 终身只算一次 embedding（磁盘缓存）。
- Anthropic prompt cache 命中范围从「仅 system」扩展到「system + 稳定 schema/instruction」。

### 前置依赖

Plan 0 已落地（要看指标）。

### 涉及文件

- 修改：`src/extract/embedding.rs`（增加 `embed_batch`，改造 `FastEmbedMatcher`）
- 新增：`src/extract/embedding_cache.rs`（磁盘缓存层）
- 修改：`src/extract/dedupe.rs` 或 `SemanticDeduper::dedup_against_existing` 调用方（用 batch 接口）
- 修改：`src/provider.rs::anthropic_request_body`（user prompt 拆 system-stable / dynamic 两段，前段加 `cache_control`）
- 新增：`src/extract/embedding_cache_tests.rs`（按 `.gitignore` 约定单独文件）

### 任务拆解

#### T1.1 embedding 批量接口

1. 给 `FastEmbedMatcher` 增加 `pub fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>`，内部一次 `model.embed(texts.to_vec(), None)`。
2. 给 `SemanticMatcher` trait 新增 `compute_similarity_batch(&self, query: &str, candidates: &[String]) -> Vec<f32>`，default 实现是循环调旧接口（保 jaccard 不破）。
3. `SemanticDeduper::dedup_against_existing` 改为：先 collect 所有 existing body 进 `Vec<String>`，调一次 `compute_similarity_batch(&new_body, &existing_bodies)`，再循环判 threshold。
4. 旧的 `embed_pair` 保留供 jaccard fallback 路径使用，避免破坏现有测试。

#### T1.2 embedding 磁盘缓存

1. 新建 `src/extract/embedding_cache.rs`：
   - 接口：`pub fn get_or_compute<F: FnOnce(&[String]) -> Result<Vec<Vec<f32>>>>(texts: &[String], compute: F) -> Result<Vec<Vec<f32>>>`
   - key = `sha256(text)` 取前 2 字节做目录分片，剩余作文件名，内容是 `Vec<f32>` 的小端序二进制
   - 路径：`.agent-kernel/cache/embeddings/<aa>/<bb>/<sha256>.bin`
   - 命中：直接读盘 → `metrics::record("embedding.cache_hit", ...)`
   - 未命中：把所有未命中的 texts 一次传给 compute，写盘后返回
2. `FastEmbedMatcher::embed_batch` 改为先走 cache，未命中部分才进模型。
3. 新增 `src/extract/embedding_cache_tests.rs`，测试：
   - 第一次调用 compute 被调用 N 次，第二次调用 0 次
   - 不同 text 不串话
   - 文件损坏时降级到重新计算（不能 panic）

#### T1.3 Anthropic prompt cache 扩边界

1. 重新审视 `src/provider.rs::anthropic_request_body` 中 user prompt 的构造。当前是单字符串。
2. 把所有 ProviderRole 的 prompt 模板改造为 `(stable_prefix: String, dynamic_suffix: String)` 二元组：
   - `stable_prefix` = JSON schema + 任务指令 + few-shot 示例（不随 candidate 变化）
   - `dynamic_suffix` = candidate body / observation material（每次都不同）
3. 在 anthropic body 里把 user content 改为数组：
   ```json
   "content": [
     {"type": "text", "text": "<stable_prefix>", "cache_control": {"type": "ephemeral"}},
     {"type": "text", "text": "<dynamic_suffix>"}
   ]
   ```
4. 单测：`anthropic_request_body_marks_user_prefix_cacheable`（参考现有 `anthropic_request_body_marks_system_prompt_cacheable` 的写法）。

### 验证方式

- 落地 T1.1：跑 50 个 candidate × 30 张 existing card 的 stress test，`embedding.compute` 次数应从 ~1500 降到约 ~50（每 candidate 一次 batch）。
- 落地 T1.2：清空 `.agent-kernel/cache/embeddings/` 跑两次 evolve，第二次 `embedding.cache_hit` 应占 90% 以上，end-to-end duration 至少减半。
- 落地 T1.3：Anthropic provider 第二次调用 `provider.call` 事件中 `cache_read_input_tokens` 应明显大于 0。

### 风险与回滚

- T1.2 风险：缓存目录占盘。缓解：每个 embedding ~1.5KB（384 维 × 4 字节 + 头），万张卡 ~15MB，可接受。可在 README 提示用户 `.agent-kernel/cache/` 可随时删除。
- T1.3 风险：prompt 拆分后如果 stable_prefix 不稳定（含日期、随机 id 等）反而每次都 cache miss。缓解：单测里强制拼接两次相同输入，校验前缀字节级相等。
- 整体回滚：每个 T1.x 都是独立 commit，逐个 revert 即可。

---

## Plan 2：模型路由与提示合并

**对应原 T5 + T3 + T10**

**为什么放第二**：在缓存生效后，进一步压缩 cost 与 latency；多模型路由能 60-80% 降本，多 role 合并能把单候选的 RTT 从 3-5 次降到 1 次。

### 目标

- 不同 role 走不同模型（classify/judge → Haiku，refine/synthesize → Sonnet，难任务才 Opus）。
- 把 classify + judge + refine 三步合并成一次 LLM 调用。
- 所有 Anthropic 调用打开 `structured-outputs-2025-11-13` beta，强制 JSON schema 校验。

### 前置依赖

Plan 0 + Plan 1。需要 metrics 验证收益，需要 prompt cache 已扩边界（合并后 prompt 更长，对 cache 依赖更高）。

### 涉及文件

- 修改：`~/.claude/settings.json`（拆分 Sonnet/Haiku/Opus 模型映射；注意：这个文件全局生效，建议先在 `.claude/settings.local.json` 项目级覆盖测试）
- 修改：`src/provider.rs`（增加 `provider_name_for_role` 的多模型映射逻辑，目前已有此函数但需检查其语义）
- 修改：`src/extract/llm_pipeline_impl.rs`（合并三 role 的 prompt 与响应解析）
- 修改：`src/extract/llm.rs`（删除或简化 `Update` / `Judge` 单独调用路径）
- 修改：`src/extract/refine.rs`（refine 不再单独调 LLM，结果从合并响应里取）
- 修改：`src/draft.rs:687` 周边（同上）
- 修改：`src/provider.rs::anthropic_beta_header`（确保 `structured-outputs-2025-11-13` 永久启用）
- 新增：`src/extract/llm_pipeline_combined_tests.rs`

### 任务拆解

#### T2.1 多模型路由配置

1. 在 `ProviderConfig` 增加 `role_model_overrides: BTreeMap<ProviderRole, String>`，默认空。
2. `call_provider_for_role` 在解析 model 时，先查 `role_model_overrides`，命中则覆盖默认。
3. 推荐默认配置（写到 README 或 quickstart.md）：
   - `Judge` → `claude-haiku-4-5`
   - `Abstract` → `claude-sonnet-4-6`
   - `Refine` → `claude-sonnet-4-6`
   - `Update` → `claude-sonnet-4-6`
   - 默认（synthesize）→ `claude-opus-4-7`
4. 单测：构造两份 config，断言不同 role 路由到不同 model。

#### T2.2 三 role 合并

1. 设计合并 prompt schema：
   ```json
   {
     "classification": { "kind": "...", "scope": "...", "activation": "..." },
     "judgement": { "should_keep": true, "confidence": 0.85, "reason": "..." },
     "refined": { "title": "...", "body": "...", "brief": "...", "tags": [...] }
   }
   ```
2. 把 `extract/llm_pipeline_impl.rs` 的主入口改为单次 `call_provider_for_role(Role::Combined, ...)`，max_tokens 设为原三次的总和约 2048。
3. 解析响应，分发到原本的下游 stage 函数（不需要改下游接口，只改上游入口）。
4. 保留分离调用模式作为 fallback：响应 schema 校验失败 → 退回到分三次调。
5. 测试：用相同输入跑合并 vs 分离两条路径，断言 classification 与 refined.body 字段一致或相似度 > 0.95。

#### T2.3 强制 structured outputs

1. `anthropic_beta_header` 函数已经写过 `structured-outputs-2025-11-13`，确认 `body.get("output_config")` 的判断在所有 Anthropic 调用上为 true。
2. `ProviderRequest` 必须始终设 `json_schema`（当前是 `Option`，多处传 None）。
3. 把所有 `Option<ProviderJsonSchema>` 改为必填或在 helper 内默认填充。
4. 跑现有 `extract/methodology_tests.rs` / `provider.rs` 的所有 anthropic 相关测试，确保 beta header 都正确拼装。

### 验证方式

- T2.1：`metrics show` 中按 role × provider/model 分组的调用数应分散到 Haiku/Sonnet/Opus 三档。
- T2.2：跑 50 candidate baseline，合并后 `provider.call_count` 应降到约 1/3；指标 `extract.candidate_emit` 数量与基线差异 < 5%（行为等价）。
- T2.3：刻意构造异形 LLM 输出（缺字段、JSON 不闭合）灌入 mock provider，观察 `parse_agent_candidates` 不再失败、或 fallback 路径正确兜住。

### 风险与回滚

- T2.1 风险：Haiku 在中文 reasoning 任务上能力下降。缓解：先用现有 fixture（`extract/methodology_tests.rs`、`extract/preference.rs` 测试样例）做 A/B；不达标的 role 留在 Sonnet/Opus。
- T2.2 风险：单 prompt 太长挤占 max_tokens。缓解：合并 prompt 上限设硬约束（input < 8K tokens），超过则自动退回分离模式。
- T2.3 风险：beta header 在某些 region/endpoint 不支持。缓解：失败重试时移除 beta header 走标准路径。

---

## Plan 3：并发与增量

**对应原 T6 + T7 + T8 + T9**

**为什么放第三**：Plan 1 + 2 把单次调用做轻了，Plan 3 把"调用次数"也压下来（增量）+ "总墙钟时间"压下来（并发）。这是 latency 的最大跳变点。

### 目标

- candidate 之间并发跑 LLM（受限并发度）。
- chunk 之间并发跑 synthesize。
- 同 observation 重复 evolve 时跳过 LLM。
- 重复内容在 LLM 之前用 embedding 早期过滤。

### 前置依赖

Plan 0 + Plan 1（embedding cache 已落地，T9 才有意义）+ Plan 2（合并调用让并发性价比更高）。

### 涉及文件

- 修改：`src/provider.rs::call_provider_for_role`（改 async 或包裹 `spawn_blocking`）
- 修改：`src/observation.rs::synthesize_with_agent_engine` 调用处（chunk 间并发）
- 修改：`src/extract.rs::extract_text_to_drafts` 等入口（candidate 间并发）
- 新增：`src/observation/synthesis_cache.rs`（observation 级结果缓存）
- 修改：`src/observation.rs::evolve_local_conversations_from_files_with_engine`（接入 cache）
- 修改：`src/extract.rs`（新增"早期 embedding filter"stage，在 LLM classify 之前）
- 新增：`src/extract/early_filter.rs`（cosine 早过滤逻辑）

### 任务拆解

#### T3.1 Provider 调用 async 化

1. 评估两种方案：
   - 方案 A：彻底改 `call_provider_for_role` 为 `async fn`，reqwest 用 async 客户端（`reqwest::Client`，去掉 `blocking`）。改动大但符合长期方向。
   - 方案 B：保留 sync 接口，并发处用 `tokio::task::spawn_blocking` 包一层。改动小但占 OS 线程。
2. 推荐方案 B（小步快跑），后续 Plan 4 重构时再做方案 A。
3. 在 `call_provider_for_role` 上方新增 `pub async fn call_provider_for_role_async(...)`，内部 `spawn_blocking` 调用 sync 版本。
4. 单测：并发 spawn 10 个 task，断言全部完成且无 panic。

#### T3.2 candidate-level 并发

1. 在 `extract_text_to_drafts` 走 LLM 路径的位置（即 `extract_llm_text_to_drafts`），把对 candidate 的循环改为：
   ```rust
   let semaphore = Arc::new(Semaphore::new(5));
   let futures = candidates.into_iter().map(|c| {
       let permit = semaphore.clone().acquire_owned();
       async move {
           let _permit = permit.await?;
           process_candidate_async(c).await
       }
   });
   let results: Vec<_> = futures::stream::iter(futures)
       .buffer_unordered(5)
       .collect()
       .await;
   ```
2. 并发度根据 provider 类型动态调整：
   - `Anthropic` direct → permit = 5
   - `ClaudeCli` / `CodexCli` → permit = 2（子进程开销大）
   - `LocalHeuristic` → 不走并发，保持原 sync
3. 顺序敏感的合并步骤（按 confidence 排序、final dedup）放到所有并发完成后再做。
4. 测试：50 candidate 并发跑，断言失败的 candidate 不影响其他成功的，最终输出数量正确。

#### T3.3 chunk 间并发 synthesize

1. 找到 `synthesis_material_chunks` 的调用点（搜 `synthesis_material_chunks(`）。
2. 把外层 for 循环改为 `buffer_unordered(3)` 并发（`ClaudeCli` 限 2）。
3. 各 chunk 的候选合并后，统一过本地 gate（不要每个 chunk 单独过，避免重复过滤）。
4. 注意保留确定性：合并后按 `(confidence desc, title asc)` 排序再 truncate(5)，与现有 `synthesize_with_agent_engine:586-592` 一致。

#### T3.4 observation 级增量缓存

1. 新建 `src/observation/synthesis_cache.rs`：
   - key = `sha256(observation_id ++ "\n" ++ prompt_template_version ++ "\n" ++ model_name)`
   - value = 序列化的 `Vec<AgentMemoryCardCandidate>`
   - 路径：`.agent-kernel/cache/synthesis/<sha256-prefix>/<sha256>.json`
   - TTL：默认 7 天，超过则视为 miss
2. 接入点：`evolve_local_conversations_from_files_with_engine` 在调 `synthesize_with_agent_engine` 之前先查 cache，命中则直接走下游 gate；未命中正常调 LLM 后写盘。
3. CLI 增加 `--no-cache` flag 强制重算。
4. 测试：跑两次 evolve，第二次 `provider.call_count` 应降为 0；带 `--no-cache` 时恢复为正常调用次数。

#### T3.5 早期 embedding filter

1. 新建 `src/extract/early_filter.rs`：
   - 接口：`pub fn filter_by_embedding_distance(candidates: Vec<Candidate>, existing_cards: &[MemoryCardRecord], thresholds: EarlyFilterThresholds) -> (Vec<Candidate>, Vec<EarlyFilterDecision>)`
   - thresholds：`high_skip = 0.95`（直接判 SKIP）/ `low_pass = 0.7`（直接通过到 LLM）
2. 在 `extract.rs::extract_high_value_candidates_with_preferences` 之前插入这一 stage。
3. 命中 high_skip 的 candidate：跳过后续 LLM 调用，记 `dedup.skip` (reason="early-embedding")。
4. 0.7 ~ 0.95 之间的 candidate：标记为"需要 LLM 仲裁"，进入合并 prompt（Plan 2 T2.2 合并的那个 prompt 增加 `merge_decision` 字段，让 LLM 输出 CREATE/MERGE/SKIP）。
5. 测试：构造 1 个与 existing card 完全相同的 candidate（cosine ≈ 1.0）+ 1 个完全无关 candidate，断言前者被早期过滤、后者照常进入 LLM。

### 验证方式

- T3.1+T3.2：50 candidate 端到端时间应从串行的 N×T 降到约 N/5×T（理想），实际看 metric `evolve.duration_ms`。
- T3.3：3 个 chunk 的端到端时间约等于最慢单 chunk 时间。
- T3.4：第二次 evolve 时间下降 70% 以上，`provider.call_count` 接近 0。
- T3.5：高重复场景下 LLM 调用次数显著下降，质量基线（fixture 测试）不回归。

### 风险与回滚

- T3.1/T3.2 风险：spawn_blocking 池打满导致死锁。缓解：tokio 默认 blocking pool 是 512 线程，permit=5 远低于上限；监控 `tokio_runtime` 指标。
- T3.4 风险：缓存陈旧导致用户改了 prompt 但不生效。缓解：cache key 里包含 `prompt_template_version`，prompt 改动时 bump 版本号即可全量失效。
- T3.5 风险：embedding 误判导致丢弃了有价值的新 candidate。缓解：threshold=0.95 已经非常保守；落地后用 metrics 抽样检查 `dedup.skip(reason=early-embedding)` 的卡是否真的与 existing 重复。
- 回滚：每个子任务独立 commit，按依赖反向 revert。

---

## Plan 4：架构升级（择期）

**对应原 T11 + T12 + T13**

**为什么放最后**：这些是大改动，价值不在"今天的 latency/cost"，在"未来的可维护性与可扩展性"。前面三个 Plan 不依赖 Plan 4 即可独立交付。

### 目标

- 离线 evolve 走 Anthropic Message Batches API（成本 -50%）。
- `extract.rs` / `observation.rs` 顶层瘦身到 < 500 行，链路用单一 `ExtractionPipeline` 入口收口。
- prompt 模板从代码内嵌字符串外置到 `prompts/*.md`，支持本地 override。

### 前置依赖

Plan 0 + Plan 1 + Plan 2 + Plan 3 全部落地，且至少跑过 1 个月真实使用收集到性能与质量基线。

### 涉及文件

- 新增：`src/provider/batch.rs`（Message Batches API 客户端）
- 修改：`src/cli.rs`（增加 `--mode batch` 选项）
- 修改：`src/observation.rs::auto_evolve_registered_projects`（dispatch 到 batch 模式）
- 重构：`src/extract.rs`（拆出 `src/extract/pipeline.rs`，定义 trait `ExtractionStage`，把 classify/dedup/llm/quality/refine 各成 stage）
- 重构：`src/observation.rs`（拆出 `src/observation/orchestrator.rs`，把 evolve 主流程从顶层文件下沉）
- 新增：`prompts/agent_synthesis.md`（从 `src/observation/agent_engine_impl.rs:130-173` 迁出）
- 新增：`prompts/refine.md`、`prompts/classify.md`、`prompts/judge.md`
- 修改：所有 prompt 调用点（用 `include_str!` 加载默认；运行时检查 `prompts/*.local.md` 覆盖）

### 任务拆解

#### T4.1 Anthropic Batches API

1. 新建 `src/provider/batch.rs`：
   - 接口：`pub async fn submit_batch(requests: Vec<BatchRequest>) -> Result<BatchId>` / `poll_batch(id: BatchId) -> Result<BatchStatus>` / `fetch_results(id: BatchId) -> Result<Vec<BatchResult>>`
   - 内部走 `https://api.anthropic.com/v1/messages/batches`
2. CLI `auto-evolve` 增加 `--mode batch`：先把所有 candidate 序列化成 batch 请求，提交后写 `.agent-kernel/cache/batches/<batch_id>.json`，CLI 立即返回。
3. 增加 `cargo run -- batch poll <batch_id>` 子命令，命令完成后回填候选。
4. 配合 Plan 1 T1.3 的 prompt cache，cache_control 用 1h TTL（batch 处理时间通常超过 5 分钟）。

#### T4.2 ExtractionPipeline 收口

1. 定义 trait：
   ```rust
   #[async_trait]
   pub trait ExtractionStage: Send + Sync {
       fn name(&self) -> &'static str;
       async fn run(&self, ctx: &mut PipelineContext) -> Result<()>;
   }
   ```
2. 把现有 `extract.rs` 中的逻辑拆成至少 6 个 stage：`PreferenceLoader` / `RecurrenceBoost` / `EarlyEmbeddingFilter` / `LlmRouter` / `QualityGate` / `Dedup` / `Refine`.
3. `Pipeline::new(stages)` + `Pipeline::run(input)` 单一入口，所有 4 个 `extract_*_to_drafts` 入口都改为构造不同 stage 组合。
4. 单测：mock 各 stage，断言 stage 顺序、错误传播、metric 上报正确。
5. 顺手把 `extract.rs`、`observation.rs` 的行数压到 < 500（`memory_card.rs` 同思路压到 < 1000）。

#### T4.3 prompt 模板外置

1. 新建 `prompts/` 目录（项目根目录），加入 `.gitignore` 例外（确保模板被跟踪）。
2. 把 `agent_engine_impl.rs:130-173` 的 raw string 移到 `prompts/agent_synthesis.md`。
3. 加载逻辑：
   ```rust
   fn load_prompt(name: &str) -> String {
       let local_path = format!("prompts/{}.local.md", name);
       if let Ok(text) = std::fs::read_to_string(&local_path) {
           return text;
       }
       match name {
           "agent_synthesis" => include_str!("../prompts/agent_synthesis.md").to_string(),
           "refine" => include_str!("../prompts/refine.md").to_string(),
           // ...
           _ => panic!("unknown prompt: {name}"),
       }
   }
   ```
4. 把 `*.local.md` 加进 `.gitignore`，让用户能在本地迭代 prompt 不污染仓库。
5. README 增加章节"如何调优 prompt"。

### 验证方式

- T4.1：跑 100 个 candidate 的 `auto-evolve --mode batch`，cost（按 metrics 计算）应降到原 sync 模式的 ~50%；处理时间增加但在可接受范围（< 24h）。
- T4.2：`extract.rs` 行数从 942 降到 < 500；stage trait 单测覆盖率 > 80%。
- T4.3：`prompts/agent_synthesis.local.md` 被识别并覆盖默认 prompt；删除该文件后回到默认行为。

### 风险与回滚

- T4.1 风险：batch 模式下用户失去实时反馈。缓解：仅用于 `auto-evolve` 这种后台命令，UI 路径仍走 sync。
- T4.2 风险：trait 抽象不到位反而增加心智负担。缓解：先做 6 个具体 stage，跑一段时间再决定是否引入 generic param、依赖注入等高级抽象。
- T4.3 风险：模板文件改动后 cache key 失效（Plan 3 T3.4），导致整轮缓存失效。缓解：模板外置时同步引入 prompt 文件 sha256 进 cache key，让 prompt 调整自动失效旧缓存。

---

## 实施时间线建议

```
Week 1:  Plan 0  (度量基线)                                  必做
Week 2:  Plan 1  (T1.1 → T1.2 → T1.3)                       高 ROI
Week 3:  Plan 2  (T2.1 → T2.3 → T2.2)                       中等 ROI
Week 4:  Plan 3  (T3.1 → T3.2 → T3.3)                       并发主线
Week 5:  Plan 3  (T3.4 → T3.5)                              增量与早期过滤
Week 6+: 观察一段真实使用，再决定是否启动 Plan 4
```

每个 Plan 完成后必须看 `metrics show` 的对比数据，没有数字就不算完成。

## 不在本规划范围内的事

- 切换向量库（LanceDB / Qdrant）：当前 fastembed + 内存 cosine 在 < 500 张卡场景下没有瓶颈，引入向量库的运维成本不划算。
- LLM 语义缓存（不同于 embedding cache）：material 长尾分布严重、命中率低、且引入语义近似的质量风险。
- 跨 provider 的 cache 共享：Anthropic 的 cache 是 provider/region 隔离的，不要假设 Bedrock / Vertex 之间能共享。

## 参考资料

- [Anthropic Prompt Caching](https://platform.claude.com/docs/en/build-with-claude/prompt-caching)
- [Anthropic Message Batches](https://platform.claude.com/docs/en/build-with-claude/batch-processing)
- [Lessons from building Claude Code: Prompt caching is everything](https://claude.com/blog/lessons-from-building-claude-code-prompt-caching-is-everything)
- [hermes-agent #509 Cognitive Memory Operations encoding pipeline](https://github.com/NousResearch/hermes-agent/issues/509)
- [memory-lancedb-pro two-stage dedup pattern](https://github.com/CortexReach/memory-lancedb-pro)
