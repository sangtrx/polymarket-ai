# Story 2.9: Add Emergency Controls and Automatic Safe-State Triggers

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an operator,  
I want immediate emergency actions and automatic protection behavior,  
so that capital is protected during volatility or system uncertainty.

## Acceptance Criteria

1. **Manual emergency command (story-local BDD):**  
   **Given** operator invokes pause, reduce-only, or cancel-all  
   **When** command is accepted  
   **Then** control-plane acknowledgment occurs within 1 second and state reflects within 5 seconds.
2. **Automatic safe-state trigger (story-local BDD):**  
   **Given** critical safety trigger fires (stale feed, reconciliation-critical fault, or control uncertainty)  
   **When** automation evaluates trigger  
   **Then** system transitions to safe-state within 5 seconds without operator intervention.
3. **Post-action verification evidence (story-local BDD):**  
   **Given** a manual or automated safety action completed  
   **When** operator queries action result  
   **Then** response includes timestamp, actor/source, resulting mode, and correlated audit reference.
4. **UAC-1 Failure handling:** Invalid command payloads, unauthorized control attempts, and unavailable orchestration dependencies must return explicit machine-readable errors with no unsafe side effects or fail-open control behavior.
5. **UAC-2 Boundary behavior:** Emergency-control latency boundaries are deterministic and test-covered: acknowledgment `<= 1s`, state reflection `<= 5s`, stale-feed threshold `> 30s`, and drawdown/critical-halt boundaries remain inclusive and fail-closed.
6. **UAC-3 Verifiable evidence:** Successful and failed manual/automatic safety-control operations emit timestamped telemetry and immutable audit evidence with stable reason codes and correlation metadata.
7. **Schema/dependency/traceability contract:** Story depends only on `2.6` and `2.8`, creates only `safety_control_actions`, and maps explicitly to `FR20`, `NFR2`, and `NFR5`.
8. **Safe-state containment contract (NFR5/NFR12):** On stale-feed, reconciliation-critical, or control-uncertainty triggers, new-order submission rate drops to zero within 5 seconds and remains blocked until recovery gates explicitly pass.
9. **Execution containment contract:** Cancel-all and reduce-only behavior must reuse existing execution lifecycle semantics (cancel/batch-cancel and mode enforcement) with deterministic per-order outcomes and auditable reason codes.

## Tasks / Subtasks

- [x] **Task 1: Define canonical emergency-control domain contracts and reason taxonomy** (AC: 1, 2, 3, 4, 5, 6, 7, 8, 9)
  - [x] Extend `crates/domain/src/risk.rs` with typed emergency-control contracts (command/action enums, trigger-source enums, resulting-mode model, and machine-readable reason taxonomy) that can represent manual and automatic safety actions.
  - [x] Add validation helpers enforcing normalized identifiers, RFC3339 UTC timestamps, required actor/source semantics, and deterministic latency-bound evaluation fields.
  - [x] Reuse existing order-mode compatibility (`limit` vs `reduce_only`) and pre-trade reason-code normalization patterns; do not introduce parallel ad-hoc reason taxonomies.
  - [x] Add domain tests for parsing/validation, boundary-latency semantics, and required evidence-field presence.

- [x] **Task 2: Add forward-only persistence migration for Story 2.9 schema scope** (AC: 1, 2, 3, 5, 6, 7, 8)
  - [x] Add migration under `crates/persistence/migrations/` creating only `safety_control_actions`.
  - [x] Enforce constraints for canonical identifiers, non-empty machine reason codes, source/action enumerations, UTC timestamp fields, resulting-mode validity, and deterministic dedupe/idempotency behavior.
  - [x] Add indexes for action lookup (`action_id`), incident lookup (`correlation_id`, `reason_code`, `source` + time), and latest-state projection (`resulting_mode`, `effective_at_utc`).
  - [x] Ensure migration scope explicitly excludes unrelated tables/entities (`risk_limit_profiles`, `pretrade_gate_decisions`, and prior story schemas).

- [x] **Task 3: Implement PostgreSQL safety-control persistence adapter** (AC: 1, 2, 3, 4, 6, 7)
  - [x] Add `crates/persistence/src/postgres/safety_controls.rs` and wire module exports through `crates/persistence/src/postgres/mod.rs`.
  - [x] Implement durable write APIs for manual/automatic safety actions and deterministic read APIs for latest action by `action_id`/`correlation_id` and current effective safety mode.
  - [x] Return typed machine-readable persistence errors (constraint/query/decode/runtime), preserving fail-closed behavior.

- [x] **Task 4: Add governance-service safety-control orchestration** (AC: 1, 2, 3, 4, 5, 6, 8, 9)
  - [x] Add `services/governance-service/src/safety_controls/mod.rs` and export through `services/governance-service/src/lib.rs`.
  - [x] Define orchestration inputs/evidence and ports for manual command execution and automatic trigger handling, including role checks for manual controls and explicit source attribution for automation.
  - [x] Reuse approval/audit conventions from existing governance modules where applicable; do not bypass privileged-action telemetry/audit pathways.
  - [x] Persist action records with deterministic reason codes and returned evidence fields required by AC3.

- [x] **Task 5: Add control-api emergency-control endpoints and verification query surface** (AC: 1, 2, 3, 4, 6, 8)
  - [x] Extend `services/control-api/src/routes/mod.rs` route wiring with authenticated emergency-control endpoints (pause, reduce-only, cancel-all) plus action-result query endpoint.
  - [x] Reuse `authorize_critical_action` and existing response envelope patterns for accepted/denied/error/audit-failure outcomes; no new ad-hoc envelope format.
  - [x] Add request payload validation and explicit reason-code mapping for malformed or unauthorized control attempts.
  - [x] Ensure action-result response includes timestamp, actor/source, resulting mode, and correlated audit reference.

- [x] **Task 6: Extend risk-engine safe-state signaling and automatic trigger orchestration** (AC: 2, 4, 5, 6, 8)
  - [x] Extend `services/risk-engine/src/safe_state/mod.rs` to support generic emergency/safe-state action signaling while preserving drawdown-protective compatibility behavior from Story 2.8.
  - [x] In `services/risk-engine/src/gates/mod.rs`, map stale-feed, reconciliation-critical halt, and control-uncertainty conditions to automatic safe-state triggers with deterministic machine-readable reason codes.
  - [x] Wire runtime bootstrap/hydration seams in `services/risk-engine/src/main.rs` to consume effective safety-control mode without synthetic healthy defaults.
  - [x] Maintain fail-closed behavior: unknown/unavailable trigger-state dependencies must transition to safe-state, not allow trading.

- [x] **Task 7: Integrate execution-engine emergency containment behavior using existing lifecycle seams** (AC: 1, 2, 4, 6, 8, 9)
  - [x] Reuse `services/execution-engine/src/orders/mod.rs` existing cancel and batch-cancel pathways for cancel-all behavior rather than introducing a parallel order-cancel implementation.
  - [x] Enforce reduce-only/pause gating on submit path via existing pre-trade adjudication and mode-validation seams; denied paths must remain side-effect free.
  - [x] Ensure deterministic telemetry/evidence for bulk cancel outcomes and emergency-control deny behavior.

- [x] **Task 8: Add deterministic Story 2.9 coverage, QA command, and operations runbook** (AC: 1, 2, 3, 4, 5, 6, 8, 9)
  - [x] Add domain tests for emergency-control contract parsing/validation and latency-bound boundary behavior.
  - [x] Add persistence tests validating migration scope, constraints/indexes, and write/read determinism for `safety_control_actions`.
  - [x] Add governance-service tests for manual authorization boundaries, automatic-trigger transitions, and required action-evidence payload fields.
  - [x] Add control-api route tests for accepted/denied/error paths, response-envelope consistency, and audit metadata correctness.
  - [x] Add risk-engine tests for stale-feed/reconciliation/control-uncertainty automatic trigger paths and fail-closed mode behavior.
  - [x] Add execution-engine tests validating cancel-all reuse, reduce-only enforcement, and no-side-effect deny paths.
  - [x] Add `qa:test:story-2-9` in `package.json` and update `_bmad-output/implementation-artifacts/tests/test-summary.md` with Story 2.9 evidence.
  - [x] Add/update `docs/operations/*emergency*safe-state*` runbook guidance for operator workflows, incident triage, and recovery gate checks.

### Review Findings

- [x] [Review][Patch] Cancel-all orchestration now validates and deduplicates manual requests before containment side effects [services/governance-service/src/safety_controls/mod.rs]
- [x] [Review][Patch] Execution runtime no longer defaults to healthy mode when safety-mode evidence is missing [services/execution-engine/src/orders/mod.rs]
- [x] [Review][Patch] Risk-engine bootstrap now preserves existing auth-block state for normal/reduce-only safety modes and only hard-blocks on paused mode [services/risk-engine/src/main.rs]
- [x] [Review][Patch] Emergency-control API now maps safety-control persistence constraint/query/decode codes to deterministic conflict/service-unavailable responses [services/control-api/src/routes/mod.rs]

## Dev Notes

### Technical Requirements

- Story dependency and schema scope are strict:
  - Depends only on `2.6` and `2.8`.
  - Creates only `safety_control_actions`.
  - Traceability: `FR20`, `NFR2`, `NFR5` (plus containment behavior from `NFR12`).
- Manual emergency controls required in this story:
  - pause,
  - reduce-only,
  - cancel-all.
- Automatic trigger inputs required in this story:
  - stale feed,
  - reconciliation-critical fault,
  - control uncertainty (missing/unavailable critical control state).
- Deterministic behavior contracts:
  - command acknowledgment within `<= 1s`,
  - state reflection within `<= 5s`,
  - automatic trigger to safe-state within `<= 5s`,
  - no fail-open behavior on unavailable/invalid dependencies.
- Evidence contract (AC3/UAC-3):
  - timestamp (UTC),
  - actor/source,
  - resulting mode,
  - correlated audit reference,
  - stable reason code and correlation metadata.
- **Out of scope for Story 2.9:** full operator-console UX implementation (Epic 3 action rail), controlled-resume UX workflows, and non-emergency policy domains.

[Source: _bmad-output/planning-artifacts/epics.md#Story 2.9: Add Emergency Controls and Automatic Safe-State Triggers]  
[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: _bmad-output/planning-artifacts/prd.md#Functional Requirements]  
[Source: _bmad-output/planning-artifacts/prd.md#Non-Functional Requirements]  
[Source: _bmad-output/planning-artifacts/ux-design-specification.md#Journey 2 — Incident Safe-State Workflow]

### Architecture Compliance

- Preserve architecture ownership boundaries:
  - `control-api` is the control-command ingress.
  - `risk-engine` owns trading-eligibility and automatic safe-state trigger evaluation.
  - `execution-engine` consumes risk/control outcomes and must not mutate policy directly.
  - `governance-service` owns privileged workflow orchestration and audit linkage.
- Keep consistency rules enforced:
  - canonical machine-readable error envelopes,
  - ISO-8601 UTC timestamps,
  - deterministic reason-code outputs,
  - no silent fallbacks or swallowed errors in safety-critical paths.
- Maintain integration flow:
  1. Control command or trigger signal enters governance/risk surfaces.
  2. Safe-state decision and execution containment are applied.
  3. Audit/telemetry evidence persists for incident forensics.

[Source: _bmad-output/planning-artifacts/architecture.md#API & Communication Patterns]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: _bmad-output/planning-artifacts/architecture.md#Integration Points]  
[Source: _bmad-output/planning-artifacts/architecture.md#Pattern Examples]  
[Source: _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)]

### Library & Framework Requirements

- Keep workspace-pinned stack for Story 2.9 compatibility:
  - `polymarket-client-sdk = 0.4.4` (`clob`, `ws`)
  - `tokio = 1.48.0`
  - `sqlx = 0.8.6`
  - `axum = 0.8.8`
  - `time = 0.3.44`
- Latest-version checks at story creation time:
  - `polymarket-client-sdk`: latest stable `0.4.4`
  - `tokio`: latest stable `1.50.0`
  - `sqlx`: latest stable `0.8.6`
  - `axum`: latest stable `0.8.8`
  - `time`: latest stable `0.3.47`
- Do not perform opportunistic dependency upgrades in Story 2.9; preserve deterministic behavior and compatibility with existing Epic 2 implementations.

[Source: Cargo.toml]  
[Source: https://docs.rs/crate/polymarket-client-sdk/latest/source/Cargo.toml]  
[Source: https://docs.rs/crate/tokio/latest/source/Cargo.toml]  
[Source: https://docs.rs/crate/sqlx/latest/source/Cargo.toml]  
[Source: https://docs.rs/crate/axum/latest/source/Cargo.toml]  
[Source: https://docs.rs/crate/time/latest/source/Cargo.toml]

### File Structure Requirements

- Primary implementation surfaces:
  - `crates/domain/src/risk.rs`
  - `crates/persistence/migrations/*safety_control_actions*.sql`
  - `crates/persistence/src/postgres/{mod.rs,safety_controls.rs}`
  - `services/governance-service/src/{lib.rs,safety_controls/mod.rs}`
  - `services/control-api/src/{main.rs,middleware/mod.rs,routes/mod.rs}`
  - `services/risk-engine/src/{gates/mod.rs,safe_state/mod.rs,main.rs}`
  - `services/execution-engine/src/{orders/mod.rs,main.rs}`
  - `package.json`
  - `_bmad-output/implementation-artifacts/tests/test-summary.md`
  - `docs/operations/*emergency*safe-state*`
- Reuse established Epic 2 layering sequence:
  1. domain contracts,
  2. scoped migration,
  3. persistence adapter,
  4. orchestration/runtime integration,
  5. deterministic tests + QA script + operations runbook.
- Preserve schema-per-story discipline: Story 2.9 must not introduce future-epic schemas.

[Source: _bmad-output/planning-artifacts/architecture.md#Complete Project Directory Structure]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: crates/persistence/src/postgres/mod.rs]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: services/risk-engine/src/gates/mod.rs]  
[Source: services/risk-engine/src/safe_state/mod.rs]  
[Source: services/execution-engine/src/orders/mod.rs]  
[Source: services/governance-service/src/lib.rs]

### Testing Requirements

- Add deterministic coverage for:
  - manual pause/reduce-only/cancel-all accepted-path latency and state-reflection contracts,
  - automatic trigger transitions for stale-feed, reconciliation-critical, and control-uncertainty conditions,
  - explicit deny/error behavior for invalid payloads, unauthorized actors, and unavailable dependencies,
  - post-action evidence payload completeness (`timestamp`, `actor/source`, `resulting_mode`, `audit_reference`),
  - fail-closed containment when orchestration, persistence, or runtime signaling is unavailable.
- Validate persistence guarantees:
  - migration creates only `safety_control_actions`,
  - constraints/indexes enforce deterministic queryability and schema scope,
  - write/read APIs preserve reason codes, mode transitions, and correlation metadata.
- Validate cross-service behavior seams:
  - risk-engine trigger mapping and safe-state signaling remain deterministic,
  - execution-engine reuses existing cancel/batch-cancel lifecycle logic for cancel-all,
  - control-api responses remain canonical and auditable for all outcome paths.
- Add story-scoped QA script and update Story 2.9 evidence summary.

[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: package.json]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: services/risk-engine/src/gates/mod.rs]  
[Source: services/execution-engine/src/orders/mod.rs]  
[Source: crates/persistence/src/postgres/pretrade_gate.rs]

### Previous Story Intelligence

- Story 2.8 already established deterministic pre-trade gate sequencing and fail-closed adjudication; Story 2.9 should extend these seams for emergency-state transitions rather than duplicating gate logic.
- Story 2.8 introduced drawdown protective-mode signaling in `services/risk-engine/src/safe_state/mod.rs`; Story 2.9 should generalize this mechanism for additional automatic trigger sources while preserving existing telemetry compatibility.
- Story 2.7 and control-api route work established reusable privileged authorization/audit/error-envelope patterns; emergency-control routes should follow the same structure.
- Execution order runtime already has deterministic `cancel_order` and `batch_cancel` outcomes with telemetry and idempotency support; cancel-all behavior should compose these paths rather than adding a parallel cancellation subsystem.
- Recent Epic 2 stories enforce strict migration-scope tests (including explicit exclusion checks for future tables); Story 2.9 should keep this discipline, now scoped to `safety_control_actions`.

[Source: _bmad-output/implementation-artifacts/stories/2-8-enforce-pre-trade-gate-evaluation-pipeline.md#Completion Notes List]  
[Source: _bmad-output/implementation-artifacts/stories/2-7-configure-portfolio-market-and-strategy-limit-policies.md#Project Structure Notes]  
[Source: services/risk-engine/src/safe_state/mod.rs]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: services/execution-engine/src/orders/mod.rs]  
[Source: crates/persistence/src/postgres/pretrade_gate.rs]  
[Source: crates/persistence/src/postgres/risk_limits.rs]

### Git Intelligence Summary

- Recent Epic 2 commit sequence remains consistent and should be preserved for Story 2.9:
  1. domain contracts/reason codes,  
  2. scoped forward-only migration,  
  3. persistence adapter wiring,  
  4. runtime/service integration,  
  5. deterministic tests + story QA command + operations runbook.
- Recent commits also reinforce strict schema scope, fail-closed defaults, and telemetry/audit evidence consistency; Story 2.9 should maintain those guardrails unchanged.

[Source: git --no-pager log --oneline -5]  
[Source: git --no-pager show --name-only --pretty=format:'%h %s' -5]

### Latest Technical Information

- Dependency checks confirm current architecture-selected versions remain suitable for Story 2.9.
- `tokio` and `time` have newer stable releases than workspace pins, but Story 2.9 should prioritize compatibility and deterministic safety behavior over opportunistic upgrades.
- No dependency upgrade is required to implement emergency-control scope.

[Source: Cargo.toml]  
[Source: https://docs.rs/crate/polymarket-client-sdk/latest/source/Cargo.toml]  
[Source: https://docs.rs/crate/tokio/latest/source/Cargo.toml]  
[Source: https://docs.rs/crate/sqlx/latest/source/Cargo.toml]  
[Source: https://docs.rs/crate/axum/latest/source/Cargo.toml]  
[Source: https://docs.rs/crate/time/latest/source/Cargo.toml]

### Project Context Reference

- No `project-context.md` file was found in repository scope during discovery.
- Context for this story was derived from epics, PRD, architecture, UX specification, readiness report, prior story files, git history, and current source surfaces.

### Project Structure Notes

- `services/control-api/src/routes/mod.rs` already exposes strong privileged route patterns (authorization + audit + machine-readable errors) but currently has no emergency-control route group.
- `services/risk-engine/src/safe_state/mod.rs` currently signals drawdown protective mode only; it lacks generalized emergency action signaling for stale-feed/reconciliation/control-uncertainty triggers.
- `services/risk-engine/src/gates/mod.rs` already computes required trigger conditions (freshness, reconciliation, state-unavailable) and is the correct automatic-trigger seam.
- `services/execution-engine/src/orders/mod.rs` already supports deterministic `cancel_order` and `batch_cancel`; cancel-all should reuse this runtime path.
- No `safety_control_actions` migration or persistence adapter exists yet.

[Source: services/control-api/src/routes/mod.rs]  
[Source: services/risk-engine/src/safe_state/mod.rs]  
[Source: services/risk-engine/src/gates/mod.rs]  
[Source: services/execution-engine/src/orders/mod.rs]  
[Source: crates/persistence/src/postgres/mod.rs]  
[Source: crates/persistence/migrations/20260406081500_pretrade_gate_decisions.sql]

### References

- _bmad-output/planning-artifacts/epics.md#Epic 2: Live Market Connectivity & Safe Core Execution  
- _bmad-output/planning-artifacts/epics.md#Story 2.9: Add Emergency Controls and Automatic Safe-State Triggers  
- _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)  
- _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)  
- _bmad-output/planning-artifacts/prd.md#Functional Requirements  
- _bmad-output/planning-artifacts/prd.md#Non-Functional Requirements  
- _bmad-output/planning-artifacts/architecture.md#API & Communication Patterns  
- _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping  
- _bmad-output/planning-artifacts/architecture.md#Integration Points  
- _bmad-output/planning-artifacts/ux-design-specification.md#Journey 2 — Incident Safe-State Workflow  
- _bmad-output/planning-artifacts/ux-design-specification.md#Critical Success Moments  
- _bmad-output/planning-artifacts/implementation-readiness-report-2026-04-05.md#✅ Remediation Outcomes  
- _bmad-output/implementation-artifacts/stories/2-7-configure-portfolio-market-and-strategy-limit-policies.md  
- _bmad-output/implementation-artifacts/stories/2-8-enforce-pre-trade-gate-evaluation-pipeline.md  
- Cargo.toml  
- crates/domain/src/governance.rs  
- crates/domain/src/order.rs  
- crates/domain/src/risk.rs  
- crates/persistence/src/postgres/mod.rs  
- crates/persistence/src/postgres/pretrade_gate.rs  
- services/control-api/src/main.rs  
- services/control-api/src/middleware/mod.rs  
- services/control-api/src/routes/mod.rs  
- services/execution-engine/src/orders/mod.rs  
- services/governance-service/src/lib.rs  
- services/governance-service/src/risk_limits/mod.rs  
- services/risk-engine/src/gates/mod.rs  
- services/risk-engine/src/main.rs  
- services/risk-engine/src/safe_state/mod.rs  
- https://docs.rs/crate/polymarket-client-sdk/latest/source/Cargo.toml  
- https://docs.rs/crate/tokio/latest/source/Cargo.toml  
- https://docs.rs/crate/sqlx/latest/source/Cargo.toml  
- https://docs.rs/crate/axum/latest/source/Cargo.toml  
- https://docs.rs/crate/time/latest/source/Cargo.toml

## Story Completion Status

- Story implementation completed across domain, persistence, governance-service, control-api, risk-engine, and execution-engine seams.
- Story-scoped QA command (`qa:test:story-2-9`) and evidence summary were added and executed successfully.
- Story lifecycle moved from `ready-for-dev` through `in-progress` to `review`.

## Dev Agent Record

### Agent Model Used

GPT-5.3-Codex (gpt-5.3-codex)

### Debug Log References

- BMAD dev-story workflow execution (automated)
- Sprint backlog discovery and lifecycle transitions from `_bmad-output/implementation-artifacts/sprint-status.yaml`
- Workspace Rust/full-suite validation (`cargo test --workspace --all-targets`, `cargo build --workspace --all-targets`)
- Story QA execution (`npm run --silent qa:test:story-2-9`)
- Bootstrap regression execution (`npm run bootstrap:test`)

### Completion Notes List

- Selected first backlog story: `2-9-add-emergency-controls-and-automatic-safe-state-triggers`.
- Implemented canonical emergency-control domain contracts, validation helpers, and deterministic boundary tests.
- Added scoped schema migration plus Postgres safety-control adapter for durable action evidence and current-mode projection.
- Implemented governance safety-control orchestration (manual + automatic flows), containment-port integration, and deterministic service tests.
- Added control-api emergency endpoints (`pause`, `reduce-only`, `cancel-all`) and action-result query surface with canonical authorization/audit/error envelopes.
- Extended risk-engine safe-state signaling and gate mapping for stale-feed, reconciliation-critical, and control-uncertainty automatic triggers.
- Added execution-engine emergency containment behavior: paused/reduce-only submit gating and cancel-all reuse through batch-cancel lifecycle seams.
- Added Story 2.9 QA script, updated automation summary, and published emergency safe-state operations runbook.
- Completed adversarial code-review remediation: removed fail-open safety-mode fallback, enforced cancel-all side-effect safety/idempotency, preserved auth-block semantics under reduce-only/normal bootstrap hydration, and corrected emergency-control status-code mapping.
- Cross-checked story file list against git working-tree reality; `.scripts/bmad-auto/copilot/bmad-progress.log` is changed but intentionally excluded as non-application workflow telemetry.
- Added Story 2.9 QA regression coverage for emergency action-query not-found errors and paused-mode deny behavior for reduce-only submit attempts; reran `qa:test:story-2-9` successfully.

### File List

- _bmad-output/implementation-artifacts/stories/2-9-add-emergency-controls-and-automatic-safe-state-triggers.md
- _bmad-output/implementation-artifacts/sprint-status.yaml
- _bmad-output/implementation-artifacts/tests/test-summary.md
- package.json
- docs/operations/emergency-safe-state-controls.md
- crates/domain/src/risk.rs
- crates/persistence/migrations/20260406100000_safety_control_actions.sql
- crates/persistence/src/postgres/mod.rs
- crates/persistence/src/postgres/safety_controls.rs
- services/governance-service/src/lib.rs
- services/governance-service/src/safety_controls/mod.rs
- services/control-api/src/main.rs
- services/control-api/src/middleware/mod.rs
- services/control-api/src/routes/mod.rs
- services/risk-engine/src/safe_state/mod.rs
- services/risk-engine/src/gates/mod.rs
- services/risk-engine/src/main.rs
- services/execution-engine/src/orders/mod.rs
- services/execution-engine/src/main.rs

### Change Log

- 2026-04-06: Created Story 2.9 context file and moved lifecycle state from `backlog` to `ready-for-dev`.
- 2026-04-06: Implemented Story 2.9 emergency controls, automatic safe-state triggers, execution containment integration, and Story 2.9 QA/runbook artifacts.
- 2026-04-06: Moved story lifecycle state to `review`.
- 2026-04-06: Completed adversarial review auto-fixes and moved story lifecycle state to `done`.
- 2026-04-06: Extended Story 2.9 QA automation with additional API and execution containment regression tests and refreshed test-summary evidence.
