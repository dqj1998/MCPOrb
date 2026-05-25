# AI Governance Bilingual FAQ

This demo supplement adds a substantial Simplified Chinese section alongside English governance terminology so that the AI Governance example can exercise multilingual retrieval, CJK trigram indexing, and cross-language question phrasing.

## 双语总览 / Bilingual overview

人工智能治理不是单一制度文件，而是一套持续运行的管理体系。它要求组织把政策、流程、责任、证据、审计、监控、升级、整改、复盘这些环节连成闭环。If a team only writes principles but does not define ownership, review cadence, monitoring thresholds, exception handling, and incident evidence, the governance system is incomplete. 一个成熟的治理体系需要把用途边界、模型清单、数据来源、风险等级、人工监督、变更审批、上线门槛、运行监控、回滚条件、第三方评估连接起来。

组织在落地阶段经常遇到几个典型问题：谁对高风险场景承担最终责任？哪些证据必须在上线前归档？当模型输出出现幻觉、偏差、违规建议、泄露敏感信息、误导性解释时，谁负责触发暂停、升级、人工复核和整改？这些问题必须在治理文件、控制矩阵、发布流程和事故手册中保持一致。 The same terms should appear across policy, standards, procedures, checklists, review notes, and dashboards so that search and audit traceability remain reliable.

## 常见问题 / Frequently asked questions

### 1. 什么叫“人工监督” / What is human oversight?

人工监督不是简单地让人“看一眼结果”。真正的人工监督包括权限设计、拒绝覆盖能力、升级路径、复核抽样、异常解释、责任分配和操作留痕。Human oversight means a person can understand the decision context, challenge the recommendation, inspect source evidence, override unsafe actions, and trigger escalation when confidence is low or harm potential is high.

### 2. 高风险 AI 工作流上线前需要准备什么 / What should be prepared before launching a high-risk AI workflow?

至少应准备以下内容：用途说明、禁止用途、数据来源说明、训练与评测边界、模型卡、系统卡、风险登记册、控制矩阵、人工监督方案、事故响应预案、回滚方案、供应商尽调记录、法律与隐私评审意见、发布审批记录、持续监控指标。 Before launch, the team should also define alert thresholds, evidence retention rules, fallback behavior, user disclosure text, and an accountable owner for residual risk acceptance.

### 3. 什么是“可追溯证据链” / What is a traceable evidence chain?

可追溯证据链要求每个治理结论都能追溯到原始材料、评测记录、审批节点和责任人。A traceable evidence chain links policy identifiers, control IDs, model versions, evaluation artifacts, sign-off records, monitoring dashboards, incident tickets, and exception memos. 没有证据链，组织就难以回答监管问询、内部审计、客户尽调或事故复盘中的关键问题。

### 4. 为什么需要持续监控 / Why is continuous monitoring required?

因为模型行为会随着数据分布、用户输入、检索语料、供应商更新、系统配置和业务流程变化而发生偏移。Continuous monitoring detects drift, misuse, safety regressions, retrieval poisoning, broken safeguards, latency spikes, and emerging failure patterns. 治理团队需要设置告警阈值、人工复核触发器、灰度发布规则、回滚条件和升级通道。

### 5. 第三方模型或工具接入时要关注什么 / What should be checked when integrating third-party models or tools?

要关注供应商声明、服务条款、数据处理边界、子处理者、日志保留、地区限制、出口管制、模型更新节奏、安全事件通报、SLA、审计支持、删除承诺、测试环境隔离以及停服应对方案。 Vendor risk review should also verify how prompts, embeddings, metadata, and user attachments are stored and whether opt-out or regional controls are available.

## 控制术语对照 / Governance term mapping

| 中文术语 | English term | Typical evidence |
|---|---|---|
| 风险接受 | risk acceptance | signed memo, exception register |
| 人工复核 | human review | approval workflow, reviewer logs |
| 上线门槛 | launch gate | readiness checklist, release sign-off |
| 持续监控 | continuous monitoring | dashboard, alerts, periodic reports |
| 事故升级 | incident escalation | severity matrix, on-call playbook |
| 证据归档 | evidence archival | repository links, retention record |
| 模型文档 | model documentation | model card, system card |
| 数据来源 | data provenance | lineage note, dataset inventory |
| 例外审批 | exception approval | waiver record, compensating control |
| 回滚条件 | rollback criteria | release plan, emergency switch |

## 中文问答扩展 / Expanded Chinese prompts

下面这些查询句子是为了让演示中的检索结果更接近真实业务提问：

- 高风险 AI 应用在上线前必须完成哪些治理检查？
- 哪些材料说明了人工监督、人工复核和人工兜底的差异？
- 什么证据可以证明模型变更已经经过审批和评测？
- 事故响应手册里应该包含哪些升级、通报和回滚步骤？
- 供应商接入外部模型服务时，数据边界和日志留存如何审查？
- 如何把模型卡、风险登记册、控制矩阵和发布审批记录关联起来？

这些中文段落故意保持较高的术语密度，覆盖治理、审计、风险、证据、控制、整改、监控、发布、回滚、责任、尽调、法规等关键词，以便 trigram 检索、BM25、TF-IDF 和向量检索都能从同一个示例中获得有代表性的测试语料。