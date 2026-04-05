# Story 2.6: Build Reconciliation and Exposure Visibility Core

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an operator,  
I want internal execution records reconciled against venue truth,  
so that exposure and order correctness are trustworthy.

## Acceptance Criteria

1. **Deterministic reconciliation run (story-local BDD):**  
   **Given** matching internal and venue windows  
   **When** reconciliation executes  
   **Then** mismatches are identified deterministically with diff classification.
2. **Critical mismatch failure path (story-local BDD):**  
   **Given** mismatch rate exceeds `0.1%`  
   **When** reconciliation completes  
   **Then** system transitions to safe-state and blocks new order placement.
3. **Halted-state visibility (story-local BDD):**  
   **Given** new order creation is paused  
   **When** operator opens exposure view  
   **Then** latest exposure snapshot remains queryable and timestamped.
4. **UAC-1 Failure handling:** Invalid reconciliation inputs/windows, unauthorized reconciliation/exposure read-path access, unavailable venue or persistence dependencies, and state-hydration failures must return explicit machine-readable reason codes with no unsafe side effects.
5. **UAC-2 Boundary behavior:** Mismatch-threshold boundaries are deterministic and test-covered (`<= 0.1%` does not trigger reconciliation halt; `> 0.1%` does trigger halt), with stable diff classification for identical inputs.
6. **UAC-3 Verifiable evidence:** Reconciliation success/failure and halt-state transitions emit timestamped, correlation-aware evidence suitable for incident and QA traceability.
7. **Schema/dependency/traceability contract:** Story depends only on `2.5`, creates only `reconciliation_runs`, `reconciliation_diffs`, and `exposure_snapshots`, and maps explicitly to `FR14`, `FR16`, and `NFR16`.
8. **NFR16 incident-query contract:** Reconciliation outcomes, diff records, and latest exposure snapshots are queryable via explicit read paths that avoid manual log stitching and support incident investigation workflows within `<= 5s` for representative incident-query paths.

## Tasks / Subtasks

- [x] **Task 1: Add canonical reconciliation and exposure domain contracts** (AC: 1, 2, 3, 4, 5, 6, 7, 8)
  - [x] Add reconciliation/exposure contracts in `crates/domain` (new `reconciliation` module or equivalent bounded domain surface) and export through `crates/domain/src/lib.rs`.
  - [x] Define deterministic diff-classification taxonomy and machine-readable reason-code set for reconciliation outcomes and safe-state triggers.
  - [x] Implement deterministic mismatch-rate calculation helpers with explicit threshold semantics for `0.1%`.
  - [x] Add domain tests for deterministic run outcomes, diff classification stability, and threshold boundary behavior.

- [x] **Task 2: Add forward-only persistence migration for Story 2.6 schema scope** (AC: 1, 2, 3, 5, 7, 8)
  - [x] Add migration under `crates/persistence/migrations/` creating only `reconciliation_runs`, `reconciliation_diffs`, and `exposure_snapshots`.
  - [x] Enforce constraints for non-empty identifiers, normalized reason/status fields, deterministic run identity, and UTC timestamp fields.
  - [x] Add indexes for run-window lookup, per-run diff retrieval, correlation-time incident queries, and latest exposure-snapshot lookup.
  - [x] Ensure migration scope explicitly excludes Story 2.7+ tables (`risk_limit_profiles`, `inventory_limit_rules`, `pretrade_gate_decisions`, `safety_control_actions`).

- [x] **Task 3: Implement PostgreSQL reconciliation persistence adapter** (AC: 1, 3, 4, 6, 7, 8)
  - [x] Add `crates/persistence/src/postgres/reconciliation.rs` and wire it through `crates/persistence/src/postgres/mod.rs`.
  - [x] Implement transactional writes for reconciliation run summary, associated diff rows, and exposure snapshot persistence.
  - [x] Implement explicit read paths for latest exposure snapshots and reconciliation run/diff evidence retrieval.
  - [x] Return typed persistence errors; do not swallow constraint, decode, or transaction failures.

- [x] **Task 4: Replace execution-engine reconciliation placeholder with runtime orchestration** (AC: 1, 4, 6, 7, 8)
  - [x] Replace `services/execution-engine/src/reconciliation/mod.rs` placeholder with runtime/controller seams for deterministic reconciliation execution.
  - [x] Use existing lifecycle truth from `orders` and `order_state_transitions` as internal execution source-of-truth input for reconciliation windows.
  - [x] Add venue-truth provider seam (SDK-backed or deterministic stubbed adapter) and classify mismatches deterministically against internal records.
  - [x] Emit structured reconciliation telemetry for run lifecycle, mismatch summary, and failure paths.

- [x] **Task 5: Enforce reconciliation-critical safe-state gating for new intents** (AC: 2, 3, 4, 5, 6, 8)
  - [x] Extend risk/runtime gate state surfaces to carry reconciliation-halt reason state alongside existing freshness/auth fail-closed state.
  - [x] On mismatch rate `> 0.1%`, persist reconciliation-critical outcome and enforce deny-path behavior for new order intents with machine-readable reason code.
  - [x] Ensure halted state does not hide exposure visibility read paths.
  - [x] Add deterministic tests proving mismatch-rate breach blocks new intents and boundary-safe runs do not trigger halt.

- [x] **Task 6: Implement exposure snapshot visibility read model surface** (AC: 3, 4, 6, 8)
  - [x] Add read-model query functions for latest exposure snapshot retrieval (global and market-scoped where applicable).
  - [x] Ensure snapshot payload includes timestamp and correlation metadata suitable for incident timeline composition.
  - [x] Enforce existing auth/RBAC policy on reconciliation/exposure read surfaces and return explicit machine-readable denial reason codes for unauthorized access.
  - [x] Keep read behavior available while reconciliation halt state is active.

- [x] **Task 7: Add deterministic Story 2.6 coverage and QA command** (AC: 1, 2, 3, 4, 5, 6, 8)
  - [x] Add domain tests for deterministic diff classification and mismatch-threshold boundary semantics.
  - [x] Add persistence tests validating migration scope, constraints/indexes, write/read correctness, and latest-snapshot retrieval behavior.
  - [x] Add execution-engine tests for deterministic reconciliation run, critical mismatch transition path, and halted-state snapshot visibility.
  - [x] Add risk gate tests for reconciliation-halt deny behavior in combination with existing freshness/auth gating.
  - [x] Add authorization tests proving unauthorized reconciliation/exposure read queries fail closed with explicit machine-readable denial reason codes and no side effects.
  - [x] Add incident-query path tests proving reconciliation outcomes/diffs/latest exposure snapshot reads satisfy `<= 5s` target behavior under representative fixtures.
  - [x] Add `qa:test:story-2-6` in `package.json` and update `_bmad-output/implementation-artifacts/tests/test-summary.md` with Story 2.6 evidence.

- [x] **Task 8: Add reconciliation/exposure operations runbook** (AC: 2, 3, 6, 8)
  - [x] Add/update `docs/operations/*reconciliation*` guidance for mismatch triage, halt-state handling, and exposure visibility verification.
  - [x] Document deterministic threshold policy (`> 0.1%` trigger), incident evidence queries, and safe-state recovery expectations without bypassing controls.
  - [x] Include explicit failure-mode playbooks for venue-unavailable, persistence-unavailable, and state-hydration errors.

### Review Findings

- [x] [Review][Patch] Incident evidence lookup could return a snapshot from a different run [services/execution-engine/src/reconciliation/mod.rs:509]
- [x] [Review][Patch] Incident evidence path lacked run-scoped snapshot query/index guarantees [crates/persistence/src/postgres/reconciliation.rs:369]

## Dev Notes

### Technical Requirements

- Story dependency is strict: `2.6` depends only on `2.5` (no forward dependencies).
- Story scope is explicit and narrow:
  - Functional scope: deterministic reconciliation + exposure visibility core.
  - Schema scope: `reconciliation_runs`, `reconciliation_diffs`, `exposure_snapshots` only.
  - Traceability scope: `FR14`, `FR16`, `NFR16`.
- Functional guardrails:
  - Reconciliation output must be deterministic for the same internal/venue input window.
  - Mismatch threshold is strict `> 0.1%` for critical halt; `<= 0.1%` is non-critical.
  - Critical mismatch state must block **new** order creation while preserving exposure visibility.
  - Exposure snapshots must remain queryable and timestamped even when halt is active.
  - Reconciliation/exposure read paths must enforce existing auth/RBAC policy and return explicit machine-readable denial reason codes for unauthorized access.
  - Reconciliation evidence query paths must satisfy NFR16 operability target (`<= 5s`) without manual log stitching.
- **Out of scope for Story 2.6:** risk-limit profile configuration (Story 2.7), full pre-trade gate orchestration (Story 2.8), and emergency-control UX/control workflows (Story 2.9 / Epic 3 controls).

[Source: _bmad-output/planning-artifacts/epics.md#Story 2.6: Build Reconciliation and Exposure Visibility Core]  
[Source: _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)]  
[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: _bmad-output/planning-artifacts/prd.md#Trade Execution & Order Management]  
[Source: _bmad-output/planning-artifacts/prd.md#Risk & Capital Management]  
[Source: _bmad-output/planning-artifacts/prd.md#Observability & Operability]

### Architecture Compliance

- Keep architecture boundaries explicit and consistent:
  - `services/execution-engine/src/reconciliation` owns reconciliation runtime orchestration.
  - `services/execution-engine/src/orders` remains canonical order-lifecycle state source for internal truth windows.
  - `services/risk-engine/src/gates` owns new-intent allow/deny adjudication, including reconciliation-critical halt deny behavior.
  - Shared contracts and persistence adapters stay in workspace crates (`domain`, `persistence`).
- Preserve architecture safety and process rules:
  - fail closed when reconciliation certainty degrades,
  - no swallowed errors in execution/risk-critical paths,
  - append-only evidence for reconciliation outcomes and halt transitions.
- Keep naming/envelope conventions consistent:
  - snake_case modules and DB identifiers,
  - UTC timestamps only,
  - machine-readable reason codes and explicit `correlation_id`.
- Reconciliation outcomes must remain compatible with incident/timeline-forensics workflows.

[Source: _bmad-output/planning-artifacts/architecture.md#Data Architecture]  
[Source: _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules]  
[Source: _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: _bmad-output/planning-artifacts/architecture.md#Integration Points]  
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
- Keep official integration patterns:
  - use documented Polymarket market/user websocket lifecycle semantics,
  - keep heartbeat/watchdog-safe behavior aligned with fail-closed controls.
- Do not introduce opportunistic dependency upgrades in Story 2.6; prioritize deterministic reconciliation correctness and workspace consistency.

[Source: Cargo.toml]  
[Source: https://crates.io/api/v1/crates/polymarket-client-sdk]  
[Source: https://crates.io/api/v1/crates/tokio]  
[Source: https://crates.io/api/v1/crates/sqlx]  
[Source: https://crates.io/api/v1/crates/axum]  
[Source: https://crates.io/api/v1/crates/time]  
[Source: _bmad-output/planning-artifacts/research/market-rust-polymarket-trading-system-lopez-de-prado-research-2026-04-04.md#Integration Patterns Analysis]

### File Structure Requirements

- Primary implementation surfaces:
  - `crates/domain/src/lib.rs`
  - `crates/domain/src/reconciliation.rs` (or equivalent bounded domain contract surface)
  - `crates/domain/src/risk.rs` (reconciliation-halt reason/gate integration as needed)
  - `crates/persistence/migrations/*reconciliation*.sql`
  - `crates/persistence/src/postgres/{mod.rs,reconciliation.rs}`
  - `services/execution-engine/src/reconciliation/mod.rs`
  - `services/execution-engine/src/main.rs`
  - `services/risk-engine/src/gates/mod.rs`
  - `services/risk-engine/src/main.rs`
  - `package.json`
  - `_bmad-output/implementation-artifacts/tests/test-summary.md`
  - `docs/operations/*reconciliation*`
- Keep Story 2 sequencing intact:
  - do not pre-implement Story 2.7/2.8 policy engines in this story,
  - do not expand emergency control workflows reserved for Story 2.9/Epic 3.
- Reuse established Story 2 patterns for reason-code taxonomy, fail-closed behavior, telemetry envelopes, and seam-based runtime integration.

[Source: _bmad-output/planning-artifacts/architecture.md#Complete Project Directory Structure]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: services/execution-engine/src/reconciliation/mod.rs]  
[Source: services/execution-engine/src/orders/mod.rs]  
[Source: services/risk-engine/src/gates/mod.rs]  
[Source: crates/persistence/src/postgres/orders.rs]

### Testing Requirements

- Add deterministic coverage for:
  - reconciliation deterministic run with stable diff classification when windows match,
  - critical mismatch path where `mismatch_rate > 0.1%` blocks new order intents,
  - boundary path where `mismatch_rate == 0.1%` remains non-critical,
  - exposure snapshot queryability while halt state is active,
  - unauthorized reconciliation/exposure read-path access returns explicit machine-readable denial reason codes with no side effects,
  - representative incident-query paths for reconciliation outcomes/diffs/latest exposure snapshots satisfy `<= 5s` target behavior,
  - fail-closed behavior for missing venue/internal windows and persistence unavailability.
- Keep quality-gate compatibility with existing repository workflows:
  - targeted crate/service tests (`domain`, `persistence`, `execution-engine`, `risk-engine`),
  - story-scoped QA command pattern in `package.json`,
  - workspace checks via existing scripts (`rust:lint`, `rust:test`, `rust:build`, `test`).
- Evidence assertions must validate reason codes, UTC timestamps, and correlation IDs across reconciliation outcomes and halt transitions.

[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: package.json]  
[Source: _bmad-output/implementation-artifacts/stories/2-4-enforce-data-freshness-gates-and-stale-feed-pausing.md#Testing Requirements]  
[Source: _bmad-output/implementation-artifacts/stories/2-5-implement-venue-compatible-order-lifecycle-handling.md#Testing Requirements]

### Previous Story Intelligence

- Story 2.5 established canonical lifecycle truth that Story 2.6 must reuse:
  - durable `orders` + `order_state_transitions` persistence,
  - deterministic transition semantics and idempotency contracts,
  - execution telemetry envelopes with correlation/timestamp conventions.
- Story 2.4 established fail-closed pause semantics and deterministic boundary handling that should inform reconciliation-critical halt behavior.
- Story 2.3 established authenticated user-stream ordering/idempotency guarantees that must remain source-consistent for reconciliation window inputs.
- Do not create parallel lifecycle truth stores; reconcile against existing canonical lifecycle surfaces and venue truth.

[Source: _bmad-output/implementation-artifacts/stories/2-5-implement-venue-compatible-order-lifecycle-handling.md#Completion Notes List]  
[Source: _bmad-output/implementation-artifacts/stories/2-5-implement-venue-compatible-order-lifecycle-handling.md#Project Structure Notes]  
[Source: _bmad-output/implementation-artifacts/stories/2-4-enforce-data-freshness-gates-and-stale-feed-pausing.md#Completion Notes List]  
[Source: _bmad-output/implementation-artifacts/stories/2-3-ingest-authenticated-user-stream-with-ordering-guarantees.md#Completion Notes List]  
[Source: services/execution-engine/src/orders/mod.rs]  
[Source: crates/persistence/src/postgres/orders.rs]

### Git Intelligence Summary

- Recent Epic 2 commit pattern is consistent and should be preserved for Story 2.6:
  1. domain contracts/reason codes,  
  2. scoped forward-only migration,  
  3. persistence adapter wiring,  
  4. runtime integration,  
  5. deterministic tests + story QA command + operations runbook.
- Current Story 2 implementation history confirms incremental layering on existing ingestion/order/freshness surfaces is preferred over broad refactors.

[Source: git --no-pager log --oneline -5]  
[Source: git --no-pager log --name-only --pretty=format:'COMMIT %h %s' -5]

### Latest Technical Information

- Crate compatibility checks confirm architecture-selected versions remain valid for Story 2.6:
  - `polymarket-client-sdk` latest stable `0.4.4` (matches workspace),
  - `tokio` latest stable `1.51.0` (workspace pinned lower),
  - `sqlx` latest stable `0.8.6` (`0.9.0-alpha.1` pre-release),
  - `axum` latest stable `0.8.8`,
  - `time` latest stable `0.3.47` (workspace pinned lower).
- Integration references remain aligned with story scope:
  - official market/user WS channels,
  - full lifecycle order/trade stream support,
  - heartbeat/watchdog reliability controls.
- No dependency upgrade is required to implement Story 2.6 safely.

[Source: https://crates.io/api/v1/crates/polymarket-client-sdk]  
[Source: https://crates.io/api/v1/crates/tokio]  
[Source: https://crates.io/api/v1/crates/sqlx]  
[Source: https://crates.io/api/v1/crates/axum]  
[Source: https://crates.io/api/v1/crates/time]  
[Source: _bmad-output/planning-artifacts/research/market-rust-polymarket-trading-system-lopez-de-prado-research-2026-04-04.md#Integration Patterns Analysis]  
[Source: _bmad-output/planning-artifacts/research/market-rust-polymarket-trading-system-lopez-de-prado-research-2026-04-04.md#Reliability architecture]

### Project Context Reference

- No `project-context.md` file was found in repository scope during discovery.
- Context for this story was derived from epics, PRD, architecture, UX specification, implementation-readiness report, research artifacts, previous stories, git history, and current codebase surfaces.

### Project Structure Notes

- `services/execution-engine/src/reconciliation/mod.rs` and `services/risk-engine/src/safe_state/mod.rs` are still explicit placeholders and are the intended Story 2.x extension seams for reconciliation/safe-state behavior.
- Existing Story 2 foundations already provide:
  - canonical lifecycle persistence and transition replay (`orders`, `order_state_transitions`),
  - authenticated user-stream ordering/idempotency guarantees,
  - deterministic freshness fail-closed pause semantics for new intents.
- Reconciliation implementation must extend these foundations rather than introducing duplicate lifecycle stores or divergent reason-code taxonomies.

[Source: services/execution-engine/src/reconciliation/mod.rs]  
[Source: services/risk-engine/src/safe_state/mod.rs]  
[Source: services/execution-engine/src/orders/mod.rs]  
[Source: services/execution-engine/src/ingestion/user_stream.rs]  
[Source: services/execution-engine/src/ingestion/freshness_gate.rs]  
[Source: crates/persistence/migrations/20260406050000_order_lifecycle.sql]

### References

- _bmad-output/planning-artifacts/epics.md#Epic 2: Live Market Connectivity & Safe Core Execution  
- _bmad-output/planning-artifacts/epics.md#Story 2.6: Build Reconciliation and Exposure Visibility Core  
- _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)  
- _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)  
- _bmad-output/planning-artifacts/prd.md#Trade Execution & Order Management  
- _bmad-output/planning-artifacts/prd.md#Risk & Capital Management  
- _bmad-output/planning-artifacts/prd.md#Operations Dashboard & Incident Handling  
- _bmad-output/planning-artifacts/prd.md#Observability & Operability  
- _bmad-output/planning-artifacts/prd.md#Trading Reactivation Readiness Checklist  
- _bmad-output/planning-artifacts/architecture.md#Data Architecture  
- _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules  
- _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries  
- _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping  
- _bmad-output/planning-artifacts/architecture.md#Integration Points  
- _bmad-output/planning-artifacts/architecture.md#Implementation Handoff  
- _bmad-output/planning-artifacts/ux-design-specification.md#Journey 2 — Incident Safe-State Workflow  
- _bmad-output/planning-artifacts/ux-design-specification.md#Causal Timeline Panel  
- _bmad-output/planning-artifacts/ux-design-specification.md#Flow Optimization Principles  
- _bmad-output/planning-artifacts/implementation-readiness-report-2026-04-05.md#✅ Remediation Outcomes  
- _bmad-output/planning-artifacts/research/market-rust-polymarket-trading-system-lopez-de-prado-research-2026-04-04.md#Integration Patterns Analysis  
- _bmad-output/planning-artifacts/research/market-rust-polymarket-trading-system-lopez-de-prado-research-2026-04-04.md#Reliability architecture  
- _bmad-output/implementation-artifacts/stories/2-3-ingest-authenticated-user-stream-with-ordering-guarantees.md  
- _bmad-output/implementation-artifacts/stories/2-4-enforce-data-freshness-gates-and-stale-feed-pausing.md  
- _bmad-output/implementation-artifacts/stories/2-5-implement-venue-compatible-order-lifecycle-handling.md  
- crates/domain/src/order.rs  
- crates/domain/src/risk.rs  
- crates/persistence/src/postgres/orders.rs  
- services/execution-engine/src/orders/mod.rs  
- services/execution-engine/src/reconciliation/mod.rs  
- services/risk-engine/src/gates/mod.rs

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
- Latest-version checks via crates.io API endpoints
- Implemented reconciliation/exposure domain contracts and deterministic mismatch threshold logic in `crates/domain`.
- Added Story 2.6 forward-only persistence migration and PostgreSQL adapter in `crates/persistence`.
- Replaced execution reconciliation placeholder with runtime/controller seams and explicit read paths in `services/execution-engine`.
- Extended risk gate runtime surfaces with reconciliation-halt deny semantics in `services/risk-engine`.
- Ran `npm run --silent qa:test:story-2-6` and workspace quality gates (`rust:lint`, `rust:test`, `rust:build`).
- Cross-checked story File List against `git status --porcelain`; excluded `.scripts/bmad-auto/copilot/bmad-progress.log` from review as non-application automation output.
- Adversarial code review triage auto-fixed cross-run incident-evidence snapshot drift with run-scoped lookup/query/index updates and regression tests.

### Completion Notes List

- Selected first backlog story: `2-6-build-reconciliation-and-exposure-visibility-core`.
- Captured story-local BDD acceptance criteria plus universal UAC contracts.
- Added implementation guardrails for strict schema scope, deterministic mismatch-threshold behavior, fail-closed safe-state transitions, and halted-state exposure visibility.
- Carried forward Story 2.3/2.4/2.5 patterns for reason-code taxonomy, telemetry evidence contracts, and seam-based runtime integration.
- Defined concrete implementation surfaces, testing requirements, and operations guidance for Story 2.6 execution.
- Implemented deterministic reconciliation run orchestration with stable diff classification taxonomy and strict threshold semantics (`> 0.1%` triggers halt).
- Added reconciliation evidence persistence/read contracts (`reconciliation_runs`, `reconciliation_diffs`, `exposure_snapshots`) with constrained schema scope and incident-query indexes.
- Added reconciliation/exposure read-model authz checks enforcing existing read-analytics RBAC with explicit machine reason codes on denial.
- Added execution and risk-gate coverage for critical mismatch safe-state deny behavior while preserving halted-state exposure visibility.
- Added Story 2.6 QA command and updated Story 2.6 test evidence and operations runbook.
- Auto-fixed incident-evidence snapshot correlation by scoping incident lookup to the requested reconciliation run and adding run-scoped snapshot index coverage.
- Re-ran Story 2.6 QA automation with `PATH="$HOME/.cargo/bin:$PATH" npm run --silent qa:test:story-2-6`; all story-scoped suites passed and status remained `done`.

### File List

- _bmad-output/implementation-artifacts/sprint-status.yaml
- _bmad-output/implementation-artifacts/stories/2-6-build-reconciliation-and-exposure-visibility-core.md
- _bmad-output/implementation-artifacts/tests/test-summary.md
- crates/domain/src/lib.rs
- crates/domain/src/reconciliation.rs
- crates/persistence/migrations/20260406061000_reconciliation_exposure_core.sql
- crates/persistence/src/postgres/mod.rs
- crates/persistence/src/postgres/reconciliation.rs
- docs/operations/reconciliation-exposure-core.md
- package.json
- services/execution-engine/src/main.rs
- services/execution-engine/src/reconciliation/mod.rs
- services/risk-engine/src/gates/mod.rs
- services/risk-engine/src/main.rs

### Change Log

- 2026-04-06: Created Story 2.6 context file and moved lifecycle state from `backlog` to `ready-for-dev`.
- 2026-04-06: Validate-story gate remediation added explicit unauthorized-access failure handling and NFR16 `<= 5s` incident-query requirements; story remains `ready-for-dev`.
- 2026-04-06: Implemented Story 2.6 reconciliation/exposure core across domain, persistence, execution runtime, risk gating, QA automation, and operations runbook; story moved to `review`.
- 2026-04-06: Adversarial review auto-fixed incident-evidence snapshot correlation drift (run-scoped lookup + index + regression coverage) and advanced story to `done`.
- 2026-04-06: QA automation rerun passed for `qa:test:story-2-6`; lifecycle status confirmed as `done`.
