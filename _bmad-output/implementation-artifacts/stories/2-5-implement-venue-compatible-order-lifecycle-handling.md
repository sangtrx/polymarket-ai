# Story 2.5: Implement Venue-Compatible Order Lifecycle Handling

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an operator,  
I want reliable submission/cancellation across supported order modes,  
so that order state remains accurate throughout execution.

## Acceptance Criteria

1. **Lifecycle happy path (story-local BDD):**  
   **Given** a limit or reduce-only order is submitted  
   **When** venue state progresses  
   **Then** states are tracked through terminal resolution (filled/canceled/expired).
2. **Invalid transition failure path (story-local BDD):**  
   **Given** an invalid state transition is received  
   **When** transition validation runs  
   **Then** transition is rejected, flagged, and auditable without corrupting canonical order state.
3. **Batch-cancel partial success (story-local BDD):**  
   **Given** a batch-cancel request where some orders are already terminal  
   **When** cancel responses return mixed results  
   **Then** each order receives explicit per-order outcome with retry-safe idempotency keys.
4. **UAC-1 Failure handling:** Invalid payloads, unsupported venue lifecycle states, unavailable venue/persistence dependencies, and unauthorized/auth-expired execution paths return explicit machine-readable reason codes with no unsafe side effects.
5. **UAC-2 Boundary behavior:** Terminal-state and idempotency boundaries are deterministic and test-covered (no terminal-to-nonterminal rollback, duplicates/out-of-order updates do not mutate canonical state, and mixed batch-cancel responses remain deterministic on retry).
6. **UAC-3 Verifiable evidence:** Successful and failed lifecycle operations emit timestamped, correlation-aware audit/telemetry evidence suitable for incident and QA traceability.
7. **Schema/dependency/traceability contract:** Story depends only on `2.3` and `2.4`, introduces only `orders` and `order_state_transitions`, and maps explicitly to `FR12`, `FR13`, and `NFR14`.
8. **NFR14 observability contract:** Lifecycle transitions and cancel outcomes include `correlation_id`, `order_id`, `market_id`, transition/state reason code, and UTC timestamps so signal -> order -> fill timeline correlation remains machine-queryable.

## Tasks / Subtasks

- [x] **Task 1: Expand canonical order lifecycle contracts and transition validation rules** (AC: 1, 2, 4, 5, 7, 8)
  - [x] Extend `crates/domain/src/order.rs` with canonical enums/types for order mode, lifecycle states, transition reasons, and terminal-state detection.
  - [x] Add deterministic transition-validation helpers enforcing allowed state progression and explicit invalid-transition errors.
  - [x] Add idempotency normalization helpers for submission/cancel/batch-cancel keys aligned with existing user-stream normalization conventions.
  - [x] Add domain unit tests for happy-path progression, invalid transition rejection, duplicate idempotency handling, and terminal-state immutability.

- [x] **Task 2: Add forward-only migration for lifecycle persistence scope** (AC: 1, 2, 3, 5, 7, 8)
  - [x] Add migration under `crates/persistence/migrations/` creating only `orders` and `order_state_transitions`.
  - [x] Enforce schema constraints for non-empty IDs, allowed state/reason-code enums, UTC timestamps, and per-order transition sequence monotonicity.
  - [x] Add indexes for latest-order lookup, order history replay, and correlation/time-window incident queries.
  - [x] Ensure migration scope explicitly excludes reconciliation/exposure tables (Story 2.6) and pre-trade policy tables (Stories 2.7/2.8).

- [x] **Task 3: Implement PostgreSQL adapter for order lifecycle writes/reads** (AC: 1, 2, 3, 4, 5, 6, 8)
  - [x] Add `crates/persistence/src/postgres/orders.rs` and wire it through `crates/persistence/src/postgres/mod.rs`.
  - [x] Implement idempotent submission/cancel persistence paths that append immutable transition records and update canonical order projection deterministically.
  - [x] Implement read paths required for runtime hydration and per-order lifecycle replay.
  - [x] Return explicit typed persistence errors; do not swallow constraint, decode, or transaction failures.

- [x] **Task 4: Build execution-engine order lifecycle runtime with venue-compatible mappings** (AC: 1, 2, 3, 4, 5, 6, 8)
  - [x] Replace placeholder `services/execution-engine/src/orders/mod.rs` with runtime/controller seams for submit, cancel, and batch-cancel flows.
  - [x] Map venue/SDK order updates to canonical lifecycle states and reject unknown/invalid mappings with explicit reason codes.
  - [x] Ensure batch-cancel returns explicit per-order outcomes (`canceled`, `already_terminal`, `retryable_failure`, `hard_failure`) with idempotency keys.
  - [x] Emit append-only transition records for every accepted state mutation.

- [x] **Task 5: Integrate user-stream lifecycle progression without breaking existing ingestion guarantees** (AC: 1, 2, 4, 5, 6, 8)
  - [x] Extend `services/execution-engine/src/ingestion/user_stream.rs` accepted-event path to feed canonical order lifecycle updates through the new orders runtime seam.
  - [x] Preserve existing Story 2.3 duplicate/out-of-order behavior as non-mutating for canonical order state.
  - [x] Ensure fresh/stale gate behavior from Story 2.4 remains authoritative for new-order intent blocking while still allowing visibility into existing order progression.
  - [x] Keep correlation IDs and timestamps consistent across ingestion, lifecycle transitions, and risk-gate evidence.

- [x] **Task 6: Add lifecycle telemetry and audit evidence contracts** (AC: 2, 3, 6, 8)
  - [x] Emit structured telemetry events for submit/cancel requests, transition acceptance/rejection, and batch-cancel mixed outcomes.
  - [x] Include canonical evidence fields (`event_id`, `correlation_id`, `order_id`, `market_id`, `from_state`, `to_state`, `reason_code`, `timestamp_utc`).
  - [x] Keep event naming/versioning aligned with architecture event-envelope conventions.

- [x] **Task 7: Add deterministic test coverage and story-scoped QA command** (AC: 1, 2, 3, 4, 5, 6, 8)
  - [x] Add domain tests for valid lifecycle transitions, invalid transition rejection, terminal-state boundaries, and idempotency handling.
  - [x] Add persistence tests validating migration scope, constraints/indexes, and transition append/projection consistency.
  - [x] Add execution-engine tests for lifecycle happy path, invalid transition rejection, and batch-cancel partial-success determinism.
  - [x] Add integration tests proving user-stream duplicate/out-of-order events do not corrupt canonical lifecycle state.
  - [x] Add `qa:test:story-2-5` to `package.json` following Epic 2 QA command conventions.

- [x] **Task 8: Update order-lifecycle operations runbook** (AC: 3, 6, 8)
  - [x] Add/update `docs/operations/*order*lifecycle*.md` with state model, cancel semantics, idempotency retry guidance, and partial-cancel incident handling.
  - [x] Document failure-mode response for invalid-transition alerts, persistence outages, and venue mismatch scenarios.

### Review Findings

- [x] [Review][Patch] Initial order-transition persistence could violate `order_state_transitions.order_id -> orders.order_id` foreign key on first write [`crates/persistence/src/postgres/orders.rs`] — fixed by inserting the `orders` projection before inserting the first transition, while keeping both writes in the same transaction.

## Dev Notes

### Technical Requirements

- Story dependency is strict: `2.5` depends only on `2.3` and `2.4` (no forward dependencies).
- Story scope is explicit and narrow:
  - Functional scope: venue-compatible submission/cancel/batch-cancel lifecycle handling only.
  - Schema scope: `orders`, `order_state_transitions` only.
  - Traceability scope: `FR12`, `FR13`, `NFR14`.
- Functional guardrails:
  - Support venue-compatible order modes required by FR12: limit, reduce-only, and batch-cancel workflows.
  - Canonical lifecycle must include pending/live/partially-filled/filled/canceled/expired progression with deterministic terminal handling.
  - Invalid transitions must be rejected without mutating canonical state and with explicit machine-readable reason codes.
  - Batch-cancel mixed outcomes must be explicit per-order and retry-safe via idempotency keys.
  - Lifecycle telemetry and persistence evidence must preserve correlation and UTC timestamps for incident forensics.
- **Out of scope for Story 2.5:** reconciliation/exposure truth pipelines (Story 2.6), limit policy configuration (Story 2.7), and full pre-trade gate composition (Story 2.8).

[Source: _bmad-output/planning-artifacts/epics.md#Story 2.5: Implement Venue-Compatible Order Lifecycle Handling]  
[Source: _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)]  
[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: _bmad-output/planning-artifacts/prd.md#Trade Execution & Order Management]  
[Source: _bmad-output/planning-artifacts/prd.md#Observability & Operability]

### Architecture Compliance

- Keep service boundaries explicit and consistent:
  - `services/execution-engine/src/orders` owns order-lifecycle command handling and venue-state mapping.
  - `services/execution-engine/src/ingestion/user_stream.rs` remains ingestion/order-event normalization seam.
  - shared contracts and persistence adapters stay in workspace crates (`domain`, `persistence`).
- Preserve architecture safety and process rules:
  - fail closed on uncertain lifecycle state or persistence uncertainty,
  - no swallowed errors in execution/risk-critical paths,
  - append-only transition evidence for auditable mutation history.
- Keep naming and envelope conventions consistent:
  - snake_case modules and DB identifiers,
  - UTC timestamps only,
  - machine-readable reason codes and explicit `correlation_id`.
- Follow requirements-to-structure mapping for FR12-FR16 in `services/execution-engine/src/orders`.

[Source: _bmad-output/planning-artifacts/architecture.md#Technical Constraints & Dependencies]  
[Source: _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: _bmad-output/planning-artifacts/architecture.md#Implementation Handoff]

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
- Do not introduce opportunistic dependency upgrades in Story 2.5; prioritize deterministic lifecycle correctness and workspace consistency.
- Use official SDK/protocol surfaces for lifecycle semantics; do not add undocumented contract call paths.

[Source: Cargo.toml]  
[Source: https://crates.io/api/v1/crates/polymarket-client-sdk]  
[Source: https://crates.io/api/v1/crates/tokio]  
[Source: https://crates.io/api/v1/crates/sqlx]  
[Source: https://crates.io/api/v1/crates/axum]  
[Source: https://crates.io/api/v1/crates/time]  
[Source: _bmad-output/planning-artifacts/prd.md#Smart-Contract / Protocol Interaction Boundaries]

### File Structure Requirements

- Primary implementation surfaces:
  - `crates/domain/src/order.rs`
  - `crates/domain/src/lib.rs` (exports)
  - `crates/persistence/migrations/*order*lifecycle*.sql`
  - `crates/persistence/src/postgres/{mod.rs,orders.rs}`
  - `services/execution-engine/src/orders/mod.rs`
  - `services/execution-engine/src/ingestion/user_stream.rs`
  - `services/execution-engine/src/main.rs`
  - `package.json`
  - `docs/operations/*order*lifecycle*.md`
- Keep Story 2 sequencing intact:
  - do not pre-implement reconciliation diff/exposure snapshots from Story 2.6,
  - do not pre-implement full policy/evaluation gate orchestration from Stories 2.7 and 2.8.
- Reuse established Story 2.2/2.3/2.4 patterns for reason-code taxonomy, fail-closed semantics, telemetry emission, and seam-based runtime integration.

[Source: _bmad-output/planning-artifacts/architecture.md#Complete Project Directory Structure]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: services/execution-engine/src/ingestion/user_stream.rs]  
[Source: services/execution-engine/src/orders/mod.rs]  
[Source: crates/domain/src/order.rs]  
[Source: crates/persistence/src/postgres/mod.rs]

### Testing Requirements

- Add deterministic coverage for:
  - lifecycle happy path from submission through terminal state,
  - invalid transition rejection with no canonical-state corruption,
  - batch-cancel partial success with explicit per-order outcomes,
  - duplicate/out-of-order idempotency behavior with no unsafe mutation,
  - terminal-state boundary behavior (already terminal orders remain terminal),
  - explicit machine-readable error behavior for invalid payload/auth/persistence failures.
- Keep quality-gate compatibility with existing repository workflows:
  - targeted crate tests (`domain`, `persistence`, `execution-engine`, `risk-engine` as applicable),
  - story-scoped QA command pattern in `package.json`,
  - workspace-level rust checks via existing scripts.
- Evidence assertions must validate reason codes, UTC timestamps, and correlation IDs across submission/cancel/transition flows.

[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: package.json]  
[Source: _bmad-output/implementation-artifacts/stories/2-3-ingest-authenticated-user-stream-with-ordering-guarantees.md#Testing Requirements]  
[Source: _bmad-output/implementation-artifacts/stories/2-4-enforce-data-freshness-gates-and-stale-feed-pausing.md#Testing Requirements]

### Previous Story Intelligence

- Story 2.3 established deterministic user-stream ordering and idempotency primitives:
  - `user_stream_events` + `order_event_offsets` persistence contracts,
  - duplicate/out-of-order suppression and auth-expiry fail-safe behavior,
  - normalized idempotency/correlation conventions that Story 2.5 should reuse.
- Story 2.4 established fail-closed freshness-pause behavior and risk-gate integration:
  - new-order intent blocking while stale/uncertain,
  - deterministic machine-readable reason propagation through risk gate decisions.
- Story 2.5 should extend these foundations instead of creating parallel lifecycle stores or telemetry formats.

[Source: _bmad-output/implementation-artifacts/stories/2-3-ingest-authenticated-user-stream-with-ordering-guarantees.md#Dev Notes]  
[Source: _bmad-output/implementation-artifacts/stories/2-4-enforce-data-freshness-gates-and-stale-feed-pausing.md#Dev Notes]  
[Source: services/execution-engine/src/ingestion/user_stream.rs]  
[Source: services/risk-engine/src/gates/mod.rs]

### Git Intelligence Summary

- Recent Epic 2 commits reinforce an implementation pattern that Story 2.5 should preserve:
  1. domain contracts/reason codes,  
  2. scoped forward-only migration,  
  3. persistence adapter wiring,  
  4. runtime integration,  
  5. deterministic tests + story QA script.
- File-change history confirms Story 2.5 should build incrementally on current ingestion/risk surfaces rather than refactoring prior stories.

[Source: git --no-pager log --oneline -5]  
[Source: git --no-pager log --name-only --pretty=format:'%h %s' -5]

### Latest Technical Information

- Official Polymarket lifecycle integration remains aligned with story requirements:
  - authenticated user WS order/trade lifecycle events,
  - venue lifecycle support for limit/cancel/batch operations,
  - heartbeat/watchdog reliability expectations for fail-safe behavior.
- No dependency upgrade is required to implement Story 2.5 safely.

[Source: _bmad-output/planning-artifacts/research/market-rust-polymarket-trading-system-lopez-de-prado-research-2026-04-04.md#Integration Patterns Analysis]  
[Source: _bmad-output/planning-artifacts/research/market-rust-polymarket-trading-system-lopez-de-prado-research-2026-04-04.md#Architectural Patterns and Design]  
[Source: _bmad-output/planning-artifacts/research/market-rust-polymarket-trading-system-lopez-de-prado-research-2026-04-04.md#Requirement Coverage Matrix]

### Project Context Reference

- No `project-context.md` file was found in repository scope during discovery.
- Context for this story was derived from epics, PRD, architecture, UX, readiness/research artifacts, previous stories, git history, and current codebase surfaces.

### Project Structure Notes

- Current Epic 2 foundation already provides:
  - user-stream ingestion ordering/idempotency and auth fail-safe controls,
  - data-freshness gate pause/recovery semantics,
  - risk-engine order-intent gate deny/allow seam with machine-readable reason codes.
- `services/execution-engine/src/orders/mod.rs` is currently a placeholder, and `crates/domain/src/order.rs` is intentionally minimal, making Story 2.5 the canonical place to establish lifecycle domain + runtime contracts.
- Ensure lifecycle evidence remains compatible with operator incident/timeline UX expectations (signal -> order -> fill -> PnL traceability).

[Source: services/execution-engine/src/orders/mod.rs]  
[Source: crates/domain/src/order.rs]  
[Source: services/execution-engine/src/ingestion/user_stream.rs]  
[Source: services/risk-engine/src/gates/mod.rs]  
[Source: _bmad-output/planning-artifacts/ux-design-specification.md#Journey 2 — Incident Safe-State Workflow]  
[Source: _bmad-output/planning-artifacts/ux-design-specification.md#Action Rail (Safety Controls)]  
[Source: _bmad-output/planning-artifacts/ux-design-specification.md#Causal Timeline Panel]

### References

- _bmad-output/planning-artifacts/epics.md#Epic 2: Live Market Connectivity & Safe Core Execution  
- _bmad-output/planning-artifacts/epics.md#Story 2.5: Implement Venue-Compatible Order Lifecycle Handling  
- _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)  
- _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)  
- _bmad-output/planning-artifacts/prd.md#Trade Execution & Order Management  
- _bmad-output/planning-artifacts/prd.md#Observability & Operability  
- _bmad-output/planning-artifacts/prd.md#Smart-Contract / Protocol Interaction Boundaries  
- _bmad-output/planning-artifacts/architecture.md#Technical Constraints & Dependencies  
- _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules  
- _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping  
- _bmad-output/planning-artifacts/architecture.md#Implementation Handoff  
- _bmad-output/planning-artifacts/ux-design-specification.md#Journey 2 — Incident Safe-State Workflow  
- _bmad-output/planning-artifacts/ux-design-specification.md#Action Rail (Safety Controls)  
- _bmad-output/planning-artifacts/ux-design-specification.md#Causal Timeline Panel  
- _bmad-output/planning-artifacts/implementation-readiness-report-2026-04-05.md#✅ Remediation Outcomes  
- _bmad-output/planning-artifacts/research/market-rust-polymarket-trading-system-lopez-de-prado-research-2026-04-04.md#Integration Patterns Analysis  
- _bmad-output/planning-artifacts/research/market-rust-polymarket-trading-system-lopez-de-prado-research-2026-04-04.md#Architectural Patterns and Design  
- _bmad-output/implementation-artifacts/stories/2-3-ingest-authenticated-user-stream-with-ordering-guarantees.md  
- _bmad-output/implementation-artifacts/stories/2-4-enforce-data-freshness-gates-and-stale-feed-pausing.md  
- services/execution-engine/src/orders/mod.rs  
- services/execution-engine/src/ingestion/user_stream.rs  
- services/risk-engine/src/gates/mod.rs  
- crates/domain/src/order.rs  
- crates/persistence/src/postgres/mod.rs  
- Cargo.toml

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
- Dependency/version checks via crates.io API endpoints
- Implemented lifecycle domain contracts, persistence migration/adapter, execution runtime, and user-stream seam integration for Story 2.5
- Added and executed story-scoped QA command `qa:test:story-2-5`
- Executed full Rust quality gates (`rust:fmt`, `rust:lint`, `rust:test`, `rust:build`)
- Executed adversarial code-review workflow, triaged findings, and auto-fixed HIGH/MEDIUM issues
- QA automation follow-up generated Story 2.5 unsupported-venue-state and batch-cancel retry-determinism tests; reran `npm run --silent qa:test:story-2-5`, `npm run --silent rust:lint`, and `npm run --silent rust:build`.

### Completion Notes List

- Selected first backlog story: `2-5-implement-venue-compatible-order-lifecycle-handling`.
- Captured story-local BDD acceptance criteria plus universal UAC contracts.
- Added implementation guardrails for strict schema scope, deterministic lifecycle transitions, terminal-state immutability, and retry-safe idempotency.
- Carried forward Story 2.3/2.4 patterns for fail-closed behavior, reason-code taxonomy, and seam-based runtime integration.
- Defined concrete implementation surfaces, testing requirements, and operations runbook expectations for Story 2.5 execution.
- Implemented canonical order lifecycle domain contracts with deterministic transition validation, terminal-state immutability enforcement, and normalized submit/cancel/batch idempotency helpers.
- Added forward-only migration `20260406050000_order_lifecycle.sql` introducing only `orders` and `order_state_transitions` with monotonic transition sequencing and incident-query indexes.
- Added PostgreSQL lifecycle adapter (`crates/persistence/src/postgres/orders.rs`) for idempotent transition writes, canonical projection updates, and replay/hydration reads with typed machine-readable errors.
- Replaced execution-engine orders placeholder with runtime/controller seams for submit/cancel/batch-cancel and venue/user-stream mapping into canonical lifecycle states.
- Integrated accepted user-stream events into lifecycle runtime while preserving duplicate/out-of-order non-mutating behavior and freshness-gate fail-closed authority for new intents.
- Added structured lifecycle telemetry for transition accept/reject and batch-cancel outcomes with `event_id`, `correlation_id`, `order_id`, `market_id`, `from_state`, `to_state`, `reason_code`, and UTC timestamps.
- Added deterministic Story 2.5 test coverage across domain, persistence, execution runtime, and ingestion integration; updated `package.json` with `qa:test:story-2-5`.
- Added operations runbook `docs/operations/order-lifecycle-handling.md` covering lifecycle state model, cancel semantics, idempotency retry guidance, and incident response.
- Fixed initial Postgres lifecycle persistence ordering so first-write transitions satisfy `orders` foreign-key constraints atomically.
- Cross-checked story File List against git reality; excluded non-application automation log drift from review scope.
- Added QA automation regression coverage for unsupported venue order states/message types (fail-closed, no-side-effect guarantees) and retry-safe batch-cancel idempotency determinism; refreshed Story 2.5 test summary evidence.

### File List

- _bmad-output/implementation-artifacts/stories/2-5-implement-venue-compatible-order-lifecycle-handling.md
- _bmad-output/implementation-artifacts/sprint-status.yaml
- _bmad-output/implementation-artifacts/tests/test-summary.md
- crates/domain/src/order.rs
- crates/persistence/migrations/20260406050000_order_lifecycle.sql
- crates/persistence/src/postgres/mod.rs
- crates/persistence/src/postgres/orders.rs
- services/execution-engine/src/orders/mod.rs
- services/execution-engine/src/ingestion/user_stream.rs
- services/execution-engine/src/main.rs
- services/execution-engine/src/ingestion/freshness_gate.rs
- package.json
- docs/operations/order-lifecycle-handling.md

### Change Log

- 2026-04-06: Created Story 2.5 context file and moved lifecycle state from `backlog` to `ready-for-dev`.
- 2026-04-06: Implemented Story 2.5 lifecycle domain, persistence, runtime integration, telemetry evidence contracts, deterministic QA coverage, and operations runbook; moved story to `review`.
- 2026-04-06: Completed adversarial code review, auto-fixed lifecycle persistence FK ordering defect, re-ran quality gates, and moved story to `done`.
- 2026-04-06: QA automation follow-up added unsupported-venue-state and batch-cancel retry-determinism coverage, updated Story 2.5 QA test summary, reran story QA/lint/build gates, and kept status as `done`.
