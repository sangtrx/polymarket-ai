# Story 5.4: Configure Core/Satellite Market Stratification Policies

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an operator,  
I want separate policies for core and satellite market buckets,  
so that exposure and allocation are tuned by market class.

## Acceptance Criteria

1. **Scenario A - Story-local BDD baseline:**  
   **Given** market stratification and policy profiles are configured  
   **When** routing and risk checks run  
   **Then** bucket-specific rules are applied consistently  
   **And** policy outcomes satisfy FR42.

2. **Scenario B - deterministic stratification contract:**  
   **Given** a market is assigned to a stratification bucket  
   **When** configuration is validated  
   **Then** only canonical bucket values (`core`, `satellite`) are accepted  
   **And** identifier normalization is deterministic (trim + lowercase) across control, persistence, and runtime layers.

3. **Scenario C - stratification persistence and schema scope:**  
   **Given** a stratification profile mutation is accepted  
   **When** persistence completes  
   **Then** evidence is stored only in `market_bucket_profiles` with bucket type, policy links, actor, correlation, and UTC timestamps  
   **And** no unrelated schema artifacts are introduced.

4. **Scenario D - authenticated mutation/read surfaces:**  
   **Given** an authorized operator submits or reads stratification configuration  
   **When** control-plane endpoints execute  
   **Then** responses follow canonical machine-readable envelope/error patterns  
   **And** unauthorized or malformed requests are denied explicitly.

5. **Scenario E - bucket-specific risk-policy application in pre-trade flow:**  
   **Given** an order intent references a market with an active bucket profile  
   **When** risk-engine gate evaluation runs  
   **Then** the effective risk-policy key is resolved from the market bucket profile  
   **And** risk-limit and participation-guardrail checks use that resolved policy context consistently.

6. **Scenario F - bucket-specific allocation-policy linkage continuity:**  
   **Given** bucket profiles include allocation policy keys  
   **When** routing/allocation decision context is generated  
   **Then** allocation policy linkage for the resolved bucket is surfaced consistently for downstream allocation/rebalance seams  
   **And** cross-bucket policy leakage is prevented.

7. **Scenario G - fail-closed unavailable-state behavior:**  
   **Given** bucket mapping or linked policy dependencies are missing, stale, or invalid  
   **When** stratification-dependent routing/risk evaluation executes  
   **Then** the system fails closed with explicit machine-readable reason codes  
   **And** no success-shaped fallback path is emitted.

8. **Scenario H - boundary and conflict determinism:**  
   **Given** duplicate active mappings, unknown bucket values, or conflicting policy links are submitted  
   **When** validation and persistence constraints run  
   **Then** conflicts are rejected deterministically with field-level diagnostics  
   **And** canonical one-active-mapping semantics remain enforced.

9. **Scenario I - NFR17 auditability:**  
   **Given** stratification updates and stratification-driven decisions occur  
   **When** audit evidence is written  
   **Then** records include actor, action, parameters, approval status, reason, correlation, and timestamp metadata  
   **And** query paths remain suitable for sub-5-second audit retrieval targets.

10. **UAC-1 Failure handling:** Invalid payloads, unauthorized access, unsupported bucket values, and unavailable dependency paths return explicit machine-readable errors with no unsafe side effects.

11. **UAC-2 Boundary behavior:** Bucket parsing, active-profile uniqueness, policy-link resolution precedence, and fallback/fail-closed semantics are deterministic and test-covered.

12. **UAC-3 Verifiable evidence:** Successful and failed stratification mutations and stratification-driven routing decisions emit timestamped telemetry/audit evidence for incident and QA traceability.

13. **Schema/dependency/traceability contract:** Story depends on `5.1`; schema scope introduces only `market_bucket_profiles`; traceability maps to `FR42` and `NFR17`.

14. **Scope boundary contract:** Story 5.4 delivers market stratification configuration and policy-link application only; it must not pre-implement Epic 6 research-governance features.

## Tasks / Subtasks

- [x] **Task 1: Define FR42 domain contracts for market-bucket stratification and policy-link resolution** (AC: 1, 2, 7, 8, 10, 11, 13)
  - [x] Extend `crates/domain/src/risk.rs` with canonical bucket-type contracts (`core`, `satellite`), stratification profile structures, and validation helpers.
  - [x] Add deterministic policy-link resolution helpers (market/cluster -> risk policy key + allocation policy key) with explicit fail-closed errors for unavailable or invalid mappings.
  - [x] Add/extend reason-code taxonomy for FR42 invalid/unavailable/conflict states and align mapping to existing pre-trade machine-readable decision surfaces.
  - [x] Ensure UTC timestamp and identifier normalization contracts match existing market-policy/risk-limit patterns.

- [x] **Task 2: Add forward-only migration and persistence adapter for `market_bucket_profiles`** (AC: 3, 7, 8, 10, 11, 13)
  - [x] Add migration under `crates/persistence/migrations/` creating only `market_bucket_profiles` with canonical identifiers, bucket type, linked policy keys, actor/correlation evidence fields, and UTC constraints.
  - [x] Add persistence adapter module (for example `crates/persistence/src/postgres/market_bucket_profiles.rs`) and wire via `crates/persistence/src/postgres/mod.rs`.
  - [x] Implement deterministic upsert/read/list operations with conflict-safe uniqueness semantics for active mappings.
  - [x] Add persistence tests for schema constraints, deterministic ordering, and machine-readable query/constraint/decode error mapping.

- [x] **Task 3: Implement governance orchestration for stratification profile lifecycle** (AC: 3, 4, 7, 9, 10, 13)
  - [x] Reuse and extend existing market-policy governance seams in `services/governance-service/src/market_policy/mod.rs` (or introduce a narrowly scoped sibling module only if required) for FR42 mutation/read orchestration.
  - [x] Enforce role validation, canonical normalization, and fail-closed dependency behavior consistent with Story 5.1–5.3 service patterns.
  - [x] Emit evidence payloads including bucket type and linked risk/allocation policy keys plus actor/correlation/timestamp metadata.
  - [x] Preserve operation-lock semantics to prevent race-condition profile conflicts.

- [x] **Task 4: Expose authenticated control-plane FR42 routes and payload contracts** (AC: 4, 8, 9, 10, 12, 13)
  - [x] Add authenticated stratification mutation/read routes in `services/control-api/src/routes/mod.rs` under canonical plural kebab-case resource paths.
  - [x] Add payload/query structs and route wiring using existing authorization/audit middleware patterns.
  - [x] Ensure canonical `data/meta/error` response envelopes and explicit status mapping for `400/403/409/503/500`.
  - [x] Add route tests validating accepted evidence, malformed payload rejection, unauthorized-role denial, and persistence-unavailable fail-closed behavior.

- [x] **Task 5: Integrate FR42 stratification into risk-engine runtime hydration and gate evaluation** (AC: 1, 5, 7, 8, 10, 11, 13)
  - [x] Extend `services/risk-engine/src/main.rs` bootstrap hydration to load active market-bucket stratification state.
  - [x] Extend `services/risk-engine/src/gates/mod.rs` runtime policy state with stratification read helpers keyed by canonical market/cluster identifiers.
  - [x] Resolve effective risk profile key from stratification before risk-limit-state and FR41 baseline-dependent checks execute.
  - [x] Ensure unresolved/invalid stratification links fail closed with explicit machine-readable deny reasons.

- [x] **Task 6: Wire allocation-policy linkage continuity for stratified routing context** (AC: 1, 6, 7, 8, 10, 11, 13)
  - [x] Reuse existing allocation-policy contracts (`services/governance-service/src/allocation_policy/mod.rs`, `services/control-api/src/routes/mod.rs`, `services/portfolio-engine/src/allocation/mod.rs`) to consume bucket-linked allocation policy keys without creating parallel policy stacks.
  - [x] Surface resolved allocation policy linkage in routing/rebalance context where required for deterministic operator workflows.
  - [x] Prevent implicit fallback to unrelated non-bucket policy keys when explicit bucket mapping exists.

- [x] **Task 7: Add deterministic Story 5.4 QA coverage and command wiring** (AC: 1-14)
  - [x] Add domain tests for bucket parsing, link resolution precedence, and fail-closed invalid/unavailable cases.
  - [x] Add persistence tests for `market_bucket_profiles` constraints, conflict handling, and deterministic lookup behavior.
  - [x] Add governance/control-api tests for mutation/read success, validation errors, unauthorized access, and dependency-unavailable handling.
  - [x] Add risk-engine tests proving bucket-specific policy application in gate evaluation and no regression to existing gate order semantics.
  - [x] Add story-scoped API/E2E tests (`tests/api/story-5-4*.test.mjs`, `tests/e2e/story-5-4*.test.mjs`) and wire `qa:test:story-5-4` in root `package.json`.

- [x] **Task 8: Publish FR42 operations runbook and artifact updates** (AC: 9, 12, 13)
  - [x] Add `docs/operations/core-satellite-market-stratification.md` documenting bucket semantics, policy-link lifecycle, fail-closed playbooks, and audit verification steps.
  - [x] Cross-link FR42 runbook with existing `market-policy`, `risk-limit`, `allocation-rebalance`, `reward-risk`, `regime-shift`, and `pretrade-gate` runbooks.
  - [x] Update `_bmad-output/implementation-artifacts/tests/test-summary.md` with Story 5.4 evidence after implementation.

### Review Findings

- [x] [Review][Patch] Runtime bootstrap now hydrates risk-limit state with the effective bucket-linked risk policy key (`services/risk-engine/src/main.rs`), preventing false `pretrade_risk_limit_state_unavailable` denies when FR42 bucket policy keys differ from the default profile key.
- [x] [Review][Patch] Bucket-profile read misses now emit explicit deny telemetry (`services/governance-service/src/market_policy/mod.rs`) so failed mapping reads remain traceable under FR42/NFR17 evidence expectations.
- [x] [Review][Defer] Git reality includes `.scripts/bmad-auto/copilot/bmad-progress.log`, which is an automation runtime artifact outside Story 5.4 application implementation scope.

## Dev Notes

### Technical Requirements

- Story objective is FR42 policy-aware market stratification:
  - operator-configured `core`/`satellite` bucket assignment,
  - separate linked risk/allocation policy keys per bucket,
  - deterministic bucket-specific application during routing and risk checks.
- Story dependency and scope contract:
  - dependency: `5.1`,
  - schema scope: `market_bucket_profiles` only,
  - traceability: `FR42`, `NFR17`.
- Journey continuity requirement:
  - incentive-shift operations must remain able to review reward-per-risk impact by market bucket and reallocation context.
- FR42 must preserve fail-closed posture:
  - unknown bucket value, missing mapping, or unresolved linked policy states are deny/unavailable outcomes, not implicit pass.
- Out of scope for Story 5.4:
  - FR40 alert trigger redesign,
  - FR41 threshold recalibration,
  - Epic 6 research-governance feature delivery.

[Source: _bmad-output/planning-artifacts/epics.md#Story 5.4: Configure Core/Satellite Market Stratification Policies]  
[Source: _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)]  
[Source: _bmad-output/planning-artifacts/prd.md#Incentive Intelligence & Market Regime Management]  
[Source: _bmad-output/planning-artifacts/prd.md#Journey 7 — Incentive Shift Response: Sang, Independent Quant Operator]  
[Source: _bmad-output/planning-artifacts/implementation-readiness-report-2026-04-05.md]

### Architecture Compliance

- Preserve bounded responsibilities:
  - `control-api` remains authenticated control-plane ingress,
  - `governance-service` owns privileged mutation/read orchestration,
  - `risk-engine` owns runtime routing/risk eligibility application,
  - `execution-engine` must not mutate stratification policy state directly.
- Follow architecture rules:
  - canonical naming (`snake_case` in Rust/DB, plural kebab-case resource paths),
  - ISO-8601 UTC timestamps only,
  - deterministic machine-readable reason/error codes,
  - no swallowed errors in control or execution paths.
- Maintain gate safety invariants:
  - preserve existing pre-trade gate order,
  - no bypass of risk/governance pathways for convenience.

[Source: _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules]  
[Source: _bmad-output/planning-artifacts/architecture.md#Architectural Boundaries]  
[Source: _bmad-output/planning-artifacts/architecture.md#Integration Points]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: docs/operations/pretrade-gate-pipeline.md#Gate Evaluation Order]

### Library & Framework Requirements

- Keep workspace-pinned dependencies for compatibility:
  - `axum = 0.8.8`
  - `sqlx = 0.8.6`
  - `tokio = 1.48.0`
  - `time = 0.3.44`
  - `polymarket-client-sdk = 0.4.4`
- Latest checks at story creation time:
  - `axum` latest stable remains `0.8.8`,
  - `sqlx` latest indexed release is `0.9.0-alpha.1` (pre-release), stable workspace target remains `0.8.6`,
  - `tokio` latest stable is `1.51.0`,
  - `time` latest stable is `0.3.47`,
  - `polymarket-client-sdk` latest stable remains `0.4.4`.
- Do not perform opportunistic dependency upgrades in Story 5.4.

[Source: Cargo.toml]  
[Source: source $HOME/.cargo/env && cargo search axum --limit 1]  
[Source: source $HOME/.cargo/env && cargo search sqlx --limit 1]  
[Source: source $HOME/.cargo/env && cargo search tokio --limit 1]  
[Source: source $HOME/.cargo/env && cargo search time --limit 1]  
[Source: source $HOME/.cargo/env && cargo search polymarket-client-sdk --limit 1]

### File Structure Requirements

- Primary implementation surfaces for Story 5.4:
  - `crates/domain/src/risk.rs`
  - `crates/persistence/migrations/*market_bucket_profiles*.sql`
  - `crates/persistence/src/postgres/{mod.rs,market_bucket_profiles.rs}`
  - `services/governance-service/src/market_policy/mod.rs` (or tightly scoped FR42 sibling module if required)
  - `services/control-api/src/{routes/mod.rs,middleware/mod.rs,main.rs}`
  - `services/risk-engine/src/{main.rs,gates/mod.rs,limits/mod.rs}`
  - `services/portfolio-engine/src/allocation/mod.rs` (if bucket-linked allocation context is surfaced there)
  - `docs/operations/core-satellite-market-stratification.md`
  - `tests/api/story-5-4*.test.mjs`
  - `tests/e2e/story-5-4*.test.mjs`
  - `package.json`
  - `_bmad-output/implementation-artifacts/tests/test-summary.md`
- Reuse established vertical-slice sequence from Story 5.1–5.3:
  - domain contracts -> migration -> persistence adapter -> governance/control integration -> risk runtime integration -> tests -> runbook.
- Keep schema/story boundaries strict; do not introduce Epic 6 or unrelated schema entities.

[Source: _bmad-output/planning-artifacts/architecture.md#Complete Project Directory Structure]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: _bmad-output/implementation-artifacts/stories/5-1-implement-reward-per-risk-policy-configuration-and-scoring.md#File Structure Requirements]  
[Source: _bmad-output/implementation-artifacts/stories/5-2-add-incentive-and-regime-shift-detection-alerts.md#File Structure Requirements]  
[Source: _bmad-output/implementation-artifacts/stories/5-3-enforce-low-liquidity-and-overnight-participation-guardrails.md#File Structure Requirements]

### Testing Requirements

- Add deterministic coverage for:
  - canonical bucket parsing and normalization (`core`, `satellite` only),
  - conflict/uniqueness behavior for active bucket mappings,
  - fail-closed unresolved stratification and linked-policy dependency behavior,
  - bucket-specific risk-profile application in pre-trade gate flow without gate-order regression,
  - allocation-policy link continuity and non-leakage across bucket boundaries,
  - machine-readable endpoint error mapping and authorization boundaries.
- Keep test layering aligned with repository conventions:
  - domain contract tests in `crates/domain`,
  - migration/adapter tests in `crates/persistence`,
  - orchestrator tests in `services/governance-service`,
  - route tests in `services/control-api`,
  - runtime integration tests in `services/risk-engine`,
  - story-scoped API/E2E tests in `tests/api` + `tests/e2e`.
- Add story QA command:
  - `qa:test:story-5-4` in root `package.json`.

[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: package.json]  
[Source: docs/operations/pretrade-gate-pipeline.md]  
[Source: docs/operations/risk-limit-policy-operations.md]  
[Source: docs/operations/allocation-rebalance-workflows.md]

### Previous Story Intelligence

- Story 5.1 established FR39 policy-override seams and deterministic reward-per-risk fail-closed semantics; FR42 must integrate with these seams, not replace them.
- Story 5.2 established FR40 evidence/dispatch/query contract rigor and machine-readable dependency-state handling patterns; FR42 should mirror this control-plane consistency.
- Story 5.3 introduced FR41 guardrail evidence and `normal_max_order_size_units(profile_key, market_id, cluster_id)` dependency; FR42 must ensure resolved bucket-linked profile keys propagate correctly into this path.
- Existing runtime seam to extend:
  - pre-trade evaluation currently receives a single `profile_key`; FR42 should resolve this key from stratification before risk/guardrail evaluation.

[Source: _bmad-output/implementation-artifacts/stories/5-1-implement-reward-per-risk-policy-configuration-and-scoring.md#Project Structure Notes]  
[Source: _bmad-output/implementation-artifacts/stories/5-2-add-incentive-and-regime-shift-detection-alerts.md#Project Structure Notes]  
[Source: _bmad-output/implementation-artifacts/stories/5-3-enforce-low-liquidity-and-overnight-participation-guardrails.md#Project Structure Notes]  
[Source: services/risk-engine/src/gates/mod.rs]  
[Source: services/risk-engine/src/main.rs]  
[Source: services/risk-engine/src/limits/mod.rs]

### Git Intelligence Summary

- Recent commit history (`5-1`, `5-2`, `5-3`) follows a stable implementation pattern:
  1. domain contracts and reason-code updates,  
  2. forward-only migration and persistence adapter,  
  3. governance/control-api wiring,  
  4. runtime risk integration,  
  5. story-scoped QA command/tests and operations documentation.
- Story 5.4 should follow this same sequence to minimize regression risk and maintain consistency.

[Source: git --no-pager log --oneline -5]  
[Source: git --no-pager log -5 --name-only --pretty=format:'--- %h %s']

### Latest Technical Information

- Workspace dependencies are already suitable for Story 5.4 scope; no mandatory upgrades are required.
- `sqlx` newest index listing is pre-release (`0.9.0-alpha.1`), so implementation should remain on stable workspace `0.8.6`.
- Dependency drift risk remains low if Story 5.4 stays within existing architecture seams and version pins.

[Source: Cargo.toml]  
[Source: source $HOME/.cargo/env && cargo search axum --limit 1]  
[Source: source $HOME/.cargo/env && cargo search sqlx --limit 1]  
[Source: source $HOME/.cargo/env && cargo search tokio --limit 1]  
[Source: source $HOME/.cargo/env && cargo search time --limit 1]  
[Source: source $HOME/.cargo/env && cargo search polymarket-client-sdk --limit 1]

### Project Context Reference

- No `project-context.md` file was found during discovery.
- Story context is derived from epics/PRD/architecture/UX artifacts, implementation-readiness artifacts, prior Epic 5 stories, recent git history, and current code seams.

### Project Structure Notes

- Existing FR42-adjacent seams already available:
  - market policy configuration paths (`/control/market-policy/...`),
  - risk-limit profile/state evaluation keyed by `profile_key`,
  - allocation-policy orchestration keyed by `policy_key`,
  - pre-trade gate pipeline where resolved policy keys are consumed.
- Primary integration point for FR42:
  - resolve stratified policy links before risk-limit snapshot and FR41 guardrail evaluation paths, while preserving deterministic gate ordering.
- Implementation should avoid creating duplicate policy stacks; extend established market-policy/risk-limit/allocation seams with explicit stratification linkage.

[Source: services/control-api/src/routes/mod.rs]  
[Source: services/control-api/src/middleware/mod.rs]  
[Source: services/governance-service/src/market_policy/mod.rs]  
[Source: services/governance-service/src/risk_limits/mod.rs]  
[Source: services/governance-service/src/allocation_policy/mod.rs]  
[Source: crates/persistence/src/postgres/{market_policy.rs,risk_limits.rs,allocation_policies.rs}]  
[Source: docs/operations/market-policy-engine.md]  
[Source: docs/operations/risk-limit-policy-operations.md]  
[Source: docs/operations/allocation-rebalance-workflows.md]

### References

- _bmad-output/planning-artifacts/epics.md#Epic 5: Incentive-Regime Adaptive Trading Controls  
- _bmad-output/planning-artifacts/epics.md#Story 5.4: Configure Core/Satellite Market Stratification Policies  
- _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)  
- _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)  
- _bmad-output/planning-artifacts/prd.md#Incentive Intelligence & Market Regime Management  
- _bmad-output/planning-artifacts/prd.md#Journey 7 — Incentive Shift Response: Sang, Independent Quant Operator  
- _bmad-output/planning-artifacts/prd.md#Compliance & Auditability  
- _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules  
- _bmad-output/planning-artifacts/architecture.md#Architectural Boundaries  
- _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping  
- _bmad-output/planning-artifacts/architecture.md#Integration Points  
- _bmad-output/planning-artifacts/ux-design-specification.md#Form Patterns  
- _bmad-output/planning-artifacts/ux-design-specification.md#Feedback Patterns  
- _bmad-output/planning-artifacts/implementation-readiness-report-2026-04-05.md  
- _bmad-output/implementation-artifacts/stories/5-1-implement-reward-per-risk-policy-configuration-and-scoring.md  
- _bmad-output/implementation-artifacts/stories/5-2-add-incentive-and-regime-shift-detection-alerts.md  
- _bmad-output/implementation-artifacts/stories/5-3-enforce-low-liquidity-and-overnight-participation-guardrails.md  
- docs/operations/market-policy-engine.md  
- docs/operations/reward-risk-policy-operations.md  
- docs/operations/incentive-regime-shift-alerts.md  
- docs/operations/low-liquidity-overnight-guardrails.md  
- docs/operations/risk-limit-policy-operations.md  
- docs/operations/allocation-rebalance-workflows.md  
- docs/operations/pretrade-gate-pipeline.md  
- services/control-api/src/{main.rs,middleware/mod.rs,routes/mod.rs}  
- services/governance-service/src/{lib.rs,market_policy/mod.rs,risk_limits/mod.rs,allocation_policy/mod.rs,reward_risk/mod.rs}  
- services/risk-engine/src/{main.rs,gates/mod.rs,limits/mod.rs}  
- services/portfolio-engine/src/allocation/mod.rs  
- crates/domain/src/{risk.rs,allocation.rs}  
- crates/persistence/migrations/{20260406004500_market_policy_profiles.sql,20260406072000_risk_limit_profiles.sql,20260406113000_allocation_policies_rebalance_recommendations.sql}  
- crates/persistence/src/postgres/{mod.rs,market_policy.rs,risk_limits.rs,allocation_policies.rs,reward_risk.rs,participation_guardrail_events.rs}  
- Cargo.toml  
- package.json  
- git --no-pager log --oneline -5  
- git --no-pager log -5 --name-only --pretty=format:'--- %h %s'  
- source $HOME/.cargo/env && cargo search axum --limit 1  
- source $HOME/.cargo/env && cargo search sqlx --limit 1  
- source $HOME/.cargo/env && cargo search tokio --limit 1  
- source $HOME/.cargo/env && cargo search time --limit 1  
- source $HOME/.cargo/env && cargo search polymarket-client-sdk --limit 1

## Story Completion Status

- Story 5.4 implementation is complete and all tasks/subtasks are verified.
- FR42 market-bucket contracts, persistence, governance/control integration, risk-engine stratification enforcement, and operations docs are delivered.
- Story-scoped QA plus full repository regression tests pass with no failures.

## Dev Agent Record

### Agent Model Used

GPT-5.3-Codex (gpt-5.3-codex)

### Debug Log References

- BMAD dev-story workflow execution (automated, non-interactive resume from `in-progress`)
- Story-scoped QA run: `npm run --silent qa:test:story-5-4`
- Story 5.4 API/E2E rerun: `node --test tests/api/story-5-4*.test.mjs tests/e2e/story-5-4*.test.mjs`
- Full repository regression run: `npm run --silent test`
- Compatibility rerun for Story 5.1 static contract after FR42 key-resolution integration: `node --test tests/e2e/story-5-1-reward-risk-policy-gating.e2e.test.mjs`
- QA automation rerun after Story 5.4 API critical-flow coverage expansion: `source "$HOME/.cargo/env" && node --test tests/api/story-5-4*.test.mjs tests/e2e/story-5-4*.test.mjs && npm run --silent qa:test:story-5-4`

### Completion Notes List

- Implemented FR42 domain contracts in `crates/domain/src/risk.rs` for canonical `core`/`satellite` bucket parsing, deterministic identifier normalization, policy-link resolution, and fail-closed reason taxonomy.
- Added `market_bucket_profiles` forward-only migration and Postgres adapter with canonical constraints, one-active-mapping uniqueness, deterministic ordering, and machine-readable persistence error mapping.
- Extended governance market-policy orchestration with bucket-profile upsert/read flows, lock-protected mutation semantics, role enforcement, and actor/correlation/timestamp evidence continuity.
- Added authenticated control-api mutation/read routes for `/control/market-policy/buckets/{market_id}/{cluster_id}` with canonical accepted/error envelopes and deterministic HTTP status mapping (`400/403/409/503/500`).
- Integrated stratification into risk-engine runtime hydration and pre-trade evaluation so effective risk policy key resolves from active market bucket mapping before exposure/reward-risk/FR41 checks.
- Preserved fail-closed runtime behavior for unavailable stratification state using `pretrade_stratification_state_unavailable` and control-uncertainty emergency signal compatibility.
- Added Story 5.4 API/E2E static regression tests and `qa:test:story-5-4` command wiring; refreshed Story 5.1 static E2E assertion to match effective-profile-key integration.
- Published FR42 runbook and linked related operations guides (`market-policy`, `risk-limit`, `allocation-rebalance`, `reward-risk`, `regime-shift`, `pretrade-gate`) for operator continuity.
- Completed code-review remediation by aligning risk-limit bootstrap hydration to resolved FR42 bucket policy keys and emitting explicit deny telemetry for bucket-profile read misses.
- Cross-checked story file list against git reality; only `.scripts/bmad-auto/copilot/bmad-progress.log` is outside Story 5.4 scope and remains intentionally excluded from application review/fix scope.
- Expanded Story 5.4 API static QA coverage with deterministic assertions for route input-contract wiring and explicit unknown-reason `500` fallback mapping, then reran Story 5.4 API/E2E + `qa:test:story-5-4` successfully while keeping story status `done`.

### File List

- _bmad-output/implementation-artifacts/sprint-status.yaml
- _bmad-output/implementation-artifacts/stories/5-4-configure-core-satellite-market-stratification-policies.md
- _bmad-output/implementation-artifacts/tests/test-summary.md
- crates/domain/src/risk.rs
- crates/persistence/migrations/20260407133000_market_bucket_profiles.sql
- crates/persistence/src/postgres/mod.rs
- crates/persistence/src/postgres/market_bucket_profiles.rs
- docs/operations/allocation-rebalance-workflows.md
- docs/operations/core-satellite-market-stratification.md
- docs/operations/incentive-regime-shift-alerts.md
- docs/operations/market-policy-engine.md
- docs/operations/pretrade-gate-pipeline.md
- docs/operations/reward-risk-policy-operations.md
- docs/operations/risk-limit-policy-operations.md
- package.json
- services/control-api/src/routes/mod.rs
- services/governance-service/src/market_policy/mod.rs
- services/risk-engine/src/gates/mod.rs
- services/risk-engine/src/main.rs
- tests/api/story-5-4-market-bucket-stratification-api.test.mjs
- tests/e2e/story-5-1-reward-risk-policy-gating.e2e.test.mjs
- tests/e2e/story-5-4-market-bucket-stratification-gating.e2e.test.mjs

### Change Log

- 2026-04-07: Implemented Story 5.4 FR42 core/satellite stratification end-to-end (domain contracts, `market_bucket_profiles` schema/adapter, governance/control routes, risk-engine stratification enforcement, QA command wiring, and story-scoped API/E2E tests).
- 2026-04-07: Published FR42 operations runbook and continuity cross-links across market-policy/risk-limit/allocation/reward-risk/regime-shift/pretrade runbooks.
- 2026-04-07: Completed Story 5.4 QA + full repository regression validation and advanced the story to `review`.
- 2026-04-07: Code-review auto-fix pass resolved runtime risk-limit hydration/profile-key consistency and added deny telemetry on bucket-profile read misses; status advanced from `review` to `done`.
- 2026-04-07: QA automation rerun expanded Story 5.4 API static coverage for deterministic orchestrator input wiring and unknown-reason `500` fallback handling; reran API/E2E and `qa:test:story-5-4` with all checks passing; story status remains `done`.
