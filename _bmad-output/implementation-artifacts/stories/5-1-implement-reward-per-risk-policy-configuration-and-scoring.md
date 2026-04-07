# Story 5.1: Implement Reward-Per-Risk Policy Configuration and Scoring

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an operator,  
I want configurable reward-per-risk scoring policies,  
so that participation focuses on high expected-value opportunities.

## Acceptance Criteria

1. **Scenario A - FR39 reward-per-risk baseline (story-local BDD):**  
   **Given** reward, rebate, cost, and volatility inputs are available  
   **When** strategy routing policy is evaluated  
   **Then** reward-per-risk scores are computed and compared to configurable thresholds  
   **And** output is enforceable for FR39 deployment decisions.
2. **Scenario B - deterministic formula application:**  
   **Given** `expected_reward_bps`, `maker_rebate_bps`, `expected_cost_bps`, and `expected_volatility_bps`  
   **When** reward-per-risk is computed  
   **Then** score is calculated as `(expected_reward_bps + maker_rebate_bps - expected_cost_bps) / expected_volatility_bps` using deterministic numeric handling  
   **And** invalid or non-finite inputs are rejected with machine-readable errors.
3. **Scenario C - threshold defaults and overrides:**  
   **Given** per-strategy deployment thresholds are configured or absent  
   **When** a strategy eligibility decision is requested  
   **Then** default threshold `>= 1.2` is applied when no override exists  
   **And** operator-configured overrides are honored without service redeploy.
4. **Scenario D - fail-closed policy enforcement in gating:**  
   **Given** score is below threshold or policy/scoring state is unavailable  
   **When** pre-trade routing eligibility is adjudicated  
   **Then** decision is denied fail-closed  
   **And** denial reason is emitted as explicit machine-readable code in canonical decision envelopes.
5. **Scenario E - policy persistence and audit evidence:**  
   **Given** reward-per-risk policy is created or updated  
   **When** mutation is accepted  
   **Then** policy state is persisted in `reward_risk_policies` with UTC evidence metadata  
   **And** telemetry/audit evidence captures actor, strategy/policy key, reason code, correlation id, and timestamp.
6. **Scenario F - integration continuity with existing Story 2 controls:**  
   **Given** market-universe policy (Story 2.1) and risk-limit runtime state (Story 2.7) are active  
   **When** reward-per-risk checks execute  
   **Then** new checks compose with existing venue and limit gates rather than bypassing or replacing them.
7. **UAC-1 Failure handling:** Invalid payloads, unauthorized mutations, unavailable persistence dependencies, and unavailable runtime policy state return explicit machine-readable errors with no unsafe side effects.
8. **UAC-2 Boundary behavior:** Deterministic, test-covered boundary semantics are defined for `score == threshold`, volatility near-zero handling, and invalid threshold ranges.
9. **UAC-3 Verifiable evidence:** Successful and failed policy mutations and gating outcomes emit timestamped evidence suitable for incident and QA traceability.
10. **Schema/dependency/traceability contract:** Story depends only on `2.1` and `2.7`; schema scope introduces only `reward_risk_policies`; traceability maps explicitly to `FR39` and `NFR1`.
11. **Scope boundary contract:** Story 5.1 delivers reward-per-risk configuration and enforceable scoring only; it must not pre-implement Story 5.2+ regime-alert and guardrail feature sets.

## Tasks / Subtasks

- [x] **Task 1: Define canonical reward-per-risk domain contracts and scoring logic** (AC: 1, 2, 3, 4, 7, 8, 10)
  - [x] Extend `crates/domain/src/risk.rs` with reward-per-risk policy/profile contracts keyed by strategy/policy identifier and deterministic threshold semantics.
  - [x] Add typed scoring input contract for FR39 components (`expected_reward_bps`, `maker_rebate_bps`, `expected_cost_bps`, `expected_volatility_bps`) and deterministic score evaluation helper.
  - [x] Enforce explicit validation for non-finite values, negative volatility, and invalid thresholds with machine-readable field errors.
  - [x] Add/extend reason-code taxonomy for reward-per-risk failures (below-threshold vs unavailable-state) and map cleanly to pre-trade reason surfaces.

- [x] **Task 2: Add forward-only persistence migration and repository for `reward_risk_policies`** (AC: 3, 5, 7, 8, 10)
  - [x] Add migration under `crates/persistence/migrations/` creating only `reward_risk_policies` with canonical identifiers, threshold constraints, actor/correlation evidence fields, and UTC timestamps.
  - [x] Add persistence module (for example `crates/persistence/src/postgres/reward_risk.rs`) and wire through `crates/persistence/src/postgres/mod.rs`.
  - [x] Implement idempotent upsert/read behavior by canonical policy key with explicit constraint/query error mapping.
  - [x] Add persistence tests validating schema scope boundaries, constraints, deterministic reads, and fail-closed error mapping.

- [x] **Task 3: Implement governance-service reward-per-risk orchestration** (AC: 3, 5, 7, 9, 10)
  - [x] Add `services/governance-service/src/reward_risk/mod.rs` and export via `services/governance-service/src/lib.rs`.
  - [x] Mirror existing market-policy orchestration patterns for role validation, deterministic normalization, telemetry emission, and repository interaction.
  - [x] Ensure accepted and denied mutations produce machine-readable service errors plus evidence payloads with actor/correlation/timestamp fields.
  - [x] Preserve fail-closed behavior when persistence/runtime bridges are unavailable.

- [x] **Task 4: Expose authenticated control-plane reward-per-risk endpoints** (AC: 3, 5, 7, 9, 10)
  - [x] Extend `services/control-api/src/routes/mod.rs` with authenticated reward-risk upsert/read endpoints (policy-keyed) using canonical route and envelope conventions.
  - [x] Add request/response payload contracts, explicit status mapping (400/403/409/503/500), and field-level validation error surfaces.
  - [x] Wire control state/orchestrator plumbing in `services/control-api/src/middleware/mod.rs` and `services/control-api/src/main.rs`.
  - [x] Reuse privileged audit append and correlation-aware telemetry patterns from market-policy/risk-limit endpoints.

- [x] **Task 5: Integrate reward-per-risk enforcement into risk-engine gate pipeline** (AC: 1, 2, 4, 6, 7, 8, 9)
  - [x] Extend runtime policy state reader/writer surfaces in `services/risk-engine/src/gates/mod.rs` and bootstrap hydration in `services/risk-engine/src/main.rs` to load reward-risk policy state.
  - [x] Add deterministic reward-per-risk gate evaluation in pre-trade pipeline without regressing existing gate order/behavior from Story 2.8.
  - [x] Map reward-per-risk outcomes into pre-trade reason/gate result taxonomy with explicit unavailable vs ineligible semantics.
  - [x] Ensure unavailable policy/scoring state denies fail-closed and remains alert-compatible through existing emergency-signal mapping patterns.

- [x] **Task 6: Wire FR39 score inputs from existing market snapshot seams without unsafe shortcuts** (AC: 1, 2, 4, 6, 8)
  - [x] Reuse/extend domain market-snapshot seams (`market_stream_tick_to_snapshot` + runtime snapshot state) to carry FR39 component inputs or derived score deterministically.
  - [x] Replace `RISK_ENGINE_BOOTSTRAP_MARKET_REWARD_SCORE`-only assumptions with policy-aligned score computation path (while keeping deterministic bootstrap/test fallback behavior explicit).
  - [x] Ensure score computation path does not silently default on missing volatility/reward components; propagate explicit unavailable reason codes.

- [x] **Task 7: Add Story 5.1 deterministic test coverage and QA command** (AC: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10)
  - [x] Add domain tests for FR39 formula correctness, threshold boundaries (`==`, `<`, `>`), and invalid numeric payload rejection.
  - [x] Add persistence tests for `reward_risk_policies` schema constraints and deterministic repository behavior.
  - [x] Add governance/control-api tests for unauthorized mutation denial, invalid payload handling, accepted mutation evidence, and machine-readable error envelopes.
  - [x] Add risk-engine tests for below-threshold denial, unavailable-state fail-closed behavior, and composition with existing venue/risk-limit gates.
  - [x] Add `qa:test:story-5-1` to root `package.json` and update `_bmad-output/implementation-artifacts/tests/test-summary.md` with Story 5.1 evidence once implemented.

- [x] **Task 8: Add reward-per-risk operations runbook** (AC: 3, 5, 7, 8, 9, 10)
  - [x] Add `docs/operations/reward-risk-policy-operations.md` describing FR39 field units, threshold semantics, formula interpretation, and fail-closed troubleshooting.
  - [x] Document operator workflow for policy rollout/rollback, validation failures, and incident response when reward-risk policy state becomes unavailable.

### Review Findings (Code Review 2026-04-07)

- [x] [Review][Patch] Restore direct `ReportingExportReasonCode::MissingIncidentContext` BAD_REQUEST status mapping expression to preserve Story 4.4 static API contract checks while keeping Story 5.1 reward-risk endpoint changes intact [`services/control-api/src/routes/mod.rs`].

## Dev Notes

### Technical Requirements

- Story objective is FR39 enforceability: configurable reward-per-risk policy plus deterministic gating decisions using formula-based score semantics.
- Dependency and scope contract is fixed:
  - dependencies: `2.1`, `2.7`;
  - schema scope: `reward_risk_policies` only;
  - traceability: `FR39`, `NFR1`.
- Formula and threshold rules are non-negotiable:
  - score formula: `(expected_reward_bps + maker_rebate_bps - expected_cost_bps) / expected_volatility_bps`;
  - default deployment threshold: `>= 1.2` when no strategy override exists.
- Fail-closed guardrail is mandatory for any unavailable or invalid policy/scoring state in control and risk paths.
- **Out of scope for Story 5.1:** FR40 regime-shift alerting, FR41 liquidity/overnight guardrails, and FR42 core/satellite stratification policies.

[Source: _bmad-output/planning-artifacts/epics.md#Story 5.1: Implement Reward-Per-Risk Policy Configuration and Scoring]  
[Source: _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)]  
[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)]  
[Source: _bmad-output/planning-artifacts/prd.md#Incentive Intelligence & Market Regime Management]  
[Source: _bmad-output/planning-artifacts/prd.md#Journey 7 — Incentive Shift Response: Sang, Independent Quant Operator]

### Architecture Compliance

- Preserve bounded ownership and control-plane layering:
  - `control-api` is authenticated mutation/read boundary,
  - `governance-service` owns privileged policy orchestration,
  - `risk-engine` owns trading eligibility decisions and fail-closed gate behavior,
  - `execution-engine` must not mutate policy state directly.
- Keep canonical architecture conventions:
  - snake_case naming for Rust modules and DB identifiers,
  - UTC ISO-8601 timestamps only,
  - canonical `data/meta/error` response envelopes,
  - explicit machine-readable reason codes.
- Preserve safety-first process requirements:
  - unknown or ambiguous runtime policy/scoring state must deny (not allow),
  - no swallowed errors in control/risk paths.

[Source: _bmad-output/planning-artifacts/architecture.md#Architectural Boundaries]  
[Source: _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules]  
[Source: _bmad-output/planning-artifacts/architecture.md#Format Patterns]  
[Source: _bmad-output/planning-artifacts/architecture.md#Process Patterns]  
[Source: _bmad-output/planning-artifacts/architecture.md#Integration Points]

### Library & Framework Requirements

- Keep workspace-pinned dependencies for Story 5.1 implementation compatibility:
  - `axum = 0.8.8`
  - `sqlx = 0.8.6`
  - `tokio = 1.48.0`
  - `time = 0.3.44`
  - `polymarket-client-sdk = 0.4.4`
- Latest checks at story creation time:
  - `axum` latest stable remains `0.8.8`,
  - `sqlx` newest is `0.9.0-alpha.1` (pre-release), stable remains `0.8.6`,
  - `tokio` latest stable `1.51.0`,
  - `time` latest stable `0.3.47`,
  - `polymarket-client-sdk` latest stable `0.4.4`.
- Do not perform opportunistic dependency upgrades in Story 5.1.

[Source: Cargo.toml]  
[Source: source $HOME/.cargo/env && cargo search axum --limit 1]  
[Source: source $HOME/.cargo/env && cargo search sqlx --limit 1]  
[Source: source $HOME/.cargo/env && cargo search tokio --limit 1]  
[Source: source $HOME/.cargo/env && cargo search time --limit 1]  
[Source: source $HOME/.cargo/env && cargo search polymarket-client-sdk --limit 1]

### File Structure Requirements

- Primary implementation surfaces for Story 5.1:
  - `crates/domain/src/{risk.rs,lib.rs}`
  - `crates/persistence/migrations/*reward_risk_policies*.sql`
  - `crates/persistence/src/postgres/{mod.rs,reward_risk.rs}`
  - `services/governance-service/src/{lib.rs,reward_risk/mod.rs}`
  - `services/control-api/src/{routes/mod.rs,middleware/mod.rs,main.rs}`
  - `services/risk-engine/src/{main.rs,gates/mod.rs}`
  - `package.json`
  - `_bmad-output/implementation-artifacts/tests/test-summary.md`
  - `docs/operations/reward-risk-policy-operations.md`
- Reuse established Story 2 vertical-slice pattern: domain -> migration -> persistence -> governance orchestration -> control-api -> risk-engine gating -> tests/runbook.
- Keep schema/story boundaries strict; do not introduce Story 5.2+ entities in Story 5.1 migration scope.

[Source: _bmad-output/planning-artifacts/architecture.md#Complete Project Directory Structure]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: crates/persistence/src/postgres/mod.rs]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: services/control-api/src/middleware/mod.rs]  
[Source: services/governance-service/src/lib.rs]

### Testing Requirements

- Add deterministic coverage for:
  - FR39 formula computation and threshold boundary behavior,
  - strategy default threshold fallback (`>= 1.2`) vs explicit override behavior,
  - invalid input handling (non-finite values, non-positive volatility, malformed identifiers/timestamps),
  - fail-closed denial when reward-risk policy/scoring state is unavailable,
  - composition with existing market-policy + risk-limit + pretrade decision envelope behavior.
- Keep test layering consistent with repository patterns:
  - domain tests in `crates/domain`,
  - persistence migration/adapter tests in `crates/persistence`,
  - governance/control-api route tests in `services/governance-service` and `services/control-api`,
  - risk-engine gate tests in `services/risk-engine`.
- Add story QA command in root `package.json`:
  - `qa:test:story-5-1` should chain targeted Rust tests similar to `qa:test:story-2-7` and `qa:test:story-2-8`.

[Source: package.json]  
[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: _bmad-output/implementation-artifacts/stories/2-7-configure-portfolio-market-and-strategy-limit-policies.md#Testing Requirements]  
[Source: _bmad-output/implementation-artifacts/stories/2-8-enforce-pre-trade-gate-evaluation-pipeline.md#Testing Requirements]

### Previous Story Intelligence

- There is no prior Story 5.x implementation artifact yet; use dependency stories for continuity:
  - Story 2.1 established current market-policy profile and `min_reward_score` gating seams,
  - Story 2.7 established runtime limit-state patterns and fail-closed availability handling,
  - Story 2.8 established deterministic pre-trade gate pipeline and reason-code/evidence patterns.
- Existing code already enforces reward threshold using `MarketSnapshot.reward_score` and `MarketPolicyProfile.min_reward_score`; Story 5.1 should evolve this seam to FR39 formula-backed scoring rather than creating a parallel gate path.
- Keep orchestration and API patterns aligned with existing market-policy flow to prevent duplicate infrastructure.

[Source: _bmad-output/implementation-artifacts/stories/2-1-configure-market-universe-policy-engine.md#Completion Notes List]  
[Source: _bmad-output/implementation-artifacts/stories/2-7-configure-portfolio-market-and-strategy-limit-policies.md#Completion Notes List]  
[Source: _bmad-output/implementation-artifacts/stories/2-8-enforce-pre-trade-gate-evaluation-pipeline.md#Completion Notes List]  
[Source: crates/domain/src/risk.rs]  
[Source: services/risk-engine/src/gates/mod.rs]  
[Source: services/governance-service/src/market_policy/mod.rs]  
[Source: services/control-api/src/routes/mod.rs]

### Git Intelligence Summary

- Recent implementation history shows a stable vertical slice pattern for completed stories:
  1. domain contracts and reason-code taxonomy,
  2. forward-only migration and persistence adapter,
  3. orchestrator + control-plane route wiring,
  4. risk/runtime integration,
  5. QA command and operations handoff docs.
- Story 5.1 should follow this sequence to stay consistent with repository conventions and reduce regression risk.

[Source: git --no-pager log --oneline -5]

### Latest Technical Information

- Current workspace versions are already aligned with Story 5.1 needs; no mandatory dependency upgrades are required.
- `sqlx` newer release is pre-release only (`0.9.0-alpha.1`), so story implementation should remain on stable `0.8.6`.
- Dependency drift risk is low if story changes stay within existing workspace stack.

[Source: Cargo.toml]  
[Source: source $HOME/.cargo/env && cargo search axum --limit 1]  
[Source: source $HOME/.cargo/env && cargo search sqlx --limit 1]  
[Source: source $HOME/.cargo/env && cargo search tokio --limit 1]  
[Source: source $HOME/.cargo/env && cargo search time --limit 1]  
[Source: source $HOME/.cargo/env && cargo search polymarket-client-sdk --limit 1]

### Project Context Reference

- No `project-context.md` file was found during discovery.
- Story context is derived from epics/PRD/architecture/UX artifacts, prior implementation stories, recent git history, and current codebase seams.

### Project Structure Notes

- Current market-policy path is cluster-centric:
  - `MarketPolicyProfile` includes `min_reward_score` and `MarketSnapshot` includes scalar `reward_score`,
  - `evaluate_market_eligibility` blocks on `reward_score < min_reward_score`,
  - pre-trade gate maps market-policy outcomes through venue-eligibility gate reasons.
- Risk-engine bootstrap currently seeds `reward_score` from `RISK_ENGINE_BOOTSTRAP_MARKET_REWARD_SCORE`; Story 5.1 should align runtime scoring to FR39 component inputs and preserve explicit unavailable-state behavior.
- Control-plane route and orchestration seams already exist for policy mutation workflows and should be reused for reward-risk policies.

[Source: crates/domain/src/risk.rs]  
[Source: services/risk-engine/src/gates/mod.rs]  
[Source: services/risk-engine/src/main.rs]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: services/governance-service/src/market_policy/mod.rs]  
[Source: crates/persistence/migrations/20260406004500_market_policy_profiles.sql]

### References

- _bmad-output/planning-artifacts/epics.md#Epic 5: Incentive-Regime Adaptive Trading Controls  
- _bmad-output/planning-artifacts/epics.md#Story 5.1: Implement Reward-Per-Risk Policy Configuration and Scoring  
- _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)  
- _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)  
- _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)  
- _bmad-output/planning-artifacts/prd.md#Journey 7 — Incentive Shift Response: Sang, Independent Quant Operator  
- _bmad-output/planning-artifacts/prd.md#Incentive Intelligence & Market Regime Management  
- _bmad-output/planning-artifacts/prd.md#Non-Functional Requirements  
- _bmad-output/planning-artifacts/architecture.md#Requirements Overview  
- _bmad-output/planning-artifacts/architecture.md#Technical Constraints & Dependencies  
- _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules  
- _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries  
- _bmad-output/planning-artifacts/architecture.md#Architectural Boundaries  
- _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping  
- _bmad-output/planning-artifacts/architecture.md#Integration Points  
- _bmad-output/planning-artifacts/ux-design-specification.md#UX Consistency Patterns  
- _bmad-output/planning-artifacts/ux-design-specification.md#Responsive Design & Accessibility  
- _bmad-output/implementation-artifacts/stories/2-1-configure-market-universe-policy-engine.md  
- _bmad-output/implementation-artifacts/stories/2-7-configure-portfolio-market-and-strategy-limit-policies.md  
- _bmad-output/implementation-artifacts/stories/2-8-enforce-pre-trade-gate-evaluation-pipeline.md  
- crates/domain/src/risk.rs  
- crates/domain/src/lib.rs  
- crates/persistence/migrations/20260406004500_market_policy_profiles.sql  
- crates/persistence/src/postgres/market_policy.rs  
- crates/persistence/src/postgres/mod.rs  
- services/governance-service/src/lib.rs  
- services/governance-service/src/market_policy/mod.rs  
- services/control-api/src/routes/mod.rs  
- services/control-api/src/middleware/mod.rs  
- services/risk-engine/src/gates/mod.rs  
- services/risk-engine/src/main.rs  
- package.json  
- Cargo.toml  
- docs/operations/market-policy-engine.md  
- docs/operations/risk-limit-policy-operations.md  
- git --no-pager log --oneline -5

## Story Completion Status

- Story context generated with exhaustive artifact and codebase analysis, including epics/PRD/architecture/UX, dependency stories, and runtime implementation seams.
- Story file is created and ready for implementation by dev agents.
- Completion note: Ultimate context engine analysis completed - comprehensive developer guide created.

## Dev Agent Record

### Agent Model Used

GPT-5.3-Codex (gpt-5.3-codex)

### Debug Log References

- BMAD create-story workflow execution (automated)
- Sprint backlog discovery from `_bmad-output/implementation-artifacts/sprint-status.yaml`
- Artifact discovery across planning and implementation surfaces
- Recent commit-pattern review via `git --no-pager log --oneline -5`
- Latest crate checks via `cargo search`

### Completion Notes List

- Implemented reward-per-risk domain contracts, FR39 scoring helpers, threshold/default semantics, and pre-trade reason-code taxonomy extensions.
- Added forward-only `reward_risk_policies` migration plus Postgres adapter (`upsert`/`read`) with deterministic validation and adapter-level tests.
- Added governance reward-risk orchestration with role validation, canonical normalization, telemetry, fail-closed persistence handling, and evidence-rich responses.
- Added authenticated control-plane reward-risk upsert/read endpoints with privileged-audit integration, machine-readable error envelopes, and route test coverage.
- Integrated reward-per-risk pre-trade gate enforcement in risk-engine runtime pipeline, including fail-closed unavailable-state handling and emergency-signal compatibility.
- Extended bootstrap snapshot hydration to support FR39 component-driven score computation with explicit legacy fallback behavior and unavailable-state propagation.
- Added Story 5.1 QA command, updated automated test summary evidence, and published reward-risk operations runbook.
- Completed adversarial code-review pass and auto-fixed a medium-severity cross-story contract regression in `report_export_service_error_status` so full-repository API contract tests remain green.
- Generated Story 5.1 API/E2E QA automation files (`tests/api/story-5-1-reward-risk-policy-api.test.mjs`, `tests/e2e/story-5-1-reward-risk-policy-gating.e2e.test.mjs`), extended `qa:test:story-5-1` to execute them, and refreshed QA evidence inventory to include 8 additional regression checks.
- File-list cross-check discrepancy: `.scripts/bmad-auto/copilot/bmad-progress.log` is modified in git state but intentionally excluded from story application-code review scope.

### File List

- crates/domain/src/risk.rs
- crates/persistence/migrations/20260407090000_reward_risk_policies.sql
- crates/persistence/src/postgres/mod.rs
- crates/persistence/src/postgres/reward_risk.rs
- services/governance-service/src/lib.rs
- services/governance-service/src/reward_risk/mod.rs
- services/control-api/src/main.rs
- services/control-api/src/middleware/mod.rs
- services/control-api/src/routes/mod.rs
- services/risk-engine/src/gates/mod.rs
- services/risk-engine/src/main.rs
- package.json
- tests/api/story-5-1-reward-risk-policy-api.test.mjs
- tests/e2e/story-5-1-reward-risk-policy-gating.e2e.test.mjs
- docs/operations/reward-risk-policy-operations.md
- _bmad-output/implementation-artifacts/sprint-status.yaml
- _bmad-output/implementation-artifacts/tests/test-summary.md
- _bmad-output/implementation-artifacts/stories/5-1-implement-reward-per-risk-policy-configuration-and-scoring.md

### Change Log

- 2026-04-07: Completed adversarial code review, auto-fixed the identified medium-severity issue, re-ran Story 5.1 QA and full-repository tests, and advanced story status to `done`.
- 2026-04-07: Added Story 5.1 API/E2E automation coverage for reward-risk control-plane and gate integration flows, updated `qa:test:story-5-1`, and re-ran Story 5.1 QA suite with passing results.
