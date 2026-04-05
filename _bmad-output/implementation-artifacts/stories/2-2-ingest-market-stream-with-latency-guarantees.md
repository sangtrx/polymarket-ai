# Story 2.2: Ingest Market Stream with Latency Guarantees

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an execution service,  
I want to ingest market data events with strict latency targets,  
so that pricing and quoting use current venue state.

## Acceptance Criteria

1. **Normal ingestion latency (story-local BDD):**  
   **Given** the market stream is healthy  
   **When** events are processed under normal load  
   **Then** 99% of market updates are persisted within 2 seconds.
2. **Malformed payload handling (story-local BDD):**  
   **Given** a malformed market event is received  
   **When** ingestion validation fails  
   **Then** the event is quarantined and logged without crashing stream processing.
3. **Burst boundary condition (story-local BDD):**  
   **Given** ingest load reaches 2x baseline for 60 seconds  
   **When** processing completes  
   **Then** backlog remains ≤ 10 seconds or service enters degraded mode with alert.
4. **UAC-1 Failure handling:** Stream disconnects, heartbeat failures, malformed/unsupported event shapes, and persistence failures return explicit machine-readable reason codes and preserve fail-closed behavior (no silent acceptance of uncertain data).
5. **UAC-2 Boundary behavior:** Threshold boundaries are deterministic and test-covered, including `ingest_latency_seconds == 2.0`, backlog transition around `5s` and `10s`, and sustained backlog duration gate (`> 10s for > 30s`) that controls degraded-mode entry.
6. **UAC-3 Verifiable evidence:** Successful ingest, quarantined payloads, reconnect/degrade transitions, and persistence failures emit timestamped telemetry evidence with correlation metadata suitable for incident and QA traceability.
7. **Schema/dependency/traceability contract:** Story depends only on `2.1`, introduces only `market_ticks` and `market_stream_health`, and maps explicitly to `FR2`, `NFR3`, and `NFR5`.

## Tasks / Subtasks

- [x] **Task 1: Define canonical market-stream contracts and validation rules** (AC: 1, 2, 4, 5, 7)
  - [x] Add/extend domain contracts for stream-ingestion payloads to cover FR2 fields (`best_bid`, `best_ask`, top-5 depth, last trade, tick-size, market status) with explicit validation and machine-readable failure reasons.
  - [x] Add stream-health state contracts (healthy/degraded and reason metadata) that can be persisted and consumed by downstream risk/safe-state workflows.
  - [x] Reuse existing correlation/timestamp patterns (RFC3339 UTC strings, explicit `correlation_id`) and preserve snake_case naming conventions.
  - [x] Keep policy-evaluation surfaces from Story 2.1 compatible (market snapshot translation remains deterministic and fail-closed).

- [x] **Task 2: Add forward-only persistence migration for market ingest state** (AC: 1, 2, 3, 5, 7)
  - [x] Add a migration under `crates/persistence/migrations/` that creates only `market_ticks` and `market_stream_health`.
  - [x] Include schema constraints for non-negative numerical fields, non-empty IDs/reason codes, UTC timestamp fields, and deterministic status value checks.
  - [x] Add indexes for low-latency lookups by market/time and current stream-health evaluation paths.
  - [x] Ensure migration scope does not introduce Story 2.3+ entities (`user_stream_events`, `order_event_offsets`, `freshness_gate_events`).

- [x] **Task 3: Implement persistence adapters for market ticks and stream health** (AC: 1, 2, 3, 4, 6, 7)
  - [x] Add `crates/persistence/src/postgres/market_stream.rs` and wire it via `crates/persistence/src/postgres/mod.rs`.
  - [x] Implement durable write paths for accepted ticks, quarantined events, and health/degraded transition records.
  - [x] Return explicit machine-readable persistence errors and avoid silent retry loops that hide failure states.
  - [x] Keep data writes deterministic so malformed payload handling never causes partial/ambiguous persistence.

- [x] **Task 4: Implement execution-engine market stream ingestion pipeline** (AC: 1, 2, 3, 4, 6)
  - [x] Replace current ingestion placeholder flow with websocket-driven ingestion using official `polymarket-client-sdk` (`ws` capability).
  - [x] Parse incoming events into canonical domain contracts, validate, and persist normalized tick records.
  - [x] Measure per-event ingestion latency and enforce AC tracking for the `99% <= 2 seconds` target.
  - [x] Ensure malformed payloads are quarantined/logged and processing continues for subsequent valid events.

- [x] **Task 5: Implement backlog/heartbeat supervision and degraded-mode transitions** (AC: 3, 4, 5, 6)
  - [x] Add watchdog logic for backlog age, heartbeat continuity, and reconnect behavior in ingestion runtime.
  - [x] Persist stream-health transitions into `market_stream_health` with reason codes and correlation metadata.
  - [x] When backlog exceeds boundary policy (`>10s for >30s`), emit degraded-mode evidence with alert-compatible telemetry.
  - [x] Keep behavior aligned with safety-first architecture: uncertain stream health must not be treated as normal.

- [x] **Task 6: Wire runtime bootstrap/configuration for production-like ingestion** (AC: 1, 3, 4, 6)
  - [x] Extend `services/execution-engine/src/main.rs` to initialize DB-backed ingestion dependencies and supervised stream startup.
  - [x] Add explicit configuration surface for stream endpoint/session settings without embedding secrets in code.
  - [x] Preserve startup health-gate posture so runtime clearly reports readiness/degraded state.
  - [x] Keep execution-engine boundaries tight; do not pull in control-api/governance concerns for this story.

- [x] **Task 7: Add deterministic tests for latency, quarantine, and burst boundaries** (AC: 1, 2, 3, 4, 5, 6)
  - [x] Add domain unit tests for payload validation and reason-code determinism.
  - [x] Add persistence tests for migration scope, constraints, and explicit failure mapping.
  - [x] Add execution-ingestion tests that prove:
    - [x] normal-load path achieves the AC latency envelope in controlled test conditions,
    - [x] malformed payloads are quarantined without processor crash,
    - [x] burst boundary handling keeps backlog ≤10 seconds or enters degraded mode with alert evidence.
  - [x] Keep tests aligned with existing workspace quality gates and add story-scoped QA command only if it follows current `package.json` conventions.

- [x] **Task 8: Document operational stream-health semantics and remediation guidance** (AC: 3, 4, 6)
  - [x] Add/update operations documentation for stream health states, backlog thresholds, quarantine behavior, and alert conditions.
  - [x] Include operator/on-call remediation guidance for disconnect storms, malformed event spikes, and persistence outages.
  - [x] Ensure guidance is consistent with deployment/runbook principles already defined in PRD/architecture artifacts.

### Review Findings

- [x] [Review][Patch] Persistence-path failures did not emit explicit telemetry evidence, violating AC/UAC traceability guarantees [services/execution-engine/src/ingestion/mod.rs:369]
- [x] [Review][Patch] Websocket init/subscribe/disconnect failures terminated runtime immediately instead of attempting bounded reconnect behavior [services/execution-engine/src/ingestion/mod.rs:752]
- [x] [Review][Patch] Quarantine event identifiers could collide for repeated malformed events with the same venue timestamp [services/execution-engine/src/ingestion/mod.rs:628]

## Dev Notes

### Technical Requirements

- Story dependency is strict: `2.2` depends on `2.1` and has no forward dependencies.
- Story scope is explicit and narrow:
  - Functional scope: market stream ingestion only (not authenticated user stream or freshness gating).
  - Schema scope: `market_ticks`, `market_stream_health` only.
  - Traceability scope: `FR2`, `NFR3`, `NFR5`.
- FR2 event payload coverage must include best bid/ask, top-5 depth, last trade, tick-size, and market status.
- AC behavior must preserve three paths: normal-latency success, malformed-event quarantine, and burst/degraded boundary handling.
- Safety-first behavior is mandatory: stale/uncertain stream conditions require explicit degraded/fail-closed handling rather than silent continuation.
- **Out of scope for Story 2.2:** authenticated user stream ordering (Story 2.3), stale-feed pause gate orchestration (Story 2.4), and order lifecycle persistence (Story 2.5+).

[Source: _bmad-output/planning-artifacts/epics.md#Story 2.2: Ingest Market Stream with Latency Guarantees]  
[Source: _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)]  
[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: _bmad-output/planning-artifacts/prd.md#Market Universe & Data Intake]  
[Source: _bmad-output/planning-artifacts/prd.md#Performance]  
[Source: _bmad-output/planning-artifacts/prd.md#Reliability & Availability]

### Architecture Compliance

- Keep architecture boundaries explicit:
  - `execution-engine/src/ingestion` owns market-ingestion implementation for FR1–FR5 surfaces.
  - `risk-engine` remains owner of trading-eligibility/safe-state decisions; Story 2.2 should expose clear health evidence signals rather than bypassing risk ownership.
  - Shared contracts should remain in workspace crates (`domain`, `persistence`) instead of duplicating logic in service binaries.
- Use official Polymarket SDK integration path with required `ws` capability and heartbeat-safe operation.
- Preserve implementation patterns:
  - snake_case naming for Rust/files/DB,
  - machine-readable reason/error envelopes,
  - UTC timestamps and explicit correlation metadata,
  - no swallowed errors in control/execution-critical paths.
- Treat stale/uncertain stream state as a safety trigger, consistent with architecture handoff guidance.

[Source: _bmad-output/planning-artifacts/architecture.md#Technical Constraints & Dependencies]  
[Source: _bmad-output/planning-artifacts/architecture.md#API & Communication Patterns]  
[Source: _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: _bmad-output/planning-artifacts/architecture.md#Implementation Handoff]

### Library & Framework Requirements

- Continue workspace-pinned stack for compatibility with existing Epic 1/2 work:
  - `polymarket-client-sdk = 0.4.4` (`clob`, `ws`)
  - `tokio = 1.48.0`
  - `sqlx = 0.8.6`
  - `axum = 0.8.8` (indirect for control surfaces; avoid unnecessary API churn in this story)
  - `time = 0.3.44`
- Latest-version checks at story creation time:
  - `polymarket-client-sdk` latest stable: `0.4.4`
  - `tokio` latest stable: `1.51.0`
  - `sqlx` latest stable: `0.8.6` (`0.9.0-alpha.1` is pre-release)
  - `axum` latest stable: `0.8.8`
  - `time` latest stable: `0.3.47`
- Do not perform opportunistic dependency upgrades in Story 2.2; prioritize stability and consistency with current workspace lockstep.

[Source: Cargo.toml]  
[Source: services/execution-engine/Cargo.toml]  
[Source: https://crates.io/api/v1/crates/polymarket-client-sdk]  
[Source: https://crates.io/api/v1/crates/tokio]  
[Source: https://crates.io/api/v1/crates/sqlx]  
[Source: https://crates.io/api/v1/crates/axum]  
[Source: https://crates.io/api/v1/crates/time]

### File Structure Requirements

- Primary implementation surfaces:
  - `services/execution-engine/src/ingestion/*`
  - `services/execution-engine/src/main.rs`
  - `crates/domain/src/{risk.rs,events.rs,lib.rs}` (only where canonical contracts belong)
  - `crates/persistence/migrations/*market_stream*.sql`
  - `crates/persistence/src/postgres/{mod.rs,market_stream.rs}`
  - `docs/operations/*market-stream*` or equivalent runbook location
- Preserve Story 2 sequencing:
  - Keep `orders/` and `reconciliation/` scope for later stories unless directly required for ingestion health signaling.
  - Do not pre-implement user-stream ordering/freshness gate logic from Stories 2.3/2.4.
- Reuse established Story 2.1 patterns for canonicalization, error mapping, and deterministic telemetry payload structure.

[Source: _bmad-output/planning-artifacts/architecture.md#Complete Project Directory Structure]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: services/execution-engine/src/ingestion/mod.rs]  
[Source: services/execution-engine/src/main.rs]  
[Source: crates/persistence/src/postgres/market_policy.rs]  
[Source: crates/persistence/migrations/20260406004500_market_policy_profiles.sql]

### Testing Requirements

- Add deterministic coverage for:
  - normal ingestion latency path (`99% <= 2 seconds` under controlled normal-load tests),
  - malformed payload quarantine/logging without stream crash,
  - burst behavior at 2x baseline for 60 seconds with deterministic backlog/degraded outcome checks,
  - explicit failure handling for reconnect/heartbeat/persistence issues with machine-readable reason codes,
  - threshold boundary behavior (`2s`, `5s`, `10s`, and sustained `>30s` policy checks).
- Keep quality-gate compatibility with existing workflows:
  - targeted crate tests for domain/persistence/execution-engine,
  - workspace-level Rust tests via existing scripts,
  - optional story-scoped QA command following existing naming pattern (`qa:test:story-*`) when added.
- Ensure telemetry evidence assertions include timestamps, reason codes, and correlation IDs.

[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: package.json]  
[Source: _bmad-output/implementation-artifacts/stories/2-1-configure-market-universe-policy-engine.md#Testing Requirements]

### Previous Story Intelligence

- Story 2.1 established patterns that should continue in 2.2:
  - strict machine-readable error/validation payloads,
  - fail-closed behavior for missing/ambiguous runtime state,
  - canonicalized identifiers (trim + lowercase) for cluster-scoped behavior,
  - per-story schema discipline with forward-only migrations and explicit constraints,
  - telemetry events with correlation metadata and UTC timestamps.
- Existing execution/risk seam from Story 2.1 already evaluates eligibility/toggles; Story 2.2 should feed this seam with fresh, validated market data rather than replacing it.
- Story 2.1 adversarial fixes highlight high-risk areas to avoid repeating:
  - profile/cluster identity drift,
  - non-deterministic lookup ordering,
  - missing telemetry on deny/error paths.

[Source: _bmad-output/implementation-artifacts/stories/2-1-configure-market-universe-policy-engine.md#Previous Story Intelligence]  
[Source: _bmad-output/implementation-artifacts/stories/2-1-configure-market-universe-policy-engine.md#Review Findings]  
[Source: services/execution-engine/src/ingestion/mod.rs]  
[Source: services/risk-engine/src/gates/mod.rs]

### Git Intelligence Summary

- Recent commits show a repeatable delivery pattern:
  - domain contract changes first,
  - then narrow schema migration + persistence adapter,
  - then service orchestration/routes,
  - then targeted tests and QA scripts.
- For Story 2.2, apply the same order to reduce integration risk:
  1) domain + migration scope,  
  2) persistence adapter,  
  3) execution-engine ingestion wiring,  
  4) deterministic tests and story QA evidence.

[Source: git --no-pager log --oneline -5]  
[Source: git --no-pager log -5 --name-only --pretty=format:'%h %s']

### Latest Technical Information

- Latest crate checks confirm the architecture-selected stack remains valid:
  - `polymarket-client-sdk`: `0.4.4`
  - `tokio`: `1.51.0` latest stable (workspace pinned lower)
  - `sqlx`: `0.8.6` stable (`0.9.0-alpha.1` pre-release)
  - `axum`: `0.8.8`
  - `time`: `0.3.47` latest stable (workspace pinned lower)
- Story 2.2 should prioritize workspace compatibility and deterministic ingestion behavior over dependency churn.

[Source: Cargo.toml]  
[Source: https://crates.io/api/v1/crates/polymarket-client-sdk]  
[Source: https://crates.io/api/v1/crates/tokio]  
[Source: https://crates.io/api/v1/crates/sqlx]  
[Source: https://crates.io/api/v1/crates/axum]  
[Source: https://crates.io/api/v1/crates/time]

### Project Context Reference

- No `project-context.md` file was found in repository scope during discovery.
- Story context was derived from epics, PRD, architecture, UX specification, implementation-readiness report, previous story artifacts, git history, and current codebase surfaces.

### Project Structure Notes

- Current execution/risk/service state indicates Story 2 remains in staged foundation mode:
  - `services/execution-engine/src/ingestion/mod.rs` currently contains policy-evaluation scaffolding,
  - `orders` and `reconciliation` modules are placeholders,
  - risk limits/safe_state modules are placeholders for later Story 2 work.
- Story 2.2 should convert ingestion from placeholder evaluation-only behavior into durable stream intake while keeping boundaries clear for 2.3/2.4 onward.
- Preserve operational runbook quality from Story 2.1 by documenting stream health, degradation, and remediation expectations for on-call workflows.

[Source: services/execution-engine/src/main.rs]  
[Source: services/execution-engine/src/ingestion/mod.rs]  
[Source: services/execution-engine/src/orders/mod.rs]  
[Source: services/execution-engine/src/reconciliation/mod.rs]  
[Source: services/risk-engine/src/limits/mod.rs]  
[Source: services/risk-engine/src/safe_state/mod.rs]  
[Source: docs/operations/market-policy-engine.md]

### References

- _bmad-output/planning-artifacts/epics.md#Epic 2: Live Market Connectivity & Safe Core Execution  
- _bmad-output/planning-artifacts/epics.md#Story 2.2: Ingest Market Stream with Latency Guarantees  
- _bmad-output/planning-artifacts/epics.md#Story 2.3: Ingest Authenticated User Stream with Ordering Guarantees  
- _bmad-output/planning-artifacts/epics.md#Story 2.4: Enforce Data Freshness Gates and Stale-Feed Pausing  
- _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)  
- _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)  
- _bmad-output/planning-artifacts/prd.md#Market Universe & Data Intake  
- _bmad-output/planning-artifacts/prd.md#Performance  
- _bmad-output/planning-artifacts/prd.md#Reliability & Availability  
- _bmad-output/planning-artifacts/prd.md#Operational Controls  
- _bmad-output/planning-artifacts/architecture.md#Technical Constraints & Dependencies  
- _bmad-output/planning-artifacts/architecture.md#API & Communication Patterns  
- _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules  
- _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping  
- _bmad-output/planning-artifacts/architecture.md#Implementation Handoff  
- _bmad-output/planning-artifacts/ux-design-specification.md#Core User Experience  
- _bmad-output/planning-artifacts/implementation-readiness-report-2026-04-05.md  
- _bmad-output/implementation-artifacts/stories/2-1-configure-market-universe-policy-engine.md  
- crates/domain/src/risk.rs  
- crates/domain/src/events.rs  
- crates/persistence/src/postgres/market_policy.rs  
- services/execution-engine/src/ingestion/mod.rs  
- services/risk-engine/src/gates/mod.rs  
- docs/operations/market-policy-engine.md  
- Cargo.toml  
- package.json

## Story Completion Status

- Story context generated with exhaustive artifact analysis (workflow inputs, epic/PRD/architecture/UX, implementation-readiness report, previous story, git history, and current codebase surfaces).
- Story file is created and ready for implementation by dev agents.
- Completion note: Ultimate context engine analysis completed - comprehensive developer guide created.

## Dev Agent Record

### Agent Model Used

GPT-5.3-Codex (gpt-5.3-codex)

### Debug Log References

- BMAD create-story workflow execution (automated)
- Sprint backlog discovery from `_bmad-output/implementation-artifacts/sprint-status.yaml`
- Artifact discovery across planning/implementation/code surfaces
- Latest crate-version checks for ingestion-relevant dependencies
- Domain contract and validation implementation in `crates/domain/src/risk.rs`
- Forward-only migration + adapter implementation in `crates/persistence/migrations/20260406020000_market_stream_ingestion.sql` and `crates/persistence/src/postgres/market_stream.rs`
- Execution ingestion runtime implementation in `services/execution-engine/src/ingestion/mod.rs` and startup wiring in `services/execution-engine/src/main.rs`
- Story-scoped verification command `npm run qa:test:story-2-2`
- Full workspace quality gates: `npm run rust:fmt && npm run rust:lint && npm run rust:test && npm run rust:build`
- Adversarial code-review triage with automated patching for medium-severity ingestion-runtime gaps (persistence telemetry evidence, reconnect handling, quarantine-id collision hardening)
- BMAD QA automation workflow execution for Story 2.2 with additional ingestion critical-flow tests (heartbeat timeout, stream disconnect, persistence failure reason-code propagation)

### Completion Notes List

- Selected first backlog story: `2-2-ingest-market-stream-with-latency-guarantees`.
- Loaded and analyzed Epic 2 story requirements, PRD FR/NFR context, architecture constraints, UX guidance, implementation-readiness findings, previous story intelligence, and recent git implementation patterns.
- Produced implementation-ready tasks and guardrails aligned to schema discipline, architecture boundaries, and deterministic safety behavior.
- Implemented canonical market-stream tick, quarantine, and stream-health contracts with deterministic boundary validation and machine-readable reason-code handling.
- Added forward-only persistence migration for `market_ticks` and `market_stream_health`, including constraints/indexes and out-of-scope schema exclusions.
- Added durable persistence adapters for accepted ticks, quarantined payloads, and health transitions with explicit error envelopes.
- Replaced execution-engine placeholder ingestion with websocket-driven Polymarket CLOB orderbook ingestion, canonicalization, latency tracking, quarantine routing, and degraded-mode telemetry.
- Added deterministic domain/persistence/execution tests and introduced `qa:test:story-2-2` for story-scoped verification.
- Added operational runbook documentation for stream health semantics, alert criteria, and remediation guidance.
- Code-review reconciliation found one extra non-story automation artifact in git reality (`.scripts/bmad-auto/copilot/bmad-progress.log`); kept out of review scope and story file list.
- Auto-fixed medium-severity review findings by emitting persistence-failure telemetry evidence, adding bounded reconnect behavior, and hardening quarantine identifier uniqueness.
- Extended execution-engine ingestion QA coverage with new runtime tests for heartbeat-timeout degradation, disconnect evidence persistence, and machine-readable persistence-failure propagation.
- Refreshed `_bmad-output/implementation-artifacts/tests/test-summary.md` for Story 2.2 and re-ran story-scoped QA plus Rust format/lint/build gates; story state remains `done`.

### File List

- _bmad-output/implementation-artifacts/stories/2-2-ingest-market-stream-with-latency-guarantees.md
- _bmad-output/implementation-artifacts/sprint-status.yaml
- _bmad-output/implementation-artifacts/tests/test-summary.md
- Cargo.lock
- crates/domain/src/risk.rs
- crates/persistence/migrations/20260406020000_market_stream_ingestion.sql
- crates/persistence/src/postgres/mod.rs
- crates/persistence/src/postgres/market_stream.rs
- docs/operations/market-stream-ingestion.md
- package.json
- services/execution-engine/Cargo.toml
- services/execution-engine/src/ingestion/mod.rs
- services/execution-engine/src/main.rs

### Change Log

- 2026-04-06: Created Story 2.2 context file and moved lifecycle state to `ready-for-dev`.
- 2026-04-06: Implemented Story 2.2 market-stream ingestion domain contracts, persistence migration/adapters, execution runtime supervision, deterministic tests, and operations runbook; story moved to `review`.
- 2026-04-06: Completed adversarial code review, auto-fixed medium findings in ingestion runtime (persistence telemetry/reconnect/id-collision hardening), and moved story to `done`.
- 2026-04-06: Executed BMAD QA automation workflow for Story 2.2, added ingestion runtime E2E tests for heartbeat/disconnect/persistence-failure critical flows, and refreshed test summary evidence while retaining `done` status.
