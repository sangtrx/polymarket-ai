# Story 6.8: Deliver Alpha Governance Readiness Card UX

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an operator,  
I want a governance card that summarizes validation readiness and lifecycle state,  
so that I can make promotion/deallocation decisions quickly and safely.

## Acceptance Criteria

1. **Scenario A - governance card completeness (Epic 6 baseline):**  
   **Given** alpha telemetry and validation artifacts exist  
   **When** the governance readiness card renders  
   **Then** it shows lifecycle state, validation completeness, shadow stability, and guardrail state.

2. **Scenario B - missing evidence failure path:**  
   **Given** required evidence artifacts are missing  
   **When** readiness is computed  
   **Then** the card renders a blocked state with an explicit missing-artifact list.

3. **Scenario C - loading/empty UX contract:**  
   **Given** candidate inputs are absent or data is delayed  
   **When** the card is requested  
   **Then** tokenized skeleton and empty states are shown with actionable next-step guidance (no spinner-only fallback).

4. **Scenario D - metadata-first evidence rendering:**  
   **Given** card data is available  
   **When** content is displayed  
   **Then** metadata appears before details (timestamp/source/reason/status/correlation first), consistent with UX-DR12.

5. **Scenario E - deterministic lifecycle-state derivation:**  
   **Given** promotion decision and shadow evidence histories  
   **When** lifecycle state is derived  
   **Then** state mapping is deterministic and limited to: `draft`, `shadow`, `candidate-live`, `production`, `deallocated`.

6. **Scenario F - deterministic validation-completeness derivation:**  
   **Given** validation-run artifacts and promotion packet evidence  
   **When** completeness is computed  
   **Then** required FR7 stages and FR45 promotion packet fields are evaluated with machine-readable missing-field output.

7. **Scenario G - shadow stability and guardrail derivation:**  
   **Given** shadow-evaluation outputs and alpha-health metric/breach streams  
   **When** readiness signals are computed  
   **Then** shadow stability and guardrail state are deterministic, auditable, and explainable in-card.

8. **Scenario H - FR10 window visibility:**  
   **Given** alpha-health windows are available  
   **When** the card renders guardrail evidence  
   **Then** 1h, 24h, and 30d windows are surfaced with explicit boundary semantics where relevant.

9. **Scenario I - fail-closed API/dependency behavior:**  
   **Given** one or more upstream read-model dependencies return malformed, unauthorized, or unavailable results  
   **When** card data loads  
   **Then** the UI renders explicit machine-readable error context and does not emit success-shaped fallback readiness.

10. **Scenario J - route/shell composition continuity:**  
    **Given** dashboard and governance route shells are loaded  
    **When** Story 6.8 card is integrated  
    **Then** persistent shell composition and existing route contracts remain non-regressive.

11. **Scenario K - accessibility and reduced-motion continuity:**  
    **Given** keyboard-only and assistive-technology usage  
    **When** operators interact with card controls and status transitions  
    **Then** WCAG-aligned semantics are preserved (status roles, announcements, focus-visible, reduced-motion parity).

12. **UAC-1 Failure handling:** Invalid input, unauthorized access, and dependency-unavailable paths surface explicit machine-readable errors with no unsafe side effects.

13. **UAC-2 Boundary behavior:** Lifecycle/readiness/threshold-window boundaries are deterministic and test-covered (inclusive/exclusive rules and equality handling documented).

14. **UAC-3 Verifiable evidence:** Successful and failed readiness evaluations expose timestamped, correlation-safe evidence for incident and QA traceability.

15. **Schema/dependency/traceability contract:** Dependencies are `3.1`, `3.4`, and `6.7`; schema impact is **none** (consume existing projections only); traceability maps to `FR10`, `UX-DR8`, `UX-DR12`, and `UX-DR20`.

## Tasks / Subtasks

- [x] **Task 1: Implement a typed governance-readiness data client that composes existing research read endpoints** (AC: 1, 2, 6, 7, 8, 9, 12, 13, 14, 15)
  - [x] Add `apps/operator-console/src/lib/governance/readiness.ts` with a typed query surface (error class + result contracts) following existing client patterns (`attribution.ts`, `forensics.ts`).
  - [x] Consume existing control-api reads only (no backend/schema expansion):  
        `/control/research/promotion-decisions`, `/control/research/shadow-evaluations`, `/control/research/validation-runs`, `/control/research/validation-runs/{run_id}`, `/control/research/alpha-health-metrics`, `/control/research/alpha-threshold-breaches`.
  - [x] Parse canonical envelope shapes (`data/meta/error`) and fail closed on contract mismatches with explicit `governance_readiness_contract_mismatch`-style errors.
  - [x] Canonicalize `candidate_id` / `alpha_id` inputs consistently with existing operator-console identifier validation patterns.

- [x] **Task 2: Implement deterministic readiness derivation logic (lifecycle, completeness, shadow, guardrails)** (AC: 1, 2, 5, 6, 7, 8, 13, 14)
  - [x] Encode lifecycle-state derivation rules in one pure helper (no UI-side ad hoc branching).
  - [x] Compute validation completeness from required FR7 validation stages (`quality`, `labeling`, `purged_cv`, `cpcv`, `overfit_diagnostics`) and FR45 promotion packet fields (`data_quality_report`, `purged_cpcv_results`, `calibration_report`, `counterfactual_replay_summary`).
  - [x] Compute guardrail state from latest health metrics + threshold breaches and expose explicit reason codes.
  - [x] Emit a deterministic `missingArtifacts[]` list and `recommendedNextAction` text used by blocked/empty/error states.

- [x] **Task 3: Implement Alpha Governance Readiness card UI with explicit state machine contracts** (AC: 1, 2, 3, 4, 7, 8, 9, 11)
  - [x] Add `apps/operator-console/src/components/governance/AlphaGovernanceReadinessCard.tsx` as a client component.
  - [x] Preserve `apps/operator-console/src/components/governance/GovernanceQueueCard.tsx` as a compatibility wrapper/export to avoid Story 3.x composition regressions.
  - [x] Implement explicit UI states: `loading`, `ready`, `empty`, `error`, `critical`, plus `readiness=blocked|ready`.
  - [x] Render metadata-first layout (as-of/source/reason/correlation/status first), lifecycle/status badges, missing-artifact list, and next-action guidance.

- [x] **Task 4: Integrate card into governance and dashboard route composition without shell regressions** (AC: 1, 3, 10, 11)
  - [x] Update `apps/operator-console/src/app/(governance)/governance/page.tsx` to pass API base URL and freshness context into the readiness card surface.
  - [x] Keep dashboard and governance card composition aligned with existing shell/tab patterns and route-level risk-posture wiring.
  - [x] Preserve existing `GovernanceQueueCard` symbol usage where static story tests currently assert route composition.

- [x] **Task 5: Add tokenized styles for readiness card states and evidence surfaces** (AC: 3, 4, 11)
  - [x] Extend `apps/operator-console/src/app/globals.css` with governance-card class contracts (metadata grid, skeleton, empty state, blocked list, error/critical surfaces).
  - [x] Reuse existing semantic tokens from `apps/operator-console/src/styles/tokens.css` (no raw hex in feature styles).
  - [x] Preserve focus-visible and `prefers-reduced-motion` behavior for any new interactive elements/announcements.

- [x] **Task 6: Add Story 6.8 QA automation (story, api, e2e) and command wiring** (AC: 1-15)
  - [x] Add `tests/story-6-8/alpha-governance-readiness-card.story-6-8.test.mjs` to assert card composition/state-machine semantics and metadata-first ordering.
  - [x] Add `tests/api/story-6-8-alpha-governance-readiness-api.test.mjs` to compile/test the new governance-readiness client contract mapping and fail-closed errors.
  - [x] Add `tests/e2e/story-6-8-alpha-governance-readiness.e2e.test.mjs` for route wiring/non-regression assertions across dashboard + governance pages.
  - [x] Add `qa:test:story-6-8` in root `package.json`:  
        `npm run web:lint && npm run web:typecheck && npm run web:build && node --test tests/story-6-8/*.test.mjs tests/api/story-6-8*.test.mjs tests/e2e/story-6-8*.test.mjs`.

- [x] **Task 7: Publish operations runbook continuity for governance readiness UX** (AC: 2, 3, 14, 15)
  - [x] Add `docs/operations/alpha-governance-readiness-card.md` documenting readiness signal interpretation, blocked-state triage, and next-action playbooks.
  - [x] Cross-link Story 6.8 runbook with Story 6.5/6.6/6.7 governance runbooks.
  - [x] Update `_bmad-output/implementation-artifacts/tests/test-summary.md` with Story 6.8 QA evidence after implementation.

### Review Findings

- [x] [Review][Patch][Medium] Wrap non-contract transport failures in `queryAlphaGovernanceReadiness` so dependency-unavailable/network failures surface machine-readable `governance_readiness_request_failed` diagnostics instead of opaque runtime errors. [apps/operator-console/src/lib/governance/readiness.ts]
- [x] [Review][Patch][Medium] Add API regression coverage for transport-level failures to enforce the fail-closed error contract in Story 6.8. [tests/api/story-6-8-alpha-governance-readiness-api.test.mjs]

## Dev Notes

### Technical Requirements

- Story 6.8 is a **frontend/read-model composition story**. It must not add migrations, backend routes, or new schema entities.
- Primary existing evidence surfaces to compose:
  - `GET /control/research/promotion-decisions?candidate_id={candidate_id}&limit={n}`
  - `GET /control/research/shadow-evaluations?candidate_id={candidate_id}&limit={n}`
  - `GET /control/research/validation-runs?candidate_id={candidate_id}&limit={n}`
  - `GET /control/research/validation-runs/{run_id}`
  - `GET /control/research/alpha-health-metrics?alpha_id={alpha_id}&limit={n}`
  - `GET /control/research/alpha-threshold-breaches?alpha_id={alpha_id}&limit={n}`
- Required FR45 promotion packet fields for completeness logic:
  - `data_quality_report`
  - `purged_cpcv_results`
  - `calibration_report`
  - `counterfactual_replay_summary`
- Required FR10 health windows for guardrail evidence:
  - `1h`, `24h`, `30d`
- Deterministic lifecycle derivation contract for Story 6.8:

| Derived lifecycle state | Deterministic rule |
| --- | --- |
| `deallocated` | Latest **allowed** promotion decision has `lifecycle_action=retire`. |
| `production` | At least 2 consecutive **allowed** `promote` decisions exist, with no later allowed `pause`/`retire`. |
| `candidate-live` | Latest allowed action is `promote` but production criteria not met, or latest allowed action is `pause`. |
| `shadow` | No allowed `promote`/`retire` decision, and latest shadow evaluation is `completed`. |
| `draft` | None of the above evidence conditions are met. |

- Deterministic blocked-readiness contract:
  - `missingArtifacts` is the union of missing FR7 stages, missing FR45 packet fields, missing shadow-stability evidence, and missing FR10 windows.
  - Any non-empty `missingArtifacts` yields `readiness=blocked` with explicit list rendering.
  - Upstream unavailable/malformed responses must render explicit error/critical states with machine-readable code and correlation evidence.

### Architecture Compliance

- Keep bounded ownership intact:
  - `apps/operator-console` owns Story 6.8 UI and client composition logic.
  - `services/control-api` and `services/research-gateway` are consumed as-is.
- Follow established patterns:
  - Next.js App Router route composition + shell tabs.
  - Typed API client contracts with strict parsing and explicit error classes.
  - Metadata-first rendering and explicit UI state machines.
- Preserve safety-first failure behavior:
  - no silent fallbacks for malformed envelopes,
  - no broad catches that hide machine-readable diagnostics,
  - explicit operator guidance in empty/error/blocked states.

### Library & Framework Requirements

- Use existing operator-console stack and pinned versions:
  - `next`: `16.2.2`
  - `react`: `19.2.4`
  - `react-dom`: `19.2.4`
- Latest package lookups at story-creation time match pinned versions (`next 16.2.2`, `react/react-dom 19.2.4`), so no dependency upgrades are required.
- Reuse current TypeScript + Node test tooling patterns; do not introduce new frontend state/query libraries for this story.

### File Structure Requirements

- Expected implementation surfaces:
  - `apps/operator-console/src/lib/governance/readiness.ts` (new)
  - `apps/operator-console/src/components/governance/AlphaGovernanceReadinessCard.tsx` (new)
  - `apps/operator-console/src/components/governance/GovernanceQueueCard.tsx` (compatibility wrapper/update)
  - `apps/operator-console/src/app/(governance)/governance/page.tsx`
  - `apps/operator-console/src/app/(dashboard)/dashboard/page.tsx`
  - `apps/operator-console/src/app/globals.css`
  - `tests/story-6-8/*.test.mjs` (new)
  - `tests/api/story-6-8*.test.mjs` (new)
  - `tests/e2e/story-6-8*.test.mjs` (new)
  - `docs/operations/alpha-governance-readiness-card.md` (new)
  - `package.json` (`qa:test:story-6-8`)
  - `_bmad-output/implementation-artifacts/tests/test-summary.md` (post-implementation update)
- Keep Story 6.8 scope out of Rust/persistence/backend schema files.

### Testing Requirements

- Story-scoped UI contract tests should cover:
  - lifecycle-state rendering for all supported states,
  - blocked readiness with explicit missing-artifact list,
  - metadata-first ordering and status badge behavior,
  - loading/empty/error/critical state contracts.
- API client tests should cover:
  - canonical query construction and endpoint paths,
  - envelope parsing for all composed read endpoints,
  - machine-readable fail-closed error propagation on non-2xx and malformed 2xx payloads.
- E2E/static composition tests should cover:
  - governance + dashboard route integration,
  - non-regression for existing shell composition assertions,
  - presence of new governance-readiness CSS/state classes.
- Story QA command target:
  - `npm run --silent qa:test:story-6-8`

### Previous Story Intelligence

- Story 6.7 already delivers alpha-health metrics/breaches with deterministic threshold semantics and is the primary guardrail input seam for 6.8.
- Story 6.6 already delivers replay summary/gate outcomes consumed through promotion evidence packets; 6.8 should not reconstruct replay logic.
- Story 6.5 already enforces FR45 packet completeness and surfaces `missing_evidence_fields`; 6.8 should reuse those fields for blocked-state rendering.
- Story 3.4 and Story 3.5 established operator-console patterns to follow:
  - typed fail-closed client modules (`src/lib/...`),
  - explicit card state machines (`loading|ready|empty|error|critical`),
  - metadata-first evidence layout,
  - static story/api/e2e contract-test style in `tests/`.
- Existing Story 3.x tests assert `GovernanceQueueCard` composition in dashboard/governance routes; preserve symbol continuity or update those tests as part of 6.8.

### Git Intelligence Summary

- Recent commit cadence confirms Epic 6 vertical-slice delivery conventions:
  1. `da9c17d` — Story 6.7 live alpha health monitoring
  2. `1e1e758` — Story 6.6 counterfactual replay stress gates
  3. `f0ff1b1` — Story 6.5 promotion threshold and lifecycle governance
  4. `5df6e5d` — Story 6.4 shadow mode evaluation pipeline
  5. `92f1fe1` — Story 6.3 validation workflow and diagnostics store
- Story 6.8 should preserve that rigor while remaining frontend-only (typed client + component + tests + runbook).

### Latest Technical Information

- Operator-console dependencies are current and aligned with latest npm versions for `next` and `react` families; no version migration is required for Story 6.8.
- Existing UI/client contract patterns in `apps/operator-console/src/lib/portfolio/attribution.ts` and `apps/operator-console/src/lib/incidents/forensics.ts` are the authoritative implementation templates for strict parsing and error handling.

### Project Context Reference

- `project-context.md` is not present in this repository.
- Story context is derived from planning artifacts, prior Story 6.x implementation artifacts, and current operator-console/control-api seams.

### Project Structure Notes

- Story 6.8 aligns with current unified structure: App Router pages under `apps/operator-console/src/app/*`, workflow components under `src/components/*`, and typed client seams under `src/lib/*`.
- No project-structure conflicts were found; this story should stay within existing frontend boundaries and avoid backend/schema edits.

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 6.8: Deliver Alpha Governance Readiness Card UX]
- [Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]
- [Source: _bmad-output/planning-artifacts/epics.md#UX Design Requirements]
- [Source: _bmad-output/planning-artifacts/prd.md#Model Integrity & Promotion Governance]
- [Source: _bmad-output/planning-artifacts/architecture.md#Frontend Architecture]
- [Source: _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules]
- [Source: _bmad-output/planning-artifacts/ux-design-specification.md#Journey 3 — Alpha Promotion Governance]
- [Source: _bmad-output/planning-artifacts/ux-design-specification.md#Alpha Governance Card]
- [Source: _bmad-output/planning-artifacts/ux-design-specification.md#Additional Patterns]
- [Source: _bmad-output/implementation-artifacts/stories/6-7-implement-live-alpha-health-monitoring-and-threshold-detection.md]
- [Source: _bmad-output/implementation-artifacts/stories/6-6-integrate-counterfactual-replay-stress-gates.md]
- [Source: _bmad-output/implementation-artifacts/stories/6-5-enforce-promotion-thresholds-evidence-criteria-and-lifecycle-actions.md]
- [Source: apps/operator-console/src/app/(governance)/governance/page.tsx]
- [Source: apps/operator-console/src/app/(dashboard)/dashboard/page.tsx]
- [Source: apps/operator-console/src/components/governance/GovernanceQueueCard.tsx]
- [Source: apps/operator-console/src/components/portfolio/PnlAttributionSummaryCard.tsx]
- [Source: apps/operator-console/src/lib/portfolio/attribution.ts]
- [Source: apps/operator-console/src/lib/incidents/forensics.ts]
- [Source: apps/operator-console/src/app/globals.css]
- [Source: apps/operator-console/src/styles/tokens.css]
- [Source: services/control-api/src/routes/mod.rs]
- [Source: crates/domain/src/research.rs]
- [Source: docs/operations/alpha-promotion-lifecycle-governance.md]
- [Source: docs/operations/alpha-counterfactual-replay-stress-gating.md]
- [Source: docs/operations/alpha-live-health-monitoring-threshold-breaches.md]
- [Source: package.json]
- [Source: apps/operator-console/package.json]
- [Source: git --no-pager log --oneline -5]
- [Source: npm view next version]
- [Source: npm view react version]
- [Source: npm view react-dom version]

## Story Completion Status

- Story 6.8 implementation was adversarially reviewed and all HIGH/MEDIUM findings were auto-fixed.
- Sprint tracking status for `6-8-deliver-alpha-governance-readiness-card-ux` should be `done`.

## Dev Agent Record

### Agent Model Used

GPT-5.3-Codex (gpt-5.3-codex)

### Debug Log References

- create-story workflow context assembly (non-implementation pass)
- Implemented Story 6.8 governance-readiness client/card integration and style contracts.
- Executed `npm run --silent qa:test:story-6-8`.
- Executed full regression suite via `source "$HOME/.cargo/env" && npm test`.
- Executed 2026-04-08 QA automation refresh for Story 6.8 (`npm run --silent qa:test:story-6-8`) after expanding blocked-readiness and fail-closed UI coverage.

### Completion Notes List

- Added complete Story 6.8 implementation guidance with deterministic readiness-derivation rules, frontend integration surfaces, and story-scoped QA expectations.
- Implemented `queryAlphaGovernanceReadiness` and `deriveAlphaGovernanceReadiness` with strict canonical envelope parsing, identifier canonicalization, deterministic lifecycle/completeness/shadow/guardrail derivation, and machine-readable blocked-artifact output.
- Added `AlphaGovernanceReadinessCard` with explicit `loading|ready|empty|error|critical` contracts, `readiness=blocked|ready` rendering, metadata-first evidence, FR10 window visibility, and machine-readable failure context.
- Preserved `GovernanceQueueCard` symbol continuity as compatibility wrapper while wiring dashboard/governance routes with base URL + freshness context.
- Added Story 6.8 story/api/e2e QA automation, `qa:test:story-6-8` command wiring, Story 6.8 operations runbook, and Story 6.8 QA summary entry.
- Auto-fixed a medium review finding by wrapping transport failures in `queryAlphaGovernanceReadiness` with explicit `governance_readiness_request_failed` diagnostics, and added an API regression test for that failure path.
- Cross-check discrepancy: `.scripts/bmad-auto/copilot/bmad-progress.log` appears in git status but is an automation log outside Story 6.8 application scope and was intentionally excluded from implementation review/file tracking.
- Expanded Story 6.8 QA automation with explicit blocked-readiness derivation coverage (`missingArtifacts`/recommended action) and machine-readable fail-closed accessibility/error-surface assertions; Story 6.8 QA suite currently passes with 13 tests.

### Change Log

- 2026-04-08: Completed adversarial review triage for Story 6.8, auto-fixed medium fail-closed transport error handling in `apps/operator-console/src/lib/governance/readiness.ts`, and added transport-failure API regression coverage in `tests/api/story-6-8-alpha-governance-readiness-api.test.mjs`.
- 2026-04-08: Ran BMAD QA automation refresh for Story 6.8, added blocked-readiness API regression coverage and fail-closed accessibility/error-surface E2E assertions, and re-ran `npm run --silent qa:test:story-6-8` (13/13 passing).

### File List

- _bmad-output/implementation-artifacts/stories/6-8-deliver-alpha-governance-readiness-card-ux.md
- _bmad-output/implementation-artifacts/sprint-status.yaml
- _bmad-output/implementation-artifacts/tests/test-summary.md
- apps/operator-console/src/app/(dashboard)/dashboard/page.tsx
- apps/operator-console/src/app/(governance)/governance/page.tsx
- apps/operator-console/src/app/globals.css
- apps/operator-console/src/components/governance/AlphaGovernanceReadinessCard.tsx
- apps/operator-console/src/components/governance/GovernanceQueueCard.tsx
- apps/operator-console/src/lib/governance/readiness.ts
- docs/operations/alpha-governance-readiness-card.md
- package.json
- tests/api/story-6-8-alpha-governance-readiness-api.test.mjs
- tests/e2e/story-6-8-alpha-governance-readiness.e2e.test.mjs
- tests/story-6-8/alpha-governance-readiness-card.story-6-8.test.mjs
