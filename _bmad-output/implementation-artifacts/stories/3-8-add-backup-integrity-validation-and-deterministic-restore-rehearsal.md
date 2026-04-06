# Story 3.8: Add Backup Integrity Validation and Deterministic Restore Rehearsal

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an ops administrator,  
I want deterministic backup-restore rehearsals with integrity checks,  
so that severe incidents can be recovered with verified data consistency.

## Acceptance Criteria

1. **Scenario A — scheduled rehearsal success (story-local BDD):**  
   **Given** rehearsal is triggered  
   **When** restore is executed in controlled environment  
   **Then** restored state matches expected checksums and reconciliation sanity checks pass.
2. **Scenario B — integrity failure path (story-local BDD):**  
   **Given** checksum or reconciliation mismatch is detected  
   **When** rehearsal completes  
   **Then** result is marked failed and production resume remains blocked.
3. **Scenario C — deterministic replay evidence (story-local BDD):**  
   **Given** two rehearsals run on same backup artifact  
   **When** both complete  
   **Then** key outputs are identical except for run metadata timestamps.
4. **UAC-1 Failure handling:** Invalid rehearsal payloads, unauthorized rehearsal/verification requests, missing backup artifact metadata, unavailable restore/reconciliation dependencies, and malformed checksum inputs return explicit machine-readable errors with no unsafe side effects or success-shaped fallback behavior.
5. **UAC-2 Boundary behavior:** Deterministic boundaries are test-covered and enforced: digest comparisons require exact lowercase 64-hex equality, reconciliation sanity pass criteria are explicit and deterministic, and deterministic replay comparison ignores allowed metadata fields (run IDs/timestamps) while requiring all operational outputs to match exactly.
6. **UAC-3 Verifiable evidence:** Every rehearsal persists timestamped evidence suitable for incident and QA traceability, including `run_id`, `correlation_id`, `artifact_id`, `artifact_checksum`, per-check outcomes, deterministic comparison signature, reconciliation sanity summary, and final pass/fail reason codes.
7. **Schema/dependency/traceability contract:** Story depends only on `2.6` and `3.7`, creates only `restore_rehearsal_runs` and `backup_integrity_checks`, and maps explicitly to `FR49`, `NFR6`, and `NFR16`.
8. **Resume-gating contract for severe incidents:** For Severity-1/Severity-2 incident recovery paths, controlled resume remains blocked unless the latest relevant rehearsal run is successful and includes passing integrity + reconciliation evidence.
9. **NFR16 evidence-query objective:** Rehearsal evidence retrieval paths (latest by artifact, by run ID, by correlation/incident context) are queryable within `<= 5s` for representative incident workload.
10. **Scope boundary contract:** Story 3.8 delivers backup-integrity rehearsal and deterministic restore validation only; accessibility/reduced-motion hardening remains Story 3.9.

## Tasks / Subtasks

- [x] **Task 1: Define backup-integrity rehearsal domain contracts and deterministic comparison rules** (AC: 1, 2, 3, 4, 5, 6, 7, 8)
  - [x] Add/extend domain contracts in `crates/domain` for restore rehearsal requests, backup integrity check items, rehearsal result envelope, and deterministic replay signature artifacts.
  - [x] Extend recovery reason taxonomy with explicit machine-readable rehearsal reasons (success, checksum mismatch, reconciliation sanity failure, deterministic replay mismatch, dependency unavailable, unauthorized, invalid payload).
  - [x] Implement canonicalization helper(s) to compute deterministic restore output signatures that exclude run metadata timestamps but preserve all business-relevant evidence fields.
  - [x] Enforce canonicalization rules explicitly: sort serialized object keys lexicographically, normalize decimal ratio fields to deterministic strings, use UTF-8 canonical JSON without formatting whitespace, and exclude only `run_id`, `started_at_utc`, `completed_at_utc`, and wall-clock duration metadata.
  - [x] Add domain tests for deterministic replay equality/inequality and integrity boundary conditions.

- [x] **Task 2: Add forward-only persistence migration for Story 3.8 schema scope** (AC: 1, 2, 3, 5, 6, 7, 9)
  - [x] Add migration under `crates/persistence/migrations/` creating only `restore_rehearsal_runs` and `backup_integrity_checks`.
  - [x] Apply strict constraints for canonical IDs, non-empty reason codes, UTC timestamps, finite reconciliation/integrity metrics, and normalized checksum formats.
  - [x] Persist deterministic replay evidence (`deterministic_signature`, optional prior signature reference, mismatch summary) and reconciliation sanity evidence.
  - [x] Add indexes for run lookup (`run_id`), artifact lookup (`artifact_id`, `completed_at_utc`), correlation/incident lookup (`correlation_id`, `completed_at_utc`), and status-time query paths.
  - [x] Keep migration scope strict: do **not** alter `recovery_gate_runs` or unrelated prior-story tables.

- [x] **Task 3: Implement rehearsal persistence adapters and deterministic query paths** (AC: 2, 3, 4, 6, 7, 9)
  - [x] Add PostgreSQL adapters in `crates/persistence/src/postgres/` for writing rehearsal runs + integrity checks and reading latest/by-id/by-correlation evidence.
  - [x] Reuse existing persistence seams for reconciliation truth (Story 2.6) and recovery run context (Story 3.7) instead of introducing parallel truth stores.
  - [x] Return typed machine-readable persistence errors; do not swallow query/constraint/decode failures.

- [x] **Task 4: Add governance-service restore rehearsal orchestration** (AC: 1, 2, 3, 4, 5, 6, 8)
  - [x] Add bounded orchestration module under `services/governance-service/src/recovery/` (or equivalent) for running restore rehearsal and assembling evidence.
  - [x] Implement deterministic evaluation sequence:
    1. validate artifact/checksum input contract,  
    2. execute/record controlled restore rehearsal context,  
    3. perform integrity checks,  
    4. run reconciliation sanity checks,  
    5. compute deterministic replay signature and compare with prior run for same artifact.
  - [x] Persist full run/check evidence and explicit pass/fail reason.
  - [x] Fail closed when dependency evidence is unavailable or inconsistent.

- [x] **Task 5: Integrate severe-incident rehearsal outcome with controlled resume gating** (AC: 2, 8, 9)
  - [x] Extend the controlled recovery flow so Severity-1/Severity-2 resume attempts require successful latest relevant rehearsal evidence.
  - [x] Define the "latest relevant rehearsal" selector contract: use latest successful run by `correlation_id` when incident-linked context is present, otherwise by `artifact_id`; order by `completed_at_utc DESC`; allow explicit `run_id` override only for audit replay.
  - [x] Ensure blocked resume responses include explicit failing reason codes and operator-actionable next steps.
  - [x] Preserve existing Story 3.7 gate semantics for non-severe or non-rehearsal-required recovery paths unless explicitly elevated by incident severity policy.

- [x] **Task 6: Extend control-api contracts and routes for rehearsal execution/query** (AC: 1, 2, 3, 4, 6, 8, 9)
  - [x] Add authenticated routes in `services/control-api/src/routes/mod.rs` using canonical response/error envelopes:
    - `POST /control/recovery/rehearsals` (execute rehearsal),
    - `GET /control/recovery/rehearsals/{run_id}` (query by run),
    - `GET /control/recovery/rehearsals?artifact_id=<id>&limit=<n>` (latest by artifact),
    - `GET /control/recovery/rehearsals?correlation_id=<id>&limit=<n>` (latest by incident correlation).
  - [x] Validate payload fields (artifact ID/checksum, restore target context, optional incident metadata, audit references) with explicit field-level machine errors.
  - [x] Reuse authorization + privileged audit emission patterns already used by emergency/recovery endpoints.

- [x] **Task 7: Implement operator-console rehearsal visibility and resume-block guidance** (AC: 2, 3, 4, 8)
  - [x] Extend typed risk/recovery client contracts in `apps/operator-console/src/lib/risk/control-actions.ts` for rehearsal run + query payloads.
  - [x] Update relevant risk/incident surfaces (`SafetyActionRail` and/or incident context surfaces) to show rehearsal status, failing checks, deterministic replay result, and one clear recommended action.
  - [x] Preserve Trigger -> Context -> Action -> Verification evidence ordering and accessibility/status semantics from Epic 3 patterns.

- [x] **Task 8: Add Story 3.8 QA automation, runbook coverage, and evidence updates** (AC: 2, 3, 4, 5, 6, 8, 9, 10)
  - [x] Add `qa:test:story-3-8` in root `package.json` following Epic 3 QA conventions.
  - [x] Add story/API/E2E suites under `tests/story-3-8`, `tests/api/story-3-8*`, `tests/e2e/story-3-8*` covering:
    - [x] rehearsal success/failure paths,
    - [x] deterministic replay equality and mismatch detection,
    - [x] machine-readable failure envelopes,
    - [x] severe-incident resume blocking behavior when rehearsal evidence is failed/missing/stale,
    - [x] NFR16 rehearsal evidence query latency assertions (`<= 5s`) with representative fixtures.
  - [x] Add/update runbook(s): `docs/operations/backup-integrity-restore-rehearsal.md` and cross-link with controlled recovery, incident forensics, and severity-alert runbooks.
  - [x] Update `_bmad-output/implementation-artifacts/tests/test-summary.md` with Story 3.8 evidence after implementation.

### Review Findings

- [x] [Review][Patch] Align restore-rehearsal persistence error-code namespace with governance/control-api status mappings [crates/persistence/src/postgres/restore_rehearsals.rs]
- [x] [Review][Patch] Use latest relevant rehearsal (regardless of status) for severe-resume gating, then fail closed unless it is successful [services/governance-service/src/recovery/mod.rs]
- [x] [Review][Patch] Enforce incident-severity normalization and rehearsal selector consistency checks for severe-resume override paths [services/governance-service/src/recovery/mod.rs]
- [x] [Review][Patch] Use latest successful rehearsal as deterministic replay baseline to prevent failed-run baseline drift [services/governance-service/src/recovery/mod.rs]
- [x] [Review][Patch] Enforce structured-object `restore_output` validation for restore rehearsal request payloads [crates/domain/src/recovery_rehearsal.rs]
- [x] [Review][Patch] Reject invalid `incident_severity` values during rehearsal evidence canonicalization instead of silently dropping them [crates/persistence/src/postgres/restore_rehearsals.rs]
- [x] [Review][Patch] Remove implicit severe-flow artifact selector default so selector metadata remains explicit [apps/operator-console/src/lib/risk/posture.ts]
- [x] [Review][Patch] Validate rehearsal response checksum/signature digest fields and clear stale rehearsal evidence state when running a new resume workflow [apps/operator-console/src/lib/risk/control-actions.ts; apps/operator-console/src/components/risk/SafetyActionRail.tsx]
- [x] [Review][Patch] Map malformed rehearsal JSON payloads to the canonical machine-readable recovery error envelope and add rehearsal query p95 latency coverage [services/control-api/src/routes/mod.rs]

## Dev Notes

### Technical Requirements

- Story dependency and scope are strict:
  - depends on `2.6` and `3.7`,
  - schema scope limited to `restore_rehearsal_runs` and `backup_integrity_checks`,
  - traceability scope: `FR49`, `NFR6`, `NFR16`.
- Rehearsal must validate both:
  - backup integrity (expected vs observed checksum/evidence consistency),
  - reconciliation sanity (using canonical reconciliation seams from Story 2.6).
- Deterministic replay contract:
  - two rehearsals on the same artifact must produce identical deterministic output signatures,
  - allowed variance is metadata timestamps/run IDs only.
- Deterministic signature canonicalization must be implementation-invariant:
  - serialize with lexicographically ordered keys and deterministic numeric string representation,
  - exclude only run metadata (`run_id`, `started_at_utc`, `completed_at_utc`, wall-clock durations),
  - treat any payload-schema mismatch during canonicalization as a hard failure (`rehearsal_signature_contract_error`).
- Severe-incident resume gate:
  - failed or missing rehearsal evidence must keep production resume blocked for Severity-1/Severity-2 paths.
- "Latest relevant rehearsal" semantics for Severity-1/Severity-2 gating:
  - first match on `correlation_id` when incident context exists, otherwise match on `artifact_id`,
  - choose most recent `completed_at_utc`,
  - if no successful run is found, return blocked decision with explicit reason code (`rehearsal_missing_or_failed`).
- Rehearsal API contract must remain deterministic and machine-readable:
  - execution endpoint accepts `artifact_id`, `artifact_checksum`, restore target context, optional `correlation_id`, optional `audit_reference`,
  - query surfaces support `run_id` path lookup and latest-by-selector (`artifact_id` or `correlation_id`) with bounded `limit`,
  - invalid selector combinations (for example, both selectors omitted) fail explicitly with machine-readable validation errors.
- Fail-closed behavior is mandatory:
  - unavailable dependency evidence, malformed payloads, or ambiguous restore outcomes must block resume and return explicit machine-readable failures.
- **Out of scope:** UI accessibility/reduced-motion hardening (Story 3.9).

[Source: _bmad-output/planning-artifacts/epics.md#Story 3.8: Add Backup Integrity Validation and Deterministic Restore Rehearsal]  
[Source: _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)]  
[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: _bmad-output/planning-artifacts/prd.md#Operations Dashboard & Incident Handling]  
[Source: _bmad-output/planning-artifacts/prd.md#Reliability & Availability]  
[Source: _bmad-output/planning-artifacts/prd.md#Observability & Operability]  
[Source: _bmad-output/planning-artifacts/architecture.md#Format Patterns]  
[Source: docs/operations/controlled-recovery-readiness-gates.md#Scope]

### Architecture Compliance

- Preserve ownership boundaries:
  - `services/control-api` remains authenticated command/query ingress,
  - `services/governance-service` owns rehearsal orchestration and resume-policy enforcement,
  - `crates/persistence` owns durable rehearsal and integrity evidence,
  - `services/risk-engine` remains enforcement point for fail-closed runtime containment posture.
- Reuse existing Story 3.7 recovery seams and contracts rather than introducing parallel resume logic.
- Maintain canonical architecture conventions:
  - machine-readable error envelopes,
  - ISO-8601 UTC timestamps only,
  - no swallowed errors in safety-critical paths.
- Keep auditability and incident forensics continuity across emergency controls, readiness gates, and restore rehearsal evidence.

[Source: _bmad-output/planning-artifacts/architecture.md#Cross-Component Dependencies]  
[Source: _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: services/governance-service/src/recovery/mod.rs]  
[Source: services/risk-engine/src/main.rs]

### Library & Framework Requirements

- Keep workspace-pinned stack for Story 3.8 compatibility:
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
  - `sha2 = 0.10.9`
- Latest-version checks at story creation time:
  - npm: `next 16.2.2`, `react 19.2.4`, `react-dom 19.2.4`, `tailwindcss 4.2.2`, `typescript 6.0.2`, `eslint-config-next 16.2.2`
  - crates.io: `axum 0.8.8`, `sqlx max_stable 0.8.6 (newest 0.9.0-alpha.1)`, `tokio 1.51.0`, `polymarket-client-sdk 0.4.4`, `time 0.3.47`, `opentelemetry 0.31.0`, `sha2 0.11.0`
- Do not perform opportunistic dependency upgrades in Story 3.8 unless strictly required for restore-rehearsal scope.

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

- Primary implementation surfaces (expected Story 3.8 seams):
  - `crates/domain/src/{lib.rs,recovery.rs}` and/or `crates/domain/src/recovery_rehearsal.rs`
  - `crates/persistence/migrations/*restore_rehearsal_runs*backup_integrity_checks*.sql`
  - `crates/persistence/src/postgres/{mod.rs,restore_rehearsals.rs,recovery_gate_runs.rs,reconciliation.rs}`
  - `services/governance-service/src/recovery/{mod.rs,rehearsal.rs}`
  - `services/control-api/src/routes/mod.rs`
  - `services/risk-engine/src/main.rs`
  - `apps/operator-console/src/lib/risk/control-actions.ts`
  - `apps/operator-console/src/components/risk/SafetyActionRail.tsx`
  - `apps/operator-console/src/app/(incidents)/incidents/page.tsx`
  - `tests/story-3-8/*.test.mjs`
  - `tests/api/story-3-8*.test.mjs`
  - `tests/e2e/story-3-8*.test.mjs`
  - `docs/operations/backup-integrity-restore-rehearsal.md`
  - `docs/operations/controlled-recovery-readiness-gates.md`
  - `package.json`
  - `_bmad-output/implementation-artifacts/tests/test-summary.md`
- Preserve established Epic 3 layering pattern:
  - domain -> migration -> persistence adapter -> governance orchestration -> control-api contract -> typed web client -> UI integration -> story QA command -> runbook/evidence.

[Source: _bmad-output/planning-artifacts/architecture.md#Complete Project Directory Structure]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: services/governance-service/src/recovery/mod.rs]  
[Source: services/risk-engine/src/main.rs]  
[Source: apps/operator-console/src/lib/risk/control-actions.ts]  
[Source: apps/operator-console/src/components/risk/SafetyActionRail.tsx]

### Testing Requirements

- Add deterministic coverage for:
  - scheduled rehearsal success path with passing integrity + reconciliation sanity,
  - checksum mismatch and reconciliation mismatch failure paths,
  - deterministic replay equality for same artifact and mismatch detection when operational output changes,
  - machine-readable errors for invalid payload, unauthorized, dependency unavailable, and stale/missing rehearsal evidence,
  - severe-incident resume blocking semantics when rehearsal evidence is not successful,
  - NFR16 rehearsal evidence query latency (`<= 5s`) with representative fixtures.
- Keep test layering consistent with repository standards:
  - Rust tests for domain/persistence/governance/control-api/risk-engine seams,
  - story/API/E2E suites under `tests/`,
  - story-scoped QA command `qa:test:story-3-8`.
- Add regression assertions so Story 3.7 readiness semantics remain stable while rehearsal evidence adds additional severe-incident guardrails.

[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: _bmad-output/planning-artifacts/prd.md#Reliability & Availability]  
[Source: _bmad-output/planning-artifacts/prd.md#Observability & Operability]  
[Source: _bmad-output/implementation-artifacts/stories/3-7-implement-controlled-recovery-readiness-gates.md#Testing Requirements]  
[Source: package.json]

### Previous Story Intelligence

- Story 3.7 introduced controlled recovery gate orchestration, strict machine-readable response contracts, and fail-closed resume semantics; Story 3.8 should extend this recovery model rather than creating parallel resume pathways.
- Story 3.7 established `RecoveryGateRunEvidence` durability + runbook + route conventions (`/control/recovery/*`) that should be reused for rehearsal execution/query behavior.
- Story 3.6 established severity-based incident alerting and recommended-action metadata; Story 3.8 should align severe-incident rehearsal gating language with the same operator-actionable semantics.
- Story 2.6 remains the canonical reconciliation evidence seam and should be reused for rehearsal sanity checks.
- Current risk-engine containment release logic already depends on approved/verified recovery evidence; Story 3.8 should preserve fail-closed defaults while integrating rehearsal-success requirements for severe incidents.

[Source: _bmad-output/implementation-artifacts/stories/3-7-implement-controlled-recovery-readiness-gates.md#Previous Story Intelligence]  
[Source: _bmad-output/implementation-artifacts/stories/3-6-add-severity-based-alerts-with-recommended-operator-actions.md#Previous Story Intelligence]  
[Source: _bmad-output/implementation-artifacts/stories/2-6-build-reconciliation-and-exposure-visibility-core.md#Technical Requirements]  
[Source: services/risk-engine/src/main.rs]

### Git Intelligence Summary

- Recent Epic 3 commits (`3-3` through `3-7`) follow a consistent vertical-slice delivery pattern:
  1. domain + migration + persistence contracts,
  2. governance/control-api orchestration and machine-readable error semantics,
  3. operator-console integration with strict typed parsing,
  4. story-scoped QA command + story/API/E2E suites,
  5. runbook + evidence updates.
- Story 3.8 should follow this pattern and avoid introducing side-channel restore/recovery logic outside existing seams.

[Source: git --no-pager log --oneline -5]

### Latest Technical Information

- Frontend stack remains aligned with latest stable values for Next.js/React surfaces used in operator-console.
- Rust stack remains compatible for Story 3.8 scope; stable latest versions are available for `tokio`, `time`, and `sha2`, with `sqlx` showing a newer pre-release line.
- Dependency upgrades are optional for this story and should be avoided unless strictly required by restore rehearsal implementation.

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
- Story context was derived from epics, PRD, architecture, UX specification, implementation-readiness report, prior Epic 3 stories, current code seams, runbooks, and recent git history.

### Project Structure Notes

- `services/control-api/src/routes/mod.rs` currently supports controlled readiness evaluate/resume/query endpoints but does not yet expose dedicated restore rehearsal execution/query routes.
- `services/governance-service/src/recovery/mod.rs` currently evaluates freshness/reconciliation/risk-checksum/signoff gates and persists `recovery_gate_runs`; backup-integrity rehearsal evidence paths are not yet modeled there.
- `crates/domain/src/recovery.rs` currently models recovery gate outcomes and checksum helpers; backup artifact rehearsal contracts and deterministic replay signatures are not yet represented.
- `crates/persistence/migrations/` currently includes `recovery_gate_runs` but has no `restore_rehearsal_runs` or `backup_integrity_checks` tables.
- `services/risk-engine/src/main.rs` currently releases containment based on approved recovery run with resume verification; severe-incident rehearsal-success linkage is not yet enforced.
- `docs/operations/controlled-recovery-readiness-gates.md` currently documents readiness gates and resume verification, and should be cross-linked with new backup-integrity rehearsal operational guidance.

[Source: services/control-api/src/routes/mod.rs]  
[Source: services/governance-service/src/recovery/mod.rs]  
[Source: crates/domain/src/recovery.rs]  
[Source: crates/persistence/migrations/20260406223000_recovery_gate_runs.sql]  
[Source: services/risk-engine/src/main.rs]  
[Source: docs/operations/controlled-recovery-readiness-gates.md]

### References

- _bmad-output/planning-artifacts/epics.md#Epic 3: Portfolio Command Center, Alerts & Recovery Operations  
- _bmad-output/planning-artifacts/epics.md#Story 3.8: Add Backup Integrity Validation and Deterministic Restore Rehearsal  
- _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)  
- _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)  
- _bmad-output/planning-artifacts/prd.md#Operations Dashboard & Incident Handling  
- _bmad-output/planning-artifacts/prd.md#Reliability & Availability  
- _bmad-output/planning-artifacts/prd.md#Observability & Operability  
- _bmad-output/planning-artifacts/architecture.md#Cross-Component Dependencies  
- _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules  
- _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping  
- _bmad-output/planning-artifacts/implementation-readiness-report-2026-04-05.md#Remediation Outcomes  
- _bmad-output/planning-artifacts/ux-design-specification.md#Journey 2 — Incident Safe-State Workflow  
- _bmad-output/implementation-artifacts/stories/3-6-add-severity-based-alerts-with-recommended-operator-actions.md  
- _bmad-output/implementation-artifacts/stories/3-7-implement-controlled-recovery-readiness-gates.md  
- services/control-api/src/routes/mod.rs  
- services/governance-service/src/recovery/mod.rs  
- services/risk-engine/src/main.rs  
- crates/domain/src/recovery.rs  
- crates/persistence/migrations/20260406223000_recovery_gate_runs.sql  
- docs/operations/controlled-recovery-readiness-gates.md  
- apps/operator-console/package.json  
- Cargo.toml  
- package.json  
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

- Story 3.8 implementation completed and validated with full QA workflow (`qa:test:story-3-8`).
- Rehearsal execution/query, severe-incident resume gating, and operator-console rehearsal evidence surfaces are implemented.
- Story status is set to `done`.

## Dev Agent Record

### Agent Model Used

GPT-5.3-Codex (gpt-5.3-codex)

### Debug Log References

- BMAD dev-story workflow execution (automated, non-interactive)
- Story 3.8 sprint transition: `ready-for-dev` -> `in-progress` -> `review` -> `done`
- Full Story 3.8 QA execution (`npm run --silent qa:test:story-3-8`) with passing Rust/web/node suites
- Story 3.8 QA automation refresh (`node --test tests/story-3-8/*.test.mjs tests/api/story-3-8*.test.mjs tests/e2e/story-3-8*.test.mjs` and `npm run --silent qa:test:story-3-8`) with expanded API/E2E coverage and passing suites
- BMAD code-review workflow execution with high/medium findings auto-fixed and story promoted to `done`

### Completion Notes List

- Added domain rehearsal contracts, strict deterministic canonicalization/signature rules, and explicit rehearsal reason-code taxonomy.
- Added Story 3.8 migration + persistence adapters for rehearsal run/check evidence and selector queries.
- Implemented governance rehearsal orchestration and Severity-1/Severity-2 resume fail-closed gating on successful rehearsal evidence.
- Added control-api rehearsal execute/query routes with canonical machine-readable envelopes and audit reuse.
- Extended operator-console typed client + safety rail to surface rehearsal evidence (status/failing checks/deterministic replay/recommended action).
- Added Story 3.8 QA command, story/API/E2E test suites, new rehearsal runbook, and test summary evidence.
- Expanded Story 3.8 API/E2E QA coverage for machine-readable recovery error mapping, malformed JSON handling, query p95 latency budget assertions, severe-incident selector requirements, stale evidence clearing, and blocked-with-reasons rehearsal failure diagnostics.
- Adversarial code review fixed high/medium findings: severe-resume now enforces latest-relevant rehearsal evidence and selector consistency, deterministic baseline now uses latest successful rehearsal evidence, restore-output payload validation is strict object-only, restore-rehearsal persistence error codes and severity canonicalization are strict, malformed rehearsal JSON maps to machine-readable recovery errors, rehearsal query p95 latency coverage was added, and severe-flow selector defaults are explicit.
- Review cross-check noted unrelated workspace drift in `.scripts/bmad-auto/copilot/bmad-progress.log`, excluded from Story 3.8 scope.

### File List

- crates/domain/src/recovery.rs
- crates/domain/src/recovery_rehearsal.rs
- crates/domain/src/lib.rs
- crates/persistence/migrations/20260406234500_restore_rehearsal_runs.sql
- crates/persistence/src/postgres/mod.rs
- crates/persistence/src/postgres/restore_rehearsals.rs
- services/governance-service/src/recovery/mod.rs
- services/control-api/src/routes/mod.rs
- apps/operator-console/src/lib/risk/control-actions.ts
- apps/operator-console/src/lib/risk/posture.ts
- apps/operator-console/src/components/risk/SafetyActionRail.tsx
- apps/operator-console/src/components/risk/RiskCommandSurface.tsx
- docs/operations/backup-integrity-restore-rehearsal.md
- docs/operations/controlled-recovery-readiness-gates.md
- docs/operations/incident-search-causal-timeline-forensics.md
- docs/operations/severity-alert-delivery.md
- tests/story-3-8/backup-integrity-rehearsal.story-3-8.test.mjs
- tests/api/story-3-8-recovery-rehearsal-api.test.mjs
- tests/e2e/story-3-8-recovery-rehearsal.e2e.test.mjs
- _bmad-output/implementation-artifacts/tests/test-summary.md
- package.json
- _bmad-output/implementation-artifacts/sprint-status.yaml
- _bmad-output/implementation-artifacts/stories/3-8-add-backup-integrity-validation-and-deterministic-restore-rehearsal.md

### Change Log

- 2026-04-06: Created Story 3.8 context file and advanced sprint status from `backlog` to `ready-for-dev`.
- 2026-04-06: Validate-story gate remediation added explicit deterministic canonicalization rules, severe-incident rehearsal selector semantics, and concrete rehearsal API route contracts.
- 2026-04-06: Implemented Story 3.8 rehearsal domain, persistence, governance, control-api, and operator-console surfaces with severe-incident resume gating and deterministic evidence contracts.
- 2026-04-06: Added Story 3.8 QA command + story/api/e2e suites, runbook cross-links, test summary evidence, and transitioned story to `review`.
- 2026-04-06: Adversarial code review triage auto-fixed all identified high/medium findings and transitioned story to `done`.
- 2026-04-07: Refreshed Story 3.8 API/E2E QA automation coverage for error-path reason codes, malformed JSON handling, NFR16 query latency guardrails, and severe-resume selector/blocking UI flows; reran full `qa:test:story-3-8` successfully.
