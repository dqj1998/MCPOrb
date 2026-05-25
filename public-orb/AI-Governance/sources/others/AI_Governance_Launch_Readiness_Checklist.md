# AI Governance Launch Readiness Checklist

This checklist is a structured companion source for the public AI Governance demo. It is intentionally table-heavy and bilingual so that the corpus exercises lexical, term-frequency, trigram, and dense retrieval over governance evidence questions that operators actually ask before a high-risk AI workflow goes live. Every row pins a control objective, a concrete evidence artifact, an accountable owner role, an activation trigger, and a rollback expectation. The checklist is a working artifact, not a normative framework — read it together with the NIST AI RMF, the EU AI Act overview, and the Risk Scenario Library in this corpus.

## 1. Pre-launch governance approvals / 上线前治理审批

| ID | Control objective | Evidence artifact | Owner role | Trigger | Rollback expectation |
|---|---|---|---|---|---|
| G-01 | Use case is registered, scoped, and classified by risk tier | use-case register entry, risk classification memo | AI governance program manager | new system entering pre-launch review | suspend onboarding until classification is signed |
| G-02 | Executive sponsor accountable for residual risk | signed risk acceptance memo | accountable executive | risk classification ≥ medium | retract acceptance memo and freeze deployment |
| G-03 | Policy, legal, privacy, and security review completed | review minutes, sign-off log | compliance officer, legal counsel, privacy officer, security lead | risk classification ≥ medium | revert to prior approved release |
| G-04 | Model card and system card published in registry | model card identifier, system card identifier | model owner | new model version | restore previous documented version |
| G-05 | Prompt and retrieval template version locked | prompt registry entry, retrieval allowlist | platform engineering lead | new template version | re-pin previous template |
| G-06 | Evaluation results meet release thresholds | evaluation report, threshold sign-off | model owner, evaluation lead | release candidate evaluation finished | block promotion to production |
| G-07 | Red-team review completed within freshness window | red-team report identifier | security lead, AI governance manager | release candidate, quarterly cadence | block release until refreshed |
| G-08 | Incident response playbook reviewed and current | playbook revision, on-call list | incident commander, security lead | release candidate, semiannual cadence | escalate to crisis-only handling |
| G-09 | Monitoring dashboards configured with alert routes | dashboard URL, alert routing config | MLOps engineer, monitoring lead | release candidate | revert dashboards and pause launch |
| G-10 | Public transparency notice approved per locale | transparency notice version per locale | UX lead, legal counsel | release candidate, locale change | hide AI feature until notice is approved |

## 2. Data governance and provenance / 数据治理与来源

| ID | Control objective | Evidence artifact | Owner role | Trigger | Rollback expectation |
|---|---|---|---|---|---|
| D-01 | Training, fine-tuning, and evaluation datasets documented | dataset inventory, lineage diagram | data steward | new dataset, dataset refresh | unlist dataset and revert affected models |
| D-02 | Data sourcing, consent, and license verified | sourcing memo, license register | privacy counsel, data steward | new dataset, license change | remove dataset and re-evaluate model |
| D-03 | Sensitive attributes treated with documented policy | sensitive-attribute handling policy | privacy officer | new sensitive field detected | block ingestion path until policy update |
| D-04 | Evaluation dataset separated from training dataset | evaluation split documentation | evaluation lead | new evaluation suite | rebuild evaluation suite from approved corpus |
| D-05 | Retention, deletion, and right-to-erasure flows tested | retention test report | privacy engineer | annual cadence, regulatory change | freeze data writes until flow is restored |
| D-06 | Cross-border data movement assessed | transfer impact assessment | privacy counsel | new region, infra migration | restrict region until assessment is closed |
| D-07 | Synthetic data flagged and traceable | synthetic data label, generator config | data engineering lead | synthetic data introduced | quarantine synthetic outputs |
| D-08 | Retrieval index contents reviewed for sensitivity | retrieval source allowlist, scan report | platform engineering lead | new retrieval source | remove retrieval source from index |
| D-09 | Embeddings generation pipeline reproducible | embedding pipeline config, hash | platform engineering lead | new embedding model | revert embeddings to prior pinned set |
| D-10 | Logs containing user content protected and minimized | log handling SOP, redaction tests | security engineer | new logging surface | disable surface until redaction is verified |

## 3. Safety, security, and resilience / 安全、抗攻击、韧性

| ID | Control objective | Evidence artifact | Owner role | Trigger | Rollback expectation |
|---|---|---|---|---|---|
| S-01 | Prompt injection defenses tested for direct and indirect vectors | red-team evaluation, injection probe set | security lead | release candidate, model upgrade | revert to last-known-good template |
| S-02 | Jailbreak resistance evaluation passes thresholds | jailbreak evaluation report | safety evaluation lead | release candidate, quarterly cadence | block release pending remediation |
| S-03 | Retrieval poisoning detection active | retrieval signature monitor | platform engineering lead | new retrieval source | quarantine source and rebuild index |
| S-04 | Tool invocation restricted to allowlisted actions | tool registry, allowlist policy | platform engineering lead | new tool introduced | disable tool until review is complete |
| S-05 | Secrets exposure controls verified | secrets scanning report, redaction tests | security engineer | new code path | revert to last verified surface |
| S-06 | Rate limits and abuse controls in place | rate limit config, abuse detection rules | platform engineering lead | new public-facing surface | tighten limits or block surface |
| S-07 | Sandbox isolation for risky tool actions | sandbox policy, isolation test | platform engineering lead | new high-risk tool | disable tool until isolation is proven |
| S-08 | Output moderation pipeline operational | moderation policy, audit log | trust and safety lead | release candidate, weekly review | disable surface if moderation degrades |
| S-09 | Resilience under degraded service confirmed | degraded-mode test report | reliability engineering lead | release candidate, semiannual cadence | switch to fallback service |
| S-10 | Rollback runbook executable end to end | runbook dry-run, on-call attestation | reliability engineering lead | release candidate, quarterly cadence | execute documented rollback |

## 4. Human oversight, transparency, and user rights / 人工监督、透明度与用户权利

| ID | Control objective | Evidence artifact | Owner role | Trigger | Rollback expectation |
|---|---|---|---|---|---|
| H-01 | Human-in-the-loop required for defined high-stakes actions | escalation policy, reviewer roster | program owner | high-stakes action requested | block high-stakes action automation |
| H-02 | Human-on-the-loop sampling cadence defined | sampling plan, review log | quality lead | release candidate, monthly cadence | increase sampling rate |
| H-03 | Override and challenge pathway documented for users | user help center entry, override tests | UX lead | release candidate | hide feature until pathway is functional |
| H-04 | Right to a human review honored within service level | response time report | program owner | every challenge request | route challenges to human-only queue |
| H-05 | Refusal messages explain reason without overclaiming | refusal copy review, localization sign-off | content design lead, legal counsel | new refusal pattern | revert to previous approved copy |
| H-06 | Disclosure of AI usage visible per surface | per-surface disclosure inventory | UX lead | new surface | suppress surface until disclosed |
| H-07 | Accessibility checks passed across surfaces | accessibility audit report | accessibility lead | release candidate, annual cadence | hold launch until findings closed |
| H-08 | Localized transparency notices match policy registry | per-locale notice version map | legal counsel | new locale or regulatory change | hide AI feature in affected locale |
| H-09 | Operator training completed before access | training completion register | operations lead | new operator onboarded | revoke access until training is complete |
| H-10 | Customer feedback loop instrumented and reviewed | feedback dashboard, monthly review log | quality lead | monthly cadence | escalate findings to incident workflow |

## 5. Monitoring, attestation, and continuous assurance / 监控、证明与持续保障

| ID | Control objective | Evidence artifact | Owner role | Trigger | Rollback expectation |
|---|---|---|---|---|---|
| M-01 | Hallucination rate measured against production sample | hallucination sampling report | quality lead | daily cadence | throttle traffic, escalate model owner |
| M-02 | Drift indicators monitored across signals | drift dashboard, alert thresholds | MLOps engineer | continuous | freeze model promotion |
| M-03 | Bias indicators tracked across protected dimensions | bias monitoring report | equity lead | monthly cadence | open remediation ticket |
| M-04 | Latency, cost, and reliability monitored | reliability dashboard | reliability engineering lead | continuous | activate fallback or capacity controls |
| M-05 | Incident telemetry retained per policy | telemetry retention policy, audit log | security engineer | quarterly cadence | restore retention from backup |
| M-06 | Quarterly attestation signed by model owner | attestation package | model owner | quarterly cadence | escalate missed attestation to governance committee |
| M-07 | External audit readiness pack current | audit readiness folder | AI governance manager | annual cadence, regulator request | activate audit response team |
| M-08 | Exception register reviewed and closed on schedule | exception register, closure log | AI governance manager | monthly cadence | escalate aging exceptions |
| M-09 | Policy registry updated with material changes | policy registry diff | policy steward | material change | revert to prior approved policy |
| M-10 | Board reporting cadence respected | board report archive | AI governance manager | semiannual cadence | document missed cycle and root cause |

## 6. Cross-reference / 交叉索引

The Orb demo questions resolve into specific rows of this checklist:

| Demo question | Most relevant rows |
|---|---|
| What controls should a team prepare before launching a high-risk AI workflow? | G-01 to G-10, D-01, D-04, S-01, S-02, H-01 |
| Which sources discuss human oversight requirements? | H-01, H-02, H-03, H-04, H-09, H-10 |
| Where can I find a practical AI risk assessment checklist? | the entire checklist, with anchors G-01, D-01, S-01, H-01, M-01 |
| Which documents discuss model transparency and documentation? | G-04, G-05, H-05, H-06, H-08, M-09 |

每一行控制项都同时支持英文和中文检索：英文术语包括 release gate、attestation、residual risk、rollback runbook、retrieval poisoning、prompt injection、human-in-the-loop、observability、exception register；中文术语包括上线门槛、证明签署、剩余风险、回滚剧本、检索污染、提示注入、人工兜底、可观测性、例外登记。术语对齐让多语言用户在同一个 Orb 包里得到一致的检索结果。
