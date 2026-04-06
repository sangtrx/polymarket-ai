# Story 4.3: Implement Recurring Summary Report Scheduling

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an operator,  
I want daily/weekly/monthly report schedules,  
so that strategy and risk oversight happens automatically.

## Acceptance Criteria

1. **Scenario A - recurring summary scheduling baseline (story-local BDD):**  
   **Given** report cadence configuration is active  
   **When** scheduled execution windows arrive  
   **Then** reports are generated at required daily/weekly/monthly cadence  
   **And** delivery history is auditable per FR38.
2. **Scenario B - FR38 UTC cadence boundaries:**  
   **Given** a schedule is configured for one cadence  
   **When** next-run time is computed  
   **Then** cadence boundaries are deterministic and UTC-only:
   - daily: `00:00 UTC`,
   - weekly: `Monday 00:00 UTC`,
   - monthly: `day 1, 00:00 UTC`  
   **And** no local-time or DST-adjusted behavior is applied.
3. **Scenario C - auditable report-run history:**  
   **Given** scheduled generation is attempted  
   **When** a run succeeds, fails, or is marked missed  
   **Then** each attempt is persisted with machine-readable status and timestamps in `report_runs`  
   **And** schedule-level traceability in `report_schedules` supports deterministic operational/audit lookup.
4. **Scenario D - controlled schedule mutation authorization:**  
   **Given** an actor attempts to create, pause, resume, or update a schedule  
   **When** RBAC policy is evaluated  
   **Then** only roles with control-plane mutation permission are allowed  
   **And** denied attempts return explicit machine-readable authorization failures.
5. **Scenario E - critical schedule failure escalation (NFR15):**  
   **Given** schedule execution cannot complete due to dependency-unavailable, stale-evidence, or persistence failure conditions  
   **When** failure classification is critical  
   **Then** alert evidence is emitted within 30 seconds containing cause, impacted system, and runbook reference  
   **And** the failed run remains queryable in `report_runs` with no success-shaped fallback.
6. **UAC-1 Failure handling:** Invalid cadence payloads, malformed UTC timestamps, unauthorized mutation attempts, unavailable reporting dependencies, and persistence failures return explicit machine-readable errors with no unsafe side effects.
7. **UAC-2 Boundary behavior:** Boundary behavior is deterministic and test-covered for daily/weekly/monthly window rollover, exact-threshold execution timestamps (`00:00 UTC` edges), schedule pause/resume transitions, and at-most-one run record per schedule-window key.
8. **UAC-3 Verifiable evidence:** Successful, failed, and missed runs emit timestamped traceability evidence including schedule id, cadence, window bounds, reason code, correlation id, actor/source context, and run status transitions.
9. **Schema/dependency/traceability contract:** Story depends only on `4.1`; schema scope introduces only `report_schedules` and `report_runs`; traceability maps explicitly to `FR38` and `NFR15`.
10. **Scope boundary contract:** Story 4.3 delivers recurring scheduling and run-history generation only; export package assembly and incident/on-demand export orchestration remain Story 4.4 scope.

## Tasks / Subtasks

- [x] **Task 1: Define canonical reporting-schedule domain contracts and cadence math** (AC: 1, 2, 4, 6, 7, 8, 9, 10)
  - [x] Add/extend domain scheduling types under `crates/domain/src/` (for example `reporting_schedule.rs` and `lib.rs` wiring) for:
    - [x] cadence enum (`daily`, `weekly`, `monthly`),
    - [x] schedule lifecycle states (`active`, `paused`, `disabled`),
    - [x] run states (`pending`, `running`, `succeeded`, `failed`, `missed`),
    - [x] machine-readable schedule reason-code taxonomy aligned with existing `reporting_*` conventions.
  - [x] Implement deterministic UTC window helpers for FR38 boundaries (daily/weekly/monthly) and next-run calculation.
  - [x] Reuse canonical role/permission primitives from `domain::governance` for schedule mutation authorization checks (no ad hoc role strings).
  - [x] Add domain tests for edge boundaries: `23:59:59Z -> 00:00:00Z`, week rollover to Monday, month rollover (including leap-year February).

- [x] **Task 2: Add forward-only persistence migration for `report_schedules` and `report_runs`** (AC: 1, 2, 3, 6, 7, 8, 9, 10)
  - [x] Add migration under `crates/persistence/migrations/` that introduces only `report_schedules` and `report_runs`.
  - [x] Add strict check constraints for cadence/status enums, UTC timestamp fields, and non-empty canonical identifiers.
  - [x] Add uniqueness + indexing contracts for:
    - [x] schedule lookup by cadence/status/next_run_at_utc,
    - [x] run lookup by schedule id and execution window,
    - [x] deterministic audit retrieval by correlation/time.
  - [x] Enforce at-most-one run per schedule-window key to prevent duplicate generation.
  - [x] Add migration contract tests asserting scope boundaries and required index/check names.

- [x] **Task 3: Implement persistence adapters for schedule lifecycle and run history** (AC: 1, 2, 3, 6, 7, 8, 9)
  - [x] Add `crates/persistence/src/postgres/report_schedules.rs` and export via `crates/persistence/src/postgres/mod.rs`.
  - [x] Implement typed persistence functions for schedule create/update/pause/resume, due-schedule query, run insert/update, and run-history retrieval.
  - [x] Preserve explicit machine-readable persistence failures (constraint, decode, query, unavailable) with no silent fallback.
  - [x] Add adapter tests for deterministic ordering, boundary validation, idempotent run-key behavior, and invalid payload rejection.

- [x] **Task 4: Implement reporting-service scheduling orchestration in exports boundary** (AC: 1, 2, 3, 5, 6, 7, 8, 10)
  - [x] Replace `services/reporting-service/src/exports/mod.rs` placeholder with scheduling orchestration module(s) (for example `exports/scheduling.rs`).
  - [x] Wire scheduler bootstrap in `services/reporting-service/src/main.rs` with deterministic tick/dispatch loop and graceful degraded mode when persistence is unavailable.
  - [x] Reuse Story 4.1 read-model orchestrator outputs for summary inputs; do not introduce parallel reporting truth models.
  - [x] Persist run lifecycle transitions (`pending -> running -> succeeded|failed|missed`) with reason codes and UTC timestamps.
  - [x] Ensure scheduler execution path is idempotent for each schedule-window key and resilient to restart/replay.

- [x] **Task 5: Add control-plane schedule management APIs and bounded run-history reads** (AC: 1, 3, 4, 6, 7, 8, 10)
  - [x] Add authenticated schedule management routes in `services/control-api/src/routes/` for create/update/pause/resume and run-history retrieval, following canonical envelope and naming conventions.
  - [x] Enforce governance-grade authorization through existing control-api auth/middleware primitives (`execute_control_action`) with explicit machine-readable denial contracts.
  - [x] Keep `services/reporting-service/src/api.rs` focused on read-only external reporting contracts from Story 4.2; do not add control-plane mutation endpoints there.
  - [x] Wire control-api route handlers to typed scheduling/report-history ports backed by reporting-service orchestration + persistence adapters.
  - [x] Emit route-level telemetry for schedule operations (`actor`, `role`, `schedule_id`, `cadence`, `reason_code`, `correlation_id`, `timestamp_utc`).

- [x] **Task 6: Integrate NFR15-critical failure alert evidence for scheduling incidents** (AC: 5, 6, 8, 9)
  - [x] Reuse existing incident-alert/runbook patterns (Story 3.6) to emit critical schedule-failure alerts within 30 seconds.
  - [x] Ensure alert payload includes cause, impacted systems (`reporting-service scheduler`), and runbook link to scheduling operations documentation.
  - [x] Persist and surface correlation linkage between alert evidence and `report_runs` failure records.
  - [x] Fail closed if alert evidence cannot be emitted deterministically (no success-shaped schedule completion).

- [x] **Task 7: Add Story 4.3 QA automation and operations handoff docs** (AC: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10)
  - [x] Add `qa:test:story-4-3` in root `package.json` using established QA command patterns.
  - [x] Add Rust tests for:
    - [x] domain cadence/window boundary logic and authorization gate behavior,
    - [x] persistence migration/adapter constraints for `report_schedules` and `report_runs`,
    - [x] reporting-service scheduler orchestration success/failure/missed paths and API route contracts.
  - [x] Add story/API/contract tests under `tests/` for schedule route behavior, machine-readable failures, and run-history determinism.
  - [x] Add/update operations runbook (e.g., `docs/operations/recurring-report-scheduling.md`) with cadence policy, pause/resume workflow, missed-run handling, and incident escalation path.
  - [x] Update `_bmad-output/implementation-artifacts/tests/test-summary.md` with Story 4.3 evidence after implementation.

### Review Findings

- [x] [Review][Patch] Added recurring scheduler tick loop with configurable interval bootstrap in reporting-service runtime [services/reporting-service/src/main.rs]
- [x] [Review][Patch] Replayed non-terminal schedule-window runs to avoid deadlock and enforce restart-safe progression [services/reporting-service/src/exports/scheduling.rs]
- [x] [Review][Patch] Enforced `paused -> active` resume transition and explicit bounded history-limit validation [services/reporting-service/src/exports/scheduling.rs]
- [x] [Review][Patch] Enforced fail-closed critical alert retry behavior with explicit 30-second NFR15 SLA validation before schedule advancement [services/reporting-service/src/exports/scheduling.rs]
- [x] [Review][Patch] Switched schedule mutation timestamps to server-authoritative values while validating optional client UTC timestamp payloads [services/control-api/src/routes/mod.rs]
- [x] [Review][Patch] Preserved immutable `created_at_utc` on `report_schedules` upsert conflicts [crates/persistence/src/postgres/report_schedules.rs]
- [x] [Review][Patch] Added actor/source context evidence to run-history responses and emitted concrete schedule/cadence telemetry context [services/control-api/src/routes/mod.rs]
- [x] [Review][Patch] Harmonized identifier-boundary handling by allowing 200-char window keys and using bounded deterministic run-id fingerprints [crates/domain/src/reporting_schedule.rs; services/reporting-service/src/exports/scheduling.rs]
- [x] [Review][Patch] Removed session-timezone-dependent UTC check constraints from Story 4.3 migration contract [crates/persistence/migrations/20260407050000_report_schedules_report_runs.sql]
- [x] [Review][Defer] Add a dedicated due-loader index on `(status, next_run_at_utc, schedule_id)` for large dataset optimization — deferred as non-blocking performance follow-up.
- [x] [Review][Defer] `.scripts/bmad-auto/copilot/bmad-progress.log` appeared in git reality but is outside Story 4.3 application-source review scope.

## Dev Notes

### Technical Requirements

- Story objective is FR38 delivery through recurring schedule management and auditable run history for strategy/risk summary oversight.
- Required cadence boundaries are fixed and UTC-only:
  - daily `00:00 UTC`,
  - weekly `Monday 00:00 UTC`,
  - monthly `day 1, 00:00 UTC`.
- Traceability contract is fixed by epic standards: dependencies `4.1`, schema scope `report_schedules` + `report_runs`, mapping `FR38` + `NFR15`.
- Story 4.1 read-model contracts must be reused for summary generation inputs; do not duplicate normalized dataset logic.
- Story 4.2 contract/version metadata and envelope patterns should remain stable; Story 4.3 must not break existing versioned read-only contract behavior.
- NFR15 integration is mandatory for critical scheduling failures (alert within 30 seconds with cause, impacted systems, runbook link).
- **Out of scope:** Story 4.4 export package assembly, incident/on-demand export orchestration, and non-reporting control-plane feature expansion.

[Source: _bmad-output/planning-artifacts/epics.md#Story 4.3: Implement Recurring Summary Report Scheduling]  
[Source: _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)]  
[Source: _bmad-output/planning-artifacts/prd.md#External Interfaces & Reporting]  
[Source: _bmad-output/planning-artifacts/prd.md#Observability & Operability]  
[Source: docs/operations/normalized-reporting-read-models.md#Explicit Story 4.2 non-goals]  
[Source: _bmad-output/implementation-artifacts/stories/4-2-build-versioned-read-only-api-contracts.md#Scope boundary contract]

### Architecture Compliance

- Keep implementation centered in reporting bounded context:
  - runtime orchestration under `services/reporting-service/src/exports/`,
  - schedule/run persistence in `crates/persistence`,
  - domain contracts in `crates/domain`.
- Keep API-boundary discipline:
  - control-plane schedule mutation commands are exposed via `services/control-api`,
  - `services/reporting-service` remains read-only for external reporting contracts.
- Follow architecture format rules:
  - resource paths plural kebab-case,
  - query params snake_case,
  - ISO-8601 UTC timestamps only,
  - canonical `data/meta/error` envelopes.
- Preserve data-boundary discipline: reporting schedules and runs are operational projections/evidence, not execution truth ledgers.
- Reuse governance role/permission matrix for mutation authorization; do not invent alternate role semantics.
- Preserve fail-closed behavior: unavailable dependencies, stale evidence, or uncertain scheduler state must not produce success-like outcomes.

[Source: _bmad-output/planning-artifacts/architecture.md#Pattern Categories Defined]  
[Source: _bmad-output/planning-artifacts/architecture.md#Format Patterns]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: _bmad-output/planning-artifacts/architecture.md#Architectural Boundaries]  
[Source: _bmad-output/planning-artifacts/architecture.md#Data Boundaries]  
[Source: docs/governance/rbac-role-model.md#Canonical roles]  
[Source: crates/domain/src/governance.rs]

### Library & Framework Requirements

- Keep workspace-pinned stack unless story acceptance criteria explicitly require change:
  - `axum = 0.8.8`
  - `sqlx = 0.8.6`
  - `tokio = 1.48.0`
  - `time = 0.3.44`
  - `polymarket-client-sdk = 0.4.4`
- Latest checks at story creation:
  - `axum 0.8.8` (matches workspace),
  - `sqlx 0.9.0-alpha.1` latest line (workspace remains stable `0.8.6`),
  - `tokio 1.51.0`,
  - `time 0.3.47`,
  - `polymarket-client-sdk 0.4.4` (matches workspace),
  - scheduler helpers available: `tokio-cron-scheduler 0.15.1`, `cron 0.16.0`.
- Prefer deterministic UTC schedule-window computation first; avoid opportunistic dependency upgrades or scheduler-library churn without explicit story need.

[Source: Cargo.toml]  
[Source: services/reporting-service/Cargo.toml]  
[Source: source $HOME/.cargo/env && cargo search axum --limit 1]  
[Source: source $HOME/.cargo/env && cargo search sqlx --limit 1]  
[Source: source $HOME/.cargo/env && cargo search tokio --limit 1]  
[Source: source $HOME/.cargo/env && cargo search time --limit 1]  
[Source: source $HOME/.cargo/env && cargo search polymarket-client-sdk --limit 1]  
[Source: source $HOME/.cargo/env && cargo search tokio-cron-scheduler --limit 1]  
[Source: source $HOME/.cargo/env && cargo search cron --limit 1]

### File Structure Requirements

- Primary expected implementation surfaces for Story 4.3:
  - `crates/domain/src/{lib.rs,reporting_schedule.rs}` (new/extended scheduling contracts)
  - `crates/persistence/migrations/*report_schedules*report_runs*.sql`
  - `crates/persistence/src/postgres/{mod.rs,report_schedules.rs}`
  - `services/reporting-service/src/exports/{mod.rs,scheduling.rs}`
  - `services/reporting-service/src/{main.rs,lib.rs}`
  - `services/control-api/src/{main.rs,routes/*,handlers/*,middleware/*}` (schedule management ingress)
  - `package.json` (`qa:test:story-4-3`)
  - `tests/contract/*story-4-3*` and/or `tests/api/*story-4-3*`
  - `docs/operations/recurring-report-scheduling.md`
  - `_bmad-output/implementation-artifacts/tests/test-summary.md`
- Keep Epic 4 boundaries explicit:
  - Story 4.1: normalized read models,
  - Story 4.2: versioned read-only API contracts,
  - Story 4.3: recurring summary schedule orchestration,
  - Story 4.4: weekly/on-demand/incident export workflows.

[Source: _bmad-output/planning-artifacts/architecture.md#Complete Project Directory Structure]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: services/reporting-service/src/{lib.rs,main.rs,exports/mod.rs}]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: crates/persistence/src/postgres/mod.rs]  
[Source: package.json]

### Testing Requirements

- Add deterministic coverage for:
  - cadence boundary semantics (`daily`, `weekly`, `monthly`) with UTC rollover edge cases,
  - schedule lifecycle transitions (`active`, `paused`, `disabled`) and authorization boundaries,
  - due-run idempotency (single run per schedule-window key),
  - scheduler failure classes and machine-readable error mapping,
  - critical alert evidence emission within 30 seconds for NFR15 schedule-failure scenarios,
  - control-api envelope and auth behavior for schedule read/mutate endpoints,
  - reporting-service external contract endpoints remain read-only and backward-compatible,
  - query determinism and bounded retrieval for run history.
- Keep QA layering aligned with repository conventions:
  - Rust domain/persistence/service tests,
  - story-scoped command in root `package.json`,
  - evidence update in `_bmad-output/implementation-artifacts/tests/test-summary.md`.

[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1-6.9)]  
[Source: _bmad-output/planning-artifacts/prd.md#Observability & Operability]  
[Source: _bmad-output/implementation-artifacts/tests/test-summary.md#Story 4.2 QA Automation Refresh]  
[Source: package.json]

### Previous Story Intelligence

- Story 4.1 delivered normalized reporting read models with strict fail-closed/evidence rules; Story 4.3 summary generation should consume those seams directly.
- Story 4.2 established versioned reporting contracts, canonical envelope metadata, and contract-resolution precedence; Story 4.3 must preserve these route/contract guarantees.
- Story 4.2 explicitly deferred recurring scheduling to Story 4.3; this story is the first place to replace `exports` placeholder with concrete scheduler orchestration.
- Story 3.6 alerting established NFR15-consistent alert payload/dispatch expectations and runbook linkage patterns that Story 4.3 should reuse for critical schedule failures.

[Source: _bmad-output/implementation-artifacts/stories/4-1-define-normalized-reporting-read-models.md#Technical Requirements]  
[Source: _bmad-output/implementation-artifacts/stories/4-2-build-versioned-read-only-api-contracts.md#Technical Requirements]  
[Source: docs/operations/normalized-reporting-read-models.md#Explicit Story 4.2 non-goals]  
[Source: _bmad-output/implementation-artifacts/stories/3-6-add-severity-based-alerts-with-recommended-operator-actions.md#NFR15 content contract]  
[Source: docs/operations/severity-alert-delivery.md]

### Git Intelligence Summary

- Recent commit pattern for Epic 4 follows a stable vertical slice:
  1. migration + persistence adapter contracts,
  2. service orchestration and API behavior,
  3. story-scoped QA command/test updates,
  4. operations documentation + evidence updates.
- Story 4.3 should keep that bounded pattern and avoid broad cross-service refactors unrelated to recurring scheduling scope.

[Source: git --no-pager log --oneline -5]  
[Source: git --no-pager show --name-only --pretty='format:%h %s' -5]

### Latest Technical Information

- Workspace and latest crate checks confirm no forced upgrades are required for this story.
- Scheduling crates (`tokio-cron-scheduler`, `cron`) are available if needed, but introducing them is optional and should be justified against deterministic UTC scheduling requirements.

[Source: Cargo.toml]  
[Source: source $HOME/.cargo/env && cargo search tokio-cron-scheduler --limit 1]  
[Source: source $HOME/.cargo/env && cargo search cron --limit 1]

### Project Context Reference

- No repository `project-context.md` artifact was found during discovery.
- Story context is derived from epics, PRD, architecture, UX spec, readiness/validation artifacts, prior stories, current reporting-service seams, and recent git history.

### Project Structure Notes

- `services/reporting-service/src/exports/mod.rs` is currently a Story 4.4 placeholder and has no scheduling implementation yet.
- `services/reporting-service/src/main.rs` currently boots read-model and contract ports; scheduler bootstrap/warmup should be integrated without regressing existing behavior.
- `services/control-api/src/routes/mod.rs` already provides authenticated control-plane route structure that schedule mutation endpoints should follow.
- `services/reporting-service/src/api.rs` should remain focused on read-only external reporting contracts (Story 4.2 compatibility boundary).
- Existing persistence modules include explicit migration-scope tests; Story 4.3 should follow the same migration-contract guardrail style for `report_schedules` and `report_runs`.

[Source: services/reporting-service/src/exports/mod.rs]  
[Source: services/reporting-service/src/main.rs]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: services/reporting-service/src/api.rs]  
[Source: crates/persistence/src/postgres/{reporting_read_models.rs,api_contract_versions.rs}]

### References

- _bmad-output/planning-artifacts/epics.md#Epic 4: Reporting, Exports & External Analytics Integrations  
- _bmad-output/planning-artifacts/epics.md#Story 4.3: Implement Recurring Summary Report Scheduling  
- _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)  
- _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1-6.9)  
- _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)  
- _bmad-output/planning-artifacts/prd.md#External Interfaces & Reporting  
- _bmad-output/planning-artifacts/prd.md#Observability & Operability  
- _bmad-output/planning-artifacts/architecture.md#Pattern Categories Defined  
- _bmad-output/planning-artifacts/architecture.md#Format Patterns  
- _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping  
- _bmad-output/planning-artifacts/architecture.md#Complete Project Directory Structure  
- _bmad-output/planning-artifacts/ux-design-specification.md#Feedback Patterns  
- _bmad-output/planning-artifacts/ux-design-specification.md#Form Patterns  
- _bmad-output/planning-artifacts/ux-design-specification.md#Testing Strategy  
- _bmad-output/planning-artifacts/implementation-readiness-report-2026-04-05.md#Summary and Recommendations  
- _bmad-output/planning-artifacts/prd-validation-report.md#Traceability Validation  
- docs/operations/normalized-reporting-read-models.md  
- docs/operations/severity-alert-delivery.md  
- docs/governance/rbac-role-model.md  
- _bmad-output/implementation-artifacts/stories/4-1-define-normalized-reporting-read-models.md  
- _bmad-output/implementation-artifacts/stories/4-2-build-versioned-read-only-api-contracts.md  
- _bmad-output/implementation-artifacts/stories/3-6-add-severity-based-alerts-with-recommended-operator-actions.md  
- services/reporting-service/src/{main.rs,lib.rs,api.rs,exports/mod.rs,read_models/queries.rs,contracts/mod.rs}  
- services/control-api/src/routes/mod.rs  
- crates/persistence/src/postgres/{mod.rs,reporting_read_models.rs,api_contract_versions.rs,credential_rotation.rs}  
- crates/domain/src/{reporting.rs,governance.rs}  
- Cargo.toml  
- services/reporting-service/Cargo.toml  
- package.json  
- git --no-pager log --oneline -5  
- git --no-pager show --name-only --pretty='format:%h %s' -5  
- source $HOME/.cargo/env && cargo search axum --limit 1  
- source $HOME/.cargo/env && cargo search sqlx --limit 1  
- source $HOME/.cargo/env && cargo search tokio --limit 1  
- source $HOME/.cargo/env && cargo search time --limit 1  
- source $HOME/.cargo/env && cargo search polymarket-client-sdk --limit 1  
- source $HOME/.cargo/env && cargo search tokio-cron-scheduler --limit 1  
- source $HOME/.cargo/env && cargo search cron --limit 1

## Story Completion Status

- Story context generated with exhaustive artifact analysis across epics, PRD, architecture, UX, readiness/validation reports, previous stories, reporting-service seams, and recent git patterns.
- Story status is set to `done`.
- Completion note: Adversarial review completed; all identified HIGH/MEDIUM issues were remediated and validated.

## Dev Agent Record

### Agent Model Used

GPT-5.3-Codex (gpt-5.3-codex)

### Debug Log References

- `source $HOME/.cargo/env && cargo test -p reporting-service --quiet`
- `source $HOME/.cargo/env && cargo test -p control-api report_schedule --quiet`
- `source $HOME/.cargo/env && cargo test -p control-api --quiet`
- `source $HOME/.cargo/env && npm run --silent qa:test:story-4-3`
- `source $HOME/.cargo/env && npm run --silent rust:fmt`
- `source $HOME/.cargo/env && npm run --silent rust:test`

### Completion Notes List

- Implemented Story 4.3 domain contracts and UTC cadence math with deterministic daily/weekly/monthly window boundaries and mutation authorization guards.
- Added forward-only migration plus persistence adapters for `report_schedules` and `report_runs`, including deterministic ordering/index/constraint coverage and idempotent run-window invariants.
- Added reporting-service scheduling orchestration (`exports/scheduling.rs`) with run lifecycle transitions, read-model reuse, replay-safe due-run dispatch, and fail-closed critical alert evidence linkage.
- Added authenticated control-plane schedule management and run-history endpoints in control-api with machine-readable envelopes, authorization/audit integration, and schedule-route telemetry.
- Added Story 4.3 QA command, Rust + contract tests, scheduling operations runbook, and QA evidence refresh in test summary artifacts.
- Completed adversarial review remediation: fixed scheduler replay/loop and alert fail-closed gaps, hardened mutation timestamp integrity, closed run-history evidence/telemetry gaps, and aligned persistence identifier/immutability contracts.
- Expanded Story 4.3 QA automation with dedicated API and E2E story suites, wired them into `qa:test:story-4-3`, and refreshed execution evidence in test summary artifacts.

### File List

- crates/domain/src/lib.rs
- crates/domain/src/recovery_rehearsal.rs
- crates/domain/src/reporting_schedule.rs
- crates/persistence/migrations/20260407050000_report_schedules_report_runs.sql
- crates/persistence/src/postgres/mod.rs
- crates/persistence/src/postgres/report_schedules.rs
- services/governance-service/src/recovery/mod.rs
- services/reporting-service/src/api.rs
- services/reporting-service/src/exports/mod.rs
- services/reporting-service/src/exports/scheduling.rs
- services/reporting-service/src/main.rs
- services/control-api/Cargo.toml
- services/control-api/src/main.rs
- services/control-api/src/middleware/mod.rs
- services/control-api/src/routes/mod.rs
- docs/operations/recurring-report-scheduling.md
- tests/contract/story-4-3-recurring-report-scheduling.test.mjs
- tests/api/story-4-3-recurring-report-scheduling-api.test.mjs
- tests/e2e/story-4-3-recurring-report-scheduling.e2e.test.mjs
- package.json
- Cargo.lock
- _bmad-output/implementation-artifacts/deferred-work.md
- _bmad-output/implementation-artifacts/tests/test-summary.md
- _bmad-output/implementation-artifacts/stories/4-3-implement-recurring-summary-report-scheduling.md
- _bmad-output/implementation-artifacts/sprint-status.yaml

### Change Log

- 2026-04-06: Created Story 4.3 context file and advanced lifecycle from `backlog` to `ready-for-dev`.
- 2026-04-07: Validate-story remediation aligned API-boundary guidance so schedule mutations route through `control-api`, preserving `reporting-service` read-only contract boundaries; validation re-run passed.
- 2026-04-07: Implemented recurring report scheduling domain/persistence/service/control-plane routes, NFR15 alert linkage, Story 4.3 QA automation command, runbook, and contract coverage; advanced story lifecycle to `review`.
- 2026-04-07: Completed adversarial code review remediation; resolved all identified HIGH/MEDIUM findings, refreshed story evidence/tests, and advanced lifecycle from `review` to `done`.
- 2026-04-07: Executed QA automation refresh for Story 4.3 by adding dedicated API and E2E test suites, extending the story QA command to run them, and updating test-summary evidence while keeping lifecycle status `done`.
