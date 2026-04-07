# Story 6.3: Implement Validation Workflow and Diagnostics Artifact Store

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As a research user,  
I want end-to-end validation execution with persistent evidence,  
so that promotion decisions are evidence-backed and auditable.

## Acceptance Criteria

1. **Scenario A - FR7 baseline (story-local BDD):**  
   **Given** a candidate is submitted for validation  
   **When** workflow stages (quality, labeling, purged CV, CPCV, overfit diagnostics) execute  
   **Then** failures block progression and diagnostics are stored for comparison  
   **And** FR7 and FR44 requirements are met.

2. **Scenario B - deterministic stage workflow contract:**  
   **Given** a validation request is accepted  
   **When** workflow orchestration runs  
   **Then** stages execute in deterministic order (`quality -> labeling -> purged_cv -> cpcv -> overfit_diagnostics`)  
   **And** stage transitions are timestamped, machine-readable, and test-covered.

3. **Scenario C - mandatory FR43 gate dependency enforcement at entry:**  
   **Given** Story 6.2 gate policies are configured  
   **When** a candidate enters the validation workflow  
   **Then** the existing training-entry gate evaluator is invoked before stage execution  
   **And** unresolved/missing mandatory gate policy state denies the run fail-closed.

4. **Scenario D - fail-closed progression on stage failure:**  
   **Given** a stage fails or required inputs are unavailable  
   **When** the run is evaluated for continuation  
   **Then** downstream stages are not executed  
   **And** run outcome is stored as blocked/failed with explicit machine-readable reason codes.

5. **Scenario E - FR44 diagnostics payload contract:**  
   **Given** stage computations produce diagnostics  
   **When** artifacts are persisted  
   **Then** each candidate run stores comparable diagnostics including out-of-sample Sharpe, max drawdown, and Brier score (or expected calibration error) with overfit indicators  
   **And** diagnostics payload schema is deterministic and queryable.

6. **Scenario F - artifact persistence and schema isolation:**  
   **Given** validation stages complete (pass or fail)  
   **When** evidence is written  
   **Then** evidence is persisted only in `validation_runs` and `validation_artifacts` with actor/correlation/timestamp metadata  
   **And** no unrelated schema entities are introduced in this story.

7. **Scenario G - diagnostics comparison readiness:**  
   **Given** multiple runs exist for a candidate  
   **When** diagnostics are queried  
   **Then** current run metrics can be compared against prior runs deterministically (ordered by run timestamp and stage)  
   **And** comparison output preserves machine-readable metric keys and reason codes.

8. **Scenario H - authenticated control-plane run and evidence surfaces:**  
   **Given** authorized users start or inspect validation runs/artifacts  
   **When** control-plane routes execute  
   **Then** responses follow canonical `data/meta/error` envelope conventions  
   **And** unauthorized, malformed, or unavailable states map to explicit deterministic status codes.

9. **Scenario I - unavailable dependency behavior:**  
   **Given** required stage executors, policy state, or persistence dependencies are unavailable  
   **When** orchestration executes  
   **Then** requests fail closed with explicit `*_dependency_unavailable`, `*_state_unavailable`, or `*_persistence_unavailable` style reason codes  
   **And** no success-shaped fallback path is emitted.

10. **Scenario J - NFR14 and NFR17 evidence continuity:**  
    **Given** allow and deny outcomes for validation-run start/read/list/artifact retrieval  
    **When** telemetry and audit events emit  
    **Then** records include actor, action, stage, parameters, reason code, correlation id, and UTC timestamp  
    **And** evidence remains queryable for incident and QA workflows.

11. **UAC-1 Failure handling:** Invalid payloads, unauthorized access, and unavailable dependency paths return explicit machine-readable errors with no unsafe side effects.

12. **UAC-2 Boundary behavior:** Stage-order boundaries, comparator semantics, and run-state transitions are deterministic (inclusive/exclusive rules documented and test-covered).

13. **UAC-3 Verifiable evidence:** Successful and failed validation operations emit timestamped telemetry and audit evidence suitable for incident and QA traceability.

14. **Schema/dependency/traceability contract:** Story depends on `6.2`; schema scope introduces only `validation_runs` and `validation_artifacts`; traceability maps to `FR7`, `FR44`, `NFR14`, and `NFR17`.

15. **Scope boundary contract:** Story 6.3 delivers validation-run execution and diagnostics artifact persistence only; it must not pre-implement Story 6.4 shadow-mode simulation, Story 6.5 promotion-threshold approvals, or Stories 6.6-6.9 lifecycle automation/UX flows.

## Tasks / Subtasks

- [x] **Task 1: Define FR7/FR44 domain contracts and reason-code taxonomy** (AC: 1, 2, 4, 5, 9, 11, 12, 14)
  - [x] Extend `crates/domain/src/research.rs` with validation workflow stage, run-state, diagnostics-artifact, and machine-readable reason-code contracts for FR7/FR44.
  - [x] Add canonical validation helpers for run identifiers, stage ordering, UTC timestamps, and diagnostics payload fields.
  - [x] Define deterministic comparison-contract types for run-level metric deltas and stage-level outcomes.
  - [x] Preserve explicit fail-closed error taxonomy (no broad success defaults for ambiguous state).

- [x] **Task 2: Add forward-only migration and persistence adapters for `validation_runs` and `validation_artifacts`** (AC: 2, 4, 5, 6, 7, 9, 12, 14)
  - [x] Add migration under `crates/persistence/migrations/` creating only `validation_runs` and `validation_artifacts` with canonical constraints and UTC evidence metadata.
  - [x] Add persistence modules (recommended: `crates/persistence/src/postgres/{validation_runs.rs,validation_artifacts.rs}`) and wire through `crates/persistence/src/postgres/mod.rs`.
  - [x] Implement deterministic create/read/list operations for runs and artifacts, including candidate-scoped and run-scoped queries.
  - [x] Add persistence tests validating schema-scope isolation, deterministic ordering, constraint enforcement, and machine-readable error classification.

- [x] **Task 3: Implement research-gateway validation workflow orchestration** (AC: 1, 2, 3, 4, 5, 6, 7, 9, 10, 12)
  - [x] Add orchestration module in `services/research-gateway/src/validation/` (recommended: `workflow_runs.rs`) and export via `validation/mod.rs`.
  - [x] Reuse `evaluate_training_entry_gates(...)` from Story 6.2 before stage execution.
  - [x] Introduce explicit stage-execution seam(s) for quality, labeling, purged CV, CPCV, and overfit diagnostics with deterministic fail-closed handling when executors are unavailable.
  - [x] Persist run state transitions and stage artifacts, then build deterministic diagnostics-comparison output for prior-run lookups.
  - [x] Emit structured telemetry for allow/deny outcomes with stage, reason code, actor, candidate id, correlation id, and timestamp.

- [x] **Task 4: Expose authenticated control-plane validation-run and artifact routes** (AC: 6, 7, 8, 9, 10, 11, 14)
  - [x] Add authenticated routes in `services/control-api/src/routes/mod.rs` for run start/read/list and artifact retrieval under `/control/research/validation-runs...`.
  - [x] Add canonical payload/query/response structs reusing `data/meta/error` conventions and deterministic status mapping (`400/403/409/503/500`).
  - [x] Wire new orchestrator state in `services/control-api/src/middleware/mod.rs` and startup wiring in `services/control-api/src/main.rs`.
  - [x] Append privileged audit records for both success and denial paths with action, stage, parameters, reason, correlation id, and timestamp continuity.

- [x] **Task 5: Preserve downstream integration seams for Stories 6.4 and 6.5** (AC: 2, 4, 5, 7, 15)
  - [x] Add explicit read/query contracts that downstream shadow-mode and promotion workflows can consume without breaking API compatibility.
  - [x] Ensure run/artifact model captures the minimum validation packet needed for future promotion-gating checks, without implementing promotion decisions in this story.
  - [x] Document deferred integration points in code and runbook so future stories reuse contracts instead of forking logic.

- [x] **Task 6: Add deterministic Story 6.3 automated coverage and QA command wiring** (AC: 1-15)
  - [x] Add domain tests for stage-order validation, run-state transitions, diagnostics payload boundary rules, and reason-code parsing.
  - [x] Add persistence tests for run/artifact constraints, query ordering, and failure classification.
  - [x] Add research-gateway tests for gate-precheck integration, stage failure blocking, diagnostics persistence, and comparison semantics.
  - [x] Add control-api route tests for authenticated run/artifact operations, envelope contracts, unauthorized-role denial, and status mapping.
  - [x] Add story-scoped API/E2E tests (`tests/api/story-6-3*.test.mjs`, `tests/e2e/story-6-3*.test.mjs`) and wire `qa:test:story-6-3` in root `package.json`.

- [x] **Task 7: Publish FR7/FR44 operations guidance and traceability artifacts** (AC: 8, 10, 13, 14)
  - [x] Add `docs/operations/alpha-validation-workflow-and-diagnostics.md` documenting stage pipeline semantics, artifact schema contract, comparison queries, and fail-closed playbooks.
  - [x] Cross-link with `alpha-hypothesis-registry`, `alpha-validation-gate-policies`, and report-export/governance runbooks.
  - [x] Update `_bmad-output/implementation-artifacts/tests/test-summary.md` with Story 6.3 evidence after implementation.

### Review Findings

- [x] [Review][Patch] Comparison baseline now excludes newer runs when reading historical validation runs [services/research-gateway/src/validation/workflow_runs.rs:1194]
- [x] [Review][Patch] Added regression coverage proving older runs are never compared against newer baselines [services/research-gateway/src/validation/workflow_runs.rs:2225]
- [x] [Review][Defer] Repository-wide `rust:lint` still reports pre-existing clippy findings outside this story scope (`crates/domain/src/recovery.rs`); left unchanged per non-story scope guardrails [crates/domain/src/recovery.rs:694] — deferred, pre-existing
- [x] [Review][Defer] Git cross-check found `.scripts/bmad-auto/copilot/bmad-progress.log` outside story File List and outside application-source review scope; excluded from adjudication [.scripts/bmad-auto/copilot/bmad-progress.log:1] — deferred, pre-existing

## Dev Notes

### Technical Requirements

- Story objective is FR7 + FR44 enforceability: run full validation workflow with persistent diagnostics artifacts and fail-closed progression control.
- Story-level dependency/scope contract:
  - dependency: `6.2`,
  - schema scope: `validation_runs`, `validation_artifacts`,
  - traceability: `FR7`, `FR44`, `NFR14`, `NFR17`.
- Mandatory workflow stages for this story:
  - `quality`,
  - `labeling`,
  - `purged_cv`,
  - `cpcv`,
  - `overfit_diagnostics`.
- Required FR44 diagnostics dimensions must be storable and queryable:
  - out-of-sample Sharpe,
  - max drawdown,
  - Brier score (or expected calibration error),
  - overfit indicators.
- Validation workflow must fail closed when:
  - FR43 gate policy state is unresolved/missing,
  - stage executors or required inputs are unavailable,
  - persistence is unavailable or returns ambiguous state.
- Out of scope for Story 6.3:
  - shadow-mode execution simulation (Story 6.4),
  - promotion threshold and approval enforcement (Story 6.5),
  - counterfactual replay, live-health deallocation, and governance card UX (Stories 6.6-6.9).

[Source: _bmad-output/planning-artifacts/epics.md#Story 6.3: Implement Validation Workflow and Diagnostics Artifact Store]  
[Source: _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)]  
[Source: _bmad-output/planning-artifacts/prd.md#Strategy Research & Alpha Lifecycle]  
[Source: _bmad-output/planning-artifacts/prd.md#Model Integrity & Promotion Governance]  
[Source: _bmad-output/planning-artifacts/prd.md#Journey 6 — Research User (Phase 2+): Noor, Quant Research Lead]

### Architecture Compliance

- Preserve bounded architecture ownership:
  - `control-api` remains authenticated ingress and canonical envelope/audit surface,
  - `research-gateway` owns validation workflow orchestration and evidence contracts,
  - `crates/persistence` owns deterministic data access and error classification.
- Follow architecture conventions exactly:
  - plural kebab-case route resources,
  - snake_case Rust modules/files/DB identifiers,
  - UTC ISO-8601 timestamps,
  - canonical machine-readable reason codes with explicit status mapping.
- Enforce process guardrails:
  - distinguish domain, dependency, and persistence failures,
  - no swallowed errors in validation/control flows,
  - uncertain state must deny progression.
- NFR14/NFR17 obligations are explicit:
  - structured logs/metrics/traces for run lifecycle and stage outcomes,
  - audit/telemetry evidence queryable by actor/action/stage/reason/correlation/timestamp.

[Source: _bmad-output/planning-artifacts/architecture.md#Data Architecture]  
[Source: _bmad-output/planning-artifacts/architecture.md#API & Communication Patterns]  
[Source: _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules]  
[Source: _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]

### Library & Framework Requirements

- Keep workspace-pinned dependencies for compatibility:
  - `axum = 0.8.8`
  - `sqlx = 0.8.6`
  - `tokio = 1.48.0`
  - `time = 0.3.44`
  - `polymarket-client-sdk = 0.4.4`
- Latest checks at story creation time:
  - `axum` latest stable remains `0.8.8`,
  - `sqlx` latest indexed is `0.9.0-alpha.1` (pre-release), so stable workspace `0.8.6` remains the target,
  - `tokio` latest stable is `1.51.0`,
  - `time` latest stable is `0.3.47`,
  - `polymarket-client-sdk` latest stable remains `0.4.4`.
- Do not perform opportunistic dependency upgrades in Story 6.3.

[Source: Cargo.toml]  
[Source: source "$HOME/.cargo/env" && cargo search axum --limit 1]  
[Source: source "$HOME/.cargo/env" && cargo search sqlx --limit 1]  
[Source: source "$HOME/.cargo/env" && cargo search tokio --limit 1]  
[Source: source "$HOME/.cargo/env" && cargo search time --limit 1]  
[Source: source "$HOME/.cargo/env" && cargo search polymarket-client-sdk --limit 1]

### File Structure Requirements

- Primary implementation surfaces for Story 6.3:
  - `crates/domain/src/{lib.rs,research.rs}`
  - `crates/persistence/migrations/*validation_runs*.sql`
  - `crates/persistence/migrations/*validation_artifacts*.sql`
  - `crates/persistence/src/postgres/{mod.rs,validation_runs.rs,validation_artifacts.rs}`
  - `services/research-gateway/src/{lib.rs,validation/mod.rs,validation/gate_policies.rs,validation/workflow_runs.rs,promotion/mod.rs}`
  - `services/control-api/src/{main.rs,middleware/mod.rs,routes/mod.rs}`
  - `docs/operations/alpha-validation-workflow-and-diagnostics.md`
  - `tests/api/story-6-3*.test.mjs`
  - `tests/e2e/story-6-3*.test.mjs`
  - `package.json`
  - `_bmad-output/implementation-artifacts/tests/test-summary.md`
- Reuse established vertical-slice cadence:
  - domain contracts -> migration -> persistence adapters -> research orchestration -> control-api routes/state wiring -> tests -> runbook.
- Keep schema/story boundaries strict: only `validation_runs` and `validation_artifacts` in this story.

[Source: _bmad-output/planning-artifacts/architecture.md#Complete Project Directory Structure]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: services/control-api/src/main.rs]  
[Source: services/control-api/src/middleware/mod.rs]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: services/research-gateway/src/lib.rs]  
[Source: services/research-gateway/src/validation/mod.rs]  
[Source: services/research-gateway/src/promotion/mod.rs]  
[Source: crates/persistence/src/postgres/mod.rs]

### Testing Requirements

- Add deterministic coverage for:
  - workflow stage ordering and blocked downstream stage execution semantics,
  - FR43 gate precheck integration at validation entry,
  - diagnostics metric payload validation and comparison ordering across runs,
  - explicit route envelope/status mapping for run/artifact operations,
  - telemetry/audit continuity for allow and deny outcomes.
- Keep test layering aligned with repository conventions:
  - domain contract tests in `crates/domain`,
  - migration/adapter tests in `crates/persistence`,
  - orchestration tests in `services/research-gateway`,
  - route tests in `services/control-api`,
  - story-scoped API/E2E tests in `tests/api` + `tests/e2e`.
- Add story QA command:
  - `qa:test:story-6-3` in root `package.json`.

[Source: package.json]  
[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: tests/api/story-6-2-validation-gate-policies-api.test.mjs]  
[Source: tests/e2e/story-6-2-validation-gate-policies.e2e.test.mjs]

### Previous Story Intelligence

- Story 6.2 already established the FR43 enforcement seam:
  - `evaluate_training_entry_gates(...)` in `services/research-gateway/src/validation/mod.rs`,
  - fail-closed reason-code taxonomy for unresolved policy, dependency-unavailable, and state-unavailable conditions.
- Reuse, do not reinvent:
  - existing research route family under `/control/research/...`,
  - canonical response/error mapping helpers in `services/control-api/src/routes/mod.rs`,
  - operation-lock and telemetry patterns in `validation/gate_policies.rs` and `validation/hypothesis_registry.rs`,
  - canonical migration constraints and persistence classification patterns from `alpha_hypotheses` + `validation_gate_policies`.
- Preserve role-boundary continuity:
  - mutation flows require privileged roles,
  - read surfaces can allow `read_only_analytics` where route semantics are read-only.

[Source: _bmad-output/implementation-artifacts/stories/6-2-configure-leakage-and-data-quality-gate-definitions.md]  
[Source: _bmad-output/implementation-artifacts/stories/6-1-build-alpha-hypothesis-registry-with-required-metadata.md]  
[Source: services/research-gateway/src/validation/mod.rs]  
[Source: services/research-gateway/src/validation/gate_policies.rs]  
[Source: services/research-gateway/src/validation/hypothesis_registry.rs]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: docs/operations/alpha-validation-gate-policies.md]

### Git Intelligence Summary

- Recent commit cadence remains consistent and should be reused for Story 6.3:
  1. domain contracts + reason-code taxonomy,  
  2. forward-only migration + persistence adapters,  
  3. service orchestration with fail-closed dependency semantics,  
  4. authenticated control-plane route/state integration + audit continuity,  
  5. story-scoped QA scripts/tests and operations runbook updates.
- Recent file-touch patterns confirm Story 6.x implementation is centered in:
  - `crates/domain`,
  - `crates/persistence`,
  - `services/research-gateway`,
  - `services/control-api`,
  - `tests/api` and `tests/e2e`,
  - `docs/operations`.

[Source: git --no-pager log --oneline -5]  
[Source: git --no-pager log -5 --name-only --pretty=format:'%h %s']

### Latest Technical Information

- Workspace stack is sufficient for Story 6.3 scope; no mandatory upgrades are required.
- `sqlx` latest indexed version is pre-release (`0.9.0-alpha.1`), so stable workspace `0.8.6` should remain in use.
- `research-gateway` runtime binary remains scaffold-focused while orchestration logic is library-first; Story 6.3 should extend existing library seams and keep runtime wiring minimal.

[Source: Cargo.toml]  
[Source: services/research-gateway/src/main.rs]  
[Source: services/research-gateway/src/lib.rs]  
[Source: source "$HOME/.cargo/env" && cargo search sqlx --limit 1]

### Project Context Reference

- No `project-context.md` file was found during discovery.
- Story context is derived from epics, PRD, architecture, UX, research artifacts, prior Story 6.1/6.2 implementation details, and recent git history.

### Project Structure Notes

- Current Story 6.3 seams in repository:
  - `research-gateway` already supports FR6 and FR43 orchestration, including stage-entry gate checks and fail-closed telemetry patterns.
  - `control-api` state already wires `research_hypothesis_orchestrator` and `research_validation_gate_orchestrator`; Story 6.3 should extend this pattern for validation-run orchestration.
  - `persistence` currently contains only `alpha_hypotheses` and `validation_gate_policies` for Epic 6, so Story 6.3 must introduce run/artifact storage without schema creep.
- **Blocking design constraint documented:** there is no existing in-repo computation engine for quality/labeling/purged-CV/CPCV/overfit execution.
  - Story 6.3 should define an explicit stage-executor port and fail closed when executors/dependencies are unavailable,
  - keep contracts deterministic so Stories 6.4/6.5 can consume validation evidence without rework.
- No blocking issues prevent create-story output; scope is implementation-ready with explicit boundaries and deferred-seam guidance.

[Source: services/research-gateway/src/main.rs]  
[Source: services/research-gateway/src/lib.rs]  
[Source: services/research-gateway/src/validation/mod.rs]  
[Source: services/research-gateway/src/promotion/mod.rs]  
[Source: services/control-api/src/middleware/mod.rs]  
[Source: crates/persistence/src/postgres/mod.rs]  
[Source: _bmad-output/implementation-artifacts/deferred-work.md]

### References

- _bmad-output/planning-artifacts/epics.md#Epic 6: Research-to-Production Alpha Governance Lifecycle  
- _bmad-output/planning-artifacts/epics.md#Story 6.3: Implement Validation Workflow and Diagnostics Artifact Store  
- _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)  
- _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)  
- _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)  
- _bmad-output/planning-artifacts/prd.md#Journey 6 — Research User (Phase 2+): Noor, Quant Research Lead  
- _bmad-output/planning-artifacts/prd.md#Strategy Research & Alpha Lifecycle  
- _bmad-output/planning-artifacts/prd.md#Model Integrity & Promotion Governance  
- _bmad-output/planning-artifacts/prd.md#Observability & Operability  
- _bmad-output/planning-artifacts/prd.md#Compliance & Auditability  
- _bmad-output/planning-artifacts/architecture.md#Data Architecture  
- _bmad-output/planning-artifacts/architecture.md#API & Communication Patterns  
- _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules  
- _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries  
- _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping  
- _bmad-output/planning-artifacts/ux-design-specification.md#Target Users  
- _bmad-output/planning-artifacts/ux-design-specification.md#Custom Components  
- _bmad-output/planning-artifacts/research/domain-polymarket-rust-clob-profit-research-2026-04-04.md#Phase 5 - Validation & Overfitting Defense  
- _bmad-output/planning-artifacts/research/market-rust-polymarket-trading-system-lopez-de-prado-research-2026-04-04.md#Lopez de Prado Method Mapping (Practical Set)  
- _bmad-output/planning-artifacts/research/technical-polymarket-claim-validity-audit-research-2026-04-04.md#Practical extraction for your stack  
- _bmad-output/implementation-artifacts/stories/6-1-build-alpha-hypothesis-registry-with-required-metadata.md  
- _bmad-output/implementation-artifacts/stories/6-2-configure-leakage-and-data-quality-gate-definitions.md  
- _bmad-output/implementation-artifacts/deferred-work.md  
- docs/operations/alpha-hypothesis-registry.md  
- docs/operations/alpha-validation-gate-policies.md  
- services/control-api/src/{main.rs,middleware/mod.rs,routes/mod.rs}  
- services/research-gateway/src/{lib.rs,main.rs,validation/mod.rs,validation/gate_policies.rs,validation/hypothesis_registry.rs,promotion/mod.rs}  
- crates/domain/src/research.rs  
- crates/persistence/src/postgres/{mod.rs,alpha_hypotheses.rs,validation_gate_policies.rs}  
- crates/persistence/migrations/{20260407160000_alpha_hypotheses.sql,20260407173000_validation_gate_policies.sql}  
- tests/api/story-6-2-validation-gate-policies-api.test.mjs  
- tests/e2e/story-6-2-validation-gate-policies.e2e.test.mjs  
- Cargo.toml  
- package.json  
- git --no-pager log --oneline -5  
- git --no-pager log -5 --name-only --pretty=format:'%h %s'  
- source "$HOME/.cargo/env" && cargo search axum --limit 1  
- source "$HOME/.cargo/env" && cargo search sqlx --limit 1  
- source "$HOME/.cargo/env" && cargo search tokio --limit 1  
- source "$HOME/.cargo/env" && cargo search time --limit 1  
- source "$HOME/.cargo/env" && cargo search polymarket-client-sdk --limit 1

## Story Completion Status

- Story 6.3 implementation and adversarial code review are complete.
- High/medium review findings were auto-fixed; verification suites for Story 6.3 and repository tests pass.

## Dev Agent Record

### Agent Model Used

GPT-5.3-Codex (gpt-5.3-codex)

### Debug Log References

- Story 6.3 implementation and verification executed in non-interactive dev-story mode.
- `source "$HOME/.cargo/env" && npm run --silent qa:test:story-6-3`
- `source "$HOME/.cargo/env" && npm run --silent qa:test:story-6-3` *(2026-04-07 19:46 QA automation refresh after Story 6.3 API/E2E test expansion)*
- `source "$HOME/.cargo/env" && npm test`
- `source "$HOME/.cargo/env" && cargo fmt --all`
- `source "$HOME/.cargo/env" && npm run --silent rust:fmt`
- `source "$HOME/.cargo/env" && npm run --silent rust:lint` *(fails on pre-existing, out-of-scope clippy findings in `crates/domain/src/recovery.rs`)*

### Completion Notes List

- Added FR7/FR44 domain contracts for validation workflow stages, run/artifact records, diagnostics payloads, deterministic comparison deltas, and validation-run reason-code taxonomy.
- Added forward-only migration plus persistence adapters for `validation_runs` and `validation_artifacts` with deterministic ordering and machine-readable error classification.
- Implemented research-gateway validation workflow orchestration with FR43 gate precheck reuse, explicit stage executor seam, fail-closed progression, artifact persistence, and prior-run comparisons.
- Added authenticated control-api `/control/research/validation-runs...` start/read/list/artifact surfaces with canonical `data/meta/error` envelopes, deterministic `400/403/409/503/500` mapping, and audit continuity.
- Added Story 6.3 API/E2E static contract tests, `qa:test:story-6-3` script wiring, and FR7/FR44 operations runbook documentation.
- Code review patch: fixed historical run comparison baseline selection to use only strictly earlier completed runs and added targeted regression coverage.
- QA automation refresh expanded Story 6.3 API/E2E assertions for deterministic artifact/comparison contracts, allow/deny audit evidence continuity, FR43 precheck fail-closed progression, and deterministic comparison baseline/timing semantics.

### File List

- _bmad-output/implementation-artifacts/stories/6-3-implement-validation-workflow-and-diagnostics-artifact-store.md
- _bmad-output/implementation-artifacts/sprint-status.yaml
- _bmad-output/implementation-artifacts/deferred-work.md
- _bmad-output/implementation-artifacts/tests/test-summary.md
- Cargo.lock
- package.json
- crates/domain/src/research.rs
- crates/persistence/migrations/20260407193000_validation_runs_validation_artifacts.sql
- crates/persistence/src/postgres/mod.rs
- crates/persistence/src/postgres/validation_runs.rs
- crates/persistence/src/postgres/validation_artifacts.rs
- services/research-gateway/Cargo.toml
- services/research-gateway/src/validation/mod.rs
- services/research-gateway/src/validation/workflow_runs.rs
- services/control-api/src/main.rs
- services/control-api/src/middleware/mod.rs
- services/control-api/src/routes/mod.rs
- docs/operations/alpha-validation-workflow-and-diagnostics.md
- tests/api/story-6-3-validation-workflow-api.test.mjs
- tests/e2e/story-6-3-validation-workflow.e2e.test.mjs

### Change Log

- 2026-04-07: Created Story 6.3 ready-for-dev context via automated create-story workflow execution.
- 2026-04-07: Implemented Story 6.3 validation workflow orchestration, persistence, control-api surfaces, Story QA automation, and FR7/FR44 operations runbook; moved story status to review.
- 2026-04-07: Completed adversarial code review auto-fix pass; corrected historical comparison baseline logic, added regression test, and moved story status to done.
- 2026-04-07: Executed Story 6.3 QA automation refresh, expanded API/E2E critical-flow assertions, re-ran `qa:test:story-6-3`, and retained story status as done.
