# Story 2.7: Configure Portfolio, Market, and Strategy Limit Policies

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As a risk engine,  
I want configurable exposure and inventory limits across scopes,  
so that policy can be tuned without changing execution code.

## Acceptance Criteria

1. **Scoped limit configuration (story-local BDD):**  
   **Given** an operator defines portfolio/market/strategy limits  
   **When** limits are saved  
   **Then** versioned limit profiles are persisted and applied by the risk engine.
2. **Invalid configuration failure path (story-local BDD):**  
   **Given** a limit update breaches invariants (for example `market_limit > portfolio_limit`)  
   **When** validation executes  
   **Then** update is rejected with explicit validation errors.
3. **Governance boundary for critical increase (story-local BDD):**  
   **Given** a critical risk-limit increase is requested  
   **When** approval checks run  
   **Then** change remains pending until dual-approval constraints pass.
4. **UAC-1 Failure handling:** Invalid payloads, unauthorized mutations, unavailable persistence dependencies, and missing approval prerequisites return explicit machine-readable errors with no unsafe side effects.
5. **UAC-2 Boundary behavior:** Inclusive/exclusive limit boundaries are deterministic and test-covered (including `==` boundary acceptance and strict `>` rejection semantics for child-scope-over-parent-scope violations).
6. **UAC-3 Verifiable evidence:** Accepted, pending, and denied limit mutations emit timestamped evidence with actor, action, reason code, correlation metadata, and approval reference linkage when applicable.
7. **Schema/dependency/traceability contract:** Story depends only on `2.6`, creates only `risk_limit_profiles` and `inventory_limit_rules`, and maps explicitly to `FR17`, `FR19`, and `NFR17`.
8. **Audit-query contract (NFR17):** Active and pending limit-policy actions must be queryable with actor, action type, parameters, approval status, and reason fields within incident/audit retrieval paths.

## Tasks / Subtasks

- [x] **Task 1: Define canonical risk-limit domain contracts and reason codes** (AC: 1, 2, 3, 4, 5, 6, 7, 8)
  - [x] Extend `crates/domain/src/risk.rs` with typed contracts for versioned portfolio/market/strategy limit profiles and inventory/concentration rules.
  - [x] Add deterministic validation helpers for cross-scope invariants (portfolio >= market >= strategy where applicable) and non-negative/unit-bound fields.
  - [x] Add machine-readable reason-code taxonomy for applied/pending/denied limit mutations (including approval-required and policy-state-unavailable paths).
  - [x] Add domain tests for valid boundary acceptance, cross-scope invariant rejection, and deterministic reason-code outputs.

- [x] **Task 2: Add forward-only persistence migration for Story 2.7 schema scope** (AC: 1, 2, 5, 7, 8)
  - [x] Add migration under `crates/persistence/migrations/` creating only `risk_limit_profiles` and `inventory_limit_rules`.
  - [x] Add constraints for canonical identifiers, scope enums, finite/non-negative numeric limits, concentration/exposure percentage ranges, and UTC timestamps.
  - [x] Add indexes for active-profile lookup, scope/version retrieval, pending-approval retrieval, actor/correlation audit queries, and latest-effective policy reads.
  - [x] Ensure migration scope explicitly excludes Story 2.8+ tables (`pretrade_gate_decisions`, `safety_control_actions`).

- [x] **Task 3: Implement PostgreSQL risk-limit persistence adapter** (AC: 1, 2, 4, 5, 6, 7, 8)
  - [x] Add `crates/persistence/src/postgres/risk_limits.rs` and wire through `crates/persistence/src/postgres/mod.rs`.
  - [x] Implement transactional writes for versioned profile updates and associated inventory-rule sets.
  - [x] Implement read APIs for active profile bundles and pending critical updates (approval-gated).
  - [x] Return typed persistence errors with explicit machine codes; do not swallow constraint/transaction failures.

- [x] **Task 4: Add governance-service risk-limit orchestration with approval gating** (AC: 1, 2, 3, 4, 6, 8)
  - [x] Add `services/governance-service/src/risk_limits/mod.rs` (and export via `services/governance-service/src/lib.rs`) for risk-limit mutation orchestration.
  - [x] Reuse existing role authorization patterns used by market policy and critical-action workflows.
  - [x] Detect critical limit increases and gate activation until approval evidence is present for `risk_limit_increase`.
  - [x] Emit structured telemetry for accepted/pending/denied outcomes with machine reason codes and correlation metadata.

- [x] **Task 5: Expose authenticated control-plane risk-limit endpoints** (AC: 1, 2, 3, 4, 6, 8)
  - [x] Extend `services/control-api/src/routes/mod.rs` with authenticated endpoints for profile/rule updates and pending-status responses.
  - [x] Reuse `authorize_critical_action`, error-envelope conventions, and audit append patterns.
  - [x] For critical increases, ensure API response remains `pending` until dual approval succeeds; do not return synthetic approval references on denied/pending paths.
  - [x] Wire service state in `services/control-api/src/main.rs` and middleware/context surfaces as needed.

- [x] **Task 6: Implement risk-engine limit-state runtime surfaces** (AC: 1, 4, 5, 6, 8)
  - [x] Replace `services/risk-engine/src/limits/mod.rs` placeholder with runtime state and read interfaces for active limit bundles.
  - [x] Integrate bootstrap/loading seams in `services/risk-engine/src/main.rs` and gate-state interfaces needed by downstream pre-trade enforcement (without implementing Story 2.8 gate pipeline).
  - [x] Preserve fail-closed semantics when limit state is missing, stale, or invalid.
  - [x] Emit telemetry events compatible with existing gate/reason-code patterns.

- [x] **Task 7: Add deterministic Story 2.7 test coverage and QA command** (AC: 1, 2, 3, 4, 5, 6, 8)
  - [x] Add domain tests for cross-scope invariant validation and boundary semantics.
  - [x] Add persistence tests validating migration scope, constraints/indexes, write/read correctness, and pending-vs-active retrieval behavior.
  - [x] Add governance/control-api tests for unauthorized access, invalid payload rejection, pending critical-increase behavior, and approval-gated activation.
  - [x] Add risk-engine tests for limit-state availability semantics and fail-closed handling when policy state is unavailable.
  - [x] Add `qa:test:story-2-7` to `package.json` and update `_bmad-output/implementation-artifacts/tests/test-summary.md` with Story 2.7 evidence.

- [x] **Task 8: Add risk-limit policy operations runbook** (AC: 3, 4, 6, 8)
  - [x] Add/update `docs/operations/*risk-limit*` guidance for scope definitions, unit semantics, and approval-gated critical increase workflow.
  - [x] Document failure-mode playbooks for invalid invariants, missing approval evidence, and persistence unavailability.
  - [x] Include operator verification steps for active-vs-pending policy status and audit-reference correlation.

## Dev Notes

### Technical Requirements

- Story dependency is strict: `2.7` depends only on `2.6` and introduces no forward dependencies.
- Story scope is explicit and narrow:
  - Functional scope: configuration and persistence/orchestration of portfolio/market/strategy limit policies and inventory rules.
  - Schema scope: `risk_limit_profiles` and `inventory_limit_rules` only.
  - Traceability scope: `FR17`, `FR19`, `NFR17`.
- Critical behavior guardrails:
  - Profile updates must be versioned and deterministic.
  - Invalid cross-scope configurations must fail with explicit field-level errors and no partial writes.
  - Critical increases must remain pending until dual approval constraints are satisfied (`risk_limit_increase` critical action).
  - Machine-readable reason codes and correlation metadata are mandatory on allow/pending/deny paths.
- **Out of scope for Story 2.7:** full pre-trade gate evaluation orchestration (Story 2.8), emergency control workflows (Story 2.9), and operator-facing dashboard implementations (Epic 3).

[Source: _bmad-output/planning-artifacts/epics.md#Story 2.7: Configure Portfolio, Market, and Strategy Limit Policies]  
[Source: _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)]  
[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: _bmad-output/planning-artifacts/prd.md#Risk & Capital Management]  
[Source: _bmad-output/planning-artifacts/prd.md#Compliance & Auditability]

### Architecture Compliance

- Preserve architecture ownership boundaries:
  - `risk-engine` owns trading-eligibility and risk-limit runtime decision state.
  - `governance-service` owns privileged approval workflow enforcement.
  - `control-api` remains authenticated command boundary for policy mutations.
  - `execution-engine` must not mutate policy state directly.
- Keep FR17-FR21 mapping aligned with `services/risk-engine/src/{gates,limits,safe_state}` and integrate incrementally without bypassing existing gates.
- Follow consistency conventions:
  - snake_case module/database identifiers,
  - UTC timestamps only,
  - explicit machine-readable error envelopes,
  - no swallowed errors in risk/control paths.
- Treat stale/uncertain policy state as a fail-closed safety condition.

[Source: _bmad-output/planning-artifacts/architecture.md#Requirements Overview]  
[Source: _bmad-output/planning-artifacts/architecture.md#Technical Constraints & Dependencies]  
[Source: _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules]  
[Source: _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: _bmad-output/planning-artifacts/architecture.md#Implementation Handoff]

### Library & Framework Requirements

- Continue workspace-pinned stack for compatibility:
  - `polymarket-client-sdk = 0.4.4` (`clob`, `ws`)
  - `tokio = 1.48.0`
  - `sqlx = 0.8.6`
  - `axum = 0.8.8`
  - `time = 0.3.44`
- Latest-version checks at story creation time:
  - `polymarket-client-sdk`: latest stable `0.4.4`
  - `tokio`: latest stable `1.51.0`
  - `sqlx`: newest release `0.9.0-alpha.1` (pre-release); latest stable `0.8.6`
  - `axum`: latest stable `0.8.8`
  - `time`: latest stable `0.3.47`
- Do not introduce opportunistic dependency upgrades in Story 2.7; prioritize deterministic policy behavior and compatibility with existing Story 2 patterns.

[Source: Cargo.toml]  
[Source: https://crates.io/api/v1/crates/polymarket-client-sdk]  
[Source: https://crates.io/api/v1/crates/tokio]  
[Source: https://crates.io/api/v1/crates/sqlx]  
[Source: https://crates.io/api/v1/crates/axum]  
[Source: https://crates.io/api/v1/crates/time]

### File Structure Requirements

- Primary implementation surfaces:
  - `crates/domain/src/risk.rs`
  - `crates/persistence/migrations/*risk_limit*.sql`
  - `crates/persistence/src/postgres/{mod.rs,risk_limits.rs}`
  - `services/governance-service/src/{lib.rs,risk_limits/mod.rs}`
  - `services/control-api/src/{main.rs,routes/mod.rs,middleware/mod.rs}`
  - `services/risk-engine/src/{main.rs,limits/mod.rs,gates/mod.rs}`
  - `package.json`
  - `_bmad-output/implementation-artifacts/tests/test-summary.md`
  - `docs/operations/*risk-limit*`
- Reuse existing Story 2 layering pattern (domain → migration → persistence adapter → service orchestration → API integration → tests/runbook).
- Maintain strict schema scope and avoid introducing Story 2.8/2.9 entities in this story.

[Source: _bmad-output/planning-artifacts/architecture.md#Complete Project Directory Structure]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: services/risk-engine/src/limits/mod.rs]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: services/governance-service/src/market_policy/mod.rs]  
[Source: crates/persistence/src/postgres/market_policy.rs]

### Testing Requirements

- Add deterministic coverage for:
  - versioned profile persistence and active-profile resolution by scope,
  - invalid invariant rejection with explicit field errors (`market > portfolio`, `strategy > market/portfolio`, invalid percentage bounds),
  - pending critical-increase behavior until dual approval is satisfied,
  - fail-closed behavior for unauthorized mutation requests and unavailable persistence/state dependencies,
  - machine-readable reason codes, correlation IDs, UTC timestamps, and approval-reference behavior across accepted/pending/denied paths.
- Keep test layering consistent with existing repository patterns:
  - domain tests (`crates/domain`),
  - persistence migration/adapter tests (`crates/persistence`),
  - governance/control-api route tests (`services/governance-service`, `services/control-api`),
  - risk runtime tests (`services/risk-engine`).
- Add story-scoped QA script in `package.json` and update story test evidence summary.

[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: package.json]  
[Source: _bmad-output/implementation-artifacts/stories/2-1-configure-market-universe-policy-engine.md#Testing Requirements]  
[Source: _bmad-output/implementation-artifacts/stories/2-6-build-reconciliation-and-exposure-visibility-core.md#Testing Requirements]

### Previous Story Intelligence

- Story 2.6 established deterministic fail-closed deny semantics and reconciliation-halt state integration in `risk-engine` gates; Story 2.7 should extend these runtime-state patterns rather than introducing divergent gating behavior.
- Story 2.5/2.6 reinforced schema-per-story discipline and transactional persistence adapters with explicit machine-readable errors.
- Story 2.1 provides the closest implementation blueprint for policy-configuration flows (domain contracts, migration constraints, governance orchestration, control-api endpoints, telemetry, and operations runbook).
- Existing critical-action approval flow already recognizes `risk_limit_increase`; Story 2.7 should integrate with this pipeline for approval-gated critical limit increases.

[Source: _bmad-output/implementation-artifacts/stories/2-6-build-reconciliation-and-exposure-visibility-core.md#Completion Notes List]  
[Source: _bmad-output/implementation-artifacts/stories/2-5-implement-venue-compatible-order-lifecycle-handling.md#Completion Notes List]  
[Source: _bmad-output/implementation-artifacts/stories/2-1-configure-market-universe-policy-engine.md#Completion Notes List]  
[Source: services/risk-engine/src/gates/mod.rs]  
[Source: services/governance-service/src/market_policy/mod.rs]  
[Source: crates/domain/src/governance.rs#CriticalActionId]

### Git Intelligence Summary

- Recent Epic 2 commit sequence is consistent and should be preserved for Story 2.7:
  1. domain contracts/reason codes,  
  2. scoped forward-only migration,  
  3. persistence adapter wiring,  
  4. runtime/service/API integration,  
  5. deterministic tests + story QA command + operations runbook.
- Recent changes also show strict story-scoped schema exclusions and explicit source-of-truth boundaries, which should remain intact.

[Source: git --no-pager log --oneline -5]  
[Source: git --no-pager show --name-only --pretty=format:'%h %s' -5]

### Latest Technical Information

- Crate-version checks confirm architecture-selected versions remain safe for Story 2.7 implementation.
- `sqlx` has a newer pre-release (`0.9.0-alpha.1`), but stable remains `0.8.6`; avoid pre-release upgrades in this story.
- No dependency upgrades are required to deliver scoped risk-limit policy configuration and approval-gated behavior.

[Source: Cargo.toml]  
[Source: https://crates.io/api/v1/crates/polymarket-client-sdk]  
[Source: https://crates.io/api/v1/crates/tokio]  
[Source: https://crates.io/api/v1/crates/sqlx]  
[Source: https://crates.io/api/v1/crates/axum]  
[Source: https://crates.io/api/v1/crates/time]

### Project Context Reference

- No `project-context.md` file was found in repository scope during discovery.
- Context for this story was derived from epics, PRD, architecture, UX specification, readiness report, prior story files, git history, and current source surfaces.

### Project Structure Notes

- `services/risk-engine/src/limits/mod.rs` is currently a placeholder and is the intended Story 2.7 extension seam.
- `services/control-api/src/routes/mod.rs` already contains authenticated policy and critical-action flow patterns that Story 2.7 should reuse (including pending/deny mechanics and audit linkage).
- `crates/domain/src/governance.rs` already canonicalizes `risk_limit_increase` as a critical action identifier, enabling Story 2.7 approval-gated increase behavior without inventing a parallel approval mechanism.
- Story 2.7 should configure limits and runtime state only; enforce full pre-trade gate evaluation in Story 2.8.

[Source: services/risk-engine/src/limits/mod.rs]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: crates/domain/src/governance.rs#CriticalActionId]  
[Source: _bmad-output/planning-artifacts/epics.md#Story 2.8: Enforce Pre-Trade Gate Evaluation Pipeline]

### References

- _bmad-output/planning-artifacts/epics.md#Epic 2: Live Market Connectivity & Safe Core Execution  
- _bmad-output/planning-artifacts/epics.md#Story 2.7: Configure Portfolio, Market, and Strategy Limit Policies  
- _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)  
- _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)  
- _bmad-output/planning-artifacts/prd.md#Risk & Capital Management  
- _bmad-output/planning-artifacts/prd.md#Compliance & Auditability  
- _bmad-output/planning-artifacts/prd.md#Trading Reactivation Readiness Checklist  
- _bmad-output/planning-artifacts/architecture.md#Requirements Overview  
- _bmad-output/planning-artifacts/architecture.md#Technical Constraints & Dependencies  
- _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules  
- _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries  
- _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping  
- _bmad-output/planning-artifacts/architecture.md#Implementation Handoff  
- _bmad-output/planning-artifacts/ux-design-specification.md#Journey 2 — Incident Safe-State Workflow  
- _bmad-output/planning-artifacts/implementation-readiness-report-2026-04-05.md#✅ Remediation Outcomes  
- _bmad-output/implementation-artifacts/stories/2-1-configure-market-universe-policy-engine.md  
- _bmad-output/implementation-artifacts/stories/2-5-implement-venue-compatible-order-lifecycle-handling.md  
- _bmad-output/implementation-artifacts/stories/2-6-build-reconciliation-and-exposure-visibility-core.md  
- crates/domain/src/risk.rs  
- crates/domain/src/governance.rs  
- crates/persistence/src/postgres/market_policy.rs  
- services/control-api/src/routes/mod.rs  
- services/governance-service/src/market_policy/mod.rs  
- services/risk-engine/src/gates/mod.rs  
- services/risk-engine/src/limits/mod.rs  
- Cargo.toml  
- docs/operations/market-policy-engine.md  
- docs/operations/reconciliation-exposure-core.md

## Story Completion Status

- Story context generated with exhaustive artifact analysis (workflow inputs, epic/PRD/architecture/UX/readiness artifacts, previous story intelligence, git history, dependency checks, and current codebase seams).
- Story file is created and ready for implementation by dev agents.
- Completion note: Ultimate context engine analysis completed - comprehensive developer guide created.

## Dev Agent Record

### Agent Model Used

GPT-5.3-Codex (gpt-5.3-codex)

### Debug Log References

- BMAD create-story workflow execution (automated)
- Sprint backlog discovery from `_bmad-output/implementation-artifacts/sprint-status.yaml`
- Artifact discovery across planning/implementation/source-code surfaces
- Recent commit and changed-file pattern analysis for Story 2 continuity
- Latest-version checks via crates.io API endpoints
- Dev-story implementation run: domain → migration → persistence adapter → governance orchestration → control-api integration → risk-engine runtime surfaces
- Story QA command execution: `npm run --silent qa:test:story-2-7`
- Full quality gates execution: `npm run --silent rust:lint`, `npm run --silent test`, `npm run --silent rust:build`
- QA automation rerun for Story 2.7 (non-interactive BMAD execution): `npm run --silent qa:test:story-2-7` (pass)

### Completion Notes List

- Selected first backlog story: `2-7-configure-portfolio-market-and-strategy-limit-policies`.
- Captured story-local BDD acceptance criteria plus universal UAC contracts.
- Added implementation guardrails for strict schema scope, cross-scope invariant validation, and approval-gated critical limit increases.
- Carried forward Story 2.1/2.5/2.6 patterns for machine reason codes, fail-closed behavior, and deterministic persistence/runtime layering.
- Defined concrete implementation surfaces, testing requirements, and operations guidance for Story 2.7 execution.
- Implemented canonical risk-limit domain contracts with versioned portfolio/market/strategy profiles, inventory-rule contracts, deterministic invariant validation, and reason-code taxonomy for applied/pending/denied paths.
- Added forward-only migration `20260406072000_risk_limit_profiles.sql` constrained to `risk_limit_profiles` and `inventory_limit_rules` with strict scope, numeric, timestamp, and index contracts.
- Implemented `crates/persistence/src/postgres/risk_limits.rs` transactional persistence adapter with typed machine errors and active/pending profile bundle read APIs.
- Added governance orchestration module `services/governance-service/src/risk_limits/mod.rs` with role-gated mutations, critical-increase pending behavior, and structured telemetry.
- Extended control-api state + routes with authenticated risk-limit profile update and pending-query endpoints; reused authorization/audit/error-envelope patterns and preserved pending-path approval-reference constraints.
- Replaced risk-engine limits placeholder with runtime limit-state interfaces and fail-closed snapshot evaluation; integrated bootstrap loading seam and gate-facing limit-state function.
- Added deterministic Story 2.7 test coverage across domain, persistence, governance-service, control-api, and risk-engine modules; added `qa:test:story-2-7` script and updated test evidence summary.
- Added operations runbook `docs/operations/risk-limit-policy-operations.md` covering units/scopes, approval-gated workflow, failure playbooks, and operator verification steps.
- Code review fix: corrected control-api authorization audit metadata so the pending risk-limit GET endpoint records `http_method: GET` instead of `POST`, and added regression test `risk_limit_pending_route_authorization_audit_records_get_http_method`.
- Code review scope fix: removed unrelated formatting-only edits outside Story 2.7 implementation surfaces.
- Re-executed Story 2.7 QA automation suite (`qa:test:story-2-7`) in non-interactive workflow mode; all targeted critical-flow tests remained green and story status stayed `done`.

### File List

- _bmad-output/implementation-artifacts/stories/2-7-configure-portfolio-market-and-strategy-limit-policies.md
- _bmad-output/implementation-artifacts/sprint-status.yaml
- _bmad-output/implementation-artifacts/tests/test-summary.md
- Cargo.lock
- crates/domain/src/risk.rs
- crates/persistence/migrations/20260406072000_risk_limit_profiles.sql
- crates/persistence/src/postgres/mod.rs
- crates/persistence/src/postgres/risk_limits.rs
- docs/operations/risk-limit-policy-operations.md
- package.json
- services/control-api/src/main.rs
- services/control-api/src/middleware/mod.rs
- services/control-api/src/routes/mod.rs
- services/governance-service/src/lib.rs
- services/governance-service/src/risk_limits/mod.rs
- services/risk-engine/Cargo.toml
- services/risk-engine/src/gates/mod.rs
- services/risk-engine/src/limits/mod.rs
- services/risk-engine/src/main.rs

### Change Log

- 2026-04-06: Created Story 2.7 context file and moved lifecycle state from `backlog` to `ready-for-dev`.
- 2026-04-06: Implemented Story 2.7 risk-limit policy contracts, persistence/governance/API/runtime integrations, deterministic tests, QA command, and operations runbook; advanced story status to `review`.
- 2026-04-06: Completed code review auto-fixes (risk-limit pending-route audit method metadata + scope-cleanup of unrelated formatting changes) and advanced story status to `done`.
- 2026-04-06: Ran BMAD QA automation execution for Story 2.7 (`npm run --silent qa:test:story-2-7`); suite passed and status remained `done`.
