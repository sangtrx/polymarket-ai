# Story 6.2: Configure Leakage and Data-Quality Gate Definitions

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As a research lead,  
I want mandatory leakage and data-quality gates,  
so that invalid training/promotion paths are blocked early.

## Acceptance Criteria

1. **Scenario A - Story-local BDD baseline:**  
   **Given** gate policies are configured  
   **When** a candidate enters training or promotion workflow  
   **Then** forward-bias, leakage, and survivability checks are enforced before progression  
   **And** gate configuration satisfies FR43.

2. **Scenario B - mandatory gate catalog contract:**  
   **Given** an FR43 policy definition request  
   **When** validation executes  
   **Then** canonical mandatory checks are enforced for `forward_bias`, `data_leakage`, and `regime_survivability`  
   **And** data-quality gate definitions are required for workflow-stage enforcement.

3. **Scenario C - stage-scoped policy definition semantics:**  
   **Given** gate policies are configured for candidate lifecycle stages  
   **When** configuration is accepted  
   **Then** each gate declares deterministic stage applicability (`training`, `promotion`, or `training_and_promotion`)  
   **And** policy payloads include explicit fail criteria (threshold/operator semantics) with machine-readable diagnostics.

4. **Scenario D - deterministic boundary behavior:**  
   **Given** gate thresholds and comparators are configured  
   **When** measured values are exactly at boundary conditions  
   **Then** inclusive/exclusive semantics are applied deterministically  
   **And** boundary behavior is documented and test-covered.

5. **Scenario E - persistence scope and schema isolation:**  
   **Given** a valid gate-policy mutation  
   **When** persistence succeeds  
   **Then** evidence is stored only in `validation_gate_policies` with actor/correlation/timestamp metadata  
   **And** no unrelated schema entities are introduced in this story.

6. **Scenario F - authenticated control-plane configuration/read surfaces:**  
   **Given** authorized users mutate or read gate policies  
   **When** control-plane routes execute  
   **Then** responses follow canonical machine-readable envelope/error conventions  
   **And** unauthorized or malformed requests are denied explicitly with deterministic status mapping.

7. **Scenario G - enforcement seam before workflow progression:**  
   **Given** a candidate enters training or promotion gating entrypoints  
   **When** gate evaluation runs  
   **Then** missing/failed mandatory gates block progression fail-closed  
   **And** deny payloads include failed gate identifiers and machine-readable reason codes.

8. **Scenario H - unavailable dependency fail-closed behavior:**  
   **Given** required gate inputs or policy state are unavailable/ambiguous  
   **When** evaluation executes  
   **Then** workflow progression is denied with explicit `*_state_unavailable`/`*_dependency_unavailable` style machine-readable reason codes  
   **And** no success-shaped fallback path is emitted.

9. **Scenario I - NFR17 auditability and telemetry evidence:**  
   **Given** allow and deny outcomes for gate policy mutation/read/evaluation  
   **When** telemetry/audit emit  
   **Then** records include actor, action, parameters, reason code, correlation id, and UTC timestamp  
   **And** evidence remains queryable for incident/QA workflows.

10. **UAC-1 Failure handling:** Invalid payloads, unauthorized access, unavailable dependency paths, and unresolved policy references return explicit machine-readable errors with no unsafe side effects.

11. **UAC-2 Boundary behavior:** Threshold, comparator, and stage-scope boundaries are deterministic (inclusive/exclusive semantics documented and test-covered).

12. **UAC-3 Verifiable evidence:** Successful and failed critical operations emit timestamped telemetry/audit evidence suitable for incident and QA traceability.

13. **Schema/dependency/traceability contract:** Story depends on `6.1`; schema scope introduces only `validation_gate_policies`; traceability maps to `FR43` and `NFR17`.

14. **Scope boundary contract:** Story 6.2 delivers gate-definition configuration and enforcement seams only; it must not pre-implement Story 6.3 validation-run artifacts, Story 6.5 promotion-threshold workflows, or Stories 6.6-6.9 lifecycle automation/UX flows.

## Tasks / Subtasks

- [x] **Task 1: Define FR43 domain contracts, gate taxonomy, and reason-code semantics** (AC: 1, 2, 3, 4, 8, 10, 11, 13)
  - [x] Extend `crates/domain/src/research.rs` with canonical gate-definition contracts (gate type, stage scope, comparator semantics, mandatory flags, and evaluation outcomes).
  - [x] Add deterministic machine-readable reason-code taxonomy for invalid payloads, unauthorized role, unresolved policy, gate failure, and dependency-unavailable states.
  - [x] Implement canonical normalization/validation helpers for identifiers and threshold payloads (including explicit boundary semantics).
  - [x] Keep parsing/validation fail-closed with field-level diagnostics (no broad acceptance/default-pass behavior).

- [x] **Task 2: Add forward-only migration and persistence adapter for `validation_gate_policies`** (AC: 3, 4, 5, 8, 10, 11, 13)
  - [x] Add migration under `crates/persistence/migrations/` creating only `validation_gate_policies` with canonical identifier constraints, stage applicability fields, threshold payload, actor/correlation evidence metadata, and UTC constraints.
  - [x] Add persistence adapter module (recommended: `crates/persistence/src/postgres/validation_gate_policies.rs`) and wire through `crates/persistence/src/postgres/mod.rs`.
  - [x] Implement deterministic upsert/read/list operations with explicit query/constraint/decode error classification.
  - [x] Add persistence tests verifying schema-scope isolation, canonical constraints, deterministic ordering, and machine-readable error mapping.

- [x] **Task 3: Implement research-gateway FR43 orchestration for gate-policy lifecycle** (AC: 2, 3, 4, 7, 8, 9, 10, 12, 13)
  - [x] Add validation-gate orchestration module in `services/research-gateway/src/validation/` (recommended: `gate_policies.rs`) and export via `validation/mod.rs`.
  - [x] Define input/evidence contracts for policy mutation/read and stage-aware candidate gate evaluation.
  - [x] Reuse operation-lock and fail-closed dependency handling patterns established in `validation/hypothesis_registry.rs`.
  - [x] Emit structured telemetry for allow/deny outcomes with deterministic reason-code continuity.

- [x] **Task 4: Wire enforcement seams for training/promotion entry checks** (AC: 1, 3, 7, 8, 11, 14)
  - [x] Add explicit pre-progression gate-evaluation seam callable by training/promotion workflows (current story can wire minimal entrypoints in `validation` and `promotion` modules without implementing full Story 6.3/6.5 workflows).
  - [x] Ensure missing policy state or unavailable gate inputs deny progression fail-closed.
  - [x] Document deferred integration points so Story 6.3 (validation runs) and Story 6.5 (promotion actions) consume the same evaluator contract without breaking changes.

- [x] **Task 5: Expose authenticated control-plane FR43 configuration/evaluation routes** (AC: 6, 7, 8, 9, 10, 12, 13)
  - [x] Add authenticated routes in `services/control-api/src/routes/mod.rs` for gate-policy mutation/read and evaluation (recommended resource family: `/control/research/validation-gate-policies/...`).
  - [x] Add payload/query/response structs using canonical `data/meta/error` conventions and deterministic machine-readable status mapping (`400/403/409/503/500`).
  - [x] Wire orchestration into `services/control-api/src/middleware/mod.rs` and `services/control-api/src/main.rs` following existing Story 6.1 state-builder patterns.
  - [x] Append privileged audit records for both allow and deny paths with actor/action/parameters/reason/correlation/timestamp continuity.

- [x] **Task 6: Add deterministic Story 6.2 automated coverage and QA command wiring** (AC: 1-14)
  - [x] Add domain tests for gate taxonomy parsing, stage applicability validation, threshold boundary semantics, and fail-closed reason-code behavior.
  - [x] Add persistence tests for `validation_gate_policies` constraints, deterministic reads, and conflict/error mapping.
  - [x] Add research-gateway tests for mutation/read/evaluation success and deny paths (including dependency-unavailable and unresolved policy states).
  - [x] Add control-api route tests for authenticated mutation/read/evaluation flows, unauthorized-role denial, malformed payload handling, and status-code mapping.
  - [x] Add story-scoped API/E2E tests (`tests/api/story-6-2*.test.mjs`, `tests/e2e/story-6-2*.test.mjs`) and wire `qa:test:story-6-2` in root `package.json`.

- [x] **Task 7: Publish FR43 operations guidance and traceability artifacts** (AC: 9, 12, 13)
  - [x] Add `docs/operations/alpha-validation-gate-policies.md` documenting gate catalog semantics, stage enforcement behavior, failure-mode playbooks, and audit verification queries.
  - [x] Cross-link FR43 runbook with `alpha-hypothesis-registry`, `reward-risk-policy-operations`, `risk-limit-policy-operations`, and report/export governance workflows.
  - [x] Update `_bmad-output/implementation-artifacts/tests/test-summary.md` with Story 6.2 evidence after implementation.

### Review Findings

- [x] [Review][Patch] Reject malformed observed metric payload values as invalid input [services/research-gateway/src/validation/gate_policies.rs:1305]
- [x] [Review][Patch] Enforce `data_quality` mandatory contract consistently across domain validation and persistence schema [crates/domain/src/research.rs:675]
- [x] [Review][Patch] Map validation-gate JSON extraction rejections to canonical `validation_gate_invalid_payload` envelopes for upsert/evaluate routes [services/control-api/src/routes/mod.rs:1237]
- [x] [Review][Patch] Reject duplicate canonical observed-metric keys to prevent silent overwrite of gate evidence [services/research-gateway/src/validation/gate_policies.rs:1305]
- [x] [Review][Patch] Include missing mandatory gate identifiers in deny payload `failed_gate_ids` for fail-closed diagnostics [services/research-gateway/src/validation/gate_policies.rs:93]
- [x] [Review][Patch] Reclassify persisted enum token corruption as row-decode/persistence-unavailable instead of client invalid-payload [crates/persistence/src/postgres/validation_gate_policies.rs:225]
- [x] [Review][Defer] Git diff contains non-story automation log drift [.scripts/bmad-auto/copilot/bmad-progress.log] — deferred, out-of-scope artifact.
- [x] [Review][Defer] Repository-wide `TIMESTAMPTZ` timezone-offset CHECK pattern needs platform-level migration policy alignment; changing only Story 6.2 would create inconsistent schema conventions.

## Dev Notes

### Technical Requirements

- Story objective is FR43 enforceability: research users can configure mandatory leakage and data-quality gates that block invalid training/promotion progression.
- Story-level dependency/scope contract:
  - dependency: `6.1`,
  - schema scope: `validation_gate_policies` only,
  - traceability: `FR43`, `NFR17`.
- Mandatory FR43 gate families are non-negotiable:
  - leakage checks: `forward_bias`, `data_leakage`, `regime_survivability`,
  - data-quality gates: stage-scoped quality criteria required before progression.
- Enforcement contract must be fail-closed:
  - unresolved/missing gate policies or unavailable gate-input dependencies deny progression explicitly,
  - no implicit pass/default-open behavior.
- Out of scope for Story 6.2:
  - full validation-run execution/artifact lifecycle (Story 6.3 / FR7, FR44),
  - promotion thresholds/evidence packet governance (Story 6.5 / FR8, FR11, FR45),
  - replay/health/deallocation UX and lifecycle automation (Stories 6.6-6.9).

[Source: _bmad-output/planning-artifacts/epics.md#Story 6.2: Configure Leakage and Data-Quality Gate Definitions]  
[Source: _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)]  
[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: _bmad-output/planning-artifacts/prd.md#Model Integrity & Promotion Governance]  
[Source: _bmad-output/planning-artifacts/prd.md#Strategy Research & Alpha Lifecycle]  
[Source: _bmad-output/planning-artifacts/prd.md#Journey 6 — Research User (Phase 2+): Noor, Quant Research Lead]  
[Source: _bmad-output/planning-artifacts/prd.md#Compliance & Auditability]

### Architecture Compliance

- Preserve bounded ownership:
  - `control-api` remains authenticated ingress and envelope/audit surface,
  - `research-gateway` owns FR43 gate-policy mutation/read/evaluation orchestration,
  - `governance-service` remains source of privileged audit/approval foundations.
- Follow architecture conventions exactly:
  - plural kebab-case route resources,
  - snake_case Rust modules/files/DB identifiers,
  - ISO-8601 UTC timestamps only,
  - canonical machine-readable reason-code taxonomy with explicit status mapping.
- Apply process-pattern guardrails:
  - separate domain validation, dependency-unavailable, and persistence failures,
  - no swallowed errors in control or validation pathways,
  - uncertain state must fail closed before training/promotion progression.

[Source: _bmad-output/planning-artifacts/architecture.md#API & Communication Patterns]  
[Source: _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules]  
[Source: _bmad-output/planning-artifacts/architecture.md#Architectural Boundaries]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: services/research-gateway/src/validation/hypothesis_registry.rs]

### Library & Framework Requirements

- Keep workspace-pinned dependencies for compatibility:
  - `axum = 0.8.8`
  - `sqlx = 0.8.6`
  - `tokio = 1.48.0`
  - `time = 0.3.44`
  - `polymarket-client-sdk = 0.4.4`
- Latest checks at story creation time:
  - `axum` latest stable remains `0.8.8`,
  - `sqlx` latest indexed is `0.9.0-alpha.1` (pre-release), stable target remains `0.8.6`,
  - `tokio` latest stable is `1.51.0`,
  - `time` latest stable is `0.3.47`,
  - `polymarket-client-sdk` latest stable remains `0.4.4`.
- Do not perform opportunistic dependency upgrades in Story 6.2.

[Source: Cargo.toml]  
[Source: services/research-gateway/Cargo.toml]  
[Source: source $HOME/.cargo/env && cargo search axum --limit 1]  
[Source: source $HOME/.cargo/env && cargo search sqlx --limit 1]  
[Source: source $HOME/.cargo/env && cargo search tokio --limit 1]  
[Source: source $HOME/.cargo/env && cargo search time --limit 1]  
[Source: source $HOME/.cargo/env && cargo search polymarket-client-sdk --limit 1]

### File Structure Requirements

- Primary implementation surfaces for Story 6.2:
  - `crates/domain/src/{lib.rs,research.rs}`
  - `crates/persistence/migrations/*validation_gate_policies*.sql`
  - `crates/persistence/src/postgres/{mod.rs,validation_gate_policies.rs}`
  - `services/research-gateway/src/{lib.rs,validation/mod.rs,validation/gate_policies.rs,promotion/mod.rs}`
  - `services/control-api/src/{main.rs,middleware/mod.rs,routes/mod.rs}`
  - `docs/operations/alpha-validation-gate-policies.md`
  - `tests/api/story-6-2*.test.mjs`
  - `tests/e2e/story-6-2*.test.mjs`
  - `package.json`
  - `_bmad-output/implementation-artifacts/tests/test-summary.md`
- Reuse established vertical-slice cadence from recent stories:
  - domain contracts -> migration -> persistence adapter -> research orchestration -> control-api routes/state wiring -> tests -> runbook.
- Keep schema/story boundaries strict: only `validation_gate_policies` in this story.

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
  - mandatory gate-catalog and stage-applicability validation,
  - threshold comparator boundary behavior (including equality),
  - fail-closed unresolved-policy and dependency-unavailable paths,
  - machine-readable route status mapping and role-based authorization boundaries,
  - telemetry/audit evidence continuity for allow and deny outcomes.
- Keep test layering aligned with repository conventions:
  - domain contract tests in `crates/domain`,
  - migration/adapter tests in `crates/persistence`,
  - orchestration tests in `services/research-gateway`,
  - route tests in `services/control-api`,
  - story-scoped API/E2E tests in `tests/api` + `tests/e2e`.
- Add story QA command:
  - `qa:test:story-6-2` in root `package.json`.

[Source: package.json]  
[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: _bmad-output/implementation-artifacts/stories/6-1-build-alpha-hypothesis-registry-with-required-metadata.md#Testing Requirements]

### Previous Story Intelligence

- Story 6.1 established the FR6 baseline and introduces a deliberate seam for future FR43/FR7 evolution:
  - `DatasetSnapshotRegistryPort` currently backed by static env-based source (`RESEARCH_DATASET_SNAPSHOT_VERSIONS`) in `StaticDatasetSnapshotRegistry`,
  - explicit note that Stories 6.2/6.3 should evolve this assumption without breaking route contracts.
- Reuse, do not reinvent:
  - machine-readable error/status/audit response helpers already implemented in `control-api` (`alpha_hypothesis_response` + `alpha_hypothesis_service_error_response` patterns),
  - operation-lock + fail-closed dependency handling already implemented in `research-gateway` hypothesis orchestration,
  - deterministic normalization and UTC validation helpers in `domain::research`.
- Preserve role boundary continuity:
  - mutation flows use privileged roles,
  - read flows can permit `read_only_analytics` when route semantics are read-only.

[Source: _bmad-output/implementation-artifacts/stories/6-1-build-alpha-hypothesis-registry-with-required-metadata.md]  
[Source: services/research-gateway/src/validation/hypothesis_registry.rs]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: docs/operations/alpha-hypothesis-registry.md]  
[Source: docs/governance/rbac-role-model.md]

### Git Intelligence Summary

- Recent commit cadence is consistent and should be reused for Story 6.2:
  1. domain contracts + reason-code taxonomy,  
  2. forward-only migration + persistence adapter,  
  3. service orchestration with fail-closed dependency semantics,  
  4. authenticated control-plane route/state integration + audit continuity,  
  5. story-scoped QA scripts/tests and operations runbook updates.
- Recent files touched confirm this vertical-slice pattern across Stories 5.1-5.4 and 6.1.

[Source: git --no-pager log --oneline -5]  
[Source: git --no-pager log -5 --name-only --pretty=format:'%h %s']

### Latest Technical Information

- Workspace stack remains current and sufficient for Story 6.2 scope; no mandatory upgrades are required.
- `sqlx` latest indexed version is pre-release (`0.9.0-alpha.1`), so stable workspace `0.8.6` should remain in use for this story.
- `research-gateway` remains a thin scaffold plus `validation/hypothesis_registry` implementation; Story 6.2 should extend this surface incrementally without introducing unrelated stack churn.

[Source: Cargo.toml]  
[Source: services/research-gateway/Cargo.toml]  
[Source: services/research-gateway/src/main.rs]  
[Source: source $HOME/.cargo/env && cargo search axum --limit 1]  
[Source: source $HOME/.cargo/env && cargo search sqlx --limit 1]  
[Source: source $HOME/.cargo/env && cargo search tokio --limit 1]  
[Source: source $HOME/.cargo/env && cargo search time --limit 1]  
[Source: source $HOME/.cargo/env && cargo search polymarket-client-sdk --limit 1]

### Project Context Reference

- No `project-context.md` file was found during discovery.
- Story context is derived from epics/PRD/architecture/UX artifacts, implementation-readiness output, research artifacts, previous Story 6.1 implementation details, and recent git history.

### Project Structure Notes

- Current FR43-relevant seams in repository:
  - `research-gateway` validation currently implements only hypothesis registry orchestration,
  - `promotion` module is a placeholder and should only receive minimal pre-progression seam wiring in this story,
  - `control-api` already contains robust authenticated route/audit/error patterns to reuse.
- Design constraint to document explicitly:
  - no dedicated `research_*` RBAC role exists yet in canonical role model,
  - Story 6.2 should reuse current canonical roles (`operational_control` / `administrative_actions` for mutation; read-only role allowance only where route semantics are read-only) until role model evolution is explicitly planned.
- No blocking issues found for create-story output; scope is implementation-ready with clear boundaries.

[Source: services/research-gateway/src/validation/mod.rs]  
[Source: services/research-gateway/src/promotion/mod.rs]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: services/control-api/src/middleware/mod.rs]  
[Source: docs/governance/rbac-role-model.md]  
[Source: _bmad-output/planning-artifacts/implementation-readiness-report-2026-04-05.md]

### References

- _bmad-output/planning-artifacts/epics.md#Epic 6: Research-to-Production Alpha Governance Lifecycle  
- _bmad-output/planning-artifacts/epics.md#Story 6.2: Configure Leakage and Data-Quality Gate Definitions  
- _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)  
- _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)  
- _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)  
- _bmad-output/planning-artifacts/prd.md#Strategy Research & Alpha Lifecycle  
- _bmad-output/planning-artifacts/prd.md#Model Integrity & Promotion Governance  
- _bmad-output/planning-artifacts/prd.md#Journey 6 — Research User (Phase 2+): Noor, Quant Research Lead  
- _bmad-output/planning-artifacts/prd.md#Compliance & Auditability  
- _bmad-output/planning-artifacts/architecture.md#Data Architecture  
- _bmad-output/planning-artifacts/architecture.md#API & Communication Patterns  
- _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules  
- _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries  
- _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping  
- _bmad-output/planning-artifacts/ux-design-specification.md#Journey 3 — Alpha Promotion Governance  
- _bmad-output/planning-artifacts/implementation-readiness-report-2026-04-05.md  
- _bmad-output/planning-artifacts/research/domain-polymarket-rust-clob-profit-research-2026-04-04.md#Phase 5 — Validation & Overfitting Defense  
- _bmad-output/planning-artifacts/research/market-rust-polymarket-trading-system-lopez-de-prado-research-2026-04-04.md#López de Prado Method Mapping (Practical Set)  
- _bmad-output/planning-artifacts/research/technical-polymarket-claim-validity-audit-research-2026-04-04.md#Applicability to Your Polymarket System  
- _bmad-output/implementation-artifacts/stories/6-1-build-alpha-hypothesis-registry-with-required-metadata.md  
- docs/operations/alpha-hypothesis-registry.md  
- docs/governance/rbac-role-model.md  
- services/control-api/src/{main.rs,middleware/mod.rs,routes/mod.rs}  
- services/research-gateway/src/{main.rs,lib.rs,validation/mod.rs,validation/hypothesis_registry.rs,promotion/mod.rs}  
- crates/domain/src/{lib.rs,research.rs}  
- crates/persistence/src/postgres/{mod.rs,alpha_hypotheses.rs}  
- crates/persistence/migrations/20260407160000_alpha_hypotheses.sql  
- Cargo.toml  
- package.json  
- git --no-pager log --oneline -5  
- git --no-pager log -5 --name-only --pretty=format:'%h %s'  
- source $HOME/.cargo/env && cargo search axum --limit 1  
- source $HOME/.cargo/env && cargo search sqlx --limit 1  
- source $HOME/.cargo/env && cargo search tokio --limit 1  
- source $HOME/.cargo/env && cargo search time --limit 1  
- source $HOME/.cargo/env && cargo search polymarket-client-sdk --limit 1

## Story Completion Status

- Story 6.2 context is created and ready for development execution.
- Ultimate context engine analysis completed - comprehensive developer guide created.

## Dev Agent Record

### Agent Model Used

GPT-5.3-Codex (gpt-5.3-codex)

### Debug Log References

- BMAD create-story workflow execution (automated, non-interactive).

### Completion Notes List

- Story context compiled from epics, PRD, architecture, UX, research artifacts, previous Story 6.1 implementation, and recent git history.
- Story scoped with strict schema/dependency boundaries and explicit fail-closed guardrails for FR43 enforcement.
- Implemented FR43 domain contracts in `domain::research` for gate taxonomy, stage scope semantics, comparator boundaries, reason-code taxonomy, canonical validation, and fail-closed diagnostics.
- Added forward-only `validation_gate_policies` migration and persistence adapter with deterministic upsert/read/list behavior plus explicit constraint/query/decode error classification.
- Implemented research-gateway `validation::gate_policies` orchestration with role-gated mutation/read/list/evaluate paths, fail-closed mandatory-catalog enforcement, dependency/state-unavailable denial handling, and telemetry continuity.
- Added training/promotion pre-progression evaluator seam functions in `validation/mod.rs` and `promotion/mod.rs` to preserve Story 6.3/6.5 integration boundaries.
- Added authenticated control-api routes for validation-gate policy mutate/read/list/evaluate with canonical `data/meta/error` envelopes, deterministic machine-readable status mapping (`400/403/409/503/500`), and privileged audit record continuity.
- Added Story 6.2 QA coverage across domain, persistence, research-gateway, control-api route tests, and story-scoped API/E2E contract tests; wired root command `qa:test:story-6-2`.
- Published FR43 operations runbook and cross-runbook links; appended Story 6.2 evidence to `_bmad-output/implementation-artifacts/tests/test-summary.md`.
- Code review hardening: malformed metric payloads now fail with `validation_gate_invalid_payload`; duplicate canonical metric aliases are rejected; missing-mandatory denials now expose failed gate identifiers.
- Code review hardening: validation-gate route JSON rejections now emit canonical `data/meta/error` envelopes, persisted contract-token corruption is treated as persistence-unavailable, and deny-path unit coverage was expanded (`policy_unresolved` / `state_unavailable`).
- QA automation refresh (2026-04-07 18:16:21): expanded Story 6.2 API/E2E contract assertions for stage-filter query semantics, failed-gate diagnostic propagation, unauthorized security-signal tokens, deterministic comparator boundary documentation, and fail-closed no-fallback runbook posture; reran `source "$HOME/.cargo/env" && npm run --silent qa:test:story-6-2` with all Story 6.2 checks passing.

### File List

- _bmad-output/implementation-artifacts/sprint-status.yaml
- _bmad-output/implementation-artifacts/deferred-work.md
- _bmad-output/implementation-artifacts/stories/6-2-configure-leakage-and-data-quality-gate-definitions.md
- _bmad-output/implementation-artifacts/tests/test-summary.md
- crates/domain/src/research.rs
- crates/persistence/migrations/20260407173000_validation_gate_policies.sql
- crates/persistence/src/postgres/mod.rs
- crates/persistence/src/postgres/validation_gate_policies.rs
- docs/operations/alpha-hypothesis-registry.md
- docs/operations/alpha-validation-gate-policies.md
- docs/operations/report-export-workflows.md
- docs/operations/reward-risk-policy-operations.md
- docs/operations/risk-limit-policy-operations.md
- package.json
- services/control-api/src/main.rs
- services/control-api/src/middleware/mod.rs
- services/control-api/src/routes/mod.rs
- services/research-gateway/src/promotion/mod.rs
- services/research-gateway/src/validation/gate_policies.rs
- services/research-gateway/src/validation/mod.rs
- tests/api/story-6-2-validation-gate-policies-api.test.mjs
- tests/e2e/story-6-2-validation-gate-policies.e2e.test.mjs

### Change Log

- 2026-04-07: Implemented Story 6.2 FR43 validation-gate policy contracts, persistence, orchestration, control-plane routes, QA automation, runbook artifacts, and sprint/test evidence updates.
- 2026-04-07: Code review remediation fixed malformed metric payload classification, mandatory `data_quality` contract consistency, and canonical route-envelope handling for JSON extraction failures.
- 2026-04-07: Added edge-case remediations for duplicate canonical metric keys, missing mandatory gate identifier propagation, persisted token corruption classification, and expanded deny-path test coverage.
- 2026-04-07: QA automation refresh expanded Story 6.2 API/E2E contract assertions (stage-filter query, failed-gate diagnostics, unauthorized security-signal tokens, and deterministic boundary/no-fallback runbook coverage) and reran story QA command to green.
