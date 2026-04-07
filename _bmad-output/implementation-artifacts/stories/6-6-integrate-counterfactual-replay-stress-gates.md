# Story 6.6: Integrate Counterfactual Replay Stress Gates

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As a research user,  
I want baseline and stressed replay scenarios evaluated automatically,  
so that fragile alphas are blocked before live rollout.

## Acceptance Criteria

1. **Scenario A - Epic 6 BDD baseline (story-local):**  
   **Given** a promotion candidate reaches final review  
   **When** counterfactual replay scenarios execute  
   **Then** promotion is blocked when stressed outcome violates configured tolerance  
   **And** replay gating behavior satisfies FR46.

2. **Scenario B - FR46 scenario completeness contract:**  
   **Given** a replay run is started for a promotion candidate  
   **When** simulation executes  
   **Then** all required scenarios are evaluated and persisted for one replay run id: `baseline`, `stressed_execution` (2x slippage and 50% reduced fill rate), and `delayed_exit` (60-second delay)  
   **And** each scenario emits machine-readable outcome metadata.

3. **Scenario C - stress-gate tolerance enforcement:**  
   **Given** baseline and stressed net PnL are available  
   **When** stress-gate evaluation runs  
   **Then** replay gate outcome is deterministic using this formula: `degradation_pct = ((stressed_net_pnl - baseline_net_pnl) / baseline_net_pnl) * 100`  
   **And** promotion is denied when `degradation_pct < -5.0` per FR46.

4. **Scenario D - deterministic boundary behavior:**  
   **Given** stressed degradation equals the tolerance boundary exactly (-5.0%)  
   **When** replay gate evaluation runs  
   **Then** boundary behavior is explicit and test-covered (`degradation_pct == -5.0` is allow-path, not deny)  
   **And** no ambiguous boundary outcome is possible.

5. **Scenario E - baseline admissibility safety rule:**  
   **Given** baseline net PnL is missing, non-finite, or `<= 0` (invalid denominator for FR46 ratio semantics)  
   **When** stress-gate evaluation runs  
   **Then** the replay gate fails closed with explicit machine-readable diagnostics  
   **And** promotion does not proceed on undefined tolerance math.

6. **Scenario F - Story 6.3 + 6.4 seam reuse contract:**  
   **Given** replay execution needs upstream candidate evidence  
   **When** orchestration loads dependencies  
   **Then** Story 6.3 validation runs/artifacts and Story 6.4 shadow evaluation seams are reused  
   **And** no duplicate validation or shadow pipelines are introduced.

7. **Scenario G - Story 6.5 deferred-placeholder replacement:**  
   **Given** Story 6.5 currently allows `counterfactual_replay_summary` placeholders marked `deferred_to_story_6_6`  
   **When** Story 6.6 replay integration is active  
   **Then** canonical replay summary evidence is produced and used for promote decisions  
   **And** deferred placeholder values are rejected for allow-path promotion.

8. **Scenario H - promote decision integration path:**  
   **Given** a `promote` lifecycle decision is evaluated  
   **When** replay evidence is required  
   **Then** promotion-decision orchestration consumes replay-gate outcomes deterministically  
   **And** deny outcomes map to stable reason-code families with fail-closed behavior.

9. **Scenario I - schema isolation and persistence scope:**  
   **Given** replay execution succeeds or fails  
   **When** records persist  
   **Then** persistence is limited to `counterfactual_replay_runs` for this story  
   **And** no unrelated schema entities are introduced.

10. **Scenario J - authenticated control-plane replay surfaces:**  
    **Given** authorized users start/read/list replay runs  
    **When** control-plane routes execute  
    **Then** responses follow canonical `data/meta/error` envelopes  
    **And** unauthorized/malformed/unavailable paths map to deterministic status classes.

11. **Scenario K - deterministic list/read query behavior:**  
    **Given** replay read/list queries with ids, limits, and optional time windows  
    **When** normalization and ordering are applied  
    **Then** outputs are deterministic and boundary-tested  
    **And** invalid query boundaries fail with explicit field-level errors.

12. **Scenario L - NFR14 observability continuity:**  
    **Given** replay start/read/list and integrated promotion decisions  
    **When** allow and deny outcomes occur  
    **Then** structured telemetry includes replay run id, candidate id, action, reason code, correlation id, and UTC timestamps  
    **And** evidence is suitable for incident and QA traceability.

13. **Scenario M - fail-closed dependency handling:**  
    **Given** validation/shadow/persistence dependencies are unavailable or ambiguous  
    **When** replay orchestration executes  
    **Then** requests fail closed with `*_dependency_unavailable`, `*_state_unavailable`, or `*_persistence_unavailable` style outcomes  
    **And** no success-shaped fallback promotes unsafe state transitions.

14. **Scenario N - downstream story compatibility boundary:**  
    **Given** future stories 6.7-6.9 consume replay-governance outputs  
    **When** they read Story 6.6 artifacts  
    **Then** replay contracts expose stable machine-readable outputs and diagnostics  
    **And** Story 6.6 does not pre-implement Story 6.7 live health monitoring, Story 6.8 governance card UX, or Story 6.9 automatic deallocation policies.

15. **UAC-1 Failure handling:** Invalid payloads, unauthorized access, unavailable dependencies, and undefined stress-gate math return explicit machine-readable errors with no unsafe side effects.

16. **UAC-2 Boundary behavior:** Replay tolerance and query-window boundaries are deterministic (inclusive/exclusive semantics documented and test-covered).

17. **UAC-3 Verifiable evidence:** Successful and failed replay-gate operations emit timestamped audit/telemetry evidence suitable for incident and QA traceability.

18. **Schema/dependency/traceability contract:** Story dependency baseline is `6.3`; schema scope introduces only `counterfactual_replay_runs`; traceability maps to `FR46` and `NFR14` (with integration continuity to FR45 evidence contracts from Story 6.5).

## Tasks / Subtasks

- [x] **Task 1: Define counterfactual replay domain contracts and tolerance semantics** (AC: 1, 2, 3, 4, 5, 8, 11, 15, 16, 18)
  - [x] Extend `crates/domain/src/research.rs` with replay-run models, scenario result structures, and reason-code taxonomy for allow/deny/unavailable outcomes.
  - [x] Add deterministic stress-gate helpers that encode FR46 scenario parameters (`2x` slippage, `50%` reduced fill rate, `60s` delayed exit), formula semantics (`degradation_pct = ((stressed - baseline) / baseline) * 100`), and explicit boundary behavior (`< -5.0` denies, `== -5.0` allows).
  - [x] Define canonical replay-summary shape for `counterfactual_replay_summary` evidence and reject deferred placeholders on allow-path promotion.
  - [x] Keep contract validation fail-closed for invalid baseline denominators (`<= 0`, missing, non-finite) and malformed scenario payloads.

- [x] **Task 2: Add forward-only migration and persistence adapter for `counterfactual_replay_runs`** (AC: 2, 5, 9, 11, 13, 15, 16, 18)
  - [x] Add migration under `crates/persistence/migrations/` creating `counterfactual_replay_runs` with canonical constraints/indexes and UTC timestamp checks.
  - [x] Add persistence module (recommended: `crates/persistence/src/postgres/counterfactual_replay_runs.rs`) and wire via `crates/persistence/src/postgres/mod.rs`.
  - [x] Implement deterministic upsert/read/list operations keyed by canonical replay run identifiers and candidate ids.
  - [x] Add persistence tests for schema isolation, ordering determinism, constraint coverage, and error classification.

- [x] **Task 3: Implement research-gateway replay orchestration service** (AC: 1, 2, 3, 4, 5, 6, 11, 12, 13)
  - [x] Add replay orchestration module in `services/research-gateway/src/promotion/` (recommended: `counterfactual_replay.rs` + `mod.rs` exports).
  - [x] Reuse Story 6.3 validation and Story 6.4 shadow seams through explicit ports; avoid duplicate validation/shadow execution logic.
  - [x] Compute and persist baseline/stressed/delayed-exit outcomes with deterministic reason-code classification.
  - [x] Emit replay telemetry with correlation continuity for start/read/list and deny-path conditions.

- [x] **Task 4: Integrate replay gate outcomes into promotion-decision orchestration** (AC: 7, 8, 13, 14, 15)
  - [x] Update `services/research-gateway/src/promotion/decisions.rs` to consume replay results for `promote` actions and enforce FR46 deny behavior.
  - [x] Replace deferred `counterfactual_replay_summary` placeholders with canonical replay summary evidence.
  - [x] Ensure failed/ambiguous replay states deny promotion with machine-readable reason codes and no unsafe fallback.
  - [x] Preserve Story 6.5 approval and threshold contracts while adding replay-gate integration.

- [x] **Task 5: Expose authenticated control-plane replay routes and wiring** (AC: 10, 11, 12, 13, 15, 16)
  - [x] Add route family in `services/control-api/src/routes/mod.rs` (recommended: `/control/research/counterfactual-replay-runs` and `/control/research/counterfactual-replay-runs/{replay_run_id}`) with canonical `data/meta/error` envelopes.
  - [x] Wire replay orchestrator into `services/control-api/src/middleware/mod.rs` and bootstrap in `services/control-api/src/main.rs`.
  - [x] Add deterministic status mapping for invalid payload (`400`), unauthorized (`403`), conflict/not-found (`409`), unavailable (`503`), and internal failures (`500`).
  - [x] Keep unauthorized security-signal patterns aligned with existing research route conventions.

- [x] **Task 6: Add deterministic Story 6.6 automation coverage and QA command wiring** (AC: 1-18)
  - [x] Add domain tests for scenario parameterization, tolerance formula, boundary equality behavior, and replay-summary contract validation.
  - [x] Add persistence tests for replay-run table constraints and deterministic query ordering.
  - [x] Add research-gateway tests for scenario execution, integration with validation/shadow evidence ports, and fail-closed dependency handling.
  - [x] Add control-api route tests for replay start/read/list envelopes, auth gates, and error mapping continuity.
  - [x] Add story-scoped API/E2E tests (`tests/api/story-6-6*.test.mjs`, `tests/e2e/story-6-6*.test.mjs`) and root script `qa:test:story-6-6` in `package.json`.

- [x] **Task 7: Publish FR46 operations guidance and traceability artifacts** (AC: 12, 14, 17, 18)
  - [x] Add `docs/operations/alpha-counterfactual-replay-stress-gating.md` documenting scenario definitions, tolerance semantics, deny playbooks, and runbook triage.
  - [x] Cross-link with `alpha-promotion-lifecycle-governance.md`, `alpha-validation-workflow-and-diagnostics.md`, and `alpha-shadow-mode-evaluation.md`.
  - [x] Update `_bmad-output/implementation-artifacts/tests/test-summary.md` with Story 6.6 evidence once implementation completes.

## Dev Notes

### Technical Requirements

- Story objective is FR46 enforceability: execute and gate counterfactual replay scenarios (`baseline`, `stressed_execution`, `delayed_exit`) before promotion can proceed.
- Story-level dependency/scope contract:
  - dependency baseline: `6.3` (per traceability index),
  - integration continuity required with Story `6.5` promote decision path,
  - schema scope: `counterfactual_replay_runs` only,
  - traceability: `FR46`, `NFR14`.
- FR46 stress profile requirements are non-negotiable:
  - stressed execution applies `2x` slippage and `50%` reduced fill rate,
  - delayed-exit scenario applies `60` seconds delay,
  - degradation is computed as `((stressed_net_pnl - baseline_net_pnl) / baseline_net_pnl) * 100`, valid only when baseline is finite and `> 0`,
  - promotion is blocked only when computed degradation is strictly less than `-5.0` (boundary `-5.0` is allow-path).
- Replace Story 6.5 deferred replay placeholder (`counterfactual_replay_summary.status = deferred_to_story_6_6`) with canonical replay summary evidence.
- Reuse existing promotion gate/decision seam from Story 6.5:
  - `evaluate_promotion_entry_gates(...)` in `services/research-gateway/src/promotion/mod.rs`,
  - `start_promotion_decision(...)` orchestration in `services/research-gateway/src/promotion/decisions.rs`.
- Out of scope for Story 6.6:
  - Story 6.7 live alpha health monitoring,
  - Story 6.8 governance readiness card UX,
  - Story 6.9 automatic deallocation and stop-research policy automation.

[Source: _bmad-output/planning-artifacts/epics.md#Story 6.6: Integrate Counterfactual Replay Stress Gates]  
[Source: _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)]  
[Source: _bmad-output/planning-artifacts/prd.md#Model Integrity & Promotion Governance]  
[Source: services/research-gateway/src/promotion/mod.rs]  
[Source: services/research-gateway/src/promotion/decisions.rs]  
[Source: services/control-api/src/routes/mod.rs]

### Architecture Compliance

- Preserve bounded architecture ownership:
  - `control-api` remains authenticated ingress, envelope/error mapping, and audit/security-signal surface,
  - `research-gateway` owns replay execution orchestration and promotion-gate integration,
  - `crates/persistence` owns deterministic replay-run storage and query behavior.
- Follow architecture conventions exactly:
  - plural kebab-case route resources,
  - snake_case Rust modules/files and DB identifiers,
  - RFC3339 UTC timestamps only,
  - deterministic machine-readable reason-code/status mapping.
- Maintain process guardrails:
  - no swallowed errors,
  - fail closed on ambiguous dependency/state/persistence paths,
  - keep privileged mutation/review flows auditable with correlation continuity.
- Keep Phase 2 model-governance scope localized to `services/research-gateway/src/{validation,promotion}` and avoid cross-boundary duplication.

[Source: _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules]  
[Source: _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: services/control-api/src/main.rs]  
[Source: services/control-api/src/middleware/mod.rs]  
[Source: services/control-api/src/routes/mod.rs]

### Library & Framework Requirements

- Keep workspace-pinned dependencies for compatibility:
  - `axum = 0.8.8`
  - `sqlx = 0.8.6`
  - `tokio = 1.48.0`
  - `time = 0.3.44`
  - `polymarket-client-sdk = 0.4.4`
- Latest checks at story creation time:
  - `axum` latest stable remains `0.8.8`,
  - `sqlx` latest indexed result is `0.9.0-alpha.1` (pre-release); stable `0.8.6` remains target,
  - `tokio` latest stable is `1.51.0`,
  - `time` latest stable is `0.3.47`,
  - `polymarket-client-sdk` latest stable remains `0.4.4`.
- Do not perform opportunistic dependency upgrades in Story 6.6.

[Source: Cargo.toml]  
[Source: source "$HOME/.cargo/env" && cargo search axum --limit 1]  
[Source: source "$HOME/.cargo/env" && cargo search sqlx --limit 1]  
[Source: source "$HOME/.cargo/env" && cargo search tokio --limit 1]  
[Source: source "$HOME/.cargo/env" && cargo search time --limit 1]  
[Source: source "$HOME/.cargo/env" && cargo search polymarket-client-sdk --limit 1]

### File Structure Requirements

- Primary implementation surfaces for Story 6.6:
  - `crates/domain/src/{lib.rs,research.rs}`
  - `crates/persistence/migrations/*counterfactual_replay_runs*.sql`
  - `crates/persistence/src/postgres/{mod.rs,counterfactual_replay_runs.rs}`
  - `services/research-gateway/src/{lib.rs,promotion/mod.rs,promotion/decisions.rs,promotion/counterfactual_replay.rs,validation/mod.rs,validation/shadow_mode.rs}`
  - `services/control-api/src/{main.rs,middleware/mod.rs,routes/mod.rs}`
  - `docs/operations/alpha-counterfactual-replay-stress-gating.md`
  - `tests/api/story-6-6*.test.mjs`
  - `tests/e2e/story-6-6*.test.mjs`
  - `package.json`
  - `_bmad-output/implementation-artifacts/tests/test-summary.md`
- Reuse established Epic 6 vertical-slice cadence:
  - domain contracts -> migration -> persistence adapter -> research orchestration -> control-api routes/state wiring -> tests -> runbook.
- Keep schema/story boundaries strict: only `counterfactual_replay_runs` in this story.

[Source: _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries]  
[Source: crates/persistence/migrations/20260407193000_validation_runs_validation_artifacts.sql]  
[Source: crates/persistence/migrations/20260407210000_shadow_evaluations.sql]  
[Source: crates/persistence/migrations/20260407223000_promotion_decisions.sql]  
[Source: crates/persistence/src/postgres/promotion_decisions.rs]  
[Source: services/research-gateway/src/promotion/decisions.rs]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: package.json]

### Testing Requirements

- Add deterministic coverage for:
  - FR46 scenario generation (`baseline`, `stressed_execution`, `delayed_exit`) and parameter fidelity (`2x` slippage, `50%` reduced fills, `60s` delay),
  - stress tolerance formula (`degradation_pct = ((stressed - baseline) / baseline) * 100`) and boundary behavior (`< -5.0` deny, `== -5.0` allow),
  - invalid-baseline fail-closed behavior (`<= 0`, missing, non-finite),
  - Story 6.5 integration behavior replacing deferred replay summary placeholders,
  - canonical route envelope/status mapping for replay run endpoints,
  - telemetry/audit continuity for allow and deny outcomes.
- Keep test layering aligned with repository conventions:
  - domain contract tests in `crates/domain`,
  - migration/adapter tests in `crates/persistence`,
  - orchestration tests in `services/research-gateway`,
  - route tests in `services/control-api`,
  - story-scoped API/E2E tests in `tests/api` + `tests/e2e`.
- Add story QA command:
  - `qa:test:story-6-6` in root `package.json`.

[Source: package.json]  
[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: tests/api/story-6-5-promotion-lifecycle-governance-api.test.mjs]  
[Source: tests/e2e/story-6-5-promotion-lifecycle-governance.e2e.test.mjs]

### Previous Story Intelligence

- Story 6.5 already enforces FR45 packet completeness and currently requires `counterfactual_replay_summary`, but seeded allow-path fixtures still use `status = deferred_to_story_6_6`.
- Story 6.5 promotion orchestration already centralizes threshold/gate/approval checks and is the correct seam for replay-gate integration.
- Story 6.4 shadow-mode simulation provides deterministic read-only outcome patterns (including simulated slippage fields) that can inform replay scenario construction.
- Story 6.3 provides canonical validation-run/artifact evidence seams required for replay candidate inputs.
- Reuse, do not reinvent:
  - `/control/research/...` route-family conventions,
  - `data/meta/error` envelope contracts and error status mapping helpers,
  - migration + persistence adapter layering used by `validation_runs`, `shadow_evaluations`, and `promotion_decisions`.

[Source: _bmad-output/implementation-artifacts/stories/6-5-enforce-promotion-thresholds-evidence-criteria-and-lifecycle-actions.md]  
[Source: _bmad-output/implementation-artifacts/stories/6-4-add-shadow-mode-evaluation-pipeline.md]  
[Source: _bmad-output/implementation-artifacts/stories/6-3-implement-validation-workflow-and-diagnostics-artifact-store.md]  
[Source: services/research-gateway/src/promotion/decisions.rs]  
[Source: services/research-gateway/src/validation/shadow_mode.rs]  
[Source: services/control-api/src/routes/mod.rs]

### Git Intelligence Summary

- Recent Epic 6 commit cadence should be reused for Story 6.6:
  1. domain contracts + reason-code taxonomy,  
  2. forward-only migration + persistence adapter,  
  3. research-gateway orchestration with fail-closed dependency handling,  
  4. authenticated control-api route/state wiring + audit/security continuity,  
  5. story-scoped QA automation + runbook updates.
- Last five commits confirm the same vertical slices and target file surfaces across Stories 6.1-6.5, so Story 6.6 should preserve those conventions.

[Source: git --no-pager log --oneline -5]  
[Source: git --no-pager log -5 --name-only --pretty=format:'%h %s']

### Latest Technical Information

- Current workspace stack remains sufficient for Story 6.6 scope; no mandatory dependency upgrades are required.
- `sqlx` latest indexed result is still pre-release (`0.9.0-alpha.1`), so stable workspace `0.8.6` remains the implementation target.
- Research artifacts reinforce that stress gating should prioritize net, out-of-sample economic robustness and fail closed when slippage/tail behavior degrades materially.

[Source: Cargo.toml]  
[Source: source "$HOME/.cargo/env" && cargo search sqlx --limit 1]  
[Source: _bmad-output/planning-artifacts/research/domain-polymarket-rust-clob-profit-research-2026-04-04.md#Phase 5 — Validation & Overfitting Defense]  
[Source: _bmad-output/planning-artifacts/research/technical-polymarket-claim-validity-audit-research-2026-04-04.md#Can it be applied to “anything”?]  
[Source: _bmad-output/planning-artifacts/research/market-rust-polymarket-trading-system-lopez-de-prado-research-2026-04-04.md#Risk Assessment and Mitigation]

### Project Context Reference

- No `project-context.md` file was found during discovery.
- Story context is derived from epics, PRD, architecture, UX, implementation-readiness, research artifacts, prior Story 6.x artifacts, and current repository seams.

### Project Structure Notes

- Current repository includes no replay-run schema/module yet (`counterfactual_replay_runs` not present), so Story 6.6 introduces the first dedicated replay persistence layer.
- Promotion decision flow already exists and should be extended in-place for replay-gate integration:
  - `services/research-gateway/src/promotion/decisions.rs` holds start/read/list orchestration,
  - `services/control-api/src/routes/mod.rs` already exposes `/control/research/promotion-decisions...`,
  - `PromotionDecisionService::postgres(...)` is already wired in `services/control-api/src/main.rs`.
- Concrete Story 6.5 seam proving deferred implementation target:
  - `counterfactual_replay_summary.status = deferred_to_story_6_6` appears in promotion decision fixtures/routes/tests and must be replaced with canonical replay evidence.
- No blocking issues were found for create-story output; implementation scope is ready with explicit boundaries.

[Source: services/research-gateway/src/promotion/decisions.rs]  
[Source: services/research-gateway/src/promotion/mod.rs]  
[Source: services/control-api/src/main.rs]  
[Source: services/control-api/src/middleware/mod.rs]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: crates/persistence/migrations/20260407223000_promotion_decisions.sql]  
[Source: crates/domain/src/research.rs]  
[Source: _bmad-output/planning-artifacts/implementation-readiness-report-2026-04-05.md#Overall Readiness Status]

### References

- _bmad-output/planning-artifacts/epics.md#Epic 6: Research-to-Production Alpha Governance Lifecycle  
- _bmad-output/planning-artifacts/epics.md#Story 6.6: Integrate Counterfactual Replay Stress Gates  
- _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)  
- _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)  
- _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)  
- _bmad-output/planning-artifacts/prd.md#Journey 6 — Research User (Phase 2+): Noor, Quant Research Lead  
- _bmad-output/planning-artifacts/prd.md#Model Integrity & Promotion Governance  
- _bmad-output/planning-artifacts/prd.md#Non-Functional Requirements  
- _bmad-output/planning-artifacts/prd.md#Service Components & Isolation  
- _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules  
- _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries  
- _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping  
- _bmad-output/planning-artifacts/ux-design-specification.md#Journey 3 — Alpha Promotion Governance  
- _bmad-output/planning-artifacts/ux-design-specification.md#Alpha Governance Card  
- _bmad-output/planning-artifacts/implementation-readiness-report-2026-04-05.md  
- _bmad-output/planning-artifacts/research/domain-polymarket-rust-clob-profit-research-2026-04-04.md  
- _bmad-output/planning-artifacts/research/technical-polymarket-claim-validity-audit-research-2026-04-04.md  
- _bmad-output/planning-artifacts/research/market-rust-polymarket-trading-system-lopez-de-prado-research-2026-04-04.md  
- _bmad-output/implementation-artifacts/stories/6-3-implement-validation-workflow-and-diagnostics-artifact-store.md  
- _bmad-output/implementation-artifacts/stories/6-4-add-shadow-mode-evaluation-pipeline.md  
- _bmad-output/implementation-artifacts/stories/6-5-enforce-promotion-thresholds-evidence-criteria-and-lifecycle-actions.md  
- docs/operations/alpha-validation-workflow-and-diagnostics.md  
- docs/operations/alpha-shadow-mode-evaluation.md  
- docs/operations/alpha-promotion-lifecycle-governance.md  
- services/research-gateway/src/{lib.rs,promotion/mod.rs,promotion/decisions.rs,validation/mod.rs,validation/shadow_mode.rs}  
- services/control-api/src/{main.rs,middleware/mod.rs,routes/mod.rs}  
- crates/domain/src/research.rs  
- crates/persistence/src/postgres/{mod.rs,validation_runs.rs,shadow_evaluations.rs,promotion_decisions.rs}  
- crates/persistence/migrations/{20260407193000_validation_runs_validation_artifacts.sql,20260407210000_shadow_evaluations.sql,20260407223000_promotion_decisions.sql}  
- package.json  
- git --no-pager log --oneline -5  
- git --no-pager log -5 --name-only --pretty=format:'%h %s'  
- source "$HOME/.cargo/env" && cargo search axum --limit 1  
- source "$HOME/.cargo/env" && cargo search sqlx --limit 1  
- source "$HOME/.cargo/env" && cargo search tokio --limit 1  
- source "$HOME/.cargo/env" && cargo search time --limit 1  
- source "$HOME/.cargo/env" && cargo search polymarket-client-sdk --limit 1

## Story Completion Status

- Story implementation completed and status advanced to `done`.
- FR46 replay scenarios, deterministic gate boundaries, promotion integration, and control-plane replay routes are implemented end-to-end.
- Code review remediations enforced strict FR46 boundary semantics, fail-closed shadow-state prerequisites, canonical replay start/list error envelopes, and replay-run telemetry continuity in promotion decisions.
- Full repository test suite execution (`npm run --silent test`) passed after review fixes.

## Dev Agent Record

### Agent Model Used

GPT-5.3-Codex (gpt-5.3-codex)

### Debug Log References

- `source "$HOME/.cargo/env" && cargo test -p domain --quiet && cargo test -p research-gateway --quiet`
- `source "$HOME/.cargo/env" && cargo test -p control-api routes::tests::counterfactual_replay_ --quiet`
- `source "$HOME/.cargo/env" && cargo test -p control-api routes::tests::promotion_decision_ --quiet`
- `source "$HOME/.cargo/env" && npm run --silent qa:test:story-6-6`
- `source "$HOME/.cargo/env" && npm run --silent qa:test:story-6-5`
- `node --test tests/api/story-6-6*.test.mjs tests/e2e/story-6-6*.test.mjs`
- `source "$HOME/.cargo/env" && cargo test -p domain research::tests::counterfactual_replay_ --quiet`
- `source "$HOME/.cargo/env" && cargo test -p research-gateway promotion::counterfactual_replay::tests::counterfactual_replay_start_boundary_equal_to_negative_five_is_allow_path --quiet`
- `source "$HOME/.cargo/env" && npm run --silent test`
- `source "$HOME/.cargo/env" && node --test tests/api/story-6-6*.test.mjs tests/e2e/story-6-6*.test.mjs && npm run --silent qa:test:story-6-6`

### Completion Notes List

- Implemented FR46 replay contracts, persistence schema/adapter, research-gateway replay orchestration, and promote-path replay-gate integration with fail-closed error mapping.
- Added authenticated control-plane replay start/read/list routes with canonical envelopes, deterministic status mapping, and unauthorized security-signal continuity.
- Published Story 6.6 operations runbook, cross-linked Story 6.3/6.4/6.5 runbooks, and refreshed Story 6.6 QA evidence/test-summary artifacts.
- Code review remediated shadow evaluation lookup completeness by replacing the fixed 20-record scan with a deterministic candidate+validation_run query and completed-state gating.
- Code review remediated FR46 threshold precision drift by normalizing degradation_pct before strict `< -5.0` deny evaluation (while preserving exact `== -5.0` allow behavior).
- Code review remediated replay-route contract drift by returning canonical replay envelopes for start-route authorization denials and malformed list-query rejections.
- Code review remediated promotion telemetry traceability by carrying `replay_run_id` in promotion-decision telemetry events.
- QA automation refresh expanded Story 6.6 API/E2E coverage for replay route negative paths (malformed JSON/query + auth denial envelope), list-window boundary diagnostics, and NFR14 telemetry traceability tuple assertions; Story 6.6 QA rerun remained green.
- Git reality discrepancy noted: `.scripts/bmad-auto/copilot/bmad-progress.log` was modified by automation but intentionally excluded from story file-list review scope.

### File List

- crates/domain/src/research.rs
- crates/persistence/migrations/20260408010000_counterfactual_replay_runs.sql
- crates/persistence/src/postgres/counterfactual_replay_runs.rs
- crates/persistence/src/postgres/mod.rs
- crates/persistence/src/postgres/promotion_decisions.rs
- crates/persistence/src/postgres/shadow_evaluations.rs
- services/research-gateway/src/promotion/mod.rs
- services/research-gateway/src/promotion/counterfactual_replay.rs
- services/research-gateway/src/promotion/decisions.rs
- services/control-api/src/main.rs
- services/control-api/src/middleware/mod.rs
- services/control-api/src/routes/mod.rs
- docs/operations/alpha-counterfactual-replay-stress-gating.md
- docs/operations/alpha-promotion-lifecycle-governance.md
- docs/operations/alpha-validation-workflow-and-diagnostics.md
- docs/operations/alpha-shadow-mode-evaluation.md
- tests/api/story-6-6-counterfactual-replay-stress-gating-api.test.mjs
- tests/e2e/story-6-6-counterfactual-replay-stress-gating.e2e.test.mjs
- tests/e2e/story-6-5-promotion-lifecycle-governance.e2e.test.mjs
- package.json
- _bmad-output/implementation-artifacts/tests/test-summary.md
- _bmad-output/implementation-artifacts/stories/6-6-integrate-counterfactual-replay-stress-gates.md
- _bmad-output/implementation-artifacts/sprint-status.yaml

### Change Log

- 2026-04-07: Created Story 6.6 ready-for-dev context via automated create-story workflow execution.
- 2026-04-07: Validate-story gate remediation clarified FR46 degradation formula, boundary semantics, and invalid-baseline fail-closed rules.
- 2026-04-07: Implemented Story 6.6 FR46 replay orchestration, promote-path replay-gate integration, control-plane replay routes, runbook/docs, and Story 6.6 QA automation wiring/evidence.
- 2026-04-08: Code review remediations fixed shadow evidence lookup/state gating, FR46 boundary precision handling, replay-route auth/query envelope consistency, and promotion telemetry replay_run_id traceability; full suite revalidated.
- 2026-04-08: QA automation refresh added Story 6.6 API/E2E negative-path and telemetry traceability assertions and reran Story 6.6 QA suite (18 story-scoped Node tests passing).
