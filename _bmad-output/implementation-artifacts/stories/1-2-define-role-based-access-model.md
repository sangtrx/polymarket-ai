# Story 1.2: Define Role-Based Access Model

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an admin,  
I want explicit role definitions and permission boundaries,  
so that read-only users and trading-control users are strictly separated.

## Acceptance Criteria

1. **Role boundary enforcement (story-local BDD):**  
   **Given** user roles are configured  
   **When** a user attempts to access control-plane actions outside their role  
   **Then** access is denied with an explicit authorization error  
   **And** role mappings satisfy FR31 and least-privilege separation constraints.
2. **UAC-1 Failure handling:** Invalid role assignments, unknown roles/permissions, and unauthorized access checks return explicit machine-readable errors with no privilege escalation and no unsafe side effects.
3. **UAC-2 Boundary behavior:** Permission boundaries are deterministic and test-covered for read-only analytics, operational control, and administrative actions (including inclusive/exclusive edge rules for inherited or composite permissions).
4. **UAC-3 Verifiable evidence:** Successful and failed authorization decisions emit timestamped telemetry evidence (actor, role, action, outcome, reason) suitable for incident and QA traceability.
5. **Traceability and dependency constraints:** Story depends on 1.1, introduces only `roles`, `role_permissions`, and `user_roles` schema entities, and preserves explicit FR31 + NFR8 mapping.

## Tasks / Subtasks

- [x] **Task 1: Define canonical RBAC domain model and permission taxonomy** (AC: 1, 2, 3, 5)
  - [x] Expand `crates/domain/src/governance.rs` with role, permission, and authorization-decision types aligned to FR31 categories.
  - [x] Define the minimum role set and permission matrix for `read-only analytics`, `operational control`, and `administrative actions`.
  - [x] Add deterministic validation for unknown roles, duplicate mappings, and invalid permission combinations.
- [x] **Task 2: Implement RBAC persistence schema and access layer** (AC: 1, 2, 3, 5)
  - [x] Add forward-only SQL migration(s) under `crates/persistence/migrations/` for `roles`, `role_permissions`, and `user_roles` with PK/FK/unique constraints.
  - [x] Add `sqlx`-based RBAC persistence module(s) in `crates/persistence/src/postgres/` for role lookup and user-role resolution.
  - [x] Ensure schema and repository APIs enforce least-privilege defaults (no implicit admin grants).
- [x] **Task 3: Add authorization evaluation service logic** (AC: 1, 2, 3, 4)
  - [x] Introduce governance-side authorization evaluator module(s) that take actor role + requested control action and return allow/deny decision.
  - [x] Return explicit, machine-readable authorization errors for denied decisions (policy violation/insufficient role/unknown action).
  - [x] Emit structured decision telemetry with UTC timestamp and correlation metadata.
- [x] **Task 4: Wire control-plane integration seam without full auth implementation** (AC: 1, 2, 4, 5)
  - [x] Add reusable authorization guard interfaces in `services/control-api` that Story 1.3 can plug authenticated identity into.
  - [x] Integrate at least one non-health control-path check to prove deny behavior for out-of-role actions.
  - [x] Keep authentication-provider integration out of scope for this story (Story 1.3 owns authenticated identity middleware).
- [x] **Task 5: Add coverage for positive/negative authorization paths and boundaries** (AC: 2, 3, 4, 5)
  - [x] Add unit tests for role-permission matrix rules (allow/deny and boundary edges).
  - [x] Add integration tests for denied control-path access returning explicit machine-readable authorization errors.
  - [x] Add migration/schema tests verifying constraints prevent invalid role mappings and duplicate assignments.
- [x] **Task 6: Document role model and operational guardrails** (AC: 1, 4, 5)
  - [x] Add/update governance documentation for role definitions, permission boundaries, and assignment policy.
  - [x] Document operational review expectations for access changes (traceability and least-privilege checks).

### Review Findings

- [x] [Review][Patch] Removed explicit `BEGIN` / `COMMIT` statements from RBAC migration to keep compatibility with transaction-managed migration runners. [crates/persistence/migrations/20260405025656_rbac_roles_permissions.sql]
- [x] [Review][Patch] Updated authorization telemetry timestamps to RFC3339 UTC format for architecture compliance and traceability consistency. [crates/domain/src/governance.rs]
- [x] [Review][Patch] Added missing negative control-path tests for invalid actor context and unknown role handling. [services/control-api/src/routes/mod.rs]

## Dev Notes

### Technical Requirements

- Story 1.2 depends on Story 1.1 and must only introduce the RBAC schema entities defined in the traceability index: `roles`, `role_permissions`, and `user_roles`.
- FR31 is the primary functional contract: explicit access levels for analytics read-only, operational control, and administrative actions.
- NFR8 applies immediately: every administrative or trading-control action requires authenticated identity + RBAC policy checks, and unauthorized attempts must be observable.
- Preserve universal story contracts: explicit failure handling, deterministic boundary behavior, and verifiable telemetry evidence for both success and failure paths.
- **Out of scope for Story 1.2:** full authentication provider/token middleware (Story 1.3), immutable privileged audit storage (Story 1.4), and dual-approval flow logic (Story 1.5).

[Source: _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)]  
[Source: _bmad-output/planning-artifacts/epics.md#Story 1.2: Define Role-Based Access Model]  
[Source: _bmad-output/planning-artifacts/prd.md#Governance, Security & Audit]  
[Source: _bmad-output/planning-artifacts/prd.md#Non-Functional Requirements]  
[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]

### Architecture Compliance

- Follow strict RBAC separation between read-only and trading-control roles; do not collapse governance boundaries for convenience.
- Route privileged authorization decisions through governance-owned pathways; avoid direct execution-side policy mutation.
- Preserve canonical machine-readable error envelope behavior and deterministic control-flow outcomes.
- Keep naming, timestamp, and event conventions aligned with architecture patterns (`snake_case`, ISO-8601 UTC, versioned event names).

[Source: _bmad-output/planning-artifacts/architecture.md#Authentication & Security]  
[Source: _bmad-output/planning-artifacts/architecture.md#Architectural Boundaries]  
[Source: _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules]

### Library & Framework Requirements

- Rust workspace conventions remain the baseline (`edition = 2024`, Tokio async runtime, Axum for control API surfaces, Serde for typed contracts).
- Implement persistence through PostgreSQL + `sqlx` according to architecture direction; do not introduce alternate ORM/storage patterns.
- Reuse existing shared crates (`crates/domain`, `crates/common`, `crates/persistence`) before creating new cross-cutting utility layers.

[Source: Cargo.toml]  
[Source: _bmad-output/planning-artifacts/architecture.md#Data Architecture]  
[Source: _bmad-output/planning-artifacts/architecture.md#API & Communication Patterns]

### File Structure Requirements

- Prefer extending existing bounded modules instead of introducing new top-level structure:
  - `crates/domain/src/governance.rs`
  - `crates/persistence/src/postgres/*` + `crates/persistence/migrations/*`
  - `services/control-api/src/{routes,handlers,middleware}/*`
  - `services/governance-service/src/*` (RBAC evaluator/service composition)
- Keep service boundaries explicit: control API coordinates request handling, governance evaluates authorization policy, persistence owns schema/query mechanics.
- Keep health endpoints non-privileged; only control-plane mutation paths should require RBAC checks.

[Source: _bmad-output/planning-artifacts/architecture.md#Complete Project Directory Structure]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: _bmad-output/planning-artifacts/architecture.md#Service Boundaries]

### Testing Requirements

- Add deterministic unit tests for authorization matrix rules and boundary conditions.
- Add integration coverage proving explicit deny behavior for out-of-role control actions with machine-readable errors.
- Add migration/schema tests for FK/unique constraints in `roles`, `role_permissions`, `user_roles`.
- Maintain existing baseline quality gates (`npm run ci:rust`, `npm run ci:security`) and keep no-secret / least-privilege guarantees intact.

[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: package.json]  
[Source: _bmad-output/planning-artifacts/prd.md#Technical Constraints]

### Previous Story Intelligence

- Story 1.1 established a strict machine-readable failure pattern and timestamped evidence expectations; keep the same rigor for authorization denials and policy-check telemetry.
- Story 1.1 also enforced security-first bootstrap guardrails (no plaintext secrets, least-privilege runtime defaults); Story 1.2 must preserve these constraints while adding role policy logic.
- The repository now has architecture-aligned scaffolds; implement RBAC by extending those scaffolds, not by restructuring the project.

[Source: _bmad-output/implementation-artifacts/stories/1-1-set-up-initial-project-from-starter-template.md#Acceptance Criteria]  
[Source: _bmad-output/implementation-artifacts/stories/1-1-set-up-initial-project-from-starter-template.md#Completion Notes List]

### Git Intelligence Summary

- Recent implementation work is concentrated in Story 1.1 bootstrap scaffolding across Rust services, shared crates, CI workflows, and bootstrap validation tests.
- Follow the same pattern of explicit guardrail tests + clear failure contracts when adding RBAC behavior.
- Keep Story 1.2 changes scoped to role model definition and authorization boundaries; avoid forward-implementing Story 1.3+ features.

[Source: git log -5]  
[Source: git show 4bbf2f7 --name-only]

### Latest Technical Information

- Architecture-selected versions remain the active baseline for this codebase and should be kept consistent while implementing Story 1.2:
  - Axum `0.8.8`
  - Tokio `1.48.0`
  - Serde `1.0.228`
  - Polymarket SDK `0.4.4` (`clob`, `ws` features) for later execution stories
  - PostgreSQL 18 + `sqlx` `0.8.6` architecture target for persistence
- For this story, prioritize consistency with architecture-pinned stack choices over opportunistic version churn.

[Source: Cargo.toml]  
[Source: _bmad-output/planning-artifacts/architecture.md#Selected Starter: Dual-Starter Monorepo Baseline]  
[Source: _bmad-output/planning-artifacts/architecture.md#Data Architecture]  
[Source: _bmad-output/planning-artifacts/architecture.md#API & Communication Patterns]

### Project Context Reference

- No `project-context.md` was found in repository scope during discovery.
- Context was derived from planning artifacts, architecture, PRD, UX spec, prior story file, and recent commit history.

### Project Structure Notes

- Current service modules are intentionally scaffold-level; Story 1.2 should introduce the first substantive governance/security behavior while keeping interfaces clean for Story 1.3 auth middleware and Story 1.4 audit append.
- Keep implementation narrowly focused on role model definitions and authorization boundaries to prevent cross-story scope bleed.

### References

- _bmad-output/planning-artifacts/epics.md#Epic 1: Secure Operator Access & Governance Control Plane  
- _bmad-output/planning-artifacts/epics.md#Story 1.2: Define Role-Based Access Model  
- _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)  
- _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)  
- _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)  
- _bmad-output/planning-artifacts/prd.md#Technical Constraints  
- _bmad-output/planning-artifacts/prd.md#Governance, Security & Audit  
- _bmad-output/planning-artifacts/prd.md#Non-Functional Requirements  
- _bmad-output/planning-artifacts/architecture.md#Authentication & Security  
- _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules  
- _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries  
- _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping  
- _bmad-output/implementation-artifacts/stories/1-1-set-up-initial-project-from-starter-template.md  
- Cargo.toml  
- package.json  

## Story Completion Status

- Story context generated with exhaustive artifact analysis (epics, PRD, architecture, UX, prior story, codebase scaffold, and git history).
- Story implementation and review completed; status advanced to `done` after adversarial review and patch application.
- Completion note: Added post-review reliability/compliance fixes for migration execution, telemetry timestamp formatting, and negative-path authorization coverage.

## Dev Agent Record

### Agent Model Used

GPT-5.3-Codex (gpt-5.3-codex)

### Debug Log References

- BMAD create-story workflow execution (automated)
- BMAD bmad-qa-generate-e2e-tests workflow execution (automated)
- `cargo test -p control-api --all-targets`
- `npm run rust:fmt`
- `npm run rust:lint`
- `npm run qa:test:story-1-2`
- `cargo test --workspace --all-targets`
- `npm run ci:rust`
- `npm run ci:security`
- `npm test`

### Completion Notes List

- Implemented canonical RBAC domain model in `crates/domain/src/governance.rs` with explicit FR31 role taxonomy, deterministic permission boundaries, and machine-readable deny/error reasons.
- Added deterministic validation for unknown roles/permissions, duplicate role-permission mappings, and invalid boundary combinations with unit coverage for allow/deny and edge paths.
- Added forward-only RBAC migration (`roles`, `role_permissions`, `user_roles`) and `sqlx` repository helpers for role-permission and user-role resolution without implicit privilege grants.
- Added governance-side authorization evaluator service module and telemetry evidence formatting for actor/role/action/outcome/reason/correlation/timestamp traceability.
- Wired control-plane integration seam in `control-api` with reusable authorization guard interface + `POST /control/rebalance` deny/allow enforcement while keeping health endpoint non-privileged.
- Added integration tests that verify explicit machine-readable denied control-path behavior and positive operational-control authorization behavior.
- Added governance RBAC operational documentation for role definitions, assignment guardrails, and access-review expectations.
- Code review patch: removed explicit SQL transaction wrappers in RBAC migration for migration-runner compatibility.
- Code review patch: switched authorization decision timestamps to RFC3339 UTC and added format coverage.
- Code review patch: added negative API tests for missing correlation-id and unknown role request paths.
- QA automation: added denied-path traceability assertions for actor/role/correlation/message/timestamp fields on `POST /control/rebalance`.
- QA automation: added deterministic role-boundary matrix coverage for read-only, operational-control, and administrative roles on `POST /control/rebalance`.
- QA automation: added story-specific QA execution entrypoint (`npm run qa:test:story-1-2`) for repeatable API/E2E verification.
- Code review discrepancy note: `.scripts/bmad-auto/copilot/bmad-progress.log` appeared in git status but was excluded from application-source review scope.

### File List

- Cargo.lock
- Cargo.toml
- _bmad-output/implementation-artifacts/sprint-status.yaml
- _bmad-output/implementation-artifacts/stories/1-2-define-role-based-access-model.md
- _bmad-output/implementation-artifacts/tests/test-summary.md
- crates/domain/Cargo.toml
- crates/domain/src/governance.rs
- crates/persistence/Cargo.toml
- crates/persistence/migrations/20260405025656_rbac_roles_permissions.sql
- crates/persistence/src/postgres/mod.rs
- crates/persistence/src/postgres/rbac.rs
- docs/governance/rbac-role-model.md
- services/control-api/Cargo.toml
- services/control-api/src/main.rs
- services/control-api/src/middleware/mod.rs
- services/control-api/src/routes/mod.rs
- package.json
- services/governance-service/Cargo.toml
- services/governance-service/src/main.rs
- services/governance-service/src/rbac/mod.rs

### Change Log

- 2026-04-05: Implemented Story 1.2 RBAC role model, persistence schema/access layer, governance authorization evaluation, control-path guard integration, comprehensive tests, and governance documentation.
- 2026-04-05: Adversarial code review triage applied: fixed migration transaction wrapping, standardized authorization telemetry timestamp format (RFC3339 UTC), and expanded negative authorization-path test coverage.
- 2026-04-05: QA automation pass added story-specific RBAC API/E2E coverage for traceability fields and deterministic role-boundary enforcement; wired repeatable `qa:test:story-1-2` execution command.
