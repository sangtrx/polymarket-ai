# Story 4.4: Deliver Weekly, On-Demand, and Incident Export Workflows

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As a governance stakeholder,  
I want exportable evidence packages on demand and during incidents,  
so that audits and postmortems are fast and complete.

## Acceptance Criteria

1. **Scenario A - FR36 export workflow baseline (story-local BDD):**  
   **Given** an export request is initiated by schedule, manual trigger, or incident trigger  
   **When** export generation completes  
   **Then** governance/performance artifacts are packaged and retrievable  
   **And** export coverage satisfies FR36 requirements.
2. **Scenario B - trigger-source and lifecycle determinism:**  
   **Given** an export job is created from `scheduled_weekly`, `on_demand`, or `incident_triggered` source  
   **When** job lifecycle transitions execute  
   **Then** `export_jobs` persists deterministic state transitions (`queued -> running -> succeeded|failed`) with UTC timestamps and machine-readable reason codes  
   **And** weekly scheduled export execution is idempotent per source schedule-window/run linkage from Story 4.3.
3. **Scenario C - required artifact coverage contract:**  
   **Given** an export job reaches `succeeded`  
   **When** `export_artifacts` rows and package manifest are written  
   **Then** package coverage includes promotion decisions, validation evidence, reconciliation summaries, access audits, and incident postmortems  
   **And** each artifact entry contains provenance metadata (`artifact_type`, `source`, `as_of_utc`, `reason_code`, `correlation_id`, checksum, retrievable path/reference).  
4. **Scenario D - unavailable-source handling is explicit and fail-closed:**  
   **Given** a required FR36 artifact source is unavailable, stale, or unresolved  
   **When** export assembly is attempted  
   **Then** the run does not silently omit required categories  
   **And** unavailable categories are recorded explicitly with machine-readable failure metadata, and the export job status is `failed` (never `succeeded`) with auditable outcome evidence.
5. **Scenario E - retrieval integrity and deterministic response envelopes:**  
   **Given** an authorized stakeholder queries export job results  
   **When** artifact metadata or package content is requested  
   **Then** retrieval uses canonical `data/meta/error` envelopes with deterministic ordering/tie-break behavior  
   **And** checksum/path mismatch or missing artifact payloads return explicit machine-readable failures (no success-shaped fallback).
6. **Scenario F - authorization and auditability (NFR17):**  
   **Given** an actor triggers or reads export workflows  
   **When** RBAC policy is evaluated  
   **Then** only control-permitted roles can create/trigger exports while read-analytics roles can retrieve export evidence  
   **And** each mutation/read emits auditable actor/action/parameter/reason/correlation/timestamp evidence queryable within 5 seconds for p95 audit queries.
7. **Scenario G - incident-triggered export escalation linkage:**  
   **Given** a severity incident context exists (operator trigger or incident workflow trigger)  
   **When** incident export generation fails due to dependency, stale evidence, or persistence error  
   **Then** alert-compatible evidence is emitted within 30 seconds with impacted subsystem and runbook link  
   **And** export job/run state remains failed with no implicit success fallback.
8. **UAC-1 Failure handling:** Invalid payloads, unauthorized triggers, missing incident context, unavailable dependencies, stale evidence, checksum mismatches, and persistence failures return explicit machine-readable errors with no unsafe side effects.
9. **UAC-2 Boundary behavior:** Deterministic and test-covered boundary behavior is enforced for weekly boundary scheduling windows, idempotent replay/retry handling, query pagination limits, and canonical identifier/time filters.
10. **UAC-3 Verifiable evidence:** Successful and failed export operations emit timestamped audit/telemetry evidence linking actor, trigger source, schedule/run linkage, artifact categories, reason code, and correlation id.
11. **Schema/dependency/traceability contract:** Story depends only on `4.1` and `4.3`; schema scope introduces only `export_jobs` and `export_artifacts`; traceability maps explicitly to `FR36` and `NFR17`.
12. **Scope boundary contract:** Story 4.4 delivers export workflow orchestration and retrievability only; it must not regress Story 4.2 versioned read-only contract guarantees or Story 4.3 schedule lifecycle guarantees.

## Tasks / Subtasks

- [x] **Task 1: Define canonical export domain contracts and reason-code taxonomy** (AC: 2, 3, 4, 6, 7, 8, 9, 10, 11)
  - [x] Add/extend domain export contracts under `crates/domain/src/` (for example `reporting_export.rs` + `lib.rs` wiring) for:
    - [x] export trigger source enum (`scheduled_weekly`, `on_demand`, `incident_triggered`),
    - [x] export job states (`queued`, `running`, `succeeded`, `failed`),
    - [x] export artifact categories aligned with FR36 coverage set.
  - [x] Define canonical export reason-code taxonomy aligned with existing `reporting_*`/`reporting_schedule_*` style (machine-readable, deterministic parse/format).
  - [x] Add strict canonical identifier and UTC timestamp validation helpers for export job/artifact contracts.
  - [x] Reuse governance permission boundaries for trigger vs read operations (no ad hoc role strings).
  - [x] Add domain tests for state-transition validity, trigger-source parsing, identifier bounds, and UTC-only timestamp enforcement.

- [x] **Task 2: Add forward-only persistence migration and adapters for `export_jobs` and `export_artifacts`** (AC: 2, 3, 4, 5, 8, 9, 10, 11)
  - [x] Add migration under `crates/persistence/migrations/` introducing only:
    - [x] `export_jobs`,
    - [x] `export_artifacts`.
  - [x] Add constraints/indexes for:
    - [x] trigger source/status enums,
    - [x] canonical identifiers and correlation fields,
    - [x] deterministic lookup ordering and bounded pagination,
    - [x] checksum/path integrity fields,
    - [x] idempotency keying for weekly scheduled exports per schedule-window/run.
  - [x] Add persistence module (for example `crates/persistence/src/postgres/export_jobs.rs`) and export wiring in `crates/persistence/src/postgres/mod.rs`.
  - [x] Add persistence tests validating migration scope boundaries (no Story 4.1-4.3 table overlap), deterministic query ordering, and constraint-failure taxonomy.

- [x] **Task 3: Implement reporting-service export orchestration in exports boundary** (AC: 1, 2, 3, 4, 5, 7, 8, 9, 10, 12)
  - [x] Extend `services/reporting-service/src/exports/` with export orchestration modules (for example `exports/workflows.rs`, `exports/artifacts.rs`) while preserving `exports/scheduling.rs` behavior.
  - [x] Implement weekly export dispatch integration from Story 4.3 schedule/run evidence with replay-safe idempotency.
  - [x] Implement on-demand and incident-triggered export orchestration entry points with explicit trigger-source context and actor/correlation propagation.
  - [x] Build deterministic package manifest generation covering FR36 categories and persist each artifact row in `export_artifacts` with provenance/checksum metadata.
  - [x] Enforce fail-closed behavior for missing/stale dependencies and checksum mismatch paths; no synthetic success payloads.
  - [x] Emit export-job telemetry evidence (`job_id`, `trigger_source`, `status`, `reason_code`, `correlation_id`, timestamp) aligned to existing reporting/control telemetry conventions.

- [x] **Task 4: Add control-plane export trigger and retrieval endpoints with RBAC and audit evidence** (AC: 1, 2, 4, 5, 6, 7, 8, 9, 10, 12)
  - [x] Add authenticated control-api routes in `services/control-api/src/routes/mod.rs` for:
    - [x] on-demand export trigger,
    - [x] incident export trigger,
    - [x] export job status lookup,
    - [x] export artifact listing/retrieval metadata.
  - [x] Keep mutation routes gated through `authorize_critical_action` and read routes through analytics/read authorization pathways.
  - [x] Reuse canonical machine-readable error mapping and response envelope conventions used by existing report-schedule and incident routes.
  - [x] Append privileged audit records and route-level telemetry/security-signal evidence for accepted/rejected export operations.
  - [x] Keep reporting-service external API boundary read-only for analytics contracts (Story 4.2 compatibility); do not move control-plane mutations into reporting-service HTTP routes.

- [x] **Task 5: Deliver operations handoff documentation and Story 4.4 QA automation** (AC: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12)
  - [x] Add operations runbook (for example `docs/operations/report-export-workflows.md`) covering:
    - [x] weekly scheduled flow,
    - [x] on-demand trigger flow,
    - [x] incident-triggered flow,
    - [x] retrieval/audit procedure,
    - [x] failure-mode remediation and runbook links.
  - [x] Add `qa:test:story-4-4` script in root `package.json` aligned with repository QA command patterns.
  - [x] Add Rust coverage for domain, persistence, export orchestration, and control-api route error/audit behavior.
  - [x] Add contract/API/E2E story tests under:
    - [x] `tests/contract/story-4-4-*.test.mjs`,
    - [x] `tests/api/story-4-4-*.test.mjs`,
    - [x] `tests/e2e/story-4-4-*.test.mjs`.
  - [x] Update `_bmad-output/implementation-artifacts/tests/test-summary.md` with Story 4.4 evidence after implementation.

## Dev Notes

### Technical Requirements

- Story objective is FR36 delivery through reliable export workflows for governance/performance evidence packages across weekly scheduled, on-demand, and incident-triggered paths.
- Story dependency contract is fixed:
  - depends on Story 4.1 read-model evidence seams,
  - depends on Story 4.3 schedule/run orchestration,
  - must not require forward dependencies on unfinished Epic 5/6 implementations.
- Traceability contract is fixed by epic index:
  - schema scope only `export_jobs` + `export_artifacts`,
  - explicit mapping to `FR36` + `NFR17`.
- Export payloads must preserve deterministic evidence metadata (`as_of_utc`, `source`, `reason_code`, `correlation_id`) and auditable trigger context.
- Required FR36 category coverage must be explicit and machine-auditable; unavailable categories must be represented explicitly with reason codes and force `failed` job status, never silent omission.
- Incident-triggered export failures must be alert-compatible within 30 seconds and remain fail-closed.

[Source: _bmad-output/planning-artifacts/epics.md#Story 4.4: Deliver Weekly, On-Demand, and Incident Export Workflows]  
[Source: _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)]  
[Source: _bmad-output/planning-artifacts/prd.md#External Interfaces & Reporting]  
[Source: _bmad-output/planning-artifacts/prd.md#Compliance & Auditability]  
[Source: _bmad-output/planning-artifacts/implementation-readiness-report-2026-04-05.md#Functional Requirements]

### Architecture Compliance

- Keep bounded ownership clear:
  - `reporting-service` owns export orchestration and artifact packaging logic,
  - `control-api` owns authenticated mutation ingress and privileged audit emission.
- Preserve architecture format conventions:
  - resource paths plural kebab-case,
  - query parameters snake_case,
  - timestamps UTC ISO-8601,
  - canonical `data/meta/error` envelopes.
- Preserve data-boundary discipline:
  - exports are evidence projections composed from existing authoritative stores,
  - no export workflow may mutate execution truth ledgers.
- Preserve safety-first behavior:
  - dependency uncertainty, stale evidence, or integrity mismatch must fail closed with machine-readable errors.
- Preserve Story 4 boundary layering:
  - Story 4.1 = read models,
  - Story 4.2 = versioned read-only APIs,
  - Story 4.3 = recurring scheduling + run history,
  - Story 4.4 = export workflow orchestration.

[Source: _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries]  
[Source: _bmad-output/planning-artifacts/architecture.md#Pattern Categories Defined]  
[Source: _bmad-output/planning-artifacts/architecture.md#Format Patterns]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: _bmad-output/implementation-artifacts/stories/4-3-implement-recurring-summary-report-scheduling.md#Architecture Compliance]

### Library & Framework Requirements

- Keep workspace-pinned stack unless acceptance criteria explicitly require addition/change:
  - `axum = 0.8.8`
  - `sqlx = 0.8.6`
  - `tokio = 1.48.0`
  - `time = 0.3.44`
  - `serde = 1.0.228`
  - `serde_json = 1.0.145`
  - `polymarket-client-sdk = 0.4.4`
- Latest checks at story-creation time indicate:
  - `axum` latest stable aligns with workspace pin (`0.8.8`),
  - `polymarket-client-sdk` latest stable aligns with workspace pin (`0.4.4`),
  - newer `tokio` (`1.51.0`) and `time` (`0.3.47`) are available,
  - `sqlx` newest is pre-release (`0.9.0-alpha.1`).
- If compressed package output is required for export artifacts, evaluate `zip` (`8.5.0`) deliberately and add only with explicit contract/tests; do not perform opportunistic upgrades.

[Source: Cargo.toml]  
[Source: services/reporting-service/Cargo.toml]  
[Source: source $HOME/.cargo/env && cargo search axum --limit 1]  
[Source: source $HOME/.cargo/env && cargo search sqlx --limit 1]  
[Source: source $HOME/.cargo/env && cargo search tokio --limit 1]  
[Source: source $HOME/.cargo/env && cargo search time --limit 1]  
[Source: source $HOME/.cargo/env && cargo search polymarket-client-sdk --limit 1]  
[Source: source $HOME/.cargo/env && cargo search zip --limit 1]

### File Structure Requirements

- Primary expected implementation surfaces for Story 4.4:
  - `crates/domain/src/{lib.rs,reporting_export.rs}` (new/extended export workflow contracts),
  - `crates/persistence/migrations/*export_jobs*export_artifacts*.sql`,
  - `crates/persistence/src/postgres/{mod.rs,export_jobs.rs}`,
  - `services/reporting-service/src/exports/{mod.rs,scheduling.rs,*}` (extend export orchestration without regressing scheduling),
  - `services/reporting-service/src/{main.rs,lib.rs}`,
  - `services/control-api/src/{routes/mod.rs,middleware/mod.rs,main.rs}` (new export trigger/read routes and orchestrator wiring as needed),
  - `docs/operations/report-export-workflows.md`,
  - `tests/contract/story-4-4*.test.mjs`,
  - `tests/api/story-4-4*.test.mjs`,
  - `tests/e2e/story-4-4*.test.mjs`,
  - `package.json` (`qa:test:story-4-4`),
  - `_bmad-output/implementation-artifacts/tests/test-summary.md`.
- Keep 4.2 reporting API contract routes read-only and compatible while adding export workflow capabilities.

[Source: _bmad-output/planning-artifacts/architecture.md#Complete Project Directory Structure]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: services/reporting-service/src/{lib.rs,main.rs,exports/mod.rs,exports/scheduling.rs,api.rs}]  
[Source: services/control-api/src/{middleware/mod.rs,routes/mod.rs,main.rs}]  
[Source: crates/persistence/src/postgres/{mod.rs,report_schedules.rs}]  
[Source: docs/operations/recurring-report-scheduling.md]

### Testing Requirements

- Add deterministic coverage for:
  - export-job state transitions and idempotency across weekly replay and retries,
  - on-demand and incident trigger authorization boundaries and failure modes,
  - FR36 category coverage manifest completeness and explicit unavailable-category signaling,
  - checksum/path integrity verification and fail-closed retrieval behavior,
  - NFR17 audit evidence completeness and p95-query compatibility assumptions,
  - alert-compatible incident export failure signaling within 30 seconds.
- Keep QA layering aligned with established Epic 4 patterns:
  - Rust tests for domain/persistence/reporting-service/control-api seams,
  - story-scoped command in root `package.json`,
  - contract/API/E2E tests in top-level `tests/`,
  - test evidence update in `_bmad-output/implementation-artifacts/tests/test-summary.md`.

[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: _bmad-output/planning-artifacts/prd.md#External Interfaces & Reporting]  
[Source: _bmad-output/planning-artifacts/prd.md#Compliance & Auditability]  
[Source: _bmad-output/implementation-artifacts/tests/test-summary.md#Story 4.3 QA Automation Refresh]  
[Source: tests/contract/story-4-3-recurring-report-scheduling.test.mjs]  
[Source: tests/api/story-4-3-recurring-report-scheduling-api.test.mjs]  
[Source: tests/e2e/story-4-3-recurring-report-scheduling.e2e.test.mjs]

### Previous Story Intelligence

- Story 4.3 already delivers deterministic schedule/run orchestration, including replay-safe due-run handling and fail-closed alert SLA logic; Story 4.4 should extend exports without weakening those guarantees.
- Story 4.3 established control-plane schedule mutation/run-history route and telemetry/audit patterns in `control-api`; Story 4.4 should mirror these patterns for export trigger/retrieval endpoints.
- Story 4.2 established read-only reporting contract/version and artifact integrity checks; Story 4.4 must not regress read-only contract behavior in `services/reporting-service/src/api.rs`.
- Story 4.1 and 4.2 enforce evidence-metadata discipline and deterministic read semantics; export packaging should reuse those seams rather than creating parallel truth paths.
- Existing deferred item from Story 4.3 (`(status, next_run_at_utc, schedule_id)` due-loader index) remains non-blocking and should be evaluated only if export workload profiling requires it.

[Source: _bmad-output/implementation-artifacts/stories/4-3-implement-recurring-summary-report-scheduling.md#Technical Requirements]  
[Source: _bmad-output/implementation-artifacts/stories/4-3-implement-recurring-summary-report-scheduling.md#Testing Requirements]  
[Source: _bmad-output/implementation-artifacts/stories/4-2-build-versioned-read-only-api-contracts.md#Technical Requirements]  
[Source: _bmad-output/implementation-artifacts/stories/4-1-define-normalized-reporting-read-models.md#Technical Requirements]  
[Source: _bmad-output/implementation-artifacts/deferred-work.md#Deferred from: code review of 4-3-implement-recurring-summary-report-scheduling (2026-04-06T22:38:02Z)]

### Git Intelligence Summary

- Recent Epic 4 commits show a stable vertical-slice pattern:
  1. domain + migration + persistence adapter contracts,
  2. service orchestration and API/control route behavior,
  3. story-scoped QA command/test wiring,
  4. operations runbook and test-summary evidence updates.
- Story 4.4 should follow the same bounded pattern and avoid broad cross-service refactors outside reporting/control/persistence seams required by FR36.

[Source: git --no-pager log --oneline -5]  
[Source: git --no-pager show --name-only --pretty='format:%h %s' -5]

### Latest Technical Information

- Workspace dependencies remain coherent for Story 4.4 scope; no forced upgrades are required.
- Keep pinned versions for compatibility and scope control; prefer implementation with existing stack unless a new export-packaging crate is explicitly justified and covered by tests.
- Latest crate checks are captured in this story for deterministic dependency decision-making at implementation time.

[Source: Cargo.toml]  
[Source: services/reporting-service/Cargo.toml]  
[Source: source $HOME/.cargo/env && cargo search axum --limit 1]  
[Source: source $HOME/.cargo/env && cargo search sqlx --limit 1]  
[Source: source $HOME/.cargo/env && cargo search tokio --limit 1]  
[Source: source $HOME/.cargo/env && cargo search time --limit 1]  
[Source: source $HOME/.cargo/env && cargo search polymarket-client-sdk --limit 1]  
[Source: source $HOME/.cargo/env && cargo search zip --limit 1]

### Project Context Reference

- No repository `project-context.md` artifact was found during discovery.
- Story context is derived from epics, PRD, architecture, UX spec, readiness/validation artifacts, prior Story 4 implementation context, codebase seams, and recent git history.

### Project Structure Notes

- `services/reporting-service/src/exports/mod.rs` currently exports `scheduling`; Story 4.4 should expand this boundary for export-job/artifact orchestration.
- `services/reporting-service/src/exports/scheduling.rs` already has deterministic run lifecycle, SLA alert handling, and schedule telemetry patterns to reuse.
- `services/reporting-service/src/main.rs` already boots read-model/contract ports and scheduler ticks; export orchestration should integrate without regressing existing startup behavior.
- `services/control-api/src/routes/mod.rs` already contains incident and report-schedule route groups with authenticated route-layer patterns suitable for export trigger/read routes.
- `crates/persistence/src/postgres/mod.rs` currently exports `report_schedules` and `api_contract_versions`; no `export_jobs` adapter exists yet.
- Current migrations include Story 4.1-4.3 schema (`reporting_read_models`, `api_contract_versions`, `report_schedules`, `report_runs`) and explicitly do not yet include `export_jobs`/`export_artifacts`.

[Source: services/reporting-service/src/exports/mod.rs]  
[Source: services/reporting-service/src/exports/scheduling.rs]  
[Source: services/reporting-service/src/main.rs]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: crates/persistence/src/postgres/mod.rs]  
[Source: crates/persistence/migrations/20260407022400_reporting_read_models.sql]  
[Source: crates/persistence/migrations/20260407033000_api_contract_versions.sql]  
[Source: crates/persistence/migrations/20260407050000_report_schedules_report_runs.sql]

### References

- _bmad-output/planning-artifacts/epics.md#Epic 4: Reporting, Exports & External Analytics Integrations  
- _bmad-output/planning-artifacts/epics.md#Story 4.4: Deliver Weekly, On-Demand, and Incident Export Workflows  
- _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)  
- _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)  
- _bmad-output/planning-artifacts/prd.md#Journey 5 — API/Integration User: Lina, External Analytics Consumer  
- _bmad-output/planning-artifacts/prd.md#External Interfaces & Reporting  
- _bmad-output/planning-artifacts/prd.md#Compliance & Auditability  
- _bmad-output/planning-artifacts/architecture.md#API & Communication Patterns  
- _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries  
- _bmad-output/planning-artifacts/architecture.md#Format Patterns  
- _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping  
- _bmad-output/planning-artifacts/ux-design-specification.md#Journey 2 — Incident Safe-State Workflow  
- _bmad-output/planning-artifacts/ux-design-specification.md#Feedback Patterns  
- _bmad-output/planning-artifacts/implementation-readiness-report-2026-04-05.md  
- _bmad-output/planning-artifacts/prd-validation-report.md#Traceability Validation  
- _bmad-output/implementation-artifacts/stories/4-1-define-normalized-reporting-read-models.md  
- _bmad-output/implementation-artifacts/stories/4-2-build-versioned-read-only-api-contracts.md  
- _bmad-output/implementation-artifacts/stories/4-3-implement-recurring-summary-report-scheduling.md  
- _bmad-output/implementation-artifacts/deferred-work.md  
- _bmad-output/implementation-artifacts/tests/test-summary.md#Story 4.3 QA Automation Refresh  
- docs/operations/normalized-reporting-read-models.md  
- docs/operations/recurring-report-scheduling.md  
- docs/operations/severity-alert-delivery.md  
- docs/governance/rbac-role-model.md  
- services/reporting-service/src/{main.rs,lib.rs,api.rs,exports/mod.rs,exports/scheduling.rs,contracts/mod.rs,read_models/queries.rs}  
- services/control-api/src/{main.rs,middleware/mod.rs,routes/mod.rs}  
- crates/domain/src/{lib.rs,reporting.rs,reporting_schedule.rs}  
- crates/persistence/src/postgres/{mod.rs,report_schedules.rs,api_contract_versions.rs}  
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
- source $HOME/.cargo/env && cargo search zip --limit 1

## Story Completion Status

- Story implementation completed across domain, persistence, reporting-service orchestration, control-api routes, operations runbook, and Story 4.4 QA automation.
- Story status is set to `done`.
- Completion note: FR36 export workflows now support deterministic weekly, on-demand, and incident-triggered paths with fail-closed artifact coverage and auditable control-plane retrieval.

## Dev Agent Record

### Agent Model Used

GPT-5.3-Codex (gpt-5.3-codex)

### Debug Log References

- `source $HOME/.cargo/env && npm run --silent qa:test:story-4-3`
- `source $HOME/.cargo/env && cargo test -p control-api routes::tests:: -- --test-threads=1`
- `source $HOME/.cargo/env && npm run --silent qa:test:story-4-4`
- `source $HOME/.cargo/env && npm run rust:lint`
- `source $HOME/.cargo/env && npm run --silent qa:test:story-4-4`
- `source $HOME/.cargo/env && npm test`
- `source $HOME/.cargo/env && cargo test -p control-api routes::tests::scheduled_rotation_route_denies_not_due_boundary_with_machine_code -- --nocapture`
- `source $HOME/.cargo/env && node --test tests/api/story-4-4-report-export-workflows-api.test.mjs tests/e2e/story-4-4-report-export-workflows.e2e.test.mjs`
- `source $HOME/.cargo/env && npm run --silent qa:test:story-4-4`

### Completion Notes List

- Added `reporting_export` domain contracts with canonical trigger/state/artifact enums, reason-code taxonomy, UTC validation, authorization boundaries, and fail-closed lifecycle transition tests.
- Added forward-only migration and postgres adapter for `export_jobs` and `export_artifacts`, including deterministic indexes, weekly idempotency, integrity constraints, and adapter-level validation tests.
- Implemented reporting-service export orchestration (`exports/workflows.rs`, `exports/artifacts.rs`) and integrated weekly scheduler dispatch linkage from Story 4.3 run evidence.
- Added control-api report export endpoints for mutation and retrieval with canonical `data/meta/error` envelopes, machine-readable error/status mapping, audit evidence, telemetry, and route tests.
- Added Story 4.4 runbook plus contract/API/E2E tests and root QA script coverage; executed story QA and full repository test suite.
- Code-review auto-fix pass addressed HIGH/MEDIUM findings: immutable field protection in upsert clauses, weekly idempotency collision recovery for distributed race windows, explicit `not_found` handling for missing job artifact listings, trigger response timestamp integrity, query-correlation propagation for read/list responses, and unavailable-artifact contract validation parity.
- Stabilized a time-boundary control-api test by switching scheduled rotation "not due" fixture timestamps to runtime-relative UTC generation to keep full-suite behavior deterministic across calendar rollover.
- File-list cross-check discrepancy: `.scripts/bmad-auto/copilot/bmad-progress.log` is modified in git state but intentionally excluded from story application-code review scope.
- Refreshed Story 4.4 API/E2E QA coverage for critical flows by adding assertions for read correlation-id precedence, incident failure alert evidence SLA linkage, and retrieval fail-closed integrity/missing-job behavior.

### Review Findings (Code Review 2026-04-07)

- [x] [Review][Patch] Prevent immutable export context fields from being overwritten during `ON CONFLICT` updates for `export_jobs` and `export_artifacts` [`crates/persistence/src/postgres/export_jobs.rs`].
- [x] [Review][Patch] Handle weekly idempotency unique-index collisions by loading and returning the existing job evidence instead of surfacing a constraint error [`services/reporting-service/src/exports/workflows.rs`].
- [x] [Review][Patch] Return explicit `not_found` for artifact listing when `job_id` does not exist (no success-shaped empty fallback) [`services/reporting-service/src/exports/workflows.rs`].
- [x] [Review][Patch] Preserve effective correlation id (`query.correlation_id` override or actor default) through export artifact list/read response envelopes and audit context [`services/control-api/src/routes/mod.rs`].
- [x] [Review][Patch] Use workflow-provided trigger lifecycle timestamps in trigger responses instead of synthesized timestamp placeholders [`services/control-api/src/routes/mod.rs`, `services/reporting-service/src/exports/workflows.rs`].
- [x] [Review][Patch] Enforce unavailable-artifact checksum sentinel and retrieval-reference validation to match persistence contract semantics [`crates/domain/src/reporting_export.rs`].

### File List

- _bmad-output/implementation-artifacts/stories/4-4-deliver-weekly-on-demand-and-incident-export-workflows.md
- _bmad-output/implementation-artifacts/sprint-status.yaml
- crates/domain/src/lib.rs
- crates/domain/src/reporting_export.rs
- crates/persistence/migrations/20260407062000_export_jobs_export_artifacts.sql
- crates/persistence/src/postgres/mod.rs
- crates/persistence/src/postgres/export_jobs.rs
- services/reporting-service/src/exports/mod.rs
- services/reporting-service/src/exports/artifacts.rs
- services/reporting-service/src/exports/workflows.rs
- services/reporting-service/src/exports/scheduling.rs
- services/control-api/src/middleware/mod.rs
- services/control-api/src/main.rs
- services/control-api/src/routes/mod.rs
- docs/operations/report-export-workflows.md
- tests/contract/story-4-4-report-export-workflows.test.mjs
- tests/api/story-4-4-report-export-workflows-api.test.mjs
- tests/e2e/story-4-4-report-export-workflows.e2e.test.mjs
- package.json
- _bmad-output/implementation-artifacts/tests/test-summary.md

### Change Log

- 2026-04-07: Created Story 4.4 context file and advanced lifecycle from `backlog` to `ready-for-dev`.
- 2026-04-07: Validate-story remediation clarified fail-closed semantics so missing required FR36 categories must yield explicit `failed` export job outcomes.
- 2026-04-06: Implemented Story 4.4 export workflow vertical slice (domain, persistence, reporting-service, control-api), added runbook + QA automation, and advanced story status to `review`.
- 2026-04-07: Completed adversarial code review with automatic HIGH/MEDIUM fixes, re-ran Story 4.4 QA + full test suite, and advanced story status to `done`.
- 2026-04-07: Expanded Story 4.4 API/E2E automation for incident alert SLA evidence, retrieval fail-closed guarantees, and read-correlation precedence; re-ran `qa:test:story-4-4` with passing results.
