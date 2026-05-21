# DaoFocus 历史对话提炼效果展示

生成时间：2026-05-15

## 输入范围

- 项目根目录：`C:\DaoFocus`
- 历史来源扫描：`C:\Users\15893`
- 扫描到的本地对话来源：694
- 匹配 `C:\DaoFocus` 的来源：120
- 排除其他项目来源：571
- 排除未知项目来源：3
- 导入 observations：101
- 跳过重复/不可用来源：19

## 运行命令

```powershell
cargo run --quiet -- observe replay --project 'C:\DaoFocus' --home $env:USERPROFILE --target codex --target claude-code
```

正式运行后写入：

- `C:\DaoFocus\.agent-kernel\observations`
- `C:\DaoFocus\.agent-kernel\candidates`

注意：这次写入的是 candidate，不是已批准 Memory Card；不会自动进入 AGENTS.md，也不会自动同步给 agent。

## 当前提炼结果

### 1. DaoFocus 数值系统难度约束

- ID：`project:goal-自己搜索-规划-决策-实现下面的功能`
- Scope：project
- Kind：constraint
- Confidence：0.94
- Suggested route：always_on_rule
- Source：`obs:codex:sha256:e862a`

内容：

> 优化经济系统和修炼系统的数值。境界进阶不要太容易，每个境界难度应逐步、明显增加，而不是线性增长。点击和键盘次数达到固定值后只应有概率获得灵石；无论成功或失败，都清空现有统计值。

我的判断：这是最像“可批准项目 Memory Card”的一条。它有明确项目语义、长期约束、可转化为测试和验收标准。

### 2. DaoFocus 工作模式偏好：自主实现但别过度复杂

- ID：`project:goal-全自动执行-不要问我了-自主搜索下载`
- Scope：project
- Kind：constraint
- Confidence：0.90
- Suggested route：always_on_rule
- Source：`obs:codex:sha256:4c868`

内容：

> 在 DaoFocus 开发中，可以自主搜索、下载合适工具、自己决策和实现，最后交付成品；但不要把方案做得过度复杂。

我的判断：项目级协作偏好，能用，但需要改写得更稳：它不该覆盖高风险操作的人类审阅边界。

### 3. DaoFocus 交互设计：不要额外拖动条

- ID：`project:上面的拖动的那个小条可以不要了-因为人物就可以拖`
- Scope：project
- Kind：constraint
- Confidence：0.90
- Suggested route：always_on_rule
- Source：`obs:codex:sha256:1e0a3`

内容：

> 上方额外的拖动小条可以移除，因为人物本身已经可以拖动。

我的判断：这是非常具体的产品交互记忆。它更适合 project Memory Card，或者直接变成 UI 回归测试/验收项。

### 4. 全面体验打磨偏好

- ID：`global:goal-进一步全方面测试功能-界面-并优化`
- Scope：global
- Kind：procedure
- Confidence：0.80
- Suggested route：always_on_rule
- Source：`obs:codex:sha256:437ce`

内容：

> 进一步全面测试功能、界面和体验。特效应参考相关背景图片、小说和顶级相关产品标准，目标是达到“所有人都想用”的程度；agent 可以自主探索、规划、实现和决策，并下载必要资源。

我的判断：方向对，但 scope 被判成 global 有争议。它明显来自 DaoFocus 的体验打磨语境，更应该是 project-level product taste，除非证据显示用户在多个项目反复提出。

### 5. 先补基础体验，再补完成反馈

- ID：`global:我同意-优先做-a-b-的基础体验-再用-c`
- Scope：global
- Kind：procedure
- Confidence：0.80
- Suggested route：workflow_skill
- Source：`obs:codex:sha256:73791`

内容：

> 优先做 A + B 的基础体验，再用 C 把完成反馈补厚。

我的判断：这条太依赖上下文，当前提炼丢失了 A/B/C 指代，不能批准。它应该被 Golden Set 标记为失败样例：代词/占位符未解析时不能沉淀。

### 6. 全面理解现有项目后再优化

- ID：`global:理解现有项目的所有功能-我希望进行全面提升-包括`
- Scope：global
- Kind：procedure
- Confidence：0.80
- Suggested route：workflow_skill
- Source：`obs:codex:sha256:1e0a3`

内容：

> 在提升项目时，先理解现有项目的所有功能，再从引导、UI、特效、用户体验等方面发散找优化点。

我的判断：这条可泛化为工作流 Skill，但要附带“先读项目、再提出/实施改进”的边界。当前还行，但不应直接变 always-on。

### 7. 重复候选：全面体验打磨偏好

- ID：`global:进一步全方面测试功能-界面-并优化-直到所有体验`
- Scope：global
- Kind：procedure
- Confidence：0.80
- Suggested route：always_on_rule
- Source：`obs:codex:sha256:437ce`

内容：

> 与第 4 条几乎重复：全面测试功能、界面和体验，参考背景资料和顶级产品，把特效和整体体验做到足够吸引人。

我的判断：这是重复聚类未合并。当前系统应该把第 4、7 条聚成一个 merge review，而不是并列展示两个候选。

### 8. 保持现有功能范围，只提升体验

- ID：`global:保留例外-先局限于现有功能-不要加额外功能-只是`
- Scope：global
- Kind：procedure
- Confidence：0.78
- Suggested route：always_on_rule
- Source：`obs:codex:sha256:0002e`

内容：

> 在特定阶段先局限于现有功能，不新增额外功能，只提升现有功能体验；例如让各环节特效更酷炫、更大胆，并可以自由搜索借鉴后给规划方案。

我的判断：这条不能无条件 global 化，因为“不要加额外功能”通常是阶段性约束。应该保留为 DaoFocus 某次迭代目标，或者标记为 expirable / task-scoped memory。

## Provider 版结果

我尝试了真实 provider：

```powershell
cargo run --quiet -- observe replay --project 'C:\DaoFocus' --home $env:USERPROFILE --target codex --target claude-code --engine claude-code --dry-run
```

结果：

- `claude-code` synthesis 60 秒超时
- 系统回退到 local fallback
- 候选仍然是同一批 8 条

我也尝试了：

```powershell
cargo run --quiet -- observe replay --project 'C:\DaoFocus' --home $env:USERPROFILE --target codex --target claude-code --engine codex --dry-run
```

该命令 exit code 为 0，但没有输出可读报告。随后 `--engine llm` 在 184 秒后被外层命令超时。

## 当前效果评价

可用的部分：

- 能准确找到 DaoFocus 相关历史对话：120/694 个来源被匹配到目标项目。
- 能把历史对话落成 observations：101 条。
- 能抓住若干真实项目偏好：数值难度、灵石概率、拖动条、体验打磨、不要过度复杂。
- 候选都有 observation ID 和 quote，已经不是完全无证据的总结。

不达预期的部分：

- Candidate 和 Draft Inbox 关系不清：CLI 报 `Candidates written: 8`，但 `draft list` 显示 `No drafts found`。这不是用户可理解闭环。
- 去重不足：第 4 和第 7 条重复。
- scope 判断偏松：多条 DaoFocus 语境下的偏好被判成 global。
- 代词/上下文缺失未拦截：第 5 条里的 A/B/C 失去语义，仍被提炼。
- 阶段性约束未治理：第 8 条“不要加额外功能”应该有生命周期或适用边界。
- provider 长历史提炼不可用：真实 provider 超时或空输出，当前只能回落到 local。

## 我认为下一轮应该怎么改

1. 先把 review surface 改清楚：UI/CLI 都要明确区分 observation、candidate、draft、approved memory card。
2. 给 Golden Set 加 DaoFocus 真实失败样例：重复候选、A/B/C 指代丢失、项目偏好误判 global、阶段性目标误判 always-on。
3. 做 candidate merge review：同源/近似候选先合并成一个审查视图，而不是让用户看重复卡。
4. 增加 scope/lifecycle gate：项目语境默认 project，只有跨项目复现才升 global；阶段性语句默认 expirable。
5. 重做 provider 分批策略：按 session/chunk 分批诱导 quote，再做二次合并，不能把 101 条 observation 一次塞给 provider。
6. 把“可批准”的候选转换成更像 Memory Card 的表达，例如：

```text
DaoFocus 的经济和修炼数值应保持递增难度。境界进阶不能线性变容易；灵石获取应使用概率机制，点击/键盘统计在一次判定后无论成功或失败都清空。
```

```text
DaoFocus 的桌宠拖动应以人物本体为主要拖动入口；不要再额外保留顶部拖动条，除非后续用户明确要求恢复。
```

