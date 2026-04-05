# Story 1.6: Add Scheduled and Emergency Credential Rotation Flows

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an admin,  
I want secrets and credentials rotated on policy cadence and emergency triggers,  
so that credential compromise risk is minimized without governance downtime.

## Acceptance Criteria

1. **Scheduled + emergency rotation flow (story-local BDD):**  
   **Given** scheduled cadence or emergency compromise triggers are raised  
   **When** credential rotation executes  
   **Then** new credentials are activated without bypassing authenticated RBAC/governance controls  
   **And** rotation evidence is recorded for FR33 compliance.
2. **UAC-1 Failure handling:** Invalid trigger payloads, unauthorized role attempts, missing credential metadata, and secret-provider/runtime dependency failures return explicit machine-readable errors with no unsafe side effects (no partial cutover, no implicit privilege escalation).
3. **UAC-2 Boundary behavior:** Rotation boundary behavior is deterministic and test-covered for cadence and emergency thresholds: scheduled cadence due at `>= 90 days`, not-due at `< 90 days`, emergency completion target `<= 30 minutes` from compromise trigger, and fail-closed behavior when readiness constraints are ambiguous.
4. **UAC-3 Verifiable evidence:** Successful and failed credential-rotation attempts emit timestamped telemetry/audit evidence including actor, trigger type, credential scope, outcome, reason code, correlation id, and rotation reference.
5. **Schema/dependency/traceability contract:** Story depends on 1.5, introduces only `credential_rotation_events` schema scope, and maps explicitly to FR33 + NFR7 + NFR21.
6. **Governance continuity contract:** Existing privileged control and critical-approval workflows remain operational and policy-enforced during and immediately after rotation events.

## Tasks / Subtasks

- [x] **Task 1: Define canonical credential-rotation domain contract** (AC: 1, 2, 3, 4, 5)
  - [x] Extend `crates/domain/src/governance.rs` with explicit credential-rotation types (trigger, lifecycle state, decision/reason codes, rotation evidence contract) using existing enum/serialization conventions.
  - [x] Add deterministic helpers for scheduled due-window checks (`>= 90 days`) and emergency deadline checks (`<= 30 minutes`) with RFC3339 UTC validation.
  - [x] Enforce strict non-secret payload rules in domain contracts (store only references/metadata, never plaintext secret values).
- [x] **Task 2: Add forward-only persistence migration for `credential_rotation_events`** (AC: 1, 2, 4, 5)
  - [x] Add migration under `crates/persistence/migrations/` introducing only `credential_rotation_events` with PK/check constraints aligned to rotation trigger/state/reason code contracts.
  - [x] Add indexes for auditability and operations (`trigger_type`, `status`, `initiated_at_utc`, `actor_id`, `correlation_id`).
  - [x] Add constraints ensuring UTC timestamps, non-empty identifiers, and non-persistence of secret material.
- [x] **Task 3: Implement credential-rotation persistence adapters** (AC: 1, 2, 3, 4, 5)
  - [x] Add `crates/persistence/src/postgres/credential_rotation.rs` and wire `crates/persistence/src/postgres/mod.rs`.
  - [x] Implement create/update/load/query pathways with explicit machine-readable persistence errors and deterministic timestamp parsing.
  - [x] Add repository-level validation for idempotent rotation reference handling and status transition integrity.
- [x] **Task 4: Implement governance-owned rotation orchestration service** (AC: 1, 2, 3, 4, 6)
  - [x] Add `services/governance-service/src/credentials/mod.rs` to orchestrate scheduled and emergency rotations through a provider-agnostic secret-rotation port.
  - [x] Preserve fail-closed semantics: if secret backends or validation checks fail, rotation is denied and current credentials remain active.
  - [x] Emit structured rotation telemetry/security signals with actor/trigger/outcome/reason/correlation/timestamp fields.
  - [x] Ensure orchestration does not bypass or disable existing auth/RBAC/approval controls while rotation executes.
- [x] **Task 5: Integrate control-api rotation endpoints and policy enforcement** (AC: 1, 2, 4, 6)
  - [x] Add authenticated control-plane routes for scheduled and emergency rotation triggers in `services/control-api/src/routes/mod.rs`.
  - [x] Reuse `require_authenticated_actor` + authorization guard pipeline and preserve canonical error envelope conventions.
  - [x] Ensure route responses include deterministic machine-readable fields for allow/deny/pending-like operational states as applicable.
- [x] **Task 6: Update governance and operations documentation** (AC: 1, 4, 5, 6)
  - [x] Update `docs/governance/rbac-role-model.md` with Story 1.6 credential-rotation lifecycle, reason codes, and governance continuity expectations.
  - [x] Add/update runbook documentation in `docs/runbooks/` for scheduled rotation operations, emergency compromise response, and rollback/verification checklist.
  - [x] Document operator-safe handling rules: no secret material in source/logs/CLI args and runtime injection-only posture.
- [x] **Task 7: Add deterministic test coverage for rotation workflows and boundaries** (AC: 2, 3, 4, 5, 6)
  - [x] Add domain tests for cadence and emergency boundary logic (`89d` vs `90d`, emergency-window expiry, invalid timestamp/path failures).
  - [x] Add persistence tests for schema constraints, transition correctness, and query determinism in `credential_rotation_events`.
  - [x] Add governance-service tests for provider failures, emergency trigger handling, and evidence output correctness.
  - [x] Add control-api integration tests for unauthorized/malformed requests, successful rotation trigger flow, and machine-readable denial contracts.
  - [x] Add/maintain story-level QA command coverage (e.g., `qa:test:story-1-6`) consistent with repository script patterns.

### Review Findings

- [x] [Review][Patch] Preserve credential-rotation domain error codes when validating metadata contracts (`credential_rotation_secret_material_rejected`, `credential_rotation_missing_metadata`) [services/governance-service/src/credentials/mod.rs]
- [x] [Review][Patch] Enforce required metadata contract fields (`crypto_posture_verified`, `runtime_injection_mode`, `provider_ref`) with explicit machine-readable failures [services/governance-service/src/credentials/mod.rs]
- [x] [Review][Patch] Add control-api route tests for missing metadata and runtime dependency failure machine-readable responses [services/control-api/src/routes/mod.rs]
- [x] [Review][Defer] Non-story workspace drift detected in `.gitignore`, `_bmad/**`, and `.scripts/**`; excluded from application-source review per workflow scope — deferred, pre-existing

## Dev Notes

### Technical Requirements

- Story dependency is strict: 1.6 builds on Story 1.5 governance baseline and must keep authenticated RBAC + dual-approval control plane intact while rotation occurs.
- Traceability contract is fixed: Dependencies `1.5`, schema scope `credential_rotation_events`, mapping `FR33 + NFR7 + NFR21`.
- FR33 requires both scheduled (`90-day`) and emergency (`<= 30-minute`) rotation pathways without governance downtime.
- NFR7 and existing security constraints require no plaintext secrets in source, logs, process args, telemetry payloads, or persisted records; only references/metadata are allowed.
- NFR7 cryptographic controls are mandatory for rotated credentials: maintain at least 128-bit security strength at rest and in transit, and fail closed when provider posture cannot be verified.
- NFR21 requires rotatable production secrets without full shutdown and explicit emergency rotation capability.
- **Out of scope:** Story 2.x market ingestion/execution work, reporting contracts (FR35+), and Phase-2 model-governance automation.

[Source: _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)]  
[Source: _bmad-output/planning-artifacts/epics.md#Story 1.6: Add Scheduled and Emergency Credential Rotation Flows]  
[Source: _bmad-output/planning-artifacts/prd.md#Governance, Security & Audit]  
[Source: _bmad-output/planning-artifacts/prd.md#Security Architecture & Threat Model]  
[Source: _bmad-output/planning-artifacts/prd.md#Technical Constraints]  
[Source: _bmad-output/planning-artifacts/prd.md#Deployment Hardening]

### Architecture Compliance

- Keep service boundaries explicit:
  - `control-api` = authenticated command boundary
  - `governance-service` = rotation orchestration/policy decisions
  - `crates/persistence` = schema + query adapters
- Preserve canonical architecture contracts: machine-readable error envelopes, RFC3339 UTC timestamps, typed decision codes, and fail-closed governance behavior.
- Do not bypass governance pathways for convenience; rotation actions are privileged controls and must remain auditable.
- Maintain append-only audit evidence patterns already established in Story 1.4 and extended in Story 1.5.

[Source: _bmad-output/planning-artifacts/architecture.md#Authentication & Security]  
[Source: _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules]  
[Source: _bmad-output/planning-artifacts/architecture.md#Architectural Boundaries]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]

### Library & Framework Requirements

- Stay aligned with workspace-pinned stack:
  - `axum = 0.8.8`
  - `sqlx = 0.8.6`
  - `tokio = 1.48.0`
  - `time = 0.3.44`
  - PostgreSQL 18 baseline
- Latest checks confirm `axum` and `sqlx` pins match latest stable; `tokio` and `time` have newer stable releases, but this story should avoid opportunistic dependency churn.

[Source: Cargo.toml]  
[Source: _bmad-output/planning-artifacts/architecture.md#Data Architecture]  
[Source: _bmad-output/planning-artifacts/architecture.md#API & Communication Patterns]  
[Source: https://crates.io/api/v1/crates/axum]  
[Source: https://crates.io/api/v1/crates/sqlx]  
[Source: https://crates.io/api/v1/crates/tokio]  
[Source: https://crates.io/api/v1/crates/time]  
[Source: https://www.postgresql.org/docs/release/18.0/]

### File Structure Requirements

- Primary implementation surfaces for Story 1.6:
  - `crates/domain/src/governance.rs`
  - `crates/persistence/migrations/*credential_rotation*.sql`
  - `crates/persistence/src/postgres/{mod.rs,credential_rotation.rs}`
  - `services/governance-service/src/{lib.rs,main.rs,credentials/mod.rs}`
  - `services/control-api/src/{main.rs,middleware/mod.rs,routes/mod.rs}`
  - `docs/governance/rbac-role-model.md`
  - `docs/runbooks/*credential*`
- Keep changes additive and scoped; do not introduce unrelated top-level directories or cross-epic schema expansions.

[Source: _bmad-output/planning-artifacts/architecture.md#Complete Project Directory Structure]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: services/control-api/src/middleware/mod.rs]  
[Source: services/governance-service/src/approvals/mod.rs]  
[Source: crates/persistence/src/postgres/{mod.rs,approvals.rs,audit.rs}]

### Testing Requirements

- Add deterministic unit/integration coverage for:
  - Scheduled cadence boundaries (`< 90 days` deny/not-due, `>= 90 days` eligible)
  - Emergency-window handling (`<= 30 minutes` response target, expired/invalid windows fail closed)
  - Secret-provider outage/failure paths with explicit machine-readable errors
  - Non-compliant or unverifiable cryptographic posture paths (fail-closed denial and explicit reason code)
  - Rotation evidence/audit payload completeness and no-secret-leakage checks
- Preserve existing route and governance test patterns in `services/control-api/src/routes/mod.rs` and `services/governance-service/src/approvals/mod.rs`.
- Keep baseline quality gates compatible with repository scripts: `npm run ci:rust`, `npm run ci:security`, `npm test`.

[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: package.json]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: services/governance-service/src/approvals/mod.rs]  
[Source: crates/persistence/src/postgres/{approvals.rs,audit.rs}]

### Previous Story Intelligence

- Story 1.1 established security-first bootstrap guardrails: runtime secret injection only, no plaintext secret leakage, and reproducible policy checks.
- Story 1.2/1.3 established deterministic machine-readable auth/authz denials and strict privileged-route gating through middleware.
- Story 1.4 established immutable privileged audit append with sensitive-parameter redaction and fail-closed behavior when evidence cannot be persisted.
- Story 1.5 established canonical critical-action governance flows, explicit reason-code taxonomy, and RFC3339 UTC evidence discipline; Story 1.6 should preserve those patterns while adding rotation lifecycle coverage.

[Source: _bmad-output/implementation-artifacts/stories/1-1-set-up-initial-project-from-starter-template.md#Completion Notes List]  
[Source: _bmad-output/implementation-artifacts/stories/1-2-define-role-based-access-model.md#Completion Notes List]  
[Source: _bmad-output/implementation-artifacts/stories/1-3-add-authenticated-control-middleware.md#Completion Notes List]  
[Source: _bmad-output/implementation-artifacts/stories/1-4-implement-immutable-privileged-audit-logging.md#Completion Notes List]  
[Source: _bmad-output/implementation-artifacts/stories/1-5-enforce-dual-approval-governance-for-critical-mutations.md#Completion Notes List]

### Git Intelligence Summary

- Recent Epic 1 commit pattern concentrates changes in:
  - `services/control-api` (route/middleware enforcement + test-first hardening)
  - `services/governance-service` (policy orchestration modules)
  - `crates/domain` (canonical contracts/reason codes)
  - `crates/persistence` (forward-only migrations + explicit adapter errors)
- Continue same pattern for Story 1.6: deterministic contracts, scoped migration, explicit machine-readable failures, and evidence-rich tests.

[Source: git --no-pager log --oneline -5]  
[Source: git --no-pager show --name-only a0100cb5eaf21ce61eb6c779b8d8be26f060b91d]  
[Source: git --no-pager show --name-only 7f8ca347d5fc3548fcb81c38c269f1b76c7baa0c]

### Latest Technical Information

- Latest stable checks:
  - `axum` latest stable: `0.8.8` (matches workspace)
  - `sqlx` latest stable: `0.8.6` (matches workspace)
  - `tokio` latest stable: `1.51.0` (workspace `1.48.0`)
  - `time` latest stable: `0.3.47` (workspace `0.3.44`)
- PostgreSQL 18 remains architecture baseline for governance/audit persistence.
- Story 1.6 should prioritize compatibility with current workspace pins and avoid unrelated version upgrades.

[Source: Cargo.toml]  
[Source: https://crates.io/api/v1/crates/axum]  
[Source: https://crates.io/api/v1/crates/sqlx]  
[Source: https://crates.io/api/v1/crates/tokio]  
[Source: https://crates.io/api/v1/crates/time]  
[Source: https://www.postgresql.org/docs/release/18.0/]

### Project Context Reference

- No `project-context.md` file was found in repository scope during discovery.
- Story context was derived from epics, PRD, architecture, UX spec, previous story artifacts, governance docs, and recent commit history.

### Project Structure Notes

- Existing code already includes authenticated control-path and governance approval orchestration; credential rotation should compose with this pipeline, not replace it.
- `services/control-api/src/main.rs` currently wires durable Postgres approval persistence and in-memory audit append port; Story 1.6 must preserve fail-closed behavior and avoid weakening current evidence guarantees.
- **Open design constraint (non-blocking):** specific secret-manager vendor/adapter is not pinned in architecture; implement a provider-agnostic rotation port with explicit failure signaling and deterministic tests.

### References

- _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)  
- _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)  
- _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)  
- _bmad-output/planning-artifacts/epics.md#Story 1.6: Add Scheduled and Emergency Credential Rotation Flows  
- _bmad-output/planning-artifacts/prd.md#Governance, Security & Audit  
- _bmad-output/planning-artifacts/prd.md#Security Architecture & Threat Model  
- _bmad-output/planning-artifacts/prd.md#Technical Constraints  
- _bmad-output/planning-artifacts/prd.md#Deployment Hardening  
- _bmad-output/planning-artifacts/architecture.md#Data Architecture  
- _bmad-output/planning-artifacts/architecture.md#Authentication & Security  
- _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules  
- _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries  
- _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping  
- _bmad-output/planning-artifacts/ux-design-specification.md#Experience Principles  
- docs/governance/rbac-role-model.md  
- docs/runbooks/runtime-least-privilege.md  
- docs/operations/bootstrap.md  
- services/control-api/src/{main.rs,middleware/mod.rs,routes/mod.rs}  
- services/governance-service/src/approvals/mod.rs  
- services/governance-service/src/audit/mod.rs  
- crates/domain/src/governance.rs  
- crates/persistence/src/postgres/{mod.rs,approvals.rs,audit.rs}  
- crates/persistence/migrations/{20260405025656_rbac_roles_permissions.sql,20260405035200_audit_log_append.sql,20260405074300_approval_requests_votes.sql}  
- Cargo.toml  
- package.json

## Story Completion Status

- Story context generated with exhaustive artifact analysis (workflow inputs, epic/PRD/architecture/UX, previous stories, git history, and current implementation surfaces).
- Story is ready for implementation by dev agents.
- Completion note: Ultimate context engine analysis completed - comprehensive developer guide created.

## Dev Agent Record

### Agent Model Used

GPT-5.3-Codex (gpt-5.3-codex)

### Debug Log References

- BMAD create-story workflow execution (automated)
- Sprint backlog discovery from `_bmad-output/implementation-artifacts/sprint-status.yaml`
- Artifact loading from planning/implementation/governance docs and recent git history
- Rust toolchain bootstrap for local CI parity (`cargo`, `pnpm`) and full quality-gate execution.

### Completion Notes List

- Identified next backlog story as `1-6-add-scheduled-and-emergency-credential-rotation-flows`.
- Expanded story acceptance criteria with universal failure/boundary/evidence contracts and explicit traceability mapping.
- Added implementation-ready tasks/subtasks aligned to existing architecture and current repository module boundaries.
- Captured continuity constraints from Stories 1.1–1.5 to prevent regression in auth, audit, and governance flows.
- Included provider-agnostic secret-rotation orchestration guidance due unresolved secret-manager vendor decision in architecture artifacts.
- Implemented canonical credential-rotation domain model with deterministic cadence/emergency boundary helpers and strict non-secret metadata contract enforcement.
- Added forward-only `credential_rotation_events` migration with status/trigger constraints, traceability indexes, and secret-material rejection checks.
- Implemented persistence adapters, governance orchestration service, and authenticated control-api endpoints for scheduled + emergency rotation triggers.
- Added deterministic unit/integration coverage across domain, persistence, governance-service, and control-api; introduced `qa:test:story-1-6`.
- Updated governance and runbook documentation for Story 1.6 lifecycle, emergency response, rollback verification, and operator-safe secret handling.
- Code review triage fixed metadata error-contract fidelity and required-metadata validation gaps; expanded control-api tests for missing metadata and runtime dependency failures.
- Code review noted unrelated non-application workspace changes (`.gitignore`, `_bmad/**`, `.scripts/**`) and excluded them from story implementation scope.
- Executed BMAD QA automation workflow for Story 1.6 and expanded emergency rotation endpoint coverage for allow path, invalid payload, missing metadata, unauthorized role, and provider failure machine-reason behavior.
- Published Story 1.6 QA automation report to `_bmad-output/implementation-artifacts/tests/test-summary.md` with updated endpoint coverage and quality-gate outcomes.

### File List

- _bmad-output/implementation-artifacts/stories/1-6-add-scheduled-and-emergency-credential-rotation-flows.md
- _bmad-output/implementation-artifacts/tests/test-summary.md
- _bmad-output/implementation-artifacts/deferred-work.md
- crates/domain/src/governance.rs
- crates/persistence/migrations/20260405103000_credential_rotation_events.sql
- crates/persistence/src/postgres/mod.rs
- crates/persistence/src/postgres/credential_rotation.rs
- services/governance-service/src/lib.rs
- services/governance-service/src/main.rs
- services/governance-service/src/credentials/mod.rs
- services/control-api/src/main.rs
- services/control-api/src/middleware/mod.rs
- services/control-api/src/routes/mod.rs
- docs/governance/rbac-role-model.md
- docs/runbooks/runtime-least-privilege.md
- docs/runbooks/credential-rotation-operations.md
- package.json

### Change Log

- 2026-04-05: Created Story 1.6 context file and advanced lifecycle target to `ready-for-dev`.
- 2026-04-05: Implemented Story 1.6 scheduled/emergency credential rotation domain, persistence, governance orchestration, authenticated control-api integration, documentation, and deterministic QA coverage.
- 2026-04-05: Completed adversarial code-review auto-fix pass; corrected metadata failure-code contracts, enforced required metadata fields, and added route-level missing-metadata/runtime-failure coverage.
- 2026-04-05: Ran BMAD QA automation for Story 1.6, added emergency rotation API automation scenarios, and refreshed `_bmad-output/implementation-artifacts/tests/test-summary.md`.
