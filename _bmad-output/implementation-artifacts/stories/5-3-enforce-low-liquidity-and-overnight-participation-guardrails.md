# Story 5.3: Enforce Low-Liquidity and Overnight Participation Guardrails

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As a risk-aware trader,  
I want automatic participation guardrails under weak liquidity conditions,  
so that order behavior de-risks during fragile windows.

## Acceptance Criteria

1. **Scenario A - Story-local BDD baseline:**  
   **Given** depth and inactivity metrics breach configured limits  
   **When** a new quote/order is evaluated  
   **Then** participation is paused or size-capped according to guardrail policy  
   **And** behavior satisfies FR41 constraints.

2. **Scenario B - low-liquidity pause threshold semantics:**  
   **Given** top-of-book depth is available for the target market  
   **When** `liquidity_depth_usd < 10_000`  
   **Then** new quote/order participation is denied with a machine-readable FR41 pause reason code  
   **And** the exact boundary `liquidity_depth_usd == 10_000` is not treated as low-liquidity breach.

3. **Scenario C - short-gap inactivity pause semantics:**  
   **Given** trade-activity recency is available for the target market  
   **When** `inactivity_gap_seconds > 900` and `<= 14_400`  
   **Then** new quote/order participation is denied with a machine-readable FR41 inactivity-pause reason  
   **And** exact boundary `inactivity_gap_seconds == 900` does not trigger pause.

4. **Scenario D - overnight-gap size-cap semantics:**  
   **Given** trade-activity recency is available and overnight-gap condition is present  
   **When** `inactivity_gap_seconds > 14_400`  
   **Then** participation remains constrained by max order size = `25%` of normal order size baseline  
   **And** exact boundary `inactivity_gap_seconds == 14_400` does not trigger the overnight cap mode.

5. **Scenario E - deterministic guardrail precedence and conflict handling:**  
   **Given** multiple FR41 guardrail conditions can co-occur  
   **When** guardrail evaluation executes  
   **Then** precedence is deterministic and test-covered: low-liquidity pause supersedes inactivity-derived cap  
   **And** overnight size-cap mode applies only when pause conditions are not active.

6. **Scenario F - machine-readable enforcement and evidence contract:**  
   **Given** FR41 guardrail evaluation completes  
   **When** decision/evidence is emitted  
   **Then** payloads include normalized `reason_code`, `guardrail_mode`, `market_id`, `cluster_id`, `correlation_id`, `observed_at_utc`, and threshold context  
   **And** size-cap decisions include `normal_max_order_size_units` and `capped_max_order_size_units`.

7. **Scenario G - guardrail event persistence:**  
   **Given** a guardrail decision is produced  
   **When** persistence succeeds  
   **Then** an evidence row is stored in `participation_guardrail_events` with mode/reason/threshold/value context and UTC timestamps  
   **And** rows are queryable deterministically for incident and QA traceability.

8. **Scenario H - pre-trade pipeline composition contract:**  
   **Given** Story 2.8 pre-trade gate sequencing and Story 5.1 reward-risk gate are active  
   **When** FR41 logic is introduced  
   **Then** FR41 composes into the existing pipeline without bypassing prior gates  
   **And** denied outcomes continue to preserve single-failed-gate deterministic reason semantics.

9. **Scenario I - fail-closed unavailable-state behavior (NFR5/NFR12):**  
   **Given** required dependencies for FR41 evaluation are missing or stale (depth, inactivity baseline, or normal-order-size baseline)  
   **When** a new order/quote evaluation is requested  
   **Then** system fails closed with explicit machine-readable unavailable reason code  
   **And** no success-shaped response or unsafe side effects are emitted.

10. **Scenario J - execution-path cap enforcement continuity:**  
    **Given** risk adjudication returns FR41 overnight size-cap mode  
    **When** submit order size exceeds the capped maximum  
    **Then** submit flow rejects with explicit machine-readable reason  
    **And** no order lifecycle side effects are persisted for the rejected submit.

11. **UAC-1 Failure handling:** Invalid payloads, malformed timestamps/identifiers, and unavailable dependency paths must return explicit machine-readable errors with no unsafe side effects.

12. **UAC-2 Boundary behavior:** FR41 thresholds (`<10k`, `>15m`, `>4h`, and `25%` cap) must be deterministic, documented, and test-covered for exact boundary equality and strict inequality behavior.

13. **UAC-3 Verifiable evidence:** Successful and failed FR41 guardrail evaluations must emit timestamped telemetry/persistence evidence suitable for incident and QA traceability.

14. **Schema/dependency/traceability contract:** Story depends only on `5.2`; schema scope introduces only `participation_guardrail_events`; traceability maps explicitly to `FR41`, `NFR5`, and `NFR12`.

15. **Scope boundary contract:** Story 5.3 delivers participation guardrail enforcement and evidence only; Story 5.4 core/satellite stratification policy behavior remains out of scope.

## Tasks / Subtasks

- [x] **Task 1: Define canonical FR41 participation-guardrail contracts and reason-code taxonomy** (AC: 1, 2, 3, 4, 5, 6, 9, 12, 14)
  - [x] Extend `crates/domain/src/risk.rs` with FR41 guardrail mode/reason contracts (pause vs size-cap vs unavailable) and deterministic threshold constants.
  - [x] Add typed guardrail evaluation input/output contracts including `liquidity_depth_usd`, `inactivity_gap_seconds`, and optional normal/capped order-size evidence fields.
  - [x] Extend `PreTradeGateDimension` and `PreTradeReasonCode` for FR41 outcomes without breaking existing parse/validation invariants.
  - [x] Add deterministic domain validation for UTC timestamps, finite numeric values, and precedence behavior with explicit field-level machine-readable errors.

- [x] **Task 2: Add persistence migration and adapter for `participation_guardrail_events`** (AC: 6, 7, 9, 12, 14)
  - [x] Add forward-only migration under `crates/persistence/migrations/` creating only `participation_guardrail_events` with canonical IDs, FR41 mode/reason fields, threshold/value evidence, and UTC constraints.
  - [x] Add Postgres adapter module (for example `crates/persistence/src/postgres/participation_guardrail_events.rs`) and wire via `crates/persistence/src/postgres/mod.rs`.
  - [x] Add deterministic indexes for market/time, reason/time, and correlation/time triage.
  - [x] Add persistence tests validating schema-scope isolation, constraints, decode behavior, and machine-readable adapter error mapping.

- [x] **Task 3: Integrate FR41 evaluation into risk-engine pre-trade gating pipeline** (AC: 1, 2, 3, 4, 5, 8, 9, 10)
  - [x] Add `evaluate_pretrade_participation_guardrail_gate(...)` in `services/risk-engine/src/gates/mod.rs` and wire it into deterministic gate order without regressing existing Story 2.8 + 5.1 behavior.
  - [x] Ensure FR41 guardrail failures map to canonical deny reason codes; overnight cap mode emits structured cap metadata.
  - [x] Preserve fail-closed behavior when FR41 dependencies are unavailable and keep emergency-signal compatibility aligned with existing control-uncertainty patterns.
  - [x] Extend runtime state management in `services/risk-engine/src/{gates/mod.rs,main.rs}` to hydrate/support FR41 required inputs.

- [x] **Task 4: Bridge FR41 inactivity and baseline-size inputs from existing seams (no parallel stack)** (AC: 2, 3, 4, 5, 9, 12)
  - [x] Reuse market snapshot/runtime seams for trade recency and depth rather than introducing a duplicate market-state pipeline.
  - [x] Reuse active risk-limit inventory rules (`max_order_size_units`) as normal-size baseline for 25% overnight cap computation.
  - [x] Explicitly fail closed when required baseline-size context is missing or invalid (no silent fallback defaults).

- [x] **Task 5: Extend adjudication/submit contracts to enforce capped-order behavior** (AC: 4, 5, 6, 8, 9, 10, 12)
  - [x] Extend order submit and adjudication contract surfaces (notably `services/execution-engine/src/orders/mod.rs`) to carry requested order size needed for FR41 cap enforcement.
  - [x] Ensure capped-mode submissions above allowed size are rejected with machine-readable FR41 reason and no lifecycle side effects.
  - [x] Preserve current deny/pass invariants (`deny` cannot use pass reason, `allow` must use pass reason) after FR41 contract extension.

- [x] **Task 6: Add authenticated guardrail evidence retrieval surface in control-api** (AC: 6, 7, 11, 13)
  - [x] Add guarded query route (for example `GET /control/incidents/participation-guardrails`) in `services/control-api/src/routes/mod.rs` with canonical `data/meta/error` envelopes.
  - [x] Support deterministic filters (`market_id`, `reason_code`, `correlation_id`, `start_ts`, `end_ts`, `limit`) and stable result ordering.
  - [x] Ensure role checks, privileged audit append, and machine-readable dependency/unavailable errors follow established Story 5.2 query conventions.

- [x] **Task 7: Add Story 5.3 deterministic QA coverage and command wiring** (AC: 1-15)
  - [x] Add domain tests for FR41 thresholds, strict-boundary behavior, and precedence (`pause` vs `size_cap`).
  - [x] Add risk-engine tests for new pre-trade gate ordering, fail-closed unavailable-state behavior, and emergency-signal compatibility.
  - [x] Add persistence tests for `participation_guardrail_events` migration/adapter behavior.
  - [x] Add execution-engine tests confirming capped-order denial paths produce no submit side effects.
  - [x] Add `tests/api/story-5-3-*.test.mjs` and `tests/e2e/story-5-3-*.test.mjs`, then wire `qa:test:story-5-3` in root `package.json`.

- [x] **Task 8: Publish FR41 operations runbook and cross-links** (AC: 6, 7, 10, 13)
  - [x] Add `docs/operations/low-liquidity-overnight-guardrails.md` documenting thresholds, precedence rules, cap math, and failure remediation.
  - [x] Cross-link `docs/operations/pretrade-gate-pipeline.md`, `docs/operations/risk-limit-policy-operations.md`, and `docs/operations/incentive-regime-shift-alerts.md` for operator continuity.
  - [x] Update `_bmad-output/implementation-artifacts/tests/test-summary.md` with Story 5.3 evidence entries after implementation.

### Review Findings

- [x] [Review][Patch] Remove implicit fallback to unrelated inventory rules when deriving FR41 overnight baseline size [services/risk-engine/src/limits/mod.rs].
- [x] [Review][Patch] Add deterministic regression coverage proving FR41 fails closed when no market/strategy inventory baseline matches [services/risk-engine/src/{limits/mod.rs,gates/mod.rs}].
- [x] [Review][Dismiss] Duplicate FR41 size-cap checks in execution adjudication were retained intentionally as defense-in-depth across adapter and service-layer normalization paths.
- [x] [Review][Info] Story file list matches application-code changes; `.scripts/bmad-auto/copilot/bmad-progress.log` is an unrelated automation artifact and was excluded from code review scope.

## Dev Notes

### Technical Requirements

- Story objective is FR41 participation protection under weak market conditions:
  - pause new quotes/orders when depth is below `$10,000`,
  - pause for short inactivity windows (`>15m` and `<=4h`),
  - apply `25%` normal-size cap for overnight inactivity windows (`>4h`) where pause conditions are not active.
- Story dependency and scope contract:
  - dependency: `5.2`,
  - schema scope: `participation_guardrail_events` only,
  - traceability: `FR41`, `NFR5`, `NFR12`.
- FR41 execution must remain deterministic and fail-closed:
  - strict threshold inequalities (`<`, `>`) with equality boundary coverage,
  - explicit precedence between pause and size-cap outcomes,
  - no silent defaulting when required state is missing.
- **Out of scope for Story 5.3:** FR42 core/satellite bucket policy behavior (Story 5.4) and new strategy allocation frameworks.

[Source: _bmad-output/planning-artifacts/epics.md#Story 5.3: Enforce Low-Liquidity and Overnight Participation Guardrails]  
[Source: _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)]  
[Source: _bmad-output/planning-artifacts/prd.md#Incentive Intelligence & Market Regime Management]  
[Source: _bmad-output/planning-artifacts/prd.md#Journey 7 — Incentive Shift Response: Sang, Independent Quant Operator]

### Architecture Compliance

- Preserve bounded responsibilities:
  - `risk-engine` owns FR41 gating/evaluation decisions,
  - `execution-engine` enforces submit-path side effects and contract invariants,
  - `crates/persistence` owns durable evidence rows,
  - `control-api` remains authenticated query/control boundary.
- Keep canonical architecture/process rules:
  - machine-readable reason/error contracts,
  - UTC ISO-8601 timestamps,
  - deterministic gate order and reason precedence,
  - fail-closed behavior for ambiguous/unavailable state.
- Reuse existing seams rather than introducing parallel pipelines:
  - market snapshot/runtime inputs,
  - risk-limit inventory-rule baseline for normal order size,
  - pre-trade decision persistence and adjudication flow.

[Source: _bmad-output/planning-artifacts/architecture.md#Architectural Boundaries]  
[Source: _bmad-output/planning-artifacts/architecture.md#Integration Points]  
[Source: _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules]  
[Source: docs/operations/pretrade-gate-pipeline.md]

### Library & Framework Requirements

- Keep workspace-pinned dependencies for compatibility:
  - `axum = 0.8.8`
  - `sqlx = 0.8.6`
  - `tokio = 1.48.0`
  - `time = 0.3.44`
  - `polymarket-client-sdk = 0.4.4`
- Latest checks at story creation time:
  - `axum` latest stable remains `0.8.8`,
  - `sqlx` latest listed release is pre-release `0.9.0-alpha.1`; stay on stable `0.8.6`,
  - `tokio` latest stable is `1.51.0`,
  - `time` latest stable is `0.3.47`,
  - `polymarket-client-sdk` latest stable remains `0.4.4`.
- Do not perform opportunistic dependency upgrades in Story 5.3.

[Source: Cargo.toml]  
[Source: source $HOME/.cargo/env && cargo search axum --limit 1]  
[Source: source $HOME/.cargo/env && cargo search sqlx --limit 1]  
[Source: source $HOME/.cargo/env && cargo search tokio --limit 1]  
[Source: source $HOME/.cargo/env && cargo search time --limit 1]  
[Source: source $HOME/.cargo/env && cargo search polymarket-client-sdk --limit 1]

### File Structure Requirements

- Primary implementation surfaces for Story 5.3:
  - `crates/domain/src/risk.rs`
  - `crates/persistence/migrations/*participation_guardrail_events*.sql`
  - `crates/persistence/src/postgres/{mod.rs,participation_guardrail_events.rs}`
  - `crates/persistence/src/postgres/pretrade_gate.rs` (if pre-trade evidence contract is extended)
  - `services/risk-engine/src/{gates/mod.rs,main.rs}`
  - `services/execution-engine/src/orders/mod.rs`
  - `services/control-api/src/routes/mod.rs`
  - `docs/operations/low-liquidity-overnight-guardrails.md`
  - `tests/api/story-5-3-*.test.mjs`
  - `tests/e2e/story-5-3-*.test.mjs`
  - `package.json`
  - `_bmad-output/implementation-artifacts/tests/test-summary.md`
- Reuse established vertical-slice execution sequence:
  - domain contracts -> migration -> persistence adapter -> risk/runtime + execution integration -> control-api evidence retrieval -> tests -> runbook.
- Keep schema/story boundaries strict; do not introduce Story 5.4 entities in Story 5.3 migration scope.

[Source: _bmad-output/planning-artifacts/architecture.md#Complete Project Directory Structure]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: _bmad-output/implementation-artifacts/stories/5-2-add-incentive-and-regime-shift-detection-alerts.md#File Structure Requirements]  
[Source: crates/persistence/src/postgres/risk_limits.rs]  
[Source: services/risk-engine/src/gates/mod.rs]  
[Source: services/execution-engine/src/orders/mod.rs]

### Testing Requirements

- Add deterministic coverage for:
  - FR41 low-liquidity threshold (`<10k`) and exact-boundary behavior (`==10k`),
  - inactivity windows (`>15m`, `>4h`) and exact-boundary behavior (`==15m`, `==4h`),
  - precedence logic (`pause` over `size_cap` when both candidate conditions exist),
  - cap math (`capped_max_order_size_units == normal_max_order_size_units * 0.25`),
  - fail-closed unavailable-state behavior for missing depth/inactivity/baseline-size dependencies,
  - submit-path rejection behavior when capped-mode requests exceed allowed size.
- Keep test layering aligned with repository conventions:
  - domain tests in `crates/domain`,
  - migration/adapter tests in `crates/persistence`,
  - gate pipeline tests in `services/risk-engine`,
  - submit-path enforcement tests in `services/execution-engine`,
  - API/E2E assertions in `tests/api` and `tests/e2e`.
- Add Story QA command in root `package.json`:
  - `qa:test:story-5-3` should chain story-scoped Rust + Node tests consistent with existing `qa:test:story-*` conventions.

[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: docs/operations/pretrade-gate-pipeline.md]  
[Source: package.json]  
[Source: _bmad-output/implementation-artifacts/stories/5-1-implement-reward-per-risk-policy-configuration-and-scoring.md#Testing Requirements]  
[Source: _bmad-output/implementation-artifacts/stories/5-2-add-incentive-and-regime-shift-detection-alerts.md#Testing Requirements]

### Previous Story Intelligence

- Story 5.1 established reward-per-risk gate placement and fail-closed unavailable-state handling inside pre-trade evaluation.
- Story 5.2 established FR40 regime-shift detection/evidence patterns and control-plane retrieval/dispatch conventions.
- Story 2.8 established deterministic pre-trade gate sequencing and single-failed-gate deny semantics.
- Story 2.7 established inventory-rule baseline (`max_order_size_units`) that should be reused for FR41 overnight cap normalization.
- Existing seam gap to resolve in Story 5.3: submit/adjudication contracts currently do not carry explicit requested order size, so FR41 cap enforcement needs additive contract extension rather than implicit assumptions.

[Source: _bmad-output/implementation-artifacts/stories/5-1-implement-reward-per-risk-policy-configuration-and-scoring.md#Project Structure Notes]  
[Source: _bmad-output/implementation-artifacts/stories/5-2-add-incentive-and-regime-shift-detection-alerts.md#Project Structure Notes]  
[Source: docs/operations/pretrade-gate-pipeline.md#Gate Evaluation Order]  
[Source: crates/domain/src/risk.rs]  
[Source: crates/persistence/src/postgres/risk_limits.rs]  
[Source: services/execution-engine/src/orders/mod.rs]

### Git Intelligence Summary

- Recent commits (`4-2` through `5-2`) follow a stable delivery pattern:
  1. domain contracts and reason-code updates,
  2. forward-only migration plus persistence adapter,
  3. runtime/service/control wiring,
  4. story-scoped QA command/tests and runbook updates.
- Story 5.3 should follow the same sequence to reduce regression risk and maintain consistency.

[Source: git --no-pager log --oneline -5]  
[Source: git --no-pager log -5 --name-only --pretty=format:'%h %s']

### Latest Technical Information

- Current workspace versions are already compatible with Story 5.3 scope; no mandatory dependency upgrades are required.
- `sqlx` latest indexed release is pre-release (`0.9.0-alpha.1`), so story implementation should remain on stable `0.8.6`.
- Dependency drift risk remains low when changes stay within existing architecture seams.

[Source: Cargo.toml]  
[Source: source $HOME/.cargo/env && cargo search axum --limit 1]  
[Source: source $HOME/.cargo/env && cargo search sqlx --limit 1]  
[Source: source $HOME/.cargo/env && cargo search tokio --limit 1]  
[Source: source $HOME/.cargo/env && cargo search time --limit 1]  
[Source: source $HOME/.cargo/env && cargo search polymarket-client-sdk --limit 1]

### Project Context Reference

- No `project-context.md` file was found during discovery.
- Story context is derived from epics/PRD/architecture/UX artifacts, prior implementation stories, operations runbooks, and current code seams.

### Project Structure Notes

- Existing FR41-adjacent seams already available:
  - market depth + observed timestamps in `MarketSnapshot` and runtime snapshot state,
  - deterministic pre-trade gate pipeline with machine-readable reason codes,
  - risk-limit inventory rules exposing `max_order_size_units`,
  - control-plane evidence query patterns from Story 5.2.
- Existing contract variance requiring explicit resolution:
  - submit/adjudication runtime structs currently lack requested order-size field needed for strict FR41 `25%` cap enforcement.
- Guardrail implementation should extend current seams rather than introduce separate market-state or alert stacks.

[Source: crates/domain/src/risk.rs]  
[Source: services/risk-engine/src/gates/mod.rs]  
[Source: crates/persistence/src/postgres/risk_limits.rs]  
[Source: services/execution-engine/src/orders/mod.rs]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: docs/operations/pretrade-gate-pipeline.md]  
[Source: docs/operations/incentive-regime-shift-alerts.md]

### References

- _bmad-output/planning-artifacts/epics.md#Epic 5: Incentive-Regime Adaptive Trading Controls  
- _bmad-output/planning-artifacts/epics.md#Story 5.3: Enforce Low-Liquidity and Overnight Participation Guardrails  
- _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)  
- _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)  
- _bmad-output/planning-artifacts/prd.md#Incentive Intelligence & Market Regime Management  
- _bmad-output/planning-artifacts/prd.md#Journey 7 — Incentive Shift Response: Sang, Independent Quant Operator  
- _bmad-output/planning-artifacts/prd.md#Non-Functional Requirements  
- _bmad-output/planning-artifacts/architecture.md#Architectural Boundaries  
- _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules  
- _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping  
- _bmad-output/planning-artifacts/ux-design-specification.md#Component Strategy  
- _bmad-output/planning-artifacts/ux-design-specification.md#Feedback Patterns  
- _bmad-output/implementation-artifacts/stories/5-1-implement-reward-per-risk-policy-configuration-and-scoring.md  
- _bmad-output/implementation-artifacts/stories/5-2-add-incentive-and-regime-shift-detection-alerts.md  
- docs/operations/pretrade-gate-pipeline.md  
- docs/operations/risk-limit-policy-operations.md  
- docs/operations/reward-risk-policy-operations.md  
- docs/operations/incentive-regime-shift-alerts.md  
- docs/operations/emergency-safe-state-controls.md  
- crates/domain/src/risk.rs  
- crates/persistence/src/postgres/{risk_limits.rs,pretrade_gate.rs,regime_shift_alerts.rs}  
- services/risk-engine/src/{main.rs,gates/mod.rs}  
- services/execution-engine/src/orders/mod.rs  
- services/control-api/src/routes/mod.rs  
- package.json  
- Cargo.toml  
- git --no-pager log --oneline -5  
- git --no-pager log -5 --name-only --pretty=format:'%h %s'

## Story Completion Status

- Story 5.3 implementation is complete and all tasks/subtasks are verified.
- FR41 guardrail contracts, persistence evidence, risk/execution/control integration, and operator docs are delivered and wired.
- Story-scoped QA plus full build/test regression passes are recorded in implementation artifacts.

## Dev Agent Record

### Agent Model Used

GPT-5.3-Codex (gpt-5.3-codex)

### Debug Log References

- BMAD dev-story workflow execution (automated, non-interactive resume from `in-progress`)
- Story-scoped QA run: `npm run --silent qa:test:story-5-3`
- QA automation refresh rerun: `node --test tests/api/story-5-3*.test.mjs tests/e2e/story-5-3*.test.mjs`
- Full regression/build run: `npm run --silent rust:build && npm run --silent test`
- Formatting/quality checks: `cargo fmt --all` and `npm run --silent rust:lint` (lint reports pre-existing unrelated clippy debt in `crates/domain/src/recovery.rs`)

### Completion Notes List

- Implemented FR41 domain contracts, threshold constants, deterministic precedence logic, fail-closed dependency validation, and participation evidence validation.
- Added `participation_guardrail_events` migration + Postgres adapter with canonical validation, deterministic sort/index contracts, and machine-readable error mapping.
- Integrated FR41 into risk-engine gate order and persistence path, including unavailable-mode fail-closed evidence and emergency control-uncertainty compatibility.
- Extended execution submit/adjudication contracts with requested size and deterministic overnight cap enforcement to deny oversize submits without side effects.
- Added authenticated control-api evidence retrieval route and response contracts for FR41 incident triage filters.
- Published FR41 operations runbook and continuity cross-links; added Story 5.3 QA command plus Rust/Node deterministic regression coverage.
- Code-review patch hardening: removed silent FR41 baseline fallback behavior and enforced strict market/strategy baseline matching before overnight cap evaluation.
- Added post-review regression tests for strict baseline matching and fail-closed FR41 unavailable behavior under overnight conditions.
- Refreshed Story 5.3 API/E2E automation with deterministic accepted-envelope assertions and FR41 threshold/precedence coverage for pause-versus-cap guardrail behavior.

### File List

- _bmad-output/implementation-artifacts/sprint-status.yaml
- _bmad-output/implementation-artifacts/stories/5-3-enforce-low-liquidity-and-overnight-participation-guardrails.md
- _bmad-output/implementation-artifacts/tests/test-summary.md
- crates/domain/src/risk.rs
- crates/persistence/migrations/20260407113000_participation_guardrail_events.sql
- crates/persistence/src/postgres/mod.rs
- crates/persistence/src/postgres/participation_guardrail_events.rs
- crates/persistence/src/postgres/pretrade_gate.rs
- docs/operations/incentive-regime-shift-alerts.md
- docs/operations/low-liquidity-overnight-guardrails.md
- docs/operations/pretrade-gate-pipeline.md
- docs/operations/risk-limit-policy-operations.md
- package.json
- services/control-api/src/routes/mod.rs
- services/execution-engine/src/orders/mod.rs
- services/risk-engine/src/gates/mod.rs
- services/risk-engine/src/limits/mod.rs
- services/risk-engine/src/main.rs
- tests/api/story-5-3-participation-guardrails-api.test.mjs
- tests/e2e/story-5-1-reward-risk-policy-gating.e2e.test.mjs
- tests/e2e/story-5-3-participation-guardrails-gating.e2e.test.mjs

### Change Log

- 2026-04-07: Implemented Story 5.3 FR41 participation guardrails end-to-end (domain contracts, persistence evidence table/adapter, risk/execution/control integration, deterministic QA coverage, runbook, and story-scoped QA command wiring).
- 2026-04-07: Code review hardening pass fixed FR41 baseline-selection fail-closed gap and added regression coverage in `services/risk-engine/src/limits/mod.rs` and `services/risk-engine/src/gates/mod.rs`.
- 2026-04-07: QA automation refresh added Story 5.3 API accepted-envelope/error-fallback assertions and E2E FR41 threshold/precedence regression checks; reran Story 5.3 API/E2E suite successfully.
