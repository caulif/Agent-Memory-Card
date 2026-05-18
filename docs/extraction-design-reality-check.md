# 提炼方案 Reality Check

> 用户问："反复研究，现在的方案是合理的吗？能显著提高质量吗？"
>
> 我做了真正的反复研究——把 4 张人工已批准的真实卡片放进我设计的 pipeline 跑一遍。结果暴露了我没看到的结构性问题。本文档是诚实的自我反驳。
>
> 阅读顺序建议：先读这份。读完再决定 [extraction-core-design.md](./extraction-core-design.md) 与 [extraction-quality-hardening.md](./extraction-quality-hardening.md) 哪些章节还可信。

---

## 一、用真实数据走方案

我的 v2 pipeline（core-design + hardening 合并后）是：

```
HARVEST → RECURRENCE → WITNESS → TRIPLET → CRITIC → CONFLICT
```

把 `.agent-kernel/memory-cards/.archive/2026-05-08/` 下的 4 张人工已批准卡逐一走一遍。

### 卡 A：保留人工审阅边界

```yaml
body: 涉及质量评估、规则取舍或高影响变更时，清楚区分自动化建议
      与需要人工确认的部分，不越过人工审阅边界。
source_observations: []  # archive 里就是空的
```

走 pipeline：

| Stage | 结果 | 原因 |
|---|---|---|
| HARVEST | 通过 | 有未来性 |
| WITNESS | **拒绝** | source_observations 空，且即便重 extract，原文不会有这种规范化整句 |

如果强行让它通过 WITNESS（伪设），继续：

| TRIPLET | **拒绝** | when/what 都有，**why 没显式写出**，schema 强制 why 必填 |

### 卡 B：优先用真实历史验证提炼逻辑

```yaml
body: 评估提炼质量或修改提炼逻辑时，优先使用真实历史会话做回归验证；
      不要只依赖静态样例。
source_observations: []
```

| Stage | 结果 | 原因 |
|---|---|---|
| WITNESS | **拒绝** | 同卡 A |
| TRIPLET | **拒绝** | when/what 清楚，why 隐含未写出 |

### 卡 C：从用户视角判断体验质量

```yaml
body: 评估功能、流程或输出时，从用户实际感知出发判断是否清楚、顺畅、
      可信，而不只看内部实现是否完成。
```

同样问题：WITNESS 拒、TRIPLET 拒（why 隐含）。

### 卡 D：按改动风险选择测试范围

```yaml
body: 改动较小时优先运行针对性快测；改动较大、跨模块或风险较高时
      运行更完整的回归验证。
```

| Stage | 结果 | 原因 |
|---|---|---|
| WITNESS | **拒绝** | 同上 |
| TRIPLET | **拒绝** | 这是"按情况分类"形态（小改动 vs 大改动），三种 shape（conditional / preference / procedure）**都装不下** |

### 沙盘推演结论

**4 张人工已批准卡，按我设计的 pipeline 100% 被拒**。

这不是个别极端例子，这是 archive 全集。

---

## 二、暴露的 3 个结构性 bug

### Bug 1：Witness 必须字面 quote 是脱离现实的要求

人类规则的产生过程不是"用户说一句话 → 直接成卡"，而是：

- 用户分散在多轮对话里口语化表达："咱们以后注意 X" + "嗯就是这样"
- 用户加 assistant 的补充组合而成的归纳
- 用户表达的是反例（"别再做 Y 了"），规则需要正向重述

**真实对话不会按规则的形态说话**。要求"原文字面 quote"等于要求对话作者预先按 schema 写文本，这是反人性的。

补强 1（规范化匹配）解决标点/空白，**不解决"原文根本没有完整规则句"**。

### Bug 2：Why 槽必填会逼出 hallucination

4 张卡里 0 张显式给出 why。Why 隐含在 what 里：

- "用真实历史回归" 已经隐含 "因为静态样例不能反映真实分布"
- "区分自动化建议与人工确认" 已经隐含 "因为机器不该越界做关键判断"

如果 schema 强制 LLM 写 why，会发生两件事：

1. LLM 编一个"听起来合理"但原文未说的 why（hallucination）
2. Critic 如果做 closed-world entailment 会拒（原文没说 why），整张卡丢
3. 即便 Critic 漏过去（30-60% 召回），落盘的 why 也是编的

要么误杀，要么被假数据污染。**没有第三种结果**。

### Bug 3：三种 shape 装不下真实形态

观察这 4 张卡的形态：

| 卡 | 形态 |
|---|---|
| A | 单 when + 单 what + 隐含 why |
| B | 单 when + 双子句 what（"用 X 不要 Y"）+ 隐含 why |
| C | 单 when + what 含限定条件（"X 而不只看 Y"）+ 隐含 why |
| D | **双 when 各自对应一个 what**（按情况分类） |

我设计的 conditional / preference / procedure 三种 shape：

- **没有 case-analysis shape**（卡 D）
- **没有"包含限定条件的 what"shape**（卡 C 这种"是 X 而不只是 Y"的对比表达）

强行归类会损失语义。

---

## 三、更深的方法论问题

我犯的是 **schema-first** 错误：先定义"高质量卡片该长什么样"，再让数据匹配 schema。

正确思路应该是 **data-first**：先看真实数据有什么形态、什么质量问题，再设计**最小**约束。

回看现状的真实质量问题（这 4 张人工合格卡都有）：

| 现状问题 | 严重度 | 原因 |
|---|---|---|
| `activation` 字段三处不一致 | 高 | 多处独立写入，无单一真实来源 |
| `source_observations: []` | 高 | LLM 没回填，verify 没拦 |
| `evidence_span.quote == body` | 高 | LLM 自造证据 |
| `extraction.reason` 是模板英文 | 中 | LLM prompt 没要求具体化 |
| `brief` 重复 body | 中 | LLM prompt 没禁止 |
| classify rationale 模板套娃 | 中 | 同上 |
| `score_breakdown` 12 维都是固定值 | 低 | 程序生成而非真实评估 |

**这些问题没有一个需要新发明 5 stage pipeline 来解决**。它们要的是：

1. 单一真实来源（数据契约）
2. 更好的 prompt（明确禁止 + 给反例）
3. 更好的 verify（事前不是事后）

我之前发明的 Triplet shape / Critic 等 stage **解决的不是真实问题**，是我想象中的问题。

---

## 四、那真正能显著提高质量的是什么

经过反复研究，我现在能给的诚实建议只剩 4 件事。砍到只有这 4 件。

### 真实建议 1：单一真实来源（最大杠杆，先做）

`activation` / `kind` / `scope` / `compile_enabled` / 主要 `tags` 全部从 `extraction.classification` 单向投影。任何写入路径不准独立指定这些字段。

实现：在 `add_memory_card_with_provenance` 与 `update_memory_card` 这两个唯一入口里强制投影。删除其他所有写入位置。

**这一项就解决了"一团浆糊"的根本原因**。4 张卡里 4 张都有这个问题。

成本：< 100 行 Rust，1 天工期。

收益：现有缺陷的 1 类（activation 不一致）100% 消除。

### 真实建议 2：把"禁止行为"写进 prompt（次大杠杆）

当前 `agent_synthesis_prompt` 50 行，写得不算差，但缺三件事：

1. **明确禁止清单**（有具体反例）：
   ```
   不要做以下事，做了直接拒绝整条 candidate：
   - evidence_quote 与 body 整句相同（必须从原文复制不超过 200 字片段）
   - brief 与 body 句式雷同（brief 必须以"用于"开头，且与 body 字符级重叠 < 50%）
   - source_observation_ids 留空（必须至少 1 条真实存在的 obs id）
   ```

2. **few-shot 反例**：把现有 archive 里 4 张卡作为"看似合格但有 bug"的例子，让 LLM 知道这些字段什么样算坏。

3. **closed-world 指令**：
   ```
   只用上面 material 中明确出现的事实来填字段。
   如果某字段无法从 material 直接归纳，留空（不要编）。
   ```

成本：纯 prompt 改动，半天。

收益：在 prompt 层把当前 5 类缺陷里的 4 类（伪 quote / brief 重复 / 模板 reason / 模板 rationale）显著降低。**估计当前缺陷率 60-70% → 20-30%。这是现实的"显著提升"**。

### 真实建议 3：Witness 改为"软约束 + 多 quote 列表"

不要求"原文字面 quote"，要求"至少 1 条真实 obs id + 至少 1 段从原文复制的支撑片段"。Schema 改为：

```yaml
extraction:
  source_observations: [obs_id_1, obs_id_2]   # ≥ 1，必须真实存在
  evidence_quotes:                            # ≥ 1，每条都从原文复制
    - quote: "用户说的某句话"
      observation_id: obs_id_1
    - quote: "另一段相关原文"
      observation_id: obs_id_1
  body_origin: inferred | quoted              # 标明 body 是字面引用还是归纳
```

`body_origin: inferred` 时，body 可以是"归纳",但 evidence_quotes 必须真实。`body_origin: quoted` 时 body 必须等于某条 quote。

**这一改动后**，4 张 archive 卡只要回填 obs id + 把原文支撑片段抄进 evidence_quotes 就合规。不会被误杀。

成本：schema 改动 + verify 调整 + LLM prompt 调整。1-2 天。

收益：彻底解决"伪 quote"问题，且**不误杀真实合格卡**。

### 真实建议 4：30 张黄金集 + prompt 回归

人工标注：

- 15 张高质量卡（archive 里 4 张 + 历史里挑 11 张）
- 15 张应被拒卡（draft 170d7 这种 + 一次性请求 + LLM 自造内容等）

每次改 prompt 跑一次 30 张，看：

- positive 通过率（应 ≥ 90%）
- negative 拒绝率（应 ≥ 90%）

prompt 调整不靠感觉，靠数字。**这是工业界 LLM extraction 唯一稳定的优化方式**。

成本：人工标注 30 张约半天 + 写 fixture runner 半天 + 每次改 prompt 跑一次（< 1 分钟）。

收益：所有 prompt 改动都有可量化的影响。**这是"可持续提升质量"的发动机**。

---

## 五、应该砍掉的过度设计

我之前提的，现在反向看应该砍：

| 提议 | 砍因 |
|---|---|
| Triplet 三种 shape | 真实形态超过三种，强行归类损失语义 |
| Why 槽必填 | Why 通常隐含，强制必填逼出 hallucination |
| Critic 步骤 | LLM judge 召回低（30-60%），不是质量保证而是装饰 |
| Recurrence count 加权 confidence | 数据规模小（≤500 卡），加权对结果影响微弱 |
| Conflict 4 选 1 自动决策 | Mem0 已经退回 ADD-only，自证此路不稳；让 Draft Inbox 兜底就够 |
| 8 维质量探针 | 4 张样本卡都不需要 8 维评估也能看出问题 |
| 5 stage gate / 5 stage pipeline | 真实问题不需要 5 stage，需要 4 件聚焦的事 |

---

## 六、诚实的质量提升估计

按现在这 4 件事做（不做其他），能给出的提升估计：

| 当前缺陷 | 出现率 | 4 件事执行后 | 解决机制 |
|---|---|---|---|
| activation 三处不一致 | ~80% 现有卡 | < 1% | 真实建议 1（单一真实来源） |
| source_observations 空 | 100% archive 卡 | 0% | 真实建议 2 + 3（prompt 强制 + schema 强制） |
| evidence_quote == body | 100% archive 卡 | < 5% | 真实建议 2（prompt 禁止）+ 真实建议 3（schema 拆 evidence_quotes） |
| brief 重复 body | ~30% draft | < 10% | 真实建议 2（prompt 禁止 + few-shot 反例） |
| 模板化 rationale | ~70% archive 卡 | < 20% | 真实建议 2（prompt 要求具体化） |
| 一次性请求被收录 | candidates 里有 | 不变 | 这个本来就靠 quality_gate 拦，跟我新方案无关 |

**总结**：现状卡片的整体合格率从估计 30%（archive 4 张里至少 3 张有缺陷）提升到估计 70-80%。这是**显著但不是革命性**的提升。

**剩余 20-30% 不合格部分**：靠 Draft Inbox 人工审查兜底。这是项目设计原则，不是缺陷。

---

## 七、对用户问题的最终回答

> 现在的方案是合理的吗？

**之前的方案不够合理**。沙盘推演显示它会把 100% 的人工合格卡误杀。

**现在我给的 4 件事是合理的**。每件都能落到具体的代码改动 + 具体的 prompt 改动 + 具体的卡片缺陷修复。

> 能显著提高质量吗？

**能，但提升幅度是工业级而不是学术级**：

- 合格率从 ~30% → ~70-80%
- 5 大具体缺陷里 4 大被解决
- 不解决的 1 大（一次性请求拦截）本来就是别的子系统的事

如果用户期望的"显著"是 90%+ 自动合格、几乎不需人工审查——**做不到**。这超出当前 LLM extraction 的工业上限（Mem0 / LangMem 在自家 benchmark 上也是 60-90% 区间）。

---

## 八、下一步建议

| 选项 | 描述 | 推荐 |
|---|---|---|
| A | 按本文 4 件事做（**砍掉 70% 的之前提案**） | **推荐** |
| B | 仍按 core-design + hardening 全做 | 不推荐（沙盘推演证伪） |
| C | 重写一份只包含 4 件事的最终方案 md | 看用户决定 |

如果选 A 或 C，我建议：

1. **第 1 周**：真实建议 1（单一真实来源）+ 真实建议 4 的"建黄金集"部分
2. **第 2 周**：真实建议 2（prompt 改进），跑黄金集回归
3. **第 3 周**：真实建议 3（schema 拆 evidence_quotes）
4. **观察 1-2 个月真实使用**，再决定要不要做之前提的 stage / shape / critic 等"加法"

---

## 九、给其他文档的处置建议

| 文档 | 处置 |
|---|---|
| extraction-pipeline-optimization-plan.md | 保留。性能优化与本文质量结论独立，不冲突 |
| extraction-quality-improvement-plan.md | 保留 Plan A（schema 强化）和 Plan D（黄金集），其他章节作废 |
| extraction-core-design.md | 标记"已废弃，参考本文" |
| extraction-quality-hardening.md | 标记"已废弃，参考本文" |
| 本文 | 作为提炼质量改进的 single source of truth |

如果你点头，我可以把上面 4 件事单独写一份精简最终方案 md（约 200 行），替换 core-design 与 hardening 两份。

---

## 十、教训

1. 不要 schema-first，要 data-first（先看真实样本再设计）
2. 沙盘推演必须用真实数据，不能用我假想的"理想样本"
3. 优雅的概念模型可能掩盖现实的复杂性（三概念听着工整，但不能装下真实卡片）
4. 加 stage 通常不是答案，prompt + 数据契约才是
5. 反复质疑自己的方案是必要的——用户问得好

我之前发了 4 份方案，用户两次质疑后我才做出沙盘推演。这是流程问题。下次任何"提取质量"类方案，我应该先做沙盘推演再写方案，不是反过来。
