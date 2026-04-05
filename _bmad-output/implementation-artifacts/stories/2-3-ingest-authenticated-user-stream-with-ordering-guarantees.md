# Story 2.3: Ingest Authenticated User Stream with Ordering Guarantees

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an execution service,  
I want to ingest authenticated user-order events with deterministic ordering,  
so that order/fill state remains trustworthy for risk decisions.

## Acceptance Criteria

1. **Event persistence SLA (story-local BDD):**  
   **Given** authenticated stream connectivity is healthy  
   **When** order/fill/cancel events arrive  
   **Then** 99% of events are persisted within 2 seconds with monotonic offset tracking.
2. **Duplicate/out-of-order handling (story-local BDD):**  
   **Given** duplicate or out-of-order events are received  
   **When** dedupe/order checks run  
   **Then** duplicates are ignored and order state remains idempotent and consistent.
3. **Auth-expiry failure path (story-local BDD):**  
   **Given** stream authentication expires  
   **When** reconnect attempts begin  
   **Then** new trading intents are blocked until authenticated state is restored.
4. **UAC-1 Failure handling:** Invalid auth payloads, malformed user events, persistence failures, and auth/reconnect failures must return explicit machine-readable reason codes and preserve fail-closed behavior.
5. **UAC-2 Boundary behavior:** Ordering boundaries are deterministic and test-covered for duplicate key equality, equal-offset tie cases, and lower-than-last-offset out-of-order events.
6. **UAC-3 Verifiable evidence:** Successful ingest, duplicate suppression, out-of-order rejections, and auth-state transitions emit timestamped telemetry with correlation metadata suitable for incident and QA traceability.
7. **Schema/dependency/traceability contract:** Story depends only on `2.2`, introduces only `user_stream_events` and `order_event_offsets`, and maps explicitly to `FR3`, `NFR3`, and `NFR14`.

## Tasks / Subtasks

- [x] **Task 1: Define canonical authenticated-user-stream domain contracts** (AC: 1, 2, 4, 5, 7)
  - [x] Extend `crates/domain/src/risk.rs` with typed user-stream contracts for order/trade lifecycle events, ordering cursor state, and auth-state transitions.
  - [x] Add machine-readable reason codes for duplicate, out-of-order, auth-expired, and persistence-unavailable paths.
  - [x] Add deterministic ordering helpers (monotonic cursor compare + idempotency key normalization) and strict validation for required IDs/timestamps/UTC format.
  - [x] Keep naming and envelope conventions consistent with existing market-stream contracts from Story 2.2.

- [x] **Task 2: Add forward-only persistence migration for user stream state** (AC: 1, 2, 3, 5, 7)
  - [x] Add a migration in `crates/persistence/migrations/` that creates only `user_stream_events` and `order_event_offsets`.
  - [x] Enforce constraints for non-empty identifiers, valid status/reason values, UTC timestamps, non-negative offsets, and monotonic offset semantics.
  - [x] Add indexes for low-latency lookups by order/market/time plus offset-cursor retrieval paths.
  - [x] Keep migration scope strict; do not introduce `freshness_gate_events`, `orders`, or reconciliation tables.

- [x] **Task 3: Implement user-stream persistence adapters with idempotent ordering updates** (AC: 1, 2, 4, 5, 6)
  - [x] Add `crates/persistence/src/postgres/user_stream.rs` and wire it through `crates/persistence/src/postgres/mod.rs`.
  - [x] Implement durable writes for accepted user events and cursor/auth-state updates in `order_event_offsets`.
  - [x] Enforce idempotency: duplicate events become no-op writes; lower-offset events are rejected deterministically with machine-readable reason code.
  - [x] Return explicit error codes (no silent retries or swallowed SQL failures).

- [x] **Task 4: Implement execution-engine authenticated user-stream runtime** (AC: 1, 2, 4, 6)
  - [x] Introduce runtime config + env parsing for user stream endpoint/market targets/reconnect controls in `services/execution-engine/src/ingestion/*`.
  - [x] Use official SDK authenticated flow (`Client::new(...).authenticate(...).subscribe_orders(...)` and `subscribe_trades(...)`) for user stream ingestion.
  - [x] Normalize SDK `OrderMessage`/`TradeMessage` payloads into canonical domain contracts and persist with <2s latency target instrumentation.
  - [x] Emit structured telemetry for accepted, duplicate, out-of-order, and persistence-failure paths with correlation IDs.

- [x] **Task 5: Enforce deterministic duplicate and out-of-order handling** (AC: 2, 4, 5, 6)
  - [x] Implement offset cursor evaluation using persisted `order_event_offsets` state before mutating user-stream event state.
  - [x] Treat same-id/same-offset repeats as duplicates (ignored, telemetry emitted) and lower-than-last offset as out-of-order (ignored, telemetry emitted).
  - [x] Ensure resulting order-event state remains idempotent and never regresses when stale events arrive.
  - [x] Add deterministic tie-break behavior for equal-offset but different-event cases.

- [x] **Task 6: Implement auth-expiry fail-safe gating for new intents** (AC: 3, 4, 6)
  - [x] Classify SDK auth failures (`authenticate` failures and user-channel `AuthenticationFailed` paths) into explicit `user_stream_auth_expired`-class reason codes.
  - [x] Persist auth-state transition in `order_event_offsets` and publish fail-closed signal (`block_new_intents = true`) until re-auth success is confirmed.
  - [x] Extend risk gate seam in `services/risk-engine/src/gates/mod.rs` to deny new intents while auth-expired block is active.
  - [x] Clear block only after authenticated stream restoration and first successful post-recovery user event persistence.

- [x] **Task 7: Wire bootstrap/runtime configuration and operational docs** (AC: 1, 3, 4, 6)
  - [x] Update `services/execution-engine/src/main.rs` startup so market-stream (2.2) and user-stream (2.3) runtimes can run together with fail-closed behavior.
  - [x] Update `.env.example` with required user-stream/auth placeholders (API secret/passphrase/address + user stream config keys) without introducing plaintext secrets.
  - [x] Add `docs/operations/user-stream-ingestion.md` with auth-expiry, reconnect, duplicate/out-of-order, and persistence-failure runbook steps.
  - [x] Keep existing Story 2.2 market-stream behavior unchanged and backward-compatible.

- [x] **Task 8: Add deterministic tests and story QA command coverage** (AC: 1, 2, 3, 4, 5, 6)
  - [x] Add domain tests for user event validation, offset monotonicity, duplicate/out-of-order classification, and reason-code determinism.
  - [x] Add persistence tests for migration scope, constraints/indexes, and monotonic cursor update semantics.
  - [x] Add execution-engine tests proving:  
    - [x] 99% user-event persistence path meets `<= 2s` under controlled load,  
    - [x] duplicates/out-of-order events are ignored without state regression,  
    - [x] auth-expiry forces intent-block signal until authenticated recovery.  
  - [x] Add/extend risk-engine gate tests for auth-block deny behavior and recovery unblock path.
  - [x] Add a story-scoped QA command `qa:test:story-2-3` in `package.json` following existing Epic 2 convention.

### Review Findings

- [x] [Review][Patch] Fail-closed dual-runtime bootstrap now exits immediately on the first market/user stream runtime halt instead of waiting for both loops to terminate [`services/execution-engine/src/main.rs`].
- [x] [Review][Defer] `.scripts/bmad-auto/copilot/bmad-progress.log` appears in git changes but is non-application automation output and intentionally excluded from code review scope.

## Dev Notes

### Technical Requirements

- Story dependency is strict: `2.3` depends only on `2.2` (no forward dependencies).
- Scope is explicit and narrow:
  - Functional scope: authenticated user order/trade ingest with deterministic ordering/idempotency.
  - Schema scope: `user_stream_events`, `order_event_offsets` only.
  - Traceability scope: `FR3`, `NFR3`, `NFR14`.
- This story must preserve all three local BDD paths (SLA, duplicate/out-of-order handling, auth-expiry blocking).
- Ordering guidance for implementation:
  - Use a deterministic per-partition cursor (`order_event_offsets`) and compare incoming event offset before state mutation.
  - If incoming offset is lower than stored cursor, classify as out-of-order and ignore.
  - If incoming event key matches existing cursor/event identity, classify as duplicate and ignore.
  - Equal-offset non-identical events require deterministic tie-break (stable lexical or explicit precedence rule) and test coverage.
- **Out of scope:** stale-feed pause orchestration (Story 2.4), order lifecycle execution APIs/state machine (Story 2.5), reconciliation visibility (Story 2.6).

[Source: _bmad-output/planning-artifacts/epics.md#Story 2.3: Ingest Authenticated User Stream with Ordering Guarantees]  
[Source: _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)]  
[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: _bmad-output/planning-artifacts/prd.md#Market Universe & Data Intake]  
[Source: _bmad-output/planning-artifacts/prd.md#Non-Functional Requirements]

### Architecture Compliance

- Keep architecture boundaries explicit:
  - `services/execution-engine/src/ingestion` owns market/user ingest implementation (FR1–FR5 slice).
  - `risk-engine` owns allow/deny gate decisions for new intents; execution-engine must emit consumable auth-state evidence, not bypass risk ownership.
  - Shared contracts remain in workspace crates (`domain`, `persistence`) to avoid duplicated policy logic.
- Preserve communication and data standards:
  - snake_case naming in Rust/DB surfaces,
  - machine-readable reason/error codes,
  - UTC timestamps only (RFC3339),
  - explicit correlation metadata.
- Safety-first rule is mandatory: uncertain auth state must fail closed (intent blocking) until authenticated stream health is restored.

[Source: _bmad-output/planning-artifacts/architecture.md#Technical Constraints & Dependencies]  
[Source: _bmad-output/planning-artifacts/architecture.md#API & Communication Patterns]  
[Source: _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: _bmad-output/planning-artifacts/architecture.md#Implementation Handoff]

### Library & Framework Requirements

- Continue workspace-pinned stack for consistency:
  - `polymarket-client-sdk = 0.4.4` (`clob`, `ws`)
  - `tokio = 1.48.0`
  - `sqlx = 0.8.6`
  - `axum = 0.8.8`
  - `time = 0.3.44`
- SDK user-stream integration requirements:
  - authenticated client state is required for user channel methods,
  - use `subscribe_user_events` / `subscribe_orders` / `subscribe_trades` for lifecycle events,
  - map `WsMessage::Order` and `WsMessage::Trade` payloads to canonical contracts.
- Latest stable checks at story-creation time:
  - `polymarket-client-sdk`: `0.4.4`
  - `tokio`: `1.51.0`
  - `sqlx`: `0.8.6`
  - `axum`: `0.8.8`
  - `time`: `0.3.47`
- Do not perform opportunistic dependency upgrades in this story; prioritize deterministic behavior and workspace compatibility.

[Source: Cargo.toml]  
[Source: services/execution-engine/Cargo.toml]  
[Source: https://github.com/Polymarket/rs-clob-client/blob/77264a4eab775fd0b094deb1c72575c712ab74fb/src/clob/ws/client.rs]  
[Source: https://github.com/Polymarket/rs-clob-client/blob/77264a4eab775fd0b094deb1c72575c712ab74fb/src/clob/ws/types/response.rs]  
[Source: https://github.com/Polymarket/rs-clob-client/blob/77264a4eab775fd0b094deb1c72575c712ab74fb/src/ws/error.rs]  
[Source: https://crates.io/api/v1/crates/polymarket-client-sdk]  
[Source: https://crates.io/api/v1/crates/tokio]  
[Source: https://crates.io/api/v1/crates/sqlx]  
[Source: https://crates.io/api/v1/crates/axum]  
[Source: https://crates.io/api/v1/crates/time]

### File Structure Requirements

- Primary implementation surfaces:
  - `services/execution-engine/src/ingestion/*` (authenticated user-stream runtime + mapping + telemetry)
  - `services/execution-engine/src/main.rs` (runtime bootstrap/wiring)
  - `crates/domain/src/risk.rs` (canonical user-stream contracts, reason codes, validators)
  - `crates/persistence/migrations/*user_stream*.sql`
  - `crates/persistence/src/postgres/{mod.rs,user_stream.rs}`
  - `services/risk-engine/src/gates/mod.rs` (intent-block behavior tied to auth state)
  - `.env.example`
  - `docs/operations/user-stream-ingestion.md`
- Preserve staged Story 2 sequencing:
  - do not implement Story 2.4 freshness gates beyond auth-expiry blocking required by this story,
  - do not pre-implement full order state machine/execution actions from Story 2.5.
- Keep Story 2.2 market-stream module behavior stable while adding user-stream capabilities.

[Source: _bmad-output/planning-artifacts/architecture.md#Complete Project Directory Structure]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: services/execution-engine/src/ingestion/mod.rs]  
[Source: services/execution-engine/src/main.rs]  
[Source: crates/persistence/src/postgres/market_stream.rs]  
[Source: services/risk-engine/src/gates/mod.rs]

### Testing Requirements

- Add deterministic coverage for:
  - event persistence SLA path (`99% <= 2s` under controlled normal load),
  - duplicate message suppression,
  - out-of-order rejection without state regression,
  - auth-expiry -> block-new-intents path and authenticated recovery unblock,
  - boundary ties (equal offset, equal timestamp, empty/invalid IDs, non-UTC timestamps).
- Keep quality-gate compatibility with existing workflows:
  - targeted crate tests in domain/persistence/execution-engine/risk-engine,
  - existing workspace checks via `ci:rust` / `test`,
  - story-scoped QA command pattern in `package.json`.
- Assertions must validate telemetry evidence shape (reason codes, correlation IDs, UTC timestamps) for both success and failure paths.

[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: package.json]  
[Source: _bmad-output/implementation-artifacts/stories/2-2-ingest-market-stream-with-latency-guarantees.md#Testing Requirements]

### Previous Story Intelligence

- Story 2.2 established non-negotiable ingestion patterns that must continue:
  - machine-readable reason codes across all deny/error paths,
  - explicit persistence-failure telemetry (no silent degradation),
  - bounded reconnect behavior,
  - deterministic, collision-resistant IDs for quarantined/derived events,
  - strict schema-per-story discipline with migration scope tests.
- Reuse existing market-stream timestamp parsing/UTC validation/correlation conventions where possible instead of re-inventing utilities.
- Preserve fail-closed behavior: uncertainty in stream/auth state should block risky progression until health is re-established.

[Source: _bmad-output/implementation-artifacts/stories/2-2-ingest-market-stream-with-latency-guarantees.md#Dev Notes]  
[Source: _bmad-output/implementation-artifacts/stories/2-2-ingest-market-stream-with-latency-guarantees.md#Review Findings]  
[Source: services/execution-engine/src/ingestion/mod.rs]  
[Source: crates/persistence/src/postgres/market_stream.rs]

### Git Intelligence Summary

- Recent commits follow a consistent implementation order:
  1) domain contracts/reason codes,  
  2) narrow migration scope,  
  3) persistence adapters,  
  4) service runtime wiring,  
  5) deterministic tests + story QA command.  
- Follow the same sequence to reduce integration risk for Story 2.3.

[Source: git --no-pager log --oneline -5]  
[Source: git --no-pager log -5 --name-only --pretty=format:'%h %s']

### Latest Technical Information

- Official user-channel support remains available in the SDK:
  - authenticated user stream endpoint and methods for order/trade subscriptions,
  - `WsMessage` user event shapes include `order` and `trade` event types with lifecycle fields.
- Research and source docs align on required user-stream controls:
  - auth payload requires API key/secret/passphrase,
  - user-stream reliability needs ping/pong + reconnect watchdog behavior,
  - auth anomalies are expected safety triggers.

[Source: _bmad-output/planning-artifacts/research/market-rust-polymarket-trading-system-lopez-de-prado-research-2026-04-04.md#Integration Patterns Analysis]  
[Source: _bmad-output/planning-artifacts/research/domain-polymarket-rust-clob-profit-research-2026-04-04.md#Required Reliability Controls Are Officially Documented]  
[Source: https://github.com/Polymarket/rs-clob-client/blob/77264a4eab775fd0b094deb1c72575c712ab74fb/src/clob/ws/client.rs]  
[Source: https://github.com/Polymarket/rs-clob-client/blob/77264a4eab775fd0b094deb1c72575c712ab74fb/src/clob/ws/types/response.rs]

### Project Context Reference

- No `project-context.md` file was found in repository scope during discovery.
- Context for this story was derived from epics, PRD, architecture, research artifacts, previous story artifacts, git history, and current codebase surfaces.

### Project Structure Notes

- Current codebase already has a production-like market-stream ingestion runtime (Story 2.2) and placeholder order/reconciliation modules.
- Story 2.3 should add authenticated user-stream ingestion + auth-block gating without collapsing future Story 2.4/2.5 boundaries.
- Keep service ownership intact: execution-engine emits user-stream truth; risk-engine consumes gating truth for intent decisions.

[Source: services/execution-engine/src/ingestion/mod.rs]  
[Source: services/execution-engine/src/orders/mod.rs]  
[Source: services/execution-engine/src/reconciliation/mod.rs]  
[Source: services/risk-engine/src/gates/mod.rs]  
[Source: _bmad-output/planning-artifacts/epics.md#Story 2.4: Enforce Data Freshness Gates and Stale-Feed Pausing]

### References

- _bmad-output/planning-artifacts/epics.md#Epic 2: Live Market Connectivity & Safe Core Execution  
- _bmad-output/planning-artifacts/epics.md#Story 2.3: Ingest Authenticated User Stream with Ordering Guarantees  
- _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)  
- _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)  
- _bmad-output/planning-artifacts/prd.md#Market Universe & Data Intake  
- _bmad-output/planning-artifacts/prd.md#Technical Constraints  
- _bmad-output/planning-artifacts/prd.md#Integration Requirements  
- _bmad-output/planning-artifacts/prd.md#Blockchain Web3 Specific Requirements  
- _bmad-output/planning-artifacts/prd.md#Non-Functional Requirements  
- _bmad-output/planning-artifacts/architecture.md#Technical Constraints & Dependencies  
- _bmad-output/planning-artifacts/architecture.md#API & Communication Patterns  
- _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules  
- _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping  
- _bmad-output/planning-artifacts/architecture.md#Implementation Handoff  
- _bmad-output/planning-artifacts/research/market-rust-polymarket-trading-system-lopez-de-prado-research-2026-04-04.md  
- _bmad-output/planning-artifacts/research/domain-polymarket-rust-clob-profit-research-2026-04-04.md  
- _bmad-output/planning-artifacts/implementation-readiness-report-2026-04-05.md  
- _bmad-output/implementation-artifacts/stories/2-2-ingest-market-stream-with-latency-guarantees.md  
- Cargo.toml  
- .env.example  
- services/execution-engine/src/ingestion/mod.rs  
- crates/persistence/src/postgres/market_stream.rs  
- services/risk-engine/src/gates/mod.rs

## Story Completion Status

- Story context generated with exhaustive artifact analysis (workflow inputs, epic/PRD/architecture/UX/research/readiness artifacts, previous story, git history, and current codebase surfaces).
- Story file is created and ready for implementation by dev agents.
- Completion note: Ultimate context engine analysis completed - comprehensive developer guide created.

## Dev Agent Record

### Agent Model Used

GPT-5.3-Codex (gpt-5.3-codex)

### Debug Log References

- BMAD create-story workflow execution (automated)
- Sprint backlog discovery from `_bmad-output/implementation-artifacts/sprint-status.yaml`
- Artifact discovery across planning/implementation/code/research surfaces
- SDK source review for authenticated user stream methods and event payloads
- Latest crate-version checks for ingestion-relevant dependencies
- Domain user-stream contract and deterministic ordering implementation in `crates/domain/src/risk.rs`
- Forward-only user-stream persistence migration and adapters in `crates/persistence/migrations/20260406030000_user_stream_ingestion.sql` and `crates/persistence/src/postgres/user_stream.rs`
- Execution-engine authenticated user-stream runtime implementation in `services/execution-engine/src/ingestion/user_stream.rs`
- Execution-engine bootstrap wiring for concurrent market/user ingestion in `services/execution-engine/src/main.rs`
- Risk-gate auth-block seam implementation in `services/risk-engine/src/gates/mod.rs`
- Adversarial code review triage and fail-closed runtime orchestration patch in `services/execution-engine/src/main.rs`
- Story-scoped verification command `npm run qa:test:story-2-3`
- Full quality gates and regression checks: `npm run ci:rust` and `npm test`
- BMAD QA automation workflow execution for Story 2.3 with additional user-stream runtime tests (machine-readable outcome evidence, status-only matched/cancellation mapping, and non-UTC ingest rejection)

### Completion Notes List

- Selected first backlog story: `2-3-ingest-authenticated-user-stream-with-ordering-guarantees`.
- Captured story-local BDD acceptance criteria plus universal failure/boundary/evidence contracts.
- Added implementation guardrails for ordering/idempotency, auth-expiry intent blocking, schema scope, architecture boundaries, and deterministic telemetry.
- Integrated prior Story 2.2 learnings (bounded reconnect, explicit persistence-failure evidence, collision-safe identifiers, strict per-story schema discipline).
- Implemented canonical authenticated user-stream domain contracts, reason-code taxonomy, strict UTC validation, and deterministic duplicate/out-of-order/tie-break ordering helpers.
- Added forward-only persistence schema for `user_stream_events` and `order_event_offsets` with monotonic offset trigger enforcement and low-latency lookup indexes.
- Implemented Postgres user-stream persistence adapters with fail-closed error envelopes and idempotent/no-op behavior for duplicate and stale events.
- Added authenticated websocket runtime using official SDK `authenticate + subscribe_orders + subscribe_trades` flow with latency instrumentation and structured telemetry evidence.
- Implemented auth-expiry fail-safe transitions that persist `block_new_intents` state and only clear intent blocking after first successful post-recovery event persistence.
- Extended risk gate seam to deny order intents while user-stream auth block is active and added deterministic recovery unblock tests.
- Added operational runbook for user-stream ingestion/auth/reconnect/ordering/persistence incidents and introduced story QA command `qa:test:story-2-3`.
- Completed adversarial code review and patched execution-engine startup to enforce fail-closed behavior when either ingestion runtime halts.
- Extended user-stream QA coverage with runtime tests for machine-readable outcome evidence, status-only matched/cancellation mapping, and fail-closed non-UTC ingest timestamp rejection.
- Refreshed `_bmad-output/implementation-artifacts/tests/test-summary.md` for Story 2.3 and re-ran story-scoped QA plus Rust format/lint/build gates; story state remains `done`.

### File List

- .env.example
- _bmad-output/implementation-artifacts/stories/2-3-ingest-authenticated-user-stream-with-ordering-guarantees.md
- _bmad-output/implementation-artifacts/sprint-status.yaml
- _bmad-output/implementation-artifacts/tests/test-summary.md
- crates/domain/src/risk.rs
- crates/persistence/migrations/20260406030000_user_stream_ingestion.sql
- crates/persistence/src/postgres/mod.rs
- crates/persistence/src/postgres/user_stream.rs
- docs/operations/user-stream-ingestion.md
- package.json
- services/execution-engine/src/ingestion/mod.rs
- services/execution-engine/src/ingestion/user_stream.rs
- services/execution-engine/src/main.rs
- services/risk-engine/src/gates/mod.rs
- services/risk-engine/src/main.rs

### Change Log

- 2026-04-06: Created Story 2.3 context file and moved lifecycle state to `ready-for-dev`.
- 2026-04-06: Implemented authenticated user-stream ingestion contracts, persistence schema/adapters, runtime wiring, auth-expiry fail-safe gating, deterministic tests, and operations runbook; story moved to `review`.
- 2026-04-06: Executed adversarial code review workflow, auto-fixed fail-closed runtime orchestration in execution-engine bootstrap, and moved story to `done`.
- 2026-04-06: Executed BMAD QA automation workflow for Story 2.3, added user-stream runtime E2E tests for outcome evidence/status-mapping/UTC-boundary critical flows, and refreshed test summary evidence while retaining `done` status.
