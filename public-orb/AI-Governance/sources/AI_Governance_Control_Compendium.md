# AI Governance Control Compendium

This companion source is intentionally authored for the MCPOrb public demo. It complements the official NIST and EU materials with a long-form operational control narrative, a denser governance vocabulary, and repeated cross-links between policy, process, evidence, and assurance concepts. The goal is practical: the public AI Governance corpus should be rich enough to exercise lexical search, TF-IDF weighting, typo-tolerant trigram search, and dense retrieval in one reproducible example.

## Executive framing

An effective AI governance program is not a single policy document. It is a connected operating model that links strategy, model inventory, data lineage, risk taxonomy, incident management, model documentation, access control, red-team review, vendor diligence, change approval, human oversight, rollback readiness, control testing, and executive accountability. Teams fail when they maintain a policy layer without an evidence layer, or an evidence layer without ownership, or ownership without a recurring operating cadence. A mature program therefore connects governance committee decisions, system-level control objectives, model-level risk registers, deployment criteria, and continuous monitoring signals through one explicit traceability chain.

In a healthy operating model, every high-impact workflow has a named business owner, a named technical owner, a risk acceptance path, a data steward, a model card, an evaluation protocol, a fallback path, and an incident response playbook. The governance program uses a controlled vocabulary so that audit teams, engineering teams, product teams, procurement teams, privacy counsel, safety reviewers, and external assessors can all reason over the same artifacts. This vocabulary includes terms such as intended use, prohibited use, high-risk use, human-in-the-loop, human-on-the-loop, post-deployment monitoring, residual risk, control sufficiency, model drift, prompt injection, retrieval poisoning, policy exception, compensating control, escalation threshold, and evidentiary completeness.

## Governance architecture

The governance architecture should define how policies depend on standards, how standards call implementation procedures, how procedures use templates, how templates reference evidence repositories, and how evidence repositories support internal review and external inspection. A reference architecture can include the following chain: board risk appetite statement, enterprise AI policy, secure development standard, model lifecycle procedure, launch readiness checklist, supplier review questionnaire, evaluation trace log, monitoring dashboard, incident register, and quarterly attestation package. When a control owner updates one document, the update should reference upstream policy identifiers and downstream evidence identifiers so that the system preserves a machine-searchable relation graph between strategy, controls, and proof.

The strongest programs encode governance relations explicitly. A use-case register references the model inventory. The model inventory references training data notes, evaluation reports, and deployment restrictions. The deployment restrictions reference applicable laws, sector obligations, and contract clauses. The contract clauses reference vendor representations, subprocessors, export restrictions, and service-level guarantees. The incident workflow references severity definitions, notification thresholds, regulator reporting pathways, and communications approval rules. The monitoring workflow references data quality alerts, hallucination benchmarks, bias indicators, safety overrides, and rollback triggers. This web of references matters because search quality improves when governance terms co-occur with architectural entities, responsible roles, control identifiers, and exception pathways.

## Control families

### Governance and accountability

The accountability family defines who approves a use case, who accepts residual risk, who signs the deployment memo, who owns the risk register, and who certifies that mandatory evidence is complete. Evidence may include charter language for the AI governance council, meeting cadences, quorum rules, decision logs, escalation paths, and exception approvals. Important search phrases in this family include governance committee, accountability matrix, risk acceptance memo, approval authority, separation of duties, delegated authority, exception register, and control attestation.

### Data governance and provenance

The data governance family defines what data is used, why the data is used, how the data was sourced, which restrictions attach to the data, how sensitive attributes are treated, and what retention or deletion rules apply. Strong evidence includes dataset lineage diagrams, labeling guidance, provenance records, transformation logs, usage restrictions, and evaluation datasets separated from training datasets. Important search phrases include provenance chain, training corpus, benchmark split, representative sampling, protected class signal, synthetic augmentation, consent boundary, retention schedule, deletion workflow, and lineage verification.

### Model transparency and documentation

The transparency family defines the documentation required to explain model scope, limitations, evaluation boundaries, intended users, known failure modes, confidence calibration, uncertainty communication, and user-facing disclosure. Strong programs require model cards, system cards, release notes, prompt usage guidance, evaluation summaries, and change logs. Important search phrases include model documentation, limitation statement, confidence caveat, failure taxonomy, evaluation summary, release approval, usage restriction, transparency notice, traceability record, and user disclosure.

### Safety, security, and resilience

The safety and security family defines adversarial testing, abuse-case analysis, prompt injection defense, retrieval contamination review, secrets exposure prevention, insecure tool invocation controls, rate limiting, escalation hooks, and rollback plans. Evidence includes red-team reports, abuse scenario tables, sandbox configuration baselines, incident playbooks, and resilience tests for degraded modes. Important search phrases include prompt injection, jailbreak resistance, retrieval poisoning, exfiltration path, containment strategy, degraded mode, rollback trigger, service hardening, safeguard coverage, and incident rehearsal.

### Monitoring, auditability, and change management

The monitoring family defines how the organization observes performance, detects policy drift, records operator actions, and decides when to retrain, disable, or roll back a system. Evidence includes dashboards, alert thresholds, change tickets, sign-off records, periodic reviews, and audit logs. Important search phrases include post-deployment monitoring, drift threshold, alert routing, audit trail, rollback decision, change review board, release gate, evidence archive, control test, and continuous assurance.

## Risk scenario library

Scenario one concerns a support assistant that summarizes customer complaints and recommends remediation actions. Governance reviewers should assess whether the assistant may fabricate facts, expose regulated personal data, or recommend actions outside approved policy. The evidence package should include a complaint taxonomy, human-review rules, escalation criteria, and a testing set covering ambiguous, adversarial, multilingual, and emotionally charged inputs.

Scenario two concerns a procurement copilot that drafts vendor risk summaries and contract clauses. Governance reviewers should assess whether the system misstates legal obligations, omits export restrictions, overstates compliance claims, or fails to surface high-risk suppliers. Evidence should include clause libraries, legal review checkpoints, procurement workflow dependencies, and exception approval records.

Scenario three concerns an internal engineering assistant that retrieves design patterns and generates implementation suggestions. Governance reviewers should assess dependency confusion, insecure configuration suggestions, secrets disclosure, unsafe code generation, and hallucinated API guidance. Evidence should include secure coding checklists, approved package policies, retrieval-source allowlists, and security sign-off steps.

## Evidence mapping template

Use the following evidence pattern for each governed AI system:

1. Define intended use, prohibited use, and high-risk use boundaries.
2. Record the model version, data lineage, and evaluation baseline.
3. Link the use case to applicable policy, legal, privacy, security, and safety controls.
4. Identify the accountable executive, technical owner, and review forum.
5. Capture evaluation results for quality, safety, robustness, bias, and misuse.
6. Record open issues, compensating controls, and residual risk acceptance.
7. Define post-launch monitoring, alert thresholds, and rollback criteria.
8. Archive a traceable release package with approvals, exceptions, and evidence URIs.

The point of this source is not normative authority. The point is to provide long, vocabulary-rich, relation-heavy material that connects the official framework texts to operational questions that users actually ask during demos: Which controls apply before launch? Where is human oversight defined? Which sources mention red-team review? Which documents discuss incident escalation, model cards, or residual risk? Those questions benefit from lexical precision, term weighting, typo tolerance, and semantic similarity at the same time.