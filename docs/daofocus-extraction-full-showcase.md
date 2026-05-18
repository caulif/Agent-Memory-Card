# DaoFocus 历史对话提炼完整展示

生成时间：2026-05-15

## 输入范围

- 项目根目录：`C:\DaoFocus`
- 历史来源扫描根目录：`C:\Users\15893`
- 扫描到的本地对话来源：703
- 匹配 `C:\DaoFocus` 的来源：120
- 排除其他项目来源：580
- 排除未知项目来源：3
- 导入 observations：101
- 跳过重复或不可用来源：19

## 真实 Provider 运行结果

使用 Anthropic-compatible 第三方 provider 运行五层 pipeline：

```powershell
cargo run --quiet -- pipeline --project 'C:\DaoFocus' --provider anthropic --timeout-secs 300 --save-report --json
```

环境变量使用进程级注入，没有写入仓库：

- `ANTHROPIC_BASE_URL`
- `ANTHROPIC_AUTH_TOKEN`
- `ANTHROPIC_MODEL`

报告文件：

- `C:\DaoFocus\docs\runs\pipeline-20260515T115636.json`

结果摘要：

- observations_in：101
- STRIP kept：100
- TRUNCATE messages：100，其中 28 条长消息被截断
- CLUSTER：33 个簇
- INDUCE：1 accepted / 7 rejected / 0 failed
- LLM calls：9 次
- selected clusters：8 / 33
- CRYSTALLIZE：1 accepted / 0 rejected

注意：fastembed 当前不可用，cluster 自动 fallback 到 jaccard：

```text
[cluster] fastembed unavailable, falling back to jaccard: request error: io: Connection refused
```

## Provider 最终结晶卡

### 1. 执行代码变更后的质量审查并重复审查修复

- cluster_id：`c_15deee77`
- recurrence：8
- kind：procedure
- scope：global
- activation：skill
- confidence：0.90
- temporal_status：stable

正文：

```text
在完成每个代码变更任务后，以及收到修复时，agent 应该执行代码质量审查（仅检查变更代码），并在发现需要修复的问题后，重新执行审查以确认修复已正确完成且未引入新风险；目标是确保代码质量、可维护性、测试充分性和集成安全性，避免回退或新问题。
```

Brief：

```text
用于确保代码质量、可维护性、测试充分性和集成安全性，避免回退或新问题。
```

证据：

```text
obs:codex:sha256:e26f0
Re-run code quality review for Task 1 after fix.
```

```text
obs:codex:sha256:bcfe3
Code quality review for Task 2.
```

```text
obs:codex:sha256:916f4
Review changed code only for quality, maintainability, tests, and integration risk.
```

判断：

这张卡证据真实、复现次数高、scope=global 合理，适合沉淀成按需 Skill，而不是 always-on AGENTS 规则。它说明真实 provider 链路已经能工作，但当前 top-k 召回偏向工程流程，没优先抓住 DaoFocus 产品规则。

## 本地全历史 Candidate 结果

这些 candidate 来自：

- `C:\DaoFocus\.agent-kernel\candidates`

它们是候选，不是已批准 Memory Card；不会自动进入 `AGENTS.md`。

### 1. DaoFocus 经济与修炼数值约束

- 文件：`C:\DaoFocus\.agent-kernel\candidates\project\project-goal-自己搜索-规划-决策-实现下面的功能.yml`
- id：`project:goal-自己搜索-规划-决策-实现下面的功能`
- kind：constraint
- scope：project
- confidence：0.94
- suggested route：always_on_rule
- source：`obs:codex:sha256:e862a`

正文：

```text
/goal 自己搜索，规划，决策，实现下面的功能，优化经济系统和修炼系统的数值，我希望更难进阶，不要太容易，比如每个境界的难度应该是越来越大的，难度应该明显增加，而不是线性增长，然后点击和键盘次数达到固定值只是有概率获得灵石，不管获得还是失败都清空现有统计值
```

可批准改写：

```text
DaoFocus 的经济系统和修炼系统应保持递进难度：境界进阶不能线性变容易，每个境界的难度要明显增加；点击和键盘次数达到阈值后只应有概率获得灵石，并且无论成功或失败都清空本轮统计值。
```

判断：最值得批准。它是明确的 DaoFocus 产品规则，可以直接变成项目级 Memory Card 和回归测试。

### 2. DaoFocus 自主执行但避免过度复杂

- 文件：`C:\DaoFocus\.agent-kernel\candidates\project\project-goal-全自动执行-不要问我了-自主搜索下载.yml`
- id：`project:goal-全自动执行-不要问我了-自主搜索下载`
- kind：constraint
- scope：project
- confidence：0.90
- suggested route：always_on_rule
- source：`obs:codex:sha256:4c868`

正文：

```text
/goal 全自动执行，不要问我了，自主搜索下载好用的工具，自己决策和实现，最后给我成品就行了，注意不要搞的太复杂
```

可批准改写：

```text
在 DaoFocus 开发中，agent 可以自主搜索资料、选择工具、规划并实现到可运行成品；方案要保持克制，不为单个需求引入过度复杂的架构或流程。
```

判断：可批准，但必须保留人工审阅边界。不能解释成“任意高风险操作都无需确认”。

### 3. DaoFocus 拖动交互约束

- 文件：`C:\DaoFocus\.agent-kernel\candidates\project\project-上面的拖动的那个小条可以不要了-因为人物就可以拖.yml`
- id：`project:上面的拖动的那个小条可以不要了-因为人物就可以拖`
- kind：constraint
- scope：project
- confidence：0.90
- suggested route：always_on_rule
- source：`obs:codex:sha256:1e0a3`

正文：

```text
上面的拖动的那个小条可以不要了，因为人物就可以拖动了
```

可批准改写：

```text
DaoFocus 的桌宠拖动应以人物本体作为主要拖动入口；不要额外保留顶部拖动小条，除非用户之后明确要求恢复。
```

判断：可批准。它是具体、稳定、可验收的产品交互规则。

### 4. 全面体验打磨偏好

- 文件：`C:\DaoFocus\.agent-kernel\candidates\global\global-goal-进一步全方面测试功能-界面-并优化.yml`
- id：`global:goal-进一步全方面测试功能-界面-并优化`
- kind：procedure
- scope：global
- confidence：0.80
- suggested route：always_on_rule
- source：`obs:codex:sha256:437ce`

正文：

```text
/goal 进一步全方面测试功能，界面，并优化，直到所有体验都非常好，比如特效就可以再好好做，参考相关背景的的图片或小说等，参考顶级相关产品的标准，达到所有人都想用的程度，自己探索，自己规划，自己实现，自己决策，可以下载必要的东西
```

判断：方向有价值，但 scope=global 偏松。它来自 DaoFocus 产品体验语境，更应该先作为 project-level product taste，除非跨多个项目复现。

### 5. 全面体验打磨偏好重复候选

- 文件：`C:\DaoFocus\.agent-kernel\candidates\global\global-进一步全方面测试功能-界面-并优化-直到所有体验.yml`
- id：`global:进一步全方面测试功能-界面-并优化-直到所有体验`
- kind：procedure
- scope：global
- confidence：0.80
- suggested route：always_on_rule
- source：`obs:codex:sha256:437ce`

正文：

```text
进一步全方面测试功能，界面，并优化，直到所有体验都非常好，比如特效就可以再好好做，参考相关背景的的图片或小说等，参考顶级相关产品的标准，达到所有人都想用的程度，自己探索，自己规划，自己实现，自己决策，可以下载必要的东西
```

判断：应与第 4 条合并，不该并列展示。这里暴露了 candidate merge review 仍不足。

### 6. 先理解项目再全面优化

- 文件：`C:\DaoFocus\.agent-kernel\candidates\global\global-理解现有项目的所有功能-我希望进行全面提升-包括.yml`
- id：`global:理解现有项目的所有功能-我希望进行全面提升-包括`
- kind：procedure
- scope：global
- confidence：0.80
- suggested route：workflow_skill
- source：`obs:codex:sha256:1e0a3`

正文：

```text
理解现有项目的所有功能，我希望进行全面提升，包括引导，ui，特效，用户体验等等方面，发挥发散性思维，找出可以优化的地方
```

可批准改写：

```text
在做产品体验全面提升前，先理解现有功能、交互入口、UI、特效和用户路径，再发散寻找优化点并形成可执行方案。
```

判断：适合 workflow Skill，不适合 always-on。scope 可以是 global，但需要避免把“全面发散”变成未约束的功能膨胀。

### 7. A/B/C 上下文丢失候选

- 文件：`C:\DaoFocus\.agent-kernel\candidates\global\global-我同意-优先做-a-b-的基础体验-再用-c.yml`
- id：`global:我同意-优先做-a-b-的基础体验-再用-c`
- kind：procedure
- scope：global
- confidence：0.80
- suggested route：workflow_skill
- source：`obs:codex:sha256:73791`

正文：

```text
我同意，优先做 A + B 的基础体验，再用 C 把完成反馈补厚
```

判断：应拒绝。A/B/C 指代丢失，不能沉淀成 Memory Card。这条应该加入 Golden Set 的真实失败样例。

### 8. 阶段性“不要加功能”约束

- 文件：`C:\DaoFocus\.agent-kernel\candidates\global\global-保留例外-先局限于现有功能-不要加额外功能-只是.yml`
- id：`global:保留例外-先局限于现有功能-不要加额外功能-只是`
- kind：procedure
- scope：global
- confidence：0.78
- suggested route：always_on_rule
- source：`obs:codex:sha256:0002e`

正文：

```text
保留例外：先局限于现有功能，不要加额外功能，只是提高现有功能的体验，比如各个环节的特效可以再酷炫一点，大胆一点，你可以自由搜索借鉴，然后给我一个规划方案
```

判断：不能批准为 global always-on。它是阶段性约束，应标记为 task-scoped 或 expirable；否则以后会错误阻止合理的新功能。

## 我会批准什么

第一批建议只批准 4 张或 3 张：

1. DaoFocus 经济与修炼数值约束：批准为 project constraint。
2. DaoFocus 拖动交互约束：批准为 project constraint。
3. DaoFocus 自主执行但避免过度复杂：批准为 project collaboration preference，但附带人工审阅边界。
4. 执行代码变更后的质量审查并重复审查修复：批准为 global workflow skill。

暂不批准：

1. 两条“全面体验打磨”：先合并，再降到 DaoFocus project taste。
2. “先理解项目再全面优化”：转 workflow skill 草稿。
3. “A+B/C”：拒绝，原因是上下文丢失。
4. “不要加额外功能”：转 task-scoped / expirable，不进入 always-on。

## 暴露出的系统问题

1. Provider 路径已经可用，但默认 batch 对 DeepSeek Anthropic-compatible 接口不稳，需要 batch failure 后逐 cluster fallback。
2. 真实 provider 最终只产出 1 张 global 工程流程卡，DaoFocus 产品规则召回不足。
3. 本地 candidate 有产品规则召回，但去重、scope、生命周期治理不足。
4. Candidate 和 Draft Inbox 仍割裂：`Candidates written: 8`，但 `draft list` 是空。
5. 当前缺少 merge review：第 4 和第 5 条重复，应该合并展示。
6. 当前缺少 context-loss gate：A/B/C 这种指代丢失候选应该被自动拒绝。
7. 当前缺少 expirable/task-scoped memory：阶段性“不要加功能”不应进入 global always-on。

## 结论

这次从 DaoFocus 全历史里能真实提炼出两类东西：

1. **可落地的 DaoFocus 产品记忆**：经济/修炼数值、拖动入口、自主但克制的实现偏好。
2. **可复用的工程流程记忆**：代码变更后做质量审查，修复后重复审查。

但现在系统还达不到理想效果：provider 版本证据更干净但召回太少；本地版本召回更好但治理不够。下一步最应该做的是把这次 8+1 的结果写进 Golden Set：正例约束可批准卡，负例约束重复候选、scope 误判、上下文丢失、阶段性规则误入 always-on。
