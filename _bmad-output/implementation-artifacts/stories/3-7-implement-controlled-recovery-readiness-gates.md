# Story 3.7: Implement Controlled Recovery Readiness Gates

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an operator,  
I want guided readiness checks before resuming trading,  
so that post-incident reactivation is safe and policy-compliant.

## Acceptance Criteria

1. **Scenario A — gate-enforced resume path (story-local BDD):**  
   **Given** system is in safe-state after an incident  
   **When** resume is requested  
   **Then** resume is blocked until all readiness gates pass (freshness, reconciliation, risk bundle checksum, operator sign-off).
2. **Scenario B — failed gate behavior (story-local BDD):**  
   **Given** any readiness gate fails  
   **When** recovery evaluation completes  
   **Then** system remains in safe-state and returns machine-readable failing gate reasons.
3. **Scenario C — post-resume evidence (story-local BDD):**  
   **Given** all gates pass and resume is approved  
   **When** trading reactivates  
   **Then** operator sees timestamped verification evidence within 10 seconds.
4. **UAC-1 Failure handling:** Invalid resume payloads, unauthorized recovery commands, unavailable dependency reads (freshness/reconciliation/risk-limit profile state), and malformed checksum/sign-off inputs return explicit machine-readable errors with no unsafe side effects or success-shaped fallback behavior.
5. **UAC-2 Boundary behavior:** Readiness thresholds are deterministic and test-covered: freshness passes only when `<= 30s`, reconciliation passes only when `< 0.1%` (exactly `0.1%` fails readiness), checksum passes only on exact digest equality, and operator sign-off must be explicitly recorded.
6. **UAC-3 Verifiable evidence:** Each readiness evaluation (pass/fail) persists timestamped evidence with `run_id`, `correlation_id`, gate-level outcomes, reason codes, and sign-off metadata suitable for incident and QA traceability.
7. **Schema/dependency/traceability contract:** Story depends only on `2.6`, `2.8`, and `3.6`, creates only `recovery_gate_runs`, and maps explicitly to `FR30`, `NFR6`, `NFR16`, and `UX-DR22`.
8. **NFR6 readiness objective:** Recovery workflow path is designed and measured so 95% of restart incidents restore full trading readiness within 10 minutes once dependency health is restored and operator sign-off is provided.
9. **NFR16 evidence-query objective:** Recovery evidence retrieval paths (latest run, by correlation/run ID, gate failure detail) are queryable within `<= 5s` for representative incident workload.
10. **Scope boundary contract:** Story 3.7 delivers controlled readiness-gate evaluation and resume reactivation workflow only; backup integrity rehearsal remains Story 3.8 and accessibility/reduced-motion hardening remains Story 3.9.

## Tasks / Subtasks

- [x] **Task 1: Define controlled-recovery domain contracts and canonical reason taxonomy** (AC: 1, 2, 3, 4, 5, 6, 7, 8, 9)
  - [x] Add recovery gate contract types in `crates/domain` (new `recovery` module wired through `crates/domain/src/lib.rs`, or explicit bounded extension of `risk.rs`) for request input, per-gate result, run summary, sign-off evidence, and resume verification envelope.
  - [x] Define canonical machine-readable reason codes for gate failures and orchestration errors (freshness stale, reconciliation mismatch, checksum mismatch, sign-off missing, dependency unavailable, unauthorized, invalid payload, resume approved/blocked).
  - [x] Add deterministic checksum helper contract for risk-limit bundle hashing (canonical serialization ordering + digest format) and validation helpers for checksum/sign-off payload fields.
  - [x] Add domain tests for threshold boundaries (`<=30s`, `<0.1%`), exact checksum match semantics, and sign-off-required behavior.

- [x] **Task 2: Add forward-only persistence migration for Story 3.7 schema scope** (AC: 1, 2, 3, 5, 6, 7, 8, 9)
  - [x] Add migration under `crates/persistence/migrations/` creating only `recovery_gate_runs`.
  - [x] Include strict constraints for normalized identifiers, non-empty reason codes, UTC timestamps, deterministic readiness status enum, finite ratio/age fields, and gate evidence payload structure.
  - [x] Persist explicit gate evidence fields needed for NFR16 forensics: freshness age + source timestamp, reconciliation run/mismatch summary, approved/computed checksum values, sign-off actor metadata, failing gate list, and correlation metadata.
  - [x] Add indexes for run lookup (`run_id`), incident correlation (`correlation_id`, `evaluated_at_utc`), latest readiness state (`readiness_status`, `evaluated_at_utc`), and operator/audit lookup (`actor_id`, `evaluated_at_utc`).
  - [x] Keep migration scope strict: do **not** alter existing story schemas (`safety_control_actions`, `pretrade_gate_decisions`, `risk_limit_profiles`, `incident_alerts`, etc.).

- [x] **Task 3: Implement PostgreSQL recovery-gate persistence and read adapters** (AC: 2, 3, 4, 6, 7, 9)
  - [x] Add `crates/persistence/src/postgres/recovery_gate_runs.rs` and wire via `crates/persistence/src/postgres/mod.rs`.
  - [x] Implement write path for full recovery gate run evidence and deterministic read paths for latest run, run-by-id, and correlation-based retrieval.
  - [x] Reuse existing persistence seams for gate inputs instead of duplicating truth stores:
    - [x] `load_latest_freshness_gate_event` for freshness evidence.
    - [x] `load_reconciliation_run`/`load_reconciliation_incident_evidence` for mismatch evidence.
    - [x] `load_active_risk_limit_profile_bundle` (+ inventory rules) for checksum source bundle.
  - [x] Return typed machine-readable persistence errors; do not swallow constraint/query/decode failures.

- [x] **Task 4: Add governance-service controlled-recovery orchestration** (AC: 1, 2, 3, 4, 5, 6, 8)
  - [x] Add recovery orchestration module (for example `services/governance-service/src/recovery/mod.rs` + `lib.rs` export) or an equivalent bounded extension in `safety_controls` without collapsing concerns.
  - [x] Implement deterministic gate evaluation sequence:
    1. freshness gate (`<=30s`),
    2. reconciliation mismatch gate (`<0.1%`),
    3. risk-limit bundle checksum gate (exact equality),
    4. operator sign-off gate.
  - [x] Ensure first failing gate returns deterministic top-level reason while still persisting full per-gate evidence for UAC-3.
  - [x] Record pass/fail run evidence in `recovery_gate_runs` and produce explicit resume approval/denial payload metadata.
  - [x] Keep fail-closed behavior for unavailable dependencies and undefined gate state.

- [x] **Task 5: Extend control-api recovery/readiness routes and contracts** (AC: 1, 2, 3, 4, 6, 8, 9)
  - [x] Add authenticated control routes in `services/control-api/src/routes/mod.rs` for controlled recovery evaluation/resume and recovery evidence retrieval (for example: evaluate + resume command and run query endpoint).
  - [x] Reuse existing authorization and response-envelope conventions (`authorize_critical_action`, machine-readable `error_code`/`reason_code`/`correlation_id`, UTC timestamps).
  - [x] Validate and normalize payload fields (`audit_reference`, sign-off intent, approved checksum/manifests references) with explicit field-level machine errors.
  - [x] Ensure resume response includes gate verdict, failing gates (if blocked), and verification evidence metadata consumable by operator-console within the UX `<=10s` confirmation expectation.

- [x] **Task 6: Implement operator-console controlled-resume UX and evidence states** (AC: 1, 2, 3, 4, 5, 6, 10)
  - [x] Extend `apps/operator-console/src/lib/risk/control-actions.ts` (or adjacent `recovery` client module) to support recovery readiness evaluate/resume contracts with strict payload parsing.
  - [x] Update `apps/operator-console/src/components/risk/SafetyActionRail.tsx` and `apps/operator-console/src/lib/risk/posture.ts` so resume transitions from static `gated` copy to runtime gate-aware states (`gated`, `in-progress`, `completed`, `blocked-with-reasons`).
  - [x] Render failing gate reasons and one clear recommended next action per failed gate in Trigger -> Context -> Action -> Verification order.
  - [x] Surface timestamped verification evidence in the rail/banner context within 10 seconds of approved resume.
  - [x] Preserve existing pause/reduce-only/cancel-all behavior and non-regression UX from Stories 3.2, 3.5, and 3.6.

- [x] **Task 7: Ensure risk-engine containment release is readiness-gated and deterministic** (AC: 1, 2, 3, 5, 8, 10)
  - [x] Extend risk runtime hydration/evaluation seams (`services/risk-engine/src/main.rs`, `services/risk-engine/src/gates/mod.rs`) to consume latest approved recovery gate run as the deterministic release condition for new-order blocking.
  - [x] Preserve fail-closed defaults: if no approved recovery run exists, or evidence is stale/invalid, containment remains active.
  - [x] Ensure approved recovery run can clear containment deterministically (including user-stream-auth blocking) without requiring ad-hoc/manual runtime mutation.
  - [x] Keep pre-trade gate sequencing from Story 2.8 and reconciliation halt semantics from Story 2.6 non-regressive.

- [x] **Task 8: Add Story 3.7 QA automation, latency assertions, and runbook updates** (AC: 2, 3, 4, 5, 6, 8, 9, 10)
  - [x] Add `qa:test:story-3-7` in root `package.json` following Epic 3 QA command conventions (targeted Rust tests + web checks + story/API/E2E suites).
  - [x] Add story/API/E2E suites under `tests/story-3-7`, `tests/api/story-3-7*`, `tests/e2e/story-3-7*` covering:
    - [x] gate pass/fail matrices and boundary thresholds,
    - [x] machine-readable failure envelopes,
    - [x] checksum mismatch and sign-off-missing failures,
    - [x] `<=10s` verification evidence visibility in UI,
    - [x] NFR16 evidence-query latency checks (`<=5s`) with representative fixtures,
    - [x] NFR6 workflow timing instrumentation assertions for 10-minute recovery objective.
  - [x] Add/update runbook(s): `docs/operations/controlled-recovery-readiness-gates.md` and cross-link with `docs/operations/emergency-safe-state-controls.md`, `docs/operations/incident-search-causal-timeline-forensics.md`, and `docs/operations/severity-alert-delivery.md`.
  - [x] Update `_bmad-output/implementation-artifacts/tests/test-summary.md` with Story 3.7 evidence after implementation.

## Dev Notes

### Technical Requirements

- Story dependency and scope are strict:
  - depends on `2.6`, `2.8`, and `3.6`,
  - schema scope limited to `recovery_gate_runs`,
  - traceability scope: `FR30`, `NFR6`, `NFR16`, `UX-DR22`.
- Required readiness gates (FR30) and pass criteria:
  - freshness gate passes only when stream freshness is `<= 30s`,
  - reconciliation gate passes only when mismatch rate is `< 0.1%`,
  - risk-limit gate passes only when computed bundle checksum exactly equals approved manifest checksum,
  - operator sign-off must be explicitly recorded at evaluation/resume time.
- Boundary clarification for implementation:
  - Story 2.6 critical-halt threshold is `> 0.1%`; Story 3.7 resume threshold is stricter (`< 0.1%`) by FR30 contract.
  - Therefore, reconciliation mismatch exactly `0.1%` is non-critical for halt semantics but **fails readiness** for resume.
- Readiness remains fail-closed:
  - missing/unavailable gate evidence blocks resume,
  - malformed checksum/sign-off inputs block resume,
  - no silent fallback to "resume allowed."
- Keep schema discipline explicit:
  - create only `recovery_gate_runs`,
  - do not mutate prior-story table contracts in this story.
- **Out of scope:** backup integrity + deterministic restore rehearsal (Story 3.8), accessibility/reduced-motion hardening (Story 3.9).

[Source: _bmad-output/planning-artifacts/epics.md#Story 3.7: Implement Controlled Recovery Readiness Gates]  
[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: _bmad-output/planning-artifacts/prd.md#Operations Dashboard & Incident Handling]  
[Source: _bmad-output/planning-artifacts/prd.md#Reliability & Availability]  
[Source: _bmad-output/planning-artifacts/prd.md#Observability & Operability]  
[Source: crates/domain/src/reconciliation.rs]

### Architecture Compliance

- Preserve ownership boundaries:
  - `services/control-api` remains authenticated command/query ingress.
  - `services/governance-service` owns readiness orchestration and policy-safe resume approval.
  - `crates/persistence` owns durable recovery run evidence and source-of-truth reads.
  - `services/risk-engine` remains the trading eligibility enforcement point.
  - `apps/operator-console` owns resume UX state rendering and operator guidance.
- Reuse canonical seams instead of reinvention:
  - freshness via existing freshness-gate persistence seam,
  - reconciliation mismatch via existing reconciliation run/evidence seam,
  - risk-limit source bundle via active profile + inventory rule seams,
  - emergency/safe-state context via existing safety-control mode seam.
- Maintain consistency rules:
  - machine-readable errors with explicit reason codes,
  - canonical response envelopes,
  - ISO-8601 UTC timestamps only,
  - no swallowed errors in safety-critical workflows.
- Keep UX flow contract visible in recovery interactions:
  - Trigger -> Context -> Action -> Verification ordering,
  - timestamped verification evidence within 10 seconds.

[Source: _bmad-output/planning-artifacts/architecture.md#Cross-Component Dependencies]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules]  
[Source: _bmad-output/planning-artifacts/ux-design-specification.md#Journey 2 — Incident Safe-State Workflow]  
[Source: _bmad-output/planning-artifacts/ux-design-specification.md#Journey Patterns]  
[Source: _bmad-output/planning-artifacts/ux-design-specification.md#2.3 Success Criteria]

### Library & Framework Requirements

- Keep workspace-pinned stack for Story 3.7 compatibility:
  - `next = 16.2.2`
  - `react = 19.2.4`
  - `react-dom = 19.2.4`
  - `tailwindcss = ^4`
  - `typescript = ^5`
  - `axum = 0.8.8`
  - `sqlx = 0.8.6`
  - `tokio = 1.48.0`
  - `polymarket-client-sdk = 0.4.4`
  - `time = 0.3.44`
  - `opentelemetry = 0.31.0`
- Latest-version checks at story creation time:
  - npm: `next 16.2.2`, `react 19.2.4`, `react-dom 19.2.4`, `tailwindcss 4.2.2`, `typescript 6.0.2`, `eslint-config-next 16.2.2`
  - crates.io: `axum 0.8.8`, `sqlx 0.8.6`, `tokio 1.51.0`, `polymarket-client-sdk 0.4.4`, `time 0.3.47`, `opentelemetry 0.31.0`, `sha2 0.11.0`
- If checksum hashing requires a new crate, add it only as story-scoped necessity (no opportunistic upgrades across unrelated dependencies).

[Source: apps/operator-console/package.json]  
[Source: Cargo.toml]  
[Source: https://registry.npmjs.org/next/latest]  
[Source: https://registry.npmjs.org/react/latest]  
[Source: https://registry.npmjs.org/react-dom/latest]  
[Source: https://registry.npmjs.org/tailwindcss/latest]  
[Source: https://registry.npmjs.org/typescript/latest]  
[Source: https://registry.npmjs.org/eslint-config-next/latest]  
[Source: https://crates.io/api/v1/crates/axum]  
[Source: https://crates.io/api/v1/crates/sqlx]  
[Source: https://crates.io/api/v1/crates/tokio]  
[Source: https://crates.io/api/v1/crates/polymarket-client-sdk]  
[Source: https://crates.io/api/v1/crates/time]  
[Source: https://crates.io/api/v1/crates/opentelemetry]  
[Source: https://crates.io/api/v1/crates/sha2]

### File Structure Requirements

- Primary implementation surfaces (expected Story 3.7 seams):
  - `crates/domain/src/lib.rs`
  - `crates/domain/src/recovery.rs` (new) and/or bounded extension in `crates/domain/src/risk.rs`
  - `crates/persistence/migrations/*recovery_gate_runs*.sql`
  - `crates/persistence/src/postgres/{mod.rs,recovery_gate_runs.rs,risk_limits.rs,reconciliation.rs,freshness_gate.rs,safety_controls.rs}`
  - `services/governance-service/src/{lib.rs,recovery/mod.rs}` (or equivalent bounded module surface)
  - `services/control-api/src/{main.rs,middleware/mod.rs,routes/mod.rs}`
  - `services/risk-engine/src/{main.rs,gates/mod.rs,safe_state/mod.rs}`
  - `apps/operator-console/src/components/risk/{RiskPostureBanner.tsx,SafetyActionRail.tsx}`
  - `apps/operator-console/src/lib/risk/{control-actions.ts,posture.ts}` (+ optional `recovery.ts` client/parser module)
  - `apps/operator-console/src/app/(dashboard)/dashboard/page.tsx`
  - `apps/operator-console/src/app/(incidents)/incidents/page.tsx`
  - `apps/operator-console/src/app/globals.css`
  - `tests/story-3-7/*.test.mjs`
  - `tests/api/story-3-7*.test.mjs`
  - `tests/e2e/story-3-7*.test.mjs`
  - `docs/operations/controlled-recovery-readiness-gates.md`
  - `docs/operations/emergency-safe-state-controls.md`
  - `package.json`
  - `_bmad-output/implementation-artifacts/tests/test-summary.md`
- Preserve Epic 3 layering pattern:
  - domain -> migration -> persistence adapter -> governance orchestration -> control-api contract -> typed web client -> UI integration -> story QA command -> runbook/evidence.

[Source: _bmad-output/planning-artifacts/architecture.md#Complete Project Directory Structure]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: crates/persistence/src/postgres/mod.rs]  
[Source: apps/operator-console/src/components/risk/SafetyActionRail.tsx]  
[Source: apps/operator-console/src/lib/risk/control-actions.ts]

### Testing Requirements

- Add deterministic coverage for:
  - gate-enforced resume blocked path when any single gate fails,
  - all-gates-pass resume approval path with post-action verification evidence visible in `<=10s`,
  - boundary semantics (`freshness == 30s` pass, `freshness > 30s` fail; `mismatch < 0.1%` pass, `mismatch == 0.1%` fail),
  - checksum exact-match semantics and mismatch failure response,
  - explicit sign-off-required behavior,
  - machine-readable error responses for invalid payload, unauthorized, dependency unavailable, and stale/invalid source evidence,
  - NFR16 query performance for recovery evidence retrieval (`<=5s` representative fixtures),
  - NFR6 workflow timing instrumentation toward 10-minute readiness objective.
- Keep test layering consistent with repository patterns:
  - Rust tests for domain/persistence/governance/control-api/risk-engine seams,
  - Story/API/E2E suites under `tests/`,
  - story-scoped QA command `qa:test:story-3-7`.
- Add regression assertions so Story 3.2 safety rail behavior remains stable for pause/reduce-only/cancel-all while resume becomes gate-aware.

[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: _bmad-output/planning-artifacts/prd.md#Reliability & Availability]  
[Source: _bmad-output/planning-artifacts/prd.md#Observability & Operability]  
[Source: _bmad-output/planning-artifacts/ux-design-specification.md#2.3 Success Criteria]  
[Source: package.json]  
[Source: _bmad-output/implementation-artifacts/stories/3-6-add-severity-based-alerts-with-recommended-operator-actions.md#Testing Requirements]

### Previous Story Intelligence

- Story 3.6 established strict alert payload guidance (`recommended_next_action`, evidence links, issued timestamps) and fallback discipline; Story 3.7 recovery failures should reuse this actionable guidance pattern.
- Story 3.5 established machine-readable incident forensics and `<=5s` evidence-query expectations; recovery gate evidence should remain correlation-aware and timeline-compatible.
- Story 3.2 intentionally hard-gated resume in `SafetyActionRail`/`posture` pending Story 3.7; this story should replace static gating with live readiness evaluation while preserving rail interaction budgets.
- Story 2.9 provides emergency containment command/query seams and safe-state reason taxonomy; Story 3.7 should build on that containment evidence instead of introducing parallel control command conventions.
- Story 2.8 pre-trade gate sequencing and fail-closed semantics are canonical for risk decisions; Story 3.7 should keep these non-regressive while adding resume-readiness release logic.
- Story 2.6 defines reconciliation mismatch threshold mechanics and incident evidence seams; Story 3.7 should reuse these sources and apply FR30 resume boundary semantics.

[Source: _bmad-output/implementation-artifacts/stories/3-6-add-severity-based-alerts-with-recommended-operator-actions.md#Previous Story Intelligence]  
[Source: _bmad-output/implementation-artifacts/stories/3-5-implement-incident-search-and-causal-timeline-forensics.md#Previous Story Intelligence]  
[Source: _bmad-output/implementation-artifacts/stories/3-2-implement-risk-posture-banner-and-persistent-safety-action-rail.md]  
[Source: _bmad-output/implementation-artifacts/stories/2-9-add-emergency-controls-and-automatic-safe-state-triggers.md#Technical Requirements]  
[Source: _bmad-output/implementation-artifacts/stories/2-8-enforce-pre-trade-gate-evaluation-pipeline.md#Technical Requirements]  
[Source: _bmad-output/implementation-artifacts/stories/2-6-build-reconciliation-and-exposure-visibility-core.md#Technical Requirements]

### Git Intelligence Summary

- Recent Epic 3 commits (`3-2` through `3-6`) follow a stable vertical-slice pattern:
  1. domain + migration + persistence contracts,
  2. control-api route contracts and strict parsing,
  3. operator-console state-machine integration,
  4. story-scoped QA script and tests,
  5. runbook + evidence updates.
- Changed-file history confirms this project favors extending canonical seams (control-api routes, domain/persistence modules, risk components, incident surfaces) over introducing parallel stacks.
- Story 3.7 should preserve this delivery shape and avoid ad-hoc recovery contracts outside existing layering.

[Source: git --no-pager log --oneline -5]  
[Source: git --no-pager log --name-only --pretty='format:%h %s' -5]

### Latest Technical Information

- Frontend package versions required for this story remain aligned with latest stable values for Next.js/React surfaces in operator-console.
- Rust core stack remains compatible for control/persistence/risk surfaces; no forced upgrade is needed for Story 3.7 delivery.
- `sha2` latest is `0.11.0`; adopt only if checksum implementation requires it and keep dependency scope minimal.

[Source: https://registry.npmjs.org/next/latest]  
[Source: https://registry.npmjs.org/react/latest]  
[Source: https://registry.npmjs.org/react-dom/latest]  
[Source: https://registry.npmjs.org/tailwindcss/latest]  
[Source: https://registry.npmjs.org/typescript/latest]  
[Source: https://registry.npmjs.org/eslint-config-next/latest]  
[Source: https://crates.io/api/v1/crates/axum]  
[Source: https://crates.io/api/v1/crates/sqlx]  
[Source: https://crates.io/api/v1/crates/tokio]  
[Source: https://crates.io/api/v1/crates/polymarket-client-sdk]  
[Source: https://crates.io/api/v1/crates/time]  
[Source: https://crates.io/api/v1/crates/opentelemetry]  
[Source: https://crates.io/api/v1/crates/sha2]

### Project Context Reference

- No `project-context.md` file was found in repository scope during discovery.
- Story context was derived from epics, PRD, architecture, UX specification, implementation-readiness report, prior stories, git history, and current code seams.

### Project Structure Notes

- `apps/operator-console/src/components/risk/SafetyActionRail.tsx` currently hard-codes `resumeState: "gated"` with explicit Story 3.7 placeholder messaging; Story 3.7 must replace this static gate with runtime recovery checks.
- `apps/operator-console/src/lib/risk/posture.ts` mirrors the same static resume-gated contract and should be updated consistently with rail behavior.
- `services/control-api/src/routes/mod.rs` currently exposes emergency routes for pause/reduce-only/cancel-all and action-result query, but no recovery readiness/resume endpoint exists yet.
- `crates/domain/src/risk.rs` + `services/governance-service/src/safety_controls/mod.rs` currently model only emergency actions (`pause`, `reduce_only`, `cancel_all`) and should not be overextended with unrelated schema changes for Story 3.7.
- `services/risk-engine/src/main.rs` hydrates containment from latest safety mode and currently sets auth block on paused mode without explicit readiness-release path; Story 3.7 should provide deterministic release wiring from approved recovery gates.
- Existing persistence seams for freshness/reconciliation/risk-limit bundle state already exist and should be reused for gate input truth.

[Source: apps/operator-console/src/components/risk/SafetyActionRail.tsx]  
[Source: apps/operator-console/src/lib/risk/posture.ts]  
[Source: apps/operator-console/src/lib/risk/control-actions.ts]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: crates/domain/src/risk.rs]  
[Source: services/governance-service/src/safety_controls/mod.rs]  
[Source: services/risk-engine/src/main.rs]  
[Source: crates/persistence/src/postgres/freshness_gate.rs]  
[Source: crates/persistence/src/postgres/reconciliation.rs]  
[Source: crates/persistence/src/postgres/risk_limits.rs]  
[Source: crates/persistence/src/postgres/safety_controls.rs]

### References

- _bmad-output/planning-artifacts/epics.md#Epic 3: Portfolio Command Center, Alerts & Recovery Operations  
- _bmad-output/planning-artifacts/epics.md#Story 3.7: Implement Controlled Recovery Readiness Gates  
- _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)  
- _bmad-output/planning-artifacts/prd.md#Operations Dashboard & Incident Handling  
- _bmad-output/planning-artifacts/prd.md#Reliability & Availability  
- _bmad-output/planning-artifacts/prd.md#Observability & Operability  
- _bmad-output/planning-artifacts/architecture.md#Cross-Component Dependencies  
- _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules  
- _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping  
- _bmad-output/planning-artifacts/ux-design-specification.md#Journey 2 — Incident Safe-State Workflow  
- _bmad-output/planning-artifacts/ux-design-specification.md#Journey Patterns  
- _bmad-output/planning-artifacts/ux-design-specification.md#2.3 Success Criteria  
- _bmad-output/planning-artifacts/implementation-readiness-report-2026-04-05.md  
- _bmad-output/implementation-artifacts/stories/2-6-build-reconciliation-and-exposure-visibility-core.md  
- _bmad-output/implementation-artifacts/stories/2-8-enforce-pre-trade-gate-evaluation-pipeline.md  
- _bmad-output/implementation-artifacts/stories/2-9-add-emergency-controls-and-automatic-safe-state-triggers.md  
- _bmad-output/implementation-artifacts/stories/3-2-implement-risk-posture-banner-and-persistent-safety-action-rail.md  
- _bmad-output/implementation-artifacts/stories/3-5-implement-incident-search-and-causal-timeline-forensics.md  
- _bmad-output/implementation-artifacts/stories/3-6-add-severity-based-alerts-with-recommended-operator-actions.md  
- docs/operations/emergency-safe-state-controls.md  
- services/control-api/src/routes/mod.rs  
- services/governance-service/src/safety_controls/mod.rs  
- services/risk-engine/src/{main.rs,gates/mod.rs,safe_state/mod.rs}  
- crates/domain/src/{lib.rs,risk.rs,reconciliation.rs}  
- crates/persistence/src/postgres/{mod.rs,freshness_gate.rs,reconciliation.rs,risk_limits.rs,safety_controls.rs}  
- crates/persistence/migrations/{20260406061000_reconciliation_exposure_core.sql,20260406072000_risk_limit_profiles.sql,20260406081500_pretrade_gate_decisions.sql,20260406100000_safety_control_actions.sql,20260406193000_incident_query_views.sql,20260406210000_incident_alerts_delivery_attempts.sql}  
- apps/operator-console/src/components/risk/{RiskPostureBanner.tsx,SafetyActionRail.tsx}  
- apps/operator-console/src/lib/risk/{posture.ts,control-actions.ts}  
- package.json  
- Cargo.toml  
- https://registry.npmjs.org/next/latest  
- https://registry.npmjs.org/react/latest  
- https://registry.npmjs.org/react-dom/latest  
- https://registry.npmjs.org/tailwindcss/latest  
- https://registry.npmjs.org/typescript/latest  
- https://registry.npmjs.org/eslint-config-next/latest  
- https://crates.io/api/v1/crates/axum  
- https://crates.io/api/v1/crates/sqlx  
- https://crates.io/api/v1/crates/tokio  
- https://crates.io/api/v1/crates/polymarket-client-sdk  
- https://crates.io/api/v1/crates/time  
- https://crates.io/api/v1/crates/opentelemetry  
- https://crates.io/api/v1/crates/sha2

## Story Completion Status

- Story 3.7 implementation completed across domain, persistence, governance orchestration, control-api ingress, risk runtime, operator-console UX, tests, and runbooks.
- Acceptance criteria and UACs were validated through Story 3.7 targeted QA and full repository regression.
- Story artifacts were updated with checklist completion, test evidence, and sprint-state transition to `review`.

## Dev Agent Record

### Agent Model Used

GPT-5.3-Codex (gpt-5.3-codex)

### Debug Log References

- BMAD dev-story workflow execution (automated, non-interactive)
- Story 3.7 sprint-status transition to `in-progress` and implementation tracking
- Story 3.7 vertical-slice implementation across domain, persistence, governance, control-api, risk-engine, and operator-console
- Story 3.7 QA execution via `npm run --silent qa:test:story-3-7`
- Full regression execution via `npm test`
- Story 3.7 API/E2E QA refresh via `node --test tests/story-3-7/*.test.mjs tests/api/story-3-7*.test.mjs tests/e2e/story-3-7*.test.mjs`

### Completion Notes List

- Added `crates/domain/src/recovery.rs` with deterministic gate contracts, checksum canonicalization, sign-off validation, resume-verification envelope helpers, and boundary-focused tests.
- Added Story 3.7 forward-only schema migration (`recovery_gate_runs`) and PostgreSQL adapters for full run persistence plus deterministic latest/by-id/by-correlation reads.
- Added governance recovery orchestration (`services/governance-service/src/recovery/mod.rs`) with fail-closed gate evaluation, resume approval enforcement, and repository adapters.
- Extended control-api state wiring and routes for readiness evaluate, resume execution, and run query surfaces with machine-readable response/error envelopes.
- Extended risk-engine hydration containment logic to remain fail-closed until approved, verification-valid recovery evidence exists.
- Extended operator-console recovery client/parsers, posture state transitions, safety rail workflow execution, blocked-gate rendering (Trigger -> Context -> Action -> Verification), and verification evidence surfaces.
- Added Story 3.7 story/API/E2E suites and root `qa:test:story-3-7` command.
- Added latency instrumentation assertions for NFR16 (`recovery query p95 <= 5s`) and NFR6 workflow timing evidence (`<= 10 minutes` objective checkpoints).
- Added controlled-recovery runbook and cross-linked emergency/incident/alerts runbooks.
- Updated Story 3.7 QA evidence in `_bmad-output/implementation-artifacts/tests/test-summary.md`.
- Code review hardening fixed premature containment release by persisting approved readiness runs without `resumed_at_utc`, requiring explicit resume execution to record verification evidence, and rejecting replayed resume commands.
- Recovery persistence now upserts `recovery_gate_runs` by `run_id` so resume verification writes durable evidence, and recovery payload parsing now enforces required timestamp fields (no success-shaped timestamp fallbacks).
- File-list cross-check: Story 3.7 application-source file list matched the reviewed implementation scope; non-source automation/tracking files were excluded from adversarial review per workflow rules.
- Added Story 3.7 API QA coverage for run-id query selection, missing-selector preflight rejection, unauthorized resume machine-error propagation, and malformed resume success-payload contract rejection.
- Added Story 3.7 E2E contract coverage for accessibility-critical recovery status/escalation semantics (`role="status"`, `role="alert"`, `aria-live="assertive"`).
- Story 3.7 node story/API/E2E automation suite now passes with 23 tests (`story: 6`, `api: 11`, `e2e: 6`).

### File List

- _bmad-output/implementation-artifacts/stories/3-7-implement-controlled-recovery-readiness-gates.md
- _bmad-output/implementation-artifacts/sprint-status.yaml
- _bmad-output/implementation-artifacts/tests/test-summary.md
- Cargo.toml
- Cargo.lock
- crates/domain/Cargo.toml
- crates/domain/src/lib.rs
- crates/domain/src/recovery.rs
- crates/persistence/migrations/20260406223000_recovery_gate_runs.sql
- crates/persistence/src/postgres/mod.rs
- crates/persistence/src/postgres/recovery_gate_runs.rs
- services/governance-service/src/lib.rs
- services/governance-service/src/recovery/mod.rs
- services/control-api/src/main.rs
- services/control-api/src/middleware/mod.rs
- services/control-api/src/routes/mod.rs
- services/risk-engine/src/main.rs
- apps/operator-console/src/lib/risk/control-actions.ts
- apps/operator-console/src/lib/risk/posture.ts
- apps/operator-console/src/components/risk/SafetyActionRail.tsx
- apps/operator-console/src/components/risk/RiskCommandSurface.tsx
- apps/operator-console/src/app/globals.css
- docs/operations/controlled-recovery-readiness-gates.md
- docs/operations/emergency-safe-state-controls.md
- docs/operations/incident-search-causal-timeline-forensics.md
- docs/operations/severity-alert-delivery.md
- package.json
- tests/story-3-7/controlled-recovery.story-3-7.test.mjs
- tests/api/story-3-7-recovery-api.test.mjs
- tests/e2e/story-3-7-controlled-recovery.e2e.test.mjs

### Change Log

- 2026-04-06: Implemented Story 3.7 controlled recovery contracts, persistence schema/adapters, governance orchestration, control-api routes, risk-engine containment release gating, and operator-console workflow integration.
- 2026-04-06: Added Story 3.7 QA automation (`qa:test:story-3-7`) and story/API/E2E suites for gate boundaries, machine-readable failures, blocked-gate rendering, and verification evidence contracts.
- 2026-04-06: Added Story 3.7 latency objective assertions covering recovery-query `p95 <= 5s` and workflow timing evidence for the 10-minute readiness objective.
- 2026-04-06: Added controlled-recovery operations runbook and cross-linked emergency/incident/alerts runbooks.
- 2026-04-06: Updated Story 3.7 checklist, file list, test summary evidence, and advanced sprint status to `review`.
- 2026-04-06: Code review remediation fixed readiness/resume persistence semantics, added replay protection for resume execution, tightened recovery timestamp contract parsing, and advanced story status to `done` after full regression.
- 2026-04-06: QA automation refresh expanded Story 3.7 API/E2E coverage (run-id selector, missing-selector preflight, unauthorized resume error handling, malformed resume payload rejection, and accessibility semantics) and re-ran story/API/E2E node suites with 23 passing tests.
