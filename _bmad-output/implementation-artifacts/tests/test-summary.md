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
