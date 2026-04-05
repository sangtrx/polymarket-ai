# Story 2.1: Configure Market Universe Policy Engine

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an operator,  
I want configurable universe thresholds for liquidity, spread, rewards, and exposure,  
so that only eligible markets are tradable.

## Acceptance Criteria

1. **Eligible market classification (story-local BDD):**  
   **Given** policy thresholds are configured  
   **When** the eligibility engine evaluates a market  
   **Then** only markets meeting all configured constraints are marked tradable.
2. **Invalid policy boundary handling (story-local BDD):**  
   **Given** an operator submits invalid thresholds (negative depth, spread < 0, or max exposure > 100%)  
   **When** validation executes  
   **Then** the policy update is rejected with explicit field-level errors and no partial persistence.
3. **Runtime cluster toggle (story-local BDD):**  
   **Given** an active cluster is toggled off at runtime  
   **When** the toggle is confirmed  
   **Then** new order intents for that cluster are blocked without service redeploy.
4. **UAC-1 Failure handling:** Invalid policy payloads, unknown cluster identifiers, malformed units, and persistence/integration failures return explicit machine-readable errors with no unsafe side effects.
5. **UAC-2 Boundary behavior:** Eligibility boundaries are deterministic and test-covered for threshold edges (including `spread == 0`, `max_exposure_pct_nav == 100`, and invalid `> 100` rejection), plus toggled-on/off cluster state transitions.
6. **UAC-3 Verifiable evidence:** Successful and failed policy updates/toggle decisions emit timestamped telemetry evidence with actor, action, reason code, and correlation metadata suitable for incident and QA traceability.
7. **Schema/dependency/traceability contract:** Story depends on `1.6`, introduces only `market_policy_profiles` and `market_cluster_overrides`, and maps explicitly to `FR1`, `FR4`, and `NFR12`.

## Tasks / Subtasks

- [x] **Task 1: Define canonical market-universe policy domain contracts** (AC: 1, 2, 4, 5, 7)
  - [x] Expand `crates/domain/src/risk.rs` with typed policy profile models for liquidity/spread/reward/exposure thresholds and cluster enablement state.
  - [x] Add deterministic validation helpers for threshold boundary rules and explicit machine-readable reason/error codes.
  - [x] Add a `MarketEligibilityDecision` contract that captures `tradable`/`blocked` outcome plus structured reason codes for downstream risk/execution usage.
  - [x] Preserve existing serialization and naming conventions (`snake_case`, explicit UTC timestamp fields where applicable).
- [x] **Task 2: Add forward-only persistence migration for policy profiles and cluster overrides** (AC: 2, 3, 5, 7)
  - [x] Add a migration under `crates/persistence/migrations/` introducing only `market_policy_profiles` and `market_cluster_overrides`.
  - [x] Enforce boundary constraints in schema checks (non-negative liquidity/depth/spread thresholds and `max_exposure_pct_nav` bounded to `[0, 100]`).
  - [x] Add indexes supporting active profile lookup and runtime cluster state resolution.
  - [x] Ensure migration scope does not introduce Story 2.2+ schema entities.
- [x] **Task 3: Implement policy persistence adapters** (AC: 1, 2, 3, 4, 7)
  - [x] Add `crates/persistence/src/postgres/market_policy.rs` and wire it via `crates/persistence/src/postgres/mod.rs`.
  - [x] Implement repository methods for upsert/update of policy thresholds, cluster toggle updates, and read APIs for evaluation paths.
  - [x] Ensure validation and writes are atomic so invalid updates cannot partially persist.
  - [x] Return explicit machine-readable persistence errors (no silent fallback behavior).
- [x] **Task 4: Implement execution-side market policy evaluation engine** (AC: 1, 4, 5, 6)
  - [x] Replace `services/execution-engine/src/ingestion/mod.rs` placeholder with ingestion-policy module wiring.
  - [x] Implement a policy evaluator that classifies market tradability from market snapshot metrics against configured thresholds.
  - [x] Emit structured telemetry for allow/deny decisions with reason codes and correlation metadata.
  - [x] Enforce fail-closed behavior when policy state or required market inputs are unavailable/ambiguous.
- [x] **Task 5: Wire runtime cluster toggle enforcement for new order intents** (AC: 3, 4, 5, 6)
  - [x] Add a runtime policy-state access seam so cluster enable/disable changes are visible without process restart.
  - [x] Integrate cluster-state checks into the order-intent gate seam used by execution/risk orchestration (block new intents when cluster is disabled).
  - [x] Ensure toggle decisions are auditable and include deterministic reason codes for blocked intents.
- [x] **Task 6: Expose authenticated control-plane policy management endpoints** (AC: 2, 3, 4, 6)
  - [x] Extend `services/control-api/src/routes/mod.rs` with authenticated endpoints for policy profile updates and cluster toggles.
  - [x] Reuse existing middleware/authz guardrails (`require_authenticated_actor` + authorization evaluator) and machine-readable error envelope conventions.
  - [x] Return field-level validation errors for invalid thresholds and explicit success evidence for accepted runtime toggles.
  - [x] Keep privileged endpoint behavior fail-closed and auditable consistent with Epic 1 patterns.
- [x] **Task 7: Add deterministic coverage for classification, validation, and runtime toggles** (AC: 1, 2, 3, 4, 5, 6)
  - [x] Add domain unit tests for threshold parsing/validation and boundary edge outcomes.
  - [x] Add persistence tests for schema constraints and atomic non-partial-failure behavior.
  - [x] Add execution/risk integration tests proving eligible vs blocked classification and runtime cluster toggle effect on new intent eligibility.
  - [x] Add control-api route tests for unauthorized access, invalid policy payload rejection, and successful runtime toggle without redeploy semantics.
- [x] **Task 8: Document operational policy semantics and limits** (AC: 2, 3, 6, 7)
  - [x] Add/update operations documentation with policy field units, valid ranges, and cluster-toggle runbook guidance.
  - [x] Document failure modes and operator remediation paths for invalid configs and disabled-cluster intent denials.

## Dev Notes

### Technical Requirements

- Story dependency is strict: `2.1` depends on `1.6` baseline governance/auth controls and has no forward dependencies.
- Story scope is explicit and narrow:
  - Functional scope: market-universe policy thresholds + runtime market-cluster enable/disable.
  - Schema scope: `market_policy_profiles`, `market_cluster_overrides` only.
  - Traceability scope: `FR1`, `FR4`, `NFR12`.
- Preserve all three BDD scenarios as implemented behavior (classification, invalid-boundary rejection, runtime toggle effect).
- Enforce universal story standards: explicit failure handling, deterministic threshold boundaries, and verifiable telemetry evidence.
- **Out of scope for Story 2.1:** market stream ingestion latency SLA implementation (Story 2.2), authenticated user stream ordering (Story 2.3), stale-feed pause logic (Story 2.4), and full order lifecycle persistence (Story 2.5).

[Source: _bmad-output/planning-artifacts/epics.md#Story 2.1: Configure Market Universe Policy Engine]  
[Source: _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)]  
[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: _bmad-output/planning-artifacts/prd.md#Market Universe & Data Intake]

### Architecture Compliance

- Keep architecture boundaries explicit:
  - `execution-engine` ingestion owns market-policy evaluation logic for FR1-FR5 surfaces.
  - `risk-engine` remains owner of trading-eligibility decisions; do not bypass risk gating contracts when wiring intent checks.
  - `control-api` remains authenticated command boundary for operator-driven policy updates/toggles.
- Follow architecture naming/format conventions (`snake_case` for Rust modules and DB names, machine-readable error codes, UTC timestamps).
- Preserve safety-first behavior: ambiguous or unavailable policy state must fail closed (not tradable) rather than allowing unsafe execution.
- Keep event and telemetry contracts consistent with versioned event naming and correlation metadata patterns.

[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: _bmad-output/planning-artifacts/architecture.md#Architectural Boundaries]  
[Source: _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules]  
[Source: _bmad-output/planning-artifacts/architecture.md#Technical Constraints & Dependencies]

### Library & Framework Requirements

- Continue workspace-pinned stack for implementation consistency:
  - `polymarket-client-sdk = 0.4.4` (`clob`, `ws`)
  - `axum = 0.8.8`
  - `sqlx = 0.8.6`
  - `tokio = 1.48.0`
  - `time = 0.3.44`
- Latest-version checks at story creation time:
  - `polymarket-client-sdk` latest stable `0.4.4` (matches workspace)
  - `axum` latest stable `0.8.8` (matches workspace)
  - `sqlx` latest stable `0.8.6` (workspace aligned; 0.9.0-alpha.1 exists but is pre-release)
  - `tokio` latest stable `1.51.0` (workspace pinned to `1.48.0`)
  - `time` latest stable `0.3.47` (workspace pinned to `0.3.44`)
- Do not introduce opportunistic dependency upgrades in this story; prioritize compatibility with existing Epic 1 code patterns.

[Source: Cargo.toml]  
[Source: https://crates.io/crates/polymarket-client-sdk]  
[Source: https://crates.io/crates/axum]  
[Source: https://crates.io/crates/sqlx]  
[Source: https://crates.io/crates/tokio]  
[Source: https://crates.io/crates/time]

### File Structure Requirements

- Primary implementation surfaces:
  - `crates/domain/src/risk.rs`
  - `crates/persistence/migrations/*market_policy*.sql`
  - `crates/persistence/src/postgres/{mod.rs,market_policy.rs}`
  - `services/execution-engine/src/ingestion/*`
  - `services/risk-engine/src/gates/*` (intent-gate seam integration only)
  - `services/control-api/src/{routes/mod.rs,middleware/mod.rs,main.rs}`
  - `docs/operations/*market*` or equivalent runbook location
- Replace Story 2 placeholder modules in execution/risk incrementally; do not create unrelated top-level directories.
- Maintain strict schema scope to avoid leaking Story 2.2+ table creation into this story.

[Source: _bmad-output/planning-artifacts/architecture.md#Complete Project Directory Structure]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: services/execution-engine/src/ingestion/mod.rs]  
[Source: services/risk-engine/src/gates/mod.rs]  
[Source: crates/persistence/migrations/20260405103000_credential_rotation_events.sql]

### Testing Requirements

- Add deterministic tests that cover:
  - Eligible market classification (all thresholds met -> tradable).
  - Invalid boundary rejection with field-level machine-readable errors and no partial writes.
  - Runtime cluster toggle off -> new order intents blocked immediately without redeploy.
  - Boundary edges (`spread == 0`, `max_exposure_pct_nav == 100` accepted; `> 100` denied).
  - Fail-closed behavior when policy profile/cluster state is missing or unreadable.
- Keep testing patterns consistent with Epic 1 practice:
  - Focused unit tests in domain/services modules.
  - Persistence constraint tests for migrations/adapters.
  - Control-api route-level tests for error envelopes and authz enforcement.
- Preserve existing quality-gate compatibility (`npm run ci:rust`, `npm run ci:security`, `npm test`).

[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: package.json]  
[Source: _bmad-output/implementation-artifacts/stories/1-6-add-scheduled-and-emergency-credential-rotation-flows.md#Testing Requirements]

### Previous Story Intelligence

- Epic 1 established non-negotiable implementation patterns that should carry into Story 2.1:
  - Explicit machine-readable error codes and structured deny payloads.
  - Fail-closed behavior for ambiguous/invalid privileged operations.
  - Strict schema-per-story discipline with forward-only migrations and check constraints.
  - RFC3339 UTC evidence fields and correlation-aware telemetry/audit linkage.
- Reuse existing control-api auth/authz middleware pattern; do not add unauthenticated policy mutation pathways.
- Maintain additive changes that preserve existing governance/audit behaviors while introducing market-policy capabilities.

[Source: _bmad-output/implementation-artifacts/stories/1-3-add-authenticated-control-middleware.md#Technical Requirements]  
[Source: _bmad-output/implementation-artifacts/stories/1-4-implement-immutable-privileged-audit-logging.md#Technical Requirements]  
[Source: _bmad-output/implementation-artifacts/stories/1-5-enforce-dual-approval-governance-for-critical-mutations.md#Technical Requirements]  
[Source: _bmad-output/implementation-artifacts/stories/1-6-add-scheduled-and-emergency-credential-rotation-flows.md#Previous Story Intelligence]

### Git Intelligence Summary

- Recent commits show a consistent implementation style:
  - Domain-first contracts in `crates/domain`.
  - Persistence changes as tightly-scoped migrations/adapters in `crates/persistence`.
  - Authenticated route behavior and deterministic error handling in `services/control-api`.
  - Governance-owned orchestration in `services/governance-service`.
- Apply the same style for Story 2.1: explicit contracts, deterministic boundaries, and scope-contained migrations.

[Source: git --no-pager log --oneline -5]  
[Source: git --no-pager log -5 --name-only --pretty=format:'%h %s']

### Latest Technical Information

- Latest crate checks confirm architecture-selected stack remains valid for this story:
  - `polymarket-client-sdk`: `0.4.4`
  - `axum`: `0.8.8`
  - `sqlx`: `0.8.6` stable (`0.9.0-alpha.1` pre-release exists)
  - `tokio`: `1.51.0` latest stable (workspace pinned lower)
  - `time`: `0.3.47` latest stable (workspace pinned lower)
- Story 2.1 should prefer workspace consistency over incidental dependency churn.

[Source: Cargo.toml]  
[Source: https://crates.io/crates/polymarket-client-sdk]  
[Source: https://crates.io/crates/axum]  
[Source: https://crates.io/crates/sqlx]  
[Source: https://crates.io/crates/tokio]  
[Source: https://crates.io/crates/time]

### Project Context Reference

- No `project-context.md` file was found in repository scope during discovery.
- Context for this story was derived from epics, PRD, architecture, UX specification, implementation-readiness report, previous story artifacts, and current codebase placeholders.

### Project Structure Notes

- `services/execution-engine/src/ingestion/mod.rs`, `services/execution-engine/src/orders/mod.rs`, `services/execution-engine/src/reconciliation/mod.rs`, and risk-engine gate modules are still Story 2 placeholders; Story 2.1 should begin replacing ingestion/gate placeholders with policy-engine foundations.
- Keep Story 2.1 implementation constrained to policy configuration and eligibility classification/toggle behavior; avoid pre-implementing stream-ingestion throughput logic from Story 2.2.
- Ensure runtime cluster toggles are reflected through service state without restart semantics, but do not broaden into full incident/recovery workflows (Stories 2.4+ and 3.x).

[Source: services/execution-engine/src/ingestion/mod.rs]  
[Source: services/execution-engine/src/orders/mod.rs]  
[Source: services/risk-engine/src/gates/mod.rs]  
[Source: _bmad-output/planning-artifacts/epics.md#Story 2.2: Ingest Market Stream with Latency Guarantees]

### References

- _bmad-output/planning-artifacts/epics.md#Epic 2: Live Market Connectivity & Safe Core Execution  
- _bmad-output/planning-artifacts/epics.md#Story 2.1: Configure Market Universe Policy Engine  
- _bmad-output/planning-artifacts/epics.md#Story 2.2: Ingest Market Stream with Latency Guarantees  
- _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)  
- _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)  
- _bmad-output/planning-artifacts/prd.md#Market Universe & Data Intake  
- _bmad-output/planning-artifacts/prd.md#Technical Constraints  
- _bmad-output/planning-artifacts/prd.md#Integration Requirements  
- _bmad-output/planning-artifacts/prd.md#Non-Functional Requirements  
- _bmad-output/planning-artifacts/architecture.md#Technical Constraints & Dependencies  
- _bmad-output/planning-artifacts/architecture.md#Core Architectural Decisions  
- _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules  
- _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries  
- _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping  
- _bmad-output/planning-artifacts/ux-design-specification.md#User Journey Flows  
- _bmad-output/planning-artifacts/implementation-readiness-report-2026-04-05.md  
- _bmad-output/implementation-artifacts/stories/1-3-add-authenticated-control-middleware.md  
- _bmad-output/implementation-artifacts/stories/1-4-implement-immutable-privileged-audit-logging.md  
- _bmad-output/implementation-artifacts/stories/1-5-enforce-dual-approval-governance-for-critical-mutations.md  
- _bmad-output/implementation-artifacts/stories/1-6-add-scheduled-and-emergency-credential-rotation-flows.md  
- services/execution-engine/src/ingestion/mod.rs  
- services/risk-engine/src/gates/mod.rs  
- Cargo.toml  

## Story Completion Status

- Story context generated with exhaustive artifact analysis (workflow inputs, epic/PRD/architecture/UX, implementation-readiness report, previous stories, git history, and current codebase surfaces).
- Story file is created and ready for implementation by dev agents.
- Completion note: Ultimate context engine analysis completed - comprehensive developer guide created.

## Dev Agent Record

### Agent Model Used

GPT-5.3-Codex (gpt-5.3-codex)

### Debug Log References

- BMAD create-story workflow execution (automated)
- Sprint backlog discovery from `_bmad-output/implementation-artifacts/sprint-status.yaml`
- Artifact discovery across planning and implementation surfaces
- Crate-version verification via crates.io API responses
- Story 2.1 implementation: domain/persistence/service/route/risk wiring
- Targeted story validation: package-level Rust tests for domain, persistence, governance-service, execution-engine, risk-engine, and control-api
- Full quality gates: `npm run ci:rust`, `npm run ci:security`, and `npm test`
- QA automation execution (bmad-qa-generate-e2e-tests): `npm run qa:test:story-2-1`

### Completion Notes List

- Selected first backlog story: `2-1-configure-market-universe-policy-engine`.
- Captured story-local BDD acceptance criteria plus universal failure/boundary/evidence contracts.
- Added implementation guardrails for schema scope, architecture boundaries, and runtime toggle behavior.
- Carried forward proven Epic 1 conventions (machine-readable errors, fail-closed behavior, strict migration scope, and test-first boundary coverage).
- Implemented canonical market policy contracts (`MarketPolicyProfile`, `MarketClusterOverride`, `MarketSnapshot`, `MarketEligibilityDecision`) with deterministic threshold validation and machine-readable field errors.
- Added forward-only migration introducing only `market_policy_profiles` and `market_cluster_overrides` with range/check constraints and active/runtime indexes.
- Added persistence adapter `postgres::market_policy` and governance-service `market_policy` orchestrator with explicit persistence error mapping and non-partial invalid-update behavior.
- Replaced execution-engine ingestion placeholder with policy evaluator + telemetry and added risk-engine runtime intent-gate seam enforcing cluster toggles without redeploy.
- Extended control-api with authenticated market policy profile/toggle endpoints, audit linkage, field-error envelopes, and deterministic status-code mapping.
- Added deterministic test coverage across domain/persistence/governance-service/execution-engine/risk-engine/control-api for boundaries, fail-closed paths, and runtime toggles.
- Added operations and governance documentation for policy field units/ranges, toggle runbook, and remediation guidance for invalid config and disabled-cluster denials.
- Adversarial review patches eliminated profile-id collision risk and non-canonical cluster override state drift in policy persistence/runtime paths.
- Adversarial review patches hardened telemetry coverage for denied validation/authz paths and scoped high-severity security signals to unauthorized-role failures only.
- Review cross-check found `.scripts/bmad-auto/copilot/bmad-progress.log` changed in git state; it was excluded from application-source review scope per workflow rules.
- Added QA automation API coverage for Story 2.1 control-api market-policy endpoints (accepted path plus `409`/`503`/`500` machine-error mappings) via deterministic orchestrator stubs.
- Added story-scoped QA runner command `qa:test:story-2-1` and refreshed `_bmad-output/implementation-artifacts/tests/test-summary.md` with passing execution evidence.

### Review Findings

- [x] [Review][Patch] Prevented cross-cluster policy overwrite collisions by switching profile identity to `policy::<normalized_cluster_id>` with regression coverage. [services/governance-service/src/market_policy/mod.rs]
- [x] [Review][Patch] Canonicalized cluster identifiers (`trim + lowercase`) on write/load paths, added deterministic override read ordering, and enforced canonical cluster constraints/indexes in migration schema. [crates/persistence/migrations/20260406004500_market_policy_profiles.sql, crates/persistence/src/postgres/market_policy.rs, services/governance-service/src/market_policy/mod.rs]
- [x] [Review][Patch] Emitted market-policy denial telemetry for all validation/authz/lock-failure return paths and added explicit `action` metadata to telemetry payloads for profile/toggle decisions. [services/governance-service/src/market_policy/mod.rs]
- [x] [Review][Patch] Limited high-severity `unauthorized_market_policy_mutation_attempt_v1` signaling to unauthorized-role service errors (no false positives for payload/persistence failures). [services/control-api/src/routes/mod.rs]
- [x] [Review][Patch] Normalized whitespace/case handling in eligibility cluster-id matching to prevent false `market_policy_unknown_cluster` denials for logically equivalent identifiers. [crates/domain/src/risk.rs]
- [x] [Review][Dismiss] End-to-end control-api-to-risk-engine process coupling was not introduced in Story 2.1 by design; runtime toggle behavior remains validated at the risk-gate seam with deterministic toggle-transition tests (`order_intent_gate_blocks_new_intents_after_runtime_toggle_without_restart`). [services/risk-engine/src/gates/mod.rs]
- [x] [Review][Dismiss] Sync-over-async runtime-bridge concern in `run_with_runtime` follows an existing governance-service repository pattern and was not escalated as story-specific breakage in this review pass.

### File List

- _bmad-output/implementation-artifacts/stories/2-1-configure-market-universe-policy-engine.md
- _bmad-output/implementation-artifacts/sprint-status.yaml
- _bmad-output/implementation-artifacts/tests/test-summary.md
- Cargo.lock
- package.json
- crates/domain/src/risk.rs
- crates/persistence/migrations/20260406004500_market_policy_profiles.sql
- crates/persistence/src/postgres/mod.rs
- crates/persistence/src/postgres/market_policy.rs
- services/governance-service/src/lib.rs
- services/governance-service/src/market_policy/mod.rs
- services/execution-engine/Cargo.toml
- services/execution-engine/src/ingestion/mod.rs
- services/execution-engine/src/main.rs
- services/risk-engine/Cargo.toml
- services/risk-engine/src/gates/mod.rs
- services/risk-engine/src/main.rs
- services/control-api/src/middleware/mod.rs
- services/control-api/src/main.rs
- services/control-api/src/routes/mod.rs
- docs/operations/market-policy-engine.md
- docs/governance/rbac-role-model.md

### Change Log

- 2026-04-06: Moved Story 2.1 lifecycle from `ready-for-dev` to `in-progress` and implemented full task scope.
- 2026-04-06: Delivered market policy domain contracts, migration + persistence adapter, governance-service orchestration, execution/risk runtime policy enforcement, authenticated control-api endpoints, deterministic tests, and operational/governance documentation.
- 2026-04-06: Completed full repository quality gates (`ci:rust`, `ci:security`, `test`) and set story status to `review`.
- 2026-04-06: Completed adversarial review triage and auto-fixes for profile identity collisions, cluster canonicalization, deterministic override lookup, telemetry/error-signal hardening, and boundary normalization.
- 2026-04-06: Re-ran full quality gates after review patches and moved story status from `review` to `done`.
- 2026-04-06: Executed QA automation workflow for Story 2.1, added market-policy API failure-path tests (`409`/`503`/`500`), introduced `qa:test:story-2-1`, and refreshed test-summary evidence while keeping status `done`.
