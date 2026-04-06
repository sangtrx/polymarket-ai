# Story 2.8: Enforce Pre-Trade Gate Evaluation Pipeline

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As a risk engine,  
I want every order intent evaluated against all pre-trade gates,  
so that unsafe orders are blocked before venue submission.

## Acceptance Criteria

1. **All gates pass (story-local BDD):**  
   **Given** freshness, stream health, exposure, drawdown, strategy approval, and eligibility gates are healthy  
   **When** an order intent is evaluated  
   **Then** a pass decision is recorded and the intent is forwarded to execution.
2. **Single gate failure (story-local BDD):**  
   **Given** any single gate fails  
   **When** evaluation completes  
   **Then** order placement is blocked and a machine-readable denial reason is returned.
3. **Drawdown boundary condition (story-local BDD):**  
   **Given** drawdown is exactly at the configured stop threshold  
   **When** order intent is evaluated  
   **Then** new order placement is denied and trading enters protective mode.
4. **UAC-1 Failure handling:** Missing, stale, malformed, or unavailable gate inputs return explicit machine-readable deny reasons; no fail-open behavior is allowed in pre-trade adjudication.
5. **UAC-2 Boundary behavior:** Gate thresholds are deterministic and test-covered, including `freshness > 30s` breach semantics, stream-health boundary semantics, and inclusive drawdown stop behavior (`current_drawdown_pct >= configured_stop_threshold_pct` denies).
6. **UAC-3 Verifiable evidence:** Every allow/deny pre-trade decision emits timestamped telemetry and durable evidence with correlation metadata and gate-specific reason codes.
7. **Schema/dependency/traceability contract:** Story depends only on `2.4` and `2.7`, creates only `pretrade_gate_decisions`, and maps explicitly to `FR18`, `FR21`, `NFR5`, and `NFR12`.
8. **Safe-state transition contract (NFR5/NFR12):** Unhealthy/uncertain gate states must force new-order submission rate to zero within the pre-trade path and surface protective-mode evidence within 5 seconds.
9. **Execution-path availability contract:** If pre-trade adjudication is unavailable or times out, submit flow fails closed with a machine-readable denial reason and persists no submission transition side effects.

## Tasks / Subtasks

- [x] **Task 1: Define canonical pre-trade gate domain contracts and reason taxonomy** (AC: 1, 2, 3, 4, 5, 6, 7, 8)
  - [x] Extend `crates/domain/src/risk.rs` with typed pre-trade contracts (gate dimensions, aggregate decision envelope, gate result payload, and machine-readable reason taxonomy) that can encode all FR21 gate outcomes.
  - [x] Add deterministic helper(s) for drawdown-stop evaluation with explicit inclusive threshold semantics (`>=` triggers deny/protective mode).
  - [x] Add validation guards ensuring UTC timestamps, normalized identifiers, and known reason-code parsing for persisted/telemetry decision payloads.
  - [x] Add domain tests for all-pass, single-fail, and drawdown-equals-threshold denial behavior.

- [x] **Task 2: Add forward-only persistence migration for Story 2.8 schema scope** (AC: 1, 2, 3, 5, 6, 7, 8, 9)
  - [x] Add migration under `crates/persistence/migrations/` creating only `pretrade_gate_decisions`.
  - [x] Add strict constraints for non-empty canonical IDs, enum-like outcome fields, machine reason codes, UTC decision timestamps, and deterministic correlation metadata.
  - [x] Add indexes for `intent_id`, `(market_id, evaluated_at_utc)`, `reason_code`, and `correlation_id` incident/audit lookup paths.
  - [x] Add deterministic dedupe constraints so retries for the same `intent_id` cannot create conflicting decision evidence.
  - [x] Ensure migration scope explicitly excludes Story 2.9 entities (`safety_control_actions`) and any unrelated table changes.

- [x] **Task 3: Implement PostgreSQL pre-trade decision persistence adapter** (AC: 1, 2, 4, 6, 7, 8)
  - [x] Add `crates/persistence/src/postgres/pretrade_gate.rs` and wire it via `crates/persistence/src/postgres/mod.rs`.
  - [x] Implement durable write API(s) for pre-trade gate decisions and deterministic latest-decision read API(s) required for incident/correlation queries.
  - [x] Return typed machine-readable persistence errors; do not swallow decode/constraint/transaction failures.

- [x] **Task 4: Expand risk-engine runtime state surfaces for full FR21 gate inputs** (AC: 1, 2, 3, 4, 5, 6, 8)
  - [x] Extend `services/risk-engine/src/gates/mod.rs` runtime reader/state types to include stream-health, drawdown-stop, and strategy-approval inputs while preserving existing freshness/auth/reconciliation/cluster/limit seams.
  - [x] Normalize runtime gate-state reason codes to canonical domain reason taxonomy and keep fail-closed defaults for unknown/invalid state.
  - [x] Add bootstrap/hydration seams in `services/risk-engine/src/main.rs` for latest-state loading (market-stream health, risk-limit profile state, and other pre-trade inputs) without introducing synthetic "healthy" fallbacks.

- [x] **Task 5: Implement deterministic pre-trade gate pipeline orchestration in risk-engine** (AC: 1, 2, 3, 4, 5, 6, 8, 9)
  - [x] Introduce explicit gate-order evaluation in `services/risk-engine/src/gates/mod.rs` covering: freshness, stream health, exposure/limit state, drawdown stop, strategy approval active-state, and venue eligibility.
  - [x] Reuse existing domain evaluators where available (`assess_market_stream_health`, `evaluate_market_eligibility`, freshness and limit evaluators) rather than duplicating logic.
  - [x] Preserve/reconcile existing reconciliation-critical halt gating from Story 2.6 as a fail-closed precondition in the same adjudication surface.
  - [x] Ensure deny outputs always contain a single deterministic machine-readable reason code and emit telemetry with correlation and UTC timestamp while preserving existing event contract `risk_order_intent_gate_decision_v1` (and, if adding `risk_pretrade_gate_decision_v1`, emit compatibility output to avoid alert/dashboard regressions).
  - [x] For drawdown threshold equality, assert protective-mode transition signal and deny new order creation.

- [x] **Task 6: Wire pre-trade adjudication into execution-side submit seam without violating ownership boundaries** (AC: 1, 2, 4, 6, 8, 9)
  - [x] Add an execution-side risk adjudication port seam (trait/interface) in `services/execution-engine/src/orders/mod.rs` so submit flow requests pre-trade decision before persisting submission transitions.
  - [x] Ensure denied/unavailable adjudication returns explicit machine-readable runtime errors and creates no submission side effects.
  - [x] Add explicit adjudication timeout handling; timeout/unreachable outcomes must fail closed with deterministic machine reason codes and no retry path that could bypass gate sequencing.
  - [x] Keep architecture boundary intact: execution consumes risk decisions, but does not mutate risk-policy state.
  - [x] Preserve existing order-lifecycle behavior for allowed intents.

- [x] **Task 7: Add deterministic Story 2.8 test coverage and QA command** (AC: 1, 2, 3, 4, 5, 6, 8, 9)
  - [x] Add domain tests for gate taxonomy parsing/validation and drawdown inclusive threshold semantics.
  - [x] Add persistence tests validating migration scope, constraints/indexes, and decision write/read determinism for `pretrade_gate_decisions`.
  - [x] Add risk-engine tests for all-pass behavior, single-gate failure behavior, gate precedence determinism, and drawdown boundary protective-mode transition.
  - [x] Add execution-engine tests proving denied pre-trade adjudication blocks submit side effects, timeout/unavailable adjudication fails closed with deterministic reason codes, and allowed decisions preserve current lifecycle transitions.
  - [x] Add `qa:test:story-2-8` to `package.json` and update `_bmad-output/implementation-artifacts/tests/test-summary.md` with Story 2.8 evidence.

- [x] **Task 8: Add pre-trade gate operations runbook** (AC: 2, 3, 6, 8)
  - [x] Add/update `docs/operations/*pretrade-gate*` guidance for gate-order semantics, reason-code matrix, fail-closed expectations, and incident triage flows.
  - [x] Document operator verification steps for drawdown-triggered protective mode and correlated decision evidence retrieval by `intent_id`/`correlation_id`.
  - [x] Document recovery expectations and handoff to Story 2.9 emergency-control workflows.

### Review Findings

- [x] [Review][Patch] Reject contradictory pre-trade deny decisions that use `pretrade_gate_pass` reason codes and normalize unknown adjudication port error codes to canonical pre-trade reason taxonomy. [services/execution-engine/src/orders/mod.rs]
- [x] [Review][Patch] Emit execution telemetry for pre-trade submit denials that occur before lifecycle transition persistence. [services/execution-engine/src/orders/mod.rs]
- [x] [Review][Patch] Enforce `protective_mode_active` only for drawdown-stop-triggered deny outcomes and add domain regression coverage for this invariant. [crates/domain/src/risk.rs]
- [x] [Review][Patch] Remove synthetic bootstrap pre-trade persistence side effect, use runtime UTC bootstrap timestamps, and keep bootstrap auth/reconciliation defaults fail-closed with explicit env-driven override controls. [services/risk-engine/src/main.rs]
- [x] [Review][Patch] Reconciled story `File List` with git reality by adding missing `Cargo.lock`; `.scripts/bmad-auto/copilot/bmad-progress.log` remains out-of-scope automation noise. [_bmad-output/implementation-artifacts/stories/2-8-enforce-pre-trade-gate-evaluation-pipeline.md]
- [x] [Review][Patch] Persist live (non-bootstrap) pre-trade decisions through the new persistence adapter from the runtime adjudication path. [services/risk-engine/src/gates/mod.rs, services/risk-engine/src/main.rs]
- [x] [Review][Patch] Wire a concrete risk adjudication integration for production execution submit flow (current runtime seam exists, but main wiring still lacks a live adjudication client). [services/execution-engine/src/main.rs]
- [x] [Review][Patch] Replace pre-trade adjudication `.expect(...)` panic path with explicit fail-closed invalid-payload fallback decision construction to keep runtime deterministic under contract violations. [services/risk-engine/src/gates/mod.rs]
- [x] [Review][Patch] Preserve valid request timestamps for invalid-payload pre-trade decisions (fall back to epoch only when timestamp input is malformed) and add regression coverage. [services/risk-engine/src/gates/mod.rs]
- [x] [Review][Patch] Emit pre-trade submit-denial telemetry for adjudication timeout/unavailable error paths in addition to deny-decision paths. [services/execution-engine/src/orders/mod.rs]
- [x] [Review][Patch] Emit success pre-trade decision telemetry only after durable persistence succeeds to avoid false-positive evidence on persistence failures. [services/risk-engine/src/gates/mod.rs]

## Dev Notes

### Technical Requirements

- Story dependency is strict: `2.8` depends only on `2.4` and `2.7`.
- Story scope is explicit and narrow:
  - Functional scope: deterministic pre-trade gate adjudication for FR21 gate set plus drawdown protective transition behavior.
  - Schema scope: `pretrade_gate_decisions` only.
  - Traceability scope: `FR18`, `FR21`, `NFR5`, `NFR12`.
- Required gate dimensions for this story:
  - freshness (`>30s` breach behavior from Story 2.4),
  - stream health (non-healthy must deny),
  - exposure/risk-limit state (Story 2.7 active-state availability),
  - drawdown stop (inclusive threshold boundary deny),
  - strategy approval active-state,
  - venue/market eligibility.
- Deterministic behavior contracts:
  - Every evaluation returns exactly one final allow/deny outcome and one final machine-readable reason code.
  - Missing/unknown gate state must deny (fail-closed) instead of defaulting to allow.
  - Drawdown equality boundary (`==`) is treated as stop-triggered deny and must assert protective-mode signal.
  - Execution-side adjudication must be timeout-bounded and fail closed; unavailable adjudication must never fall back to submit behavior.
  - Preserve existing `risk_order_intent_gate_decision_v1` telemetry compatibility (or dual-emit with any new pre-trade event name) to avoid regression in alerting/forensics consumers.
- Seams that must be explicit (no implicit defaults):
  - **Drawdown source-of-truth seam:** introduce a typed runtime drawdown state input; if unavailable/invalid, deny with explicit state-unavailable reason.
  - **Strategy approval source-of-truth seam:** derive active/inactive state from governance approval evidence; unavailable state must deny.
  - **Stream-health source-of-truth seam:** consume latest market-stream health evidence; degraded/disconnected states deny deterministically.
- **Out of scope for Story 2.8:** full emergency control orchestration/commands and resume workflows (Story 2.9+), operator dashboard UX surfaces (Epic 3).

[Source: _bmad-output/planning-artifacts/epics.md#Story 2.8: Enforce Pre-Trade Gate Evaluation Pipeline]  
[Source: _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)]  
[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: _bmad-output/planning-artifacts/prd.md#Functional Requirements]  
[Source: _bmad-output/planning-artifacts/prd.md#Non-Functional Requirements]  
[Source: services/risk-engine/src/gates/mod.rs]  
[Source: services/risk-engine/src/limits/mod.rs]  
[Source: crates/domain/src/risk.rs]

### Architecture Compliance

- Preserve architecture ownership boundaries:
  - `risk-engine` owns trading eligibility/pre-trade decisions.
  - `execution-engine` must consume decision outcomes and must not mutate risk-policy state.
  - `governance-service` remains source for privileged approval workflow evidence.
- Keep FR17-FR21 mapping aligned with `services/risk-engine/src/{gates,limits,safe_state}`.
- Maintain consistency rules:
  - canonical machine-readable error envelopes,
  - ISO-8601 UTC timestamps only,
  - deterministic gate ordering and reason-code outputs,
  - no swallowed errors or success-shaped fallbacks.
- Preserve safety-first behavior: uncertain state must transition to protective deny behavior.

[Source: _bmad-output/planning-artifacts/architecture.md#Architectural Boundaries]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: _bmad-output/planning-artifacts/architecture.md#Integration Points]  
[Source: _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules]

### Library & Framework Requirements

- Continue workspace-pinned stack for compatibility:
  - `polymarket-client-sdk = 0.4.4` (`clob`, `ws`)
  - `tokio = 1.48.0`
  - `sqlx = 0.8.6`
  - `axum = 0.8.8`
  - `time = 0.3.44`
- Latest-version checks at story creation time:
  - `polymarket-client-sdk`: latest stable `0.4.4`
  - `tokio`: latest stable `1.51.0`
  - `sqlx`: newest release `0.9.0-alpha.1` (pre-release); latest stable `0.8.6`
  - `axum`: latest stable `0.8.8`
  - `time`: latest stable `0.3.47`
- Do not introduce opportunistic dependency upgrades in Story 2.8; prioritize deterministic safety behavior and compatibility with existing Epic 2 implementations.

[Source: Cargo.toml]  
[Source: https://crates.io/api/v1/crates/polymarket-client-sdk]  
[Source: https://crates.io/api/v1/crates/tokio]  
[Source: https://crates.io/api/v1/crates/sqlx]  
[Source: https://crates.io/api/v1/crates/axum]  
[Source: https://crates.io/api/v1/crates/time]

### File Structure Requirements

- Primary implementation surfaces:
  - `crates/domain/src/risk.rs`
  - `crates/persistence/migrations/*pretrade_gate_decisions*.sql`
  - `crates/persistence/src/postgres/{mod.rs,pretrade_gate.rs}`
  - `services/risk-engine/src/{gates/mod.rs,limits/mod.rs,safe_state/mod.rs,main.rs}`
  - `services/execution-engine/src/{orders/mod.rs,main.rs}`
  - `package.json`
  - `_bmad-output/implementation-artifacts/tests/test-summary.md`
  - `docs/operations/*pretrade-gate*`
- Reuse established Story 2 layering pattern (domain -> migration -> persistence adapter -> runtime/service wiring -> tests -> runbook).
- Preserve schema-per-story discipline: do not pre-implement Story 2.9 schema/entities.

[Source: _bmad-output/planning-artifacts/architecture.md#Complete Project Directory Structure]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: crates/persistence/src/postgres/mod.rs]  
[Source: services/risk-engine/src/gates/mod.rs]  
[Source: services/risk-engine/src/safe_state/mod.rs]  
[Source: services/execution-engine/src/orders/mod.rs]

### Testing Requirements

- Add deterministic coverage for:
  - all-gates-pass allow path with persisted decision evidence,
  - each single-gate failure deny path with machine reason code,
  - drawdown exact-threshold denial with protective-mode signal,
  - fail-closed behavior when gate inputs are missing/unavailable/stale,
  - deterministic gate precedence when multiple failures are present.
- Validate persistence guarantees:
  - migration creates only `pretrade_gate_decisions`,
  - required constraints/indexes are enforced,
  - decision write/read APIs preserve reason code, outcome, timestamps, and correlation metadata.
- Validate execution integration seam:
  - denied pre-trade adjudication prevents submit transition persistence,
  - timeout/unavailable adjudication fails closed with deterministic machine reason codes and no submit side effects,
  - allowed adjudication preserves existing order lifecycle behavior.
- Keep test layering consistent with repository patterns:
  - domain tests (`crates/domain`),
  - persistence tests (`crates/persistence`),
  - risk runtime tests (`services/risk-engine`),
  - execution runtime tests (`services/execution-engine`).
- Add story-scoped QA script and update Story 2.8 test evidence summary.

[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: package.json]  
[Source: _bmad-output/implementation-artifacts/stories/2-4-enforce-data-freshness-gates-and-stale-feed-pausing.md#Testing Requirements]  
[Source: _bmad-output/implementation-artifacts/stories/2-6-build-reconciliation-and-exposure-visibility-core.md#Testing Requirements]  
[Source: _bmad-output/implementation-artifacts/stories/2-7-configure-portfolio-market-and-strategy-limit-policies.md#Testing Requirements]

### Previous Story Intelligence

- Story 2.7 established active/pending risk-limit state handling and `evaluate_order_intent_gate_with_limit_state`; Story 2.8 should build on this seam instead of introducing duplicate limit-state evaluators.
- Story 2.6 introduced reconciliation-critical halt deny semantics in `risk-engine` gate runtime; Story 2.8 must preserve this fail-closed behavior while expanding FR21 gate coverage.
- Story 2.4 established deterministic freshness boundary behavior and fail-closed normalization for invalid pause reasons; Story 2.8 should reuse that boundary logic directly.
- Story 2.3 established user-stream auth-block deny semantics and runtime-state toggles; keep this behavior intact as part of the broader pre-trade pipeline.
- Existing governance contracts already define `strategy_promotion_override` critical action IDs; strategy approval gate state should consume this evidence surface instead of inventing a parallel approval model.

[Source: _bmad-output/implementation-artifacts/stories/2-7-configure-portfolio-market-and-strategy-limit-policies.md#Completion Notes List]  
[Source: _bmad-output/implementation-artifacts/stories/2-6-build-reconciliation-and-exposure-visibility-core.md#Completion Notes List]  
[Source: _bmad-output/implementation-artifacts/stories/2-4-enforce-data-freshness-gates-and-stale-feed-pausing.md#Completion Notes List]  
[Source: _bmad-output/implementation-artifacts/stories/2-3-ingest-authenticated-user-stream-with-ordering-guarantees.md#Completion Notes List]  
[Source: crates/domain/src/governance.rs#CriticalActionId]  
[Source: services/risk-engine/src/gates/mod.rs]

### Git Intelligence Summary

- Recent Epic 2 commit sequence remains consistent and should be preserved for Story 2.8:
  1. domain contracts/reason codes,  
  2. scoped forward-only migration,  
  3. persistence adapter wiring,  
  4. runtime/service integration,  
  5. deterministic tests + story QA command + operations runbook.
- Recent commits also enforce strict schema scope and fail-closed runtime defaults; Story 2.8 should keep those guardrails unchanged.

[Source: git --no-pager log --oneline -5]  
[Source: git --no-pager show --name-only --pretty=format:'%h %s' -5]

### Latest Technical Information

- Crate-version checks confirm architecture-selected dependency versions remain valid for Story 2.8 implementation.
- `sqlx` has a newer pre-release (`0.9.0-alpha.1`), but stable remains `0.8.6`; do not adopt pre-release dependencies in this safety-critical gate story.
- No dependency upgrades are required to deliver Story 2.8 scope.

[Source: Cargo.toml]  
[Source: https://crates.io/api/v1/crates/polymarket-client-sdk]  
[Source: https://crates.io/api/v1/crates/tokio]  
[Source: https://crates.io/api/v1/crates/sqlx]  
[Source: https://crates.io/api/v1/crates/axum]  
[Source: https://crates.io/api/v1/crates/time]

### Project Context Reference

- No `project-context.md` file was found in repository scope during discovery.
- Context for this story was derived from epics, PRD, architecture, UX specification, readiness report, prior story files, git history, and current source surfaces.

### Project Structure Notes

- `services/risk-engine/src/gates/mod.rs` currently evaluates freshness/reconciliation/auth/cluster and optional limit-state availability; it does not yet implement full FR21 pre-trade pipeline composition.
- `services/risk-engine/src/safe_state/mod.rs` remains a placeholder seam and should be used for drawdown protective-mode signaling required by Story 2.8.
- `services/execution-engine/src/orders/mod.rs` currently submits order lifecycle transitions without pre-trade risk adjudication; Story 2.8 must add a risk decision seam before submit side effects.
- `crates/persistence/src/postgres/market_stream.rs`, `user_stream.rs`, `freshness_gate.rs`, `reconciliation.rs`, and `risk_limits.rs` already provide latest-state loaders that should be reused for gate inputs.

[Source: services/risk-engine/src/gates/mod.rs]  
[Source: services/risk-engine/src/safe_state/mod.rs]  
[Source: services/execution-engine/src/orders/mod.rs]  
[Source: crates/persistence/src/postgres/market_stream.rs]  
[Source: crates/persistence/src/postgres/user_stream.rs]  
[Source: crates/persistence/src/postgres/freshness_gate.rs]  
[Source: crates/persistence/src/postgres/reconciliation.rs]  
[Source: crates/persistence/src/postgres/risk_limits.rs]

### References

- _bmad-output/planning-artifacts/epics.md#Epic 2: Live Market Connectivity & Safe Core Execution  
- _bmad-output/planning-artifacts/epics.md#Story 2.8: Enforce Pre-Trade Gate Evaluation Pipeline  
- _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)  
- _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)  
- _bmad-output/planning-artifacts/prd.md#Functional Requirements  
- _bmad-output/planning-artifacts/prd.md#Non-Functional Requirements  
- _bmad-output/planning-artifacts/architecture.md#Architectural Boundaries  
- _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping  
- _bmad-output/planning-artifacts/architecture.md#Integration Points  
- _bmad-output/planning-artifacts/ux-design-specification.md#Journey 2 — Incident Safe-State Workflow  
- _bmad-output/planning-artifacts/implementation-readiness-report-2026-04-05.md#✅ Remediation Outcomes  
- _bmad-output/implementation-artifacts/stories/2-3-ingest-authenticated-user-stream-with-ordering-guarantees.md  
- _bmad-output/implementation-artifacts/stories/2-4-enforce-data-freshness-gates-and-stale-feed-pausing.md  
- _bmad-output/implementation-artifacts/stories/2-6-build-reconciliation-and-exposure-visibility-core.md  
- _bmad-output/implementation-artifacts/stories/2-7-configure-portfolio-market-and-strategy-limit-policies.md  
- crates/domain/src/risk.rs  
- crates/domain/src/governance.rs  
- crates/persistence/src/postgres/market_stream.rs  
- crates/persistence/src/postgres/user_stream.rs  
- crates/persistence/src/postgres/freshness_gate.rs  
- crates/persistence/src/postgres/reconciliation.rs  
- crates/persistence/src/postgres/risk_limits.rs  
- services/risk-engine/src/gates/mod.rs  
- services/risk-engine/src/limits/mod.rs  
- services/risk-engine/src/safe_state/mod.rs  
- services/execution-engine/src/orders/mod.rs  
- services/execution-engine/src/main.rs  
- Cargo.toml  
- package.json

## Story Completion Status

- Story implementation and adversarial review remediation are complete, including fail-closed fallback hardening, telemetry/evidence ordering fixes, and invalid-payload timestamp normalization coverage.
- Story lifecycle advanced from `review` to `done` after high/medium review findings were resolved and full validation suites passed.

## Dev Agent Record

### Agent Model Used

GPT-5.3-Codex (gpt-5.3-codex)

### Debug Log References

- BMAD create-story workflow execution (automated)
- Sprint backlog discovery from `_bmad-output/implementation-artifacts/sprint-status.yaml`
- Artifact discovery across planning/implementation/source-code surfaces
- Recent commit and changed-file pattern analysis for Story 2 continuity
- Latest-version checks via crates.io API endpoints
- Adversarial multi-layer code review (blind hunter, edge-case hunter, acceptance auditor) with auto-fix pass

### Completion Notes List

- Implemented canonical pre-trade gate contracts and deterministic adjudication helpers in domain with drawdown `>=` boundary deny/protective semantics.
- Added Story 2.8 scoped migration for `pretrade_gate_decisions` plus typed Postgres pre-trade decision persistence adapter and module wiring.
- Expanded risk-engine runtime state surfaces (stream-health, drawdown, strategy approval, venue inputs) and implemented deterministic gate-order pre-trade orchestration with compatibility telemetry.
- Implemented drawdown protective-mode signaling through `services/risk-engine/src/safe_state/mod.rs` and wired bootstrap hydration seams in `services/risk-engine/src/main.rs` with fail-closed defaults.
- Added execution-side pre-trade adjudication port seam and timeout-bounded fail-closed submit behavior in `services/execution-engine/src/orders/mod.rs`.
- Added Story 2.8 QA script, updated test evidence summary, and added operations runbook for gate order, fail-closed triage, and drawdown protective-mode verification.
- Applied review-time hardening fixes for pre-trade deny telemetry, deny/pass reason consistency enforcement, unknown adjudication error-code normalization, and protective-mode invariant validation.
- Resolved remaining runtime integration gaps by adding live pre-trade decision persistence through the risk-engine runtime adjudication path and wiring a concrete Postgres-backed execution risk adjudication client in production startup.
- Re-ran adversarial review layers, auto-fixed remaining high/medium findings, and completed final story validation with fail-closed fallback + telemetry ordering protections.
- Added QA automation regression tests for strategy-approval-missing deny, market-snapshot-missing deny, and default execution runtime fail-closed adjudication unavailability behavior.

### File List

- _bmad-output/implementation-artifacts/stories/2-8-enforce-pre-trade-gate-evaluation-pipeline.md
- _bmad-output/implementation-artifacts/sprint-status.yaml
- crates/domain/src/risk.rs
- crates/persistence/migrations/20260406081500_pretrade_gate_decisions.sql
- crates/persistence/src/postgres/mod.rs
- crates/persistence/src/postgres/pretrade_gate.rs
- services/risk-engine/Cargo.toml
- services/risk-engine/src/gates/mod.rs
- services/risk-engine/src/main.rs
- services/risk-engine/src/safe_state/mod.rs
- services/execution-engine/src/main.rs
- services/execution-engine/src/orders/mod.rs
- Cargo.lock
- package.json
- _bmad-output/implementation-artifacts/tests/test-summary.md
- docs/operations/pretrade-gate-pipeline.md

### Change Log

- 2026-04-06: Created Story 2.8 context file and moved lifecycle state from `backlog` to `ready-for-dev`.
- 2026-04-06: Validate-story gate remediation added execution-timeout fail-closed requirements, decision dedupe constraints, and telemetry compatibility guardrails; status remains `ready-for-dev`.
- 2026-04-06: Implemented Story 2.8 pre-trade gate pipeline end-to-end, added migration/adapter/runtime/execution/test/runbook updates, and advanced status to `review`.
- 2026-04-06: Ran adversarial code-review workflow, auto-fixed high/medium issues in domain/execution/risk bootstrap paths, and returned story status to `in-progress` pending unresolved production adjudication + live persistence wiring.
- 2026-04-06: Completed review follow-up fixes for live decision persistence and production adjudication client wiring, added regression coverage, re-ran story QA + full suite, and moved status to `review`.
- 2026-04-06: Completed automated adversarial re-review, fixed residual high/medium fail-closed and telemetry-ordering issues, re-ran Story 2.8 QA + workspace suites, and moved status to `done`.
- 2026-04-06: Added additional Story 2.8 QA fail-closed regression automation coverage (strategy approval missing, market snapshot missing, and default execution adjudication-port unavailable), re-ran `qa:test:story-2-8`, and kept status `done`.
