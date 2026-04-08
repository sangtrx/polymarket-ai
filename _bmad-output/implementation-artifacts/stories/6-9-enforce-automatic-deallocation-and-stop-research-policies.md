# Story 6.9: Enforce Automatic Deallocation and Stop-Research Policies

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As a governance approver,  
I want automatic deallocation and stop-research actions when policy thresholds are breached,  
so that fragile strategies are contained without manual lag.

## Acceptance Criteria

1. **Scenario A - deallocation trigger path (Epic 6 baseline):**  
   **Given** live alpha breaches a configured deallocation threshold  
   **When** the lifecycle policy evaluator runs  
   **Then** a deallocation lifecycle action is triggered, audited, and reflected in lifecycle state.

2. **Scenario B - stop-research criteria path (Epic 6 baseline):**  
   **Given** stop-research criteria are met  
   **When** evaluator runs  
   **Then** the experimental branch is terminated and decision evidence is stored.

3. **Scenario C - authorization and audit failure path (Epic 6 baseline):**  
   **Given** a lifecycle action cannot be authorized or persisted  
   **When** execution fails  
   **Then** action remains unapplied, a critical alert is raised, and failure evidence includes remediation guidance.

4. **Scenario D - FR47 deallocation source-of-truth contract:**  
   **Given** Story 6.7 threshold-breach evidence exists in `alpha_threshold_breaches` for an active alpha  
   **When** deallocation evaluation executes  
   **Then** deallocation decisions are derived from canonical breach evidence plus configured policy thresholds  
   **And** no duplicate threshold-evaluation pipeline is introduced outside Story 6.7 seams.

5. **Scenario E - FR48 stop-research criteria semantics:**  
   **Given** evaluator has current stop-research inputs (`trade_count_30d`, `out_of_sample_sharpe_30d`) and recent promotion outcomes  
   **When** criteria are evaluated  
   **Then** stop-research triggers when **any** criterion is true:  
   - `trade_count_30d < 200`  
   - `out_of_sample_sharpe_30d < 0.2`  
   - `promotion_failure_rate_last_10 > 0.70`  
   **And** triggered criterion key(s) are persisted in machine-readable evidence.

6. **Scenario F - deterministic boundary behavior contract:**  
   **Given** observed values equal threshold boundaries exactly  
   **When** evaluation runs  
   **Then** behavior is deterministic and test-covered:  
   - `trade_count_30d == 200` is allow-path  
   - `out_of_sample_sharpe_30d == 0.2` is allow-path  
   - `promotion_failure_rate_last_10 == 0.70` is allow-path.

7. **Scenario G - Story 6.5 + 6.7 seam reuse contract:**  
   **Given** lifecycle automation requires eligibility context  
   **When** orchestration loads dependencies  
   **Then** Story 6.5 promotion-decision history and Story 6.7 alpha-health breach seams are reused  
   **And** lifecycle automation does not duplicate replay/health/promotion decision engines.

8. **Scenario H - lifecycle-state reflection continuity:**  
   **Given** automatic deallocation action is accepted  
   **When** downstream lifecycle-read models are queried  
   **Then** lifecycle state reflects `deallocated` in existing governance-readiness workflows  
   **And** reflection is implemented through canonical lifecycle contracts (no ad hoc UI-only state patching).

9. **Scenario I - schema isolation and persistence scope:**  
   **Given** lifecycle automation records are persisted  
   **When** migration and persistence adapters are added  
   **Then** schema additions are limited to `alpha_lifecycle_actions` only  
   **And** no unrelated schema entities are introduced in this story.

10. **Scenario J - authenticated control-plane lifecycle-action surfaces:**  
    **Given** authorized users or automation invoke lifecycle-action start/read/list routes  
    **When** control-plane handlers execute  
    **Then** canonical `data/meta/error` envelopes are returned  
    **And** unauthorized/malformed/unavailable outcomes map to deterministic status classes.

11. **Scenario K - deterministic list/read query behavior:**  
    **Given** lifecycle-action list/read queries include ids, limits, and optional windows  
    **When** normalization and boundary checks are applied  
    **Then** ordering and filters are deterministic and test-covered  
    **And** invalid boundaries fail with explicit field-level diagnostics.

12. **Scenario L - NFR9 immutable security-event continuity:**  
    **Given** successful or failed deallocation/stop-research actions  
    **When** audit telemetry is emitted  
    **Then** security/governance events are recorded in append-only logs with machine-readable reason codes within required SLA.

13. **Scenario M - NFR17 governance auditability continuity:**  
    **Given** lifecycle action affects strategy state  
    **When** action is persisted or denied  
    **Then** audit evidence includes actor, action type, parameters, approval status (if present), reason, correlation id, and UTC timestamp.

14. **Scenario N - fail-closed dependency handling:**  
    **Given** required dependencies (promotion history, breach history, persistence, or alerting) are unavailable or ambiguous  
    **When** lifecycle evaluator executes  
    **Then** request fails closed with deterministic `*_dependency_unavailable`/`*_state_unavailable`/`*_persistence_unavailable` reason families  
    **And** no success-shaped fallback applies lifecycle state.

15. **Scenario O - downstream compatibility boundary:**  
    **Given** Story 6.8 governance-readiness and operator runbooks consume lifecycle outcomes  
    **When** Story 6.9 is delivered  
    **Then** machine-readable lifecycle-action outputs are stable for existing consumers  
    **And** Story 6.9 does not redesign Story 6.8 UI composition patterns.

16. **UAC-1 Failure handling:** Invalid input, unauthorized access, and unavailable dependency paths must return explicit machine-readable errors with no unsafe side effects.

17. **UAC-2 Boundary behavior:** Threshold boundaries and time-window filters must have deterministic, documented, and test-covered behavior.

18. **UAC-3 Verifiable evidence:** Successful and failed lifecycle actions must emit timestamped audit/telemetry evidence suitable for incident and QA traceability.

19. **Schema/dependency/traceability contract:** Dependencies are Stories `6.5` and `6.7`; schema scope is `alpha_lifecycle_actions` only; traceability maps to `FR47`, `FR48`, `NFR9`, and `NFR17`.

## Tasks / Subtasks

- [x] **Task 1: Define lifecycle-automation domain contracts and policy-evaluation semantics** (AC: 1, 2, 4, 5, 6, 12, 13, 14, 16, 17, 18, 19)
  - [x] Extend `crates/domain/src/research.rs` with canonical lifecycle-action models (recommended: action type/status/reason-code taxonomy, trigger evidence, stop-research criteria snapshot).
  - [x] Add deterministic evaluators for FR47 and FR48 criteria, including strict boundary semantics and machine-readable validation issues.
  - [x] Add canonical lifecycle action id composition using normalized identifiers + RFC3339 UTC nanosecond epoch.
  - [x] Keep contract validation fail-closed for missing/ambiguous evidence inputs and malformed criteria payloads.

- [x] **Task 2: Add forward-only migration and persistence adapter for `alpha_lifecycle_actions`** (AC: 1, 2, 5, 9, 11, 13, 14, 16, 17, 19)
  - [x] Add migration under `crates/persistence/migrations/` creating only `alpha_lifecycle_actions` with canonical checks (lowercasing, non-empty constraints, UTC timestamp checks, JSON object checks).
  - [x] Add persistence module (recommended: `crates/persistence/src/postgres/alpha_lifecycle_actions.rs`) and export via `crates/persistence/src/postgres/mod.rs`.
  - [x] Implement deterministic upsert/read/list operations with stable ordering and bounded list limits.
  - [x] Add persistence tests for schema isolation, index presence, query determinism, and constraint/error classification.

- [x] **Task 3: Implement research-gateway lifecycle-action orchestrator** (AC: 1, 2, 3, 4, 5, 6, 7, 12, 13, 14, 15)
  - [x] Add `services/research-gateway/src/promotion/lifecycle_actions.rs` and export through `services/research-gateway/src/promotion/mod.rs`.
  - [x] Define orchestrator start/read/list contracts following existing Story 6.x service patterns (`Start*Input`, `Read*Input`, `List*Input`, `*Evidence`).
  - [x] Reuse Story 6.7 breach records and Story 6.5 promotion-decision history to evaluate deallocation and stop-research without duplicating upstream logic.
  - [x] Persist lifecycle action decisions with trigger criterion evidence and deterministic reason codes for allow/deny/fail-closed outcomes.

- [x] **Task 4: Integrate lifecycle-state reflection seam for automatic deallocation** (AC: 1, 4, 8, 13, 15)
  - [x] Ensure successful automatic deallocation propagates to canonical lifecycle-state consumers used by Story 6.8.
  - [x] Implement reflection through existing governed lifecycle seams (for example, canonical retire-path integration) rather than UI-only overrides.
  - [x] Preserve deterministic audit correlation between automated lifecycle action and reflected lifecycle state.

- [x] **Task 5: Expose authenticated control-api lifecycle-action routes and state wiring** (AC: 10, 11, 12, 13, 14, 16, 17)
  - [x] Add route family in `services/control-api/src/routes/mod.rs` (recommended: `POST/GET /control/research/alpha-lifecycle-actions` and `GET /control/research/alpha-lifecycle-actions/{action_id}`).
  - [x] Add payload/query DTOs near existing research route payload definitions and keep canonical envelope serialization (`data/meta/error`) plus field-level error mapping.
  - [x] Wire orchestrator in `services/control-api/src/middleware/mod.rs` (`ControlApiState` + builder methods) and bootstrap in `services/control-api/src/main.rs`.
  - [x] Add deterministic HTTP mapping for invalid payload (`400`), unauthorized role (`403`), conflict/not-found (`409`), unavailable (`503`), and internal failures (`500`).

- [x] **Task 6: Add alerting, audit, and operations runbook continuity** (AC: 3, 12, 13, 14, 15, 18, 19)
  - [x] Reuse existing privileged audit append pattern in control-api response helpers; include lifecycle-action identifiers and trigger criteria in audit parameters.
  - [x] Add/extend alert reason taxonomy if required for critical lifecycle-automation failures.
  - [x] Add `docs/operations/alpha-automatic-deallocation-stop-research.md` documenting trigger rules, boundary semantics, deny-path playbooks, and remediation flows.
  - [x] Cross-link with Story 6.5/6.6/6.7/6.8 runbooks.

- [x] **Task 7: Add deterministic Story 6.9 QA automation and command wiring** (AC: 1-19)
  - [x] Add domain tests for FR47/FR48 evaluator rules and exact-boundary allow-path behavior.
  - [x] Add persistence tests for `alpha_lifecycle_actions` migration and adapter deterministic list/read contracts.
  - [x] Add research-gateway tests for automatic deallocation, stop-research termination, lifecycle-state reflection, and fail-closed dependency handling.
  - [x] Add control-api route tests for lifecycle-action start/read/list success and negative paths.
  - [x] Add story-scoped Node tests (`tests/api/story-6-9*.test.mjs`, `tests/e2e/story-6-9*.test.mjs`) and root script `qa:test:story-6-9` in `package.json`.
  - [x] Update `_bmad-output/implementation-artifacts/tests/test-summary.md` with Story 6.9 evidence after implementation.

### Review Findings

- [x] [Review][Patch] Lifecycle read responses could persist `AuthorizationDenied` audit outcomes for successful reads of denied actions [services/control-api/src/routes/mod.rs:11473] — fixed (read success now always records `Allow` audit outcome).
- [x] [Review][Patch] Constraint-violation persistence errors were not treated as unavailable-class fail-closed conditions [services/research-gateway/src/promotion/lifecycle_actions.rs:1046, services/control-api/src/routes/mod.rs:11728] — fixed (`alpha_lifecycle_action_constraint_violation` now maps to persistence-unavailable + `503` + critical fail-closed signal).
- [x] [Review][Patch] Caller-supplied `promotion_failure_rate_last_10` bypassed canonical FR48 promotion-history seam [services/research-gateway/src/promotion/lifecycle_actions.rs:486] — fixed (caller-supplied field rejected; value is derived from promotion history only).
- [x] [Review][Patch] Stop-research promotion-history lookup had alpha/candidate identifier seam ambiguity [services/research-gateway/src/promotion/lifecycle_actions.rs:688] — fixed (canonical alpha→candidate fallback lookup added and dual-source ambiguity now fails closed with explicit `state_unavailable`).

## Dev Notes

### Technical Requirements

- Story objective is FR47 + FR48 enforceability: automatically apply deallocation/retirement and stop-research actions when governance thresholds are breached.
- Dependency/scope contract:
  - dependencies: Story `6.5` (promotion lifecycle decisions) and Story `6.7` (live-health threshold breach stream),
  - schema scope: `alpha_lifecycle_actions` only,
  - traceability: `FR47`, `FR48`, `NFR9`, `NFR17`.
- FR48 stop-research criteria are explicit and non-negotiable:
  - `trade_count_30d < 200`,
  - `out_of_sample_sharpe_30d < 0.2`,
  - `promotion_failure_rate_last_10 > 70%`.
- Boundary semantics must be deterministic: equality at `200`, `0.2`, and `70%` is allow-path, not trigger-path.
- Lifecycle reflection requirement is mandatory: successful deallocation must be visible to existing lifecycle-read consumers (including Story 6.8 governance readiness workflows).
- Out of scope:
  - redesigning Story 6.8 UI architecture,
  - introducing new replay/health/promotion decision engines instead of reusing Story 6.5/6.7 seams,
  - adding schema outside `alpha_lifecycle_actions`.

[Source: _bmad-output/planning-artifacts/epics.md#Story 6.9: Enforce Automatic Deallocation and Stop-Research Policies]  
[Source: _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)]  
[Source: _bmad-output/planning-artifacts/prd.md#Model Integrity & Promotion Governance]  
[Source: _bmad-output/planning-artifacts/implementation-readiness-report-2026-04-05.md#Functional Requirements Coverage (FR Matrix)]  
[Source: docs/operations/alpha-live-health-monitoring-threshold-breaches.md]  
[Source: docs/operations/alpha-promotion-lifecycle-governance.md]  
[Source: docs/operations/alpha-governance-readiness-card.md]

### Architecture Compliance

- Keep bounded ownership intact:
  - `services/research-gateway` owns lifecycle-action policy orchestration and dependency composition,
  - `services/control-api` owns authenticated ingress, canonical envelopes, and privileged audit append,
  - `crates/persistence` owns deterministic storage/list/read behavior.
- Follow architecture consistency rules:
  - snake_case for Rust files/modules and DB identifiers,
  - plural kebab-case route resources,
  - RFC3339 UTC timestamps and canonical machine-readable reason codes.
- Preserve safety-first behavior:
  - fail closed on ambiguous dependency state or persistence failures,
  - no swallowed errors in governance/control paths,
  - route privileged mutations through governance/audit pathways.

[Source: _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules]  
[Source: _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: services/control-api/src/middleware/mod.rs]  
[Source: services/control-api/src/main.rs]

### Library & Framework Requirements

- Keep workspace-pinned dependencies for compatibility:
  - `axum = 0.8.8`
  - `sqlx = 0.8.6`
  - `tokio = 1.48.0`
  - `time = 0.3.44`
  - `polymarket-client-sdk = 0.4.4`
  - `opentelemetry = 0.31.0`
- Latest checks at story creation time:
  - `axum` latest stable remains `0.8.8`,
  - `sqlx` latest indexed result is `0.9.0-alpha.1` (pre-release), so stable `0.8.6` remains target,
  - `tokio` latest stable is `1.51.0`,
  - `time` latest stable is `0.3.47`,
  - `polymarket-client-sdk` latest stable remains `0.4.4`.
- Do not perform opportunistic dependency upgrades in Story 6.9.

[Source: Cargo.toml]  
[Source: source "$HOME/.cargo/env" && cargo search axum --limit 1]  
[Source: source "$HOME/.cargo/env" && cargo search sqlx --limit 1]  
[Source: source "$HOME/.cargo/env" && cargo search tokio --limit 1]  
[Source: source "$HOME/.cargo/env" && cargo search time --limit 1]  
[Source: source "$HOME/.cargo/env" && cargo search polymarket-client-sdk --limit 1]

### File Structure Requirements

- Primary implementation surfaces for Story 6.9:
  - `crates/domain/src/research.rs`
  - `crates/persistence/migrations/*alpha_lifecycle_actions*.sql`
  - `crates/persistence/src/postgres/{alpha_lifecycle_actions.rs,mod.rs}`
  - `services/research-gateway/src/promotion/{lifecycle_actions.rs,mod.rs,decisions.rs,alpha_health.rs}`
  - `services/control-api/src/{main.rs,middleware/mod.rs,routes/mod.rs}`
  - `docs/operations/alpha-automatic-deallocation-stop-research.md`
  - `tests/api/story-6-9*.test.mjs`
  - `tests/e2e/story-6-9*.test.mjs`
  - `package.json`
  - `_bmad-output/implementation-artifacts/tests/test-summary.md`
- Reuse Epic 6 vertical slice cadence established in Stories 6.5-6.8:
  - domain contracts -> migration -> persistence adapter -> research orchestrator -> control-api routes/state wiring -> tests -> runbook.
- Keep schema boundaries strict: only `alpha_lifecycle_actions` is added in this story.

[Source: _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries]  
[Source: crates/persistence/migrations/20260407223000_promotion_decisions.sql]  
[Source: crates/persistence/migrations/20260408010000_counterfactual_replay_runs.sql]  
[Source: crates/persistence/migrations/20260408023000_alpha_health_metrics_threshold_breaches.sql]  
[Source: crates/persistence/src/postgres/{promotion_decisions.rs,alpha_health_metrics.rs,counterfactual_replay_runs.rs,mod.rs}]  
[Source: services/research-gateway/src/promotion/{mod.rs,decisions.rs,alpha_health.rs}]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: package.json]

### Testing Requirements

- Add deterministic coverage for:
  - FR47 deallocation trigger from canonical breach evidence (including no-trigger paths),
  - FR48 stop-research criteria and threshold boundary equality behavior,
  - lifecycle-state reflection continuity for downstream consumers,
  - fail-closed dependency and persistence failures,
  - canonical route envelope + status mapping + field-level diagnostics.
- Keep test layering aligned with repository conventions:
  - domain tests in `crates/domain`,
  - persistence tests in `crates/persistence`,
  - orchestrator tests in `services/research-gateway`,
  - route tests in `services/control-api`,
  - story API/E2E tests in `tests/api` + `tests/e2e`.
- Add story QA command:
  - `qa:test:story-6-9` in root `package.json`.

[Source: package.json#qa:test:story-6-5]  
[Source: package.json#qa:test:story-6-6]  
[Source: package.json#qa:test:story-6-7]  
[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: services/control-api/src/routes/mod.rs]

### Previous Story Intelligence

- Story 6.5 already defines governed lifecycle actions (`promote`, `pause`, `retire`) and deterministic decision reason-code behavior; Story 6.9 should reuse those lifecycle seams instead of inventing a parallel decision model.
- Story 6.6 enforces replay-gate evidence for promotion and provides machine-readable deny semantics; Story 6.9 should not re-implement replay logic.
- Story 6.7 already persists `alpha_health_metrics` and `alpha_threshold_breaches` and emits incident alerts; Story 6.9 should consume these outputs as deallocation trigger inputs.
- Story 6.8 derives `deallocated` from lifecycle evidence; Story 6.9 must preserve compatibility with that lifecycle-read behavior.
- Existing control-api research route handlers already provide canonical `data/meta/error` envelopes and privileged audit append patterns that should be reused.

[Source: _bmad-output/implementation-artifacts/stories/6-5-enforce-promotion-thresholds-evidence-criteria-and-lifecycle-actions.md]  
[Source: _bmad-output/implementation-artifacts/stories/6-6-integrate-counterfactual-replay-stress-gates.md]  
[Source: _bmad-output/implementation-artifacts/stories/6-7-implement-live-alpha-health-monitoring-and-threshold-detection.md]  
[Source: _bmad-output/implementation-artifacts/stories/6-8-deliver-alpha-governance-readiness-card-ux.md]  
[Source: docs/operations/alpha-promotion-lifecycle-governance.md]  
[Source: docs/operations/alpha-live-health-monitoring-threshold-breaches.md]  
[Source: services/control-api/src/routes/mod.rs]

### Git Intelligence Summary

- Recent commit sequence confirms Epic 6 implementation pattern and file-surface continuity:
  1. `f7fb05a` - Story 6.8 governance readiness card UX
  2. `da9c17d` - Story 6.7 live alpha health monitoring
  3. `1e1e758` - Story 6.6 counterfactual replay stress gates
  4. `f0ff1b1` - Story 6.5 promotion thresholds and lifecycle actions
  5. `5df6e5d` - Story 6.4 shadow mode evaluation
- Story 6.9 should follow the same vertical-slice sequencing and avoid structural drift.

[Source: git --no-pager log --oneline -5]  
[Source: git --no-pager log -5 --name-only --pretty=format:'%h %s']

### Latest Technical Information

- Current workspace stack remains sufficient for Story 6.9 scope; no mandatory dependency upgrades are required.
- `sqlx` latest indexed result is pre-release (`0.9.0-alpha.1`), so stable workspace `0.8.6` remains the target.
- Research artifacts reinforce fail-closed risk posture and auto-degrade discipline for deteriorating strategy conditions.

[Source: Cargo.toml]  
[Source: source "$HOME/.cargo/env" && cargo search sqlx --limit 1]  
[Source: _bmad-output/planning-artifacts/research/domain-polymarket-rust-clob-profit-research-2026-04-04.md#Phase 5 — Validation & Overfitting Defense]  
[Source: _bmad-output/planning-artifacts/research/market-rust-polymarket-trading-system-lopez-de-prado-research-2026-04-04.md#Risk Assessment and Mitigation]

### Project Context Reference

- No `project-context.md` file was found during discovery.
- Story context is derived from epics, PRD, architecture, UX, implementation-readiness artifacts, prior Story 6.x artifacts, operations runbooks, and current repository seams.

### Project Structure Notes

- Current repository has no `alpha_lifecycle_actions` migration/table/adapter yet.
- Current research route family includes hypotheses, validation gates/runs, shadow evaluations, alpha health, replay runs, and promotion decisions; no lifecycle-actions route currently exists.
- `ControlApiState` currently wires `research_alpha_health_orchestrator`, `research_counterfactual_replay_orchestrator`, and `research_promotion_decision_orchestrator`; Story 6.9 must add lifecycle-action orchestrator wiring consistently.
- Existing Story 6.8 lifecycle derivation expects canonical lifecycle evidence (`retire` -> `deallocated`) and should remain non-regressive.
- No blocking issues were found for story-context creation; implementation scope is ready with explicit boundaries.

[Source: services/control-api/src/routes/mod.rs]  
[Source: services/control-api/src/middleware/mod.rs]  
[Source: services/control-api/src/main.rs]  
[Source: services/research-gateway/src/promotion/{mod.rs,decisions.rs,alpha_health.rs}]  
[Source: crates/persistence/migrations/20260407223000_promotion_decisions.sql]  
[Source: crates/persistence/migrations/20260408023000_alpha_health_metrics_threshold_breaches.sql]  
[Source: docs/operations/alpha-governance-readiness-card.md]

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Epic 6: Research-to-Production Alpha Governance Lifecycle]
- [Source: _bmad-output/planning-artifacts/epics.md#Story 6.9: Enforce Automatic Deallocation and Stop-Research Policies]
- [Source: _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)]
- [Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]
- [Source: _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)]
- [Source: _bmad-output/planning-artifacts/prd.md#Model Integrity & Promotion Governance]
- [Source: _bmad-output/planning-artifacts/prd.md#Non-Functional Requirements]
- [Source: _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules]
- [Source: _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries]
- [Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]
- [Source: _bmad-output/planning-artifacts/ux-design-specification.md#Alpha Governance Card]
- [Source: _bmad-output/planning-artifacts/implementation-readiness-report-2026-04-05.md]
- [Source: _bmad-output/planning-artifacts/research/domain-polymarket-rust-clob-profit-research-2026-04-04.md]
- [Source: _bmad-output/planning-artifacts/research/market-rust-polymarket-trading-system-lopez-de-prado-research-2026-04-04.md]
- [Source: _bmad-output/planning-artifacts/research/technical-polymarket-claim-validity-audit-research-2026-04-04.md]
- [Source: _bmad-output/implementation-artifacts/stories/6-5-enforce-promotion-thresholds-evidence-criteria-and-lifecycle-actions.md]
- [Source: _bmad-output/implementation-artifacts/stories/6-6-integrate-counterfactual-replay-stress-gates.md]
- [Source: _bmad-output/implementation-artifacts/stories/6-7-implement-live-alpha-health-monitoring-and-threshold-detection.md]
- [Source: _bmad-output/implementation-artifacts/stories/6-8-deliver-alpha-governance-readiness-card-ux.md]
- [Source: docs/operations/alpha-promotion-lifecycle-governance.md]
- [Source: docs/operations/alpha-counterfactual-replay-stress-gating.md]
- [Source: docs/operations/alpha-live-health-monitoring-threshold-breaches.md]
- [Source: docs/operations/alpha-governance-readiness-card.md]
- [Source: services/research-gateway/src/promotion/{mod.rs,decisions.rs,alpha_health.rs}]
- [Source: services/control-api/src/{main.rs,middleware/mod.rs,routes/mod.rs}]
- [Source: crates/domain/src/research.rs]
- [Source: crates/domain/src/alerts.rs]
- [Source: crates/persistence/src/postgres/{mod.rs,promotion_decisions.rs,alpha_health_metrics.rs,counterfactual_replay_runs.rs}]
- [Source: crates/persistence/migrations/{20260407223000_promotion_decisions.sql,20260408010000_counterfactual_replay_runs.sql,20260408023000_alpha_health_metrics_threshold_breaches.sql}]
- [Source: Cargo.toml]
- [Source: package.json]
- [Source: git --no-pager log --oneline -5]
- [Source: git --no-pager log -5 --name-only --pretty=format:'%h %s']
- [Source: source "$HOME/.cargo/env" && cargo search axum --limit 1]
- [Source: source "$HOME/.cargo/env" && cargo search sqlx --limit 1]
- [Source: source "$HOME/.cargo/env" && cargo search tokio --limit 1]
- [Source: source "$HOME/.cargo/env" && cargo search time --limit 1]
- [Source: source "$HOME/.cargo/env" && cargo search polymarket-client-sdk --limit 1]

## Story Completion Status

- Story 6.9 implementation plus latest code-review remediations are complete and regression-validated.
- All HIGH and MEDIUM review findings identified during adversarial review are now remediated in code and regression-covered.
- Full `ci:web`, full `ci:rust`, and story-scoped `qa:test:story-6-9` all pass after the latest remediations.

## Dev Agent Record

### Agent Model Used

GPT-5.3-Codex (gpt-5.3-codex)

### Debug Log References

- `source "$HOME/.cargo/env" && cargo test -p domain research::tests::alpha_lifecycle_ -- --nocapture`
- `source "$HOME/.cargo/env" && cargo test -p persistence postgres::alpha_lifecycle_actions::tests:: -- --nocapture`
- `source "$HOME/.cargo/env" && cargo test -p research-gateway promotion::lifecycle_actions::tests:: -- --nocapture`
- `source "$HOME/.cargo/env" && cargo test -p control-api alpha_lifecycle_action_ -- --nocapture`
- `npm run --silent qa:test:story-6-9`
- `node --test tests/api/story-6-8-alpha-governance-readiness-api.test.mjs tests/e2e/story-6-8-alpha-governance-readiness.e2e.test.mjs`
- `source "$HOME/.cargo/env" && npm run --silent ci:web`
- `source "$HOME/.cargo/env" && npm run --silent ci:rust`
- `source "$HOME/.cargo/env" && node --test tests/api/story-6-8-alpha-governance-readiness-api.test.mjs tests/e2e/story-6-8-alpha-governance-readiness.e2e.test.mjs`
- `source "$HOME/.cargo/env" && npm run --silent qa:test:story-6-9`
- `source "$HOME/.cargo/env" && npm run --silent ci:web && npm run --silent ci:rust`
- `source "$HOME/.cargo/env" && cargo fmt --all`
- `source "$HOME/.cargo/env" && npm run --silent ci:rust`
- `source "$HOME/.cargo/env" && npm run --silent ci:web && npm run --silent qa:test:story-6-9`
- `source "$HOME/.cargo/env" && cargo test -p control-api routes::tests::alpha_lifecycle_action_ -- --nocapture`
- `source "$HOME/.cargo/env" && cargo test -p research-gateway promotion::lifecycle_actions::tests:: -- --nocapture`
- `source "$HOME/.cargo/env" && npm run --silent ci:web && npm run --silent ci:rust && npm run --silent qa:test:story-6-9`
- `source "$HOME/.cargo/env" && cargo fmt --all && npm run --silent ci:web && npm run --silent ci:rust && npm run --silent qa:test:story-6-9`
- `source "$HOME/.cargo/env" && npm run --silent qa:test:story-6-9`

### Completion Notes List

- Added canonical Story 6.9 domain + persistence contracts for lifecycle actions, including deterministic FR47/FR48 evaluation, canonical identifiers, and fail-closed validation paths.
- Implemented research-gateway lifecycle orchestration with Story 6.5 + 6.7 seam reuse, deallocation/stop-research evidence persistence, and deterministic deallocation reflection hints.
- Added authenticated control-api lifecycle-action start/read/list routes with canonical envelopes, deterministic status/error mapping, and unauthorized security-signal continuity.
- Integrated Story 6.8 readiness continuity by consuming lifecycle actions and reflecting latest applied deallocation as canonical `deallocated` lifecycle state.
- Added Story 6.9 operational runbook plus cross-links in Story 6.5/6.6/6.7/6.8 runbooks, and refreshed test-summary evidence.
- Story-scoped QA automation (`qa:test:story-6-9`) passes; `ci:web` and full `ci:rust` now pass with deterministic coverage preserved.
- Code review auto-fixes now enforce stop-research lifecycle reflection, event-time precedence versus newer promotion decisions, and fail-closed alpha-id contract checks for lifecycle-action payloads.
- Control-api lifecycle handling now maps `ActionNotFound` to `409`, emits `critical` unauthorized lifecycle-action security signals, and attaches deterministic remediation guidance for authorization denials.
- Research-gateway lifecycle handling now treats missing stop-research input fields as invalid payload errors, enforces a full 10-decision FR48 promotion window for derived failure-rate paths, and preserves fail-closed behavior when the window is incomplete.
- Domain lifecycle validation now rejects cross-type payload drift (`stop_research_criteria` on non-`stop_research` actions) to prevent contradictory persisted lifecycle evidence.
- Additional regression coverage now validates FR48 incomplete-window fail-closed behavior, list-window ordering/boundary semantics, lifecycle unauthorized-signal severity, payload alpha-id mismatch rejection, lifecycle not-found status mapping, and limit-boundary validation.
- Deferred follow-up (non-blocking): evaluate replacing Story 6.8 compatibility fallback for `404` lifecycle-action reads with strict fail-closed behavior, and evaluate sourcing FR48 `trade_count_30d` / `out_of_sample_sharpe_30d` from canonical telemetry-only dependencies instead of caller-provided payload fields.
- Resolved previously blocking workspace clippy `-D warnings` paths in shared modules (`domain`, `governance-service`, `research-gateway`, `reporting-service`, `control-api`) and re-ran full CI successfully.
- Git reality discrepancy check: non-source workflow artifacts changed outside the story File List (`.scripts/bmad-auto/copilot/bmad-progress.log`, `_bmad-output/implementation-artifacts/sprint-status.yaml`) while application-source changes remain aligned with the listed story implementation files.
- Code-review remediation: lifecycle read success paths now always persist `Allow` privileged-audit outcomes even when the referenced lifecycle action record is `denied`/`unapplied`.
- Code-review remediation: `alpha_lifecycle_action_constraint_violation` now maps to fail-closed persistence-unavailable behavior (`503` + critical security signal) to preserve deterministic unavailable-class handling.
- Code-review remediation: stop-research evaluation now enforces canonical FR48 promotion-failure sourcing by rejecting caller-supplied `promotion_failure_rate_last_10` and deriving from promotion-history seams only.
- Code-review remediation: stop-research promotion-history lookup now supports canonical alpha/candidate identifier seams and fails closed when both lookup domains return conflicting datasets.
- Edge-case review continuity: after long-running automated edge-case pass did not return findings in-session, a manual branch/boundary walk was completed and codified with explicit seam-ambiguity guards plus regression tests.
- Automated QA refresh expanded Story 6.9 API/E2E assertions for read-path conflict/audit continuity and FR48 promotion-history seam fail-closed contracts (candidate lookup support, ambiguity fail-closed, incomplete-window fail-closed).
- Re-ran `qa:test:story-6-9` after test expansion; full story-scoped suite passed with no remediation required.

### Change Log

- 2026-04-09: Executed BMAD QA automation refresh for Story 6.9, expanded API/E2E critical-flow assertions (read-path conflict/audit continuity and FR48 promotion-history fail-closed seams), and re-ran `qa:test:story-6-9` successfully.
- 2026-04-08: Completed adversarial review triage and applied medium/high code-path remediations across readiness, domain validation, lifecycle orchestration, and control-api status/signal mapping; documented deferred architecture-level concerns.
- 2026-04-08: Applied additional review remediations for lifecycle read-audit outcome classification, fail-closed constraint-violation status/signal mapping, canonical FR48 promotion-failure-rate sourcing, and alpha/candidate promotion-history seam ambiguity handling; promoted story to `done`.

### File List

- crates/domain/src/research.rs
- crates/domain/src/recovery.rs
- crates/persistence/migrations/20260408033000_alpha_lifecycle_actions.sql
- crates/persistence/src/postgres/alpha_lifecycle_actions.rs
- crates/persistence/src/postgres/alpha_health_metrics.rs
- crates/persistence/src/postgres/mod.rs
- services/research-gateway/src/promotion/lifecycle_actions.rs
- services/research-gateway/src/promotion/mod.rs
- services/research-gateway/src/promotion/alpha_health.rs
- services/research-gateway/src/promotion/counterfactual_replay.rs
- services/research-gateway/src/validation/workflow_runs.rs
- services/governance-service/src/recovery/mod.rs
- services/reporting-service/src/contracts/lifecycle.rs
- services/reporting-service/src/exports/scheduling.rs
- services/reporting-service/src/exports/workflows.rs
- services/control-api/src/middleware/mod.rs
- services/control-api/src/main.rs
- services/control-api/src/routes/mod.rs
- apps/operator-console/src/lib/governance/readiness.ts
- tests/api/story-6-8-alpha-governance-readiness-api.test.mjs
- tests/api/story-6-9-automatic-lifecycle-actions-api.test.mjs
- tests/e2e/story-6-9-automatic-lifecycle-actions.e2e.test.mjs
- docs/operations/alpha-automatic-deallocation-stop-research.md
- docs/operations/alpha-governance-readiness-card.md
- docs/operations/alpha-promotion-lifecycle-governance.md
- docs/operations/alpha-counterfactual-replay-stress-gating.md
- docs/operations/alpha-live-health-monitoring-threshold-breaches.md
- package.json
- _bmad-output/implementation-artifacts/tests/test-summary.md
- _bmad-output/implementation-artifacts/stories/6-9-enforce-automatic-deallocation-and-stop-research-policies.md
