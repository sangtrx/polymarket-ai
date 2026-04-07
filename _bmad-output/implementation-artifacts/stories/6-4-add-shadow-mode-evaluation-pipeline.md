# Story 6.4: Add Shadow-Mode Evaluation Pipeline

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an operator,  
I want candidates evaluated in read-only mode before capital deployment,  
so that live risk is reduced while signal quality is observed.

## Acceptance Criteria

1. **Scenario A - FR9 baseline (story-local BDD):**  
   **Given** a validated candidate enters shadow mode  
   **When** simulated decisions run against live market context  
   **Then** simulated outcomes are recorded without live order placement  
   **And** evaluation state is queryable for FR9.

2. **Scenario B - validated-candidate entry contract:**  
   **Given** a candidate shadow-evaluation request  
   **When** prechecks execute  
   **Then** the candidate must reference a completed Story 6.3 validation run with queryable diagnostics evidence  
   **And** requests with missing/ineligible validation evidence are denied fail-closed.

3. **Scenario C - strict read-only execution guarantee:**  
   **Given** shadow evaluation is running  
   **When** simulated decisions are generated  
   **Then** no live order placement/cancel pathways are invoked  
   **And** output records include explicit simulation reason codes proving read-only behavior.

4. **Scenario D - deterministic simulation outcome contract:**  
   **Given** live market context and candidate signal decisions  
   **When** simulated execution outcomes are computed  
   **Then** persisted results include deterministic market/signal/outcome payloads (decision side, intended size, simulated fill/price/slippage, and timestamp metadata)  
   **And** payload shape remains machine-readable and stable for downstream consumers.

5. **Scenario E - persistence scope and schema isolation:**  
   **Given** a shadow evaluation is accepted  
   **When** persistence succeeds  
   **Then** evidence is stored only in `shadow_evaluations` with actor/correlation/timestamp fields  
   **And** no unrelated schema entities are introduced in this story.

6. **Scenario F - authenticated control-plane start/read/list surfaces:**  
   **Given** authorized users trigger or inspect shadow evaluations  
   **When** control-plane routes execute  
   **Then** responses follow canonical `data/meta/error` envelope conventions  
   **And** unauthorized/malformed/unavailable states map deterministically to explicit status codes.

7. **Scenario G - deterministic boundary behavior:**  
   **Given** window/query boundaries and candidate identifiers  
   **When** list/read operations execute  
   **Then** canonical identifier normalization and ordering behavior are deterministic and test-covered  
   **And** boundary conditions (for example invalid limit values or malformed UTC timestamps) fail with explicit field-level diagnostics.

8. **Scenario H - unavailable dependency fail-closed behavior:**  
   **Given** required market-context, simulation engine, validation-evidence, or persistence dependencies are unavailable/ambiguous  
   **When** orchestration executes  
   **Then** requests fail closed with explicit `*_dependency_unavailable`, `*_state_unavailable`, or `*_persistence_unavailable` style reason codes  
   **And** no success-shaped fallback path is emitted.

9. **Scenario I - NFR14 observability continuity:**  
   **Given** allow and deny outcomes for shadow start/read/list flows  
   **When** telemetry emits  
   **Then** logs/metrics/traces include actor, action, candidate, evaluation id, reason code, correlation id, and UTC timestamp  
   **And** evidence is queryable for incident and QA workflows.

10. **Scenario J - downstream lifecycle compatibility:**  
    **Given** Story 6.5 promotion-gating and UX governance-card consumers  
    **When** they query shadow-evaluation state  
    **Then** they can consume stable machine-readable evaluation status and confidence/outcome payloads  
    **And** Story 6.4 does not pre-implement promotion-threshold decisions or lifecycle actions.

11. **UAC-1 Failure handling:** Invalid payloads, unauthorized access, and unavailable dependency paths return explicit machine-readable errors with no unsafe side effects.

12. **UAC-2 Boundary behavior:** Identifier/query/time and list-order boundaries are deterministic (inclusive/exclusive rules documented and test-covered).

13. **UAC-3 Verifiable evidence:** Successful and failed shadow-evaluation operations emit timestamped telemetry/audit evidence suitable for incident and QA traceability.

14. **Schema/dependency/traceability contract:** Story depends on `6.3`; schema scope introduces only `shadow_evaluations`; traceability maps to `FR9` and `NFR14`.

15. **Scope boundary contract:** Story 6.4 delivers read-only shadow-evaluation execution and queryability only; it must not pre-implement Story 6.5 promotion-threshold approvals/evidence gating or Stories 6.6-6.9 replay/health/deallocation/UX lifecycle automation.

## Tasks / Subtasks

- [x] **Task 1: Define FR9 shadow-evaluation domain contracts and reason-code taxonomy** (AC: 1, 2, 3, 7, 8, 11, 12, 14)
  - [x] Extend `crates/domain/src/research.rs` with shadow-evaluation contracts (record shapes, evaluation state enums, reason-code taxonomy, and deterministic serialization/parsing helpers).
  - [x] Add canonical normalization and payload validation helpers for candidate/evaluation identifiers and UTC timestamp fields.
  - [x] Define explicit machine-readable failure families (invalid payload, unauthorized role, dependency unavailable, state unavailable, persistence unavailable).
  - [x] Preserve fail-closed semantics for all uncertain evaluation conditions.

- [x] **Task 2: Add forward-only migration and persistence adapter for `shadow_evaluations`** (AC: 1, 4, 5, 7, 8, 14)
  - [x] Add migration under `crates/persistence/migrations/` creating only `shadow_evaluations` with canonical constraints, deterministic indexes, and UTC evidence metadata.
  - [x] Add persistence module (recommended: `crates/persistence/src/postgres/shadow_evaluations.rs`) and wire through `crates/persistence/src/postgres/mod.rs`.
  - [x] Implement deterministic upsert/read/list operations keyed by canonical identifiers with explicit query/constraint/decode error classification.
  - [x] Add persistence tests validating schema isolation, ordering guarantees, and machine-readable failure mapping.

- [x] **Task 3: Implement research-gateway shadow-mode orchestration** (AC: 1, 2, 3, 4, 7, 8, 9, 10, 12)
  - [x] Add orchestration module in `services/research-gateway/src/validation/` (recommended: `shadow_mode.rs`) and export via `validation/mod.rs`.
  - [x] Reuse Story 6.3 validation-run/artifact seams to enforce validated-candidate entry requirements before shadow execution.
  - [x] Introduce explicit ports for live-market context loading and simulated execution computation; fail closed when either dependency is unavailable.
  - [x] Persist evaluation records and emit structured telemetry for allow/deny outcomes with actor/candidate/evaluation/reason/correlation/timestamp continuity.

- [x] **Task 4: Expose authenticated control-plane shadow-evaluation routes and state wiring** (AC: 3, 6, 8, 9, 11, 14)
  - [x] Add authenticated routes in `services/control-api/src/routes/mod.rs` (recommended resource family: `/control/research/shadow-evaluations...`) for start/read/list.
  - [x] Reuse canonical `data/meta/error` response envelope and deterministic status mapping (`400/403/409/503/500`) consistent with existing research routes.
  - [x] Wire shadow-evaluation orchestrator into `services/control-api/src/middleware/mod.rs` and startup wiring in `services/control-api/src/main.rs`.
  - [x] Append allow/deny audit records with action, parameters, reason code, correlation id, and UTC timestamps.

- [x] **Task 5: Enforce strict read-only and no-reinvention guardrails** (AC: 2, 3, 8, 10, 15)
  - [x] Explicitly guard against any execution-engine live order mutation path from shadow workflow codepaths.
  - [x] Reuse Story 6.3 route/error/telemetry patterns instead of creating parallel conventions.
  - [x] Document deferred integration seams so Story 6.5 promotion checks consume Story 6.4 outputs without contract forks.

- [x] **Task 6: Add deterministic Story 6.4 automated coverage and QA command wiring** (AC: 1-15)
  - [x] Add domain tests for reason-code parsing, normalization, and payload boundary validation.
  - [x] Add persistence tests for `shadow_evaluations` constraints, canonical reads, and deterministic list ordering.
  - [x] Add research-gateway tests for validated-candidate gate checks, read-only simulation guarantees, and dependency-unavailable fail-closed behavior.
  - [x] Add control-api route tests for authenticated start/read/list behavior, unauthorized-role denial, malformed payload handling, and status mapping.
  - [x] Add story-scoped API/E2E tests (`tests/api/story-6-4*.test.mjs`, `tests/e2e/story-6-4*.test.mjs`) and wire `qa:test:story-6-4` in root `package.json`.

- [x] **Task 7: Publish FR9 operations guidance and traceability artifacts** (AC: 6, 9, 10, 13, 14)
  - [x] Add `docs/operations/alpha-shadow-mode-evaluation.md` documenting start/read/list contracts, read-only guarantees, simulation payload semantics, and fail-closed playbooks.
  - [x] Cross-link with `alpha-validation-workflow-and-diagnostics.md` and `alpha-validation-gate-policies.md` to preserve Epic 6 operator/research continuity.
  - [x] Update `_bmad-output/implementation-artifacts/tests/test-summary.md` with Story 6.4 evidence after implementation.

### Review Findings

- [x] [Review][Patch] Preserve effective request correlation id in shadow-evaluation error envelopes/audit metadata for start/read/list failures (`services/control-api/src/routes/mod.rs`).
- [x] [Review][Patch] Canonicalize list response `candidate_id` to enforce deterministic identifier normalization at query boundaries (`services/control-api/src/routes/mod.rs`).
- [x] [Review][Patch] Emit deny telemetry when list timestamp boundary parsing fails (`started_after_utc` / `started_before_utc`) to preserve NFR14 failure evidence continuity (`services/research-gateway/src/validation/shadow_mode.rs`).

## Dev Notes

### Technical Requirements

- Story objective is FR9 enforceability: evaluate candidate alphas in read-only mode and persist simulated decisions/outcomes without live order placement.
- Story-level dependency/scope contract:
  - dependency: `6.3`,
  - schema scope: `shadow_evaluations` only,
  - traceability: `FR9`, `NFR14`.
- Mandatory entry precondition: candidate must have eligible Story 6.3 validation evidence available for shadow-evaluation initiation.
- Read-only guarantee is non-negotiable:
  - no live submit/cancel pathways,
  - no capital allocation side effects,
  - explicit machine-readable proof of simulated-only execution outcomes.
- Out of scope for Story 6.4:
  - promotion thresholds/evidence packet enforcement (Story 6.5),
  - counterfactual stress replay (Story 6.6),
  - live-health threshold detection and auto-deallocation policy actions (Stories 6.7 and 6.9),
  - governance readiness card UX implementation (Story 6.8).

[Source: _bmad-output/planning-artifacts/epics.md#Story 6.4: Add Shadow-Mode Evaluation Pipeline]  
[Source: _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)]  
[Source: _bmad-output/planning-artifacts/prd.md#Strategy Research & Alpha Lifecycle]  
[Source: _bmad-output/planning-artifacts/prd.md#Model Integrity & Promotion Governance]  
[Source: _bmad-output/planning-artifacts/prd.md#Observability & Operability]

### Architecture Compliance

- Preserve bounded architecture ownership:
  - `control-api` remains authenticated ingress with canonical envelope/audit behavior,
  - `research-gateway` owns shadow-evaluation orchestration logic,
  - `crates/persistence` owns deterministic storage/query semantics.
- Follow architecture conventions exactly:
  - plural kebab-case route resources,
  - snake_case Rust modules/files and DB identifiers,
  - UTC ISO-8601 timestamps,
  - explicit machine-readable reason-code and status mapping.
- Apply fail-closed process guardrails:
  - distinguish domain validation vs dependency/state vs persistence failures,
  - no swallowed errors or broad success defaults,
  - unresolved market-context/simulation dependencies deny progression.
- NFR14 obligations are explicit for shadow flows:
  - structured logs/metrics/traces with correlation continuity across start/read/list operations.

[Source: _bmad-output/planning-artifacts/architecture.md#Data Architecture]  
[Source: _bmad-output/planning-artifacts/architecture.md#API & Communication Patterns]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: _bmad-output/planning-artifacts/architecture.md#File Organization Patterns]

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
- Do not perform opportunistic dependency upgrades in Story 6.4.

[Source: Cargo.toml]  
[Source: source "$HOME/.cargo/env" && cargo search axum --limit 1]  
[Source: source "$HOME/.cargo/env" && cargo search sqlx --limit 1]  
[Source: source "$HOME/.cargo/env" && cargo search tokio --limit 1]  
[Source: source "$HOME/.cargo/env" && cargo search time --limit 1]  
[Source: source "$HOME/.cargo/env" && cargo search polymarket-client-sdk --limit 1]

### File Structure Requirements

- Primary implementation surfaces for Story 6.4:
  - `crates/domain/src/{lib.rs,research.rs}`
  - `crates/persistence/migrations/*shadow_evaluations*.sql`
  - `crates/persistence/src/postgres/{mod.rs,shadow_evaluations.rs}`
  - `services/research-gateway/src/{lib.rs,validation/mod.rs,validation/workflow_runs.rs,validation/shadow_mode.rs,promotion/mod.rs}`
  - `services/control-api/src/{main.rs,middleware/mod.rs,routes/mod.rs}`
  - `docs/operations/alpha-shadow-mode-evaluation.md`
  - `tests/api/story-6-4*.test.mjs`
  - `tests/e2e/story-6-4*.test.mjs`
  - `package.json`
  - `_bmad-output/implementation-artifacts/tests/test-summary.md`
- Reuse established vertical-slice cadence:
  - domain contracts -> migration -> persistence adapter -> research orchestration -> control-api routes/state wiring -> tests -> runbook.
- Keep schema/story boundaries strict: only `shadow_evaluations` in this story.

[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: services/research-gateway/src/validation/mod.rs]  
[Source: services/research-gateway/src/validation/workflow_runs.rs]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: services/control-api/src/middleware/mod.rs]  
[Source: crates/persistence/migrations/20260407193000_validation_runs_validation_artifacts.sql]  
[Source: crates/persistence/src/postgres/mod.rs]

### Testing Requirements

- Add deterministic coverage for:
  - validated-candidate entry gate enforcement from Story 6.3 evidence,
  - strict read-only execution behavior (no live-order mutation side effects),
  - simulation payload validation/boundary handling,
  - deterministic start/read/list ordering and pagination boundaries,
  - canonical route envelope/error mapping for shadow-evaluation endpoints,
  - telemetry evidence continuity for allow and deny outcomes.
- Keep test layering aligned with repository conventions:
  - domain contract tests in `crates/domain`,
  - migration/adapter tests in `crates/persistence`,
  - orchestration tests in `services/research-gateway`,
  - route tests in `services/control-api`,
  - story-scoped API/E2E tests in `tests/api` + `tests/e2e`.
- Add story QA command:
  - `qa:test:story-6-4` in root `package.json`.

[Source: package.json]  
[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: _bmad-output/implementation-artifacts/stories/6-3-implement-validation-workflow-and-diagnostics-artifact-store.md#Testing Requirements]

### Previous Story Intelligence

- Story 6.3 already establishes reusable FR7/FR44 validation seams:
  - `validation_runs` and `validation_artifacts` persistence contracts,
  - `ValidationWorkflowRunOrchestrator` with deterministic stage progression and fail-closed reason-code semantics,
  - control-plane routes under `/control/research/validation-runs...` with canonical envelope mapping.
- Reuse, do not reinvent:
  - validation run/artifact query contracts as upstream evidence for shadow entry,
  - reason-code and telemetry naming patterns from Story 6.3 workflow service,
  - existing control-api authorization and error-mapping helpers in `routes/mod.rs`.
- Preserve continuity for downstream stories:
  - Story 6.4 outputs should be promotion-ready inputs for Story 6.5 without changing Story 6.3 contracts.

[Source: _bmad-output/implementation-artifacts/stories/6-3-implement-validation-workflow-and-diagnostics-artifact-store.md]  
[Source: docs/operations/alpha-validation-workflow-and-diagnostics.md]  
[Source: services/research-gateway/src/validation/workflow_runs.rs]  
[Source: services/control-api/src/routes/mod.rs]

### Git Intelligence Summary

- Recent commit cadence in Epic 6 remains consistent and should be reused for Story 6.4:
  1. domain contracts + reason-code taxonomy,  
  2. forward-only migration + persistence adapter(s),  
  3. service orchestration with fail-closed dependency handling,  
  4. authenticated control-plane route/state integration,  
  5. story-scoped QA/test and runbook updates.
- Most recent commits are Story 6.3, 6.2, and 6.1 deliveries, confirming continuity of the research-gateway/control-api/persistence vertical slice.

[Source: git --no-pager log --oneline -5]

### Latest Technical Information

- Workspace dependency set remains sufficient for Story 6.4 scope; no mandatory upgrades are required.
- `sqlx` latest indexed result remains pre-release (`0.9.0-alpha.1`), so stable workspace `0.8.6` should remain in use.
- Existing Story 6.3 contracts already provide deterministic validation evidence primitives for shadow-mode reuse; Story 6.4 should extend these seams rather than re-architecting stack components.

[Source: Cargo.toml]  
[Source: source "$HOME/.cargo/env" && cargo search sqlx --limit 1]  
[Source: _bmad-output/implementation-artifacts/stories/6-3-implement-validation-workflow-and-diagnostics-artifact-store.md]

### Project Context Reference

- No `project-context.md` file was found during discovery.
- Story context is derived from epics, PRD, architecture, UX, research artifacts, prior Story 6.x implementation details, operations runbooks, and recent git history.

### Project Structure Notes

- Current Story 6.4 seams in repository:
  - `research-gateway` currently has hypothesis, gate-policy, and validation-run orchestration but no shadow-evaluation module yet.
  - `control-api` already wires `research_validation_workflow_orchestrator` and route families for FR6/FR43/FR7 read/write contracts.
  - `persistence` already follows per-story module + migration patterns with canonical constraint/index conventions.
- **Blocking design constraint documented:** no dedicated in-repo live-market-context adapter exists inside `research-gateway` today.
  - Story 6.4 should define explicit market-context/simulation ports and fail closed when dependencies are unavailable,
  - avoid adding direct live-order execution coupling to shadow workflow paths.
- No blocking issues prevent create-story output; scope is implementation-ready with explicit boundaries and dependency guardrails.

[Source: services/research-gateway/src/lib.rs]  
[Source: services/research-gateway/src/validation/mod.rs]  
[Source: services/research-gateway/src/promotion/mod.rs]  
[Source: services/control-api/src/main.rs]  
[Source: services/control-api/src/middleware/mod.rs]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: crates/persistence/src/postgres/mod.rs]

### References

- _bmad-output/planning-artifacts/epics.md#Epic 6: Research-to-Production Alpha Governance Lifecycle  
- _bmad-output/planning-artifacts/epics.md#Story 6.4: Add Shadow-Mode Evaluation Pipeline  
- _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)  
- _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)  
- _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)  
- _bmad-output/planning-artifacts/prd.md#Strategy Research & Alpha Lifecycle  
- _bmad-output/planning-artifacts/prd.md#Model Integrity & Promotion Governance  
- _bmad-output/planning-artifacts/prd.md#Journey 6 — Research User (Phase 2+): Noor, Quant Research Lead  
- _bmad-output/planning-artifacts/prd.md#Observability & Operability  
- _bmad-output/planning-artifacts/architecture.md#Data Architecture  
- _bmad-output/planning-artifacts/architecture.md#API & Communication Patterns  
- _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping  
- _bmad-output/planning-artifacts/ux-design-specification.md#Target Users  
- _bmad-output/planning-artifacts/ux-design-specification.md#2.4 Novel UX Patterns  
- _bmad-output/planning-artifacts/ux-design-specification.md#Alpha Governance Card  
- _bmad-output/planning-artifacts/research/domain-polymarket-rust-clob-profit-research-2026-04-04.md#Phase 6 — Paper-to-Production Rollout  
- _bmad-output/planning-artifacts/research/market-rust-polymarket-trading-system-lopez-de-prado-research-2026-04-04.md#D — Delivery, deployment, and iteration  
- _bmad-output/planning-artifacts/research/technical-polymarket-claim-validity-audit-research-2026-04-04.md#Practical extraction for your stack  
- _bmad-output/implementation-artifacts/stories/6-1-build-alpha-hypothesis-registry-with-required-metadata.md  
- _bmad-output/implementation-artifacts/stories/6-2-configure-leakage-and-data-quality-gate-definitions.md  
- _bmad-output/implementation-artifacts/stories/6-3-implement-validation-workflow-and-diagnostics-artifact-store.md  
- docs/operations/alpha-validation-gate-policies.md  
- docs/operations/alpha-validation-workflow-and-diagnostics.md  
- services/research-gateway/src/{lib.rs,validation/mod.rs,validation/workflow_runs.rs,promotion/mod.rs}  
- services/control-api/src/{main.rs,middleware/mod.rs,routes/mod.rs}  
- crates/domain/src/research.rs  
- crates/persistence/src/postgres/{mod.rs,validation_runs.rs,validation_artifacts.rs}  
- crates/persistence/migrations/20260407193000_validation_runs_validation_artifacts.sql  
- Cargo.toml  
- package.json  
- git --no-pager log --oneline -5  
- source "$HOME/.cargo/env" && cargo search axum --limit 1  
- source "$HOME/.cargo/env" && cargo search sqlx --limit 1  
- source "$HOME/.cargo/env" && cargo search tokio --limit 1  
- source "$HOME/.cargo/env" && cargo search time --limit 1  
- source "$HOME/.cargo/env" && cargo search polymarket-client-sdk --limit 1

## Story Completion Status

- Story 6.4 implementation and code review are complete.
- Story-scoped QA gate and full repository regression gate passed (`qa:test:story-6-4`, `npm test`), and a follow-up QA automation rerun with expanded API/E2E critical-flow coverage also passed.

## Dev Agent Record

### Agent Model Used

GPT-5.3-Codex (gpt-5.3-codex)

### Debug Log References

- `source "$HOME/.cargo/env" && cargo test -p domain research::tests::shadow_evaluation_`
- `source "$HOME/.cargo/env" && cargo test -p persistence postgres::shadow_evaluations::tests::`
- `source "$HOME/.cargo/env" && cargo test -p research-gateway validation::shadow_mode::tests::`
- `source "$HOME/.cargo/env" && cargo test -p control-api routes::tests::shadow_evaluation_`
- `source "$HOME/.cargo/env" && npm run --silent qa:test:story-6-4`
- `source "$HOME/.cargo/env" && npm test`
- `source "$HOME/.cargo/env" && npm run --silent qa:test:story-6-4` (2026-04-07 21:15:05 QA automation rerun after API/E2E critical-flow expansion)

### Completion Notes List

- Implemented Story 6.4 shadow-evaluation contracts end-to-end across domain, persistence, research-gateway orchestration, and control-api start/read/list endpoints.
- Added deterministic story-scoped automation (`qa:test:story-6-4`) plus route/API/E2E checks for envelope contracts, status mapping, wiring, and read-only evidence guarantees.
- Expanded Story 6.4 API/E2E automation to cover list-route canonical candidate normalization, effective-correlation-id continuity in deny envelopes/audit records, malformed list-window deny telemetry evidence, and deterministic list limit/repository-call behavior.
- Published FR9 operations runbook and updated traceability artifacts with Story 6.4 coverage and regression evidence.
- Applied code-review patches for correlation-id continuity, canonical list candidate identifiers, and telemetry continuity on malformed list timestamp boundaries.
- Discrepancy note: `.scripts/bmad-auto/copilot/bmad-progress.log` was modified in git status but is automation metadata outside Story 6.4 application-source review scope.

### File List

- crates/domain/src/research.rs
- crates/persistence/migrations/20260407210000_shadow_evaluations.sql
- crates/persistence/src/postgres/mod.rs
- crates/persistence/src/postgres/shadow_evaluations.rs
- services/research-gateway/src/validation/mod.rs
- services/research-gateway/src/validation/shadow_mode.rs
- services/control-api/src/main.rs
- services/control-api/src/middleware/mod.rs
- services/control-api/src/routes/mod.rs
- docs/operations/alpha-shadow-mode-evaluation.md
- tests/api/story-6-4-shadow-mode-evaluation-api.test.mjs
- tests/e2e/story-6-4-shadow-mode-evaluation.e2e.test.mjs
- package.json
- _bmad-output/implementation-artifacts/tests/test-summary.md
- _bmad-output/implementation-artifacts/stories/6-4-add-shadow-mode-evaluation-pipeline.md
- _bmad-output/implementation-artifacts/sprint-status.yaml

### Change Log

- 2026-04-07: Created Story 6.4 ready-for-dev context via automated create-story workflow execution.
- 2026-04-07: Implemented Story 6.4 shadow-mode evaluation pipeline, added FR9 runbook + QA automation, and moved status to review after passing `qa:test:story-6-4` and `npm test`.
- 2026-04-07: Completed adversarial code review, auto-fixed medium findings in `routes/mod.rs` and `shadow_mode.rs`, re-ran Story 6.4 + full regression tests, and moved status to `done`.
- 2026-04-07: Executed `bmad-qa-generate-e2e-tests` workflow rerun for Story 6.4, expanded API/E2E critical-flow assertions, re-ran `qa:test:story-6-4`, and kept story/sprint status at `done`.
