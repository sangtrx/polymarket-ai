# Test Automation Summary

## Story

- 2-9-add-emergency-controls-and-automatic-safe-state-triggers

## Generated Tests

### Domain and Persistence Tests

- [x] `crates/domain/src/risk.rs` — emergency-control tests validate reason-code parse determinism, latency boundaries (`<=1s` ack, `<=5s` reflection), stale-feed threshold (`>30s`), manual actor/source invariants, and reduce-only mode compatibility.
- [x] `crates/persistence/src/postgres/safety_controls.rs` — migration scope, constraint/index contracts, canonical validation, and deterministic latest-query ordering for `safety_control_actions`.

### Governance and Control API Tests

- [x] `services/governance-service/src/safety_controls/mod.rs` — manual-role authorization boundaries, automatic trigger transitions, containment-port orchestration failure handling, and required evidence field persistence.
- [x] `services/control-api/src/routes/mod.rs` — emergency pause/reduce-only/cancel-all/query route coverage for accepted/denied/error envelopes, query not-found behavior, and machine-readable reason-code paths.

### Risk and Execution Runtime Tests

- [x] `services/risk-engine/src/gates/mod.rs` — stale-feed, reconciliation-critical, and control-uncertainty failures publish deterministic automatic emergency safe-state signals.
- [x] `services/risk-engine/src/safe_state/mod.rs` — in-memory emergency signal retention preserves drawdown compatibility while adding generic emergency signaling.
- [x] `services/execution-engine/src/orders/mod.rs` — submit path enforces paused/reduce-only containment (including paused-mode deny for reduce-only orders) with no side effects on deny, and cancel-all reuses batch-cancel lifecycle seams.

## Coverage

- Story 2.9 critical flows covered across domain, persistence, governance orchestration, control ingress, risk-trigger signaling, and execution containment:
  - manual pause/reduce-only/cancel-all control paths with auditable evidence,
  - automatic safe-state trigger mapping for stale feed, reconciliation-critical, and unavailable control dependencies,
  - explicit machine-readable failure behavior for malformed, unauthorized, and unavailable dependencies,
  - execution submit gating and cancel-all containment reuse through existing lifecycle transitions.
- Automated Story 2.9 regression inventory in this QA pass: **40 tests passing** (`domain: 6`, `persistence: 5`, `governance: 8`, `control-api: 14`, `risk-engine: 3`, `execution-engine: 4`).

## Execution Result

- `cargo test -p domain risk::tests::emergency_control_` ✅
- `cargo test -p persistence postgres::safety_controls::tests::` ✅
- `cargo test -p governance-service safety_controls::tests::` ✅
- `cargo test -p control-api routes::tests::emergency_` ✅
- `cargo test -p risk-engine gates::tests::stale_feed_gate_failure_` ✅
- `cargo test -p risk-engine gates::tests::reconciliation_halt_failure_` ✅
- `cargo test -p risk-engine gates::tests::unavailable_runtime_state_` ✅
- `cargo test -p execution-engine orders::tests::submit_order_denied_when_emergency_mode_` ✅
- `cargo test -p execution-engine orders::tests::submit_order_allows_reduce_only_orders_when_emergency_mode_is_reduce_only` ✅
- `cargo test -p execution-engine orders::tests::emergency_cancel_all_reuses_batch_cancel_lifecycle_and_preserves_outcomes` ✅
- `npm run --silent qa:test:story-2-9` ✅

---

## Story

- 3-1-build-token-first-dashboard-shell-and-navigation-model

## Generated Tests

### Story 3.1 Shell and Navigation QA

- [x] `tests/story-3-1/operator-shell.story-3-1.test.mjs` — validates semantic token layering, risk token family coverage, no direct hex usage in route/component surfaces, shell landmark semantics (`header`/`nav`/`main`), URL-aware keyboard tabs, responsive 12/8/4 breakpoint policy, monitor-first mobile control defaults, and explicit fallback/freshness evidence fields.
- [x] `tests/api/story-3-1-shell-read-model-api.test.mjs` — validates `resolveShellReadModel` API behavior for default readiness, invalid-state degradation, source sanitization, array-first query parsing, stale-flag normalization, and explicit evidence/timestamp handling.
- [x] `tests/e2e/story-3-1-shell-runtime.e2e.test.mjs` — validates end-to-end shell route composition contracts for route ownership, navigation manifest parity, ready-vs-fallback gating, explicit fallback evidence fields, and monitor-first top-bar policy signaling.

## Coverage

- Story 3.1 shell contracts covered across token architecture, route ownership, accessibility semantics, responsive boundaries, and failure-state evidence:
  - semantic token layering (foundation/domain/product) with risk status family (`normal`, `warning`, `critical`, `locked-safe`),
  - typography/layout primitives with Source Serif 4 + IBM Plex Sans/Mono and deterministic 12/8/4 grid transitions,
  - shared shell composition for dashboard/incidents/governance with primary navigation landmarks and keyboard parity,
  - read-model API normalization for shell state/source/stale/timestamp query inputs and explicit evidence defaults/overrides,
  - explicit read-model fallback states (`loading`, `empty`, `error`, `unauthorized`) including error code, timestamp, stale indicator, and source metadata,
  - reduced-motion and focus-visibility expectations captured in repeatable story-scoped checks.
- Automated Story 3.1 regression inventory in this QA pass: **17 tests passing** (`story-shell: 9`, `api: 4`, `e2e: 4`).

## Execution Result

- `npm run qa:test:story-3-1` ✅
- `node --test tests/api/story-3-1*.test.mjs tests/e2e/story-3-1*.test.mjs` ✅

---

## Story

- 3-2-implement-risk-posture-banner-and-persistent-safety-action-rail

## Generated Tests

### Story 3.2 Risk Banner + Safety Rail QA

- [x] `tests/story-3-2/risk-command-surface.story-3-2.test.mjs` — validates persistent shell composition for banner/rail across dashboard/incidents/governance, accessibility semantics (`role="status"`/`aria-live`), resume gating messaging, and tokenized style contract surfaces.
- [x] `tests/api/story-3-2-safety-control-api.test.mjs` — validates emergency control client contract mapping for accepted/evidence payloads, explicit machine-readable failure propagation (403/503/404 + malformed action-id), canonical action-result endpoint usage, and deterministic risk posture resolver thresholds/gating metadata.
- [x] `tests/e2e/story-3-2-risk-safety-rail.e2e.test.mjs` — validates root QA command wiring, emergency endpoint string contract alignment, removal of Story 3.1 non-interactive destructive-control placeholder copy, and timestamped confirmation/error evidence + danger-dialog semantics in the safety rail.

## Coverage

- Story 3.2 operator console contracts covered across view-model mapping, control API payload handling, shell integration, and UX/accessibility boundaries:
  - risk posture states (`normal`, `warning`, `critical`, `locked-safe`) with deterministic read-model fallback mapping and required-action guidance,
  - persistent privileged safety controls (`pause`, `reduce-only`, `cancel-all`, `resume[gated]`) with explicit hierarchy and confirmation semantics,
  - canonical emergency control API alignment for `/control/emergency/pause`, `/control/emergency/reduce-only`, `/control/emergency/cancel-all`, and `/control/emergency/actions/{action_id}`,
  - machine-readable error behavior with no success-shaped fallback, including explicit unauthorized/dependency-unavailable/not-found surfaces and malformed action-id preflight validation,
  - timestamped action evidence fields (`action_id`, `reason_code`, `resulting_mode`, `correlation_id`, `audit_reference`) plus deterministic stale read-model fallback behavior,
  - token-driven risk/action styling and responsive/accessibility expectations.
- Automated Story 3.2 regression inventory in this QA pass: **17 tests passing** (`story-shell: 4`, `api: 8`, `e2e: 5`).

## Execution Result

- `npm run qa:test:story-3-1` ✅
- `npm run qa:test:story-3-2` ✅
- `npm test` ✅
- `npm run qa:test:story-3-2` ✅ (expanded API/E2E critical-flow coverage)

---

## Story

- 3-3-deliver-allocation-policy-and-drift-rebalance-workflows

## Generated Tests

### Domain, Persistence, Governance, and Control API

- [x] `crates/domain/src/allocation.rs` — drift boundary semantics, default threshold behavior, fail-closed stale/unavailable policy state handling, and recommendation status/reason-code contract coverage.
- [x] `crates/persistence/src/postgres/allocation_policies.rs` — migration/query contract validation for `allocation_policies` and `rebalance_recommendations` with canonical identifiers, status constraints, and deterministic retrieval ordering.
- [x] `services/governance-service/src/allocation_policy/mod.rs` — role boundary checks, critical increase approval-context handling, recommendation lifecycle transitions (`proposed/pending_approval/approved/executed/denied`), and machine-readable error propagation.
- [x] `services/control-api/src/routes/mod.rs` — allocation-policy upsert + pending query + recommendation execute/evaluate routes with canonical authorization/audit reuse, enriched `/control/rebalance` rationale payloads, and explicit machine-error envelope mapping.

### Portfolio Engine and Operator Console

- [x] `services/portfolio-engine/src/allocation/mod.rs` — deterministic drift evaluation seam with read-model recommendation outputs and fail-closed stale/unavailable policy behavior (no execution-state mutation ownership leak).
- [x] `apps/operator-console/src/components/portfolio/{AllocationPolicyForm.tsx,RebalanceRecommendationCard.tsx,PortfolioSummaryCard.tsx}` and `apps/operator-console/src/lib/portfolio/allocation-policy.ts` — progressive-disclosure allocation form, blur validation, risk-impact guidance, recommendation rationale/approval-context evidence rendering, and canonical control-api client contracts.
- [x] `tests/story-3-3/*.test.mjs` — story-surface contracts validate dashboard composition, progressive disclosure copy, and rationale/evidence visibility.
- [x] `tests/api/story-3-3*.test.mjs` — API contract checks validate canonical endpoint wiring plus machine-readable status/error handling across `200/202` happy paths and `400/404/500` critical failures (including field-level diagnostics).
- [x] `tests/e2e/story-3-3*.test.mjs` — E2E contract checks validate route wiring, progressive-disclosure evidence surfaces, and rebalance evaluate → pending-query → execute control-loop affordances.

## Coverage

- Story 3.3 allocation and drift-rebalance contracts covered across backend orchestration, control ingress, runtime seam, and operator-console workflows:
  - allocation policy mutation with pending/approved approval context,
  - deterministic drift boundary behavior (`==` in-bounds, `>` recommendation flow),
  - recommendation lifecycle query + execution surfaces with rationale and next-action guidance,
  - fail-closed machine-readable errors for validation (`400`), not-found (`404`), and dependency/transport (`500+`) paths,
  - progressive-disclosure UX with inline blur validation and explicit recommendation guidance text.
- Automated Story 3.3 regression inventory in this QA refresh: **14 tests passing** (`story: 3`, `api: 7`, `e2e: 4`).

## Execution Result

- `cargo test -p domain allocation::tests::` ✅
- `cargo test -p persistence postgres::allocation_policies::tests::` ✅
- `cargo test -p governance-service allocation_policy::tests::` ✅
- `cargo test -p control-api routes::tests::` ✅
- `cargo test -p portfolio-engine allocation::tests::` ✅
- `npm run web:lint` ✅
- `npm run web:typecheck` ✅
- `npm run web:build` ✅
- `node --test tests/story-3-3/*.test.mjs tests/api/story-3-3*.test.mjs tests/e2e/story-3-3*.test.mjs` ✅
- `npm run --silent qa:test:story-3-3` ✅

---

## Story

- 3-4-build-cost-aware-pnl-and-attribution-surfaces

## Generated Tests

### Domain, Persistence, Runtime, and Control API

- [x] `crates/domain/src/attribution.rs` — canonical period parsing (`1h/24h/30d`), start-inclusive/end-exclusive scope boundaries, deterministic row ordering, zero-activity empty-window semantics, and reason-code validation.
- [x] `crates/persistence/src/postgres/attribution_snapshots.rs` — migration scope checks (`attribution_snapshots` only), finite numeric guardrails, reason-code constraints, and deterministic latest-by-scope ordering.
- [x] `services/portfolio-engine/src/attribution/mod.rs` — attribution read-model seam behavior for ready/empty states, stale-source fail-closed signaling, and boundary-safe aggregation.
- [x] `services/control-api/src/routes/mod.rs` — authenticated attribution query route coverage for accepted metadata-rich payloads, actionable empty windows, invalid-period validation, and projection-unavailable machine errors.

### Operator Console and Story-Scoped QA

- [x] `tests/story-3-4/pnl-attribution.story-3-4.test.mjs` — metadata-first card/table composition, explicit UI state-machine contracts, and Story 3.3 portfolio surface continuity checks.
- [x] `tests/api/story-3-4-attribution-api.test.mjs` — typed attribution client endpoint/query contract checks, invalid-period preflight rejection, unauthorized/dependency machine-readable error propagation, canonical query evidence handling, and malformed payload mismatch handling.
- [x] `tests/e2e/story-3-4-attribution-dashboard.e2e.test.mjs` — dashboard non-regression with Story 3.2/3.3 shells plus attribution style/state integration, canonical period boundary labels, and critical dependency-escalation guidance contracts.

## Coverage

- Story 3.4 attribution contracts covered across domain, persistence, portfolio runtime seam, control-api ingress, operator-console composition, and story-scoped QA automation:
  - cost-aware decomposition fields (realized/unrealized + fees/rebates/incentives + net-cost impact),
  - deterministic temporal/filter semantics (`start_inclusive`, `end_exclusive`, canonical periods),
  - metadata-first evidence rendering (`as_of_utc`, `source`, `reason_code`, `correlation_id`, `snapshot_id`, `run_id`),
  - explicit skeleton/empty/error/critical UI contracts with actionable next-step guidance,
  - machine-readable failure handling for invalid filters and unavailable/stale dependencies.
- Automated Story 3.4 regression inventory in this QA pass: **37 tests passing** (`rust targeted: 22`, `story/api/e2e node tests: 15`).

## Execution Result

- `cargo test -p domain attribution::tests::` ✅
- `cargo test -p persistence postgres::attribution_snapshots::tests::` ✅
- `cargo test -p portfolio-engine attribution::tests::` ✅
- `cargo test -p control-api routes::tests::attribution_` ✅
- `npm run web:lint` ✅
- `npm run web:typecheck` ✅
- `npm run web:build` ✅
- `node --test tests/story-3-4/*.test.mjs tests/api/story-3-4*.test.mjs tests/e2e/story-3-4*.test.mjs` ✅
- `npm run --silent qa:test:story-3-4` ✅

---

## Story

- 3-5-implement-incident-search-and-causal-timeline-forensics

## Generated Tests

### Domain, Persistence, and Control API

- [x] `crates/domain/src/incidents.rs` — validates canonical reason-code parsing, deterministic tie-break ordering, start-inclusive/end-exclusive boundaries, empty-window behavior, and timeline event contract validation.
- [x] `crates/persistence/src/postgres/incident_query_views.rs` — validates migration contract scope (`incident_query_views` only), deterministic query ordering/boundaries, and derived causal-stage evidence mapping from reconciliation/attribution seams.
- [x] `services/control-api/src/routes/mod.rs` — validates authenticated incident forensics route behavior for accepted payloads, actionable empty states, unauthorized access, dependency-unavailable failures, and half-open window rejection.

### Operator Console and Story-Scoped QA

- [x] `tests/story-3-5/incident-forensics.story-3-5.test.mjs` — validates incidents route/client/timeline state-machine contracts and causal-flow rendering requirements.
- [x] `tests/api/story-3-5-incident-api.test.mjs` — validates canonical endpoint wiring, strict query validation, machine-readable error propagation, and malformed-success contract mismatch handling.
- [x] `tests/e2e/story-3-5-incident-timeline.e2e.test.mjs` — validates incident timeline UI integration, shell continuity, and risk-rail composition non-regression.

## Coverage

- Story 3.5 incident forensics contracts covered end-to-end across domain, persistence, control-api ingress, operator-console rendering, and story-scoped QA:
  - canonical single-submit incident filters (`market_id`, `order_id`, `alpha_id`, `actor_id`, `start_ts`, `end_ts`) with explicit field-level validation failures,
  - deterministic timeline semantics (`start_inclusive`, `end_exclusive`, stable ordering on timestamp ties),
  - causal Trigger -> Context -> Action -> Verification framing with recommended next-action guidance,
  - machine-readable unauthorized/dependency-unavailable/malformed-payload failure envelopes with traceable evidence metadata,
  - shell composition continuity with Story 3.1/3.2/3.4 surfaces.
- Automated Story 3.5 regression inventory in this QA pass: **29 tests passing** (`rust targeted: 16`, `story/api/e2e node tests: 13`).

## Execution Result

- `cargo test -p domain incidents::tests::` ✅
- `cargo test -p persistence postgres::incident_query_views::tests::` ✅
- `cargo test -p control-api routes::tests::incident_forensics_` ✅
- `npm run web:lint` ✅
- `npm run web:typecheck` ✅
- `npm run web:build` ✅
- `node --test tests/story-3-5/*.test.mjs tests/api/story-3-5*.test.mjs tests/e2e/story-3-5*.test.mjs` ✅
- `npm run --silent qa:test:story-3-5` ✅
- `npm test` ✅

---

## Story

- 3-7-implement-controlled-recovery-readiness-gates

## Generated Tests

### Domain, Persistence, Governance, Control API, and Risk Runtime

- [x] `crates/domain/src/recovery.rs` — validates deterministic gate boundaries (`freshness <= 30s`, `reconciliation < 0.1%`), strict checksum/signoff contract enforcement, and approved-run verification invariants.
- [x] `crates/persistence/src/postgres/recovery_gate_runs.rs` — validates migration scope and constraints for `recovery_gate_runs`, canonical identifier normalization, and deterministic latest-query behavior.
- [x] `services/governance-service/src/recovery/mod.rs` — validates authorization boundaries, blocked vs approved gate decision behavior, and pre-approved run requirements for resume.
- [x] `services/control-api/src/routes/mod.rs` — validates recovery evaluate/resume/query route envelopes, machine-readable stale-evidence mapping, and correlation-id lookup behavior.
- [x] `services/risk-engine/src/main.rs` — validates fail-closed release gating that requires approved, verification-backed recovery evidence before removing containment blocks.

### Operator Console, Runbooks, and Story-Scoped QA

- [x] `tests/story-3-7/controlled-recovery.story-3-7.test.mjs` — validates endpoint contracts, deterministic gate semantics, runtime recovery integration seams, migration scope, and runbook cross-link continuity.
- [x] `tests/api/story-3-7-recovery-api.test.mjs` — validates typed recovery client mapping for evaluate/resume/query paths, blocked-gate evidence parsing, run-id and correlation query selectors, checksum preflight rejection, unauthorized machine-error propagation, malformed-success contract rejection, and posture state transitions for blocked/completed recovery flows.
- [x] `tests/e2e/story-3-7-controlled-recovery.e2e.test.mjs` — validates safety-rail runtime workflow semantics, blocked-reason rendering order (Trigger -> Context -> Action -> Verification), verification evidence surfaces, accessibility status/escalation semantics, and CSS contract continuity.

## Coverage

- Story 3.7 controlled-recovery contracts covered end-to-end across deterministic gate policy, durable evidence persistence, authenticated control ingress, runtime fail-closed release logic, operator-console workflow, and operations runbooks:
  - readiness gate boundaries and reason-code behavior for freshness/reconciliation/checksum/signoff,
  - explicit machine-readable blocked/approved decision envelopes with traceable `run_id`/`correlation_id`,
  - resume verification evidence propagation into runtime posture and UI verification surfaces,
  - NFR16 query-latency coverage via repeated recovery query route assertions with `p95 <= 5,000ms`,
  - NFR6 workflow-timing instrumentation coverage via readiness/resume timestamp evidence bounded to `<= 10 minutes`,
  - strict malformed-checksum preflight handling with no success-shaped fallback,
  - runbook operational guidance and cross-links with emergency/incident/alerts procedures.
- Automated Story 3.7 API/E2E regression inventory in this QA pass: **23 Story-3.7 node tests passing** (`story: 6`, `api: 11`, `e2e: 6`).

## Execution Result

- `cargo test -p domain recovery::tests::` ✅
- `cargo test -p persistence postgres::recovery_gate_runs::tests::` ✅
- `cargo test -p governance-service recovery::tests::` ✅
- `cargo test -p control-api routes::tests::recovery_` ✅
- `cargo test -p risk-engine tests::approved_recovery_run_requires_valid_resume_verification` ✅
- `npm run web:lint` ✅
- `npm run web:typecheck` ✅
- `npm run web:build` ✅
- `node --test tests/story-3-7/*.test.mjs tests/api/story-3-7*.test.mjs tests/e2e/story-3-7*.test.mjs` ✅ (23 passing)
- `npm run --silent qa:test:story-3-7` ✅
- `npm test` ✅

---

## Story

- 3-8-add-backup-integrity-validation-and-deterministic-restore-rehearsal

## Generated Tests

### Domain, Persistence, Governance, Control API, and Runtime Guardrails

- [x] `crates/domain/src/recovery_rehearsal.rs` — validates strict lowercase 64-hex checksum boundaries, deterministic canonicalization/signature behavior, metadata-exclusion rules, and rehearsal evidence contract invariants.
- [x] `crates/persistence/src/postgres/restore_rehearsals.rs` — validates Story 3.8 migration scope (`restore_rehearsal_runs`, `backup_integrity_checks` only), canonical adapter normalization, deterministic query ordering, and selector behavior by artifact/correlation.
- [x] `services/governance-service/src/recovery/mod.rs` — validates rehearsal execution success/failure evidence, severe-incident resume fail-closed behavior when rehearsal evidence is missing/failed, and successful severe resume when latest relevant rehearsal passes.
- [x] `services/control-api/src/routes/mod.rs` — validates rehearsal execute/query routes (`POST /control/recovery/rehearsals`, `GET /control/recovery/rehearsals/{run_id}`, selector query routes), machine-readable error mapping, and response evidence envelopes.
- [x] `services/risk-engine/src/main.rs` — regression verifies runtime containment release remains gated by approved verification evidence (Story 3.7 semantics preserved while Story 3.8 severe gating is enforced upstream).

### Operator Console, Runbooks, and Story-Scoped QA

- [x] `apps/operator-console/src/lib/risk/control-actions.ts` and `apps/operator-console/src/components/risk/SafetyActionRail.tsx` — typed rehearsal execute/query client contracts, severe selector pass-through, rehearsal evidence rendering (status/failing checks/deterministic replay), and actionable operator guidance.
- [x] `apps/operator-console/src/lib/risk/posture.ts` and `apps/operator-console/src/components/risk/RiskCommandSurface.tsx` — rehearsal selector wiring (`resumeArtifactId`, incident severity/correlation, rehearsal run override) into resume workflow.
- [x] `tests/story-3-8/backup-integrity-rehearsal.story-3-8.test.mjs` — story-surface contract checks for recovery route wiring, deterministic signature/domain constraints, migration scope boundaries, severe gating orchestration seams, operator-console rehearsal visibility, and runbook continuity.
- [x] `tests/api/story-3-8-recovery-rehearsal-api.test.mjs` — API contract checks for deterministic status mapping, rehearsal envelope shape, and dedicated route-test coverage.
- [x] `tests/e2e/story-3-8-recovery-rehearsal.e2e.test.mjs` — E2E contract checks for safety-rail rehearsal visibility and selector wiring continuity.

## Coverage

- Story 3.8 backup-integrity rehearsal contracts covered end-to-end across deterministic domain rules, durable evidence persistence, governance severe-incident policy, authenticated API ingress, operator-console visibility, and runbook guidance:
  - deterministic checksum/reconciliation/replay checks with explicit pass/fail reason codes,
  - severe-incident resume fail-closed policy (`recovery_rehearsal_missing_or_failed`) unless latest relevant rehearsal evidence succeeds,
  - rehearsal execute/query selector contracts by run-id, artifact, and correlation context,
  - operator-console rehearsal evidence visibility with one clear recommended next action,
  - runbook cross-links between recovery readiness, incident forensics, severity alerts, and rehearsal operations.
- Automated Story 3.8 regression inventory in this QA pass: **40 tests passing** (`rust targeted: 28`, `story/api/e2e node tests: 12`).

## Execution Result

- `cargo test -p domain recovery_rehearsal::tests::` ✅
- `cargo test -p persistence postgres::restore_rehearsals::tests::` ✅
- `cargo test -p governance-service recovery::tests::` ✅
- `cargo test -p control-api routes::tests::recovery_` ✅
- `cargo test -p risk-engine tests::approved_recovery_run_requires_valid_resume_verification` ✅
- `npm run web:lint` ✅
- `npm run web:typecheck` ✅
- `npm run web:build` ✅
- `node --test tests/story-3-8/*.test.mjs tests/api/story-3-8*.test.mjs tests/e2e/story-3-8*.test.mjs` ✅ (12 passing)
- `npm run --silent qa:test:story-3-8` ✅

---

## Story 3.6 QA Automation Refresh

### Generated Tests

- [x] `tests/api/story-3-6-alerts-api.test.mjs` — added critical-dispatch latency (`<= 30s`) assertions, explicit fallback attempt ordering/channel assertions, unauthorized (`401`) machine-error propagation checks, and malformed `evidence_link` contract rejection coverage.
- [x] `tests/e2e/story-3-6-alerts-dashboard.e2e.test.mjs` — added fallback delivery-attempt evidence trail assertions and accessible status/error semantic coverage.

## Coverage

- Story 3.6 API/E2E critical-flow refresh verifies:
  - canonical critical alert dispatch latency remains within `<= 30s` using `issued_at` -> `delivered_at` evidence assertions,
  - fallback-attempt evidence trail integrity (`attempt_number`, channel, outcome, attempted/failed timestamps),
  - machine-readable unauthorized behavior for restricted alert queries (`alert_unauthorized`, `401`),
  - strict `evidence_link` URL validation plus accessible alert-panel semantics (`aria-label`, `role="status"`, `role="alert"`).
- Automated Story 3.6 regression inventory in this QA refresh: **19 tests passing** (`story: 5`, `api: 8`, `e2e: 6`).

## Execution Result

- `npm run web:lint` ✅
- `npm run web:typecheck` ✅
- `npm run web:build` ✅
- `node --test tests/story-3-6/*.test.mjs tests/api/story-3-6*.test.mjs tests/e2e/story-3-6*.test.mjs` ✅
- `npm run qa:test:story-3-6` ⚠️ (`cargo` unavailable in this runtime)

---

## Story

- 3-6-add-severity-based-alerts-with-recommended-operator-actions

## Generated Tests

### Domain, Persistence, and Control API

- [x] `crates/domain/src/alerts.rs` — validates FR29 trigger threshold semantics (`>` boundaries), reason-code parsing, canonical identifier composition, dedupe suppression windows, payload/attempt contract constraints, and critical-dispatch SLA guards.
- [x] `crates/persistence/src/postgres/incident_alerts.rs` — validates migration scope (`incident_alerts`, `alert_delivery_attempts` only), constraint/index coverage, canonical lookup enforcement, and deterministic adapter query ordering.
- [x] `services/control-api/src/routes/mod.rs` — validates authenticated alert query/dispatch route behavior for delivered path, primary-failure fallback path, fallback-failure terminal path, threshold boundary rejection, malformed evidence-link rejection, and unauthorized read behavior.

### Operator Console and Story-Scoped QA

- [x] `tests/story-3-6/severity-alerts.story-3-6.test.mjs` — validates incidents/dashboard alert-panel composition, required actionable guidance fields, runbook continuity, and command wiring.
- [x] `tests/api/story-3-6-alerts-api.test.mjs` — validates typed alert client contract parsing, dependency-unavailable machine-error propagation, and malformed-success payload rejection.
- [x] `tests/e2e/story-3-6-alerts-dashboard.e2e.test.mjs` — validates dashboard/incidents alert surface integration and style-contract continuity.

## Coverage

- Story 3.6 severity-alert contracts covered end-to-end across domain trigger evaluation, durable delivery evidence persistence, control-api dispatch/query behavior, and operator-console rendering:
  - deterministic FR29 threshold and dedupe behavior with exact-boundary assertions,
  - required warning/critical guidance metadata (`recommended_next_action`, `evidence_link`, `issued_at`),
  - auditable primary→fallback delivery attempts with explicit machine-readable failure codes,
  - critical dispatch timing assertions for `<= 30s` path behavior,
  - runbook continuity links across incident forensics, alert delivery, and emergency controls.
- Automated Story 3.6 regression inventory in this QA pass: **34 tests passing** (`rust targeted: 22`, `story/api/e2e node tests: 12`).

## Execution Result

- `cargo test -p domain alerts::tests::` ✅
- `cargo test -p persistence postgres::incident_alerts::tests::` ✅
- `cargo test -p control-api routes::tests::incident_alert_` ✅
- `npm run web:lint` ✅
- `npm run web:typecheck` ✅
- `npm run web:build` ✅
- `node --test tests/story-3-6/*.test.mjs tests/api/story-3-6*.test.mjs tests/e2e/story-3-6*.test.mjs` ✅
- `npm run qa:test:story-3-6` ✅
- `npm test` ✅

---

## Story 3.5 QA Automation Refresh

### Generated Tests

- [x] `tests/api/story-3-5-incident-api.test.mjs` — added canonical full-filter serialization coverage, explicit empty-window response handling, and unauthorized (`403`) machine-error propagation.
- [x] `tests/e2e/story-3-5-incident-timeline.e2e.test.mjs` — added canonical filter and UTC window control coverage plus alpha/actor query-serialization contract checks.

## Coverage

- Story 3.5 API/E2E critical-flow refresh verifies:
  - canonical filter serialization for `market_id`, `order_id`, `alpha_id`, and `actor_id`,
  - explicit no-match/empty-window behavior with actionable guidance,
  - machine-readable unauthorized incident read failures (`incident_unauthorized`, `403`),
  - incidents timeline search form completeness with UTC-bounded controls.
- Automated Story 3.5 regression inventory in this QA refresh: **41 tests passing** (`rust targeted: 20`, `story/api/e2e node tests: 21`).

## Execution Result

- `npm run --silent qa:test:story-3-5` ✅

---

## Story 3.5 Code-Review Remediation Re-Run

- Added regression coverage for review-remediated surfaces:
  - UTC normalization for datetime-local incident window input,
  - degraded-severity contract preservation from API to UI parser/state mapping,
  - single-incident causal-flow scope selection,
  - repeated-query p95 incident latency assertion.
- Updated targeted Story 3.5 QA inventory after remediation: **37 tests passing** (`rust targeted: 20`, `story/api/e2e node tests: 17`).

## Execution Result

- `cargo test -p domain incidents::tests::` ✅
- `cargo test -p persistence postgres::incident_query_views::tests::` ✅
- `cargo test -p control-api routes::tests::incident_forensics_` ✅
- `npm run web:lint` ✅
- `npm run web:typecheck` ✅
- `npm run web:build` ✅
- `node --test tests/story-3-5/*.test.mjs tests/api/story-3-5*.test.mjs tests/e2e/story-3-5*.test.mjs` ✅
- `npm run --silent qa:test:story-3-5` ✅
- `npm test` ✅

---

## Story 3.8 QA Automation Refresh

### Generated Tests

- [x] `tests/api/story-3-8-recovery-rehearsal-api.test.mjs` — expanded coverage for machine-readable error status mapping (`invalid_payload`, `unauthorized`, `dependency_unavailable`, `rehearsal_missing_or_failed`), malformed rehearsal JSON handling, and NFR16 query p95 latency guardrails.
- [x] `tests/e2e/story-3-8-recovery-rehearsal.e2e.test.mjs` — expanded severe-incident resume flow coverage for selector-required gating, stale rehearsal evidence reset before lookup, and blocked-with-reasons rehearsal failure diagnostics.

## Coverage

- Story 3.8 API/E2E critical-flow refresh now verifies:
  - explicit error-path status mapping for rehearsal payload, authorization, dependency, and severe resume-blocking reason codes;
  - malformed rehearsal JSON payload mapping to canonical machine-readable errors;
  - rehearsal query p95 latency objective enforcement (`<= 5_000ms`) in route-test coverage;
  - severe incident resume guardrails in the safety rail (selector requirement, stale evidence clearing, blocked failure presentation).
- Automated Story 3.8 regression inventory in this QA refresh: **44 tests passing** (`rust targeted: 28`, `story/api/e2e node tests: 16`).

## Execution Result

- `node --test tests/story-3-8/*.test.mjs tests/api/story-3-8*.test.mjs tests/e2e/story-3-8*.test.mjs` ✅
- `npm run --silent qa:test:story-3-8` ✅

---

## Story

- 3-9-enforce-accessibility-and-reduced-motion-standards-in-critical-flows

## Generated Tests

### Operator Console Accessibility + Reduced-Motion Hardening

- [x] `tests/story-3-9/accessibility-reduced-motion.story-3-9.test.mjs` — validates keyboard confirmation focus-return behavior on critical controls, banner/timeline/alert assistive announcement semantics, reduced-motion CSS contracts, and Story 3.9 runbook scope/cross-link coverage.
- [x] `tests/api/story-3-9-accessibility-contract-api.test.mjs` — validates strict fail-closed `control-actions` parsing for missing/malformed `timestamp_utc` evidence, missing `correlation_id` evidence on accepted emergency payloads, and valid outcome/timestamp mapping continuity.
- [x] `tests/e2e/story-3-9-critical-flow-accessibility.e2e.test.mjs` — validates Story 3.9 QA command wiring, shared shell announcement-region contract, safety-rail keyboard confirmation (`alertdialog`, `aria-modal`, `<= 10s` evidence target), reduced-motion/focus-visible CSS contracts, incident-panel keyboard feedback semantics, and runbook cross-link continuity.

## Coverage

- Story 3.9 accessibility/reduced-motion contracts covered across critical operator-console seams:
  - deterministic keyboard-only danger confirmation flow with Escape cancel handling and explicit focus-return behavior,
  - explicit assistive announcement evidence shape (`outcome`, `reason_code`, `timestamp_utc`, correlation-aware IDs) across banner/safety/timeline/alerts surfaces,
  - reduced-motion contract hardening for risk/safety/incident critical transitions with non-color semantic fallback text,
  - strict machine-readable client parsing for malformed timestamp evidence with no success-shaped fallback in critical control responses,
  - Story 3.9 operations runbook creation and incident/alerts/recovery runbook cross-link updates.
- Automated Story 3.9 regression inventory in this QA refresh: **22 Story-3.9 tests passing** (`story: 6`, `api: 10`, `e2e: 6`).

## Execution Result

- `npm run web:lint` ✅
- `npm run web:typecheck` ✅
- `npm run web:build` ✅
- `node --test tests/story-3-9/*.test.mjs tests/api/story-3-9*.test.mjs tests/e2e/story-3-9*.test.mjs` ✅
- `npm run --silent qa:test:story-3-9` ✅
- `npm run --silent bootstrap:test` ✅
- `node --test tests/story-*/*.test.mjs` ✅

---

## Story 4.1 QA Automation Refresh

### Generated Tests

- [x] `crates/domain/src/reporting.rs` (`reporting::tests::`) — validates reporting reason-code taxonomy, deterministic identifier/timestamp normalization, canonical boundary checks (`start_inclusive_utc`/`end_exclusive_utc`), malformed identifier/non-UTC filter rejection, authorization guard behavior, and deterministic ordering contracts for trade and risk-event read rows.
- [x] `crates/persistence/src/postgres/reporting_read_models.rs` (`postgres::reporting_read_models::tests::`) — validates Story 4.1 migration scope boundaries, deterministic SQL ordering clauses, query-limit/boundary validation, malformed identifier/non-UTC filter rejection, and canonical payload acceptance for trade/position/risk/performance adapter outputs.
- [x] `services/reporting-service/src/read_models/queries.rs` (`read_models::queries::tests::`) — validates orchestration fail-closed semantics for unauthorized roles, dependency-unavailable/stale conditions, malformed request-filter rejection, evidence metadata/timestamp validation, deterministic response shaping, correlation fallback behavior, and Story 4.2 handoff-ready response envelopes.

### Coverage

- Story 4.1 normalized read-model contracts now have automated coverage across domain, persistence, and reporting-service seams:
  - machine-readable error taxonomy and validation issue mapping (`reporting_invalid_payload`, `reporting_unauthorized`, `reporting_dependency_unavailable`, `reporting_stale_dependency`, `reporting_evidence_unavailable`);
  - deterministic ordering tie-break guarantees for all normalized datasets (trade/position/risk_event/performance);
  - strict bounded query behavior (`limit` range, inclusive/exclusive UTC windows, canonical filter normalization);
  - fail-closed orchestration when evidence metadata cannot be produced.
- Automated Story 4.1 regression inventory in this QA refresh: **29 Story-4.1 Rust tests passing** (`domain: 10`, `persistence: 7`, `reporting-service: 12`).

### Execution Result

- `npm run --silent qa:test:story-4-1` ✅
- `npm test` ✅

---

## Story 4.2 QA Automation Refresh

### Generated Tests

- [x] `services/reporting-service/src/contracts/artifacts.rs` (`contracts::artifacts::tests::`) — validates FR37 dataset contract registry completeness (trades/positions/risk-events/performance/alpha-attribution), schema/changelog JSON parseability, deterministic checksum generation, and required envelope metadata fields (`data/meta/error`, `contract_version`, `reason_code`, `correlation_id`).
- [x] `services/reporting-service/src/contracts/lifecycle.rs` (`contracts::lifecycle::tests::`) — validates typed contract lifecycle policy enforcement for NFR13 deprecation/support windows, replacement-support boundary checks, and active-status consistency constraints.
- [x] `crates/persistence/src/postgres/api_contract_versions.rs` (`postgres::api_contract_versions::tests::`) — validates migration scope boundaries (`api_contract_versions` only), lifecycle/index/checksum constraints, active-version fallback SQL semantics, and persistence-level lifecycle/checksum validation behavior.
- [x] `services/reporting-service/src/api.rs` (`api::tests::`) — validates versioned reporting route behavior for success/empty datasets, explicit `contract_version` override forwarding vs active-version fallback resolution, invalid query-window rejection, unauthorized failures, dependency-unavailable and stale-contract fail-closed responses, plus schema/changelog artifact discoverability and runtime envelope/schema synchronization checks.
- [x] `tests/contract/story-4-2-reporting-contract-artifacts.test.mjs` — validates contract-level artifact publication/discoverability for all Story 4.2 dataset schemas and changelog metadata in repository-level contract tests.

### Coverage

- Story 4.2 now has deterministic automated coverage for:
  - `api_contract_versions` lifecycle persistence scope and NFR13 governance windows (90-day notice / 180-day support after replacement),
  - contract-version resolution precedence (`contract_version` override, active fallback),
  - read-only versioned reporting route semantics across all required failure classes (invalid payload, unauthorized, dependency unavailable, stale metadata),
  - machine-readable schema/changelog artifact publication and checksum/path parity validation,
  - canonical contract envelope guarantees (`data`, `meta`, `error`) with traceability fields (`as_of_utc`, `source`, `reason_code`, `correlation_id`, `contract_version`).
- Automated Story 4.2 regression inventory in this QA refresh: **27 Story-4.2 tests passing** (`contracts: 7`, `persistence: 6`, `reporting API routes: 11`, `contract artifact tests: 3`).

### Execution Result

- `npm run --silent qa:test:story-4-2` ✅
- `npm test` ✅

---

## Story 4.3 QA Automation Refresh

### Generated Tests

- [x] `crates/domain/src/reporting_schedule.rs` (`reporting_schedule::tests::`) — validates UTC cadence boundaries, schedule/run contract validation, authorization gates, run-state transitions, and deterministic schedule-window composition.
- [x] `crates/persistence/src/postgres/report_schedules.rs` (`postgres::report_schedules::tests::`) — validates Story 4.3 migration scope (`report_schedules`, `report_runs` only), deterministic due/history retrieval ordering, idempotent run-window uniqueness, and persistence failure taxonomy.
- [x] `services/reporting-service/src/exports/scheduling.rs` (`exports::scheduling::tests::`) — validates scheduler orchestration transitions (`pending -> running -> succeeded|failed|missed`), UTC boundary advancement, fail-closed dependency handling, and alert-link evidence propagation.
- [x] `services/control-api/src/routes/mod.rs` (`routes::tests::report_schedule_`) — validates authenticated schedule mutation/run-history endpoints, machine-readable denial contracts, and control-plane telemetry/audit envelope behavior.
- [x] `tests/contract/story-4-3-recurring-report-scheduling.test.mjs` — validates route discoverability, NFR15 critical-failure escalation invariants, runbook coverage, and root QA script publication.
- [x] `tests/api/story-4-3-recurring-report-scheduling-api.test.mjs` — validates control-api scheduling route/auth wiring, deterministic machine-readable status mapping, and accepted/rejected telemetry+security signal evidence.
- [x] `tests/e2e/story-4-3-recurring-report-scheduling.e2e.test.mjs` — validates scheduler lifecycle/evidence orchestration and runbook-driven pause/resume/recovery operator workflow contracts.

### Coverage

- Story 4.3 now has deterministic automated coverage for:
  - FR38 UTC cadence boundaries (daily/weekly/monthly) and schedule pause/resume lifecycle behavior,
  - auditable run-history evidence persistence with bounded deterministic query behavior,
  - control-plane schedule mutation authorization and machine-readable rejection envelopes,
  - NFR15 critical scheduling failure escalation with runbook-linked alert evidence and fail-closed behavior,
  - API/E2E story-level regression checks that keep scheduling endpoint contracts and end-to-end scheduler flow expectations stable.
- Automated Story 4.3 regression inventory in this QA refresh: **41 Story-4.3 tests passing** (`rust: 31`, `contract/api/e2e: 10`).

### Execution Result

- `npm run --silent qa:test:story-4-3` ✅
- `node --test tests/contract/story-4-3*.test.mjs tests/api/story-4-3*.test.mjs tests/e2e/story-4-3*.test.mjs` ✅
