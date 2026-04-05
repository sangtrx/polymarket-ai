# Story 1.5: Enforce Dual-Approval Governance for Critical Mutations

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As a governance approver,  
I want critical actions to require independent proposer and approver authorization,  
so that risky controls cannot be executed unilaterally.

## Acceptance Criteria

1. **Dual-approval gate for critical mutations (story-local BDD):**  
   **Given** a critical action request (`promotion override`, `risk-limit increase`, `kill-switch disable`, `production config change`)  
   **When** approvals are evaluated  
   **Then** execution is blocked unless `proposer != approver` and both approvals are present  
   **And** per-actor override rate limits are enforced per FR34 (`<= 5` requests per hour).
2. **UAC-1 Failure handling:** Missing approval requests, duplicate votes, self-approval attempts, expired approval windows (`now_utc >= expires_at_utc`), unauthorized role attempts, and rate-limit violations return explicit machine-readable errors with no privileged side effects.
3. **UAC-2 Boundary behavior:** Approval behavior is deterministic and test-covered for boundary conditions including the hourly rate-limit edge (`5` allowed, `6th` denied), proposer/approver identity comparison rules, expiry boundary rules (`now_utc == expires_at_utc` must deny), and pending/approved/rejected/expired state transitions.
4. **UAC-3 Verifiable evidence:** Successful and failed critical-mutation approval decisions emit timestamped telemetry/audit evidence including actor, action, outcome, reason code, correlation id, and approval reference linkage; denied FR34 attempts must include alert-compatible security signal fields for NFR8 operational alerting.
5. **Approval ledger integrity:** Story introduces only `approval_requests` and `approval_votes` schema scope with constraints enforcing one vote per actor per request, non-empty actor/action identifiers, explicit `expires_at_utc` per request, and queryable time-indexed approval history.
6. **Traceability and dependency constraints:** Story depends on 1.4, maps explicitly to FR34 + NFR8 + NFR17, and does not pull forward Story 1.6 credential-rotation behavior.
7. **Audit linkage contract:** For FR34-listed critical actions, immutable privileged audit append records include a non-null `approval_reference` once dual approval is satisfied; denied paths keep explicit reason codes without synthetic approval references.

## Tasks / Subtasks

- [x] **Task 1: Define canonical dual-approval governance domain contract** (AC: 1, 2, 3, 6, 7)
  - [x] Extend governance domain types for critical-action categories and approval state transitions (pending, approved, rejected, expired) using existing naming and serialization conventions.
  - [x] Canonicalize FR34 machine action identifiers as `strategy_promotion_override`, `risk_limit_increase`, `kill_switch_disable`, and `production_config_change`; use these exact values across domain, persistence, telemetry, and audit payloads.
  - [x] Define deterministic expiry semantics using persisted `expires_at_utc` per request with denial rule `now_utc >= expires_at_utc`.
  - [x] Define machine-readable reason codes for all denial classes (self-approval, missing second approver, rate-limited actor, invalid state transition, unknown request).
  - [x] Define deterministic proposer/approver distinctness rules and approval-reference generation format for downstream audit usage.
- [x] **Task 2: Add forward-only persistence migration for `approval_requests` and `approval_votes`** (AC: 1, 3, 5, 6)
  - [x] Create migration(s) under `crates/persistence/migrations/` introducing only `approval_requests` and `approval_votes` with explicit PK/FK/check constraints.
  - [x] Add `expires_at_utc` to `approval_requests` and enforce non-null UTC timestamp semantics for deterministic expiration checks.
  - [x] Add indexes supporting per-actor hourly rate-limit checks and pending-request lookup by action type/status/time.
  - [x] Enforce uniqueness boundary: one vote per `(request_id, actor_id)` and no blank actor/action/request identifiers.
- [x] **Task 3: Implement approval persistence adapters in `crates/persistence`** (AC: 1, 2, 3, 5)
  - [x] Add `sqlx` adapters for creating requests, recording votes, loading approval aggregates, and querying actor-rate usage within a rolling 1-hour window.
  - [x] Return explicit machine-readable persistence errors; do not swallow or coerce failure classes.
  - [x] Keep adapter APIs deterministic and scoped to Story 1.5 approval workflow needs only.
- [x] **Task 4: Implement governance-owned approval orchestration service** (AC: 1, 2, 3, 4, 6, 7)
  - [x] Replace `services/governance-service/src/approvals/mod.rs` placeholder with a dual-approval workflow service/port.
  - [x] Implement proposer submission, independent approver vote handling, and final readiness evaluation for FR34-listed actions.
  - [x] Emit structured approval telemetry with correlation id, actor id, decision code, action id, and timestamp for both allow and deny outcomes.
  - [x] Ensure denied FR34 approval decisions emit alert-compatible signal attributes consumed by existing security-alert pipelines (NFR8).
- [x] **Task 5: Integrate dual-approval enforcement into privileged control flow** (AC: 1, 2, 4, 7)
  - [x] Add/extend control-plane integration points so FR34-critical actions are blocked unless dual approval is already satisfied.
  - [x] Reuse existing authenticated actor + RBAC guard pipeline before approval checks; preserve current machine-readable error envelope conventions.
  - [x] Ensure audit append records for approved critical actions include `approval_reference`; denied attempts preserve explicit reason codes and correlation ids.
- [x] **Task 6: Add deterministic test coverage for approval, denial, and boundary paths** (AC: 2, 3, 4, 5, 6, 7)
  - [x] Add domain/service unit tests for self-approval denial, missing second approver denial, state-transition boundaries, and rate-limit edges (`5` allowed / `6th` denied).
  - [x] Add persistence tests for schema constraints, unique vote behavior, and rate-limit query determinism.
  - [x] Add control-api route/service integration tests validating machine-readable denial payloads and approved-path audit `approval_reference` propagation.
- [x] **Task 7: Update governance documentation and operational review guidance** (AC: 4, 6, 7)
  - [x] Update `docs/governance/rbac-role-model.md` with dual-approval request/vote lifecycle, proposer/approver separation policy, and operator review expectations.
  - [x] Document incident-review evidence expectations for approval decisions and linked immutable audit records.

### Review Findings

- [x] [Review][Patch] Serialize approval orchestration mutations to prevent concurrent submit/vote race windows in shared runtime service flow. [services/governance-service/src/approvals/mod.rs]
- [x] [Review][Patch] Recover stale pending approvals when a distinct approve vote exists, promoting to approved with deterministic `approval_reference` linkage. [services/governance-service/src/approvals/mod.rs]
- [x] [Review][Patch] Wire control-api runtime to durable Postgres-backed approval persistence and remove implicit volatile runtime approval state. [services/control-api/src/main.rs]
- [x] [Review][Patch] Canonicalize persistence actor identity checks for rate limiting and duplicate vote boundaries. [crates/persistence/src/postgres/approvals.rs]
- [x] [Review][Patch] Enforce approved-record `approval_reference` integrity and canonical vote uniqueness constraints at schema level. [crates/persistence/migrations/20260405074300_approval_requests_votes.sql]
- [x] [Review][Patch] Add machine-error envelope fields (`error_code`, `message`) to denied critical-action responses while preserving existing approval evidence fields. [services/control-api/src/routes/mod.rs]

## Dev Notes

### Technical Requirements

- Story dependency is strict: 1.5 builds on Story 1.4 immutable audit flow and must populate `approval_reference` for FR34-critical actions once approvals are satisfied.
- FR34 requirements are mandatory: distinct proposer/approver identity, required dual approval for listed critical actions, and per-actor override rate limit of `<= 5 requests/hour`.
- NFR8 and NFR17 require authenticated + RBAC-gated control paths with explicit auditability for both successful and denied privileged operations.
- Use canonical FR34 action identifiers end-to-end: `strategy_promotion_override`, `risk_limit_increase`, `kill_switch_disable`, `production_config_change`.
- Expiration is deterministic and request-local: each approval request stores `expires_at_utc`, and any evaluation where `now_utc >= expires_at_utc` is denied as expired.
- Denied FR34 paths must include alert-compatible security signal fields (`error.code`, `action`, `actor_id`, `correlation_id`, `timestamp_utc`) so existing <=30s NFR8 alert objectives remain enforceable.
- Scope is constrained to `approval_requests` and `approval_votes` schema entities for this story.
- Preserve current `/control/rebalance` path behavior unless/until it is explicitly mapped to one of the canonical FR34 critical action identifiers.
- **Out of scope:** credential rotation orchestration and secret lifecycle automation (Story 1.6), and Phase-2 model-governance automation features.

[Source: _bmad-output/planning-artifacts/epics.md#Story 1.5: Enforce Dual-Approval Governance for Critical Mutations]  
[Source: _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)]  
[Source: _bmad-output/planning-artifacts/prd.md#Governance, Security & Audit]  
[Source: _bmad-output/planning-artifacts/prd.md#Non-Functional Requirements]  
[Source: docs/governance/rbac-role-model.md#Story 1.4 immutable audit handoff expectations]

### Architecture Compliance

- Keep governance boundaries explicit: `governance-service` owns approval workflow orchestration and policy decisions.
- Keep `control-api` as a thin authenticated/RBAC-gated command boundary that delegates approval logic rather than embedding policy state mutation logic.
- Preserve canonical contracts: machine-readable error envelopes, RFC3339 UTC timestamps, deterministic decision codes, and explicit correlation metadata.
- Preserve safety-first behavior: critical mutation execution must fail closed when approval requirements are not met or approval state is ambiguous.

[Source: _bmad-output/planning-artifacts/architecture.md#Authentication & Security]  
[Source: _bmad-output/planning-artifacts/architecture.md#API & Communication Patterns]  
[Source: _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules]  
[Source: _bmad-output/planning-artifacts/architecture.md#Architectural Boundaries]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]

### Library & Framework Requirements

- Stay aligned with workspace-pinned architecture stack:
  - `axum = 0.8.8`
  - `sqlx = 0.8.6`
  - `tokio = 1.48.0`
  - `time = 0.3.44`
  - PostgreSQL 18 system-of-record baseline
- Latest checks: `axum 0.8.8` and `sqlx 0.8.6` match workspace; `tokio 1.51.0` and `time 0.3.47` are newer but this story should avoid opportunistic dependency churn.

[Source: Cargo.toml]  
[Source: _bmad-output/planning-artifacts/architecture.md#Data Architecture]  
[Source: _bmad-output/planning-artifacts/architecture.md#API & Communication Patterns]  
[Source: https://crates.io/api/v1/crates/axum]  
[Source: https://crates.io/api/v1/crates/sqlx]  
[Source: https://crates.io/api/v1/crates/tokio]  
[Source: https://crates.io/api/v1/crates/time]  
[Source: https://www.postgresql.org/docs/release/18.0/]

### File Structure Requirements

- Primary implementation surfaces for Story 1.5:
  - `services/governance-service/src/approvals/mod.rs`
  - `services/governance-service/src/lib.rs` (exports/wiring only as required)
  - `crates/domain/src/governance.rs`
  - `crates/persistence/migrations/*approval*.sql`
  - `crates/persistence/src/postgres/{mod.rs,approvals.rs}`
  - `services/control-api/src/{routes/mod.rs,middleware/mod.rs,main.rs}`
  - `docs/governance/rbac-role-model.md`
- Do not introduce unrelated top-level directories, alternate service boundaries, or non-story schema expansions.
- Preserve existing privileged-path baseline (`/control/rebalance` and authentication middleware) while adding explicit FR34 critical-action approval checks where required.

[Source: _bmad-output/planning-artifacts/architecture.md#Complete Project Directory Structure]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: _bmad-output/planning-artifacts/architecture.md#File Organization Patterns]  
[Source: services/governance-service/src/approvals/mod.rs]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: services/control-api/src/middleware/mod.rs]

### Testing Requirements

- Domain-level tests for approval state machine rules, distinct actor checks, and machine-readable denial reason codes.
- Persistence tests for migration constraints and deterministic rate-limit query behavior.
- Control-api/governance integration tests for:
  - Dual-approval-required denial behavior with explicit error code.
  - Self-approval denial (`proposer == approver`).
  - Rate-limit boundary behavior (`5` accepted, `6th` denied in rolling hour).
  - Expiration boundary behavior (`now_utc == expires_at_utc` denied, `now_utc < expires_at_utc` eligible).
  - Approved critical action emits audit evidence with populated `approval_reference`.
  - Denied critical-action payloads include alert-compatible signal fields required for NFR8 pipeline integration.
- Keep baseline CI gates compatible with repository scripts (`npm run ci:rust`, `npm run ci:security`, `npm test`).

[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: package.json]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: crates/persistence/src/postgres/rbac.rs]  
[Source: crates/persistence/src/postgres/audit.rs]

### Previous Story Intelligence

- Story 1.4 already established immutable `audit_log_append` and explicit nullable `approval_reference` contract for pre-1.5 flows; Story 1.5 should now provide the real approval reference for FR34 actions.
- Story 1.3/1.4 established deterministic authentication + authorization + audit sequencing around `POST /control/rebalance`; preserve this machine-readable and fail-closed behavior style.
- Story 1.2 established canonical RBAC role boundaries; dual-approval checks must layer on top of existing RBAC, not replace it.

[Source: _bmad-output/implementation-artifacts/stories/1-4-implement-immutable-privileged-audit-logging.md#Technical Requirements]  
[Source: _bmad-output/implementation-artifacts/stories/1-4-implement-immutable-privileged-audit-logging.md#Completion Notes List]  
[Source: _bmad-output/implementation-artifacts/stories/1-3-add-authenticated-control-middleware.md#Completion Notes List]  
[Source: _bmad-output/implementation-artifacts/stories/1-2-define-role-based-access-model.md#Completion Notes List]  
[Source: docs/governance/rbac-role-model.md#Story 1.4 immutable audit handoff expectations]

### Git Intelligence Summary

- Recent Epic 1 work patterns concentrate changes in:
  - `services/control-api` (middleware/routes/test-first privileged flow hardening)
  - `crates/domain` (canonical governance contracts and decision codes)
  - `crates/persistence` (forward-only migrations and explicit adapter errors)
  - `services/governance-service` (governance-owned service modules)
- Continue this pattern: deterministic contracts, explicit machine-readable errors, and tightly scoped migrations per story.

[Source: git --no-pager log --oneline -5]  
[Source: git --no-pager log -5 --name-only --pretty=format:'%h %s']

### Latest Technical Information

- Current stable checks confirm:
  - `axum` latest stable `0.8.8` (matches workspace)
  - `sqlx` latest stable `0.8.6` (matches workspace)
  - `tokio` latest stable `1.51.0` (workspace `1.48.0`)
  - `time` latest stable `0.3.47` (workspace `0.3.44`)
- For Story 1.5, maintain workspace consistency and avoid unrelated version upgrades unless explicitly approved in a separate dependency story.

[Source: Cargo.toml]  
[Source: https://crates.io/api/v1/crates/axum]  
[Source: https://crates.io/api/v1/crates/sqlx]  
[Source: https://crates.io/api/v1/crates/tokio]  
[Source: https://crates.io/api/v1/crates/time]

### Project Context Reference

- No `project-context.md` file was found in repository scope during discovery.
- Story context was derived from epics, PRD, architecture, UX specification, previous Epic 1 story artifacts, current codebase modules, and recent commit history.

### Project Structure Notes

- `services/governance-service/src/approvals/mod.rs` is currently a Story 1.5 placeholder and should become the core approval-orchestration seam.
- Existing privileged flow already emits authentication/authorization telemetry and immutable audit records; dual-approval enforcement should compose with this flow instead of replacing it.
- Keep schema and service boundaries narrow to prevent Story 1.6/Phase-2 scope bleed.

### References

- _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)  
- _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)  
- _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)  
- _bmad-output/planning-artifacts/epics.md#Epic 1: Secure Operator Access & Governance Control Plane  
- _bmad-output/planning-artifacts/epics.md#Story 1.5: Enforce Dual-Approval Governance for Critical Mutations  
- _bmad-output/planning-artifacts/prd.md#Governance, Security & Audit  
- _bmad-output/planning-artifacts/prd.md#Fraud Prevention & Detection  
- _bmad-output/planning-artifacts/prd.md#Non-Functional Requirements  
- _bmad-output/planning-artifacts/prd.md#Compliance & Audit Matrix  
- _bmad-output/planning-artifacts/architecture.md#Data Architecture  
- _bmad-output/planning-artifacts/architecture.md#Authentication & Security  
- _bmad-output/planning-artifacts/architecture.md#API & Communication Patterns  
- _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules  
- _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries  
- _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping  
- _bmad-output/planning-artifacts/ux-design-specification.md#Core User Experience  
- _bmad-output/planning-artifacts/ux-design-specification.md#Accessibility Strategy  
- _bmad-output/implementation-artifacts/stories/1-4-implement-immutable-privileged-audit-logging.md  
- _bmad-output/implementation-artifacts/stories/1-3-add-authenticated-control-middleware.md  
- _bmad-output/implementation-artifacts/stories/1-2-define-role-based-access-model.md  
- docs/governance/rbac-role-model.md  
- services/control-api/src/routes/mod.rs  
- services/control-api/src/middleware/mod.rs  
- services/governance-service/src/approvals/mod.rs  
- crates/domain/src/governance.rs  
- crates/persistence/src/postgres/{rbac.rs,audit.rs}  
- crates/persistence/migrations/20260405025656_rbac_roles_permissions.sql  
- crates/persistence/migrations/20260405035200_audit_log_append.sql  
- Cargo.toml  
- package.json  

## Story Completion Status

- Story implementation and adversarial review completed with automated high/medium patch remediation.
- Story status is set to `done`.
- Completion note: Dual-approval governance flow now includes durable runtime wiring, tightened persistence constraints, and race-safety hardening.

## Dev Agent Record

### Agent Model Used

GPT-5.3-Codex (gpt-5.3-codex)

### Debug Log References

- BMAD create-story workflow execution (automated)
- `git --no-pager log --oneline -5`
- `git --no-pager log -5 --name-only --pretty=format:'%h %s'`
- `curl -sL -H 'User-Agent: copilot-cli' https://crates.io/api/v1/crates/{axum,sqlx,tokio,time}`
- `cargo test -p domain governance::tests::`
- `cargo test -p persistence postgres::approvals::tests::`
- `cargo test -p governance-service approvals::tests::`
- `cargo test -p control-api routes::tests::critical_action_`
- `npm run qa:test:story-1-5`
- `npm run ci:rust`
- `npm run ci:security`
- `npm test`

### Completion Notes List

- Loaded and analyzed sprint-status, epics, PRD, architecture, UX, and prior Epic 1 stories to derive Story 1.5 implementation guardrails.
- Produced acceptance criteria and task breakdown with explicit FR34 + NFR8 + NFR17 traceability and schema-boundary constraints.
- Captured previous-story handoff requirements so dual-approval logic integrates with existing auth/RBAC/audit flows without regression.
- Added canonical FR34 critical-action contracts, approval states/reason codes, expiry boundary helpers, and deterministic approval-reference generation in the governance domain.
- Added forward-only `approval_requests` + `approval_votes` migration and `sqlx` approval persistence adapters with machine-readable errors and deterministic validation coverage.
- Implemented governance-owned dual-approval orchestration service with proposer submission, independent voting, state transitions, rate-limit enforcement, and structured telemetry.
- Added authenticated control-api dual-approval routes for request/vote/execute and fail-closed enforcement that blocks critical actions until approval is satisfied.
- Ensured approved critical execution emits immutable audit records with non-null `approval_reference` and denied paths preserve explicit reason codes without synthetic references.
- Updated governance RBAC documentation with Story 1.5 lifecycle and incident-review evidence/security-signal expectations.
- Hardened approval orchestration against race windows with serialized mutation handling and deterministic pending-state recovery for distinct-approver vote evidence.
- Wired control-api runtime to Postgres-backed approval persistence and tightened schema-level integrity constraints for canonical vote uniqueness and approved-reference enforcement.
- Cross-checked Dev Agent Record file list against git working tree source changes; no discrepancies remained after review-fix updates.
- Added Story 1.5 QA automation coverage for expired-window submission denial, unknown-request vote denial, and missing-request execution denial with explicit machine-readable payload assertions.
- Ran the dedicated Story 1.5 QA suite plus full Rust CI checks to confirm dual-approval governance and control-plane integrations remain green.

### File List

- _bmad-output/implementation-artifacts/stories/1-5-enforce-dual-approval-governance-for-critical-mutations.md
- _bmad-output/implementation-artifacts/sprint-status.yaml
- _bmad-output/implementation-artifacts/tests/test-summary.md
- Cargo.lock
- crates/domain/src/governance.rs
- crates/persistence/migrations/20260405074300_approval_requests_votes.sql
- crates/persistence/src/postgres/mod.rs
- crates/persistence/src/postgres/approvals.rs
- services/governance-service/Cargo.toml
- services/governance-service/src/approvals/mod.rs
- services/control-api/Cargo.toml
- services/control-api/src/main.rs
- services/control-api/src/middleware/mod.rs
- services/control-api/src/routes/mod.rs
- package.json
- docs/governance/rbac-role-model.md
- .scripts/bmad-auto/copilot/bmad-progress.log

### Change Log

- 2026-04-05: Created Story 1.5 context file and advanced sprint tracking status to `ready-for-dev`.
- 2026-04-05: Implemented Story 1.5 dual-approval governance contracts, persistence schema/adapters, governance orchestration, control-api enforcement routes, and FR34/NFR8 evidence documentation; story advanced to `review`.
- 2026-04-05: Completed adversarial code review triage, auto-fixed all identified high/medium implementation issues, and advanced story status to `done`.
- 2026-04-05: Generated and executed Story 1.5 QA automation tests for dual-approval critical flows, added missing fail-closed API coverage, and refreshed implementation test summary output.
