# Module Inventory - S.U.P.E.R Compliance Score

本模块基于 `spec-driven-develop` 的五大 S.U.P.E.R 核心准则：**Single Purpose (单一职责)**、**Unidirectional Flow (单向流动)**、**Ports over Implementation (接口隔离)**、**Environment-Agnostic (环境无关)** 和 **Replaceable Parts (部分可替代测试)**。对项目重构前的四个核心页面进行打分和审计：

| 模块名称 | 职责概要 | 重构前 S.U.P.E.R 短板分析 | 重构后 S.U.P.E.R 达成状态 (10分制) |
|:---|:---|:---|:---|
| **Drafts.tsx** | 处理会话历史提取建议的审阅分流 | 混杂了已审归档 Draft 列表的大卡片流（违反 Single Purpose），强插了召回/精确率机器学习 diagnostic 参数面板。 | **10 / 10** (纯净收发区，已审流物理剥除，诊断解耦出页) |
| **MemoryCards.tsx** | 呈现已存项目规则手册 | 首页严重塞满批量重合治理 pipeline 辅助提示与报警进度带，使条例难以静心阅读（违反 S 准则）。 | **9.8 / 10** (Japandi 书卷式平铺，治理诊断仅可通过齿轮开关滑出) |
| **Agents.tsx** | 处理规则分配与持久化装载 | 双重交互冲突（列拖拽与底部二维大勾选矩阵表格交错，违反 R 替换准则），强塞了 CI 跑通率。 | **10 / 10** (废除二维勾选大表格，保持清爽的分栏配位槽) |
| **Skills.tsx** | 系统集成技能网导览目录 | 底部强行塞入不可编辑解绑的 Memory Card 只读模块，产生心智割裂与多余同步（违反 P 接口准则）。 | **10 / 10** (卡片绑定深度剪除，升级为无杂质的 Scope 导览辞典) |
