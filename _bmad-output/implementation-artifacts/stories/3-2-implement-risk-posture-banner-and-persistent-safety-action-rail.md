# Story 3.2: Implement Risk Posture Banner and Persistent Safety Action Rail

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an operator,  
I want always-visible risk status and safety controls,  
so that I can intervene within seconds during risky conditions.

## Acceptance Criteria

1. **Risk posture banner + persistent safety rail (story-local BDD):**  
   **Given** risk state changes or intervention is required  
   **When** the operator views the command center  
   **Then** a stateful risk banner and action rail are visible with explicit control states and confirmations  
   **And** behavior satisfies UX-DR5, UX-DR6, UX-DR9, UX-DR10, UX-DR13, and UX-DR14.
2. **UAC-1 Failure handling:** Invalid/unauthorized safety-control attempts, unavailable control API dependencies, and stale/unavailable read-model inputs surface explicit machine-readable errors with no success-shaped UI and no unsafe side effects.
3. **UAC-2 Boundary behavior:** Story enforces deterministic and test-covered interaction/latency thresholds:
   - risk posture visibility within `<= 5s` of landing,
   - protective action path in `<= 2` interactions,
   - control command acknowledgment expectation `<= 1s`,
   - reflected control state expectation `<= 5s`,
   - timestamped post-action confirmation visibility `<= 10s`.
4. **UAC-3 Verifiable evidence:** Banner/rail flows expose timestamped evidence suitable for incident and QA traceability, including action ID, actor/source, resulting mode, reason code, correlation ID, and audit reference where available.
5. **Schema/dependency/traceability contract:** Story depends only on `3.1`, creates no database tables/entities, and maps explicitly to `FR26`, `NFR2`, `UX-DR5`, `UX-DR6`, `UX-DR9`, `UX-DR10`, `UX-DR13`, and `UX-DR14`.
6. **UX feedback/hierarchy contract:** Safety actions implement explicit button hierarchy and danger confirmation contracts; warning/critical states include one clear recommended next action and required-action guidance.
7. **Accessibility/responsive contract:** Risk banner and safety controls provide keyboard parity, visible focus, screen-reader status updates (`role="status"` with `aria-live` for critical transitions), and reduced-motion-respectful behavior across desktop/tablet/mobile policies.
8. **Control API contract alignment:** Action rail reuses existing emergency control endpoints (`/control/emergency/pause`, `/control/emergency/reduce-only`, `/control/emergency/cancel-all`, `/control/emergency/actions/{action_id}`) and canonical error semantics; no ad-hoc parallel control API contract is introduced.
9. **Scope boundary contract:** `resume` must be present in the rail per UX-DR5 but is readiness-gated (explicitly disabled with rationale) until Story `3.7` introduces controlled recovery gate evaluation and resume execution flow.

## Tasks / Subtasks

- [x] **Task 1: Define operator risk posture + safety action view-model contracts** (AC: 1, 2, 3, 4, 5, 6, 8, 9)
  - [x] Add typed risk posture and safety action models for banner/rail state (`normal`, `warning`, `critical`, `locked-safe`; control button state `enabled|gated|in-progress|completed`), plus deterministic mapping from API/read-model evidence.
  - [x] Normalize machine-readable error/result payload handling so UI state never infers success without explicit confirmation evidence.
  - [x] Reuse Story 3.1 read-model evidence conventions (ISO-8601 UTC timestamps, stable source labels, deterministic stale/error behavior) instead of introducing parallel parsing patterns.

- [x] **Task 2: Implement Risk Posture Banner component with critical-state accessibility semantics** (AC: 1, 3, 4, 6, 7)
  - [x] Add/upgrade a dedicated banner component under `apps/operator-console/src/components/risk/` to render state-specific posture (`normal`, `warning`, `critical`, `locked-safe`) with required-action guidance.
  - [x] Ensure semantic status announcements: `role="status"` and `aria-live` behavior that escalates for critical/locked-safe transitions.
  - [x] Include metadata-first evidence (last update timestamp, source, current mode/reason summary) aligned with FR26 operator visibility expectations.

- [x] **Task 3: Implement persistent Safety Action Rail with confirmation contracts** (AC: 1, 3, 4, 6, 7, 9)
  - [x] Add a persistent rail component (privileged mode) exposing `pause`, `reduce-only`, `cancel-all`, and `resume` controls with explicit enabled/gated/in-progress/completed state rendering.
  - [x] Apply button hierarchy rules (danger vs secondary/tertiary) and keyboard-safe danger confirmations for destructive actions.
  - [x] Keep urgent intervention flows within two interactions while preserving explicit operator intent confirmation for dangerous mutations.
  - [x] Show `resume` as visibly present but readiness-gated with explicit explanatory copy and no fake-success path before Story 3.7.

- [x] **Task 4: Wire rail actions to control-api emergency endpoints and result evidence** (AC: 2, 3, 4, 6, 8, 9)
  - [x] Implement control-api client utilities in operator-console (`apiBaseUrl` from `getOperatorConsoleEnv`) for pause/reduce-only/cancel-all POST flows plus action-result GET lookup.
  - [x] Map accepted and error responses using canonical response fields (`status`, `error_code`, `reason_code`, `action_id`, `resulting_mode`, `timestamp_utc`, `audit_reference`).
  - [x] Provide deterministic post-action feedback states with timestamped confirmation and fallback error guidance when confirmation cannot be resolved in expected windows.
  - [x] Do not invent a resume API route in this story; keep resume interaction strictly gated until recovery-gate story surfaces are implemented.

- [x] **Task 5: Integrate banner + rail into shared shell and route-group surfaces** (AC: 1, 3, 5, 7, 8, 9)
  - [x] Update shared shell composition (`OperatorShellLayout`/`ShellTopBar` and route pages) so banner and safety rail are persistently visible in privileged contexts across `dashboard`, `incidents`, and `governance` routes.
  - [x] Remove Story 3.1 temporary non-interactive destructive-control placeholder copy/button once real rail wiring is active.
  - [x] Preserve Story 3.1 shell invariants: route ownership, navigation semantics, deterministic fallback states, and monitor-first mobile policy.

- [x] **Task 6: Extend tokenized styling for risk/action feedback states without raw color literals** (AC: 1, 6, 7)
  - [x] Extend `tokens.css`/`globals.css` with semantic classes/tokens for banner severity, required-action callouts, rail button hierarchy, and in-progress/completed status surfaces.
  - [x] Keep all feature styling token-driven; no direct hex colors in route/component surfaces.
  - [x] Ensure focus, reduced-motion, and contrast behavior remains compliant with existing shell accessibility conventions.

- [x] **Task 7: Add Story 3.2 QA automation, command wiring, and evidence update path** (AC: 2, 3, 4, 6, 7, 8, 9)
  - [x] Add story-scoped regression suites:
    - `tests/story-3-2/*.test.mjs` for component/contract invariants,
    - `tests/api/story-3-2*.test.mjs` for API client and payload/response mapping behavior,
    - `tests/e2e/story-3-2*.test.mjs` for route-level persistent visibility and action feedback behavior.
  - [x] Add root script `qa:test:story-3-2` to run existing web quality gates plus Story 3.2 tests.
  - [x] Update `_bmad-output/implementation-artifacts/tests/test-summary.md` with Story 3.2 generated coverage and execution evidence during implementation.

### Review Findings

- [x] [Review][Patch] Prevent duplicate dangerous-action submissions with single-flight guard and busy-state confirmation controls [apps/operator-console/src/components/risk/SafetyActionRail.tsx:188]
- [x] [Review][Patch] Replace single-shot action-result fetch with bounded confirmation polling and deterministic timeout behavior [apps/operator-console/src/components/risk/SafetyActionRail.tsx:135]
- [x] [Review][Patch] Convert latency threshold breaches to explicit warning evidence instead of success-shaped failure paths [apps/operator-console/src/components/risk/SafetyActionRail.tsx:101]
- [x] [Review][Patch] Add alert-dialog labeling semantics for destructive confirmation flow accessibility [apps/operator-console/src/components/risk/SafetyActionRail.tsx:361]
- [x] [Review][Patch] Resynchronize local risk posture state when shell props change [apps/operator-console/src/components/risk/RiskCommandSurface.tsx:20]
- [x] [Review][Patch] Add actor/source evidence rendering to the risk posture banner [apps/operator-console/src/components/risk/RiskPostureBanner.tsx:49]
- [x] [Review][Patch] Harden emergency action result lookup with action-id validation and URL encoding [apps/operator-console/src/lib/risk/control-actions.ts:299]
- [x] [Review][Patch] Harden client control-api base URL resolution for browser/runtime environments [apps/operator-console/src/lib/env.ts:9]

## Dev Notes

### Technical Requirements

- Story scope is strictly UI/control-surface focused:
  - Depends only on `3.1`.
  - Creates no database tables/entities.
  - Traceability: `FR26`, `NFR2`, `UX-DR5`, `UX-DR6`, `UX-DR9`, `UX-DR10`, `UX-DR13`, `UX-DR14`.
- Required story outcomes:
  - persistent safety controls for privileged users,
  - explicit risk posture banner states (`normal`, `warning`, `critical`, `locked-safe`),
  - fast intervention UX with deterministic confirmation evidence.
- Existing emergency control backend from Story 2.9 already provides pause/reduce-only/cancel-all + action-result contracts; Story 3.2 should integrate those seams rather than introducing duplicate orchestration logic.
- Readiness warning from implementation-readiness analysis must be addressed in this story: include explicit recommended-next-action guidance and confirmation metadata in UX/API handling so UX-DR14/21/22 alignment does not drift.
- **Out of scope for Story 3.2:** controlled recovery gate evaluation and resume execution (Story 3.7), severity alert delivery workflows (Story 3.6), and incident forensics search/timeline query implementation depth (Story 3.5).

[Source: _bmad-output/planning-artifacts/epics.md#Story 3.2: Implement Risk Posture Banner and Persistent Safety Action Rail]  
[Source: _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)]  
[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: _bmad-output/planning-artifacts/prd.md#Operations Dashboard & Incident Handling]  
[Source: _bmad-output/planning-artifacts/prd.md#Non-Functional Requirements]  
[Source: _bmad-output/planning-artifacts/implementation-readiness-report-2026-04-05.md#Warnings]

### Architecture Compliance

- Keep architecture ownership boundaries intact:
  - operator-console owns interaction/rendering surfaces,
  - control-api remains the only privileged control ingress,
  - governance/risk/execution backend boundaries remain unchanged.
- Reuse canonical control response/error envelope behavior and machine-readable reason semantics from control-api routes.
- Preserve shell contracts established in Story 3.1:
  - deterministic fallback behavior,
  - explicit evidence surfaces,
  - no optimistic success states for critical controls.
- Maintain timestamp/identifier conventions (ISO-8601 UTC, correlation-friendly metadata) for all operator-visible action confirmations.

[Source: _bmad-output/planning-artifacts/architecture.md#API & Communication Patterns]  
[Source: _bmad-output/planning-artifacts/architecture.md#Architectural Boundaries]  
[Source: _bmad-output/planning-artifacts/architecture.md#Integration Points]  
[Source: _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules]  
[Source: services/control-api/src/routes/mod.rs]

### Library & Framework Requirements

- Keep workspace-pinned frontend stack for compatibility:
  - `next = 16.2.2`
  - `react = 19.2.4`
  - `react-dom = 19.2.4`
  - `tailwindcss = ^4`
  - `eslint-config-next = 16.2.2`
  - `typescript = ^5`
- Latest-version checks at story creation time:
  - `next`: `16.2.2` (matches pinned)
  - `react`: `19.2.4` (matches pinned)
  - `react-dom`: `19.2.4` (matches pinned)
  - `tailwindcss`: `4.2.2` (compatible with `^4`)
  - `typescript`: `6.0.2` (newer major available; defer)
  - `eslint-config-next`: `16.2.2` (matches pinned)
- Do not do opportunistic dependency upgrades in Story 3.2.

[Source: apps/operator-console/package.json]  
[Source: https://registry.npmjs.org/next/latest]  
[Source: https://registry.npmjs.org/react/latest]  
[Source: https://registry.npmjs.org/react-dom/latest]  
[Source: https://registry.npmjs.org/tailwindcss/latest]  
[Source: https://registry.npmjs.org/typescript/latest]  
[Source: https://registry.npmjs.org/eslint-config-next/latest]

### File Structure Requirements

- Primary implementation surfaces:
  - `apps/operator-console/src/components/risk/RiskPostureCard.tsx` (replace/upgrade shell slot behavior)
  - `apps/operator-console/src/components/risk/*` (new banner/rail/action primitives as needed)
  - `apps/operator-console/src/components/shell/{OperatorShellLayout.tsx,ShellTopBar.tsx}`
  - `apps/operator-console/src/app/(dashboard)/dashboard/page.tsx`
  - `apps/operator-console/src/app/(incidents)/incidents/page.tsx`
  - `apps/operator-console/src/app/(governance)/governance/page.tsx`
  - `apps/operator-console/src/lib/{env.ts,shell/read-models.ts}` plus any new control-api client helper under `src/lib/`
  - `apps/operator-console/src/styles/tokens.css`
  - `apps/operator-console/src/app/globals.css`
  - `tests/story-3-2/*`
  - `tests/api/story-3-2*.test.mjs`
  - `tests/e2e/story-3-2*.test.mjs`
  - `package.json` (add `qa:test:story-3-2`)
  - `_bmad-output/implementation-artifacts/tests/test-summary.md`
- Preserve route-group conventions and shared shell composition; do not create parallel navigation or ad-hoc page wrappers.
- Keep backend-service code out of scope unless unavoidable interface extension is explicitly required by control-api contract mismatch.

[Source: _bmad-output/planning-artifacts/architecture.md#Complete Project Directory Structure]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: apps/operator-console/src/components/shell/OperatorShellLayout.tsx]  
[Source: apps/operator-console/src/app/(dashboard)/dashboard/page.tsx]  
[Source: apps/operator-console/src/lib/env.ts]  
[Source: services/control-api/src/routes/mod.rs]

### Testing Requirements

- Add deterministic coverage for:
  - banner state mapping and announcement semantics (`role=status`, `aria-live` escalation),
  - persistent rail visibility and control-state transitions across route groups,
  - danger confirmation and button hierarchy behavior (including keyboard paths),
  - control-api request/response mapping for pause/reduce-only/cancel-all and action-result query,
  - explicit failure handling for unauthorized, invalid payload, service unavailable, and not-found action-result paths,
  - boundary contracts (`<=2` interactions, confirmation timing evidence, mobile monitor-first policy),
  - resume readiness-gated behavior (visible but blocked with explicit rationale).
- Add Story 3.2 QA command in root package and ensure it runs existing web quality gates before story-scoped tests.
- Capture test-summary evidence for reviewers in `_bmad-output/implementation-artifacts/tests/test-summary.md`.

[Source: _bmad-output/planning-artifacts/ux-design-specification.md#Flow Optimization Principles]  
[Source: _bmad-output/planning-artifacts/ux-design-specification.md#Risk Posture Banner]  
[Source: _bmad-output/planning-artifacts/ux-design-specification.md#Action Rail (Safety Controls)]  
[Source: _bmad-output/planning-artifacts/ux-design-specification.md#Button Hierarchy]  
[Source: _bmad-output/planning-artifacts/ux-design-specification.md#Feedback Patterns]  
[Source: _bmad-output/planning-artifacts/ux-design-specification.md#Responsive Design & Accessibility]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: package.json]

### Previous Story Intelligence

- Story 3.1 intentionally deferred interactive safety controls and implemented placeholder/non-destructive copy in shell surfaces; Story 3.2 must replace those placeholders with real banner/rail behavior without regressing shell accessibility and fallback guarantees.
- Story 3.1 established token-first styling, route-group shell composition, keyboard-capable tabs, and deterministic read-model fallback/evidence semantics; Story 3.2 should extend these patterns rather than introducing a second UI architecture.
- Story 2.9 already implemented emergency control orchestration and canonical API contracts for pause/reduce-only/cancel-all; Story 3.2 should consume those existing endpoints and reason codes directly.
- Resume execution is not yet implemented in control-api/governance seams; Story 3.2 should keep resume visibly present but gated until Story 3.7 recovery workflow delivery.

[Source: _bmad-output/implementation-artifacts/stories/3-1-build-token-first-dashboard-shell-and-navigation-model.md#Scope boundary contract]  
[Source: _bmad-output/implementation-artifacts/stories/3-1-build-token-first-dashboard-shell-and-navigation-model.md#Project Structure Notes]  
[Source: _bmad-output/implementation-artifacts/stories/2-9-add-emergency-controls-and-automatic-safe-state-triggers.md#Technical Requirements]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: services/governance-service/src/safety_controls/mod.rs]

### Git Intelligence Summary

- Recent commit sequence (`2.6` → `2.9` → `3.1`) follows a consistent pattern: story-scoped vertical slices, explicit QA command wiring, deterministic error/evidence behavior, and sprint-status lifecycle updates.
- Most recent work introduced shell/UI foundations (`3.1`) plus hardened emergency backend contracts (`2.9`), making Story 3.2 the intended integration point between these two tracks.
- Keep Story 3.2 tightly scoped to operator-console + story QA surfaces, matching established commit discipline.

[Source: git --no-pager log --oneline -5]  
[Source: git --no-pager log --name-only --pretty=format:'%h %s' -5]

### Latest Technical Information

- Current workspace versions already align with latest stable releases for Next.js/React/react-dom/eslint-config-next.
- Tailwind latest stable (`4.2.2`) remains compatible with existing `^4` constraint.
- TypeScript latest (`6.0.2`) is a newer major than workspace `^5`; defer major migration to dedicated dependency story.
- No stack upgrade is required to deliver Story 3.2.

[Source: apps/operator-console/package.json]  
[Source: https://registry.npmjs.org/next/latest]  
[Source: https://registry.npmjs.org/react/latest]  
[Source: https://registry.npmjs.org/react-dom/latest]  
[Source: https://registry.npmjs.org/tailwindcss/latest]  
[Source: https://registry.npmjs.org/typescript/latest]  
[Source: https://registry.npmjs.org/eslint-config-next/latest]

### Project Context Reference

- No `project-context.md` file was found in repository scope during discovery.
- Story context was derived from epics, PRD, architecture, UX specification, implementation readiness findings, previous implementation stories, git history, and current operator-console/control-api source surfaces.

### Project Structure Notes

- `ShellTopBar` currently contains a disabled destructive-controls placeholder that should be replaced by the real safety rail in this story.
- `RiskPostureCard` is currently a shell placeholder and should evolve into a true risk posture banner+evidence surface.
- Route pages already include consistent shell composition and should remain the integration seam for persistent rail visibility.
- `getOperatorConsoleEnv()` already provides control-api base URL wiring (`OPERATOR_CONSOLE_PUBLIC_API_BASE_URL` with localhost fallback).
- Emergency control endpoints exist for pause/reduce-only/cancel-all/action lookup; no resume endpoint currently exists.

[Source: apps/operator-console/src/components/shell/ShellTopBar.tsx]  
[Source: apps/operator-console/src/components/risk/RiskPostureCard.tsx]  
[Source: apps/operator-console/src/app/(dashboard)/dashboard/page.tsx]  
[Source: apps/operator-console/src/app/(incidents)/incidents/page.tsx]  
[Source: apps/operator-console/src/app/(governance)/governance/page.tsx]  
[Source: apps/operator-console/src/lib/env.ts]  
[Source: services/control-api/src/routes/mod.rs]

### References

- _bmad-output/planning-artifacts/epics.md#Epic 3: Portfolio Command Center, Alerts & Recovery Operations  
- _bmad-output/planning-artifacts/epics.md#Story 3.2: Implement Risk Posture Banner and Persistent Safety Action Rail  
- _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)  
- _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)  
- _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)  
- _bmad-output/planning-artifacts/prd.md#Operations Dashboard & Incident Handling  
- _bmad-output/planning-artifacts/prd.md#Risk & Capital Management  
- _bmad-output/planning-artifacts/prd.md#Non-Functional Requirements  
- _bmad-output/planning-artifacts/architecture.md#Frontend Architecture  
- _bmad-output/planning-artifacts/architecture.md#API & Communication Patterns  
- _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries  
- _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping  
- _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules  
- _bmad-output/planning-artifacts/ux-design-specification.md#Critical Success Moments  
- _bmad-output/planning-artifacts/ux-design-specification.md#Risk Posture Banner  
- _bmad-output/planning-artifacts/ux-design-specification.md#Action Rail (Safety Controls)  
- _bmad-output/planning-artifacts/ux-design-specification.md#Flow Optimization Principles  
- _bmad-output/planning-artifacts/ux-design-specification.md#Button Hierarchy  
- _bmad-output/planning-artifacts/ux-design-specification.md#Feedback Patterns  
- _bmad-output/planning-artifacts/ux-design-specification.md#Responsive Design & Accessibility  
- _bmad-output/planning-artifacts/implementation-readiness-report-2026-04-05.md#Warnings  
- _bmad-output/implementation-artifacts/stories/3-1-build-token-first-dashboard-shell-and-navigation-model.md  
- _bmad-output/implementation-artifacts/stories/2-9-add-emergency-controls-and-automatic-safe-state-triggers.md  
- apps/operator-console/package.json  
- apps/operator-console/src/lib/env.ts  
- apps/operator-console/src/lib/shell/read-models.ts  
- apps/operator-console/src/components/risk/RiskPostureCard.tsx  
- apps/operator-console/src/components/shell/ShellTopBar.tsx  
- apps/operator-console/src/components/shell/OperatorShellLayout.tsx  
- services/control-api/src/routes/mod.rs  
- services/governance-service/src/safety_controls/mod.rs  
- package.json  
- https://registry.npmjs.org/next/latest  
- https://registry.npmjs.org/react/latest  
- https://registry.npmjs.org/react-dom/latest  
- https://registry.npmjs.org/tailwindcss/latest  
- https://registry.npmjs.org/typescript/latest  
- https://registry.npmjs.org/eslint-config-next/latest

## Story Completion Status

- Story context generated with exhaustive artifact analysis across epics, PRD, architecture, UX, readiness findings, previous story implementation intelligence, git history, and current code surfaces.
- Story implementation and adversarial code review are complete; lifecycle status is set to `done`.
- Completion note: Story 3.2 now includes post-review hardening for confirmation reliability, action-id safety, and operator evidence/accessibility coverage.

## Dev Agent Record

### Agent Model Used

GPT-5.3-Codex (gpt-5.3-codex)

### Debug Log References

- BMAD create-story workflow execution (automated)
- Sprint backlog discovery from `_bmad-output/implementation-artifacts/sprint-status.yaml`
- Artifact discovery across planning, implementation, and operator-console/control-api source surfaces
- Latest package version checks via npm registry endpoints
- Story 3.2 red-green-refactor cycle using story-scoped tests under `tests/story-3-2`, `tests/api/story-3-2*`, and `tests/e2e/story-3-2*`
- `npm run qa:test:story-3-1` and `npm run qa:test:story-3-2` validation runs after implementation
- `npm test` full-suite regression run after review remediation (including `rust:test`)

### Completion Notes List

- Generated complete Story 3.2 implementation guide with explicit AC/UAC coverage, architecture guardrails, and file-level delivery plan.
- Captured cross-story constraints from Story 3.1 and Story 2.9 to prevent reinvention, API drift, and unsafe control UX behavior.
- Documented resume-control gating as a known dependency on Story 3.7 to avoid fake/unsafe reactivation flows.
- Implemented typed risk posture and safety-control view models, machine-readable control API client mapping, and deterministic evidence handling for accepted/error payloads.
- Added persistent risk command surface (`RiskPostureBanner` + `SafetyActionRail`) to the shared shell with accessibility semantics (`role="status"`, `aria-live`) and explicit timing/interaction contracts.
- Integrated banner/rail across dashboard, incidents, and governance routes; removed Story 3.1 destructive-control placeholder copy while preserving shell route ownership conventions.
- Extended token-driven styling for risk/action states, hierarchy, in-progress/completed states, and confirmation/error evidence presentation.
- Added Story 3.2 QA automation command and regression suites; refreshed Story 3.1 top-bar assertions to match persistent rail behavior.
- Adversarial review hardening pass: added single-flight action guard, bounded confirmation polling, explicit latency warning evidence, and accessibility labeling for dangerous-action confirmation surfaces.
- Added action-result path safety by validating and URL-encoding `action_id` and added regression coverage for malformed action-id rejection.
- Added risk banner actor/source evidence and synchronized `RiskCommandSurface` local posture state with incoming shell posture props.
- Hardened browser control-api base URL resolution to support `NEXT_PUBLIC_OPERATOR_CONSOLE_API_BASE_URL` with environment-safe fallback behavior.
- Expanded Story 3.2 QA automation coverage for critical failure boundaries (403 unauthorized, 503 dependency unavailable, 404 action-result not found), stale read-model warning posture fallback, and safety-rail confirmation/error evidence semantics.

### File List

- _bmad-output/implementation-artifacts/stories/3-2-implement-risk-posture-banner-and-persistent-safety-action-rail.md
- _bmad-output/implementation-artifacts/sprint-status.yaml
- _bmad-output/implementation-artifacts/tests/test-summary.md
- apps/operator-console/src/lib/risk/posture.ts
- apps/operator-console/src/lib/risk/control-actions.ts
- apps/operator-console/src/components/risk/RiskPostureBanner.tsx
- apps/operator-console/src/components/risk/SafetyActionRail.tsx
- apps/operator-console/src/components/risk/RiskCommandSurface.tsx
- apps/operator-console/src/components/shell/OperatorShellLayout.tsx
- apps/operator-console/src/components/shell/ShellTopBar.tsx
- apps/operator-console/src/lib/env.ts
- apps/operator-console/src/app/(dashboard)/dashboard/page.tsx
- apps/operator-console/src/app/(incidents)/incidents/page.tsx
- apps/operator-console/src/app/(governance)/governance/page.tsx
- apps/operator-console/src/styles/tokens.css
- apps/operator-console/src/app/globals.css
- package.json
- tests/story-3-2/risk-command-surface.story-3-2.test.mjs
- tests/api/story-3-2-safety-control-api.test.mjs
- tests/e2e/story-3-2-risk-safety-rail.e2e.test.mjs
- tests/story-3-1/operator-shell.story-3-1.test.mjs
- tests/e2e/story-3-1-shell-runtime.e2e.test.mjs

### Change Log

- 2026-04-06: Created Story 3.2 context file and moved lifecycle state from `backlog` to `ready-for-dev`.
- 2026-04-06: Implemented Story 3.2 risk posture banner + persistent safety action rail, wired control API confirmation/evidence flows, added story-scoped QA automation, and advanced lifecycle to `review` (full `npm test` run blocked by missing `cargo` binary in this environment).
- 2026-04-06: Completed adversarial review remediation pass (single-flight controls, bounded confirmation polling, latency warning evidence, action-id validation/encoding, actor/source evidence, posture resync, env base-url hardening), reran Story 3.2 + full workspace tests, and advanced lifecycle to `done`.
- 2026-04-06: Ran BMAD QA automation execution for Story 3.2, added API/E2E critical-flow regression tests (dependency unavailable, action-result not found, stale-input fallback, evidence/confirmation semantics), and reran `npm run qa:test:story-3-2` with passing results while keeping lifecycle status `done`.
