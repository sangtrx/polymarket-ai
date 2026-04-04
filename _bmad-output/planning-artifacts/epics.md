---
stepsCompleted:
  - step-01-validate-prerequisites.md
  - step-02-design-epics.md
  - step-03-create-stories.md
  - step-04-final-validation.md
inputDocuments:
  - /Users/sang/polymarket-ai/_bmad-output/planning-artifacts/prd.md
  - /Users/sang/polymarket-ai/_bmad-output/planning-artifacts/architecture.md
  - /Users/sang/polymarket-ai/_bmad-output/planning-artifacts/ux-design-specification.md
---

# polymarket-ai - Epic Breakdown

## Overview

This document provides the complete epic and story breakdown for polymarket-ai, decomposing the requirements from the PRD, UX Design, and Architecture requirements into implementable stories.

## Requirements Inventory

### Functional Requirements

FR1: Operator can define market-universe selection policies with explicit thresholds for minimum 24h volume (USD), minimum top-of-book depth (USD), maximum quoted spread (bps), minimum reward/rebate rate (bps), and per-market max exposure (% NAV).

FR2: System can ingest market state updates (best bid/ask, top-5 depth, last trade, tick-size, market status) with staleness ≤ 2 seconds for 99% of updates during normal operation.

FR3: System can ingest authenticated user activity updates (order accepted/rejected, partial/full fills, cancels, position deltas) and persist each event within 2 seconds for 99% of events.

FR4: Operator can enable or disable market clusters from live trading without redeploying the system.

FR5: System can detect stale market or user data when no update is received for more than 30 seconds, log the condition, and automatically pause new order creation until data freshness is restored.

FR6: Research user can create and register alpha hypotheses with required metadata fields (hypothesis_id, feature_set_version, target regime, expected edge source, training window, risk assumptions); registration is accepted only when all required fields are populated and feature_set_version references a registered dataset snapshot.

FR7: System can execute a validation workflow (data quality checks, labeling, purged CV, CPCV, and overfit diagnostics), store timestamped evidence artifacts for each phase, and block progression when any phase fails.

FR8: Research user can promote, pause, or retire alphas through governed workflow states.

FR9: System can evaluate candidate alphas in read-only mode that records decisions and simulated execution outcomes without placing live orders before capital allocation.

FR10: Operator can inspect alpha-level performance attribution across 1-hour, 24-hour, and 30-day windows and market groups, including net PnL, Sharpe, hit rate, and max drawdown.

FR11: System can block alpha promotion when mandatory validation evidence is missing.

FR12: System can submit and cancel limit, reduce-only, and batch-cancel workflows supported by the venue, including explicit handling of pending, live, partially-filled, filled, canceled, and expired states.

FR13: System can track each order through venue lifecycle states until terminal resolution.

FR14: System can reconcile internal execution records with venue-reported records.

FR15: Operator can define order eligibility constraints (market, side, size, and state-based rules).

FR16: System can halt new order creation while preserving visibility into existing exposure.

FR17: Operator can configure portfolio, market, and strategy-level exposure limits.

FR18: System can enforce drawdown-based protection policies with automated trade-state transitions.

FR19: System can enforce inventory and concentration limits before and after each trade decision.

FR20: Operator can trigger emergency controls including pause, reduce-only behavior, and cancel-all behavior.

FR21: System can prevent trading when any pre-trade gate fails: data freshness > 30 seconds, stream health not healthy, exposure limits breached, drawdown stop active, strategy approval inactive, or venue eligibility=false.

FR22: Operator can define allocation policies across alpha sleeves and market segments.

FR23: System can rebalance allocation when market or strategy drift exceeds configured thresholds (default: 10% exposure drift or 15% relative alpha drift) and provide operator-visible rationale before execution.

FR24: Operator can view realized and unrealized PnL broken down by market, alpha, and time period.

FR25: System can compute and present cost-aware net performance attribution.

FR26: Operator can access an integrated operational view refreshed at least every 2 seconds containing trading state, risk posture, portfolio attribution, and health metrics (heartbeat freshness, reconnect rate, reconciliation lag, error rate).

FR27: Operator can retrieve correlated incident evidence linking signal generation, order decisions, fills, and risk actions with query responses in under 5 seconds.

FR28: Support user can search historical events by market, order_id, alpha_id, actor_id, and time range, and retrieve correlated evidence within 5 seconds for 95th percentile queries.

FR29: System can notify users within 30 seconds for severity-defined events (drawdown > 80% of daily limit, stream disconnect > 5 minutes, reconciliation lag > 60 seconds, stale data detection, or policy-bypass attempt) through configured channels.

FR30: Operator can perform controlled recovery workflows after an incident; trading resumes only when readiness gates pass (stream freshness ≤ 30 seconds, reconciliation mismatch rate < 0.1%, risk-limit bundle checksum exactly matches approved release manifest, and operator sign-off recorded).

FR31: Admin can assign role-based access levels for read-only analytics, operational control, and administrative actions.

FR32: System can maintain immutable audit trails for privileged actions and state transitions, recording actor_id, action_type, parameters, approval_reference, timestamp, and outcome in append-only storage within 5 seconds of action.

FR33: Admin can rotate credentials and manage secret lifecycle without interrupting governance controls, supporting scheduled rotation every 90 days and emergency rotation within 30 minutes.

FR34: System can enforce policy approvals for listed critical actions (strategy promotion/override, risk-limit increase, kill-switch disable, and production configuration changes) by requiring dual approval from distinct actors (proposer ≠ approver) and enforcing per-actor override rate limit ≤ 5 requests per hour.

FR35: Analytics user can access normalized reporting data for trades, positions, risk events, and performance metrics.

FR36: System can export governance and performance artifacts on weekly schedule, on-demand, and incident-triggered runs, including promotion decisions, validation evidence, reconciliation summaries, access audits, and incident postmortems.

FR37: External integration user can consume read-only programmatic interfaces for downstream analytics via versioned endpoints for trades, positions, pnl, risk events, and alpha attribution, with machine-readable schema artifacts published per version and changelog.

FR38: Operator can schedule recurring summary reports for strategy and risk oversight at daily (00:00 UTC), weekly (Monday 00:00 UTC), and monthly (day 1, 00:00 UTC) cadences.

FR39: Operator can define incentive-aware routing policies using reward-per-risk score = (expected_reward_bps + maker_rebate_bps − expected_cost_bps) / expected_volatility_bps, with per-strategy minimum deployment threshold (default score ≥ 1.2, operator-adjustable).

FR40: System can detect reward/rebate regime shifts and notify operators when rebate rates change by more than 20 bps, spread regimes widen by more than 50 bps, or venue eligibility status changes (eligible ↔ restricted/ineligible).

FR41: System can apply market-participation guardrails for low-liquidity and overnight-gap windows: pause new quotes when top-of-book depth < $10,000 or no trade observed for > 15 minutes, and cap order size to 25% of normal when inactivity gap > 4 hours.

FR42: Operator can configure market-universe stratification (core and satellite buckets) with separate risk/allocation policies.

FR43: Research user can configure mandatory leakage checks (forward-bias, data-leakage, regime survivability) and data-quality gates that must pass before training or promotion.

FR44: System can store and compare validation diagnostics for each candidate model, including out-of-sample Sharpe, max drawdown, Brier score (or expected calibration error), and overfit indicators.

FR45: Research user can define promotion thresholds and minimum evidence criteria for live deployment eligibility, requiring a complete validation packet (data-quality report, purged/CPCV results, calibration report, and counterfactual replay summary), human sign-off, and threshold pass.

FR46: System can run counterfactual replay across baseline, stressed execution (2x slippage and 50% reduced fill rate), and delayed-exit (60-second delay) scenarios, and block promotion if stressed net PnL is worse than -5% of baseline.

FR47: Operator can enforce automatic deallocation or retirement when live model behavior breaches governance thresholds (for example: rolling Sharpe below configured floor or drawdown breach).

FR48: Research user can define stop-research criteria that terminate experimental branches when any threshold is met: trade count < 200 over 30 days, out-of-sample Sharpe < 0.2, or promotion failure rate > 70% over the last 10 candidates.

FR49: Admin/Ops user can validate backup integrity and execute deterministic restore rehearsal with reconciliation checks before trading resumes after Severity-1 or Severity-2 incidents.

### NonFunctional Requirements

NFR1: Dashboard queries for portfolio summary, risk posture, and active order state must return within 2 seconds for p95 under sustained 100 requests/second operating load.

NFR2: Risk-control commands (pause, reduce-only, cancel-all initiation) should be acknowledged by the control plane within 1 second and reflected in system state within 5 seconds.

NFR3: Event-processing backlog must remain below 5 seconds during normal operation; if backlog exceeds 10 seconds for more than 30 seconds, trading must enter safe state.

NFR4: Core trading services must achieve monthly availability of at least 99.5%, excluding planned maintenance.

NFR5: On data-feed, heartbeat, or reconciliation-critical failures, system must transition within 5 seconds to safe state (pause new orders, preserve visibility, cancel pending unsafe orders) without operator intervention.

NFR6: Recovery workflows must restore full trading readiness within 10 minutes for 95% of restart incidents and require explicit readiness gates before trading reactivation.

NFR7: 100% of sensitive credentials must be protected by cryptographic controls providing at least 128-bit security strength at rest and in transit, verified by monthly configuration audits and annual independent security review; plaintext credentials in logs, configuration artifacts, or telemetry are prohibited.

NFR8: 100% of administrative and trading-control actions must require authenticated identity and RBAC policy checks; 100% failed privileged authorization attempts must generate alerts within 30 seconds; successful unauthorized privileged actions per month must remain at 0.

NFR9: Security events (auth changes, key rotation, privileged actions, policy overrides, and failed privilege attempts) must be recorded in append-only immutable audit logs within 5 seconds of occurrence.

NFR10: Architecture must support at least 10x growth in tracked markets and alpha candidates while maintaining p95 event-processing latency ≤ 2 seconds and without public API contract changes, verified by quarterly load tests.

NFR11: Horizontal scale-out of non-signing components (ingestion, analytics, reporting) must support at least 3x throughput increase with ≤ 20% degradation in p95 latency and no schema-breaking interface changes, verified by load testing.

NFR12: External API failures or degradations must be isolated such that the system enters safe-state within 5 seconds and new order submission rate drops to 0 until recovery gates pass.

NFR13: Integration contracts for reporting/export interfaces must be versioned, include at least 90 days deprecation notice, and remain backward-compatible for 6 months after replacement.

NFR14: System must expose structured logs, metrics, and traces with at least 99% event-correlation coverage for order lifecycle, risk actions, and strategy decisions.

NFR15: Critical incidents must generate alerts within 30 seconds containing cause, impacted systems, and runbook links; on-call acknowledgement target is under 15 minutes.

NFR16: Post-incident analysis data (timeline, root-cause evidence, impacted positions/markets) must be queryable within 5 seconds without manual log stitching.

NFR17: Operator and system actions affecting capital allocation, strategy state, or risk policy must be auditable with timestamp, actor, action type, parameters, approval status, and reason, and be queryable within 5 seconds for 95th-percentile audit queries.

NFR18: The platform must retain standard execution records for 12 months and governance/promotion decisions for 3 years for financial audit review.

NFR19: Production runtime processes must execute under least-privilege execution contexts with no default administrative privileges; 100% of temporary privilege escalation sessions must expire within 30 minutes, be approved by an independent admin, and be fully audited.

NFR20: Deployment workflows must be reproducible from versioned artifacts with matching build hashes for 100% of release runs, rollback-capable within a 48-hour window, and auditable with retained configuration/artifact version logs for 24 months.

NFR21: Secrets used in production deployment must be rotatable without full system shutdown, with rotation support for API keys every 90 days and event-driven emergency rotation.

NFR22: Security and fraud controls must be validated through monthly control execution checks and quarterly tabletop incident drills, with ≥ 95% control-test pass rate and remediation tickets opened within 5 business days for any failed control.

### Additional Requirements

- Starter template requirement: Epic 1 Story 1 must bootstrap the selected dual-starter monorepo baseline (Rust Cargo workspace + Next.js App Router operator console), including policy-safe environment scaffolding and CI baseline.
- Rust backend services must be event-driven and split by bounded contexts (`control-api`, `execution-engine`, `risk-engine`, plus `portfolio-engine`, `governance-service`, and `reporting-service`; `research-gateway` in Phase 2).
- Exchange integration must use official `polymarket-client-sdk` with `clob` and `ws` capabilities, including heartbeat-safe operation.
- Geoblock/eligibility checks are hard startup/runtime gates and must block trading when not satisfied.
- System of record must be PostgreSQL with `sqlx`, using append-only ledger tables for orders, risk events, approvals, and operator actions.
- Read models must support low-latency dashboard queries via materialized projections while preserving persisted ledger truth.
- Forward-only migration strategy with checksum validation is required in CI/CD.
- Control plane API must be REST (Axum), with live updates over WebSocket/SSE and canonical typed error envelopes.
- APIs and events must follow strict naming/versioning conventions (e.g., `domain.entity.action.v1`) and include correlation metadata.
- Safety-first orchestration is mandatory: uncertain state must trigger protective behavior and safe-state transitions.
- Governance service must gate all privileged mutations and enforce dual-approval workflows for critical actions.
- Immutable audit emission is mandatory for every privileged action and key state transition.
- Authentication model must enforce strict RBAC separation between read-only and trading-control roles.
- Secrets management must prohibit plaintext secrets in source, logs, process arguments, or shell history; secret rotation must be operationalized.
- Reconciliation readiness is a hard gate for reactivation after incidents.
- Observability baseline must include OpenTelemetry, structured logs, metrics, and traces with end-to-end correlation.
- Deployment model must support supervised long-running services, reproducible builds, and rollback windows.
- Runtime must operate with least-privilege identities (no root execution for services).
- Service/component boundaries are enforced: execution cannot mutate policy directly; risk owns eligibility; governance owns approvals/audit.
- Canonical response envelope, timestamp formats (ISO-8601 UTC), and ID standards (UUID/ULID) are consistency requirements.
- Frontend architecture must use Next.js App Router with token-first implementation aligned to UX design tokens.
- Critical operator controls (pause/reduce-only/cancel-all) must remain visible in privileged mode as a fixed interaction contract.
- Development and deployment sequencing must begin with bootstrap and a safety-critical vertical slice (ingestion → risk gate → execution → audit).
- Phase 2 capabilities (research automation and promotion loops) must be feature-flagged behind Phase 1 stability gates.

### UX Design Requirements

UX-DR1: Implement a token-first design system using semantic CSS custom properties (no raw hex values in feature components), including foundation, domain, and product token layers.

UX-DR2: Implement the “Warm Precision” visual direction with warm neutral surfaces, restrained accents, and explicit risk semantics for warning/danger states.

UX-DR3: Implement typography tokens and usage rules for `Source Serif 4` (headings), `IBM Plex Sans` (UI/body), and `IBM Plex Mono` (data IDs/metrics).

UX-DR4: Implement layout tokens for an 8px spacing grid, standardized radius scale (8/12/16/24), and responsive dashboard grids (12 desktop / 8 tablet / 4 mobile).

UX-DR5: Build a persistent, always-visible safety action rail for privileged users with controls for pause, reduce-only, cancel-all, and resume.

UX-DR6: Build a Risk Posture Banner component with explicit states (normal, warning, critical, locked-safe) and assistive announcements (`role=status`, `aria-live` for critical transitions).

UX-DR7: Build a Causal Timeline Panel that correlates signal → order decision → fill → PnL impact with timestamps, source tags, and filterable incident focus.

UX-DR8: Build an Alpha Governance Card that surfaces validation completeness, shadow stability, live guardrail state, and lifecycle status (draft/shadow/candidate-live/production/deallocated).

UX-DR9: Ensure first-login/landing UX exposes current risk posture within 5 seconds via information hierarchy and immediate critical-state visibility.

UX-DR10: Ensure incident intervention flow supports protective action in ≤2 interactions and shows timestamped state confirmation within 10 seconds.

UX-DR11: Provide single-query incident search (market/order/alpha/time) that returns timeline-oriented output and action-ready summaries.

UX-DR12: Implement metadata-first card pattern (timestamp/category/status before details) for operational clarity and scanability.

UX-DR13: Enforce explicit button hierarchy (primary, secondary, tertiary, danger) and danger-action confirmation contracts to prevent accidental destructive actions.

UX-DR14: Implement standardized feedback states (success, warning, error, critical), where critical states include required-action guidance.

UX-DR15: Implement form patterns with progressive disclosure for advanced parameters, inline validation on blur, and risk-impact guidance text.

UX-DR16: Implement navigation model with left rail (primary areas), top bar (global status/context), and in-page tabs (workflow subviews).

UX-DR17: Implement responsive policy behavior: desktop-first operations, tablet-adapted panels, and mobile monitor-first mode with destructive actions disabled by default.

UX-DR18: Meet WCAG 2.2 AA accessibility baseline across core flows, including keyboard parity, visible focus indicators, semantic labeling, and no color-only meaning.

UX-DR19: Respect reduced-motion preferences while maintaining meaningful system-state transitions.

UX-DR20: Implement loading and empty states using skeletons and actionable next-step messaging instead of spinner-only patterns.

UX-DR21: Keep high-risk controls contextually visible during incidents and pair every warning with one recommended next action.

UX-DR22: Ensure critical flows preserve the universal pattern Trigger → Context → Action → Verification with immediate post-action evidence.

### FR Coverage Map

FR1: Epic 2 - Market universe policy thresholds.
FR2: Epic 2 - Real-time market state ingestion.
FR3: Epic 2 - Authenticated user-event ingestion.
FR4: Epic 2 - Dynamic market cluster enable/disable.
FR5: Epic 2 - Stale-data detection with automatic order pause.
FR6: Epic 6 - Alpha hypothesis registration workflow.
FR7: Epic 6 - Validation workflow and evidence gating.
FR8: Epic 6 - Alpha lifecycle state management.
FR9: Epic 6 - Shadow/read-only candidate evaluation.
FR10: Epic 6 - Alpha performance attribution visibility.
FR11: Epic 6 - Promotion blocking on missing evidence.
FR12: Epic 2 - Limit/reduce-only/batch-cancel execution support.
FR13: Epic 2 - Full order lifecycle tracking.
FR14: Epic 2 - Internal-to-venue reconciliation.
FR15: Epic 2 - Configurable order eligibility constraints.
FR16: Epic 2 - Halt new orders while preserving exposure visibility.
FR17: Epic 2 - Configurable exposure limits.
FR18: Epic 2 - Drawdown protection automation.
FR19: Epic 2 - Inventory and concentration enforcement.
FR20: Epic 2 - Emergency safety controls.
FR21: Epic 2 - Pre-trade gate enforcement.
FR22: Epic 3 - Allocation policy definition.
FR23: Epic 3 - Drift-based rebalance with rationale.
FR24: Epic 3 - Realized/unrealized PnL drilldowns.
FR25: Epic 3 - Cost-aware net attribution.
FR26: Epic 3 - Integrated low-latency operations dashboard.
FR27: Epic 3 - Correlated incident evidence retrieval.
FR28: Epic 3 - Historical forensic search workflows.
FR29: Epic 3 - Severity-based operator notifications.
FR30: Epic 3 - Controlled recovery and readiness-gated resume.
FR31: Epic 1 - Role-based access control setup.
FR32: Epic 1 - Immutable audit trail foundation.
FR33: Epic 1 - Credential lifecycle and rotation operations.
FR34: Epic 1 - Dual-approval governance for critical actions.
FR35: Epic 4 - Normalized reporting data access.
FR36: Epic 4 - Scheduled/on-demand/incident exports.
FR37: Epic 4 - Versioned read-only integration APIs.
FR38: Epic 4 - Recurring strategy/risk summary scheduling.
FR39: Epic 5 - Incentive-aware routing policy engine.
FR40: Epic 5 - Incentive regime-shift detection and alerts.
FR41: Epic 5 - Low-liquidity and overnight guardrails.
FR42: Epic 5 - Core/satellite market stratification.
FR43: Epic 6 - Leakage and data-quality gate configuration.
FR44: Epic 6 - Model validation diagnostics registry.
FR45: Epic 6 - Promotion thresholds and evidence criteria.
FR46: Epic 6 - Counterfactual replay stress gating.
FR47: Epic 6 - Automatic deallocation/retirement controls.
FR48: Epic 6 - Stop-research criteria enforcement.
FR49: Epic 3 - Backup integrity validation and deterministic restore rehearsal.

## Epic List

### Epic 1: Secure Operator Access & Governance Control Plane
Deliver authenticated, auditable control access so operators can safely perform privileged actions with role separation and dual-approval governance from day one.
**FRs covered:** FR31, FR32, FR33, FR34

### Epic 2: Live Market Connectivity & Safe Core Execution
Enable operators to connect to live markets, execute and manage orders, and enforce hard pre-trade and risk protections so trading can run safely in production conditions.
**FRs covered:** FR1, FR2, FR3, FR4, FR5, FR12, FR13, FR14, FR15, FR16, FR17, FR18, FR19, FR20, FR21

### Epic 3: Portfolio Command Center, Alerts & Recovery Operations
Provide a real-time operations cockpit for allocation, PnL, incidents, and controlled recovery so users can diagnose issues and restore trading safely.
**FRs covered:** FR22, FR23, FR24, FR25, FR26, FR27, FR28, FR29, FR30, FR49

### Epic 4: Reporting, Exports & External Analytics Integrations
Give internal and external stakeholders reliable, versioned access to performance, governance, and risk data through APIs and scheduled exports.
**FRs covered:** FR35, FR36, FR37, FR38

### Epic 5: Incentive-Regime Adaptive Trading Controls
Let operators adapt trading behavior to reward/rebate changes and liquidity regimes while maintaining policy-compliant market stratification.
**FRs covered:** FR39, FR40, FR41, FR42

### Epic 6: Research-to-Production Alpha Governance Lifecycle
Enable research users to register, validate, shadow, promote, and retire alphas with rigorous anti-overfitting evidence and automated governance guardrails.
**FRs covered:** FR6, FR7, FR8, FR9, FR10, FR11, FR43, FR44, FR45, FR46, FR47, FR48

<!-- Repeat for each epic in epics_list (N = 1, 2, 3...) -->

## Story Execution Standards (Applied to All Stories)

- **Dependency contract:** A story may depend only on prior stories in the same epic or completed prior epics. No forward dependencies are allowed.
- **Schema contract:** A story may create/alter only the tables/entities required for that story’s acceptance criteria. No up-front bulk schema creation.
- **Acceptance criteria contract:** Every story must include happy-path behavior plus at least one negative/failure or boundary/threshold condition.
- **Traceability contract:** Every story must include explicit FR/NFR/UX-DR references so implementation and QA evidence can be mapped directly.

### Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)

In addition to story-local acceptance criteria, each story must satisfy these universal checks:

- **UAC-1 Failure handling:** Invalid input, unauthorized access, and unavailable dependency paths must return explicit machine-readable errors with no unsafe side effects.
- **UAC-2 Boundary behavior:** Threshold and limit boundaries must have deterministic behavior (inclusive/exclusive rules documented and test-covered).
- **UAC-3 Verifiable evidence:** Successful and failed critical operations must emit timestamped audit/telemetry evidence suitable for incident and QA traceability.

### Story Traceability & Dependency Index (Compact-Format Stories)

These entries provide explicit dependency, schema, and FR/NFR/UX coverage for stories that retain concise local formatting.

- **Story 1.1** — Dependencies: none; Schema: no tables created; Traceability: FR31, NFR19, NFR20.
- **Story 1.2** — Dependencies: 1.1; Schema: `roles`, `role_permissions`, `user_roles`; Traceability: FR31, NFR8.
- **Story 1.3** — Dependencies: 1.2; Schema: no new tables; Traceability: FR31, NFR8, NFR9.
- **Story 1.4** — Dependencies: 1.3; Schema: `audit_log_append`; Traceability: FR32, NFR9, NFR17.
- **Story 1.5** — Dependencies: 1.4; Schema: `approval_requests`, `approval_votes`; Traceability: FR34, NFR8, NFR17.
- **Story 1.6** — Dependencies: 1.5; Schema: `credential_rotation_events`; Traceability: FR33, NFR7, NFR21.

- **Story 3.1** — Dependencies: 2.9; Schema: no tables created; Traceability: FR26, NFR1, UX-DR1, UX-DR2, UX-DR3, UX-DR4, UX-DR16, UX-DR17.
- **Story 3.2** — Dependencies: 3.1; Schema: no tables created; Traceability: FR26, NFR2, UX-DR5, UX-DR6, UX-DR9, UX-DR10, UX-DR13, UX-DR14.
- **Story 3.3** — Dependencies: 2.7, 3.1; Schema: `allocation_policies`, `rebalance_recommendations`; Traceability: FR22, FR23, NFR17, UX-DR15.
- **Story 3.4** — Dependencies: 2.6, 3.1; Schema: `attribution_snapshots`; Traceability: FR24, FR25, NFR1, UX-DR12, UX-DR20.
- **Story 3.5** — Dependencies: 2.6, 3.4; Schema: `incident_query_views`; Traceability: FR27, FR28, NFR16, UX-DR7, UX-DR11, UX-DR22.

- **Story 4.1** — Dependencies: 2.6; Schema: reporting read models/views only; Traceability: FR35, NFR13, NFR16.
- **Story 4.2** — Dependencies: 4.1; Schema: `api_contract_versions`; Traceability: FR37, NFR13.
- **Story 4.3** — Dependencies: 4.1; Schema: `report_schedules`, `report_runs`; Traceability: FR38, NFR15.
- **Story 4.4** — Dependencies: 4.1, 4.3; Schema: `export_jobs`, `export_artifacts`; Traceability: FR36, NFR17.

- **Story 5.1** — Dependencies: 2.1, 2.7; Schema: `reward_risk_policies`; Traceability: FR39, NFR1.
- **Story 5.2** — Dependencies: 5.1; Schema: `regime_shift_alerts`; Traceability: FR40, NFR15.
- **Story 5.3** — Dependencies: 5.2; Schema: `participation_guardrail_events`; Traceability: FR41, NFR5, NFR12.
- **Story 5.4** — Dependencies: 5.1; Schema: `market_bucket_profiles`; Traceability: FR42, NFR17.

- **Story 6.1** — Dependencies: 1.4; Schema: `alpha_hypotheses`; Traceability: FR6, NFR17.
- **Story 6.2** — Dependencies: 6.1; Schema: `validation_gate_policies`; Traceability: FR43, NFR17.
- **Story 6.3** — Dependencies: 6.2; Schema: `validation_runs`, `validation_artifacts`; Traceability: FR7, FR44, NFR14, NFR17.
- **Story 6.4** — Dependencies: 6.3; Schema: `shadow_evaluations`; Traceability: FR9, NFR14.
- **Story 6.5** — Dependencies: 6.3; Schema: `promotion_decisions`; Traceability: FR8, FR11, FR45, NFR8, NFR17.
- **Story 6.6** — Dependencies: 6.3; Schema: `counterfactual_replay_runs`; Traceability: FR46, NFR14.

## Epic 1: Secure Operator Access & Governance Control Plane

Establish a secure and auditable control foundation so operators can safely access, authorize, and govern all privileged trading actions.

### Story 1.1: Set Up Initial Project from Starter Template

As an admin/operator,
I want the approved Rust + Next.js monorepo scaffold with CI and environment templates,
So that implementation starts from a consistent, policy-safe baseline.

**Acceptance Criteria:**

**Given** a new repository state
**When** the bootstrap story is completed
**Then** the workspace contains Rust service/crate scaffolds and a Next.js operator console scaffold aligned to architecture structure
**And** baseline CI, reproducible build scripts, and `.env.example` placeholders exist without plaintext secrets per architecture starter-template requirement.

### Story 1.2: Define Role-Based Access Model

As an admin,
I want explicit role definitions and permission boundaries,
So that read-only users and trading-control users are strictly separated.

**Acceptance Criteria:**

**Given** user roles are configured
**When** a user attempts to access control-plane actions outside their role
**Then** access is denied with an explicit authorization error
**And** role mappings satisfy FR31 and least-privilege separation constraints.

### Story 1.3: Add Authenticated Control Middleware

As an operator,
I want all privileged endpoints to require authenticated identity,
So that only verified actors can invoke production controls.

**Acceptance Criteria:**

**Given** a request to a privileged endpoint
**When** identity is missing or invalid
**Then** the request is rejected before command execution
**And** successful requests carry actor identity into downstream audit correlation to satisfy FR31 control requirements.

### Story 1.4: Implement Immutable Privileged Audit Logging

As a compliance-conscious operator,
I want append-only audit records for privileged actions,
So that I can reconstruct and verify who did what and when.

**Acceptance Criteria:**

**Given** a privileged action is executed
**When** the action completes (success or failure)
**Then** an immutable audit record is written with actor_id, action_type, parameters, approval_reference, timestamp, and outcome
**And** write latency meets FR32 timing expectations.

### Story 1.5: Enforce Dual-Approval Governance for Critical Mutations

As a governance approver,
I want critical actions to require independent proposer and approver authorization,
So that risky controls cannot be executed unilaterally.

**Acceptance Criteria:**

**Given** a critical action request (promotion override, risk-limit increase, kill-switch disable, production config change)
**When** approvals are evaluated
**Then** execution is blocked unless proposer ≠ approver and both approvals are present
**And** per-actor override rate limits are enforced per FR34.

### Story 1.6: Add Scheduled and Emergency Credential Rotation Flows

As an admin,
I want secrets and credentials rotated on policy cadence and emergency triggers,
So that credential compromise risk is minimized without governance downtime.

**Acceptance Criteria:**

**Given** scheduled or emergency rotation is triggered
**When** rotation runs
**Then** new credentials become active without bypassing governance controls
**And** rotation evidence is logged for FR33 compliance.

## Epic 2: Live Market Connectivity & Safe Core Execution

Deliver live market connectivity and order execution with deterministic safety gating so operators can trade with bounded risk.

### Story 2.1: Configure Market Universe Policy Engine

As an operator,
I want configurable universe thresholds for liquidity, spread, rewards, and exposure,
So that only eligible markets are tradable.

**Dependencies:** Story 1.6 (credential and auth baseline), no forward dependencies.

**Schema Impact:** Create `market_policy_profiles` and `market_cluster_overrides` only.

**Traceability:** FR1, FR4; NFR12.

**Acceptance Criteria:**

**Scenario A — eligible market classification**
**Given** policy thresholds are configured
**When** the eligibility engine evaluates a market
**Then** only markets meeting all configured constraints are marked tradable.

**Scenario B — invalid policy boundary**
**Given** an operator submits invalid thresholds (negative depth, spread < 0, or max exposure > 100%)
**When** validation executes
**Then** the policy update is rejected with explicit field-level errors and no partial persistence.

**Scenario C — runtime cluster toggle**
**Given** an active cluster is toggled off at runtime
**When** the toggle is confirmed
**Then** new order intents for that cluster are blocked without service redeploy.

### Story 2.2: Ingest Market Stream with Latency Guarantees

As an execution service,
I want to ingest market data events with strict latency targets,
So that pricing and quoting use current venue state.

**Dependencies:** Story 2.1 only.

**Schema Impact:** Create `market_ticks` and `market_stream_health` only.

**Traceability:** FR2; NFR3, NFR5.

**Acceptance Criteria:**

**Scenario A — normal ingestion latency**
**Given** the market stream is healthy
**When** events are processed under normal load
**Then** 99% of market updates are persisted within 2 seconds.

**Scenario B — malformed payload handling**
**Given** a malformed market event is received
**When** ingestion validation fails
**Then** the event is quarantined and logged without crashing stream processing.

**Scenario C — burst boundary condition**
**Given** ingest load reaches 2x baseline for 60 seconds
**When** processing completes
**Then** backlog remains ≤ 10 seconds or service enters degraded mode with alert.

### Story 2.3: Ingest Authenticated User Stream with Ordering Guarantees

As an execution service,
I want to ingest authenticated user-order events with deterministic ordering,
So that order/fill state remains trustworthy for risk decisions.

**Dependencies:** Story 2.2 only.

**Schema Impact:** Create `user_stream_events` and `order_event_offsets` only.

**Traceability:** FR3; NFR3, NFR14.

**Acceptance Criteria:**

**Scenario A — event persistence SLA**
**Given** authenticated stream connectivity is healthy
**When** order/fill/cancel events arrive
**Then** 99% of events are persisted within 2 seconds with monotonic offset tracking.

**Scenario B — duplicate/out-of-order event handling**
**Given** duplicate or out-of-order events are received
**When** dedupe/order checks run
**Then** duplicates are ignored and order state remains idempotent and consistent.

**Scenario C — auth-expiry failure path**
**Given** stream authentication expires
**When** reconnect attempts begin
**Then** new trading intents are blocked until authenticated state is restored.

### Story 2.4: Enforce Data Freshness Gates and Stale-Feed Pausing

As a risk engine,
I want stale-feed detection tied to automatic trading pauses,
So that trading stops when data certainty is insufficient.

**Dependencies:** Stories 2.2 and 2.3.

**Schema Impact:** Create `freshness_gate_events` only.

**Traceability:** FR5; NFR5, NFR12.

**Acceptance Criteria:**

**Scenario A — stale threshold breach**
**Given** no market or user update is received for > 30 seconds
**When** freshness checks execute
**Then** new order creation is paused automatically and reason code is recorded.

**Scenario B — boundary condition**
**Given** latest update age is 29 seconds
**When** freshness checks execute
**Then** trading remains enabled and no stale alert is emitted.

**Scenario C — recovery path**
**Given** stale pause is active
**When** freshness returns to ≤ 30 seconds for a full stability window
**Then** trading can re-enter ready state with explicit operator-visible confirmation.

### Story 2.5: Implement Venue-Compatible Order Lifecycle Handling

As an operator,
I want reliable submission/cancellation across supported order modes,
So that order state remains accurate throughout execution.

**Dependencies:** Stories 2.3 and 2.4.

**Schema Impact:** Create `orders` and `order_state_transitions` only.

**Traceability:** FR12, FR13; NFR14.

**Acceptance Criteria:**

**Scenario A — lifecycle happy path**
**Given** a limit or reduce-only order is submitted
**When** venue state progresses
**Then** states are tracked through terminal resolution (filled/canceled/expired).

**Scenario B — invalid transition failure path**
**Given** an invalid state transition is received
**When** transition validation runs
**Then** transition is rejected, flagged, and auditable without corrupting canonical order state.

**Scenario C — batch-cancel partial success**
**Given** a batch-cancel request where some orders are already terminal
**When** cancel responses return mixed results
**Then** each order receives explicit per-order outcome with retry-safe idempotency keys.

### Story 2.6: Build Reconciliation and Exposure Visibility Core

As an operator,
I want internal execution records reconciled against venue truth,
So that exposure and order correctness are trustworthy.

**Dependencies:** Story 2.5.

**Schema Impact:** Create `reconciliation_runs`, `reconciliation_diffs`, and `exposure_snapshots` only.

**Traceability:** FR14, FR16; NFR16.

**Acceptance Criteria:**

**Scenario A — deterministic reconciliation run**
**Given** matching internal and venue windows
**When** reconciliation executes
**Then** mismatches are identified deterministically with diff classification.

**Scenario B — critical mismatch failure path**
**Given** mismatch rate exceeds 0.1%
**When** reconciliation completes
**Then** system transitions to safe-state and blocks new order placement.

**Scenario C — halted-state visibility**
**Given** new order creation is paused
**When** operator opens exposure view
**Then** latest exposure snapshot remains queryable and timestamped.

### Story 2.7: Configure Portfolio, Market, and Strategy Limit Policies

As a risk engine,
I want configurable exposure and inventory limits across scopes,
So that policy can be tuned without changing execution code.

**Dependencies:** Story 2.6.

**Schema Impact:** Create `risk_limit_profiles` and `inventory_limit_rules` only.

**Traceability:** FR17, FR19; NFR17.

**Acceptance Criteria:**

**Scenario A — scoped limit configuration**
**Given** an operator defines portfolio/market/strategy limits
**When** limits are saved
**Then** versioned limit profiles are persisted and applied by the risk engine.

**Scenario B — invalid configuration failure path**
**Given** a limit update breaches invariants (e.g., market limit > portfolio limit)
**When** validation executes
**Then** update is rejected with explicit validation errors.

**Scenario C — governance boundary for critical increase**
**Given** a critical risk-limit increase is requested
**When** approval checks run
**Then** change remains pending until dual-approval constraints pass.

### Story 2.8: Enforce Pre-Trade Gate Evaluation Pipeline

As a risk engine,
I want every order intent evaluated against all pre-trade gates,
So that unsafe orders are blocked before venue submission.

**Dependencies:** Stories 2.4 and 2.7.

**Schema Impact:** Create `pretrade_gate_decisions` only.

**Traceability:** FR18, FR21; NFR5, NFR12.

**Acceptance Criteria:**

**Scenario A — all gates pass**
**Given** freshness, stream health, exposure, drawdown, strategy approval, and eligibility gates are healthy
**When** an order intent is evaluated
**Then** a pass decision is recorded and the intent is forwarded to execution.

**Scenario B — single gate failure**
**Given** any single gate fails
**When** evaluation completes
**Then** order placement is blocked and a machine-readable denial reason is returned.

**Scenario C — drawdown boundary condition**
**Given** drawdown is exactly at the configured stop threshold
**When** order intent is evaluated
**Then** new order placement is denied and trading enters protective mode.

### Story 2.9: Add Emergency Controls and Automatic Safe-State Triggers

As an operator,
I want immediate emergency actions and automatic protection behavior,
So that capital is protected during volatility or system uncertainty.

**Dependencies:** Stories 2.6 and 2.8.

**Schema Impact:** Create `safety_control_actions` only.

**Traceability:** FR20; NFR2, NFR5.

**Acceptance Criteria:**

**Scenario A — manual emergency command**
**Given** operator invokes pause, reduce-only, or cancel-all
**When** command is accepted
**Then** control-plane acknowledgment occurs within 1 second and state reflects within 5 seconds.

**Scenario B — automatic safe-state trigger**
**Given** critical safety trigger fires (stale feed, reconciliation-critical fault, or control uncertainty)
**When** automation evaluates trigger
**Then** system transitions to safe-state within 5 seconds without operator intervention.

**Scenario C — post-action verification evidence**
**Given** a manual or automated safety action completed
**When** operator queries action result
**Then** response includes timestamp, actor/source, resulting mode, and correlated audit reference.

## Epic 3: Portfolio Command Center, Alerts & Recovery Operations

Provide an operator-facing command center for allocation, attribution, incident handling, and recovery so users can act quickly and verify outcomes.

### Story 3.1: Build Token-First Dashboard Shell and Navigation Model

As an operator,
I want a consistent dashboard shell with clear navigation and responsive layout,
So that I can orient quickly across risk, execution, and governance workflows.

**Acceptance Criteria:**

**Given** the operator opens the application
**When** core dashboard surfaces render
**Then** the UI uses semantic design tokens, approved typography, and responsive grid rules
**And** left-rail/top-bar/in-page navigation and mobile policy behavior align with UX-DR1, UX-DR2, UX-DR3, UX-DR4, UX-DR16, UX-DR17, and FR26.

### Story 3.2: Implement Risk Posture Banner and Persistent Safety Action Rail

As an operator,
I want always-visible risk status and safety controls,
So that I can intervene within seconds during risky conditions.

**Acceptance Criteria:**

**Given** risk state changes or intervention is required
**When** the operator views the command center
**Then** a stateful risk banner and action rail are visible with explicit control states and confirmations
**And** behavior satisfies UX-DR5, UX-DR6, UX-DR9, UX-DR10, UX-DR13, and UX-DR14.

### Story 3.3: Deliver Allocation Policy and Drift-Rebalance Workflows

As an operator,
I want to configure allocations and review rebalance rationale,
So that portfolio drift can be corrected with auditable intent.

**Acceptance Criteria:**

**Given** allocation policy and drift thresholds are set
**When** drift exceeds policy limits
**Then** the system proposes or executes rebalance with visible rationale and approval context
**And** workflow fulfills FR22 and FR23 with progressive-disclosure controls (UX-DR15).

### Story 3.4: Build Cost-Aware PnL and Attribution Surfaces

As an operator,
I want granular realized/unrealized PnL and cost-aware attribution views,
So that I can understand performance drivers quickly.

**Acceptance Criteria:**

**Given** portfolio and execution data are available
**When** attribution views are queried
**Then** users can inspect PnL by market, alpha, and period with cost-aware breakdowns
**And** modules implement FR24/FR25 with metadata-first cards plus skeleton/empty states per UX-DR12 and UX-DR20.

### Story 3.5: Implement Incident Search and Causal Timeline Forensics

As a support analyst,
I want single-query incident search with causal event timelines,
So that I can diagnose failures and answer “what happened” rapidly.

**Acceptance Criteria:**

**Given** incident filters (market/order/alpha/time) are provided
**When** the query executes
**Then** correlated signal→order→fill→PnL evidence is returned within target latency
**And** timeline UX follows UX-DR7, UX-DR11, and UX-DR22 while satisfying FR27 and FR28.

### Story 3.6: Add Severity-Based Alerts with Recommended Operator Actions

As an operator,
I want timely alerts with clear next-step guidance,
So that I can respond to incidents without ambiguity.

**Dependencies:** Story 3.5.

**Schema Impact:** Create `incident_alerts` and `alert_delivery_attempts` only.

**Traceability:** FR29; NFR15; UX-DR14, UX-DR21.

**Acceptance Criteria:**

**Scenario A — critical alert dispatch SLA**
**Given** a severity-defined condition is triggered
**When** alerting executes
**Then** critical alerts are delivered within 30 seconds and include severity + impacted subsystem.

**Scenario B — required guidance metadata**
**Given** a warning or critical alert payload
**When** it is rendered in operator UI
**Then** payload includes `recommended_next_action`, `evidence_link`, and `issued_at` fields.

**Scenario C — delivery failure fallback**
**Given** primary delivery channel fails
**When** retry policy runs
**Then** fallback channel is attempted and failure trail is auditable.

### Story 3.7: Implement Controlled Recovery Readiness Gates

As an operator,
I want guided readiness checks before resuming trading,
So that post-incident reactivation is safe and policy-compliant.

**Dependencies:** Stories 2.6, 2.8, and 3.6.

**Schema Impact:** Create `recovery_gate_runs` only.

**Traceability:** FR30; NFR6, NFR16; UX-DR22.

**Acceptance Criteria:**

**Scenario A — gate-enforced resume path**
**Given** system is in safe-state after an incident
**When** resume is requested
**Then** resume is blocked until all readiness gates pass (freshness, reconciliation, risk bundle checksum, operator sign-off).

**Scenario B — failed gate behavior**
**Given** any readiness gate fails
**When** recovery evaluation completes
**Then** system remains in safe-state and returns machine-readable failing gate reasons.

**Scenario C — post-resume evidence**
**Given** all gates pass and resume is approved
**When** trading reactivates
**Then** operator sees timestamped verification evidence within 10 seconds.

### Story 3.8: Add Backup Integrity Validation and Deterministic Restore Rehearsal

As an ops administrator,
I want deterministic backup-restore rehearsals with integrity checks,
So that severe incidents can be recovered with verified data consistency.

**Dependencies:** Stories 2.6 and 3.7.

**Schema Impact:** Create `restore_rehearsal_runs` and `backup_integrity_checks` only.

**Traceability:** FR49; NFR6, NFR16.

**Acceptance Criteria:**

**Scenario A — scheduled rehearsal success**
**Given** rehearsal is triggered
**When** restore is executed in controlled environment
**Then** restored state matches expected checksums and reconciliation sanity checks pass.

**Scenario B — integrity failure path**
**Given** checksum or reconciliation mismatch is detected
**When** rehearsal completes
**Then** result is marked failed and production resume remains blocked.

**Scenario C — deterministic replay evidence**
**Given** two rehearsals run on same backup artifact
**When** both complete
**Then** key outputs are identical except for run metadata timestamps.

### Story 3.9: Enforce Accessibility and Reduced-Motion Standards in Critical Flows

As an operator using assistive or reduced-motion settings,
I want critical intervention flows to remain accessible and clear,
So that emergency actions are safe for all users under stress.

**Dependencies:** Stories 3.2 and 3.7.

**Schema Impact:** No schema changes.

**Traceability:** NFR1; UX-DR10, UX-DR18, UX-DR19.

**Acceptance Criteria:**

**Scenario A — keyboard accessibility path**
**Given** keyboard-only navigation
**When** user triggers emergency controls
**Then** full flow remains operable with visible focus and semantic labels.

**Scenario B — reduced motion preference**
**Given** `prefers-reduced-motion` is enabled
**When** critical state transition occurs
**Then** transition uses reduced-motion variant while preserving state clarity.

**Scenario C — assistive announcement timing**
**Given** a critical intervention is executed
**When** confirmation state is published
**Then** assistive announcement includes action outcome and timestamp within 10 seconds.

## Epic 4: Reporting, Exports & External Analytics Integrations

Enable reliable internal and external data consumption through versioned contracts, exports, and scheduled reporting workflows.

### Story 4.1: Define Normalized Reporting Read Models

As an analytics consumer,
I want normalized datasets for trades, positions, risk, and performance,
So that downstream analysis uses consistent semantics.

**Acceptance Criteria:**

**Given** execution and risk events are available
**When** reporting models are generated
**Then** normalized records are exposed for analytics use
**And** data model supports FR35 retrieval requirements.

### Story 4.2: Build Versioned Read-Only API Contracts

As an external integration user,
I want stable, versioned read-only APIs,
So that integrations remain robust across product evolution.

**Acceptance Criteria:**

**Given** API consumers call reporting endpoints
**When** contract versions evolve
**Then** endpoints remain versioned with published schema artifacts and changelogs
**And** compatibility behavior satisfies FR37/NFR13 constraints.

### Story 4.3: Implement Recurring Summary Report Scheduling

As an operator,
I want daily/weekly/monthly report schedules,
So that strategy and risk oversight happens automatically.

**Acceptance Criteria:**

**Given** report cadence configuration is active
**When** scheduled execution windows arrive
**Then** reports are generated at required daily/weekly/monthly cadence
**And** delivery history is auditable per FR38.

### Story 4.4: Deliver Weekly, On-Demand, and Incident Export Workflows

As a governance stakeholder,
I want exportable evidence packages on demand and during incidents,
So that audits and postmortems are fast and complete.

**Acceptance Criteria:**

**Given** an export request is initiated by schedule, manual trigger, or incident trigger
**When** export generation completes
**Then** governance/performance artifacts are packaged and retrievable
**And** export coverage satisfies FR36 requirements.

## Epic 5: Incentive-Regime Adaptive Trading Controls

Enable incentive-aware and liquidity-aware participation controls so trading stays economically rational and risk-bounded as regimes shift.

### Story 5.1: Implement Reward-Per-Risk Policy Configuration and Scoring

As an operator,
I want configurable reward-per-risk scoring policies,
So that participation focuses on high expected-value opportunities.

**Acceptance Criteria:**

**Given** reward, rebate, cost, and volatility inputs are available
**When** strategy routing policy is evaluated
**Then** reward-per-risk scores are computed and compared to configurable thresholds
**And** output is enforceable for FR39 deployment decisions.

### Story 5.2: Add Incentive and Regime Shift Detection Alerts

As an operator,
I want alerts when rebate/spread/eligibility regimes shift materially,
So that I can proactively adjust market participation.

**Acceptance Criteria:**

**Given** live venue economics and market conditions are monitored
**When** configured shift thresholds are exceeded
**Then** regime-shift alerts are emitted with impacted market context
**And** detection logic aligns with FR40 thresholds.

### Story 5.3: Enforce Low-Liquidity and Overnight Participation Guardrails

As a risk-aware trader,
I want automatic participation guardrails under weak liquidity conditions,
So that order behavior de-risks during fragile windows.

**Acceptance Criteria:**

**Given** depth and inactivity metrics breach configured limits
**When** a new quote/order is evaluated
**Then** participation is paused or size-capped according to guardrail policy
**And** behavior satisfies FR41 constraints.

### Story 5.4: Configure Core/Satellite Market Stratification Policies

As an operator,
I want separate policies for core and satellite market buckets,
So that exposure and allocation are tuned by market class.

**Acceptance Criteria:**

**Given** market stratification and policy profiles are configured
**When** routing and risk checks run
**Then** bucket-specific rules are applied consistently
**And** policy outcomes satisfy FR42.

## Epic 6: Research-to-Production Alpha Governance Lifecycle

Provide a governed alpha lifecycle so research ideas can be validated, promoted, monitored, and retired with evidence-backed controls.

### Story 6.1: Build Alpha Hypothesis Registry with Required Metadata

As a research user,
I want to register alpha hypotheses with complete metadata,
So that each candidate has traceable provenance and assumptions.

**Acceptance Criteria:**

**Given** a new alpha hypothesis submission
**When** required fields are validated
**Then** incomplete submissions are rejected and complete submissions are stored with dataset linkage
**And** behavior fulfills FR6.

### Story 6.2: Configure Leakage and Data-Quality Gate Definitions

As a research lead,
I want mandatory leakage and data-quality gates,
So that invalid training/promotion paths are blocked early.

**Acceptance Criteria:**

**Given** gate policies are configured
**When** a candidate enters training or promotion workflow
**Then** forward-bias, leakage, and survivability checks are enforced before progression
**And** gate configuration satisfies FR43.

### Story 6.3: Implement Validation Workflow and Diagnostics Artifact Store

As a research user,
I want end-to-end validation execution with persistent evidence,
So that promotion decisions are evidence-backed and auditable.

**Acceptance Criteria:**

**Given** a candidate is submitted for validation
**When** workflow stages (quality, labeling, purged CV, CPCV, overfit diagnostics) execute
**Then** failures block progression and diagnostics are stored for comparison
**And** FR7 and FR44 requirements are met.

### Story 6.4: Add Shadow-Mode Evaluation Pipeline

As an operator,
I want candidates evaluated in read-only mode before capital deployment,
So that live risk is reduced while signal quality is observed.

**Acceptance Criteria:**

**Given** a validated candidate enters shadow mode
**When** simulated decisions run against live market context
**Then** simulated outcomes are recorded without live order placement
**And** evaluation state is queryable for FR9.

### Story 6.5: Enforce Promotion Thresholds, Evidence Criteria, and Lifecycle Actions

As a governance approver,
I want promotion, pause, and retirement actions gated by complete evidence and sign-off,
So that only qualified alphas reach production.

**Acceptance Criteria:**

**Given** a promotion or lifecycle action request is submitted
**When** thresholds, validation packet completeness, and approvals are evaluated
**Then** action is blocked on missing criteria and allowed only when all gates pass
**And** FR8, FR11, and FR45 are satisfied.

### Story 6.6: Integrate Counterfactual Replay Stress Gates

As a research user,
I want baseline and stressed replay scenarios evaluated automatically,
So that fragile alphas are blocked before live rollout.

**Acceptance Criteria:**

**Given** a promotion candidate reaches final review
**When** counterfactual replay scenarios execute
**Then** promotion is blocked when stressed outcome violates configured tolerance
**And** replay gating behavior satisfies FR46.

### Story 6.7: Implement Live Alpha Health Monitoring and Threshold Detection

As an operator/research lead,
I want real-time alpha health monitoring with threshold detection,
So that performance degradation is detected before losses compound.

**Dependencies:** Stories 6.4 and 6.6.

**Schema Impact:** Create `alpha_health_metrics` and `alpha_threshold_breaches` only.

**Traceability:** FR10, FR47; NFR14, NFR15.

**Acceptance Criteria:**

**Scenario A — live health telemetry path**
**Given** active production alphas
**When** telemetry updates are processed
**Then** rolling Sharpe, drawdown, hit-rate, and stability metrics are updated in near real time.

**Scenario B — threshold breach detection**
**Given** a configured breach threshold is crossed
**When** monitoring evaluation runs
**Then** breach event is recorded and operator alert is emitted with alpha_id and breach_reason.

**Scenario C — boundary condition**
**Given** metric equals threshold boundary exactly
**When** evaluator runs
**Then** behavior follows documented inclusive/exclusive rule and is test-covered.

### Story 6.8: Deliver Alpha Governance Readiness Card UX

As an operator,
I want a governance card that summarizes validation readiness and lifecycle state,
So that I can make promotion/deallocation decisions quickly and safely.

**Dependencies:** Stories 3.1, 3.4, and 6.7.

**Schema Impact:** No schema changes (consumes existing validation and telemetry projections).

**Traceability:** FR10; UX-DR8, UX-DR12, UX-DR20.

**Acceptance Criteria:**

**Scenario A — governance card completeness**
**Given** alpha telemetry and validation artifacts exist
**When** governance card renders
**Then** card shows lifecycle state, validation completeness, shadow stability, and guardrail state.

**Scenario B — missing evidence failure path**
**Given** required evidence artifact is missing
**When** card computes readiness
**Then** readiness displays blocked state with explicit missing artifact list.

**Scenario C — empty/loading states**
**Given** no candidate alphas or delayed data
**When** card is requested
**Then** UI displays tokenized skeleton/empty states with actionable next-step guidance.

### Story 6.9: Enforce Automatic Deallocation and Stop-Research Policies

As a governance approver,
I want automatic deallocation and stop-research actions when policy thresholds are breached,
So that fragile strategies are contained without manual lag.

**Dependencies:** Stories 6.5 and 6.7.

**Schema Impact:** Create `alpha_lifecycle_actions` only.

**Traceability:** FR47, FR48; NFR9, NFR17.

**Acceptance Criteria:**

**Scenario A — deallocation trigger path**
**Given** live alpha breaches configured deallocation threshold
**When** policy engine evaluates the breach
**Then** deallocation action is triggered, audited, and reflected in lifecycle state.

**Scenario B — stop-research criteria path**
**Given** stop-research criteria are met (trade count, Sharpe, or failure-rate thresholds)
**When** evaluator runs
**Then** experimental branch is terminated and decision evidence is stored.

**Scenario C — authorization and audit failure path**
**Given** policy action cannot be authorized or persisted
**When** execution fails
**Then** action remains unapplied, raises critical alert, and records failure with remediation hint.
