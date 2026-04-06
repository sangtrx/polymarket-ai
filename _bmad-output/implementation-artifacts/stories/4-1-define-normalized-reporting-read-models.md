# Story 4.1: Define Normalized Reporting Read Models

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an analytics consumer,  
I want normalized datasets for trades, positions, risk, and performance,  
so that downstream analysis uses consistent semantics.

## Acceptance Criteria

1. **Scenario A — normalized reporting datasets (story-local BDD):**  
   **Given** execution and risk events are available  
   **When** reporting models are generated  
   **Then** normalized records are exposed for analytics use  
   **And** data model supports FR35 retrieval requirements.
2. **Scenario B — deterministic normalization and semantic consistency (story-local BDD):**  
   **Given** mixed-source records for trades, positions, risk events, and performance signals  
   **When** records are projected into reporting read models  
   **Then** identifiers, timestamps, status fields, and reason codes are normalized deterministically  
   **And** repeated generation on identical source windows yields stable ordering and equivalent output.
3. **Scenario C — degraded dependency failure path (story-local BDD):**  
   **Given** one or more required projection dependencies are unavailable or stale  
   **When** reporting read-model generation or query is requested  
   **Then** processing fails closed with explicit machine-readable reason codes  
   **And** no success-shaped payload is emitted.
4. **UAC-1 Failure handling:** Invalid query filters, malformed identifier/time-window inputs, unauthorized reporting reads, unavailable projection dependencies, and persistence decode/constraint failures return explicit machine-readable errors with no unsafe side effects.
5. **UAC-2 Boundary behavior:** Deterministic and test-covered boundary behavior is enforced for inclusive/exclusive time windows, max result limits, identifier normalization, and stable tie-break ordering in all read-model queries.
6. **UAC-3 Verifiable evidence:** Reporting outputs expose timestamped and correlation-aware evidence fields (`as_of_utc`, `source`, `reason_code`, `correlation_id`, and run/snapshot identifiers where available) suitable for audit and incident traceability.
7. **Schema/dependency/traceability contract:** Story depends only on `2.6`; schema scope is reporting read models/views only; traceability maps explicitly to `FR35`, `NFR13`, and `NFR16`.
8. **NFR13 compatibility contract:** Read-model schemas are explicit and evolution-safe for upcoming versioned API work (Story 4.2), with no breaking shape ambiguity for downstream contracts.
9. **NFR16 retrieval contract:** Read-model query paths for incident/post-incident analysis support deterministic retrieval of relevant records without manual log stitching and target representative `<= 5s` query behavior.
10. **Scope boundary contract:** Story 4.1 delivers normalized read models only; versioned API contracts (Story 4.2), recurring report scheduling (Story 4.3), and export workflows (Story 4.4) are out of scope.

## Tasks / Subtasks

- [x] **Task 1: Define canonical reporting-domain contracts and normalization rules** (AC: 1, 2, 3, 4, 5, 6, 7, 8)
  - [x] Add a reporting-domain module in `crates/domain` (for example `crates/domain/src/reporting.rs`) and export it from `crates/domain/src/lib.rs`.
  - [x] Define typed normalized record contracts for at least:
    - [x] trade reporting rows,
    - [x] position reporting rows,
    - [x] risk-event reporting rows,
    - [x] performance reporting rows.
  - [x] Define machine-readable reporting reason-code taxonomy and validation issues aligned with existing domain conventions.
  - [x] Implement deterministic normalization helpers for canonical identifiers, UTC timestamp parsing/formatting, and stable ordering keys.
  - [x] Add domain tests for normalization determinism, boundary validation, and failure-code mapping.

- [x] **Task 2: Add forward-only persistence migration for reporting read models/views** (AC: 1, 2, 5, 7, 8, 9, 10)
  - [x] Add a migration under `crates/persistence/migrations/` that introduces only Story 4.1 read-model objects (views/materialized views/tables dedicated to normalized reporting reads).
  - [x] Normalize source-to-read mappings using existing persisted surfaces (`orders`, `order_state_transitions`, `user_stream_events`, `reconciliation_runs`, `reconciliation_diffs`, `exposure_snapshots`, `attribution_snapshots`, and incident/risk evidence sources where required).
  - [x] Ensure deterministic ordering keys and query-supporting indexes for reporting retrieval windows and correlation-based lookups.
  - [x] Keep migration scope strict: do **not** introduce Story 4.2+ schema objects (`api_contract_versions`, `report_schedules`, `report_runs`, `export_jobs`, `export_artifacts`).

- [x] **Task 3: Implement reporting read-model persistence adapters** (AC: 1, 2, 3, 4, 5, 6, 9)
  - [x] Add `crates/persistence/src/postgres/reporting_read_models.rs` and wire it via `crates/persistence/src/postgres/mod.rs`.
  - [x] Implement typed query functions for normalized trade/position/risk/performance datasets with strict filter validation and bounded result limits.
  - [x] Return explicit machine-readable persistence errors; do not swallow decode, query, or constraint failures.
  - [x] Add persistence tests validating schema constraints, deterministic ordering, and incident-style retrieval paths.

- [x] **Task 4: Implement reporting-service internal read-model orchestration seam** (AC: 1, 2, 3, 4, 6, 8, 9, 10)
  - [x] Add an internal read-model module (for example `services/reporting-service/src/read_models/{mod.rs,queries.rs}`) with typed query request/response structures for normalized datasets.
  - [x] Update `services/reporting-service/src/main.rs` bootstrap/warm seam to initialize read-model orchestration paths against persistence adapters.
  - [x] Keep this story read-model focused and internal-only; do not introduce versioned external API contract behavior (Story 4.2) or export workflow surfaces (Story 4.4).

- [x] **Task 5: Define analytics-access handoff seam for Story 4.2** (AC: 1, 3, 8, 10)
  - [x] Expose a clear internal query facade in `reporting-service` that future versioned endpoints can consume without reshaping core records.
  - [x] Document explicit non-goals for this story: no endpoint version negotiation, no published schema artifacts/changelogs, no backward-compatibility negotiation logic.
  - [x] Preserve read-only semantics and keep mutation paths out of reporting-service scope.

- [x] **Task 6: Add observability and evidence guarantees for reporting reads** (AC: 3, 4, 6, 9)
  - [x] Ensure all read-model outputs include consistent evidence metadata (`source`, `reason_code`, `correlation_id`, timestamp fields, and upstream run/snapshot references where present).
  - [x] Add telemetry/audit-oriented evidence hooks consistent with existing governance and incident-traceability patterns.
  - [x] Fail closed when required evidence metadata cannot be produced deterministically.

- [x] **Task 7: Add Story 4.1 QA automation and command wiring** (AC: 1, 2, 3, 4, 5, 6, 8, 9)
  - [x] Add `qa:test:story-4-1` to root `package.json` following existing story QA command conventions.
  - [x] Add Rust tests for:
    - [x] `domain` reporting normalization contracts,
    - [x] `persistence` reporting-read-model adapters and ordering/constraint behavior,
    - [x] `reporting-service` read-model orchestration and failure propagation.
  - [x] Include explicit tests for invalid filters, unauthorized/dependency-unavailable semantics (where applicable), and deterministic result ordering.
  - [x] Update `_bmad-output/implementation-artifacts/tests/test-summary.md` with Story 4.1 evidence after implementation.

- [x] **Task 8: Add reporting read-model operations documentation** (AC: 2, 6, 8, 9, 10)
  - [x] Add `docs/operations/normalized-reporting-read-models.md` describing dataset semantics, source mappings, evidence fields, and operational query guidance.
  - [x] Include clear handoff notes for Story 4.2 contract/versioning work.
  - [x] Document scope boundaries to prevent overlap with Story 4.2/4.3/4.4 deliverables.

### Review Findings

- [x] [Review][Patch] Incident risk-event projection now canonicalizes optional identifiers (`market_id`, `run_id`, `snapshot_id`) with `NULLIF(lower(trim(...)), '')` to prevent non-canonical incident payloads from causing avoidable fail-closed row validation errors. [crates/persistence/migrations/20260407022400_reporting_read_models.sql]
- [x] [Review][Patch] Added migration-contract regression assertions ensuring incident optional identifier normalization remains enforced for Story 4.1 read-model risk projections. [crates/persistence/src/postgres/reporting_read_models.rs]

## Dev Notes

### Technical Requirements

- Story objective is FR35 coverage via normalized datasets for:
  - trades,
  - positions,
  - risk events,
  - performance metrics.
- Use existing persisted truth/read surfaces as canonical source inputs:
  - order and lifecycle records (`orders`, `order_state_transitions`, `user_stream_events`),
  - reconciliation/exposure records (`reconciliation_runs`, `reconciliation_diffs`, `exposure_snapshots`),
  - attribution/performance records (`attribution_snapshots`),
  - incident/risk evidence records (`incident_query_views`, `pretrade_gate_decisions`, `freshness_gate_events`, `safety_control_actions`, `recovery_gate_runs`) where needed for normalized risk views.
- Deterministic reporting output requirements:
  - canonical identifiers (normalized, stable),
  - UTC ISO-8601 timestamp fields,
  - explicit `reason_code` and `correlation_id`,
  - stable query ordering with deterministic tie-breaks.
- Constraints:
  - schema scope must remain read-model/view focused for Story 4.1,
  - no versioned API contract semantics in this story (defer to Story 4.2),
  - no scheduling/export orchestration behavior in this story (defer to Stories 4.3/4.4).

[Source: _bmad-output/planning-artifacts/epics.md#Story 4.1: Define Normalized Reporting Read Models]  
[Source: _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)]  
[Source: _bmad-output/planning-artifacts/prd.md#External Interfaces & Reporting]  
[Source: crates/persistence/migrations/20260406050000_order_lifecycle.sql]  
[Source: crates/persistence/migrations/20260406030000_user_stream_ingestion.sql]  
[Source: crates/persistence/migrations/20260406061000_reconciliation_exposure_core.sql]  
[Source: crates/persistence/migrations/20260406154000_attribution_snapshots.sql]  
[Source: crates/persistence/migrations/20260406193000_incident_query_views.sql]  
[Source: crates/persistence/migrations/20260406081500_pretrade_gate_decisions.sql]  
[Source: crates/persistence/migrations/20260406040500_freshness_gate_events.sql]  
[Source: crates/persistence/migrations/20260406100000_safety_control_actions.sql]  
[Source: crates/persistence/migrations/20260406223000_recovery_gate_runs.sql]

### Architecture Compliance

- Maintain architecture boundaries:
  - `reporting-service` owns reporting read-model orchestration and read-only data exposure seams,
  - `control-api` remains the privileged operator control plane (no external reporting contract expansion in this story),
  - `crates/domain` and `crates/persistence` own shared contracts and storage/query adapters.
  - `services/reporting-service/src/contracts` stays reserved for Story 4.2 versioned contract work, and `services/reporting-service/src/exports` stays reserved for Story 4.4 export workflows.
- Preserve data-boundary rules:
  - PostgreSQL persisted tables remain source-of-truth inputs,
  - reporting datasets are projections/read models, not new execution truth ledgers.
- Enforce architecture conventions:
  - plural snake_case DB naming and stable reason-code taxonomies,
  - ISO-8601 UTC timestamp handling,
  - explicit machine-readable error responses with no silent fallback behavior.
- Keep Story 4.1 aligned to FR35/NFR13/NFR16 and prepared for Story 4.2 versioned contract layering.

[Source: _bmad-output/planning-artifacts/architecture.md#Data Architecture]  
[Source: _bmad-output/planning-artifacts/architecture.md#API Boundaries]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules]  
[Source: _bmad-output/planning-artifacts/architecture.md#Data Boundaries]  
[Source: services/reporting-service/src/main.rs]  
[Source: services/reporting-service/src/contracts/mod.rs]  
[Source: services/reporting-service/src/exports/mod.rs]

### Library & Framework Requirements

- Keep workspace-pinned stack for Story 4.1 compatibility:
  - `axum = 0.8.8`
  - `sqlx = 0.8.6`
  - `tokio = 1.48.0`
  - `time = 0.3.44`
  - `serde = 1.0.228`
  - `polymarket-client-sdk = 0.4.4`
- Latest-version checks (via `cargo search`) at story-creation time:
  - `axum`: `0.8.8` (matches workspace)
  - `sqlx`: `0.9.0-alpha.1` (pre-release; stable workspace remains `0.8.6`)
  - `tokio`: `1.51.0` (workspace pinned lower)
  - `time`: `0.3.47` (workspace pinned lower)
  - `polymarket-client-sdk`: `0.4.4` (matches workspace)
- Do not perform opportunistic dependency upgrades in Story 4.1.
- Note: crates.io API fetch via `web_fetch` returned HTTP 403 in this environment; version checks were completed with `cargo search`.

[Source: Cargo.toml]  
[Source: source $HOME/.cargo/env && cargo search axum --limit 1]  
[Source: source $HOME/.cargo/env && cargo search sqlx --limit 1]  
[Source: source $HOME/.cargo/env && cargo search tokio --limit 1]  
[Source: source $HOME/.cargo/env && cargo search time --limit 1]  
[Source: source $HOME/.cargo/env && cargo search polymarket-client-sdk --limit 1]

### File Structure Requirements

- Primary implementation surfaces for Story 4.1:
  - `crates/domain/src/lib.rs`
  - `crates/domain/src/reporting.rs` (new)
  - `crates/persistence/migrations/*reporting_read_models*.sql` (new)
  - `crates/persistence/src/postgres/{mod.rs,reporting_read_models.rs}` (new adapter)
  - `services/reporting-service/src/main.rs`
  - `services/reporting-service/src/read_models/{mod.rs,queries.rs}` (new internal seam)
  - `services/reporting-service/src/contracts/mod.rs` (boundary marker only; no Story 4.1 contract versioning work)
  - `services/reporting-service/src/exports/mod.rs` (boundary marker only; no Story 4.1 export workflow work)
  - `docs/operations/normalized-reporting-read-models.md` (new)
  - `package.json` (add `qa:test:story-4-1`)
  - `_bmad-output/implementation-artifacts/tests/test-summary.md`
- Keep Story 4 boundaries explicit:
  - Story 4.1: normalized read models only,
  - Story 4.2: versioned read-only API contracts,
  - Story 4.3: recurring report scheduling,
  - Story 4.4: scheduled/on-demand/incident exports.

[Source: _bmad-output/planning-artifacts/architecture.md#Complete Project Directory Structure]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: services/reporting-service/src/main.rs]  
[Source: services/reporting-service/src/contracts/mod.rs]  
[Source: services/reporting-service/src/exports/mod.rs]  
[Source: package.json]

### Testing Requirements

- Add deterministic coverage for:
  - normalized field-shape consistency across trade/position/risk/performance datasets,
  - stable ordering and tie-break behavior for identical query windows,
  - boundary conditions (`start_inclusive`, `end_exclusive`, max limits, empty windows),
  - failure paths (invalid input, stale/unavailable dependencies, persistence decode/constraint errors),
  - evidence metadata completeness (`as_of_utc`, `source`, `reason_code`, `correlation_id`, run/snapshot IDs where present),
  - representative NFR16 retrieval timing behavior for incident-oriented reporting queries.
- Keep QA layering consistent with repository conventions:
  - Rust unit/integration tests in `domain`, `persistence`, and `reporting-service`,
  - story-scoped QA command in root `package.json`,
  - test evidence summary update in `_bmad-output/implementation-artifacts/tests/test-summary.md`.

[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: _bmad-output/planning-artifacts/prd.md#External Interfaces & Reporting]  
[Source: _bmad-output/planning-artifacts/prd.md#Integration]  
[Source: _bmad-output/planning-artifacts/prd.md#Observability & Operability]  
[Source: package.json]

### Previous Story Intelligence

- Story 2.6 established canonical reconciliation/exposure persistence and deterministic mismatch semantics; Story 4.1 should reuse these seams for normalized position and reconciliation-linked reporting slices.
- Story 3.4 established cost-aware attribution snapshots and deterministic period boundaries; Story 4.1 should reuse this for normalized performance datasets instead of duplicating attribution logic.
- Story 3.5 established incident query views and NFR16-friendly retrieval patterns; Story 4.1 risk-event reporting should align with those evidence and retrieval semantics.
- Stories 3.6-3.8 expanded risk/incident/recovery evidence surfaces (`incident_alerts`, `recovery_gate_runs`, `restore_rehearsal_runs`) that can contribute normalized risk reporting context where appropriate.
- Story 3.9 established accessibility-focused UI hardening and is not a data-model dependency; keep this story backend read-model scoped.

[Source: _bmad-output/implementation-artifacts/stories/2-6-build-reconciliation-and-exposure-visibility-core.md#Technical Requirements]  
[Source: _bmad-output/implementation-artifacts/stories/3-4-build-cost-aware-pnl-and-attribution-surfaces.md#Technical Requirements]  
[Source: _bmad-output/implementation-artifacts/stories/3-5-implement-incident-search-and-causal-timeline-forensics.md#Technical Requirements]  
[Source: _bmad-output/implementation-artifacts/stories/3-6-add-severity-based-alerts-with-recommended-operator-actions.md#Technical Requirements]  
[Source: _bmad-output/implementation-artifacts/stories/3-7-implement-controlled-recovery-readiness-gates.md#Technical Requirements]  
[Source: _bmad-output/implementation-artifacts/stories/3-8-add-backup-integrity-validation-and-deterministic-restore-rehearsal.md#Technical Requirements]

### Git Intelligence Summary

- Recent commit sequence (`3-5` through `3-9`) follows a stable vertical-slice pattern:
  1. domain + migration + persistence adapter contracts,
  2. service orchestration and strict machine-readable error semantics,
  3. QA command wiring and story-scoped tests,
  4. operations runbook + test-summary evidence updates.
- Changed-file history indicates strong reuse of existing persistence seams instead of introducing parallel truth stores; Story 4.1 should keep that pattern.
- Story 4.1 should avoid broad cross-service refactors and instead add bounded reporting read-model layers that future Story 4.2 APIs can consume.

[Source: git --no-pager log --oneline -5]  
[Source: git --no-pager show --name-only --pretty='format:%h %s' -3]

### Latest Technical Information

- Latest package checks completed via `cargo search` confirm:
  - `axum` latest stable equals workspace pin (`0.8.8`),
  - `polymarket-client-sdk` latest stable equals workspace pin (`0.4.4`),
  - newer `tokio`/`time` releases exist, and `sqlx` has a newer pre-release line.
- Story 4.1 should remain on workspace-pinned versions to avoid scope creep.
- crates.io HTTP API checks via `web_fetch` were blocked (403), so `cargo search` output is the authoritative check used for this story context.

[Source: source $HOME/.cargo/env && cargo search axum --limit 1]  
[Source: source $HOME/.cargo/env && cargo search sqlx --limit 1]  
[Source: source $HOME/.cargo/env && cargo search tokio --limit 1]  
[Source: source $HOME/.cargo/env && cargo search time --limit 1]  
[Source: source $HOME/.cargo/env && cargo search polymarket-client-sdk --limit 1]

### Project Context Reference

- No repository `project-context.md` artifact was found during discovery.
- Story context is derived from epics, PRD, architecture, implementation readiness artifacts, previous story files, current code seams, migration schemas, and git history.

### Project Structure Notes

- `services/reporting-service` currently contains only scaffold seams:
  - `src/main.rs`
  - `src/contracts/mod.rs`
  - `src/exports/mod.rs`
- Existing normalized-like read surfaces already exist in other bounded contexts and should be reused as source seams:
  - attribution (`crates/persistence/src/postgres/attribution_snapshots.rs`),
  - reconciliation/exposure (`crates/persistence/src/postgres/reconciliation.rs`),
  - incident query views (`crates/persistence/migrations/20260406193000_incident_query_views.sql`).
- `services/control-api` already supports operator attribution and incident reads; Story 4.1 should avoid duplicating control-plane responsibilities and instead prepare reporting-service read models for Story 4.2 contract layering.

[Source: services/reporting-service/src/main.rs]  
[Source: services/reporting-service/src/contracts/mod.rs]  
[Source: services/reporting-service/src/exports/mod.rs]  
[Source: crates/persistence/src/postgres/attribution_snapshots.rs]  
[Source: crates/persistence/src/postgres/reconciliation.rs]  
[Source: services/control-api/src/routes/mod.rs]

### References

- _bmad-output/planning-artifacts/epics.md#Epic 4: Reporting, Exports & External Analytics Integrations  
- _bmad-output/planning-artifacts/epics.md#Story 4.1: Define Normalized Reporting Read Models  
- _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)  
- _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)  
- _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)  
- _bmad-output/planning-artifacts/prd.md#External Interfaces & Reporting  
- _bmad-output/planning-artifacts/prd.md#Integration  
- _bmad-output/planning-artifacts/prd.md#Observability & Operability  
- _bmad-output/planning-artifacts/architecture.md#Data Architecture  
- _bmad-output/planning-artifacts/architecture.md#API Boundaries  
- _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping  
- _bmad-output/planning-artifacts/architecture.md#Data Boundaries  
- _bmad-output/planning-artifacts/implementation-readiness-report-2026-04-05.md#Summary and Recommendations  
- _bmad-output/planning-artifacts/prd-validation-report.md#Traceability Validation  
- _bmad-output/implementation-artifacts/stories/2-6-build-reconciliation-and-exposure-visibility-core.md  
- _bmad-output/implementation-artifacts/stories/3-4-build-cost-aware-pnl-and-attribution-surfaces.md  
- _bmad-output/implementation-artifacts/stories/3-5-implement-incident-search-and-causal-timeline-forensics.md  
- _bmad-output/implementation-artifacts/stories/3-6-add-severity-based-alerts-with-recommended-operator-actions.md  
- _bmad-output/implementation-artifacts/stories/3-7-implement-controlled-recovery-readiness-gates.md  
- _bmad-output/implementation-artifacts/stories/3-8-add-backup-integrity-validation-and-deterministic-restore-rehearsal.md  
- services/reporting-service/src/{main.rs,contracts/mod.rs,exports/mod.rs}  
- services/control-api/src/{main.rs,middleware/mod.rs,routes/mod.rs}  
- crates/domain/src/{lib.rs,attribution.rs,reconciliation.rs}  
- crates/persistence/src/postgres/{mod.rs,attribution_snapshots.rs,reconciliation.rs}  
- crates/persistence/migrations/{20260406030000_user_stream_ingestion.sql,20260406050000_order_lifecycle.sql,20260406061000_reconciliation_exposure_core.sql,20260406081500_pretrade_gate_decisions.sql,20260406040500_freshness_gate_events.sql,20260406100000_safety_control_actions.sql,20260406154000_attribution_snapshots.sql,20260406193000_incident_query_views.sql,20260406210000_incident_alerts_delivery_attempts.sql,20260406223000_recovery_gate_runs.sql}  
- Cargo.toml  
- package.json  
- git --no-pager log --oneline -5  
- git --no-pager show --name-only --pretty='format:%h %s' -3  
- source $HOME/.cargo/env && cargo search axum --limit 1  
- source $HOME/.cargo/env && cargo search sqlx --limit 1  
- source $HOME/.cargo/env && cargo search tokio --limit 1  
- source $HOME/.cargo/env && cargo search time --limit 1  
- source $HOME/.cargo/env && cargo search polymarket-client-sdk --limit 1

## Story Completion Status

- Story context generated with exhaustive artifact analysis across epics, PRD, architecture, readiness reports, previous stories, git patterns, and current code/persistence seams.
- Story status is set to `ready-for-dev`.
- Completion note: Ultimate context engine analysis completed - comprehensive developer guide created.

## Dev Agent Record

### Agent Model Used

GPT-5.3-Codex (gpt-5.3-codex)

### Debug Log References

- BMAD create-story workflow execution (automated, non-interactive)
- Sprint backlog discovery from `_bmad-output/implementation-artifacts/sprint-status.yaml`
- Artifact discovery across planning docs, prior stories, migrations, and service source seams
- Git pattern analysis using recent commit/file history
- Crate-version checks via `cargo search` (with crates.io API 403 fallback)
- `cargo fmt -p domain -p persistence -p reporting-service`
- `npm run --silent qa:test:story-4-1`
- `npm test`
- `cargo clippy -p domain -p persistence -p reporting-service --all-targets`
- Code review execution: `git --no-pager status --porcelain`, `git --no-pager diff`, `git --no-pager diff --cached`
- Code review validation rerun: `npm run --silent qa:test:story-4-1`
- Code review validation rerun: `npm test`
- QA automation refresh rerun: `npm run --silent qa:test:story-4-1`

### Completion Notes List

- Selected first backlog story: `4-1-define-normalized-reporting-read-models`.
- Expanded Story 4.1 with explicit BDD, UAC, dependency/schema/traceability contracts, and scope boundaries.
- Mapped normalized reporting datasets to existing persisted source seams and architecture boundaries.
- Added implementation guardrails to avoid overlap with Story 4.2/4.3/4.4.
- Implemented `domain::reporting` with typed trade/position/risk/performance contracts, deterministic normalization helpers, canonical boundary validation, and machine-readable reporting reason codes.
- Added Story 4.1 reporting read-model migration (`reporting_trade_read_models`, `reporting_position_read_models`, `reporting_risk_event_read_models`, `reporting_performance_read_models`) with deterministic retrieval/correlation index coverage.
- Implemented persistence adapters in `crates/persistence/src/postgres/reporting_read_models.rs` with strict query validation, deterministic ordering, and explicit decode/query/constraint failure surfacing.
- Implemented `reporting-service` internal orchestration seam (`read_models::queries`) with role-gated reads, fail-closed dependency handling, evidence metadata enforcement, and Story 4.2 handoff-ready typed response envelopes.
- Added Story 4.1 QA command wiring, refreshed test evidence summary, and published operations runbook (`docs/operations/normalized-reporting-read-models.md`) with explicit non-goals and Story 4.2 handoff boundaries.
- Adversarial code review identified a medium-severity canonicalization gap in incident-backed risk-event projection rows; migration now normalizes optional incident identifiers and adds regression assertions to keep deterministic validation/fail-closed behavior stable.
- File-list cross-check against `git status --porcelain` reconciled the story artifact set with repository reality; the automation log change in `.scripts/bmad-auto/copilot/bmad-progress.log` was treated as out-of-scope runtime metadata and excluded from application review scope.
- QA automation refresh added regression coverage for malformed/non-UTC reporting filters and fail-closed evidence timestamp handling across domain, persistence adapters, and reporting-service read-model orchestration seams.

### File List

- Cargo.lock
- _bmad-output/implementation-artifacts/stories/4-1-define-normalized-reporting-read-models.md
- _bmad-output/implementation-artifacts/sprint-status.yaml
- _bmad-output/implementation-artifacts/tests/test-summary.md
- crates/domain/src/lib.rs
- crates/domain/src/reporting.rs
- crates/persistence/migrations/20260407022400_reporting_read_models.sql
- crates/persistence/src/postgres/mod.rs
- crates/persistence/src/postgres/reporting_read_models.rs
- docs/operations/normalized-reporting-read-models.md
- package.json
- services/reporting-service/Cargo.toml
- services/reporting-service/src/lib.rs
- services/reporting-service/src/main.rs
- services/reporting-service/src/read_models/mod.rs
- services/reporting-service/src/read_models/queries.rs

### Change Log

- 2026-04-07: Created Story 4.1 context file and advanced lifecycle from `backlog` to `ready-for-dev`.
- 2026-04-07: Implemented Story 4.1 normalized reporting read-model contracts, persistence migration/adapters, reporting-service orchestration seam, QA command wiring, and operations runbook updates.
- 2026-04-07: Completed adversarial code review, fixed incident optional-identifier canonicalization in the risk-event read-model projection, added regression coverage for the migration contract, and advanced status to `done`.
- 2026-04-07: Refreshed Story 4.1 QA automation with additional malformed-filter and evidence-timestamp fail-closed regression tests; reran `qa:test:story-4-1` and retained `done` status.
