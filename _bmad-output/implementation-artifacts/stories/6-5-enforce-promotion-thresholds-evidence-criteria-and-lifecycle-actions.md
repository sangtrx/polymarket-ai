# Story 6.5: Enforce Promotion Thresholds, Evidence Criteria, and Lifecycle Actions

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As a governance approver,  
I want promotion, pause, and retirement actions gated by complete evidence and sign-off,  
so that only qualified alphas reach production.

## Acceptance Criteria

1. **Scenario A - Epic 6 BDD baseline (story-local):**  
   **Given** a promotion or lifecycle action request is submitted  
   **When** thresholds, validation packet completeness, and approvals are evaluated  
   **Then** action is blocked on missing criteria and allowed only when all gates pass  
   **And** FR8, FR11, and FR45 are satisfied.

2. **Scenario B - lifecycle action contract:**  
   **Given** a lifecycle action request  
   **When** the action is parsed  
   **Then** only canonical actions (`promote`, `pause`, `retire`) are accepted  
   **And** unsupported actions are denied with machine-readable reason codes.

3. **Scenario C - FR45 promotion packet completeness:**  
   **Given** a `promote` request  
   **When** evidence is validated  
   **Then** a complete packet is required (`data_quality_report`, `purged_cpcv_results`, `calibration_report`, `counterfactual_replay_summary`)  
   **And** missing artifacts fail closed with explicit missing-field diagnostics.

4. **Scenario D - deterministic threshold evaluation behavior:**  
   **Given** lifecycle threshold definitions and observed candidate metrics  
   **When** threshold checks execute  
   **Then** comparator behavior is deterministic at boundary equality (`lt`, `lte`, `gt`, `gte`)  
   **And** pass/fail outcomes include machine-readable reason codes per threshold.

5. **Scenario E - FR11 missing evidence block path:**  
   **Given** required validation evidence or packet artifacts are missing or unresolved  
   **When** a `promote` decision is evaluated  
   **Then** promotion is denied with conflict-class reason codes  
   **And** no success-shaped fallback decision is emitted.

6. **Scenario F - Story 6.2 + 6.3 seam reuse:**  
   **Given** a lifecycle action request references a candidate and validation run  
   **When** evaluation executes  
   **Then** existing Story 6.2 promotion-stage gate checks and Story 6.3 validation artifacts are reused  
   **And** no duplicate gate/validation evaluation pipelines are introduced.

7. **Scenario G - Story 6.4 continuity for confidence inputs:**  
   **Given** shadow evaluation evidence exists  
   **When** promotion readiness is computed  
   **Then** shadow-readiness context can be consumed through existing Story 6.4 contracts without schema forks  
   **And** this story does not mutate or bypass shadow read-only guarantees.

8. **Scenario H - governed sign-off and approval enforcement:**  
   **Given** a lifecycle action requires governed approval  
   **When** sign-off is validated  
   **Then** actions are denied unless approval evidence is valid for the intended action scope (including `strategy_promotion_override` where applicable)  
   **And** actor/proposer/approver identity constraints remain deterministic and auditable.

9. **Scenario I - persistence scope and schema isolation:**  
   **Given** a lifecycle decision is accepted or denied  
   **When** persistence succeeds  
   **Then** decision evidence is stored only in `promotion_decisions` with actor/correlation/timestamp/approval metadata  
   **And** no unrelated schema entities are introduced in this story.

10. **Scenario J - authenticated control-plane decision surfaces:**  
    **Given** authorized users submit or inspect lifecycle decisions  
    **When** control-plane routes execute  
    **Then** responses follow canonical `data/meta/error` envelope conventions  
    **And** unauthorized/malformed/unavailable paths map to deterministic status codes.

11. **Scenario K - deterministic query and identifier boundaries:**  
    **Given** list/read queries for promotion decisions  
    **When** candidate ids, decision ids, limits, and optional time windows are evaluated  
    **Then** identifier normalization and ordering are deterministic and test-covered  
    **And** invalid boundaries fail with explicit machine-readable field errors.

12. **Scenario L - fail-closed dependency handling:**  
    **Given** approval, gate-policy, validation-evidence, shadow-evidence, or persistence dependencies are unavailable/ambiguous  
    **When** decision orchestration executes  
    **Then** requests fail closed with `*_dependency_unavailable`, `*_state_unavailable`, or `*_persistence_unavailable` families  
    **And** no unsafe lifecycle state transition is applied.

13. **Scenario M - NFR8 and NFR17 evidence continuity:**  
    **Given** allow and deny outcomes for start/read/list lifecycle-decision flows  
    **When** audit/telemetry emit  
    **Then** records include actor, action, candidate, decision id, reason code, approval reference, correlation id, and UTC timestamp  
    **And** evidence is queryable for QA and incident forensics.

14. **Scenario N - downstream story compatibility boundary:**  
    **Given** future stories 6.6-6.9 consume promotion decisions  
    **When** they read decision outputs  
    **Then** contracts expose stable machine-readable lifecycle outcomes and missing-evidence diagnostics  
    **And** Story 6.5 does not pre-implement Story 6.6 replay execution, Story 6.7 health monitoring, Story 6.8 governance card UI, or Story 6.9 deallocation automation.

15. **UAC-1 Failure handling:** Invalid payloads, unauthorized access, unresolved sign-off/approval state, and unavailable dependencies return explicit machine-readable errors with no unsafe side effects.

16. **UAC-2 Boundary behavior:** Threshold comparators, evidence-completeness checks, and list/read query boundaries are deterministic (inclusive/exclusive semantics documented and test-covered).

17. **UAC-3 Verifiable evidence:** Successful and failed lifecycle decisions emit timestamped audit/telemetry evidence suitable for incident and QA traceability.

18. **Schema/dependency/traceability contract:** Story depends on `6.3`; schema scope introduces only `promotion_decisions`; traceability maps to `FR8`, `FR11`, `FR45`, `NFR8`, and `NFR17`.

19. **Scope boundary contract:** Story 6.5 delivers lifecycle decision gating and persistence contracts only; it must not pre-implement Story 6.6 counterfactual replay execution, Story 6.7 live-health thresholds, Story 6.8 governance-card UI, or Story 6.9 automatic deallocation policies.

## Tasks / Subtasks

- [x] **Task 1: Define promotion-decision domain contracts and reason-code taxonomy** (AC: 1, 2, 3, 4, 5, 11, 15, 16, 18)
  - [x] Extend `crates/domain/src/research.rs` with lifecycle action/decision-state models, promotion evidence-packet contracts, and machine-readable reason-code families for allow/deny/unavailable paths.
  - [x] Add deterministic canonicalization and validation helpers for candidate/decision identifiers, threshold payloads, and RFC3339 UTC timestamps.
  - [x] Define explicit evidence-completeness validators for FR45 promotion packets, including required artifact keys and boundary-safe threshold semantics.
  - [x] Keep failure taxonomy fail-closed (no broad success defaults when sign-off, evidence, or dependency state is ambiguous).

- [x] **Task 2: Add forward-only migration and persistence adapter for `promotion_decisions`** (AC: 2, 4, 5, 9, 11, 15, 16, 18)
  - [x] Add migration under `crates/persistence/migrations/` creating only `promotion_decisions` with canonical constraints/indexes and actor/correlation/approval evidence metadata.
  - [x] Add persistence module (recommended: `crates/persistence/src/postgres/promotion_decisions.rs`) and wire it through `crates/persistence/src/postgres/mod.rs`.
  - [x] Implement deterministic upsert/read/list operations keyed by canonical identifiers with explicit query/constraint/decode error classification.
  - [x] Add persistence tests for schema isolation, ordering determinism, and machine-readable failure mapping.

- [x] **Task 3: Implement research-gateway promotion orchestration using existing seams** (AC: 1, 3, 4, 5, 6, 7, 8, 12, 14, 15, 16)
  - [x] Expand `services/research-gateway/src/promotion/` into a concrete orchestrator service (e.g., `promotion/decisions.rs` + `mod.rs` exports) with `start/read/list` decision operations.
  - [x] Reuse `evaluate_promotion_entry_gates(...)` from `promotion/mod.rs` and Story 6.3 validation evidence contracts rather than duplicating gate logic.
  - [x] Add explicit evidence ports for validation artifacts and optional shadow-evaluation context consumption, preserving Story 6.4 read-only boundaries.
  - [x] Enforce complete FR45 packet + threshold + sign-off checks before allowing promotion and emit deterministic deny reasons when any gate fails.

- [x] **Task 4: Integrate governed sign-off verification for lifecycle actions** (AC: 1, 8, 12, 13, 15, 18)
  - [x] Reuse existing critical-approval governance contracts (`CriticalActionId::StrategyPromotionOverride`, approval request state, approval reference) for action paths that require dual approval.
  - [x] Ensure proposer/approver distinctness and approval state validity are enforced before lifecycle state transitions are marked allowed.
  - [x] Keep deny-path behavior explicit for missing, expired, rejected, or unresolved approval evidence.

- [x] **Task 5: Expose authenticated control-plane promotion decision routes and wiring** (AC: 10, 11, 12, 13, 15, 16)
  - [x] Add route family in `services/control-api/src/routes/mod.rs` (recommended: `/control/research/promotion-decisions`, `/control/research/promotion-decisions/{decision_id}`) with canonical `data/meta/error` envelopes.
  - [x] Add payload/query structs and deterministic status mapping (`400/403/409/503/500`) aligned with existing validation/shadow route conventions.
  - [x] Wire new promotion orchestrator into `services/control-api/src/middleware/mod.rs` state and bootstrap in `services/control-api/src/main.rs`.
  - [x] Emit allow/deny audit records and unauthorized security-signal names consistent with existing research route patterns.

- [x] **Task 6: Add deterministic Story 6.5 automation coverage and QA command wiring** (AC: 1-19)
  - [x] Add domain tests for lifecycle action parsing, threshold boundary semantics, evidence packet completeness, and reason-code parsing.
  - [x] Add persistence tests for `promotion_decisions` constraints, deterministic query behavior, and error-code classification.
  - [x] Add research-gateway tests for allow/deny orchestration, gate/approval/evidence dependency handling, and fail-closed unavailable states.
  - [x] Add control-api route tests for authenticated start/read/list contracts, unauthorized-role mapping, malformed payload rejection, and envelope/error continuity.
  - [x] Add story-scoped API/E2E tests (`tests/api/story-6-5*.test.mjs`, `tests/e2e/story-6-5*.test.mjs`) and root script `qa:test:story-6-5` in `package.json`.

- [x] **Task 7: Publish FR8/FR11/FR45 operations guidance and traceability artifacts** (AC: 13, 14, 17, 18)
  - [x] Add `docs/operations/alpha-promotion-lifecycle-governance.md` documenting lifecycle decision rules, evidence-packet requirements, threshold semantics, sign-off workflows, and deny-path playbooks.
  - [x] Cross-link runbooks with `alpha-validation-gate-policies.md`, `alpha-validation-workflow-and-diagnostics.md`, `alpha-shadow-mode-evaluation.md`, and governance approval workflows.
  - [x] Update `_bmad-output/implementation-artifacts/tests/test-summary.md` with Story 6.5 evidence after implementation.

### Review Findings

- [x] [Review][Patch] Allow promotion decision records to retain additional missing-evidence diagnostics while still enforcing FR45 packet gaps [crates/domain/src/research.rs:2091]
- [x] [Review][Patch] Add regression coverage for missing validation-run evidence deny behavior [services/research-gateway/src/promotion/decisions.rs:1625]

## Dev Notes

### Technical Requirements

- Story objective is FR8/FR11/FR45 enforceability: lifecycle decisions (`promote`, `pause`, `retire`) must be governed by deterministic threshold checks, complete evidence packets, and sign-off validation.
- Story-level dependency/scope contract:
  - dependency: `6.3`,
  - schema scope: `promotion_decisions` only,
  - traceability: `FR8`, `FR11`, `FR45`, `NFR8`, `NFR17`.
- Required FR45 promotion packet fields are non-negotiable for allow-path promotion:
  - `data_quality_report`,
  - `purged_cpcv_results`,
  - `calibration_report`,
  - `counterfactual_replay_summary`.
- Reuse existing promotion-stage gate seam from Story 6.2:
  - `evaluate_promotion_entry_gates(...)` in `services/research-gateway/src/promotion/mod.rs`.
- Reuse existing validation evidence contracts from Story 6.3 and shadow-readiness contracts from Story 6.4 where needed; do not fork contracts.
- Out of scope for Story 6.5:
  - automated replay execution (Story 6.6),
  - live health threshold monitoring (Story 6.7),
  - governance card UX implementation (Story 6.8),
  - automatic deallocation/stop-research policy execution (Story 6.9).

[Source: _bmad-output/planning-artifacts/epics.md#Story 6.5: Enforce Promotion Thresholds, Evidence Criteria, and Lifecycle Actions]  
[Source: _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)]  
[Source: _bmad-output/planning-artifacts/prd.md#Model Integrity & Promotion Governance]  
[Source: _bmad-output/planning-artifacts/prd.md#Functional Requirements]  
[Source: services/research-gateway/src/promotion/mod.rs]  
[Source: services/research-gateway/src/validation/workflow_runs.rs]  
[Source: services/research-gateway/src/validation/shadow_mode.rs]

### Architecture Compliance

- Preserve bounded architecture ownership:
  - `control-api` remains authenticated ingress, canonical envelope, and audit surface,
  - `research-gateway` owns lifecycle decision orchestration and threshold/evidence gate application,
  - `governance-service` remains owner of critical approval workflow semantics,
  - `crates/persistence` owns deterministic storage/query behavior.
- Follow architecture conventions exactly:
  - plural kebab-case route resources,
  - snake_case Rust modules/files and DB identifiers,
  - RFC3339 UTC timestamps only,
  - deterministic machine-readable reason-code/status mapping.
- Maintain process guardrails:
  - no swallowed errors,
  - fail closed for unresolved dependencies,
  - auditable command-driven mutations for privileged lifecycle actions.
- Keep read/mutation authorization boundaries explicit:
  - read surfaces follow analytics-read authorization patterns,
  - mutation surfaces follow privileged control authorization with unauthorized security signals.

[Source: _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules]  
[Source: _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: services/control-api/src/middleware/mod.rs]  
[Source: services/control-api/src/main.rs]  
[Source: crates/domain/src/governance.rs#CriticalActionId]

### Library & Framework Requirements

- Keep workspace-pinned dependencies for compatibility:
  - `axum = 0.8.8`
  - `sqlx = 0.8.6`
  - `tokio = 1.48.0`
  - `time = 0.3.44`
  - `polymarket-client-sdk = 0.4.4`
- Latest checks at story creation time:
  - `axum` latest stable remains `0.8.8`,
  - `sqlx` latest indexed is `0.9.0-alpha.1` (pre-release), so stable workspace `0.8.6` remains target,
  - `tokio` latest stable is `1.51.0`,
  - `time` latest stable is `0.3.47`,
  - `polymarket-client-sdk` latest stable remains `0.4.4`.
- Do not perform opportunistic dependency upgrades in Story 6.5.

[Source: Cargo.toml]  
[Source: source "$HOME/.cargo/env" && cargo search axum --limit 1]  
[Source: source "$HOME/.cargo/env" && cargo search sqlx --limit 1]  
[Source: source "$HOME/.cargo/env" && cargo search tokio --limit 1]  
[Source: source "$HOME/.cargo/env" && cargo search time --limit 1]  
[Source: source "$HOME/.cargo/env" && cargo search polymarket-client-sdk --limit 1]

### File Structure Requirements

- Primary implementation surfaces for Story 6.5:
  - `crates/domain/src/{lib.rs,research.rs}`
  - `crates/persistence/migrations/*promotion_decisions*.sql`
  - `crates/persistence/src/postgres/{mod.rs,promotion_decisions.rs}`
  - `services/research-gateway/src/{lib.rs,promotion/mod.rs,promotion/decisions.rs,validation/mod.rs}`
  - `services/control-api/src/{main.rs,middleware/mod.rs,routes/mod.rs}`
  - `docs/operations/alpha-promotion-lifecycle-governance.md`
  - `tests/api/story-6-5*.test.mjs`
  - `tests/e2e/story-6-5*.test.mjs`
  - `package.json`
  - `_bmad-output/implementation-artifacts/tests/test-summary.md`
- Reuse established Epic 6 vertical-slice cadence:
  - domain contracts -> migration -> persistence adapter -> research orchestration -> control-api routes/state wiring -> tests -> runbook.
- Keep schema/story boundaries strict: only `promotion_decisions` in this story.

[Source: _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: services/control-api/src/middleware/mod.rs]  
[Source: services/control-api/src/main.rs]  
[Source: services/research-gateway/src/lib.rs]  
[Source: services/research-gateway/src/promotion/mod.rs]  
[Source: crates/persistence/src/postgres/mod.rs]  
[Source: package.json]

### Testing Requirements

- Add deterministic coverage for:
  - lifecycle action parsing and unsupported-action denial behavior,
  - FR45 packet-completeness gate enforcement and missing-evidence diagnostics,
  - threshold boundary behavior (including equality on `lt/lte/gt/gte`),
  - sign-off/approval-required deny paths,
  - fail-closed behavior for unavailable gate/validation/approval/persistence dependencies,
  - canonical route envelope/status mapping and unauthorized security-signal emission.
- Keep test layering aligned with repository conventions:
  - domain contract tests in `crates/domain`,
  - migration/adapter tests in `crates/persistence`,
  - orchestration tests in `services/research-gateway`,
  - route tests in `services/control-api`,
  - story-scoped API/E2E tests in `tests/api` + `tests/e2e`.
- Add story QA command:
  - `qa:test:story-6-5` in root `package.json`.

[Source: package.json]  
[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: tests/api/story-6-4-shadow-mode-evaluation-api.test.mjs]  
[Source: tests/e2e/story-6-4-shadow-mode-evaluation.e2e.test.mjs]

### Previous Story Intelligence

- Story 6.4 provides reusable read-only evaluation context and deterministic reason-code patterns for candidate-level readiness signals.
- Story 6.3 provides canonical validation-run and artifact contracts needed to prove FR45 evidence prerequisites.
- Story 6.2 already provides a promotion-stage gate-evaluation seam via `evaluate_promotion_entry_gates(...)` and should be reused directly.
- Story 6.1 and prior control-plane work establish consistent machine-readable envelope/error/audit conventions and normalized identifier patterns.
- Reuse, do not reinvent:
  - research route family patterns under `/control/research/...`,
  - `data/meta/error` envelope contracts and service-error status mapping patterns,
  - per-story migration + adapter layering in persistence modules.

[Source: _bmad-output/implementation-artifacts/stories/6-4-add-shadow-mode-evaluation-pipeline.md]  
[Source: _bmad-output/implementation-artifacts/stories/6-3-implement-validation-workflow-and-diagnostics-artifact-store.md]  
[Source: _bmad-output/implementation-artifacts/stories/6-2-configure-leakage-and-data-quality-gate-definitions.md]  
[Source: _bmad-output/implementation-artifacts/stories/6-1-build-alpha-hypothesis-registry-with-required-metadata.md]  
[Source: services/research-gateway/src/promotion/mod.rs]  
[Source: services/control-api/src/routes/mod.rs]

### Git Intelligence Summary

- Recent commit cadence in Epic 6 is consistent and should be reused for Story 6.5:
  1. domain contracts + reason-code taxonomy,  
  2. forward-only migration + persistence adapter,  
  3. research-gateway orchestration with fail-closed dependency handling,  
  4. authenticated control-plane route/state integration and audit continuity,  
  5. story-scoped QA + runbook updates.
- Latest commits confirm continuity across Story 6.1-6.4 slices and the same file-surface pattern required for Story 6.5.

[Source: git --no-pager log --oneline -5]  
[Source: git --no-pager log -5 --name-only --pretty=format:'%h %s']

### Latest Technical Information

- Current workspace stack remains sufficient for Story 6.5 scope; no mandatory upgrades are required.
- `sqlx` latest indexed result remains pre-release (`0.9.0-alpha.1`), so stable workspace `0.8.6` should remain in use.
- Story 6.5 should extend existing Epic 6 seams (validation, shadow, approvals, control API) instead of introducing stack churn.

[Source: Cargo.toml]  
[Source: source "$HOME/.cargo/env" && cargo search sqlx --limit 1]  
[Source: services/research-gateway/src/validation/workflow_runs.rs]  
[Source: services/research-gateway/src/validation/shadow_mode.rs]  
[Source: services/governance-service/src/approvals/mod.rs]

### Project Context Reference

- No `project-context.md` file was found during discovery.
- Story context is derived from epics, PRD, architecture, UX, implementation-readiness, research artifacts, previous Story 6.x implementation details, and recent git history.

### Project Structure Notes

- Current Story 6.5 seams in repository:
  - `services/research-gateway/src/promotion/mod.rs` currently provides only promotion-stage gate helper wiring and no lifecycle decision orchestrator yet.
  - `services/control-api/src/routes/mod.rs` currently exposes research routes for hypotheses, gate policies, validation runs, and shadow evaluations, but no promotion-decision route family.
  - `ControlApiState` currently wires `research_hypothesis_orchestrator`, `research_validation_gate_orchestrator`, `research_validation_workflow_orchestrator`, and `research_shadow_mode_orchestrator`; Story 6.5 needs equivalent promotion-orchestrator wiring.
  - Persistence migrations currently include `validation_gate_policies`, `validation_runs`, `validation_artifacts`, and `shadow_evaluations`; `promotion_decisions` is not yet implemented.
- Governance approval stack already supports `strategy_promotion_override` as a canonical critical action id; Story 6.5 should reuse this workflow instead of inventing a parallel sign-off mechanism.
- No blocking issues prevent create-story output; scope is implementation-ready with explicit boundaries and downstream compatibility guardrails.

[Source: services/research-gateway/src/promotion/mod.rs]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: services/control-api/src/middleware/mod.rs]  
[Source: services/control-api/src/main.rs]  
[Source: crates/persistence/migrations/20260407173000_validation_gate_policies.sql]  
[Source: crates/persistence/migrations/20260407193000_validation_runs_validation_artifacts.sql]  
[Source: crates/persistence/migrations/20260407210000_shadow_evaluations.sql]  
[Source: services/governance-service/src/approvals/mod.rs]  
[Source: crates/domain/src/governance.rs#CriticalActionId]

### References

- _bmad-output/planning-artifacts/epics.md#Epic 6: Research-to-Production Alpha Governance Lifecycle  
- _bmad-output/planning-artifacts/epics.md#Story 6.5: Enforce Promotion Thresholds, Evidence Criteria, and Lifecycle Actions  
- _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)  
- _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)  
- _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)  
- _bmad-output/planning-artifacts/prd.md#Functional Requirements  
- _bmad-output/planning-artifacts/prd.md#Model Integrity & Promotion Governance  
- _bmad-output/planning-artifacts/prd.md#Journey 6 — Research User (Phase 2+): Noor, Quant Research Lead  
- _bmad-output/planning-artifacts/prd.md#Non-Functional Requirements  
- _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules  
- _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries  
- _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping  
- _bmad-output/planning-artifacts/ux-design-specification.md#Journey 3 — Alpha Promotion Governance  
- _bmad-output/planning-artifacts/ux-design-specification.md#Alpha Governance Card  
- _bmad-output/planning-artifacts/implementation-readiness-report-2026-04-05.md  
- _bmad-output/planning-artifacts/research/technical-polymarket-claim-validity-audit-research-2026-04-04.md  
- _bmad-output/planning-artifacts/research/market-rust-polymarket-trading-system-lopez-de-prado-research-2026-04-04.md  
- _bmad-output/implementation-artifacts/stories/6-1-build-alpha-hypothesis-registry-with-required-metadata.md  
- _bmad-output/implementation-artifacts/stories/6-2-configure-leakage-and-data-quality-gate-definitions.md  
- _bmad-output/implementation-artifacts/stories/6-3-implement-validation-workflow-and-diagnostics-artifact-store.md  
- _bmad-output/implementation-artifacts/stories/6-4-add-shadow-mode-evaluation-pipeline.md  
- docs/governance/rbac-role-model.md  
- docs/operations/alpha-validation-gate-policies.md  
- docs/operations/alpha-validation-workflow-and-diagnostics.md  
- docs/operations/alpha-shadow-mode-evaluation.md  
- services/research-gateway/src/{lib.rs,promotion/mod.rs,validation/mod.rs,validation/workflow_runs.rs,validation/shadow_mode.rs}  
- services/control-api/src/{main.rs,middleware/mod.rs,routes/mod.rs}  
- services/governance-service/src/approvals/mod.rs  
- crates/domain/src/{research.rs,governance.rs}  
- crates/persistence/src/postgres/{mod.rs,validation_runs.rs,validation_artifacts.rs,shadow_evaluations.rs}  
- crates/persistence/migrations/{20260407173000_validation_gate_policies.sql,20260407193000_validation_runs_validation_artifacts.sql,20260407210000_shadow_evaluations.sql}  
- Cargo.toml  
- package.json  
- git --no-pager log --oneline -5  
- git --no-pager log -5 --name-only --pretty=format:'%h %s'  
- source "$HOME/.cargo/env" && cargo search axum --limit 1  
- source "$HOME/.cargo/env" && cargo search sqlx --limit 1  
- source "$HOME/.cargo/env" && cargo search tokio --limit 1  
- source "$HOME/.cargo/env" && cargo search time --limit 1  
- source "$HOME/.cargo/env" && cargo search polymarket-client-sdk --limit 1

## Story Completion Status

- Story 6.5 implementation is complete and review is complete.
- End-to-end promotion decision lifecycle governance is wired across domain, persistence, research-gateway, and control-api with deterministic fail-closed behavior.
- Story status advanced to `done` after adversarial review fixes and full test-suite pass.

## Dev Agent Record

### Agent Model Used

GPT-5.3-Codex (gpt-5.3-codex)

### Debug Log References

- `source "$HOME/.cargo/env" && cargo test -p domain research::tests::promotion_decision_`
- `source "$HOME/.cargo/env" && cargo test -p persistence postgres::promotion_decisions::tests::`
- `source "$HOME/.cargo/env" && cargo test -p research-gateway promotion::decisions::tests::`
- `source "$HOME/.cargo/env" && cargo test -p control-api routes::tests::promotion_decision_`
- `node --test tests/api/story-6-5*.test.mjs tests/e2e/story-6-5*.test.mjs`
- `source "$HOME/.cargo/env" && npm run --silent qa:test:story-6-5`
- `source "$HOME/.cargo/env" && npm test`
- `source "$HOME/.cargo/env" && npm run --silent qa:test:story-6-5` (2026-04-07 22:48:47 QA automation rerun after Story 6.5 list critical-flow E2E expansion)

### Completion Notes List

- Implemented Story 6.5 promotion lifecycle-governance contracts end-to-end across domain, persistence, research-gateway orchestration, and control-api start/read/list endpoints.
- Enforced FR45 packet completeness, deterministic threshold comparator semantics, governed sign-off integration (`strategy_promotion_override`), and fail-closed dependency/state/persistence handling.
- Added Story 6.5 QA automation (`qa:test:story-6-5`), route/API/E2E contract checks, and FR8/FR11/FR45 operations runbook coverage with cross-runbook links.
- Expanded Story 6.5 E2E automation for list critical flows, including deterministic list limit/repository contracts with allow-telemetry continuity and fail-closed blank canonical candidate-id rejection behavior.
- Review auto-fix: patched promotion decision validation to require FR45 missing-field inclusion (not strict equality) so additional diagnostics such as `validation_run_id` remain fail-closed and machine-readable; added domain/research-gateway regression coverage.
- Discrepancy note: `.scripts/bmad-auto/copilot/bmad-progress.log` was modified in git status but is automation metadata outside Story 6.5 application-source review scope.

### File List

- crates/domain/src/research.rs
- crates/persistence/migrations/20260407223000_promotion_decisions.sql
- crates/persistence/src/postgres/mod.rs
- crates/persistence/src/postgres/promotion_decisions.rs
- services/research-gateway/src/promotion/mod.rs
- services/research-gateway/src/promotion/decisions.rs
- services/control-api/src/main.rs
- services/control-api/src/middleware/mod.rs
- services/control-api/src/routes/mod.rs
- docs/operations/alpha-promotion-lifecycle-governance.md
- tests/api/story-6-5-promotion-lifecycle-governance-api.test.mjs
- tests/e2e/story-6-5-promotion-lifecycle-governance.e2e.test.mjs
- package.json
- _bmad-output/implementation-artifacts/tests/test-summary.md
- _bmad-output/implementation-artifacts/stories/6-5-enforce-promotion-thresholds-evidence-criteria-and-lifecycle-actions.md
- _bmad-output/implementation-artifacts/sprint-status.yaml

### Change Log

- 2026-04-07: Created Story 6.5 ready-for-dev context via automated create-story workflow execution.
- 2026-04-07: Implemented Story 6.5 promotion lifecycle-governance vertical slice, added FR8/FR11/FR45 runbook + QA automation, and moved status to review after passing `qa:test:story-6-5` and `npm test`.
- 2026-04-07: Completed adversarial code review, auto-fixed promotion missing-evidence validation gap, added regression tests, reran `qa:test:story-6-5` + `npm test`, and moved status to done.
- 2026-04-07: Executed `bmad-qa-generate-e2e-tests` workflow for Story 6.5, expanded E2E list critical-flow assertions, reran `qa:test:story-6-5`, and kept story/sprint status at `done`.
