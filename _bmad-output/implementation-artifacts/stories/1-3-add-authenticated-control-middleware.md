# Story 1.3: Add Authenticated Control Middleware

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an operator,  
I want all privileged endpoints to require authenticated identity,  
so that only verified actors can invoke production controls.

## Acceptance Criteria

1. **Privileged endpoint authentication (story-local BDD):**  
   **Given** a request to a privileged endpoint  
   **When** identity is missing or invalid  
   **Then** the request is rejected before command execution  
   **And** successful requests carry actor identity into downstream audit correlation to satisfy FR31 control requirements.
2. **UAC-1 Failure handling:** Missing credentials, invalid credential material, unknown actor context fields, and unauthenticated privileged requests return explicit machine-readable errors with no privileged side effects.
3. **UAC-2 Boundary behavior:** Authentication enforcement is deterministic and test-covered for protected vs non-protected endpoints, including invalid/expired/malformed identity boundaries and correlation-id propagation boundaries.
4. **UAC-3 Verifiable evidence:** Successful and failed privileged authentication decisions emit timestamped telemetry evidence with actor/correlation context suitable for Story 1.4 audit append integration.
5. **Traceability and dependency constraints:** Story depends on 1.2, introduces no new schema entities, and preserves explicit FR31 + NFR8 + NFR9 mapping.
6. **Authentication boundary hardening:** Authentication is implemented as a provider-agnostic, fail-closed middleware seam (no implicit trust of raw caller-supplied identity fields and no hard-coded OIDC vendor selection in this story).
7. **NFR8 alertability contract:** Failed privileged authentication decisions emit alert-compatible security signals so unauthorized privileged-attempt detection can be wired to ≤30 second alerting objectives.

## Tasks / Subtasks

- [x] **Task 1: Define authenticated actor contract and middleware interfaces** (AC: 1, 2, 3, 5, 6)
  - [x] Add/extend control API middleware types for an authenticated actor context (actor_id, role, correlation_id, authentication outcome) rather than raw header pass-through.
  - [x] Define deterministic identity-validation rules for missing/empty/malformed authentication inputs and map each failure to machine-readable error codes.
  - [x] Preserve least-privilege defaults: identity is denied unless positively validated.
  - [x] Introduce a provider-agnostic authenticator interface/trait with fail-closed behavior when identity cannot be verified.
  - [x] Keep concrete provider/vendor selection out of scope for this story and avoid hard-coding unresolved OIDC vendor assumptions.
- [x] **Task 2: Enforce authentication before privileged control execution** (AC: 1, 2, 3, 5)
  - [x] Apply authentication middleware to privileged control-plane routes (starting with `/control/rebalance`) so rejection occurs before handler command logic.
  - [x] Keep non-privileged health/readiness endpoints unprotected.
  - [x] Ensure authentication failures do not invoke downstream RBAC/command execution logic.
- [x] **Task 3: Integrate authenticated identity with existing RBAC guardrails** (AC: 1, 3, 4, 5)
  - [x] Feed middleware-verified actor context into existing governance authorization evaluation path from Story 1.2.
  - [x] Ensure allow/deny responses and telemetry include authenticated actor and correlation metadata needed for downstream immutable audit correlation.
  - [x] Keep role-boundary behavior deterministic and consistent with canonical role matrix from Story 1.2.
- [x] **Task 4: Standardize machine-readable auth error and telemetry envelopes** (AC: 2, 4, 5, 7)
  - [x] Reuse canonical error envelope patterns (`error.code`, operator-safe message, structured details where applicable) for unauthenticated/invalid identity denials.
  - [x] Emit RFC3339 UTC timestamps for all auth decision telemetry.
  - [x] Ensure denied auth attempts are visible for incident and QA traceability expectations (NFR8/NFR9).
  - [x] Emit versioned auth decision security events with actor/correlation context following architecture event naming/envelope conventions.
  - [x] Include alert-compatible signal attributes for denied privileged auth attempts to support ≤30s unauthorized-attempt alert objectives.
- [x] **Task 5: Add deterministic auth middleware test coverage** (AC: 2, 3, 4, 5, 6, 7)
  - [x] Add route-level integration tests for missing credentials, malformed credentials, invalid actor context, and valid authenticated privileged requests.
  - [x] Verify privileged route rejections occur pre-execution and return machine-readable error payloads.
  - [x] Verify non-privileged routes retain expected behavior.
  - [x] Verify telemetry evidence fields remain present for both success and failure paths.
  - [x] Add fail-closed tests for authenticator verification failures (unverifiable identity material and adapter failures).
  - [x] Add assertions that denied privileged auth paths emit alert-compatible security events/signals.
- [x] **Task 6: Document control-plane authentication contract for downstream stories** (AC: 4, 5)
  - [x] Update governance/operations documentation with required authentication input contract and identity propagation expectations.
  - [x] Explicitly document Story 1.4 handoff expectations for immutable audit append integration using authenticated actor context.

### Review Findings

- [x] [Review][Patch] Correct invalid header-encoding classification for actor context [services/control-api/src/middleware/mod.rs:69] — non-UTF8 `x-correlation-id` decode failures now return `auth_unknown_actor_context` instead of adapter failure.
- [x] [Review][Patch] Close auth boundary coverage gaps for invalid credential material and actor identifier format [services/control-api/src/routes/mod.rs:309] — added regression tests for `auth_invalid_credentials` and invalid actor-id context rejection.
- [x] [Review][Defer] Non-story automation artifact mismatch [.scripts/bmad-auto/copilot/bmad-progress.log] — modified in git working tree but outside application review scope.

## Dev Notes

### Technical Requirements

- Story 1.3 is explicitly dependent on Story 1.2 and must reuse the existing RBAC role/permission baseline rather than redefining role semantics.
- Traceability for Story 1.3 is FR31 + NFR8 + NFR9 with **no new database tables**.
- All privileged control actions must require authenticated identity and be rejected prior to command execution when identity validation fails.
- Successful privileged requests must propagate actor identity + correlation metadata to downstream telemetry/audit correlation paths.
- Architecture leaves exact OIDC provider/claims mapping unresolved; this story must implement a provider-agnostic authentication seam with fail-closed defaults instead of vendor lock-in.
- Failed privileged authentication decisions must emit alert-compatible security signals so NFR8 unauthorized-attempt alert timing targets can be met without introducing Story 1.4 audit persistence scope.
- **Out of scope for Story 1.3:** immutable append-only privileged audit storage (Story 1.4), dual-approval governance flows (Story 1.5), and credential rotation lifecycle automation (Story 1.6).

[Source: _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)]  
[Source: _bmad-output/planning-artifacts/epics.md#Story 1.3: Add Authenticated Control Middleware]  
[Source: _bmad-output/planning-artifacts/prd.md#Governance, Security & Audit]  
[Source: _bmad-output/planning-artifacts/prd.md#Non-Functional Requirements]

### Architecture Compliance

- Implement authentication at the `control-api` boundary using Axum middleware so privileged commands are gated before route business logic.
- Preserve architecture error-envelope and timestamp conventions (machine-readable code + operator-safe message + ISO-8601/RFC3339 UTC timestamps).
- Keep service boundaries explicit: `control-api` enforces authenticated request gate + delegates authorization decision logic; `governance-service`/domain rules remain source of authorization truth.
- Maintain least-privilege behavior and no-secret-leakage posture in logs and runtime inputs.
- Follow architecture event conventions for auth decision telemetry/signals (versioned event naming plus actor/correlation envelope context).
- Keep middleware authentication integration provider-agnostic until the architecture-level provider selection gap is explicitly resolved.

[Source: _bmad-output/planning-artifacts/architecture.md#Authentication & Security]  
[Source: _bmad-output/planning-artifacts/architecture.md#API & Communication Patterns]  
[Source: _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules]  
[Source: _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries]

### Library & Framework Requirements

- Continue using workspace-pinned stack versions and conventions (Rust 2024 edition, Axum, Tokio, Serde, time crate).
- Reuse existing governance domain structures (`AuthorizationEvaluator`, `AuthorizationRequest`, `AuthorizationDecision`) and avoid introducing redundant authorization primitives.
- Avoid speculative framework churn; prefer extending existing middleware and domain guardrails.
- Do not introduce new external auth-provider SDK dependencies in this story unless architecture decisions are updated and explicitly cited.

[Source: Cargo.toml]  
[Source: _bmad-output/planning-artifacts/architecture.md#Core Architectural Decisions]  
[Source: _bmad-output/planning-artifacts/architecture.md#API & Communication Patterns]

### File Structure Requirements

- Primary implementation surface should remain in existing architecture-aligned modules:
  - `services/control-api/src/middleware/*`
  - `services/control-api/src/routes/*`
  - `services/control-api/src/main.rs`
  - `crates/common/src/telemetry.rs` (if shared auth event helpers are required)
  - `services/governance-service/src/rbac/*` (only if needed for identity handoff parity)
  - `crates/domain/src/governance.rs` (only for minimal shared contract extensions)
- Do not add persistence migrations or new schema artifacts in this story.
- Preserve health endpoint non-privileged behavior while enforcing auth on privileged control-plane routes.

[Source: _bmad-output/planning-artifacts/architecture.md#Complete Project Directory Structure]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: _bmad-output/implementation-artifacts/stories/1-2-define-role-based-access-model.md#Tasks / Subtasks]

### Testing Requirements

- Add deterministic integration tests for auth middleware failure and success paths on privileged routes.
- Verify unauthenticated requests fail with explicit machine-readable errors and no command-side effects.
- Verify authenticated requests continue through RBAC checks and preserve traceability fields.
- Verify authenticator failures are fail-closed and never bypass privileged gates.
- Verify denied privileged auth attempts emit alert-compatible security event/signal attributes.
- Keep regression coverage for existing role-boundary behavior introduced in Story 1.2.
- Maintain baseline quality gate compatibility with existing Rust CI scripts.

[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: _bmad-output/implementation-artifacts/stories/1-2-define-role-based-access-model.md#Testing Requirements]  
[Source: package.json]

### Previous Story Intelligence

- Story 1.2 already implemented canonical RBAC role boundaries, machine-readable deny reasons, and deterministic control-path tests; Story 1.3 should build on this foundation instead of replacing it.
- Existing `control-api` flow currently extracts actor context directly from headers; authentication middleware should harden this seam while preserving established response/telemetry contracts.
- Story 1.2 established that `/health` remains non-privileged and privileged paths are explicitly guarded; this must remain intact.
- Maintain compatibility with Story 1.2 machine-readable deny surfaces while replacing raw header trust with verified identity handoff semantics.

[Source: _bmad-output/implementation-artifacts/stories/1-2-define-role-based-access-model.md#Acceptance Criteria]  
[Source: _bmad-output/implementation-artifacts/stories/1-2-define-role-based-access-model.md#Tasks / Subtasks]  
[Source: services/control-api/src/routes/mod.rs]

### Git Intelligence Summary

- Recent implementation history shows Story 1.2 concentrated changes in `crates/domain`, `services/control-api`, `services/governance-service`, and RBAC persistence, with strong emphasis on explicit machine-readable errors and deterministic tests.
- Follow the same implementation pattern for Story 1.3: clear failure contracts, RFC3339 traceability fields, and targeted integration coverage around control routes.
- Keep Story 1.3 scoped to authenticated identity enforcement and propagation; do not pull forward Story 1.4+ scope.

[Source: git log -5]  
[Source: git show --name-only -1 HEAD]

### Latest Technical Information

- Workspace and architecture version baselines remain aligned for Story 1.3 implementation:
  - Axum `0.8.8`
  - Tokio `1.48.0`
  - Serde `1.0.228`
  - `time` crate `0.3.44` for RFC3339 timestamp formatting
  - `sqlx` `0.8.6` and PostgreSQL baseline remain in place but are not expanded by this story
- For this story, prioritize consistency with these pinned versions and existing code patterns over introducing new authentication stacks.

[Source: Cargo.toml]  
[Source: _bmad-output/planning-artifacts/architecture.md#Data Architecture]  
[Source: _bmad-output/planning-artifacts/architecture.md#Authentication & Security]

### Project Context Reference

- No `project-context.md` file was found in repository scope during discovery.
- Context was derived from planning artifacts, architecture, UX specification, prior story artifacts, and repository commit history.

### Project Structure Notes

- Authentication middleware should be treated as a control-plane gate in front of existing authorization checks, not a replacement for RBAC.
- Keep implementation constrained to privileged endpoint identity verification and propagation so Story 1.4 can add immutable audit persistence cleanly.
- Preserve existing deterministic test and telemetry conventions to minimize regression risk across ongoing Epic 1 governance stories.

### References

- _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)  
- _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)  
- _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)  
- _bmad-output/planning-artifacts/epics.md#Epic 1: Secure Operator Access & Governance Control Plane  
- _bmad-output/planning-artifacts/epics.md#Story 1.3: Add Authenticated Control Middleware  
- _bmad-output/planning-artifacts/prd.md#Governance, Security & Audit  
- _bmad-output/planning-artifacts/prd.md#Technical Constraints  
- _bmad-output/planning-artifacts/prd.md#Non-Functional Requirements  
- _bmad-output/planning-artifacts/architecture.md#Authentication & Security  
- _bmad-output/planning-artifacts/architecture.md#Important Gaps  
- _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules  
- _bmad-output/planning-artifacts/architecture.md#API & Communication Patterns  
- _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries  
- _bmad-output/implementation-artifacts/stories/1-2-define-role-based-access-model.md  
- _bmad-output/implementation-artifacts/stories/1-1-set-up-initial-project-from-starter-template.md  
- services/control-api/src/routes/mod.rs  
- services/control-api/src/middleware/mod.rs  
- crates/domain/src/governance.rs  
- Cargo.toml  
- package.json

## Story Completion Status

- Story implementation and code review completed with all HIGH/MEDIUM findings addressed.
- Acceptance criteria verified against privileged-route middleware behavior, machine-readable auth failures, and deterministic role-boundary outcomes.
- Story status synchronized to `done` after review-layer triage and patch application.
- Non-application automation log drift (`.scripts/bmad-auto/copilot/bmad-progress.log`) was observed and intentionally excluded from story code review scope.

## Dev Agent Record

### Agent Model Used

GPT-5.3-Codex (gpt-5.3-codex)

### Debug Log References

- BMAD create-story workflow execution (automated)
- `git --no-pager log --oneline -5`
- `git --no-pager show --name-only -1 HEAD`
- `cargo test -p control-api routes::tests::`
- `cargo test -p control-api`
- `npm run ci:rust`
- `npm test`
- `cargo fmt --all`
- `npm run rust:lint`
- `cargo test -p control-api routes::tests::`

### Completion Notes List

- Story requirements, acceptance criteria, and implementation guardrails synthesized from Epic 1, FR31/NFR8/NFR9 constraints, architecture boundaries, and prior-story implementation patterns.
- Task breakdown includes middleware gate enforcement, identity propagation, deterministic error behavior, and regression-safe testing requirements.
- Added provider-agnostic authenticated actor middleware seam in `control-api` with explicit `Authenticator` + `AuthorizationGuard` contracts and fail-closed behavior.
- Implemented deterministic privileged identity validation and machine-readable auth denial codes (`auth_missing_credentials`, `auth_malformed_credentials`, `auth_invalid_credentials`, `auth_expired_credentials`, `auth_unknown_actor_context`, `auth_verification_failed`).
- Wired privileged middleware enforcement on `POST /control/rebalance` while preserving unprotected `/health`; authenticated actor context now feeds existing RBAC evaluation path.
- Added auth decision telemetry/security signals with RFC3339 UTC timestamps, actor/correlation context, versioned event names, and alert-compatible denied-attempt metadata.
- Expanded control-api test coverage for missing/malformed/expired/unknown-context paths, fail-closed adapter failures, pre-execution rejection guarantees, and deterministic privileged role-boundary behavior.
- Documented control-plane authentication request contract and Story 1.4 immutable audit handoff expectations in governance docs.
- Reclassified invalid `x-correlation-id` header decoding to `auth_unknown_actor_context` for cleaner actor-context vs adapter-failure telemetry semantics.
- Added deterministic tests for invalid credential material (`auth_invalid_credentials`) and malformed actor-id boundaries to tighten UAC-2/UAC-3 regression coverage.
- Recorded git/file-list discrepancy for non-story automation artifact (`.scripts/bmad-auto/copilot/bmad-progress.log`) as deferred out-of-scope review noise.
- QA automation pass expanded Story 1.3 critical-flow coverage with route-level regression tests for invalid correlation-id format rejection and administrative allow-path traceability timestamp evidence.
- Generated Story 1.3 QA summary artifact at `_bmad-output/implementation-artifacts/tests/test-summary.md` and re-ran repository test suite to confirm guardrail stability.

### File List

- _bmad-output/implementation-artifacts/stories/1-3-add-authenticated-control-middleware.md
- _bmad-output/implementation-artifacts/sprint-status.yaml
- _bmad-output/implementation-artifacts/tests/test-summary.md
- Cargo.lock
- docs/governance/rbac-role-model.md
- services/control-api/Cargo.toml
- services/control-api/src/main.rs
- services/control-api/src/middleware/mod.rs
- services/control-api/src/routes/mod.rs

### Change Log

- 2026-04-04: Implemented Story 1.3 authenticated control middleware with fail-closed auth seam, privileged route middleware enforcement, authenticated identity propagation into RBAC, machine-readable auth error envelope, RFC3339 auth telemetry/security signals, deterministic integration/unit test coverage, and downstream Story 1.4 handoff documentation.
- 2026-04-04: Adversarial review triage auto-applied medium patches for invalid header-encoding classification and missing boundary regression tests; confirmed status advancement to `done`.
- 2026-04-05: QA automation generated additional Story 1.3 API/E2E regression tests for invalid correlation-id boundary handling and admin allow-path timestamp traceability, refreshed the QA summary artifact, and validated the full test suite with story status remaining `done`.
