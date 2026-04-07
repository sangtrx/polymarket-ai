# Story 6.7: Implement Live Alpha Health Monitoring and Threshold Detection

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As a research user,  
I want live alpha health telemetry evaluated continuously against governance thresholds,  
so that degraded models are surfaced immediately before unsafe production drift.

## Acceptance Criteria

1. **Scenario A - Epic 6 BDD live telemetry update contract (story-local):**  
   **Given** an alpha is in live monitoring scope  
   **When** rolling Sharpe, drawdown, hit-rate, and stability telemetry updates arrive  
   **Then** health metrics are persisted and queryable in near-real-time with canonical UTC timestamps  
   **And** behavior satisfies FR10/FR47 governance continuity.

2. **Scenario B - Epic 6 BDD threshold breach and operator alert contract:**  
   **Given** threshold configuration exists for a monitored alpha  
   **When** a live metric breaches a configured threshold  
   **Then** a threshold-breach record is written with `alpha_id`, `metric_key`, `observed_value`, `threshold_value`, `comparator`, and `breach_reason`  
   **And** an operator alert is emitted containing `alpha_id` and `breach_reason` within the NFR15 incident-alert timeline.

3. **Scenario C - Epic 6 BDD exact-boundary behavior contract:**  
   **Given** an observed metric equals a threshold boundary exactly  
   **When** threshold evaluation executes  
   **Then** comparator behavior is explicit, deterministic, and test-covered  
   **And** the boundary rules are fixed as: floor metrics (`rolling_sharpe`, `rolling_hit_rate`, `stability_score`) breach only on `< floor`; ceiling metrics (`rolling_drawdown`) breach only on `> ceiling`; equality follows allow-path.

4. **Scenario D - FR10 attribution window coverage:**  
   **Given** live telemetry ingestion is active  
   **When** health read-model queries execute  
   **Then** each alpha exposes 1-hour, 24-hour, and 30-day attribution windows including `net_pnl`, `rolling_sharpe`, `rolling_hit_rate`, `rolling_drawdown`, and `stability_score`.

5. **Scenario E - Story 6.4 + 6.6 seam reuse contract:**  
   **Given** upstream shadow/replay/promotion evidence already exists  
   **When** live-health orchestration derives eligibility context  
   **Then** Story 6.4 and Story 6.6 seams are reused  
   **And** duplicate validation or replay pipelines are not introduced.

6. **Scenario F - authenticated control-plane surfaces:**  
   **Given** authorized users invoke start/read/list health and breach routes  
   **When** control-plane handlers execute  
   **Then** responses use canonical `data/meta/error` envelopes  
   **And** unauthorized/malformed/dependency-unavailable outcomes map to deterministic status classes.

7. **Scenario G - deterministic list/read query behavior:**  
   **Given** read/list requests include IDs, limits, and optional time windows  
   **When** request normalization and ordering are applied  
   **Then** outputs are deterministic and boundary-tested  
   **And** invalid boundaries fail with explicit field-level diagnostics.

8. **Scenario H - schema isolation and persistence scope:**  
   **Given** this story persists live-health state  
   **When** migrations and adapters are implemented  
   **Then** schema additions are limited to `alpha_health_metrics` and `alpha_threshold_breaches`  
   **And** no unrelated schema entities are introduced.

9. **Scenario I - incident-alert contract reuse:**  
   **Given** a breach requires operator escalation  
   **When** alert payloads are generated  
   **Then** they conform to the existing incident-alert contract (`severity`, `impacted_subsystem`, `cause`, `recommended_next_action`, `evidence_link`, `reason_code`, `correlation_id`)  
   **And** reason-code taxonomy remains parseable by `crates/domain/src/alerts.rs`.

10. **Scenario J - NFR14 observability continuity:**  
    **Given** allow-path, breach-path, and dependency-failure-path executions  
    **When** telemetry is emitted  
    **Then** structured logs/metrics/traces include `alpha_id`, `metric_key`, `reason_code`, `correlation_id`, and UTC timestamps  
    **And** emitted evidence supports incident and QA traceability.

11. **Scenario K - fail-closed dependency handling:**  
    **Given** persistence, alerting, or upstream governance dependencies are unavailable/ambiguous  
    **When** monitoring execution runs  
    **Then** requests fail closed with machine-readable unavailable reason codes  
    **And** no success-shaped fallback bypasses governance.

12. **Scenario L - downstream compatibility boundary:**  
    **Given** Stories 6.8 and 6.9 consume Story 6.7 outputs  
    **When** Story 6.7 is delivered  
    **Then** machine-readable health/breach records are stable for downstream use  
    **And** Story 6.7 does not pre-implement Story 6.8 governance-card UX or Story 6.9 automatic deallocation action execution.

13. **UAC-1 Failure handling:** Invalid payloads, unauthorized access, unavailable dependencies, and malformed threshold rules return explicit machine-readable errors with no unsafe side effects.

14. **UAC-2 Boundary behavior:** Threshold and query-window boundaries are deterministic (inclusive/exclusive semantics documented and test-covered).

15. **UAC-3 Verifiable evidence:** Successful and failed health-monitor operations emit timestamped audit/telemetry evidence suitable for incident and QA traceability.

16. **Schema/dependency/traceability contract:** Story dependency baseline is `6.4` + `6.6`; schema scope is limited to `alpha_health_metrics` and `alpha_threshold_breaches`; traceability maps to `FR10`, `FR47`, `NFR14`, and `NFR15`.

## Tasks / Subtasks

- [x] **Task 1: Define live-health domain contracts and threshold semantics** (AC: 1, 2, 3, 4, 9, 13, 14, 16)
  - [x] Extend `crates/domain/src/research.rs` with canonical live-health metric records, threshold configuration/input contracts, and breach-record contracts.
  - [x] Add deterministic threshold-comparator helpers for floor/ceiling metrics with explicit equality allow-path semantics.
  - [x] Add reason-code families for invalid payload, dependency unavailable, threshold breach, and persistence unavailable outcomes in research-governance flows.
  - [x] Add unit tests for canonicalization, finite-value validation, and exact-boundary behavior (`<`, `==`, `>` cases).

- [x] **Task 2: Add forward-only migration and persistence adapters for live-health state** (AC: 1, 2, 7, 8, 11, 13, 16)
  - [x] Add one migration in `crates/persistence/migrations/` that creates only `alpha_health_metrics` and `alpha_threshold_breaches` with canonical constraints/indexes.
  - [x] Implement Postgres adapters (recommended: `crates/persistence/src/postgres/alpha_health_metrics.rs`) and wire exports via `crates/persistence/src/postgres/mod.rs`.
  - [x] Implement deterministic write/read/list behavior with canonical IDs, stable ordering, and window filtering support.
  - [x] Add persistence tests covering constraints, ordering determinism, timestamp handling, and classification of dependency/persistence failures.

- [x] **Task 3: Implement research-gateway live-health orchestration** (AC: 1, 2, 3, 4, 5, 10, 11, 12)
  - [x] Add a promotion-governance monitoring module (recommended: `services/research-gateway/src/promotion/alpha_health.rs`) and export via `promotion/mod.rs`.
  - [x] Reuse Story 6.4/6.6 seams for candidate/governance context and avoid duplicate upstream pipelines.
  - [x] Implement orchestration for telemetry update, threshold evaluation, breach persistence, and read/list query flows.
  - [x] Ensure fail-closed handling for missing/ambiguous dependency state and emit stable machine-readable reason codes.

- [x] **Task 4: Integrate operator-alert emission for threshold breaches** (AC: 2, 9, 10, 11, 15)
  - [x] Reuse incident-alert domain contracts from `crates/domain/src/alerts.rs`; add alpha-health breach reason-code support if missing.
  - [x] Map breach context to alert payloads containing `alpha_id` and `breach_reason`, plus impacted subsystem, cause, recommended action, and evidence link.
  - [x] Apply dedupe and dispatch semantics consistent with existing alert workflow patterns to avoid alert storms.
  - [x] Add tests validating alert payload contracts, reason-code parsing, and deterministic dispatch-path error mapping.

- [x] **Task 5: Add authenticated control-plane routes for live-health and breaches** (AC: 1, 2, 6, 7, 10, 11, 12)
  - [x] Add/start/read/list routes in `services/control-api/src/routes/mod.rs` under `/control/research/...` route family, matching established naming/envelope conventions.
  - [x] Wire services in `services/control-api/src/main.rs` and middleware access controls in `services/control-api/src/middleware/mod.rs` consistent with current research-governance routes.
  - [x] Validate request payloads/queries and map errors to canonical status classes with field-level diagnostics.
  - [x] Add route tests for success, malformed payload/query, unauthorized role, and dependency-unavailable flows.

- [x] **Task 6: Implement story-scoped QA automation and regression tests** (AC: 1-16)
  - [x] Add Rust unit/integration tests for domain, persistence, and research-gateway orchestration (including exact-boundary threshold behavior).
  - [x] Add control-api tests for canonical `data/meta/error` envelopes and deterministic list/read behavior.
  - [x] Add story-scoped Node QA tests (recommended: `tests/api/story-6-7-*.test.mjs`, `tests/e2e/story-6-7-*.test.mjs`) and wire `qa:test:story-6-7` in `package.json`.
  - [x] Update `_bmad-output/implementation-artifacts/tests/test-summary.md` with Story 6.7 evidence links/results.

- [x] **Task 7: Publish implementation-facing operations guidance** (AC: 2, 10, 12, 15)
  - [x] Add/extend runbook docs under `docs/operations/` for live-health metric interpretation, breach triage, and alert response expectations.
  - [x] Cross-link to existing Story 6.x governance runbooks so on-call responders can navigate replay/promotion/live-health evidence together.
  - [x] Document explicit out-of-scope boundaries for Story 6.8 (UX card) and Story 6.9 (automatic deallocation enforcement action).

## Dev Notes

### Technical Requirements

- Story objective is FR10 + FR47 continuity: surface live alpha health diagnostics in near-real-time and detect/record governance-threshold breaches with operator alerting.
- Required monitored dimensions: rolling Sharpe, rolling drawdown, rolling hit-rate, and stability metrics.
- Boundary semantics are mandatory and non-ambiguous:
  - Floor metrics breach on `< floor`, equality is allow-path.
  - Ceiling metrics breach on `> ceiling`, equality is allow-path.
- Breach records must carry machine-readable cause context (`alpha_id`, `metric_key`, comparator semantics, `breach_reason`) and canonical UTC timestamps.
- Alert payloads must include `alpha_id` and `breach_reason` and reuse existing incident-alert schema for severity/dispatch consistency.
- Preserve strict schema scope: only `alpha_health_metrics` + `alpha_threshold_breaches` are introduced in this story.
- Reuse Story 6.4/6.6 governance seams; do not duplicate shadow/replay pipelines.
- Keep outputs machine-readable for downstream Story 6.8 and Story 6.9 consumption, without implementing those stories here.
- All failure paths remain fail-closed and produce explicit machine-readable diagnostics.

### Architecture Compliance

- Keep research governance business logic inside `services/research-gateway`; use `services/control-api` for authenticated ingress and envelope mapping.
- Follow canonical response envelope (`data`, `meta`, `error`) and deterministic status mapping patterns already used in `/control/research/*`.
- Keep UTC RFC3339 timestamps throughout request, persistence, telemetry, and API responses.
- Preserve canonical identifier normalization rules and explicit field-level validation errors.
- Maintain observability parity with existing governance stories by emitting correlation identifiers and reason-code-rich telemetry on success and failure paths.
- Reuse existing incident-alert contracts and dispatch flow rather than creating a parallel alert system.

### Library & Framework Requirements

- Rust toolchain remains workspace-pinned (`rust-toolchain.toml`: `1.88.0`).
- Keep currently pinned ecosystem versions unless explicitly required by implementation constraints:
  - `axum = 0.8.8` (workspace)
  - `sqlx = 0.8.6` (workspace; latest index shows `0.9.0-alpha.1`, pre-release only)
  - `tokio = 1.48.0` (workspace)
  - `time = 0.3.44` (workspace)
  - `opentelemetry = 0.31.0` (workspace)
  - `polymarket-client-sdk = 0.4.4`
- No opportunistic dependency upgrades are required for Story 6.7.

### File Structure Requirements

- **Domain contracts:** `crates/domain/src/research.rs` (live-health models, threshold semantics, reason codes as needed).
- **Alert reason taxonomy updates (if required):** `crates/domain/src/alerts.rs`.
- **Migrations:** new SQL migration under `crates/persistence/migrations/` introducing only `alpha_health_metrics` and `alpha_threshold_breaches`.
- **Persistence adapters:** `crates/persistence/src/postgres/alpha_health_metrics.rs` and export updates in `crates/persistence/src/postgres/mod.rs`.
- **Research orchestration:** `services/research-gateway/src/promotion/alpha_health.rs` and `services/research-gateway/src/promotion/mod.rs`.
- **Control API ingress:** `services/control-api/src/routes/mod.rs`, plus wiring adjustments in `services/control-api/src/main.rs` and `services/control-api/src/middleware/mod.rs`.
- **QA automation:** `tests/api/story-6-7-*.test.mjs`, `tests/e2e/story-6-7-*.test.mjs`, and `package.json` script wiring.
- **Runbook/docs:** `docs/operations/` live-health monitoring guidance and cross-links to Story 6.4/6.5/6.6 runbooks.

### Testing Requirements

- Domain unit tests for:
  - threshold comparator semantics (especially equality boundaries),
  - canonicalization/validation for live-health payloads,
  - breach reason-code mapping.
- Persistence tests for:
  - strict schema scope (only two new tables),
  - deterministic ordering/filtering and window bounds,
  - classification of persistence/dependency errors.
- Research-gateway tests for:
  - telemetry ingestion + breach detection orchestration,
  - fail-closed behavior on missing dependency state,
  - structured telemetry continuity.
- Control-api tests for:
  - authenticated start/read/list route behavior,
  - canonical `data/meta/error` envelopes and status mapping,
  - malformed query/payload and unauthorized-path diagnostics.
- Story-scoped QA automation:
  - add `qa:test:story-6-7`,
  - include API + E2E assertions for breach event persistence and alert payload fields (`alpha_id`, `breach_reason`).

### Previous Story Intelligence

- Story 6.6 established replay orchestration and stress-gate evidence contracts that should be reused as upstream governance context, not reimplemented.
- Story 6.5 integrated promotion lifecycle decisions and machine-readable threshold outcomes; Story 6.7 should consume those decision seams to identify active candidates deterministically.
- Story 6.4 added shadow evaluation artifacts and route patterns that can seed live-health continuity checks.
- Story 6.3 standardized validation diagnostics contracts and persistence conventions; maintain naming, canonicalization, and error-taxonomy consistency.
- Story 3.6 already implemented severity-based incident alerts and delivery fallback semantics; Story 6.7 should integrate with that alert model instead of adding a separate escalation channel.

### Git Intelligence Summary

- Recent commit cadence is a vertical-slice governance sequence:
  - `1e1e758` - Story 6.6 counterfactual replay integration.
  - `f0ff1b1` - Story 6.5 promotion threshold/lifecycle governance.
  - `5df6e5d` - Story 6.4 shadow mode evaluation pipeline.
  - `92f1fe1` - Story 6.3 validation workflow/artifact store.
  - `2721578` - Story 6.2 leakage/data-quality gate definitions.
- Established pattern: domain contracts + migration/persistence + research-gateway orchestration + control-api routes + story-scoped QA automation + runbook updates.

### Latest Technical Information

- Cargo-index checks confirm workspace versions remain valid for current implementation:
  - `axum` latest stable aligns with pinned `0.8.8`.
  - `sqlx` index latest is pre-release `0.9.0-alpha.1`; stay on pinned stable `0.8.6`.
  - `tokio` latest stable is newer than pinned `1.48.0`, but no story requirement justifies upgrade.
  - `time` latest stable is newer than pinned `0.3.44`, but upgrade is out-of-scope.
  - `opentelemetry` and `polymarket-client-sdk` align with pinned versions in scope.

### Project Context Reference

- `project-context.md` was not found in repository artifacts.
- Story context is derived from `epics.md`, `prd.md`, `architecture.md`, `ux-design-specification.md`, implementation-readiness artifacts, previous Story 6.x files, and current code seams.

### Project Structure Notes

- Story 6.7 aligns with the existing `services/research-gateway` + `services/control-api` + `crates/domain` + `crates/persistence` unified architecture.
- Existing route family and middleware conventions in `services/control-api/src/routes/mod.rs` should be preserved for all new `/control/research/*` monitoring surfaces.
- Existing incident-alert contracts currently model FR29/FR40 reasons; Story 6.7 should extend reason taxonomy as needed for alpha-health breaches while preserving parse compatibility.
- Implementation constraint to document for dev execution: there is no dedicated "active alpha projection" table; active monitoring scope should be derived deterministically from existing promotion-governance records.

### References

- _bmad-output/planning-artifacts/epics.md (Epic 6, Story 6.7, schema/dependency/traceability notes)
- _bmad-output/planning-artifacts/prd.md (Journey 6, FR10, FR47, NFR14, NFR15)
- _bmad-output/planning-artifacts/architecture.md (FR43-FR48 ownership and service-boundary guidance)
- _bmad-output/planning-artifacts/ux-design-specification.md (Journey 3 governance flow and Alpha Governance Card signals)
- _bmad-output/planning-artifacts/implementation-readiness-report-2026-04-05.md
- _bmad-output/planning-artifacts/research/domain-polymarket-rust-clob-profit-research-2026-04-04.md
- _bmad-output/planning-artifacts/research/market-rust-polymarket-trading-system-lopez-de-prado-research-2026-04-04.md
- _bmad-output/planning-artifacts/research/technical-polymarket-claim-validity-audit-research-2026-04-04.md
- _bmad-output/implementation-artifacts/stories/6-3-implement-validation-workflow-and-diagnostics-artifact-store.md
- _bmad-output/implementation-artifacts/stories/6-4-add-shadow-mode-evaluation-pipeline.md
- _bmad-output/implementation-artifacts/stories/6-5-enforce-promotion-thresholds-evidence-criteria-and-lifecycle-actions.md
- _bmad-output/implementation-artifacts/stories/6-6-integrate-counterfactual-replay-stress-gates.md
- docs/operations/alpha-validation-workflow-and-diagnostics.md
- docs/operations/alpha-shadow-mode-evaluation.md
- docs/operations/alpha-promotion-lifecycle-governance.md
- docs/operations/severity-alert-delivery.md
- services/research-gateway/src/{lib.rs,promotion/mod.rs,promotion/decisions.rs,promotion/counterfactual_replay.rs,validation/mod.rs}
- services/control-api/src/{main.rs,middleware/mod.rs,routes/mod.rs}
- crates/domain/src/{research.rs,alerts.rs}
- crates/persistence/src/postgres/{mod.rs,promotion_decisions.rs,validation_artifacts.rs,incident_alerts.rs}
- crates/persistence/migrations/{20260407193000_validation_runs_validation_artifacts.sql,20260407210000_shadow_evaluations.sql,20260407223000_promotion_decisions.sql}
- package.json
- git --no-pager log --oneline -5
- source "$HOME/.cargo/env" && cargo search axum --limit 1
- source "$HOME/.cargo/env" && cargo search sqlx --limit 1
- source "$HOME/.cargo/env" && cargo search tokio --limit 1
- source "$HOME/.cargo/env" && cargo search time --limit 1
- source "$HOME/.cargo/env" && cargo search polymarket-client-sdk --limit 1
- source "$HOME/.cargo/env" && cargo search opentelemetry --limit 1

## Story Completion Status

- Story 6.7 implementation is complete and ready for review.
- Acceptance criteria coverage includes domain threshold semantics, persistence/schema scope isolation, research-gateway orchestration + alerting, authenticated control-plane ingress, deterministic query/list boundaries, and Story 6.7 QA automation wiring.

## Dev Agent Record

### Agent Model Used

GPT-5.3-Codex (gpt-5.3-codex)

### Debug Log References

- `source "$HOME/.cargo/env" && cargo test -p persistence postgres::alpha_health_metrics::tests::`
- `source "$HOME/.cargo/env" && cargo test -p research-gateway promotion::alpha_health::tests::`
- `source "$HOME/.cargo/env" && cargo test -p control-api routes::tests::alpha_health`
- `node --test tests/api/story-6-7*.test.mjs tests/e2e/story-6-7*.test.mjs`
- `source "$HOME/.cargo/env" && npm run --silent qa:test:story-6-7`
- `source "$HOME/.cargo/env" && npm test`
- `node --test tests/api/story-6-7*.test.mjs tests/e2e/story-6-7*.test.mjs` (2026-04-08 QA automation refresh)
- `source "$HOME/.cargo/env" && npm run --silent qa:test:story-6-7` (2026-04-08 QA automation refresh)

### Completion Notes List

- Implemented Story 6.7 domain contracts for alpha-health telemetry + threshold breach semantics, including deterministic floor/ceiling comparator behavior and FR10 required-window validation.
- Added isolated persistence migration + adapter for `alpha_health_metrics` and `alpha_threshold_breaches` with deterministic ordering and explicit boundary diagnostics.
- Implemented research-gateway alpha-health orchestration for start/read/list metric and breach flows with Story 6.4/6.6 seam reuse, fail-closed dependency handling, telemetry continuity, and breach alert emission contract reuse.
- Added authenticated control-api alpha-health metric/breach routes, canonical envelopes, deterministic status/error mapping, middleware/main wiring, and route-level negative-path tests.
- Added Story 6.7 Node API/E2E contract tests, `qa:test:story-6-7` script wiring, Story 6.7 test-summary evidence, and live-health operations runbook with Story 6.x cross-links.
- Code-review remediation completed: fixed alpha-health error-status mapping (`alpha_health_constraint_violation` + not-found 404), enforced non-empty/unique threshold definitions, made metric+breach persistence atomic, added fail-closed telemetry on dependency/persistence/alert failure paths, and hardened per-breach alert dedupe keying.
- Review cross-check: working-tree discrepancy `.scripts/bmad-auto/copilot/bmad-progress.log` was detected outside application source and excluded from review scope per workflow rules.
- QA automation refresh expanded Story 6.7 Node contract coverage for AC2 breach DTO field completeness, exact-boundary equality allow-path assertions, and incident-alert payload contract reuse fields, then re-ran `qa:test:story-6-7` successfully.

### File List

- crates/domain/src/research.rs
- crates/domain/src/alerts.rs
- crates/persistence/migrations/20260408023000_alpha_health_metrics_threshold_breaches.sql
- crates/persistence/src/postgres/alpha_health_metrics.rs
- crates/persistence/src/postgres/mod.rs
- services/research-gateway/src/promotion/alpha_health.rs
- services/research-gateway/src/promotion/mod.rs
- services/control-api/src/middleware/mod.rs
- services/control-api/src/main.rs
- services/control-api/src/routes/mod.rs
- tests/api/story-6-7-live-alpha-health-monitoring-api.test.mjs
- tests/e2e/story-6-7-live-alpha-health-monitoring.e2e.test.mjs
- docs/operations/alpha-live-health-monitoring-threshold-breaches.md
- docs/operations/alpha-shadow-mode-evaluation.md
- docs/operations/alpha-counterfactual-replay-stress-gating.md
- docs/operations/alpha-promotion-lifecycle-governance.md
- package.json
- _bmad-output/implementation-artifacts/tests/test-summary.md
- _bmad-output/implementation-artifacts/stories/6-7-implement-live-alpha-health-monitoring-and-threshold-detection.md
- _bmad-output/implementation-artifacts/sprint-status.yaml

### Change Log

- 2026-04-08: Created Story 6.7 ready-for-dev context via automated create-story workflow execution.
- 2026-04-08: Implemented Story 6.7 alpha-health domain/persistence/research-gateway/control-api vertical slice with route tests and schema-scope isolation.
- 2026-04-08: Added Story 6.7 QA automation (`tests/api`, `tests/e2e`, `qa:test:story-6-7`) and operations runbook + cross-links; executed full Story 6.7 QA command and full repository regression (`npm test`).
- 2026-04-08: Executed adversarial code-review remediation pass; fixed medium/high findings (atomic persistence path, threshold validation hardening, fail-closed telemetry emission, per-breach alert dedupe keying, and deterministic HTTP status corrections).
- 2026-04-08: Ran BMAD QA automation refresh for Story 6.7, expanded API/E2E contract checks (breach DTO required fields, exact-boundary allow-path assertions, incident-alert payload fields), and re-validated with `qa:test:story-6-7`.
