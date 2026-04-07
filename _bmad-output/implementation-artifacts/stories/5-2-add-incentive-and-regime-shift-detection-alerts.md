# Story 5.2: Add Incentive and Regime Shift Detection Alerts

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an operator,  
I want alerts when rebate/spread/eligibility regimes shift materially,  
so that I can proactively adjust market participation.

## Acceptance Criteria

1. **Scenario A - Story-local BDD baseline:**  
   **Given** live venue economics and market conditions are monitored  
   **When** configured shift thresholds are exceeded  
   **Then** regime-shift alerts are emitted with impacted market context  
   **And** detection logic aligns with FR40 thresholds.

2. **Scenario B - FR40 threshold semantics are deterministic:**  
   **Given** prior and current economics observations for a market  
   **When** rebate-rate delta exceeds `20 bps` or spread widening exceeds `50 bps`  
   **Then** the corresponding regime-shift reason code is emitted  
   **And** exact boundaries (`20 bps`, `50 bps`) do not trigger (strict `>` semantics).

3. **Scenario C - Eligibility transition detection:**  
   **Given** venue eligibility state is tracked per market  
   **When** state transitions between `eligible` and `restricted/ineligible`  
   **Then** an eligibility-shift alert is emitted with before/after status values  
   **And** alert context includes market and cluster identifiers.

4. **Scenario D - Alert payload contract for operator actionability:**  
   **Given** a regime shift is detected  
   **When** alert payload is produced  
   **Then** payload includes `reason_code`, `severity`, `market_id`, `cluster_id`, `observed_at`, and `correlation_id`  
   **And** includes `recommended_next_action` plus `evidence_link` for runbook-driven response.

5. **Scenario E - Existing alert dispatch path reuse (no parallel stack):**  
   **Given** a regime-shift alert candidate is produced  
   **When** dispatch occurs  
   **Then** delivery/fallback behavior reuses existing Story 3.6 incident alert dispatch contracts  
   **And** dedupe behavior prevents duplicate fan-out for same reason/correlation window.

6. **Scenario F - Fail-closed dependency behavior:**  
   **Given** required baseline data, persistence, or dispatch dependencies are unavailable  
   **When** detection/dispatch executes  
   **Then** system returns explicit machine-readable dependency/unavailable errors  
   **And** does not emit success-shaped responses.

7. **Scenario G - Regime-shift evidence persistence:**  
   **Given** a regime-shift alert is emitted  
   **When** persistence succeeds  
   **Then** an evidence row is stored in `regime_shift_alerts` with threshold/before/after context and UTC timestamps  
   **And** evidence is queryable for incident and QA traceability.

8. **UAC-1 Failure handling:** Invalid payloads, malformed identifiers/timestamps, unsupported eligibility states, and unavailable dependency paths must return explicit machine-readable errors with no unsafe side effects.

9. **UAC-2 Boundary behavior:** Rebate/spread/eligibility transition boundaries must be deterministic and test-covered, including strict-threshold behavior and dedupe-window behavior.

10. **UAC-3 Verifiable evidence:** Successful and failed regime-shift alert operations must emit timestamped telemetry/audit evidence suitable for incident and QA traceability.

11. **Schema/dependency/traceability contract:** Story depends only on `5.1`, introduces only `regime_shift_alerts`, and maps explicitly to `FR40` and `NFR15`.

12. **Scope boundary contract:** Story 5.2 delivers regime-shift detection and alerting only; low-liquidity/overnight guardrails (Story 5.3) and core/satellite stratification policy (Story 5.4) are out of scope.

## Tasks / Subtasks

- [x] **Task 1: Define FR40 regime-shift detection contracts and reason-code taxonomy** (AC: 1, 2, 3, 4, 6, 8, 9, 11)
  - [x] Extend domain contracts for regime-shift input/evaluation using existing market economics seams in `crates/domain/src/risk.rs` (do not create parallel market snapshot stacks).
  - [x] Add deterministic FR40 threshold evaluation helpers for rebate delta, spread widening, and eligibility transitions with strict boundary semantics.
  - [x] Extend alert reason-code taxonomy and validation in `crates/domain/src/alerts.rs` to include FR40-specific alert reasons while preserving existing Story 3.6 contracts.
  - [x] Add canonical validation for identifiers, timestamps, and eligibility-state transitions with explicit field-level machine-readable issues.

- [x] **Task 2: Add forward-only migration and persistence adapter for `regime_shift_alerts`** (AC: 4, 6, 7, 8, 9, 11)
  - [x] Add migration under `crates/persistence/migrations/` creating only `regime_shift_alerts` with canonical IDs, threshold/before/after fields, reason/severity, operator guidance fields, correlation, and UTC timestamps.
  - [x] Add persistence adapter module (for example `crates/persistence/src/postgres/regime_shift_alerts.rs`) and wire it via `crates/persistence/src/postgres/mod.rs`.
  - [x] Add deterministic query ordering/indexes for market/time, reason/time, and correlation/time triage.
  - [x] Add persistence tests for schema constraints, strict scope boundaries, and machine-readable adapter errors.

- [x] **Task 3: Integrate runtime regime-shift detection into risk-engine economics flow** (AC: 1, 2, 3, 6, 7, 9)
  - [x] Reuse existing `MarketSnapshot` runtime seams in `services/risk-engine/src/{main.rs,gates/mod.rs}` to compare current observations to prior baseline context.
  - [x] Evaluate FR40 thresholds deterministically and emit normalized regime-shift evidence candidates with market/cluster context and reason code.
  - [x] Preserve fail-closed semantics for unavailable required economics inputs (no silent defaults).
  - [x] Ensure detection logic composes with existing pre-trade gate ordering and does not regress Story 5.1 reward-risk behavior.

- [x] **Task 4: Reuse existing incident-alert dispatch contracts for FR40 notifications** (AC: 4, 5, 6, 10)
  - [x] Integrate FR40 alert candidates through existing Story 3.6 alert dispatch surfaces in `services/control-api/src/routes/mod.rs` and domain alert contracts, rather than introducing a parallel dispatch pipeline.
  - [x] Ensure response/error envelopes remain canonical (`data/meta/error` patterns and machine-readable codes).
  - [x] Ensure NFR15-aligned payload content for critical alerts includes cause, impacted systems, and runbook link context.
  - [x] Keep dedupe and fallback delivery semantics aligned with established `incident_alerts` + `alert_delivery_attempts` behavior.

- [x] **Task 5: Expose authenticated regime-shift evidence retrieval surface** (AC: 4, 5, 7, 10, 11)
  - [x] Add authenticated control-plane retrieval path for `regime_shift_alerts` evidence (query by market/reason/correlation/time) using existing route/authorization conventions.
  - [x] Include machine-readable `reason_code`, deterministic ordering, and explicit data-state metadata for operator consumption.
  - [x] Preserve privileged audit emission for read operations.

- [x] **Task 6: Add Story 5.2 deterministic QA coverage and command wiring** (AC: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11)
  - [x] Add domain tests for FR40 threshold boundaries (`> 20`, `> 50`) and eligibility transitions.
  - [x] Add persistence tests for `regime_shift_alerts` schema and adapter behavior.
  - [x] Add control-api tests for alert payload contracts, machine-readable error mapping, dedupe behavior, and dispatch integration.
  - [x] Add risk-engine tests for unavailable-state fail-closed handling and continuity with reward-risk pre-trade seams.
  - [x] Add `qa:test:story-5-2` in root `package.json` chaining story-scoped Rust/Node suites consistent with existing `qa:test:story-*` conventions.

- [x] **Task 7: Add FR40 operations runbook and evidence documentation updates** (AC: 4, 7, 10)
  - [x] Add `docs/operations/incentive-regime-shift-alerts.md` documenting trigger matrix, threshold semantics, dedupe behavior, and remediation guidance.
  - [x] Cross-link `docs/operations/reward-risk-policy-operations.md` and `docs/operations/severity-alert-delivery.md` for operator flow continuity.
  - [x] Update `_bmad-output/implementation-artifacts/tests/test-summary.md` with Story 5.2 evidence after implementation.

### Review Findings

- [x] [Review][Patch] Wrap FR40 incident-alert + evidence persistence writes in a single transaction to prevent partial commits [services/control-api/src/routes/mod.rs:2488]
- [x] [Review][Patch] Fail closed for regime-shift query/dispatch when persistence dependency is unavailable [services/control-api/src/routes/mod.rs:2040]
- [x] [Review][Patch] Enforce FR40 observation chronology (`current.observed_at_utc >= previous.observed_at_utc`) [crates/domain/src/risk.rs:658]
- [x] [Review][Patch] Enforce persisted evidence chronology (`issued_at >= observed_at`) in validation and migration constraints [crates/persistence/src/postgres/regime_shift_alerts.rs:425]
- [x] [Review][Patch] Scope FR40 dispatch correlation IDs by market/cluster to avoid cross-market dedupe collisions [services/control-api/src/routes/mod.rs:2178]
- [x] [Review][Patch] Update Story 5.2 static API review assertions for transaction-backed dispatch writes [tests/api/story-5-2-regime-shift-alerts-api.test.mjs:22]
- [x] [Review][Defer] Runtime auto-dispatch from risk-engine detection buffers remains a cross-service orchestration follow-up (outside current control-plane dispatch scope) [services/risk-engine/src/main.rs:240] — deferred, pre-existing

## Dev Notes

### Technical Requirements

- Story objective is FR40 detect-and-notify behavior for materially shifting economics:
  - rebate-rate change `> 20 bps`,
  - spread-regime widening `> 50 bps`,
  - venue-eligibility transition (`eligible` <-> `restricted/ineligible`).
- Story dependency and scope contract:
  - dependency: `5.1`,
  - schema scope: `regime_shift_alerts` only,
  - traceability: `FR40`, `NFR15`.
- Alert payloads must include impacted market context and operator guidance (`recommended_next_action`, `evidence_link`) aligned with NFR15 incident alert content expectations.
- Detection thresholds are configurable but must preserve FR40 default semantics and deterministic boundary behavior.
- **Out of scope for Story 5.2:** FR41 participation guardrails (Story 5.3) and FR42 stratification policies (Story 5.4).

[Source: _bmad-output/planning-artifacts/epics.md#Story 5.2: Add Incentive and Regime Shift Detection Alerts]  
[Source: _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)]  
[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: _bmad-output/planning-artifacts/prd.md#Incentive Intelligence & Market Regime Management]  
[Source: _bmad-output/planning-artifacts/prd.md#Observability & Operability]  
[Source: _bmad-output/planning-artifacts/prd.md#Journey 7 — Incentive Shift Response: Sang, Independent Quant Operator]

### Architecture Compliance

- Preserve bounded responsibilities:
  - `risk-engine` remains runtime evaluator for market/risk eligibility seams,
  - `control-api` remains authenticated control-plane boundary for alert retrieval/dispatch,
  - `crates/persistence` owns durable alert evidence rows,
  - `execution-engine` must not mutate policy/alert state directly.
- Reuse canonical architecture patterns:
  - machine-readable error contracts and stable reason codes,
  - ISO-8601 UTC timestamps only,
  - strict fail-closed behavior for unavailable/ambiguous state.
- Do not create duplicate alert dispatch systems; reuse established Story 3.6 incident-alert pipeline and its fallback/dedupe semantics.

[Source: _bmad-output/planning-artifacts/architecture.md#Architectural Boundaries]  
[Source: _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: _bmad-output/planning-artifacts/architecture.md#Integration Points]  
[Source: _bmad-output/implementation-artifacts/stories/3-6-add-severity-based-alerts-with-recommended-operator-actions.md#Architecture Compliance]

### Library & Framework Requirements

- Keep workspace-pinned dependencies for compatibility:
  - `axum = 0.8.8`
  - `sqlx = 0.8.6`
  - `tokio = 1.48.0`
  - `time = 0.3.44`
  - `polymarket-client-sdk = 0.4.4`
- Latest checks at story creation time:
  - `axum` latest stable remains `0.8.8`,
  - `sqlx` latest indexed crate is `0.9.0-alpha.1` (pre-release), stable workspace target remains `0.8.6`,
  - `tokio` latest stable `1.51.0`,
  - `time` latest stable `0.3.47`,
  - `polymarket-client-sdk` latest stable `0.4.4`.
- Do not perform opportunistic dependency upgrades in Story 5.2.

[Source: Cargo.toml]  
[Source: source $HOME/.cargo/env && cargo search axum --limit 1]  
[Source: source $HOME/.cargo/env && cargo search sqlx --limit 1]  
[Source: source $HOME/.cargo/env && cargo search tokio --limit 1]  
[Source: source $HOME/.cargo/env && cargo search time --limit 1]  
[Source: source $HOME/.cargo/env && cargo search polymarket-client-sdk --limit 1]

### File Structure Requirements

- Primary implementation surfaces for Story 5.2:
  - `crates/domain/src/{risk.rs,alerts.rs,lib.rs}`
  - `crates/persistence/migrations/*regime_shift_alerts*.sql`
  - `crates/persistence/src/postgres/{mod.rs,regime_shift_alerts.rs}`
  - `services/risk-engine/src/{main.rs,gates/mod.rs}`
  - `services/control-api/src/{main.rs,middleware/mod.rs,routes/mod.rs}`
  - `docs/operations/{incentive-regime-shift-alerts.md,reward-risk-policy-operations.md,severity-alert-delivery.md}`
  - `tests/api/story-5-2*.test.mjs`
  - `tests/e2e/story-5-2*.test.mjs`
  - `package.json`
  - `_bmad-output/implementation-artifacts/tests/test-summary.md`
- Reuse established vertical-slice sequence from recent stories:
  - domain contracts -> migration -> persistence adapter -> runtime/control integration -> tests -> runbook.
- Keep schema/story boundaries strict; do not introduce Story 5.3/5.4 entities in Story 5.2 migration scope.

[Source: _bmad-output/planning-artifacts/architecture.md#Complete Project Directory Structure]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: _bmad-output/implementation-artifacts/stories/5-1-implement-reward-per-risk-policy-configuration-and-scoring.md#File Structure Requirements]  
[Source: crates/persistence/src/postgres/mod.rs]  
[Source: services/control-api/src/routes/mod.rs]

### Testing Requirements

- Add deterministic coverage for:
  - FR40 threshold boundaries and strict `>` semantics for rebate/spread shifts,
  - eligibility-state transition detection and before/after context correctness,
  - required alert payload fields (`reason_code`, market context, recommended next action, evidence link),
  - dedupe-window suppression behavior for repeated same-reason events,
  - fail-closed unavailable-state and dependency-failure error contracts,
  - NFR15 dispatch-content requirements and SLA-compatible dispatch pathway behavior.
- Keep test layering aligned with repository conventions:
  - domain contract tests in `crates/domain`,
  - migration/adapter tests in `crates/persistence`,
  - route/contract tests in `services/control-api`,
  - runtime detection tests in `services/risk-engine`,
  - story-scoped API/E2E suites under `tests/`.
- Add story QA command:
  - `qa:test:story-5-2` in root `package.json`.

[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: _bmad-output/planning-artifacts/prd.md#Incentive Intelligence & Market Regime Management]  
[Source: _bmad-output/planning-artifacts/prd.md#Observability & Operability]  
[Source: package.json]  
[Source: _bmad-output/implementation-artifacts/stories/5-1-implement-reward-per-risk-policy-configuration-and-scoring.md#Testing Requirements]  
[Source: _bmad-output/implementation-artifacts/stories/3-6-add-severity-based-alerts-with-recommended-operator-actions.md#Testing Requirements]

### Previous Story Intelligence

- Story 5.1 already introduced reward-per-risk policy seams and FR39 economics fields on `MarketSnapshot`; Story 5.2 should build on these existing fields instead of creating parallel economics payload models.
- Story 5.1 also established fail-closed reward-risk handling and pre-trade integration; regime-shift detection should preserve that fail-closed posture and avoid gate-order regressions.
- Story 3.6 already delivered canonical incident-alert reason-code, dedupe, and fallback dispatch infrastructure; Story 5.2 should extend/reuse those contracts for FR40 notifications rather than introducing a second alert stack.

[Source: _bmad-output/implementation-artifacts/stories/5-1-implement-reward-per-risk-policy-configuration-and-scoring.md#Technical Requirements]  
[Source: _bmad-output/implementation-artifacts/stories/5-1-implement-reward-per-risk-policy-configuration-and-scoring.md#Project Structure Notes]  
[Source: services/risk-engine/src/gates/mod.rs]  
[Source: crates/domain/src/risk.rs]  
[Source: crates/domain/src/alerts.rs]  
[Source: services/control-api/src/routes/mod.rs]

### Git Intelligence Summary

- Recent commit history (`4-1` through `5-1`) follows a stable pattern:
  1. domain contracts and reason-code updates,
  2. forward-only migration plus persistence adapter,
  3. service wiring and control-plane route integration,
  4. story-scoped QA command/tests and runbook updates.
- Story 5.2 should follow the same sequence to minimize regression risk and maintain consistency.

[Source: git --no-pager log --oneline -5]  
[Source: git --no-pager log -5 --name-only --pretty=format:'--- %h %s']

### Latest Technical Information

- Current workspace versions remain compatible with Story 5.2 implementation goals; no mandatory upgrades are required.
- `sqlx` available newest listing is pre-release (`0.9.0-alpha.1`), so implementation should stay on stable workspace `0.8.6`.
- Dependency-drift risk remains low if Story 5.2 stays within existing stack and existing alert/runtime seams.

[Source: Cargo.toml]  
[Source: source $HOME/.cargo/env && cargo search axum --limit 1]  
[Source: source $HOME/.cargo/env && cargo search sqlx --limit 1]  
[Source: source $HOME/.cargo/env && cargo search tokio --limit 1]  
[Source: source $HOME/.cargo/env && cargo search time --limit 1]  
[Source: source $HOME/.cargo/env && cargo search polymarket-client-sdk --limit 1]

### Project Context Reference

- No `project-context.md` file was found during discovery.
- Story context is derived from epics/PRD/architecture/UX artifacts, Story 5.1 implementation context, existing alert/runtime code seams, and recent git history.

### Project Structure Notes

- Existing FR40-adjacent seams already available:
  - economics fields on `MarketSnapshot` in `crates/domain/src/risk.rs`,
  - reward-risk runtime policy state in `services/risk-engine/src/gates/mod.rs`,
  - incident alert dispatch/query routes in `services/control-api/src/routes/mod.rs`,
  - persistent alert evidence in `incident_alerts` + `alert_delivery_attempts`.
- Story 5.2 should extend these seams with `regime_shift_alerts` evidence and FR40 trigger classification, not duplicate existing alert transport or operator-notification primitives.
- Keep market/risk/alert signals correlated via canonical `correlation_id` and UTC evidence timestamps.

[Source: crates/domain/src/risk.rs]  
[Source: crates/domain/src/alerts.rs]  
[Source: services/risk-engine/src/gates/mod.rs]  
[Source: services/risk-engine/src/main.rs]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: crates/persistence/migrations/20260406210000_incident_alerts_delivery_attempts.sql]  
[Source: crates/persistence/src/postgres/incident_alerts.rs]

### References

- _bmad-output/planning-artifacts/epics.md#Epic 5: Incentive-Regime Adaptive Trading Controls  
- _bmad-output/planning-artifacts/epics.md#Story 5.2: Add Incentive and Regime Shift Detection Alerts  
- _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)  
- _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)  
- _bmad-output/planning-artifacts/prd.md#Journey 7 — Incentive Shift Response: Sang, Independent Quant Operator  
- _bmad-output/planning-artifacts/prd.md#Incentive Intelligence & Market Regime Management  
- _bmad-output/planning-artifacts/prd.md#Observability & Operability  
- _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules  
- _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries  
- _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping  
- _bmad-output/planning-artifacts/ux-design-specification.md#Feedback Patterns  
- _bmad-output/planning-artifacts/ux-design-specification.md#Component Strategy  
- _bmad-output/implementation-artifacts/stories/5-1-implement-reward-per-risk-policy-configuration-and-scoring.md  
- _bmad-output/implementation-artifacts/stories/3-6-add-severity-based-alerts-with-recommended-operator-actions.md  
- crates/domain/src/risk.rs  
- crates/domain/src/alerts.rs  
- crates/persistence/src/postgres/mod.rs  
- crates/persistence/src/postgres/incident_alerts.rs  
- crates/persistence/migrations/20260406210000_incident_alerts_delivery_attempts.sql  
- services/risk-engine/src/main.rs  
- services/risk-engine/src/gates/mod.rs  
- services/control-api/src/routes/mod.rs  
- docs/operations/reward-risk-policy-operations.md  
- docs/operations/severity-alert-delivery.md  
- package.json  
- Cargo.toml  
- git --no-pager log --oneline -5  
- git --no-pager log -5 --name-only --pretty=format:'--- %h %s'  
- source $HOME/.cargo/env && cargo search axum --limit 1  
- source $HOME/.cargo/env && cargo search sqlx --limit 1  
- source $HOME/.cargo/env && cargo search tokio --limit 1  
- source $HOME/.cargo/env && cargo search time --limit 1  
- source $HOME/.cargo/env && cargo search polymarket-client-sdk --limit 1

## Story Completion Status

- Story implementation completed for FR40 regime-shift detection, dispatch integration, persistence, runbook, and QA surfaces.
- Story status is `done` after adversarial review findings were triaged and patchable HIGH/MEDIUM issues were auto-fixed.

## Dev Agent Record

### Agent Model Used

GPT-5.3-Codex (gpt-5.3-codex)

### Debug Log References

- BMAD create-story workflow execution (automated, non-interactive)
- Sprint backlog discovery from `_bmad-output/implementation-artifacts/sprint-status.yaml`
- Artifact discovery across planning, implementation, and code seams
- Recent commit-pattern review via `git --no-pager log --oneline -5`
- Latest crate checks via `cargo search`

### Completion Notes List

- Implemented FR40 domain contracts and deterministic detection logic in `crates/domain/src/risk.rs`, including strict `>` boundary handling for rebate/spread and eligibility transition detection.
- Extended alert reason taxonomy in `crates/domain/src/alerts.rs` with FR40-specific dispatch reason codes.
- Added forward-only `regime_shift_alerts` migration and Postgres adapter with deterministic query ordering and machine-readable persistence error mapping.
- Integrated runtime snapshot comparison in `services/risk-engine/src/gates/mod.rs` and bootstrap eligibility support in `services/risk-engine/src/main.rs`.
- Completed control-plane query/dispatch integration in `services/control-api/src/routes/mod.rs` using existing incident-alert dedupe/fallback dispatch seams and privileged audit patterns.
- During adversarial code review, hardened FR40 flows with transaction-safe persistence, explicit persistence dependency fail-closed behavior, chronology validation for detection/persistence timestamps, and scoped dispatch correlation IDs.
- Added Story 5.2 QA wiring and static API/E2E contract suites (`tests/api/story-5-2-*.test.mjs`, `tests/e2e/story-5-2-*.test.mjs`) with `qa:test:story-5-2`.
- Refreshed Story 5.2 API/E2E QA assertions for operator-actionability payload/evidence fields (`recommended_next_action`, `evidence_link`) and re-ran `qa:test:story-5-2` with all suites passing.
- Published FR40 runbook (`docs/operations/incentive-regime-shift-alerts.md`) and cross-linked operations docs for reward-risk and severity-alert continuity.
- Existing workspace drift note: `services/reporting-service/src/exports/workflows.rs` was already modified before this story completion and was intentionally not altered by Story 5.2 work.
- Git reality discrepancy note: `.scripts/bmad-auto/copilot/bmad-progress.log` is an automation progress artifact outside Story 5.2 application source scope.
- Repository lint note: `npm run --silent rust:lint` currently fails on pre-existing Clippy findings in `crates/domain/src/recovery.rs` (`collapsible_if`), unrelated to Story 5.2 changes.
- Cross-suite verification note: updated `tests/api/story-4-4-report-export-workflows-api.test.mjs` regex assertions for rustfmt-stable match behavior in `report_export_service_error_status`.

### File List

- crates/domain/src/alerts.rs
- crates/domain/src/risk.rs
- crates/persistence/migrations/20260407103000_regime_shift_alerts.sql
- crates/persistence/src/postgres/mod.rs
- crates/persistence/src/postgres/regime_shift_alerts.rs
- services/risk-engine/src/gates/mod.rs
- services/risk-engine/src/main.rs
- services/control-api/src/routes/mod.rs
- package.json
- tests/api/story-4-4-report-export-workflows-api.test.mjs
- tests/api/story-5-2-regime-shift-alerts-api.test.mjs
- tests/e2e/story-5-2-regime-shift-alerts-gating.e2e.test.mjs
- docs/operations/incentive-regime-shift-alerts.md
- docs/operations/reward-risk-policy-operations.md
- docs/operations/severity-alert-delivery.md
- _bmad-output/implementation-artifacts/tests/test-summary.md
- _bmad-output/implementation-artifacts/sprint-status.yaml
- _bmad-output/implementation-artifacts/stories/5-2-add-incentive-and-regime-shift-detection-alerts.md

### Change Log

- 2026-04-07: Completed Story 5.2 FR40 regime-shift detection/dispatch implementation, added persistence + runbook + QA automation, executed story QA and full Rust regression, and advanced status to `review`.
- 2026-04-07: Adversarial review auto-fixes applied: transaction-safe FR40 persistence writes, fail-closed regime-shift persistence dependency behavior, chronology validation hardening, dedupe correlation scoping, and static API test updates; status advanced to `done`.
- 2026-04-07: QA automation refresh added Story 5.2 API/E2E assertions for actionability payload/evidence fields and re-ran `qa:test:story-5-2`; story status remains `done`.
