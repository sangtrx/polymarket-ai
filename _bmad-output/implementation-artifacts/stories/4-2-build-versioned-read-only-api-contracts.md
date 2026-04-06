# Story 4.2: Build Versioned Read-Only API Contracts

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an external integration user,  
I want stable, versioned read-only APIs,  
so that integrations remain robust across product evolution.

## Acceptance Criteria

1. **Scenario A - versioned read-only reporting endpoints (story-local BDD):**  
   **Given** API consumers call reporting endpoints  
   **When** contract versions evolve  
   **Then** endpoints remain versioned with published schema artifacts and changelogs  
   **And** compatibility behavior satisfies FR37/NFR13 constraints.
2. **Scenario B - complete FR37 dataset coverage:**  
   **Given** integration clients require downstream analytics consumption  
   **When** they call versioned reporting APIs  
   **Then** read-only interfaces are available for trades, positions, performance, risk events, and alpha attribution  
   **And** each response includes deterministic evidence metadata inherited from Story 4.1 surfaces (`as_of_utc`, `source`, `reason_code`, `correlation_id`).
3. **Scenario C - contract artifact publication and discoverability:**  
   **Given** a contract version is active  
   **When** clients request contract metadata  
   **Then** machine-readable schema artifacts and changelog entries are retrievable for that version  
   **And** version metadata includes release/deprecation/support fields suitable for operational governance.
4. **Scenario D - compatibility lifecycle enforcement (NFR13):**  
   **Given** a contract version is marked deprecated/replaced  
   **When** lifecycle metadata is written or updated  
   **Then** deprecation notice is at least 90 days  
   **And** backward-compatible support is preserved for at least 6 months after replacement.
5. **Scenario E - authorization and failure behavior:**  
   **Given** a caller lacks read-analytics authorization or submits invalid version/filter input  
   **When** a versioned endpoint is invoked  
   **Then** the request fails closed with explicit machine-readable errors  
   **And** no success-shaped payload is returned.
6. **UAC-1 Failure handling:** Invalid query windows, malformed identifiers, invalid/unknown contract versions, unauthorized access, unavailable persistence/schema artifact dependencies, and stale contract metadata paths return explicit machine-readable errors with no unsafe side effects.
7. **UAC-2 Boundary behavior:** Deterministic and test-covered boundary behavior is enforced for inclusive/exclusive UTC windows, pagination limits, explicit version selection precedence (`contract_version` query param -> active contract fallback), and stable tie-break ordering for all dataset responses.
8. **UAC-3 Verifiable evidence:** All successful and failed contract/query responses emit timestamped traceability fields and audit/telemetry evidence linking actor, endpoint, contract version, reason code, and correlation id.
9. **Schema/dependency/traceability contract:** Story depends only on `4.1`; schema scope introduces only `api_contract_versions`; traceability maps explicitly to `FR37` and `NFR13`.
10. **Scope boundary contract:** Story 4.2 delivers versioned read-only API contracts only; recurring report scheduling (Story 4.3) and export workflows (Story 4.4) remain out of scope.

## Tasks / Subtasks

- [x] **Task 1: Add version-registry persistence for reporting API contracts** (AC: 1, 2, 3, 4, 6, 9, 10)
  - [x] Add a forward-only migration under `crates/persistence/migrations/` that introduces `api_contract_versions` only (no `report_schedules`, `report_runs`, `export_jobs`, `export_artifacts`).
  - [x] Include contract-key + version uniqueness, lifecycle timestamps, schema/changelog artifact locations, and checksum/immutability fields needed for governance-grade publication.
  - [x] Add DB-level constraints/indexes enforcing valid lifecycle ordering (release -> deprecation notice -> support window/sunset) and efficient lookup by contract key + active version.
  - [x] Wire persistence module export in `crates/persistence/src/postgres/mod.rs` for contract-version reads/writes.

- [x] **Task 2: Implement contract lifecycle and version policy logic** (AC: 1, 3, 4, 6, 7, 9)
  - [x] Implement typed contract lifecycle entities and validation in `services/reporting-service/src/contracts/` (version id, status, effective window, replacement reference, schema/changelog linkage).
  - [x] Enforce NFR13 lifecycle invariants in code paths: minimum 90-day deprecation notice and minimum 6-month backward-compatible support after replacement.
  - [x] Keep lifecycle metadata writes internal to governed service/persistence flows (no public mutation endpoints for contract lifecycle state).
  - [x] Keep machine-readable reason-code taxonomy aligned with existing reporting reason conventions (no ad hoc free-form errors).
  - [x] Add unit tests for valid/invalid lifecycle transitions and boundary windows.

- [x] **Task 3: Expose versioned read-only reporting endpoints in reporting-service** (AC: 1, 2, 5, 6, 7, 8, 10)
  - [x] Add HTTP routing/handler surfaces in `services/reporting-service` for versioned endpoints (plural kebab-case resources), including:
    - [x] trades,
    - [x] positions,
    - [x] risk-events,
    - [x] performance,
    - [x] alpha-attribution.
  - [x] Use canonical route shape `/api/v1/reporting/<dataset>` with snake_case query filters; support optional `contract_version` query param and enforce deterministic resolution order (`contract_version` override, otherwise active version from `api_contract_versions`).
  - [x] Reuse Story 4.1 read-model orchestrator (`read_models::queries`) directly; do not reshape core dataset semantics into parallel models.
  - [x] For alpha-attribution responses, reuse existing normalized attribution/read-model seams (Story 3.4 + Story 4.1 lineage) instead of introducing new attribution truth stores.
  - [x] Enforce read-only authorization via canonical governance role/permission checks (`read_only_analytics` / `ReadAnalytics`) and explicit machine-readable unauthorized failures.
  - [x] Return canonical API envelopes (`data`, `meta`, `error`) and deterministic metadata contracts (UTC timestamps, correlation id, reason code, contract version).

- [x] **Task 4: Publish machine-readable schema artifacts and changelog surfaces** (AC: 1, 3, 4, 6, 8, 9)
  - [x] Add version-scoped schema artifacts for each reporting dataset contract in `services/reporting-service/src/contracts/` (or architecture-consistent subpaths).
  - [x] Expose read-only artifact discovery routes for version metadata and schema/changelog retrieval.
  - [x] Ensure persisted version records in `api_contract_versions` point to concrete artifact identifiers and immutable checksums/hash references.
  - [x] Add regression checks ensuring artifact payloads stay synchronized with runtime serialized response shape.

- [x] **Task 5: Preserve auditability and observability invariants for contract reads** (AC: 5, 6, 8, 9)
  - [x] Emit query-level audit/telemetry evidence for each contract endpoint invocation (actor, role, endpoint, version, dataset, reason_code, correlation_id, timestamp_utc).
  - [x] Keep failure handling fail-closed for unavailable/stale contract metadata or schema artifact resolution failures.
  - [x] Reuse existing audit/error response conventions already established in service routes and middleware patterns.

- [x] **Task 6: Add Story 4.2 QA automation and evidence updates** (AC: 1, 2, 3, 4, 5, 6, 7, 8, 9)
  - [x] Add `qa:test:story-4-2` in root `package.json` following repository story QA command conventions.
  - [x] Add Rust test coverage for:
    - [x] contract lifecycle validation (`NFR13` notice/support windows),
    - [x] persistence-level constraints and version lookup behavior for `api_contract_versions`,
    - [x] reporting-service versioned route behavior (success, empty, invalid payload, unauthorized, dependency unavailable, stale contract metadata).
  - [x] Add contract-level tests under `tests/contract/` verifying schema artifact publication and changelog discoverability.
  - [x] Update `_bmad-output/implementation-artifacts/tests/test-summary.md` with Story 4.2 coverage/evidence.

- [x] **Task 7: Document contract operations handoff boundaries** (AC: 3, 4, 9, 10)
  - [x] Add/update operations guidance under `docs/operations/` for contract version publication, deprecation workflow, support-window governance, and incident handling for artifact mismatch.
  - [x] Explicitly document Story 4.2 non-goals (no scheduling orchestration, no export-job orchestration, no control-plane mutation behavior).

### Review Findings

- [x] [Review][Patch] Enforced `read_only_analytics` authorization for contract discovery/artifact routes to prevent unauthorized metadata access.
- [x] [Review][Patch] Added regression coverage for unauthorized access attempts on contract discovery and schema artifact routes.

## Dev Notes

### Technical Requirements

- Story objective is FR37 coverage through versioned, read-only integration APIs exposing normalized reporting datasets for trades, positions, pnl/performance, risk events, and alpha attribution.
- Story 4.1 is the direct dependency and already provides normalized read-model orchestration + fail-closed semantics; Story 4.2 must consume these seams directly rather than duplicating normalization logic.
- Route/version contract must stay deterministic and explicit: API major path versioning (`/api/v1/reporting/<dataset>`) plus optional `contract_version` query selector with active-version fallback when omitted.
- Alpha-attribution coverage must extend existing attribution/read-model lineage (Story 3.4 attribution snapshots consumed through Story 4.1/4.2 seams), not introduce parallel attribution truth models.
- NFR13 governs all lifecycle behavior:
  - versioned integration contracts,
  - at least 90-day deprecation notice,
  - backward compatibility for at least 6 months after replacement.
- `api_contract_versions` is the only schema addition allowed in this story.
- Existing reporting reason-code taxonomy and UTC/canonical identifier validation behavior from Story 4.1 must remain intact.

[Source: _bmad-output/planning-artifacts/epics.md#Story 4.2: Build Versioned Read-Only API Contracts]  
[Source: _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)]  
[Source: _bmad-output/planning-artifacts/prd.md#External Interfaces & Reporting]  
[Source: _bmad-output/planning-artifacts/prd.md#Integration]  
[Source: docs/operations/normalized-reporting-read-models.md#Story 4.2 handoff seam]  
[Source: crates/domain/src/reporting.rs]  
[Source: services/reporting-service/src/read_models/queries.rs]

### Architecture Compliance

- Maintain bounded-context ownership:
  - `reporting-service` owns read-only external reporting contracts,
  - `control-api` remains privileged control-plane API and should not absorb Story 4.2 contract surfaces.
- Keep API naming/format conventions:
  - resource paths in plural kebab-case,
  - query params in snake_case,
  - UTC ISO-8601 timestamps,
  - canonical success/error envelope structure.
- Keep versioning behavior deterministic and machine-verifiable:
  - route base remains `/api/v1/reporting/*`,
  - contract selection uses optional `contract_version` with explicit fallback to active version,
  - unknown/unsupported versions fail with machine-readable errors (no silent fallback).
- Preserve data boundaries:
  - reporting payloads are projections from Story 4.1 read models (not a new source of truth),
  - no mutation endpoints are introduced in reporting-service.
- Preserve explicit machine-readable error mapping and fail-closed behavior for uncertain dependency state.

[Source: _bmad-output/planning-artifacts/architecture.md#API Boundaries]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: _bmad-output/planning-artifacts/architecture.md#Pattern Categories Defined]  
[Source: _bmad-output/planning-artifacts/architecture.md#Format Patterns]  
[Source: _bmad-output/planning-artifacts/architecture.md#Data Boundaries]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: services/control-api/src/middleware/mod.rs]

### Library & Framework Requirements

- Keep workspace-pinned stack unless story acceptance criteria explicitly requires otherwise:
  - `axum = 0.8.8`
  - `sqlx = 0.8.6`
  - `tokio = 1.48.0`
  - `time = 0.3.44`
  - `serde = 1.0.228`
  - `serde_json = 1.0.145`
  - `polymarket-client-sdk = 0.4.4`
- Latest checks at story-creation time (via `cargo search`) indicate:
  - `axum` latest stable matches workspace (`0.8.8`),
  - `polymarket-client-sdk` latest stable matches workspace (`0.4.4`),
  - newer `tokio`/`time` exist and `sqlx` newest is pre-release (`0.9.0-alpha.1`).
- Do not perform opportunistic dependency upgrades in Story 4.2.

[Source: Cargo.toml]  
[Source: source $HOME/.cargo/env && cargo search axum --limit 1]  
[Source: source $HOME/.cargo/env && cargo search sqlx --limit 1]  
[Source: source $HOME/.cargo/env && cargo search tokio --limit 1]  
[Source: source $HOME/.cargo/env && cargo search time --limit 1]  
[Source: source $HOME/.cargo/env && cargo search polymarket-client-sdk --limit 1]

### File Structure Requirements

- Primary implementation surfaces for Story 4.2:
  - `services/reporting-service/src/main.rs`
  - `services/reporting-service/src/lib.rs`
  - `services/reporting-service/src/contracts/*` (contract lifecycle + schema/changelog publication)
  - `services/reporting-service/src/read_models/queries.rs` (reuse integration seam; no semantic reshaping)
  - `crates/persistence/migrations/*api_contract_versions*.sql` (new)
  - `crates/persistence/src/postgres/{mod.rs,api_contract_versions.rs}` (new adapter)
  - `package.json` (add `qa:test:story-4-2`)
  - `tests/contract/*story-4-2*`
  - `_bmad-output/implementation-artifacts/tests/test-summary.md`
  - `docs/operations/*reporting*contract*` (runbook/changelog operations guidance)
- Keep Story 4 boundaries explicit:
  - Story 4.1: normalized read models,
  - Story 4.2: versioned read-only contracts,
  - Story 4.3: recurring summary scheduling,
  - Story 4.4: export workflows.

[Source: _bmad-output/planning-artifacts/architecture.md#Complete Project Directory Structure]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: services/reporting-service/src/{main.rs,lib.rs,contracts/mod.rs,read_models/queries.rs}]  
[Source: crates/persistence/src/postgres/mod.rs]  
[Source: package.json]  
[Source: docs/operations/normalized-reporting-read-models.md]

### Testing Requirements

- Add deterministic coverage for:
  - contract-version lifecycle validation (notice/support windows and replacement sequencing),
  - contract-version lookup precedence (`contract_version` override vs active fallback) and unknown/unsupported version failure behavior,
  - schema artifact + changelog publication/discovery,
  - endpoint response envelope conformance (`data/meta/error`) and machine-readable errors,
  - unauthorized role failures and dependency-unavailable/stale dependency fail-closed behavior,
  - boundary validation for query windows/limits/identifier canonicalization.
- Preserve existing QA pattern:
  - story-scoped root command in `package.json`,
  - Rust tests for domain/persistence/service seams,
  - contract/API tests under top-level `tests/`,
  - test evidence update in `_bmad-output/implementation-artifacts/tests/test-summary.md`.

[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1-6.9)]  
[Source: _bmad-output/planning-artifacts/architecture.md#Pattern Enforcement]  
[Source: package.json]  
[Source: _bmad-output/implementation-artifacts/tests/test-summary.md#Story 4.1 QA Automation Refresh]

### Previous Story Intelligence

- Story 4.1 created normalized reporting read models and explicit fail-closed orchestration; Story 4.2 should build transport/versioning on top of those contracts instead of duplicating data-shape logic.
- Story 4.1 runbook explicitly calls out Story 4.2 handoff and non-goals (schema/changelog publication deferred to this story).
- Story 4.1 migration tests intentionally assert `api_contract_versions` was not introduced yet; Story 4.2 should now add it with similarly strict scope tests.
- Existing reporting read query contract already validates read-only permissions and canonical request boundaries; Story 4.2 should reuse this contract path to reduce regression risk.
- Story 4.1 lineage already anchors performance/attribution semantics to existing persistence seams; Story 4.2 should preserve that lineage for alpha-attribution API contracts instead of introducing duplicate truth paths.

[Source: _bmad-output/implementation-artifacts/stories/4-1-define-normalized-reporting-read-models.md#Tasks / Subtasks]  
[Source: _bmad-output/implementation-artifacts/stories/4-1-define-normalized-reporting-read-models.md#File Structure Requirements]  
[Source: docs/operations/normalized-reporting-read-models.md#Story 4.2 handoff seam]  
[Source: crates/persistence/src/postgres/reporting_read_models.rs]  
[Source: services/reporting-service/src/read_models/queries.rs]

### Git Intelligence Summary

- Recent commits show a consistent vertical-slice implementation pattern:
  1. domain/persistence contracts and migration scope,
  2. service orchestration + machine-readable failure semantics,
  3. QA command wiring and story-targeted tests,
  4. operations documentation + test evidence updates.
- Story 4.1 commit demonstrates desired bounded-scope changes concentrated in reporting-service, domain/persistence seams, and story QA evidence.
- For Story 4.2, follow the same bounded pattern and avoid broad cross-service refactors.

[Source: git --no-pager log --oneline -5]  
[Source: git --no-pager show --name-only --pretty='format:%h %s' -3]

### Latest Technical Information

- Crate version checks confirm workspace pins remain valid for this story:
  - `axum` and `polymarket-client-sdk` latest stable align with pins,
  - newer `tokio`/`time` exist,
  - `sqlx` newest line is pre-release.
- Given Story 4.2 scope, keep pinned versions and focus on contract correctness, compatibility semantics, and regression-safe routing.

[Source: source $HOME/.cargo/env && cargo search axum --limit 1]  
[Source: source $HOME/.cargo/env && cargo search sqlx --limit 1]  
[Source: source $HOME/.cargo/env && cargo search tokio --limit 1]  
[Source: source $HOME/.cargo/env && cargo search time --limit 1]  
[Source: source $HOME/.cargo/env && cargo search polymarket-client-sdk --limit 1]

### Project Context Reference

- No repository `project-context.md` artifact was found during discovery.
- Story context is derived from epics, PRD, architecture, readiness/validation artifacts, Story 4.1 implementation context, codebase seams, and recent git history.

### Project Structure Notes

- `services/reporting-service` currently contains:
  - `src/main.rs` bootstrap seam,
  - `src/read_models/queries.rs` internal normalized-query orchestration from Story 4.1,
  - `src/contracts/mod.rs` placeholder reserved for Story 4.2.
- `services/control-api` already demonstrates authentication middleware and machine-readable envelope/error patterns that can be mirrored for reporting-service route design without changing control-plane ownership boundaries.
- `tests/contract/` is currently mostly empty (`.gitkeep`), making Story 4.2 an appropriate place to establish reporting-contract regression tests there.

[Source: services/reporting-service/src/{main.rs,lib.rs,contracts/mod.rs,read_models/queries.rs}]  
[Source: services/control-api/src/{middleware/mod.rs,routes/mod.rs}]  
[Source: tests/contract/.gitkeep]

### References

- _bmad-output/planning-artifacts/epics.md#Epic 4: Reporting, Exports & External Analytics Integrations  
- _bmad-output/planning-artifacts/epics.md#Story 4.2: Build Versioned Read-Only API Contracts  
- _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)  
- _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1-6.9)  
- _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)  
- _bmad-output/planning-artifacts/prd.md#External Interfaces & Reporting  
- _bmad-output/planning-artifacts/prd.md#Integration  
- _bmad-output/planning-artifacts/prd.md#Non-Functional Requirements  
- _bmad-output/planning-artifacts/architecture.md#API Boundaries  
- _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping  
- _bmad-output/planning-artifacts/architecture.md#Pattern Categories Defined  
- _bmad-output/planning-artifacts/architecture.md#Format Patterns  
- _bmad-output/planning-artifacts/architecture.md#Complete Project Directory Structure  
- _bmad-output/planning-artifacts/implementation-readiness-report-2026-04-05.md  
- _bmad-output/planning-artifacts/prd-validation-report.md  
- docs/operations/normalized-reporting-read-models.md  
- _bmad-output/implementation-artifacts/stories/4-1-define-normalized-reporting-read-models.md  
- crates/domain/src/reporting.rs  
- crates/persistence/src/postgres/{mod.rs,reporting_read_models.rs}  
- crates/persistence/migrations/20260407022400_reporting_read_models.sql  
- services/reporting-service/src/{main.rs,lib.rs,contracts/mod.rs,read_models/queries.rs}  
- services/control-api/src/{middleware/mod.rs,routes/mod.rs}  
- crates/persistence/src/postgres/attribution_snapshots.rs  
- Cargo.toml  
- package.json  
- tests/contract/.gitkeep  
- git --no-pager log --oneline -5  
- git --no-pager show --name-only --pretty='format:%h %s' -3  
- source $HOME/.cargo/env && cargo search axum --limit 1  
- source $HOME/.cargo/env && cargo search sqlx --limit 1  
- source $HOME/.cargo/env && cargo search tokio --limit 1  
- source $HOME/.cargo/env && cargo search time --limit 1  
- source $HOME/.cargo/env && cargo search polymarket-client-sdk --limit 1

## Story Completion Status

- Story context generated with exhaustive artifact analysis across epic/prd/architecture/ux/readiness artifacts, Story 4.1 continuity, current code seams, and recent git patterns.
- Story status is set to `done`.
- Completion note: Implementation and adversarial code-review remediation completed with all HIGH/MEDIUM findings resolved.

## Dev Agent Record

### Agent Model Used

GPT-5.3-Codex (gpt-5.3-codex)

### Debug Log References

- BMAD create-story workflow execution (automated, non-interactive)
- Sprint backlog discovery from `_bmad-output/implementation-artifacts/sprint-status.yaml`
- Artifact discovery across planning docs, previous stories, service seams, and operations runbooks
- Git pattern analysis using recent commit/file history
- Crate version checks via `cargo search`
- Story 4.2 contract migration + persistence adapter implementation (`api_contract_versions`)
- Story 4.2 reporting-service contract routes/artifacts/lifecycle implementation and route-level failure handling
- QA execution: `npm run --silent qa:test:story-4-2`
- Regression execution: `npm test`
- QA refresh execution: `source $HOME/.cargo/env && npm run --silent rust:lint` (pre-existing domain clippy findings), `source $HOME/.cargo/env && npm run --silent rust:build`, `source $HOME/.cargo/env && npm run --silent qa:test:story-4-2`, and `source $HOME/.cargo/env && npm run --silent test`

### Completion Notes List

- Selected first backlog story: `4-2-build-versioned-read-only-api-contracts`.
- Extracted Story 4.2 requirements from Epic 4 plus FR37/NFR13 constraints and universal AC contracts.
- Carried forward Story 4.1 learnings to enforce seam reuse and prevent duplicate normalization logic.
- Added explicit implementation guardrails for version lifecycle policy, schema/changelog publication, read-only auth, and fail-closed behavior.
- Validation remediation clarified deterministic version-selection precedence, canonical route/resource naming, and alpha-attribution seam reuse constraints.
- Implemented `api_contract_versions` migration with lifecycle/checksum constraints, active-version uniqueness, and lookup indexes.
- Added persistence adapter (`api_contract_versions.rs`) with typed upsert/read/list APIs, strict payload validation, and migration scope tests.
- Implemented typed contract lifecycle + artifact registry modules with NFR13 policy validation and checksum/path parity checks.
- Added versioned reporting routes (`/api/v1/reporting/*`) and contract discovery/artifact routes with deterministic `data/meta/error` envelopes and machine-readable failures.
- Added query-level telemetry emission containing actor, role, endpoint, dataset, contract version, reason code, correlation id, and timestamp evidence.
- Added Story 4.2 QA command plus Rust/contract test coverage and updated implementation test summary evidence.
- Added API regression coverage in `services/reporting-service/src/api.rs` validating that explicit `contract_version` query overrides are forwarded to contract-resolution logic and omission paths correctly fall back to active-version resolution.
- Workspace `rust:lint` currently reports pre-existing `clippy::collapsible_if` findings in `crates/domain/src/recovery.rs` (outside Story 4.2 changed surfaces); story QA and full test suite execution still pass.
- Code review identified and fixed a security gap: contract discovery/artifact routes now enforce `read_only_analytics` authorization consistently with dataset routes.
- Story file list and git-diff comparison found one extra non-application artifact (`.scripts/bmad-auto/copilot/bmad-progress.log`) outside Story 4.2 source scope.

### File List

- Cargo.lock
- package.json
- _bmad-output/implementation-artifacts/sprint-status.yaml
- _bmad-output/implementation-artifacts/tests/test-summary.md
- _bmad-output/implementation-artifacts/stories/4-2-build-versioned-read-only-api-contracts.md
- crates/persistence/migrations/20260407033000_api_contract_versions.sql
- crates/persistence/src/postgres/api_contract_versions.rs
- crates/persistence/src/postgres/mod.rs
- docs/operations/normalized-reporting-read-models.md
- services/reporting-service/Cargo.toml
- services/reporting-service/src/api.rs
- services/reporting-service/src/contracts/mod.rs
- services/reporting-service/src/contracts/lifecycle.rs
- services/reporting-service/src/contracts/artifacts.rs
- services/reporting-service/src/contracts/artifacts/v1/trades.schema.json
- services/reporting-service/src/contracts/artifacts/v1/positions.schema.json
- services/reporting-service/src/contracts/artifacts/v1/risk-events.schema.json
- services/reporting-service/src/contracts/artifacts/v1/performance.schema.json
- services/reporting-service/src/contracts/artifacts/v1/alpha-attribution.schema.json
- services/reporting-service/src/contracts/artifacts/v1/changelog.json
- services/reporting-service/src/lib.rs
- services/reporting-service/src/main.rs
- tests/contract/story-4-2-reporting-contract-artifacts.test.mjs

### Change Log

- 2026-04-07: Created Story 4.2 context file and advanced lifecycle from `backlog` to `ready-for-dev`.
- 2026-04-07: Validate-story remediation tightened contract version-resolution semantics, endpoint naming guardrails, and attribution seam reuse requirements; validation re-run passed.
- 2026-04-07: Implemented Story 4.2 end-to-end: version-registry migration/persistence, lifecycle validation, versioned reporting + contract artifact routes, telemetry evidence, QA automation command, contract tests, and operations runbook updates; story advanced to `review`.
- 2026-04-07: Code review auto-fix applied: enforced read-analytics authorization on contract metadata/artifact routes, added unauthorized-route regression tests, and advanced story status to `done`.
- 2026-04-07: QA automation refresh added explicit contract-version override/fallback forwarding coverage in `api::tests::versioned_route_passes_contract_version_override_to_resolution_port`; reran Story 4.2 QA suite and full repository test suite with story status remaining `done`.
