---
stepsCompleted:
  - step-01b-continue.md
  - step-02-discovery.md
  - step-02b-vision.md
  - step-02c-executive-summary.md
  - step-03-success.md
  - step-04-journeys.md
  - step-05-domain.md
  - step-06-innovation.md
  - step-07-project-type.md
  - step-08-scoping.md
  - step-09-functional.md
  - step-10-nonfunctional.md
  - step-11-polish.md
  - step-12-complete.md
  - step-e-01-discovery.md
  - step-e-02-review.md
  - step-e-03-edit.md
inputDocuments:
  - /Users/sang/polymarket-ai/_bmad-output/planning-artifacts/research/domain-polymarket-rust-clob-profit-research-2026-04-04.md
  - /Users/sang/polymarket-ai/_bmad-output/planning-artifacts/research/market-rust-polymarket-trading-system-lopez-de-prado-research-2026-04-04.md
  - /Users/sang/polymarket-ai/_bmad-output/planning-artifacts/research/technical-polymarket-claim-validity-audit-research-2026-04-04.md
  - /Users/sang/polymarket-ai/_bmad-output/brainstorming/brainstorming-session-2026-04-04-120000.md
documentCounts:
  briefCount: 0
  researchCount: 3
  brainstormingCount: 1
  projectDocsCount: 0
workflowType: 'prd'
workflow: 'edit'
projectName: 'polymarket-ai'
date: '2026-04-04'
lastStep: 12
lastEdited: '2026-04-04'
editHistory:
  - date: '2026-04-04'
    changes: 'Applied validation-driven fixes for measurability, traceability, and fintech compliance.'
  - date: '2026-04-04'
    changes: 'Resolved critical validation gaps: measurable FR/NFR refinements, Phase 1/2 traceability alignment, and expanded fintech security/fraud controls.'
classification:
  projectType: blockchain_web3
  domain: fintech
  complexity: high
  projectContext: greenfield
---

# Product Requirements Document - polymarket-ai

**Author:** Sang
**Date:** 2026-04-04

## Executive Summary

This PRD defines a production-grade Rust trading platform for Polymarket focused on one objective: sustained positive risk-adjusted returns with strict capital protection and explicit loss-limiting controls. The system combines event-driven CLOB execution, portfolio-level risk governance, and a research factory that continuously proposes, validates, and promotes alphas under López de Prado-style anti-overfitting rules.

The target user is a single operator or small quant team that needs reliable live deployment, deterministic risk limits, and transparent operational visibility. The product must run continuously on a hardened production environment, execute through official Polymarket interfaces, and provide a real-time dashboard covering trades, PnL attribution, inventory, risk status, and alpha-level performance.

### What Makes This Special

The differentiator is not one fragile predictive signal; it is an integrated profit engine with multiple independent alpha sources and hard risk constraints. The system prioritizes structural edge (spread capture, incentives/rebates where applicable, regime-aware market selection) and layers statistical alphas only after passing rigorous validation gates (purged/embargoed CV, CPCV, overfit diagnostics, promotion thresholds).

A second differentiator is operational discipline as a first-class product feature: heartbeat watchdogs, kill-switches, stale-data protections, reconciliation checks, controlled rollout ladders, and complete trade observability are mandatory requirements, not future enhancements. This design aims to maximize durability of edge and minimize blow-up risk in non-stationary markets.

## Project Classification

- **Project Type:** blockchain_web3 (with API/backend trading services and web dashboard interface)
- **Domain:** fintech
- **Complexity:** high
- **Project Context:** greenfield

## Success Criteria

### User Success

- Operator can launch, monitor, and control the trading engine from a single dashboard without shell access for routine operations.
- Operator can inspect every order and trade with full lifecycle traceability (signal → order intent → exchange response → fill/PnL impact).
- Operator can understand "why PnL changed" via alpha-level attribution, market-level attribution, and execution-quality diagnostics.
- Operator can trigger safe-state controls (pause, reduce-only mode, cancel-all, full stop) in under 5 seconds from dashboard actions.

### Business Success

- Primary objective: achieve and sustain positive net PnL after all fees/rebates/incentives, slippage, and infra costs over rolling 30-day windows.
- Capital-protection objective: never exceed configured daily and portfolio drawdown limits; automatic trading halt on breach.
- Phase 1 portfolio objective: maintain one production alpha family live (execution alpha), with optional statistical alpha limited to shadow/read-only evaluation.
- Phase 2 portfolio objective: maintain at least 2 independent production alpha families live (execution alpha + statistical alpha) to reduce single-strategy fragility.
- Scaling objective: capital increases only when defined promotion gates are passed for 2 consecutive evaluation windows.

### Technical Success

- Trading core runs 24/7 with supervised process recovery and deterministic safe-state behavior on partial failures.
- End-to-end reconciliation remains exact between internal ledger and exchange-reported order/trade states.
- WebSocket and heartbeat resilience must prevent uncontrolled stale trading; stale feed detection triggers protective mode immediately.
- All strategy promotions (manual in Phase 1, automated in Phase 2) are governed by leakage-safe validation and overfit diagnostics before live capital allocation.
- Security baseline includes secret isolation, least-privilege runtime user, authenticated admin access, and auditable operator actions.

### Measurable Outcomes

- **Profitability:** rolling 30-day net PnL > 0 after costs in at least 4 of first 6 production months.
- **Risk:** max daily drawdown breach rate = 0 tolerated; any breach triggers incident review and auto de-escalation.
- **Stability:** trading service availability ≥ 99.5% monthly (excluding planned maintenance windows).
- **Data integrity:** unresolved order/trade reconciliation mismatches < 0.1% of daily events.
- **Latency safety:** stale-market-data incidents with active quoting < 1 per month.
- **Phase 1 alpha coverage:** at least 1 production alpha family is active for ≥ 95% of eligible trading days after go-live.
- **Model governance:** 100% of promoted alphas include documented purged/CPCV evidence and promotion sign-off.

### Traceability Commitments

- **Multi-alpha portfolio objective (Phase 2 target)** is implemented through FR6-FR11 and FR43-FR48, with phased scope boundaries defined in Product Scope.
- **Promotion governance objective** is implemented through FR7, FR11, FR43-FR48 and audit controls in FR32, FR34, NFR9, and NFR17.
- **Security baseline objective** is implemented through FR31-FR34 and NFR7-NFR9 with measurable logging and authorization criteria.

## Product Scope

### MVP - Minimum Viable Product

- Rust trading engine using official Polymarket SDK for market/user streams and CLOB order lifecycle.
- Portfolio risk layer: inventory caps, per-market exposure limits, daily drawdown stops, kill-switch controls.
- Initial alpha set:
  - Incentive-aware market-making and spread-capture logic.
  - Rule-based regime filters for market selection.
- Core dashboard views:
  - Live orders/trades/positions.
  - PnL and drawdown.
  - Service health (WS, heartbeat, reconnect, error rates).
  - Manual controls (pause/resume/cancel-all).
- Cloud production deployment with process supervision, monitoring, alerting, and runbooks.

### Growth Features (Post-MVP)

- Automated research pipeline that generates candidate alphas from feature libraries and hypothesis templates.
- López de Prado validation pack: event-time sampling, triple-barrier labels, meta-labeling, purged CV, CPCV, and overfit diagnostics.
- XGBoost-based alpha modeling with calibration, confidence-to-sizing policy, and promotion/deprecation workflows.
- Expanded market-universe router and dynamic capital allocation across alpha sleeves.
- Strategy explainability panel showing top drivers and failure-mode annotations.

### Scope Boundary (Phase 1 vs Phase 2)

- **Phase 1 (MVP hard requirements):** FR1-FR5, FR12-FR35, FR37-FR38, FR49, and NFR1-NFR22. Phase 1 delivers one production alpha family (execution alpha) and allows statistical alpha only in shadow/read-only evaluation.
- **Phase 2 (post-MVP additions):** FR6-FR11, FR36, and FR39-FR48 with automated research, promotion governance automation, and incentive-regime intelligence, enabling a second production alpha family.
- **MVP clarification:** MVP uses a rule-based baseline alpha sleeve and manual governance checkpoints; automated model-governance loops are introduced in Phase 2.
- **Phase 1 governance acceptance:** before any live strategy or parameter change in Phase 1, the system must record manual checklist approval for data-quality sanity check, risk-impact review, and dual human sign-off (operator + independent approver) via FR32/FR34 controls.

### Vision (Future)

- Self-improving alpha factory with closed-loop learning (research → validate → shadow → promote → monitor → retire).
- Multi-agent strategy governance with automatic rollback and capital throttling by live risk posture.
- Institutional-grade portfolio cockpit with scenario stress testing, what-if simulation, and compliance/audit exports.
- Multi-node deployment with high-availability failover and regional redundancy.

## User Journeys

### Journey 1 — Primary User (Success Path): Sang, Independent Quant Operator

**Opening Scene:** Sang starts the day needing confidence that the system is profitable, safe, and behaving as designed. He opens the dashboard and sees overnight PnL, drawdown, strategy health, and open risk.

**Rising Action:** He reviews alpha sleeves, notices one sleeve underperforming, and uses model diagnostics to inspect win/loss decomposition. He narrows allocation to higher-quality regimes and confirms inventory is within limits.

**Climax:** A high-opportunity market cluster appears. The engine quotes and executes within configured constraints, with live attribution proving whether gains are from spread capture, incentives, or predictive alpha.

**Resolution:** Sang ends the session with net-positive performance, full traceability for every major trade decision, and confidence that risk policy was enforced automatically.

### Journey 2 — Primary User (Edge Case): Regime Shock and Capital Protection

**Opening Scene:** During a volatile event window, market data quality deteriorates and fill behavior changes abruptly.

**Rising Action:** The system detects stale feed risk and unusual slippage patterns. Alerts escalate from warning to critical. Sang observes drawdown acceleration toward hard stop thresholds.

**Climax:** Auto-protective controls engage: quoting is paused, open orders are canceled, and system mode degrades to safe-state. Sang confirms stop reason and timeline from incident view.

**Resolution:** Capital loss is bounded by policy. A post-incident panel provides root-cause evidence and recommended restart checks before resuming live trading.

### Journey 3 — Admin/Ops User: Maya, Production Reliability Engineer

**Opening Scene:** Maya manages infrastructure reliability for 24/7 operation in a production environment and needs deterministic behavior under restart/failure conditions.

**Rising Action:** She checks service heartbeat, WS reconnect metrics, and reconciliation lag. She rotates secrets, verifies backup/restore status, and validates process supervision policies.

**Climax:** A node-level incident occurs. Automated restart policy recovers core services; health checks gate trading re-entry until market/user streams and reconciliation pass readiness checks.

**Resolution:** Uptime target is preserved without unsafe trading. Maya closes the incident with runbook evidence and service-level metrics.

### Journey 4 — Support/Troubleshooting User: Arjun, Strategy Support Analyst

**Opening Scene:** Arjun receives a ticket: "Why did PnL drop in market X at 14:32 UTC?"

**Rising Action:** He queries timeline views linking signal state, order decisions, fills, and fees. He correlates deviations with tick-size changes and liquidity drop.

**Climax:** He identifies the exact failure mode (execution-quality deterioration under low-depth conditions), tags impacted alpha episodes, and drafts mitigation.

**Resolution:** The ticket is resolved with audit-ready evidence, and a new guardrail is proposed for similar market states.

### Journey 5 — API/Integration User: Lina, External Analytics Consumer

**Opening Scene:** Lina integrates portfolio and execution metrics into an external reporting stack for weekly strategy reviews.

**Rising Action:** She authenticates to read-only endpoints, pulls normalized trade/PnL/risk data, and schedules periodic exports.

**Climax:** She detects alpha drift from external dashboards and sends a de-allocation recommendation.

**Resolution:** Integration enables faster governance decisions without direct access to trading controls.

### Journey 6 — Research User (Phase 2+): Noor, Quant Research Lead

**Opening Scene:** Noor proposes a new alpha and needs to determine whether it is eligible for capital.

**Rising Action:** The system runs the validation sequence (data QC, labeling, purged CV, CPCV, overfit diagnostics) and stores immutable evidence artifacts.

**Climax:** Promotion thresholds are evaluated. If evidence is incomplete or thresholds fail, promotion is blocked and remediation tasks are generated.

**Resolution:** Only qualified candidates move to read-only evaluation and staged capital deployment with full sign-off traceability.

### Journey 7 — Incentive Shift Response: Sang, Independent Quant Operator

**Opening Scene:** Sang receives a warning that rebates and reward economics changed in key markets.

**Rising Action:** He reviews reward-per-risk impact by market bucket and checks strategy recommendations for reallocation.

**Climax:** Guardrails reduce exposure to low-economics markets while reallocating to higher reward-per-risk opportunities within concentration limits.

**Resolution:** Portfolio remains policy-compliant and expected-value aligned despite regime changes.

### Journey Requirements Summary

The journeys require the following capability groups:

- **Live Trading Control:** pause/resume, reduce-only, cancel-all, controlled restart gates.
- **Observability & Explainability:** timeline correlation of signal → order → fill → PnL, alpha attribution, incident diagnostics.
- **Reliability Engineering:** WS/heartbeat watchdogs, process supervision, readiness checks, failure-safe transitions.
- **Risk Governance:** hard limits, regime-aware throttles, auto-halt on policy breach, incident runbooks.
- **Data & Integration:** read-only API for reporting, consistent schemas, exportability, and audit trails.
- **Model Governance:** validation evidence pipeline, promotion gates, read-only evaluation, automatic deallocation triggers.
- **Incentive Intelligence:** reward/rebate regime monitoring, reward-per-risk routing, policy-aware market stratification.

## Domain-Specific Requirements

### Compliance & Regulatory

- Enforce regional eligibility checks before any trading action (including startup health checks and periodic revalidation).
- Maintain immutable, timestamped audit logs for operator actions, strategy promotions, and risk-control events.
- Track market/exchange policy dependencies (fees, rebates, liquidity rewards, order constraints) and require config sign-off when policy inputs change.
- Support documented operational controls aligned with financial software governance expectations: access reviews, incident records, and change-control history.

### Compliance & Audit Matrix

| Control Area | Related Requirements | Evidence Artifact | Retention | Review Cadence |
|---|---|---|---|---|
| Access control and authorization | FR31, FR34, NFR8 | Access grant/revoke logs, approval records | 12 months | Monthly |
| Immutable auditability | FR32, NFR9, NFR17 | Append-only action logs with integrity checks | 12 months | Monthly |
| Promotion governance | FR7, FR11, FR43-FR48 | Validation packets, sign-off records, decision logs | 3 years | Per promotion + quarterly audit |
| Incident and recovery governance | FR29, FR30, NFR15, NFR16 | Incident timelines, postmortems, runbook execution records | 24 months | Monthly |
| Reconciliation integrity | FR14, NFR3, NFR16 | Reconciliation exception reports and resolutions | 24 months | Weekly |
| Fraud prevention governance | FR34, FR41, FR47, NFR8, NFR9 | Dual-approval logs, fraud alerts, containment reports | 24 months | Monthly |

### Security Architecture & Threat Model

- **Critical assets:** signing credentials, trading-control permissions, order/trade ledger, audit records, validation artifacts.
- **Primary threats:** key compromise, privilege escalation, policy-bypass promotion, audit-log tampering, data-feed manipulation.
- **Control posture:** least-privilege RBAC, authenticated privileged actions, immutable audit trails, strict reconciliation gates, and deterministic safe-state transitions.
- **Threat severity model:** classify threats as Critical/High/Medium/Low; response SLA targets are 5 minutes (Critical), 30 minutes (High), 4 hours (Medium), and 1 business day (Low).
- **Defense-in-depth layers:** perimeter access controls, workload identity controls, application policy controls, and data-integrity controls are required for production rollout.
- **Key management architecture:** signing and API secrets must use centrally managed key material with rotation every 90 days and emergency rotation within 30 minutes of suspected compromise.
- **Data classification:** classify operational data into Restricted (keys/secrets), Confidential (order/risk/audit records), and Internal (operational metrics) with retention and access policy mapped per class.
- **Residual risk governance:** each unresolved High/Critical risk requires written risk acceptance by operator and reviewer before go-live.
- **Verification:** each critical control maps to FR/NFR evidence with periodic audit and incident drill validation.

### Technical Constraints

- Secrets must never reside in source code or process arguments; use secure secret injection with rotation support.
- Authentication and authorization must separate read-only users from trading-control users (principle of least privilege).
- All critical actions (deploy, restart, pause/resume, kill-switch, strategy promotion) require authenticated, auditable control paths.
- Data integrity is non-negotiable: order/trade reconciliation must detect and quarantine inconsistencies before further capital deployment.
- Under stale or uncertain data conditions, protective behavior takes priority over latency optimization.
- Production services must not run as root; runtime must use least-privilege service accounts with controlled sudo boundaries for maintenance operations.

### Integration Requirements

- Integrate with official Polymarket APIs/SDK flows for market data, user data, order placement/cancelation, and session liveness controls.
- Provide external reporting interfaces for trade history, realized/unrealized PnL, risk metrics, and alpha-performance time series.
- Support environment-specific deployment integration on production infrastructure (service manager, metrics/log shipping, alert channels).
- Maintain model pipeline integrations for feature generation, training, backtesting/validation artifacts, and promotion registry updates.

### Risk Mitigations

- **Regulatory/availability risk:** hard startup gate prevents trading when eligibility checks fail.
- **Model overfitting risk:** mandatory purged/CPCV validation and promotion governance before production enablement.
- **Execution risk:** enforce price/size guards, inventory caps, and volatility-adaptive throttles.
- **Operational risk:** watchdog-driven safe-state transitions on WS disconnects, heartbeat failures, or reconciliation drift.
- **Concentration risk:** portfolio allocation policy limits per-market and per-alpha exposure.

### Fraud Prevention & Detection

- **Fraud taxonomy:** unauthorized order placement, policy-override promotion, suspicious execution patterns, and audit tampering attempts.
- **Preventive controls:** dual-approval for kill-switch disable, risk-limit increases, and promotion overrides; per-actor rate limits on privileged actions; and mandatory segregation between proposer and approver roles.
- **Detection thresholds:**
  - Reconciliation mismatch rate > 0.1% of daily events triggers incident mode.
  - Privileged-action attempts without valid approvals trigger immediate critical alert.
  - Repeated rejected-order bursts (>25 within 5 minutes) trigger investigation.
- **Containment targets:** critical fraud-class incidents enter safe-state workflow within 5 minutes with operator escalation and full evidence capture.
- **Response playbook targets:** triage starts within 5 minutes, preliminary incident classification within 15 minutes, and containment decision recorded within 30 minutes.
- **Fraud monitoring KPIs:** monthly unauthorized privileged-action success rate = 0, mean-time-to-containment ≤ 10 minutes for Critical fraud events, and false-positive alert rate < 15%.

## Innovation & Novel Patterns

### Detected Innovation Areas

- **Alpha Factory as Product Core:** The system is designed to continuously generate, validate, and promote multiple alphas rather than rely on one static strategy.
- **Execution + Quant Governance Fusion:** Market-microstructure execution logic and López de Prado validation discipline are integrated into one operating model.
- **Profitability with Explicit Capital-Preservation Priority:** Product goals are framed as positive expectancy under hard loss controls, not just return maximization.
- **Operator-Grade Explainability:** Every live decision is traceable for human review, linking model confidence, execution path, and PnL effect.

### Market Context & Competitive Landscape

- Many trading bots optimize for isolated backtest performance; fewer provide production-grade controls for continuous model governance, rollback, and incident-safe operation.
- In prediction markets, edge decay and microstructure shifts are common; durable advantage comes from rapid research iteration plus strict promotion gates.
- Differentiation therefore comes from robust lifecycle management of alphas (research → shadow → live → monitor → retire), not headline accuracy claims.

### Validation Approach

- Use event-time data and regime-aware labeling to reduce noise and leakage risk.
- Require purged/embargoed temporal validation and CPCV distributions before live promotion.
- Gate deployment by economic metrics (net edge after costs, drawdown behavior, stability), not classifier metrics alone.
- Enforce shadow-mode burn-in and limited-capital probation before full allocation.

### Risk Mitigation

- If novel alpha candidates underperform live, auto-degrade to conservative baseline execution sleeve.
- If explainability evidence is incomplete, candidate is blocked from promotion.
- If portfolio correlation rises across alpha sleeves, enforce de-concentration and capital throttling.
- Maintain rollback-ready configuration snapshots for immediate reversion to last stable state.

## Blockchain Web3 Specific Requirements

### Project-Type Overview

The product is a blockchain-integrated trading system built on Polymarket market infrastructure, combining web3 execution semantics with centralized service reliability standards. The solution must preserve cryptographic/account-level correctness while operating as a continuously supervised production service.

### Technical Architecture Considerations

- Architecture must cleanly separate signing/auth contexts, trading decision logic, and user-facing control surfaces.
- Wallet/key operations must be isolated from strategy computation and dashboard processes.
- Transaction/execution semantics from the venue must be normalized into deterministic internal state machines for safety controls.

### Chain Specifications

- Primary chain context is Polygon ecosystem compatibility as required by Polymarket trading infrastructure.
- Environment configuration must support test/sandbox vs production endpoints, chain IDs, and signing contexts without code changes.
- Chain-dependent assumptions (finality timing, status propagation, supported order semantics) must be explicit and test-covered.

### Wallet Support

- Support secure operational wallet model appropriate for production automation (service account with strict scope, rotation, and revocation procedures).
- Separate read-only dashboard credentials from trading/signing credentials.
- Document recovery procedures for key compromise, rotation, and service continuity.

### Smart-Contract / Protocol Interaction Boundaries

- Even when using SDK abstractions, protocol-level assumptions must be documented (order lifecycle states, cancel semantics, heartbeat/session behavior).
- External protocol changes must be monitored and mapped to compatibility checks before enabling strategy updates.
- No direct undocumented contract calls are permitted in MVP; official supported interfaces only.

### Security Audit Requirements

- Pre-production security review must cover secrets handling, auth boundaries, admin action authorization, and audit-log integrity.
- High-risk operations (kill-switch disable, strategy promotion, deployment) require explicit authorization controls and traceability.
- Incident response plan must include compromise detection, containment, and postmortem template.

### Gas / Cost Optimization

- Execution strategy must account for all venue and transaction-related costs in net-edge calculations.
- Order placement/cancel cadence should optimize for economic edge rather than churn.
- If gasless/builder pathways are used, system must track eligibility, fallback behavior, and cost impact per execution path.

### Implementation Considerations

- Exclude assumptions of traditional username/password auth for trading paths; use API key/secret/passphrase and wallet-aware operational controls.
- Avoid centralized DB as single source of truth for execution reality; reconciliation with venue state is authoritative for trade correctness.
- Build integration test suites around protocol edge cases (delays, retries, partial fills, failed cancels, stale sessions).

## Project Scoping & Phased Development

### MVP Strategy & Philosophy

**MVP Approach:** Profitability-and-safety MVP. Phase 1 must prove that the system can preserve capital and generate positive net expectancy in controlled live conditions, with full observability and deterministic failsafes.

**Resource Requirements:**
- Minimum viable team: 2-3 people
  - Rust trading/backend engineer (core execution + services)
  - Quant researcher (alpha design + validation governance)
  - Part-time DevOps/SRE support (deployment, monitoring, incident response)

### MVP Feature Set (Phase 1)

**Core User Journeys Supported:**
- Primary operator success path (daily live control and attribution)
- Primary operator edge-case path (regime shock and auto-protection)
- Admin/ops reliability path (recovery, health gating, secret rotation)
- Support troubleshooting path (incident root-cause and remediation)

**Must-Have Capabilities:**
- Continuous market/user stream ingestion with heartbeat and reconnect supervision.
- CLOB order lifecycle support (place/cancel/batch/reconcile) with safe-state transitions.
- Baseline alpha sleeve (incentive-aware market making + regime filters).
- Hard risk controls (inventory caps, per-market exposure limits, daily drawdown halt, kill switch).
- Dashboard for live state, controls, and forensic traceability.
- Production deployment on hardened infrastructure with supervised services, metrics, logs, alerts, and runbooks.

### Post-MVP Features

**Phase 2 (Post-MVP):**
- Automated alpha research pipeline and candidate registry.
- López de Prado validation framework operationalized (triple-barrier, purged CV, CPCV, promotion checks).
- XGBoost/meta-labeling sleeve with confidence-calibrated sizing.
- Dynamic capital allocator across alpha sleeves and market buckets.
- Incentive-regime intelligence (rebate/reward cliff detection, reward-per-risk routing, and policy-aware market rotation).
- Model-governance analytics (PBO/DSR-style diagnostics, leakage linting, and counterfactual replay).

**Phase 3 (Expansion):**
- Closed-loop self-improving alpha factory with auto-retirement/deployment policies.
- Advanced scenario simulation and stress-testing suite.
- Multi-node high-availability architecture and regional failover.
- External partner/investor reporting and governance portals.

### Risk Mitigation Strategy

**Technical Risks:**
- Hardest risk: combining continuous execution reliability with fast strategy iteration.
- Mitigation: strict service boundaries, feature flags, shadow-mode validation, and rollback snapshots.

**Market Risks:**
- Biggest risk: non-stationary market regimes causing rapid alpha decay.
- Mitigation: regime-aware throttling, promotion gates, and forced deallocation when live diagnostics degrade.

**Resource Risks:**
- Risk: limited team capacity delays reliability work.
- Mitigation: prioritize safety-critical controls before advanced alpha complexity; defer nonessential UI/features to Phase 2+.

## Functional Requirements

### Market Universe & Data Intake

- FR1: Operator can define market-universe selection policies with explicit thresholds for minimum 24h volume (USD), minimum top-of-book depth (USD), maximum quoted spread (bps), minimum reward/rebate rate (bps), and per-market max exposure (% NAV).
- FR2: System can ingest market state updates (best bid/ask, top-5 depth, last trade, tick-size, market status) with staleness ≤ 2 seconds for 99% of updates during normal operation.
- FR3: System can ingest authenticated user activity updates (order accepted/rejected, partial/full fills, cancels, position deltas) and persist each event within 2 seconds for 99% of events.
- FR4: Operator can enable or disable market clusters from live trading without redeploying the system.
- FR5: System can detect stale market or user data when no update is received for more than 30 seconds, log the condition, and automatically pause new order creation until data freshness is restored.

### Strategy Research & Alpha Lifecycle

- FR6: Research user can create and register alpha hypotheses with required metadata fields (hypothesis_id, feature_set_version, target regime, expected edge source, training window, risk assumptions); registration is accepted only when all required fields are populated and feature_set_version references a registered dataset snapshot.
- FR7: System can execute a validation workflow (data quality checks, labeling, purged CV, CPCV, and overfit diagnostics), store timestamped evidence artifacts for each phase, and block progression when any phase fails.
- FR8: Research user can promote, pause, or retire alphas through governed workflow states.
- FR9: System can evaluate candidate alphas in read-only mode that records decisions and simulated execution outcomes without placing live orders before capital allocation.
- FR10: Operator can inspect alpha-level performance attribution across 1-hour, 24-hour, and 30-day windows and market groups, including net PnL, Sharpe, hit rate, and max drawdown.
- FR11: System can block alpha promotion when mandatory validation evidence is missing.

### Trade Execution & Order Management

- FR12: System can submit and cancel limit, reduce-only, and batch-cancel workflows supported by the venue, including explicit handling of pending, live, partially-filled, filled, canceled, and expired states.
- FR13: System can track each order through venue lifecycle states until terminal resolution.
- FR14: System can reconcile internal execution records with venue-reported records.
- FR15: Operator can define order eligibility constraints (market, side, size, and state-based rules).
- FR16: System can halt new order creation while preserving visibility into existing exposure.

### Risk & Capital Management

- FR17: Operator can configure portfolio, market, and strategy-level exposure limits.
- FR18: System can enforce drawdown-based protection policies with automated trade-state transitions.
- FR19: System can enforce inventory and concentration limits before and after each trade decision.
- FR20: Operator can trigger emergency controls including pause, reduce-only behavior, and cancel-all behavior.
- FR21: System can prevent trading when any pre-trade gate fails: data freshness > 30 seconds, stream health not healthy, exposure limits breached, drawdown stop active, strategy approval inactive, or venue eligibility=false.

### Portfolio & Allocation Management

- FR22: Operator can define allocation policies across alpha sleeves and market segments.
- FR23: System can rebalance allocation when market or strategy drift exceeds configured thresholds (default: 10% exposure drift or 15% relative alpha drift) and provide operator-visible rationale before execution.
- FR24: Operator can view realized and unrealized PnL broken down by market, alpha, and time period.
- FR25: System can compute and present cost-aware net performance attribution.

### Operations Dashboard & Incident Handling

- FR26: Operator can access an integrated operational view refreshed at least every 2 seconds containing trading state, risk posture, portfolio attribution, and health metrics (heartbeat freshness, reconnect rate, reconciliation lag, error rate).
- FR27: Operator can retrieve correlated incident evidence linking signal generation, order decisions, fills, and risk actions with query responses in under 5 seconds.
- FR28: Support user can search historical events by market, order_id, alpha_id, actor_id, and time range, and retrieve correlated evidence within 5 seconds for 95th percentile queries.
- FR29: System can notify users within 30 seconds for severity-defined events (drawdown > 80% of daily limit, stream disconnect > 5 minutes, reconciliation lag > 60 seconds, stale data detection, or policy-bypass attempt) through configured channels.
- FR30: Operator can perform controlled recovery workflows after an incident; trading resumes only when readiness gates pass (stream freshness ≤ 30 seconds, reconciliation mismatch rate < 0.1%, risk-limit bundle checksum exactly matches approved release manifest, and operator sign-off recorded).

### Governance, Security & Audit

- FR31: Admin can assign role-based access levels for read-only analytics, operational control, and administrative actions.
- FR32: System can maintain immutable audit trails for privileged actions and state transitions, recording actor_id, action_type, parameters, approval_reference, timestamp, and outcome in append-only storage within 5 seconds of action.
- FR33: Admin can rotate credentials and manage secret lifecycle without interrupting governance controls, supporting scheduled rotation every 90 days and emergency rotation within 30 minutes.
- FR34: System can enforce policy approvals for listed critical actions (strategy promotion/override, risk-limit increase, kill-switch disable, and production configuration changes) by requiring dual approval from distinct actors (proposer ≠ approver) and enforcing per-actor override rate limit ≤ 5 requests per hour.

### External Interfaces & Reporting

- FR35: Analytics user can access normalized reporting data for trades, positions, risk events, and performance metrics.
- FR36: System can export governance and performance artifacts on weekly schedule, on-demand, and incident-triggered runs, including promotion decisions, validation evidence, reconciliation summaries, access audits, and incident postmortems.
- FR37: External integration user can consume read-only programmatic interfaces for downstream analytics via versioned endpoints for trades, positions, pnl, risk events, and alpha attribution, with machine-readable schema artifacts published per version and changelog.
- FR38: Operator can schedule recurring summary reports for strategy and risk oversight at daily (00:00 UTC), weekly (Monday 00:00 UTC), and monthly (day 1, 00:00 UTC) cadences.

### Incentive Intelligence & Market Regime Management

- FR39: Operator can define incentive-aware routing policies using reward-per-risk score = (expected_reward_bps + maker_rebate_bps − expected_cost_bps) / expected_volatility_bps, with per-strategy minimum deployment threshold (default score ≥ 1.2, operator-adjustable).
- FR40: System can detect reward/rebate regime shifts and notify operators when rebate rates change by more than 20 bps, spread regimes widen by more than 50 bps, or venue eligibility status changes (eligible ↔ restricted/ineligible).
- FR41: System can apply market-participation guardrails for low-liquidity and overnight-gap windows: pause new quotes when top-of-book depth < $10,000 or no trade observed for > 15 minutes, and cap order size to 25% of normal when inactivity gap > 4 hours.
- FR42: Operator can configure market-universe stratification (core and satellite buckets) with separate risk/allocation policies.

### Model Integrity & Promotion Governance

- FR43: Research user can configure mandatory leakage checks (forward-bias, data-leakage, regime survivability) and data-quality gates that must pass before training or promotion.
- FR44: System can store and compare validation diagnostics for each candidate model, including out-of-sample Sharpe, max drawdown, Brier score (or expected calibration error), and overfit indicators.
- FR45: Research user can define promotion thresholds and minimum evidence criteria for live deployment eligibility, requiring a complete validation packet (data-quality report, purged/CPCV results, calibration report, and counterfactual replay summary), human sign-off, and threshold pass.
- FR46: System can run counterfactual replay across baseline, stressed execution (2x slippage and 50% reduced fill rate), and delayed-exit (60-second delay) scenarios, and block promotion if stressed net PnL is worse than -5% of baseline.
- FR47: Operator can enforce automatic deallocation or retirement when live model behavior breaches governance thresholds (for example: rolling Sharpe below configured floor or drawdown breach).
- FR48: Research user can define stop-research criteria that terminate experimental branches when any threshold is met: trade count < 200 over 30 days, out-of-sample Sharpe < 0.2, or promotion failure rate > 70% over the last 10 candidates.

- FR49: Admin/Ops user can validate backup integrity and execute deterministic restore rehearsal with reconciliation checks before trading resumes after Severity-1 or Severity-2 incidents.

## Non-Functional Requirements

### Performance

- NFR1: Dashboard queries for portfolio summary, risk posture, and active order state must return within 2 seconds for p95 under sustained 100 requests/second operating load.
- NFR2: Risk-control commands (pause, reduce-only, cancel-all initiation) should be acknowledged by the control plane within 1 second and reflected in system state within 5 seconds.
- NFR3: Event-processing backlog must remain below 5 seconds during normal operation; if backlog exceeds 10 seconds for more than 30 seconds, trading must enter safe state.

### Reliability & Availability

- NFR4: Core trading services must achieve monthly availability of at least 99.5%, excluding planned maintenance.
- NFR5: On data-feed, heartbeat, or reconciliation-critical failures, system must transition within 5 seconds to safe state (pause new orders, preserve visibility, cancel pending unsafe orders) without operator intervention.
- NFR6: Recovery workflows must restore full trading readiness within 10 minutes for 95% of restart incidents and require explicit readiness gates before trading reactivation.

### Security

- NFR7: 100% of sensitive credentials must be protected by cryptographic controls providing at least 128-bit security strength at rest and in transit, verified by monthly configuration audits and annual independent security review; plaintext credentials in logs, configuration artifacts, or telemetry are prohibited.
- NFR8: 100% of administrative and trading-control actions must require authenticated identity and RBAC policy checks; 100% failed privileged authorization attempts must generate alerts within 30 seconds; successful unauthorized privileged actions per month must remain at 0.
- NFR9: Security events (auth changes, key rotation, privileged actions, policy overrides, and failed privilege attempts) must be recorded in append-only immutable audit logs within 5 seconds of occurrence.

### Scalability

- NFR10: Architecture must support at least 10x growth in tracked markets and alpha candidates while maintaining p95 event-processing latency ≤ 2 seconds and without public API contract changes, verified by quarterly load tests.
- NFR11: Horizontal scale-out of non-signing components (ingestion, analytics, reporting) must support at least 3x throughput increase with ≤ 20% degradation in p95 latency and no schema-breaking interface changes, verified by load testing.

### Integration

- NFR12: External API failures or degradations must be isolated such that the system enters safe-state within 5 seconds and new order submission rate drops to 0 until recovery gates pass.
- NFR13: Integration contracts for reporting/export interfaces must be versioned, include at least 90 days deprecation notice, and remain backward-compatible for 6 months after replacement.

### Observability & Operability

- NFR14: System must expose structured logs, metrics, and traces with at least 99% event-correlation coverage for order lifecycle, risk actions, and strategy decisions.
- NFR15: Critical incidents must generate alerts within 30 seconds containing cause, impacted systems, and runbook links; on-call acknowledgement target is under 15 minutes.
- NFR16: Post-incident analysis data (timeline, root-cause evidence, impacted positions/markets) must be queryable within 5 seconds without manual log stitching.

### Compliance & Auditability

- NFR17: Operator and system actions affecting capital allocation, strategy state, or risk policy must be auditable with timestamp, actor, action type, parameters, approval status, and reason, and be queryable within 5 seconds for 95th-percentile audit queries.
- NFR18: The platform must retain standard execution records for 12 months and governance/promotion decisions for 3 years for financial audit review.

### Deployment Hardening

- NFR19: Production runtime processes must execute under least-privilege execution contexts with no default administrative privileges; 100% of temporary privilege escalation sessions must expire within 30 minutes, be approved by an independent admin, and be fully audited.
- NFR20: Deployment workflows must be reproducible from versioned artifacts with matching build hashes for 100% of release runs, rollback-capable within a 48-hour window, and auditable with retained configuration/artifact version logs for 24 months.
- NFR21: Secrets used in production deployment must be rotatable without full system shutdown, with rotation support for API keys every 90 days and event-driven emergency rotation.
- NFR22: Security and fraud controls must be validated through monthly control execution checks and quarterly tabletop incident drills, with ≥ 95% control-test pass rate and remediation tickets opened within 5 business days for any failed control.

## Deployment Strategy (Production Environment)

### Target Environment

- Initial production target: hardened production host baseline.
- Administrative provisioning access must transition to least-privilege operations before production trading begins.

### Deployment Principles

- Bootstrap once, operate safely: establish least-privilege operator/service identities, lock down administrative access policy, and disable unnecessary privileged workflows.
- Separate concerns: distinct service units for ingestion, execution, risk, research worker, and dashboard/API.
- Infrastructure as configuration: environment variables, secrets, and service definitions are versioned and reviewable.

### Operational Controls

- Health-gated startup: trading services only enter active mode after data, auth, reconciliation, and risk-policy checks pass.
- Controlled rollout: shadow mode → low-capital live mode → scaled mode, with explicit sign-off at each gate.
- Incident readiness: runbooks for disconnect storms, stale data, reconciliation drift, and drawdown-stop activation.

### Service Components & Isolation

- Ingestion service: market/user stream intake, heartbeat supervision, and freshness checks.
- Execution service: order submission, cancellation, and state transition handling.
- Risk service: drawdown controls, concentration checks, and safe-state triggers.
- Reporting service: audit exports, reconciliation summaries, and incident evidence retrieval.
- Research service (Phase 2): validation pipelines, promotion evidence packaging, and counterfactual replay.

### Trading Reactivation Readiness Checklist

- [ ] Market and user streams healthy with staleness under threshold.
- [ ] Reconciliation checks passing with no unresolved critical mismatches.
- [ ] Risk limits loaded and validated for active strategy set.
- [ ] Required audit and incident artifacts captured for previous incident.
- [ ] Operator sign-off recorded for reactivation.
