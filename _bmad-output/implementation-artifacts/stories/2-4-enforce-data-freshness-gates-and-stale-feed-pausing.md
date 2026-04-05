# Story 2.4: Enforce Data Freshness Gates and Stale-Feed Pausing

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As a risk engine,  
I want stale-feed detection tied to automatic trading pauses,  
so that trading stops when data certainty is insufficient.

## Acceptance Criteria

1. **Stale threshold breach (story-local BDD):**  
   **Given** no market or user update is received for > 30 seconds  
   **When** freshness checks execute  
   **Then** new order creation is paused automatically and reason code is recorded.
2. **Boundary condition (story-local BDD):**  
   **Given** latest update age is 29 seconds  
   **When** freshness checks execute  
   **Then** trading remains enabled and no stale alert is emitted.
3. **Recovery path (story-local BDD):**  
   **Given** stale pause is active  
   **When** freshness returns to <= 30 seconds for a full stability window  
   **Then** trading can re-enter ready state with explicit operator-visible confirmation.
4. **UAC-1 Failure handling:** Missing/invalid timestamps, unavailable stream-health state, persistence failures, and stale-check evaluation errors return explicit machine-readable reason codes and preserve fail-closed behavior (no unsafe trading enablement).
5. **UAC-2 Boundary behavior:** Freshness thresholds are deterministic and test-covered at `29s`, `30s`, and `>30s`, including stability-window edge handling (`window-1s` remains paused, `window` unlocks).
6. **UAC-3 Verifiable evidence:** Stale detection, pause activation, blocked intents, and recovery confirmation emit timestamped, correlation-aware evidence suitable for incident and QA traceability.
7. **Schema/dependency/traceability contract:** Story depends only on `2.2` and `2.3`, introduces only `freshness_gate_events`, and maps explicitly to `FR5`, `NFR5`, and `NFR12`.
8. **NFR5 transition latency contract:** When freshness breaches `>30s`, safe-state transition must assert pause-active intent blocking within `<=5s`, and evidence must include machine-verifiable breach/pause timestamps.

## Tasks / Subtasks

- [x] **Task 1: Define canonical data-freshness gate contracts and reason-code taxonomy** (AC: 1, 2, 4, 5, 7)
  - [x] Extend `crates/domain/src/risk.rs` with freshness-gate contracts (event payload and gate-evaluation outcome) covering market + user stream age checks.
  - [x] Add deterministic reason codes for stale breach, boundary-safe state, state-unavailable, recovery-pending, and recovery-confirmed outcomes.
  - [x] Add validation helpers for freshness thresholds, stability-window boundaries, and UTC timestamp fields.
  - [x] Keep naming conventions and machine-readable code style aligned with Story 2.2/2.3 contracts.

- [x] **Task 2: Add forward-only persistence migration for freshness-gate evidence** (AC: 1, 3, 4, 6, 7)
  - [x] Add a migration under `crates/persistence/migrations/` creating only `freshness_gate_events`.
  - [x] Enforce schema constraints for non-negative age metrics, non-empty reason/correlation IDs, deterministic pause-state fields, and UTC timestamps.
  - [x] Add indexes for latest-gate lookup and time-window incident analysis.
  - [x] Ensure migration scope excludes `orders`, `order_state_transitions`, and reconciliation tables.

- [x] **Task 3: Implement persistence adapter for freshness-gate events** (AC: 1, 3, 4, 6, 7)
  - [x] Add `crates/persistence/src/postgres/freshness_gate.rs` and wire it via `crates/persistence/src/postgres/mod.rs`.
  - [x] Implement durable write path for gate transitions (pause activated, pause maintained, recovery pending, recovery confirmed).
  - [x] Implement read path(s) needed to bootstrap latest gate state during runtime startup.
  - [x] Return explicit machine-readable persistence errors; no silent retry loops that hide stale-state uncertainty.

- [x] **Task 4: Expose freshness signal inputs from market and user ingestion runtimes** (AC: 1, 2, 4, 5, 6)
  - [x] Reuse existing market-stream runtime state (`last_observed_at`, health transitions) to produce deterministic latest market-update age.
  - [x] Extend user-stream runtime state to expose deterministic latest successful user-event timestamp/age.
  - [x] Add a shared freshness-snapshot seam for risk gating logic (without breaking Story 2.2/2.3 contracts).
  - [x] Preserve fail-closed semantics when either stream freshness input is missing or ambiguous.

- [x] **Task 5: Implement automatic stale-feed pause gate evaluation loop** (AC: 1, 2, 4, 5, 6, 8)
  - [x] Add periodic freshness evaluation using threshold `> 30s` across market/user feeds.
  - [x] Keep deterministic boundary behavior: `29s` allowed, `30s` allowed, `>30s` paused.
  - [x] On stale breach, activate pause state, persist gate event with reason code, and emit telemetry evidence.
  - [x] Enforce NFR5 timing: breach-to-pause transition must complete within `<=5s` and emit deterministic timing fields for verification.
  - [x] Maintain paused state while stale conditions persist; do not flap on transient checks.

- [x] **Task 6: Implement stability-window recovery and operator-visible confirmation path** (AC: 3, 4, 6)
  - [x] Add configurable stability-window logic (continuous healthy freshness before unpause).
  - [x] Persist explicit recovery transition events (`recovery_pending` and `recovery_confirmed`) with timestamps and correlation IDs.
  - [x] Clear pause state only after full stability window passes; otherwise remain fail-closed.
  - [x] Emit operator-consumable confirmation evidence suitable for incident timeline/UI surfaces.

- [x] **Task 7: Integrate freshness pause into order-intent gate behavior** (AC: 1, 3, 4, 6)
  - [x] Extend `services/risk-engine/src/gates/mod.rs` runtime policy state with freshness pause signal and reason-code propagation.
  - [x] Ensure `evaluate_order_intent_gate` denies new intents while stale pause is active, alongside existing auth-expired and cluster-toggle rules.
  - [x] Keep deny payload machine-readable and deterministic.
  - [x] Preserve current staged architecture seam (no speculative cross-service coupling beyond current repository patterns).

- [x] **Task 8: Add deterministic test and runbook coverage for stale/pause/recovery flows** (AC: 1, 2, 3, 4, 5, 6, 8)
  - [x] Add domain tests for threshold boundaries (`29s`, `30s`, `>30s`) and stability-window transitions.
  - [x] Add persistence tests validating migration scope, constraints, and transition query determinism for `freshness_gate_events`.
  - [x] Add execution-engine tests proving automatic pause on stale breach and no pause on boundary-safe inputs.
  - [x] Add risk-gate tests proving stale pause blocks new intents and recovery unblocks only after confirmed stability.
  - [x] Add integration assertions proving stale-breach-to-pause transition remains `<=5s` and includes machine-verifiable breach/pause timestamp evidence.
  - [x] Add/update operations docs for stale-feed incident handling, pause-state interpretation, and recovery runbook steps.
  - [x] Add story-scoped QA command (`qa:test:story-2-4`) in `package.json` if consistent with existing Epic 2 conventions.

## Dev Notes

### Technical Requirements

- Story dependency is strict: `2.4` depends only on `2.2` and `2.3` (no forward dependencies).
- Story scope is explicit and narrow:
  - Functional scope: stale-feed detection + automatic pause + stability-window recovery only.
  - Schema scope: `freshness_gate_events` only.
  - Traceability scope: `FR5`, `NFR5`, `NFR12`.
- Functional guardrails:
  - Stale detection triggers when either market or user stream update age is `> 30s`.
  - Boundary path at `29s` must remain non-stale and non-alerting.
  - Recovery requires continuous `<= 30s` freshness across the full stability window before unpausing.
  - Stale-breach to pause-active transition must complete within `<=5s` (NFR5) with deterministic breach/pause timestamps.
  - New order creation must remain blocked while stale pause is active.
- Preserve universal contracts:
  - explicit failure handling with machine-readable reason codes,
  - deterministic boundary behavior,
  - timestamped evidence for success/failure transitions.
- **Out of scope for Story 2.4:** order lifecycle state machine (Story 2.5), reconciliation/exposure core (Story 2.6), and full pre-trade gate pipeline composition (Story 2.8).

[Source: _bmad-output/planning-artifacts/epics.md#Story 2.4: Enforce Data Freshness Gates and Stale-Feed Pausing]  
[Source: _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)]  
[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: _bmad-output/planning-artifacts/prd.md#Market Universe & Data Intake]  
[Source: _bmad-output/planning-artifacts/prd.md#Risk & Capital Management]  
[Source: _bmad-output/planning-artifacts/prd.md#Non-Functional Requirements]

### Architecture Compliance

- Keep service boundaries explicit and consistent:
  - `execution-engine/src/ingestion` owns stream freshness signal capture.
  - `risk-engine/src/gates` owns new-order allow/deny decisions.
  - shared contracts and persistence adapters stay in workspace crates (`domain`, `persistence`).
- Preserve architecture safety rule: stale/uncertain state is a protection trigger, not a warning.
- Keep naming and envelope conventions consistent:
  - snake_case modules/DB identifiers,
  - machine-readable reason codes,
  - UTC timestamps and correlation metadata,
  - no swallowed errors on control/risk paths.
- Produce evidence surfaces compatible with risk-critical UI requirements (freshness timestamp + stale indicator + explicit confirmation).

[Source: _bmad-output/planning-artifacts/architecture.md#API & Communication Patterns]  
[Source: _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules]  
[Source: _bmad-output/planning-artifacts/architecture.md#Process Patterns]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: _bmad-output/planning-artifacts/architecture.md#Implementation Handoff]  
[Source: _bmad-output/planning-artifacts/architecture.md#Process Patterns]

### Library & Framework Requirements

- Continue workspace-pinned stack for compatibility:
  - `polymarket-client-sdk = 0.4.4` (`clob`, `ws`)
  - `tokio = 1.48.0`
  - `sqlx = 0.8.6`
  - `axum = 0.8.8`
  - `time = 0.3.44`
- Latest stable checks at story creation time:
  - `polymarket-client-sdk`: `0.4.4`
  - `tokio`: `1.51.0`
  - `sqlx`: `0.8.6` (newest `0.9.0-alpha.1` is pre-release)
  - `axum`: `0.8.8`
  - `time`: `0.3.47`
- Do not introduce opportunistic dependency upgrades in Story 2.4; prioritize deterministic behavior and workspace consistency.

[Source: Cargo.toml]  
[Source: https://crates.io/api/v1/crates/polymarket-client-sdk]  
[Source: https://crates.io/api/v1/crates/tokio]  
[Source: https://crates.io/api/v1/crates/sqlx]  
[Source: https://crates.io/api/v1/crates/axum]  
[Source: https://crates.io/api/v1/crates/time]

### File Structure Requirements

- Primary implementation surfaces:
  - `crates/domain/src/risk.rs`
  - `crates/persistence/migrations/*freshness_gate*.sql`
  - `crates/persistence/src/postgres/{mod.rs,freshness_gate.rs}`
  - `services/execution-engine/src/ingestion/mod.rs`
  - `services/execution-engine/src/ingestion/user_stream.rs`
  - `services/execution-engine/src/main.rs`
  - `services/risk-engine/src/gates/mod.rs`
  - `.env.example`
  - `docs/operations/*freshness*` (or updates to existing market/user ingestion runbooks)
- Keep Story 2 sequencing intact:
  - do not implement order submission/cancel lifecycle storage from Story 2.5,
  - do not pre-implement full multi-gate orchestration from Story 2.8.
- Reuse established Story 2.2/2.3 patterns for telemetry, reason codes, and fail-closed transitions.

[Source: _bmad-output/planning-artifacts/architecture.md#Complete Project Directory Structure]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: services/execution-engine/src/ingestion/mod.rs]  
[Source: services/execution-engine/src/ingestion/user_stream.rs]  
[Source: services/risk-engine/src/gates/mod.rs]  
[Source: crates/persistence/src/postgres/market_stream.rs]  
[Source: crates/persistence/src/postgres/user_stream.rs]

### Testing Requirements

- Add deterministic coverage for:
  - stale threshold breach (`>30s`) pauses new intents with explicit reason code,
  - boundary-safe path (`29s`) does not pause or emit stale alert,
  - recovery path requires full stability window before unpause,
  - stale-breach-to-pause transition latency remains `<=5s` with timestamped evidence,
  - fail-closed behavior when one or both stream-freshness inputs are unavailable,
  - timestamp/threshold boundary assertions and machine-readable error behavior.
- Keep quality-gate compatibility with existing repository workflows:
  - targeted crate tests (`domain`, `persistence`, `execution-engine`, `risk-engine`),
  - story-scoped QA command pattern in `package.json`,
  - workspace-level rust checks via existing scripts.
- Evidence assertions must validate reason codes, UTC timestamps, and correlation IDs across pause and recovery transitions.

[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: package.json]  
[Source: _bmad-output/implementation-artifacts/stories/2-2-ingest-market-stream-with-latency-guarantees.md#Testing Requirements]  
[Source: _bmad-output/implementation-artifacts/stories/2-3-ingest-authenticated-user-stream-with-ordering-guarantees.md#Testing Requirements]

### Previous Story Intelligence

- Story 2.2 established stream-health primitives and deterministic degradation semantics:
  - `market_stream_health` transitions, heartbeat/backlog reason codes, and fail-closed persistence telemetry.
- Story 2.3 established authenticated user-stream ordering and auth-fail-safe gating:
  - `order_event_offsets`, `block_new_intents`, deterministic auth transition states, and risk gate deny semantics.
- Story 2.4 should build directly on these foundations instead of creating parallel freshness subsystems:
  - reuse reason-code style and timestamp/correlation contract patterns,
  - reuse staged seam-based risk-gate integration pattern already used in Epic 2.

[Source: _bmad-output/implementation-artifacts/stories/2-2-ingest-market-stream-with-latency-guarantees.md#Dev Notes]  
[Source: _bmad-output/implementation-artifacts/stories/2-3-ingest-authenticated-user-stream-with-ordering-guarantees.md#Dev Notes]  
[Source: docs/operations/market-stream-ingestion.md]  
[Source: docs/operations/user-stream-ingestion.md]  
[Source: services/risk-engine/src/gates/mod.rs]

### Git Intelligence Summary

- Recent implementation pattern across Epic 2 commits is consistent and should be preserved:
  1. domain contracts/reason codes,  
  2. scoped migration,  
  3. persistence adapter wiring,  
  4. runtime integration,  
  5. gate logic + deterministic tests + story QA script.
- Commit history confirms Story 2.4 should continue incremental layering on 2.2/2.3 rather than refactoring completed ingestion flows.

[Source: git --no-pager log --oneline -5]  
[Source: git --no-pager log --name-only --pretty=format:'COMMIT %h %s' -5]

### Latest Technical Information

- External dependency checks confirm architecture-selected stack remains valid.
- SDK/runtime reliability expectations remain aligned with stale-feed guard requirements (independent watchdogs, reconnect supervision, and fail-closed uncertainty handling).
- No dependency upgrade is required to implement Story 2.4 safely.

[Source: https://crates.io/api/v1/crates/polymarket-client-sdk]  
[Source: https://crates.io/api/v1/crates/tokio]  
[Source: https://crates.io/api/v1/crates/sqlx]  
[Source: https://crates.io/api/v1/crates/axum]  
[Source: https://crates.io/api/v1/crates/time]  
[Source: _bmad-output/planning-artifacts/research/market-rust-polymarket-trading-system-lopez-de-prado-research-2026-04-04.md#Architectural Patterns and Design]

### Project Context Reference

- No `project-context.md` file was found in repository scope during discovery.
- Context for this story was derived from epics, PRD, architecture, UX, implementation-readiness, research artifacts, previous stories, git history, and current codebase surfaces.

### Project Structure Notes

- Current Epic 2 foundation already provides:
  - market stream health/degradation persistence (`market_stream_health`),
  - authenticated user stream auth/fail-safe state (`order_event_offsets`),
  - order-intent gate deny seam for auth and cluster controls.
- Story 2.4 should unify these inputs into explicit freshness gating without widening into Story 2.5+ execution lifecycle scope.
- Ensure pause/recovery transitions remain observable for operator-facing incident flows (timestamped confirmation and clear reason code path).

[Source: services/execution-engine/src/ingestion/mod.rs]  
[Source: services/execution-engine/src/ingestion/user_stream.rs]  
[Source: services/risk-engine/src/gates/mod.rs]  
[Source: _bmad-output/planning-artifacts/ux-design-specification.md#Journey 2 — Incident Safe-State Workflow]  
[Source: _bmad-output/planning-artifacts/ux-design-specification.md#2.3 Success Criteria]

### References

- _bmad-output/planning-artifacts/epics.md#Epic 2: Live Market Connectivity & Safe Core Execution  
- _bmad-output/planning-artifacts/epics.md#Story 2.4: Enforce Data Freshness Gates and Stale-Feed Pausing  
- _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)  
- _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)  
- _bmad-output/planning-artifacts/prd.md#Market Universe & Data Intake  
- _bmad-output/planning-artifacts/prd.md#Risk & Capital Management  
- _bmad-output/planning-artifacts/prd.md#Non-Functional Requirements  
- _bmad-output/planning-artifacts/prd.md#Operational Controls  
- _bmad-output/planning-artifacts/prd.md#Trading Reactivation Readiness Checklist  
- _bmad-output/planning-artifacts/architecture.md#Technical Constraints & Dependencies  
- _bmad-output/planning-artifacts/architecture.md#API & Communication Patterns  
- _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules  
- _bmad-output/planning-artifacts/architecture.md#Process Patterns  
- _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping  
- _bmad-output/planning-artifacts/architecture.md#Implementation Handoff  
- _bmad-output/planning-artifacts/ux-design-specification.md#2.3 Success Criteria  
- _bmad-output/planning-artifacts/ux-design-specification.md#Journey 2 — Incident Safe-State Workflow  
- _bmad-output/planning-artifacts/ux-design-specification.md#Feedback Patterns  
- _bmad-output/planning-artifacts/implementation-readiness-report-2026-04-05.md#✅ Remediation Outcomes  
- _bmad-output/implementation-artifacts/stories/2-2-ingest-market-stream-with-latency-guarantees.md  
- _bmad-output/implementation-artifacts/stories/2-3-ingest-authenticated-user-stream-with-ordering-guarantees.md  
- docs/operations/market-stream-ingestion.md  
- docs/operations/user-stream-ingestion.md  
- crates/domain/src/risk.rs  
- crates/persistence/src/postgres/market_stream.rs  
- crates/persistence/src/postgres/user_stream.rs  
- services/execution-engine/src/ingestion/mod.rs  
- services/execution-engine/src/ingestion/user_stream.rs  
- services/risk-engine/src/gates/mod.rs  
- Cargo.toml  
- .env.example  

## Story Completion Status

- Story context generated with exhaustive artifact analysis (workflow inputs, epic/PRD/architecture/UX/readiness/research artifacts, previous story intelligence, git history, and current codebase surfaces).
- Story file is created and ready for implementation by dev agents.
- Completion note: Ultimate context engine analysis completed - comprehensive developer guide created.

## Dev Agent Record

### Agent Model Used

GPT-5.3-Codex (gpt-5.3-codex)

### Debug Log References

- BMAD create-story workflow execution (automated)
- Sprint backlog discovery from `_bmad-output/implementation-artifacts/sprint-status.yaml`
- Artifact discovery across planning, implementation, and source-code surfaces
- Recent commit and changed-file pattern analysis for Story 2 continuity
- Latest dependency checks via crates.io API endpoints
- Story implementation executed with red-green-refactor loops across domain, persistence, execution-engine, and risk-engine surfaces
- Quality gates executed: `npm run --silent qa:test:story-2-4`, `npm run --silent rust:lint`, `npm run --silent rust:test`, `npm run --silent test`
- QA automation follow-up generated Story 2.4 critical-path error handling tests (`evaluate_once_rejects_non_utc_sample_timestamp`, `evaluate_once_surfaces_persistence_failure_with_machine_readable_code`, `invalid_hydrated_state_returns_machine_readable_evaluation_error`) and reran `npm run --silent qa:test:story-2-4`, `npm run --silent rust:lint`, and `npm run --silent rust:build`.

### Completion Notes List

- Selected first backlog story: `2-4-enforce-data-freshness-gates-and-stale-feed-pausing`.
- Captured story-local BDD acceptance criteria plus universal UAC contracts.
- Added implementation guardrails for strict schema scope, deterministic threshold boundaries, fail-closed gate behavior, and stability-window recovery semantics.
- Carried forward Story 2.2/2.3 patterns for reason-code taxonomy, telemetry evidence, and seam-based risk gate integration.
- Implemented canonical freshness-gate contracts in `domain::risk` including pause/recovery transitions, machine-readable reason taxonomy, and deterministic boundary/state-unavailable evaluation outcomes.
- Added forward-only `freshness_gate_events` migration plus persistence adapter for durable transition writes and deterministic latest-state bootstrap reads.
- Added shared freshness signal seam to market/user ingestion runtimes and a periodic freshness gate controller with fail-closed telemetry and event persistence.
- Extended risk order-intent gate runtime policy state with freshness pause reason propagation and deny-path enforcement while pause is active.
- Added deterministic unit/integration coverage for stale activation, `29s`/`30s` boundary-safe behavior, `window-1s` recovery pending, and `window` recovery confirmation.
- Added operations runbook coverage and story-scoped QA command `qa:test:story-2-4`.
- Code review auto-fixes hardened freshness re-pause timestamp handling after recovery and added regression coverage for the edge case.
- Code review auto-fixes normalized invalid/non-pausing freshness reason codes to deterministic fail-closed reasoning when pause is active.
- Added QA automation regression coverage for invalid sampled timestamp enforcement, persistence failure reason-code propagation, and invalid hydrated-state fail-closed evaluation in the freshness gate controller.

### File List

- .env.example
- _bmad-output/implementation-artifacts/sprint-status.yaml
- _bmad-output/implementation-artifacts/tests/test-summary.md
- _bmad-output/implementation-artifacts/stories/2-4-enforce-data-freshness-gates-and-stale-feed-pausing.md
- crates/domain/src/risk.rs
- crates/persistence/migrations/20260406040500_freshness_gate_events.sql
- crates/persistence/src/postgres/freshness_gate.rs
- crates/persistence/src/postgres/mod.rs
- docs/operations/data-freshness-gating.md
- package.json
- services/execution-engine/src/ingestion/freshness_gate.rs
- services/execution-engine/src/ingestion/mod.rs
- services/execution-engine/src/ingestion/user_stream.rs
- services/execution-engine/src/main.rs
- services/risk-engine/src/gates/mod.rs
- services/risk-engine/src/main.rs

### Change Log

- 2026-04-06: Created Story 2.4 context file and moved lifecycle state from `backlog` to `ready-for-dev`.
- 2026-04-06: Validate Story (VS) remediation added explicit NFR5 `<=5s` breach-to-pause guardrails and corresponding test/evidence requirements; story remains `ready-for-dev`.
- 2026-04-06: Implemented freshness gate contracts, persistence migration/adapter, execution freshness evaluator loop, risk gate pause integration, deterministic stale/recovery tests, story QA command, and operations runbook updates; status moved to `review`.
- 2026-04-06: Code review triage fixed high/medium findings (re-pause stale timestamp reuse after recovery and non-pausing freshness reason normalization), added regression tests, and moved status to `done`.
- 2026-04-06: QA automation follow-up added freshness-gate fail-closed error-path tests (non-UTC sample timestamp, persistence failure propagation, invalid hydrated state evaluation), updated Story 2.4 test summary, and reconfirmed quality gates; status remains `done`.

### Review Findings

- [x] [Review][Patch] Re-pause after `recovery_confirmed` now resets `stale_breach_detected_at_utc` to the current breach sample to keep NFR5 breach-to-pause evidence valid and prevent false contract violations [crates/domain/src/risk.rs].
- [x] [Review][Patch] `set_freshness_pause(true, reason)` now normalizes non-pausing/invalid reason codes (`boundary_safe`, `recovery_confirmed`, unknown) to deterministic fail-closed `freshness_gate_state_unavailable` [services/risk-engine/src/gates/mod.rs].
- [x] [Review][Patch] Added regression tests for both edge cases to prevent recurrence [crates/domain/src/risk.rs, services/risk-engine/src/gates/mod.rs].
- [x] [Review][Defer] Working tree contains `.scripts/bmad-auto/copilot/bmad-progress.log` outside story application scope — deferred as automation artifact.
