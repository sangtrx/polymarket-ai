# Story 6.1: Build Alpha Hypothesis Registry with Required Metadata

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As a research user,  
I want to register alpha hypotheses with complete metadata,  
so that each candidate has traceable provenance and assumptions.

## Acceptance Criteria

1. **Scenario A - FR6 baseline (story-local BDD):**  
   **Given** a new alpha hypothesis submission  
   **When** required fields are validated  
   **Then** incomplete submissions are rejected and complete submissions are stored with dataset linkage  
   **And** behavior fulfills FR6.

2. **Scenario B - required metadata completeness contract:**  
   **Given** a registration payload  
   **When** domain validation executes  
   **Then** all required FR6 metadata fields are enforced (`hypothesis_id`, `feature_set_version`, `target_regime`, `expected_edge_source`, `training_window`, `risk_assumptions`)  
   **And** rejected payloads return field-level machine-readable diagnostics.

3. **Scenario C - dataset snapshot linkage gate:**  
   **Given** a `feature_set_version` that does not resolve to a registered dataset snapshot reference  
   **When** registration is requested  
   **Then** the request is denied fail-closed with explicit machine-readable reason code  
   **And** no `alpha_hypotheses` record is written.

4. **Scenario D - canonicalization and deterministic boundary behavior:**  
   **Given** metadata values with casing/whitespace variance and training-window edge cases  
   **When** normalization and validation run  
   **Then** canonical identifier normalization is deterministic (`trim + lowercase` for identifiers)  
   **And** boundary behavior is deterministic and test-covered (e.g., `training_window_start_utc < training_window_end_utc`; equality is rejected).

5. **Scenario E - persistence scope and idempotent registry semantics:**  
   **Given** a valid registration request  
   **When** persistence succeeds  
   **Then** records are persisted only in `alpha_hypotheses` with actor/correlation/timestamp evidence  
   **And** repeat submissions for the same `hypothesis_id` follow deterministic idempotent/update semantics with no duplicate active-row ambiguity.

6. **Scenario F - authenticated control-plane register/read surfaces:**  
   **Given** an authorized actor performs register or read operations  
   **When** `POST`/`GET` control-plane routes execute  
   **Then** responses follow canonical machine-readable envelope/error conventions  
   **And** unauthorized/malformed requests are denied explicitly with deterministic status mapping.

7. **Scenario G - NFR17 auditability and telemetry evidence:**  
   **Given** accepted and denied registration/read operations  
   **When** audit and telemetry emit  
   **Then** evidence includes actor, action, parameters, reason_code, correlation_id, and UTC timestamp  
   **And** resulting evidence remains queryable for audit/incident workflows.

8. **UAC-1 Failure handling:** Invalid payloads, unauthorized access, unavailable dependency paths, and unresolved dataset-snapshot references return explicit machine-readable errors with no unsafe side effects.

9. **UAC-2 Boundary behavior:** Identifier normalization, training-window boundaries, and duplicate/idempotent registration semantics are deterministic and test-covered.

10. **UAC-3 Verifiable evidence:** Successful and failed alpha-hypothesis registration/read operations emit timestamped telemetry/audit evidence suitable for incident and QA traceability.

11. **Schema/dependency/traceability contract:** Story depends on `1.4`; schema scope introduces only `alpha_hypotheses`; traceability maps to `FR6` and `NFR17`.

12. **Scope boundary contract:** Story 6.1 delivers alpha hypothesis registry + metadata validation + dataset-linkage gating only; it must not pre-implement Stories 6.2-6.9 validation/promotion/shadow/health/deallocation features.

## Tasks / Subtasks

- [x] **Task 1: Define canonical alpha-hypothesis domain contracts and validation rules** (AC: 1, 2, 3, 4, 8, 9, 11)
  - [x] Add domain contracts for alpha-hypothesis registration (recommended new module `crates/domain/src/research.rs`, exported via `crates/domain/src/lib.rs`) including required FR6 metadata fields.
  - [x] Add machine-readable reason-code taxonomy and validation issue types for invalid payloads, unregistered dataset references, unauthorized role, and persistence-unavailable states.
  - [x] Implement deterministic normalization/validation helpers for identifiers, timestamp fields, and training-window boundaries (`start < end`).
  - [x] Ensure risk-assumptions payload validation is explicit (no broad/implicit acceptance of malformed assumptions).

- [x] **Task 2: Add forward-only migration and persistence adapter for `alpha_hypotheses`** (AC: 1, 3, 4, 5, 8, 9, 11)
  - [x] Add migration under `crates/persistence/migrations/` creating only `alpha_hypotheses` with canonical constraints, actor/correlation evidence metadata, UTC timestamp constraints, and deterministic lookup indexes.
  - [x] Add persistence adapter module (for example `crates/persistence/src/postgres/alpha_hypotheses.rs`) and wire through `crates/persistence/src/postgres/mod.rs`.
  - [x] Implement deterministic upsert/read behavior keyed by canonical `hypothesis_id` with explicit query/constraint/decode error mapping.
  - [x] Add persistence tests validating schema-scope isolation, canonical constraints, deterministic query ordering, and machine-readable persistence failure mapping.

- [x] **Task 3: Implement research-gateway validation orchestration for hypothesis registry** (AC: 1, 2, 3, 4, 7, 8, 11)
  - [x] Add `services/research-gateway/src/lib.rs` export surface and implement validation orchestration in `services/research-gateway/src/validation/` (for example `hypothesis_registry.rs` + `mod.rs` wiring).
  - [x] Define `UpsertAlphaHypothesisInput` / `ReadAlphaHypothesisInput` and evidence contracts aligned with existing service-orchestrator patterns.
  - [x] Enforce authorized-role checks, deterministic normalization, operation-lock safety, and fail-closed behavior for unresolved dependencies.
  - [x] Emit structured telemetry events for allow/deny outcomes with reason code, actor, hypothesis_id, correlation, and timestamp evidence.

- [x] **Task 4: Expose authenticated control-plane FR6 routes and state wiring** (AC: 2, 6, 7, 8, 10, 11)
  - [x] Add `research-gateway` as a dependency in `services/control-api/Cargo.toml` and wire a research orchestrator into `services/control-api/src/main.rs` + `services/control-api/src/middleware/mod.rs` state builder patterns.
  - [x] Add authenticated routes in `services/control-api/src/routes/mod.rs` using canonical plural kebab-case resource naming (recommended: `POST/GET /control/research/alpha-hypotheses/{hypothesis_id}`).
  - [x] Reuse canonical `data/meta/error` response envelope and explicit machine-readable status mapping (`400/403/409/503/500`) consistent with existing story patterns.
  - [x] Append privileged audit records for both success and denial paths with actor/action/parameters/reason/correlation/timestamp evidence continuity.

- [x] **Task 5: Implement dataset-snapshot reference verification seam without schema creep** (AC: 3, 4, 5, 8, 9, 11, 12)
  - [x] Introduce a `DatasetSnapshotRegistryPort` seam used by hypothesis validation/orchestration to verify `feature_set_version` registration status.
  - [x] Keep Story 6.1 schema scope limited to `alpha_hypotheses`; do not add extra snapshot-registry tables in this story.
  - [x] Ensure unresolved/unavailable dataset-snapshot checks deny fail-closed with deterministic machine-readable reason codes.
  - [x] Document the current registry-source assumption explicitly in code and runbook so Story 6.2/6.3 can evolve it without contract ambiguity.

- [x] **Task 6: Add deterministic Story 6.1 automated coverage and QA command wiring** (AC: 1-12)
  - [x] Add domain tests for required-field validation, timestamp/training-window boundaries, canonical normalization, and dataset-reference failure paths.
  - [x] Add persistence tests for `alpha_hypotheses` constraints, idempotent upsert semantics, and deterministic lookup ordering.
  - [x] Add research-gateway tests for authorization handling, allow/deny telemetry emission, and fail-closed dependency-unavailable behavior.
  - [x] Add control-api route tests for authenticated register/read success, field-level validation errors, unauthorized-role denial, and persistence/dataset-registry unavailable mapping.
  - [x] Add story-scoped API/E2E tests (`tests/api/story-6-1*.test.mjs`, `tests/e2e/story-6-1*.test.mjs`) and wire `qa:test:story-6-1` in root `package.json`.

- [x] **Task 7: Publish FR6 operations guidance and traceability artifacts** (AC: 7, 10, 11)
  - [x] Add `docs/operations/alpha-hypothesis-registry.md` covering registration contract, boundary rules, failure-mode playbooks, and audit verification queries.
  - [x] Cross-link relevant runbooks (report-export, risk-limit/reward-risk, and future governance lifecycle docs) for operator/research continuity.
  - [x] Update `_bmad-output/implementation-artifacts/tests/test-summary.md` with Story 6.1 QA evidence after implementation.

## Dev Notes

### Technical Requirements

- Story objective is FR6 enforceability for alpha-hypothesis registration with complete, traceable metadata and explicit dataset linkage validation.
- Story-level dependency/scope contract is fixed by epics index:
  - dependency: `1.4` (immutable privileged audit baseline),
  - schema scope: `alpha_hypotheses` only,
  - traceability: `FR6`, `NFR17`.
- Required FR6 metadata fields are non-negotiable:
  - `hypothesis_id`, `feature_set_version`, `target_regime`, `expected_edge_source`, `training_window`, `risk_assumptions`.
- Acceptance gate must be fail-closed:
  - incomplete/invalid payloads and unresolved dataset-snapshot references are denied with machine-readable codes,
  - no silent fallback and no success-shaped default when dependencies are uncertain.
- Out of scope for Story 6.1:
  - leakage/data-quality gate configuration (Story 6.2 / FR43),
  - validation artifact workflows (Story 6.3 / FR7, FR44),
  - shadow/promotion/deallocation behavior (Stories 6.4-6.9).

[Source: _bmad-output/planning-artifacts/epics.md#Story 6.1: Build Alpha Hypothesis Registry with Required Metadata]  
[Source: _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)]  
[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: _bmad-output/planning-artifacts/prd.md#Strategy Research & Alpha Lifecycle]  
[Source: _bmad-output/planning-artifacts/prd.md#Journey 6 — Research User (Phase 2+): Noor, Quant Research Lead]

### Architecture Compliance

- Preserve bounded architecture ownership:
  - `control-api` remains authenticated mutation/read ingress with canonical envelope/audit behavior,
  - `research-gateway` owns FR6 research hypothesis orchestration seams,
  - `governance-service` remains owner of privileged audit/approval foundations.
- Follow architecture conventions exactly:
  - plural kebab-case route paths,
  - snake_case Rust modules/files and DB identifiers,
  - ISO-8601 UTC timestamps only,
  - machine-readable reason/error taxonomy with explicit status mapping.
- Enforce process-pattern guardrails:
  - distinguish domain vs dependency/persistence failures,
  - no swallowed errors,
  - uncertain dependency state must fail closed.

[Source: _bmad-output/planning-artifacts/architecture.md#API & Communication Patterns]  
[Source: _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules]  
[Source: _bmad-output/planning-artifacts/architecture.md#Architectural Boundaries]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: services/governance-service/src/market_policy/mod.rs]  
[Source: services/governance-service/src/reward_risk/mod.rs]  
[Source: services/research-gateway/src/main.rs]

### Library & Framework Requirements

- Keep workspace-pinned dependencies for compatibility:
  - `axum = 0.8.8`
  - `sqlx = 0.8.6`
  - `tokio = 1.48.0`
  - `time = 0.3.44`
  - `polymarket-client-sdk = 0.4.4`
- Latest checks at story creation time:
  - `axum` latest stable remains `0.8.8`,
  - `sqlx` latest indexed is `0.9.0-alpha.1` (pre-release), stable workspace target remains `0.8.6`,
  - `tokio` latest stable is `1.51.0`,
  - `time` latest stable is `0.3.47`,
  - `polymarket-client-sdk` latest stable remains `0.4.4`.
- Do not perform opportunistic dependency upgrades in Story 6.1.

[Source: Cargo.toml]  
[Source: source $HOME/.cargo/env && cargo search axum --limit 1]  
[Source: source $HOME/.cargo/env && cargo search sqlx --limit 1]  
[Source: source $HOME/.cargo/env && cargo search tokio --limit 1]  
[Source: source $HOME/.cargo/env && cargo search time --limit 1]  
[Source: source $HOME/.cargo/env && cargo search polymarket-client-sdk --limit 1]

### File Structure Requirements

- Primary implementation surfaces for Story 6.1:
  - `crates/domain/src/{lib.rs,research.rs}`
  - `crates/persistence/migrations/*alpha_hypotheses*.sql`
  - `crates/persistence/src/postgres/{mod.rs,alpha_hypotheses.rs}`
  - `services/research-gateway/src/{lib.rs,main.rs,validation/mod.rs,validation/hypothesis_registry.rs}`
  - `services/control-api/Cargo.toml`
  - `services/control-api/src/{main.rs,middleware/mod.rs,routes/mod.rs}`
  - `docs/operations/alpha-hypothesis-registry.md`
  - `tests/api/story-6-1*.test.mjs`
  - `tests/e2e/story-6-1*.test.mjs`
  - `package.json`
  - `_bmad-output/implementation-artifacts/tests/test-summary.md`
- Reuse established vertical-slice sequence from recent stories:
  - domain contracts -> migration -> persistence adapter -> orchestrator -> control-api routes -> tests -> runbook.
- Keep schema/story boundaries strict: only `alpha_hypotheses` in this story.

[Source: _bmad-output/planning-artifacts/architecture.md#Complete Project Directory Structure]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: services/control-api/Cargo.toml]  
[Source: services/control-api/src/main.rs]  
[Source: services/control-api/src/middleware/mod.rs]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: crates/persistence/src/postgres/mod.rs]  
[Source: services/research-gateway/src/main.rs]  
[Source: services/research-gateway/src/validation/mod.rs]  
[Source: services/research-gateway/src/promotion/mod.rs]

### Testing Requirements

- Add deterministic coverage for:
  - FR6 required-field validation and machine-readable field-error contracts,
  - dataset-snapshot registration-reference failure/allow paths,
  - identifier normalization and training-window boundary semantics (`start < end`),
  - idempotent/update behavior for repeated `hypothesis_id` submissions,
  - authenticated register/read route behavior and explicit failure status mapping,
  - audit/telemetry evidence emission continuity for both allow and deny outcomes.
- Keep test layering aligned with repository conventions:
  - domain contract tests in `crates/domain`,
  - migration/adapter tests in `crates/persistence`,
  - orchestration tests in `services/research-gateway`,
  - route tests in `services/control-api`,
  - story-scoped API/E2E tests in `tests/api` + `tests/e2e`.
- Add story QA command:
  - `qa:test:story-6-1` in root `package.json`.

[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: package.json]  
[Source: _bmad-output/implementation-artifacts/stories/5-1-implement-reward-per-risk-policy-configuration-and-scoring.md#Testing Requirements]  
[Source: _bmad-output/implementation-artifacts/stories/5-4-configure-core-satellite-market-stratification-policies.md#Testing Requirements]

### Previous Story Intelligence

- There is no prior Story 6.x implementation artifact yet; continuity should come from recent Epic 5 delivery patterns:
  - deterministic machine-readable reason-code contracts at domain and API boundaries,
  - forward-only single-story schema increments with strict scope isolation,
  - service-orchestrator operation-lock + fail-closed dependency handling,
  - authenticated route + privileged audit append + telemetry emission continuity.
- Reuse, do not reinvent:
  - route response/error helper patterns in `services/control-api/src/routes/mod.rs`,
  - repository validation and query-classification patterns in `crates/persistence/src/postgres/*`,
  - orchestration role-validation + evidence models in governance-service modules.

[Source: _bmad-output/implementation-artifacts/stories/5-1-implement-reward-per-risk-policy-configuration-and-scoring.md#Project Structure Notes]  
[Source: _bmad-output/implementation-artifacts/stories/5-2-add-incentive-and-regime-shift-detection-alerts.md#Project Structure Notes]  
[Source: _bmad-output/implementation-artifacts/stories/5-3-enforce-low-liquidity-and-overnight-participation-guardrails.md#Project Structure Notes]  
[Source: _bmad-output/implementation-artifacts/stories/5-4-configure-core-satellite-market-stratification-policies.md#Project Structure Notes]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: crates/persistence/src/postgres/reward_risk.rs]  
[Source: crates/persistence/src/postgres/market_bucket_profiles.rs]

### Git Intelligence Summary

- Recent Story 5 commits show a stable implementation cadence:
  1. domain reason-code/validation contracts,  
  2. forward-only migration + persistence adapter,  
  3. service orchestration with role validation and telemetry evidence,  
  4. authenticated control-plane route integration and audit continuity,  
  5. story-scoped QA scripts/tests and operations runbook updates.
- Story 6.1 should follow this same sequence to reduce regression risk and preserve repository conventions.

[Source: git --no-pager log --oneline -5]  
[Source: git --no-pager log -5 --name-only --pretty=format:'%h %s']

### Latest Technical Information

- Workspace dependency set is sufficient for Story 6.1 scope; no mandatory upgrades are required.
- `sqlx` latest indexed version is pre-release (`0.9.0-alpha.1`), so Story 6.1 should remain on stable workspace `0.8.6`.
- `research-gateway` currently exists as a scaffold binary; Story 6.1 should introduce minimal library/orchestrator surfaces without stack churn.

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
- Story context is derived from epics/PRD/architecture/UX artifacts, implementation-readiness report, research artifacts, prior Story 4/5 implementation patterns, recent git history, and current code seams.

### Project Structure Notes

- Current codebase seams relevant to Story 6.1:
  - `research-gateway` exists but `validation`/`promotion` modules are placeholders,
  - `control-api` already supports authenticated policy mutation/read route families with canonical audit/error helpers,
  - `persistence` uses per-story module + migration wiring through `crates/persistence/src/postgres/mod.rs`.
- **Blocking design constraint documented:** FR6 requires a registered dataset-snapshot reference, but no dedicated dataset-snapshot registry table/service currently exists in persisted schema.
  - Story 6.1 should implement an explicit validation seam (`DatasetSnapshotRegistryPort`) and fail closed when unresolved/unavailable,
  - maintain schema contract (`alpha_hypotheses` only) and avoid speculative multi-table expansion in this story.
- RBAC role model currently exposes `read_only_analytics`, `operational_control`, and `administrative_actions`; Story 6.1 should reuse canonical privileged-role authorization until a formal research-specific role is introduced.

[Source: services/research-gateway/src/validation/mod.rs]  
[Source: services/research-gateway/src/promotion/mod.rs]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: services/control-api/src/middleware/mod.rs]  
[Source: crates/persistence/src/postgres/mod.rs]  
[Source: _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)]  
[Source: _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)]  
[Source: docs/governance/rbac-role-model.md]

### References

- _bmad-output/planning-artifacts/epics.md#Epic 6: Research-to-Production Alpha Governance Lifecycle  
- _bmad-output/planning-artifacts/epics.md#Story 6.1: Build Alpha Hypothesis Registry with Required Metadata  
- _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)  
- _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)  
- _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)  
- _bmad-output/planning-artifacts/prd.md#Strategy Research & Alpha Lifecycle  
- _bmad-output/planning-artifacts/prd.md#Journey 6 — Research User (Phase 2+): Noor, Quant Research Lead  
- _bmad-output/planning-artifacts/prd.md#Model Integrity & Promotion Governance  
- _bmad-output/planning-artifacts/prd.md#Compliance & Auditability  
- _bmad-output/planning-artifacts/architecture.md#Data Architecture  
- _bmad-output/planning-artifacts/architecture.md#API & Communication Patterns  
- _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules  
- _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries  
- _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping  
- _bmad-output/planning-artifacts/ux-design-specification.md#Journey 3 — Alpha Promotion Governance  
- _bmad-output/planning-artifacts/ux-design-specification.md#Feedback Patterns  
- _bmad-output/planning-artifacts/ux-design-specification.md#Form Patterns  
- _bmad-output/planning-artifacts/implementation-readiness-report-2026-04-05.md  
- _bmad-output/planning-artifacts/research/domain-polymarket-rust-clob-profit-research-2026-04-04.md  
- _bmad-output/planning-artifacts/research/market-rust-polymarket-trading-system-lopez-de-prado-research-2026-04-04.md  
- _bmad-output/planning-artifacts/research/technical-polymarket-claim-validity-audit-research-2026-04-04.md  
- _bmad-output/implementation-artifacts/stories/4-1-define-normalized-reporting-read-models.md  
- _bmad-output/implementation-artifacts/stories/5-1-implement-reward-per-risk-policy-configuration-and-scoring.md  
- _bmad-output/implementation-artifacts/stories/5-2-add-incentive-and-regime-shift-detection-alerts.md  
- _bmad-output/implementation-artifacts/stories/5-3-enforce-low-liquidity-and-overnight-participation-guardrails.md  
- _bmad-output/implementation-artifacts/stories/5-4-configure-core-satellite-market-stratification-policies.md  
- docs/governance/rbac-role-model.md  
- docs/operations/report-export-workflows.md  
- docs/operations/reward-risk-policy-operations.md  
- docs/operations/core-satellite-market-stratification.md  
- services/control-api/Cargo.toml  
- services/control-api/src/{main.rs,middleware/mod.rs,routes/mod.rs}  
- services/governance-service/src/{lib.rs,market_policy/mod.rs,reward_risk/mod.rs}  
- services/research-gateway/src/{main.rs,validation/mod.rs,promotion/mod.rs}  
- crates/domain/src/{lib.rs,risk.rs,governance.rs,reporting_export.rs}  
- crates/persistence/src/postgres/{mod.rs,reward_risk.rs,market_bucket_profiles.rs}  
- crates/persistence/migrations/{20260407090000_reward_risk_policies.sql,20260407133000_market_bucket_profiles.sql}  
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

- Story 6.1 implementation is complete and all tasks/subtasks are verified.
- FR6 alpha-hypothesis registry, dataset-linkage fail-closed validation seam, authenticated control-plane routes, and NFR17 evidence continuity are delivered end-to-end.
- Story-scoped QA and full repository regression tests pass.
- Code-review triage high/medium findings were auto-remediated and revalidated (read-surface authorization boundary + deny-path telemetry consistency).

## Dev Agent Record

### Agent Model Used

GPT-5.3-Codex (gpt-5.3-codex)

### Debug Log References

- BMAD dev-story workflow execution (automated, non-interactive)
- Story-scoped QA run: `source "$HOME/.cargo/env" && npm run qa:test:story-6-1`
- QA automation rerun (2026-04-07 16:57:57): `source "$HOME/.cargo/env" && npm run --silent qa:test:story-6-1` (pass)
- Full repository regression run: `source "$HOME/.cargo/env" && npm run test`
- Workspace build run: `source "$HOME/.cargo/env" && npm run rust:build`
- Workspace lint run: `source "$HOME/.cargo/env" && npm run rust:lint` (fails on pre-existing clippy findings in `crates/domain/src/recovery.rs`, outside Story 6.1 scope)

### Completion Notes List

- Implemented FR6 domain contracts in `crates/domain/src/research.rs` with canonical registration normalization, machine-readable reason-code taxonomy, strict UTC training-window validation, and explicit nested `risk_assumptions` validation.
- Added forward-only `alpha_hypotheses` migration and Postgres adapter (`crates/persistence/src/postgres/alpha_hypotheses.rs`) with deterministic upsert/read semantics, canonical constraints/indexes, and explicit persistence error mapping.
- Introduced `services/research-gateway/src/validation/hypothesis_registry.rs` orchestration with `DatasetSnapshotRegistryPort`, static registry seam (`RESEARCH_DATASET_SNAPSHOT_VERSIONS`), fail-closed unresolved/unavailable handling, operation lock safety, and telemetry evidence emission.
- Integrated research orchestrator wiring into control API state/startup and exposed authenticated FR6 routes (`POST/GET /control/research/alpha-hypotheses/{hypothesis_id}`) with canonical response envelopes, deterministic status mapping (`400/403/409/503/500`), and audit continuity.
- Added Story 6.1 route/service/domain/persistence tests plus story-scoped API/E2E assertions and `qa:test:story-6-1` command wiring in `package.json`.
- Published `docs/operations/alpha-hypothesis-registry.md` and cross-linked report-export/reward-risk/risk-limit/core-satellite runbooks for operator and research continuity.
- Updated `_bmad-output/implementation-artifacts/tests/test-summary.md` with Story 6.1 QA evidence and execution inventory.
- Re-executed Story 6.1 QA automation on 2026-04-07 16:57:57 and reconfirmed all story-scoped Rust plus API/E2E checks pass without additional remediation.
- Applied workspace `cargo fmt` formatting to previously modified Epic 5 files and refreshed one Story 5.4 static E2E assertion for formatting-tolerant compatibility so full regression (`npm test`) remains green.
- Verified working-tree file-list coverage; `.scripts/bmad-auto/copilot/bmad-progress.log` is an automation session artifact and intentionally excluded from implementation scope.
- Code review auto-fix pass aligned `GET /control/research/alpha-hypotheses/{hypothesis_id}` with analytics-read authorization semantics and added deterministic deny telemetry emission for non-empty/timestamp validation failures in hypothesis orchestration.

### Review Findings

- [x] [Review][Patch] Alpha-hypothesis read endpoint authorization used control-plane mutation authorization for `GET` requests; fixed by adding `authorize_alpha_hypothesis_read` with `ReadAnalyticsDashboard` evaluation and alpha-specific unauthorized envelope mapping.
- [x] [Review][Patch] Hypothesis orchestration returned early on non-empty/timestamp validation failures without emitting deny telemetry; fixed by introducing telemetry-wrapped validation helpers and UTC validation for `queried_at_utc`.

### File List

- _bmad-output/implementation-artifacts/sprint-status.yaml
- _bmad-output/implementation-artifacts/stories/6-1-build-alpha-hypothesis-registry-with-required-metadata.md
- _bmad-output/implementation-artifacts/tests/test-summary.md
- Cargo.lock
- crates/domain/src/lib.rs
- crates/domain/src/research.rs
- crates/domain/src/risk.rs
- crates/persistence/migrations/20260407160000_alpha_hypotheses.sql
- crates/persistence/src/postgres/alpha_hypotheses.rs
- crates/persistence/src/postgres/market_bucket_profiles.rs
- crates/persistence/src/postgres/mod.rs
- docs/operations/alpha-hypothesis-registry.md
- docs/operations/core-satellite-market-stratification.md
- docs/operations/report-export-workflows.md
- docs/operations/reward-risk-policy-operations.md
- docs/operations/risk-limit-policy-operations.md
- package.json
- services/control-api/Cargo.toml
- services/control-api/src/main.rs
- services/control-api/src/middleware/mod.rs
- services/control-api/src/routes/mod.rs
- services/governance-service/src/market_policy/mod.rs
- services/research-gateway/Cargo.toml
- services/research-gateway/src/lib.rs
- services/research-gateway/src/main.rs
- services/research-gateway/src/validation/hypothesis_registry.rs
- services/research-gateway/src/validation/mod.rs
- services/risk-engine/src/gates/mod.rs
- services/risk-engine/src/main.rs
- tests/api/story-6-1-alpha-hypothesis-registry-api.test.mjs
- tests/e2e/story-5-4-market-bucket-stratification-gating.e2e.test.mjs
- tests/e2e/story-6-1-alpha-hypothesis-registry.e2e.test.mjs

### Change Log

- 2026-04-07: Executed `bmad-qa-generate-e2e-tests` rerun for Story 6.1 (`source "$HOME/.cargo/env" && npm run --silent qa:test:story-6-1`); all suites passed and story status remains `done`.
- 2026-04-07: Completed adversarial code review, auto-fixed high/medium findings (analytics-read authorization guard for alpha-hypothesis reads, deny-path telemetry for validation failures, UTC validation for `queried_at_utc`), and reran Story 6.1 + full regression suites.
- 2026-04-07: Implemented Story 6.1 FR6 alpha-hypothesis registry end-to-end (domain contracts, `alpha_hypotheses` migration/adapter, research-gateway orchestration seam, control-api wiring/routes, and Story 6.1 QA automation).
- 2026-04-07: Published FR6 operations runbook and cross-runbook continuity links (report-export, reward-risk, risk-limit, core-satellite).
- 2026-04-07: Completed Story 6.1 validation runs (`qa:test:story-6-1`, `npm test`, `rust:build`) and advanced story status to `review`.
