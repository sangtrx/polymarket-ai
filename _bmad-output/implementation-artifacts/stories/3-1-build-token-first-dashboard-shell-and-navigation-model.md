# Story 3.1: Build Token-First Dashboard Shell and Navigation Model

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an operator,  
I want a consistent dashboard shell with clear navigation and responsive layout,  
so that I can orient quickly across risk, execution, and governance workflows.

## Acceptance Criteria

1. **Token-first shell and navigation (story-local BDD):**  
   **Given** the operator opens the application  
   **When** core dashboard surfaces render  
   **Then** the UI uses semantic design tokens, approved typography, and responsive grid rules  
   **And** left-rail/top-bar/in-page navigation and mobile policy behavior align with UX-DR1, UX-DR2, UX-DR3, UX-DR4, UX-DR16, UX-DR17, and FR26.
2. **UAC-1 Failure handling:** Missing/unavailable read-model payloads, route-level data errors, or unauthorized shell contexts render explicit non-destructive fallback states (error code + operator-facing message + timestamp) and never present success-shaped placeholders that imply healthy data.
3. **UAC-2 Boundary behavior:** Responsive behavior is deterministic and test-covered for 12/8/4 grid transitions and breakpoints (desktop/tablet/mobile), with mobile monitor-first mode preserving visibility while destructive controls remain disabled by default.
4. **UAC-3 Verifiable evidence:** Shell-level status and panel refresh surfaces expose timestamped freshness evidence suitable for incident/QA traceability (last update time, source, and stale indicator state).
5. **Schema/dependency/traceability contract:** Story depends only on `2.9`, creates no database tables, and maps explicitly to `FR26`, `NFR1`, `UX-DR1`, `UX-DR2`, `UX-DR3`, `UX-DR4`, `UX-DR16`, and `UX-DR17`.
6. **Performance contract (NFR1):** Dashboard shell query surfaces (portfolio summary, risk posture shell slot, active order shell slot) are wired for p95 `<= 2s` query-response expectation under sustained load assumptions, including deterministic loading and stale-state signaling.
7. **Accessibility contract (UX-DR18/UX-DR19 alignment for shell):** Navigation and shell interactions provide keyboard parity, visible focus indicators, semantic landmarks/labels, and reduced-motion respectful transitions for core layout state changes.
8. **Scope boundary contract:** Story 3.1 delivers shell/navigation/token foundations only; persistent safety action rail and risk posture banner behavior are implemented in Story 3.2 and must not be pre-implemented beyond non-interactive placeholders.

## Tasks / Subtasks

- [x] **Task 1: Implement semantic token architecture for dashboard foundations** (AC: 1, 3, 5, 7)
  - [x] Refactor `apps/operator-console/src/styles/tokens.css` into explicit foundation/domain/product token layers (color, typography, spacing, radius, motion, state semantics).
  - [x] Eliminate raw hex usage from feature-level component styling; route all UI colors through semantic variables in token layers.
  - [x] Add risk semantic token family (`normal`, `warning`, `critical`, `locked-safe`) and status-surface token variants used by shell/navigation.
  - [x] Keep token naming stable and workflow-oriented to support upcoming Epic 3 stories without rework.

- [x] **Task 2: Add typography and layout primitives matching UX contracts** (AC: 1, 3, 5, 7)
  - [x] Update app font configuration to align with UX-DR3 (`Source Serif 4` headings, `IBM Plex Sans` body/UI, `IBM Plex Mono` IDs/metrics).
  - [x] Define reusable typography utility classes/tokens for heading/body/metadata metric use in shell surfaces.
  - [x] Implement layout token primitives for 8px spacing rhythm, 8/12/16/24 radius scale, and shell container widths/grid scaffolding.

- [x] **Task 3: Build shared operator shell with left rail and top bar** (AC: 1, 5, 6, 7)
  - [x] Introduce shared shell components (e.g., `components/shell/*`) for left-rail navigation, top status/context bar, and content frame.
  - [x] Ensure shell exposes semantic landmarks (`header`, `nav`, `main`) and keyboard/focus behavior for primary navigation flows.
  - [x] Preserve route ownership (`dashboard`, `incidents`, `governance`) while moving duplicated page wrappers into the shared shell layer.

- [x] **Task 4: Implement in-page tab navigation model for workflow subviews** (AC: 1, 5, 7)
  - [x] Add in-page tab pattern aligned to UX-DR16 for scoped workflow views within dashboard surfaces.
  - [x] Ensure tab semantics are accessible (`aria-controls`, `aria-selected`, roving focus/keyboard navigation).
  - [x] Keep tab state deterministic and URL-aware when possible (query param or segment-safe behavior) without introducing unstable client-only routing hacks.

- [x] **Task 5: Enforce responsive behavior and mobile policy defaults** (AC: 1, 3, 5, 8)
  - [x] Implement deterministic 12/8/4 layout behavior at desktop/tablet/mobile breakpoints with shell-safe stacking rules.
  - [x] Apply monitor-first mobile treatment: prioritize status visibility, limit dense controls, and disable destructive actions by default in mobile view.
  - [x] Add responsive QA checks for dashboard/incident/governance shell views to prevent layout regressions.

- [x] **Task 6: Wire shell refresh/freshness affordances for FR26 + NFR1 readiness** (AC: 2, 4, 6)
  - [x] Add shell-level freshness metadata surface (last updated timestamp + stale indicator) for core operational panels.
  - [x] Standardize loading/error/empty shell states to avoid spinner-only behavior and preserve explicit operator guidance.
  - [x] Keep data fetch contracts read-only and non-mutating in Story 3.1; no privileged control mutations in this story.

- [x] **Task 7: Add Story 3.1 QA workflow and evidence hooks** (AC: 2, 3, 4, 6, 7)
  - [x] Add `qa:test:story-3-1` script in root `package.json` using existing web quality gates (`web:lint`, `web:typecheck`, `web:build`) plus any story-scoped checks implemented.
  - [x] Add/update story evidence entry in `_bmad-output/implementation-artifacts/tests/test-summary.md` after implementation.
  - [x] Verify shell accessibility essentials (keyboard path, focus visibility, reduced-motion behavior) with repeatable checks documented for reviewers.

### Review Findings

- [x] [Review][Patch] Add active-route screen reader context in shell navigation [apps/operator-console/src/components/shell/OperatorShellLayout.tsx:51]
- [x] [Review][Patch] Prevent fallback panel compression and redact production error detail [apps/operator-console/src/app/error.tsx:10]
- [x] [Review][Patch] Normalize and sanitize shell read-model query inputs for state, stale flag, source, and evidence fields [apps/operator-console/src/lib/shell/read-models.ts:59]
- [x] [Review][Patch] Canonicalize invalid tab query values and sanitize duplicate tab IDs in the in-page tab model [apps/operator-console/src/components/shell/InPageTabs.tsx:18]
- [x] [Review][Patch] Add the missing portfolio summary shell slot for AC6 shell-surface coverage [apps/operator-console/src/components/portfolio/PortfolioSummaryCard.tsx:1]
- [x] [Review][Patch] Expand Story 3.1 QA checks for fallback evidence, motion/focus safeguards, and tab canonicalization [tests/story-3-1/operator-shell.story-3-1.test.mjs:1]
- [x] [Review][Defer] Unrelated automation progress log present in git diff outside story scope [.scripts/bmad-auto/copilot/bmad-progress.log] — deferred, pre-existing

## Dev Notes

### Technical Requirements

- Story dependency and scope are strict:
  - Depends only on `2.9`.
  - Creates no database tables/entities.
  - Traceability: `FR26`, `NFR1`, `UX-DR1`, `UX-DR2`, `UX-DR3`, `UX-DR4`, `UX-DR16`, `UX-DR17`.
- Story 3.1 delivers the foundational shell/navigation/token system for Epic 3 and must precede Story 3.2+ surfaces.
- Implement only shell-level structure, navigation model, token primitives, and responsive policy behavior; do not implement safety control actions, incident forensics logic, or allocation/PnL business workflows in this story.
- Preserve universal story contracts:
  - explicit failure handling with machine-readable state,
  - deterministic boundary behavior,
  - timestamped evidence/freshness surfaces.

[Source: _bmad-output/planning-artifacts/epics.md#Story 3.1: Build Token-First Dashboard Shell and Navigation Model]  
[Source: _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)]  
[Source: _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)]  
[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: _bmad-output/planning-artifacts/prd.md#Operations Dashboard & Incident Handling]  
[Source: _bmad-output/planning-artifacts/prd.md#Non-Functional Requirements]

### Architecture Compliance

- Keep architecture ownership boundaries intact:
  - Story 3.1 is UI-shell focused under `apps/operator-console`.
  - No policy/state mutation logic moves into UI shell components.
  - Shell consumes read-model surfaces and status projections only.
- Follow architecture-directed frontend stack and structure:
  - Next.js App Router,
  - token-first “Warm Precision” design model,
  - UI organized by workflow surfaces with explicit route groups.
- Maintain consistency contracts:
  - canonical timestamp rendering in ISO-8601 UTC,
  - deterministic loading/error/critical states,
  - no swallowed errors or optimistic confirmation of critical control states.

[Source: _bmad-output/planning-artifacts/architecture.md#Frontend Architecture]  
[Source: _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules]  
[Source: _bmad-output/planning-artifacts/architecture.md#Architectural Boundaries]

### Library & Framework Requirements

- Keep workspace-pinned UI stack unless a story-scoped blocker is found:
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
  - `typescript`: `6.0.2` (newer major available; no opportunistic upgrade in this story)
  - `eslint-config-next`: `16.2.2` (matches pinned)
- Do not perform opportunistic dependency upgrades in Story 3.1; prioritize deterministic shell delivery and compatibility with existing workspace constraints.

[Source: apps/operator-console/package.json]  
[Source: https://registry.npmjs.org/next/latest]  
[Source: https://registry.npmjs.org/react/latest]  
[Source: https://registry.npmjs.org/react-dom/latest]  
[Source: https://registry.npmjs.org/tailwindcss/latest]  
[Source: https://registry.npmjs.org/typescript/latest]  
[Source: https://registry.npmjs.org/eslint-config-next/latest]

### File Structure Requirements

- Primary implementation surfaces:
  - `apps/operator-console/src/styles/tokens.css`
  - `apps/operator-console/src/app/globals.css`
  - `apps/operator-console/src/app/layout.tsx`
  - `apps/operator-console/src/app/(dashboard)/dashboard/page.tsx`
  - `apps/operator-console/src/app/(incidents)/incidents/page.tsx`
  - `apps/operator-console/src/app/(governance)/governance/page.tsx`
  - `apps/operator-console/src/app/page.tsx`
  - `apps/operator-console/src/components/{risk,execution,governance,timeline}/*.tsx` (shell alignment updates only)
  - `apps/operator-console/src/components/shell/*` (new shared shell/navigation primitives)
  - `package.json` (add `qa:test:story-3-1`)
  - `_bmad-output/implementation-artifacts/tests/test-summary.md`
- Preserve existing route-group conventions (`(dashboard)`, `(incidents)`, `(governance)`) and avoid introducing parallel ad-hoc navigation stacks.
- Keep Story 3.1 limited to shell/navigation/token concerns; Story 3.2+ owns persistent safety rail, risk banner semantics, and interaction-critical controls.

[Source: _bmad-output/planning-artifacts/architecture.md#Complete Project Directory Structure]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: apps/operator-console/src/app/layout.tsx]  
[Source: apps/operator-console/src/app/page.tsx]  
[Source: apps/operator-console/src/styles/tokens.css]

### Testing Requirements

- Add deterministic coverage/checks for:
  - left-rail/top-bar/in-page tab navigation rendering and keyboard accessibility,
  - responsive breakpoint behavior across desktop/tablet/mobile shell layouts,
  - token usage compliance (semantic token classes/variables used instead of raw feature-level colors),
  - loading/error/empty shell states with explicit operator guidance,
  - reduced-motion preference behavior and focus-state visibility.
- Minimum Story 3.1 QA command should run existing web quality gates through `qa:test:story-3-1`.
- Ensure review evidence captures:
  - route screenshots or deterministic output snapshots for all primary shell routes,
  - accessibility check outcomes (keyboard and focus path),
  - responsiveness checks at defined breakpoint ranges.

[Source: _bmad-output/planning-artifacts/ux-design-specification.md#Responsive Design & Accessibility]  
[Source: _bmad-output/planning-artifacts/ux-design-specification.md#Navigation Patterns]  
[Source: _bmad-output/planning-artifacts/ux-design-specification.md#Testing Strategy]  
[Source: package.json]

### Previous Story Intelligence

- Story 1.1 established the operator-console scaffold, route groups, and initial token placeholders; Story 3.1 should evolve these seams instead of replacing structure.
- Stories 2.8 and 2.9 reinforced deterministic state handling, explicit error evidence, and strict scope boundaries; Story 3.1 UI states should expose operational truth clearly and avoid ambiguous “healthy by default” presentation.
- Existing component placeholders already map to epic workflow domains (`risk`, `execution`, `governance`, `timeline`); Story 3.1 should wrap and normalize these surfaces under the shared shell/navigation model.

[Source: _bmad-output/implementation-artifacts/stories/1-1-set-up-initial-project-from-starter-template.md#Completion Notes List]  
[Source: _bmad-output/implementation-artifacts/stories/2-8-enforce-pre-trade-gate-evaluation-pipeline.md#Previous Story Intelligence]  
[Source: _bmad-output/implementation-artifacts/stories/2-9-add-emergency-controls-and-automatic-safe-state-triggers.md#Previous Story Intelligence]  
[Source: apps/operator-console/src/app/page.tsx]  
[Source: apps/operator-console/src/components/risk/RiskPostureCard.tsx]

### Git Intelligence Summary

- Recent implementation commits (2.5 → 2.9) show consistent story-scoped delivery with explicit QA command updates and sprint-status lifecycle transitions.
- Recent changes are backend-heavy; operator-console shell structure remains mostly scaffold-level, making Story 3.1 the first major UI architecture hardening step for Epic 3.
- Preserve disciplined scope: confine Story 3.1 changes to UI shell/navigation/token surfaces plus story-scoped QA wiring.

[Source: git --no-pager log --oneline -5]  
[Source: git --no-pager show --name-only --pretty=format:'%h %s' -5]

### Latest Technical Information

- Current workspace versions already match latest stable for Next.js, React, React DOM, and eslint-config-next.
- Tailwind CSS latest (`4.2.2`) is compatible with existing `^4` constraint.
- TypeScript latest major (`6.0.2`) exceeds current `^5` baseline; defer major upgrade to a dedicated upgrade story to avoid coupling with shell delivery.

[Source: apps/operator-console/package.json]  
[Source: https://registry.npmjs.org/next/latest]  
[Source: https://registry.npmjs.org/react/latest]  
[Source: https://registry.npmjs.org/react-dom/latest]  
[Source: https://registry.npmjs.org/tailwindcss/latest]  
[Source: https://registry.npmjs.org/typescript/latest]  
[Source: https://registry.npmjs.org/eslint-config-next/latest]

### Project Context Reference

- No `project-context.md` file was found in repository scope during discovery.
- Story context was derived from epics, PRD, architecture, UX specification, implementation readiness artifacts, prior story files, git history, and current operator-console source surfaces.

### Project Structure Notes

- `apps/operator-console/src/styles/tokens.css` currently contains raw hex-based placeholders and must be uplifted to full semantic token layering for Story 3.1.
- Root app and route pages currently provide scaffold copy and placeholder cards; introduce shared shell/navigation layout to remove duplicated wrappers.
- Existing route groups and components already provide a stable anchor for shell adoption; prefer incremental composition over route rewrites.
- Keep API integration minimal/read-only in this story; the focus is shell/navigation/token foundations, not control workflow mutations.

[Source: apps/operator-console/src/styles/tokens.css]  
[Source: apps/operator-console/src/app/layout.tsx]  
[Source: apps/operator-console/src/app/page.tsx]  
[Source: apps/operator-console/src/app/(dashboard)/dashboard/page.tsx]  
[Source: apps/operator-console/src/app/(incidents)/incidents/page.tsx]  
[Source: apps/operator-console/src/app/(governance)/governance/page.tsx]

### References

- _bmad-output/planning-artifacts/epics.md#Epic 3: Portfolio Command Center, Alerts & Recovery Operations  
- _bmad-output/planning-artifacts/epics.md#Story 3.1: Build Token-First Dashboard Shell and Navigation Model  
- _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)  
- _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)  
- _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)  
- _bmad-output/planning-artifacts/prd.md#Operations Dashboard & Incident Handling  
- _bmad-output/planning-artifacts/prd.md#Non-Functional Requirements  
- _bmad-output/planning-artifacts/architecture.md#Frontend Architecture  
- _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping  
- _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules  
- _bmad-output/planning-artifacts/architecture.md#Complete Project Directory Structure  
- _bmad-output/planning-artifacts/ux-design-specification.md#Design System Foundation  
- _bmad-output/planning-artifacts/ux-design-specification.md#Navigation Patterns  
- _bmad-output/planning-artifacts/ux-design-specification.md#Responsive Design & Accessibility  
- _bmad-output/planning-artifacts/ux-design-specification.md#Testing Strategy  
- _bmad-output/planning-artifacts/implementation-readiness-report-2026-04-05.md  
- _bmad-output/implementation-artifacts/stories/1-1-set-up-initial-project-from-starter-template.md  
- _bmad-output/implementation-artifacts/stories/2-8-enforce-pre-trade-gate-evaluation-pipeline.md  
- _bmad-output/implementation-artifacts/stories/2-9-add-emergency-controls-and-automatic-safe-state-triggers.md  
- apps/operator-console/package.json  
- apps/operator-console/src/app/layout.tsx  
- apps/operator-console/src/app/page.tsx  
- apps/operator-console/src/styles/tokens.css  
- package.json

## Story Completion Status

- Story context generated with exhaustive artifact analysis across epics, PRD, architecture, UX, prior implementation stories, git history, and current UI source surfaces.
- Story is implementation-ready and status is set to `ready-for-dev`.
- Completion note: Ultimate context engine analysis completed - comprehensive developer guide created.

## Dev Agent Record

### Agent Model Used

GPT-5.3-Codex (gpt-5.3-codex)

### Debug Log References

- BMAD create-story workflow execution (automated)
- Sprint backlog discovery from `_bmad-output/implementation-artifacts/sprint-status.yaml`
- Artifact discovery across planning, implementation, and UI source surfaces
- Latest package version checks via npm registry endpoints

### Completion Notes List

- Delivered semantic foundation/domain/product token layers, risk-state token families, and typography/layout primitives (Source Serif 4 + IBM Plex Sans/Mono) for shell-wide reuse.
- Implemented shared shell architecture (`header`/`nav`/`main`) with left rail, top status/context bar, monitor-first mobile policy, and non-interactive destructive-control placeholder boundary for Story 3.2.
- Added URL-aware accessible in-page tab model with keyboard navigation (`Arrow`/`Home`/`End` + roving focus) across dashboard, incidents, and governance route groups.
- Added deterministic shell read-model fallback surfaces (`loading`, `empty`, `error`, `unauthorized`) with explicit error code, operator guidance, ISO-8601 UTC timestamp, source, and stale indicator evidence.
- Added Story 3.1 QA workflow (`qa:test:story-3-1`) and automation checks covering token architecture, navigation/accessibility semantics, responsive breakpoint policy, and fallback/freshness evidence behavior.
- Expanded Story 3.1 QA automation with dedicated API and E2E suites for shell read-model normalization, evidence contracts, route ownership composition, and navigation manifest integrity.
- Applied adversarial review remediation for accessibility signaling, fallback safety/layout behavior, read-model input hardening, in-page tab canonicalization, and AC6 portfolio summary shell coverage.
- Validation executed: `node --test tests/api/story-3-1*.test.mjs tests/e2e/story-3-1*.test.mjs`; `npm run qa:test:story-3-1`.
- Definition of Done checklist validated for Story 3.1; story is complete.

### File List

- _bmad-output/implementation-artifacts/stories/3-1-build-token-first-dashboard-shell-and-navigation-model.md
- _bmad-output/implementation-artifacts/sprint-status.yaml
- _bmad-output/implementation-artifacts/tests/test-summary.md
- apps/operator-console/src/app/(dashboard)/dashboard/page.tsx
- apps/operator-console/src/app/(governance)/governance/page.tsx
- apps/operator-console/src/app/(incidents)/incidents/page.tsx
- apps/operator-console/src/app/error.tsx
- apps/operator-console/src/app/globals.css
- apps/operator-console/src/app/layout.tsx
- apps/operator-console/src/app/loading.tsx
- apps/operator-console/src/app/page.tsx
- apps/operator-console/src/components/execution/ExecutionSummaryCard.tsx
- apps/operator-console/src/components/governance/GovernanceQueueCard.tsx
- apps/operator-console/src/components/portfolio/PortfolioSummaryCard.tsx
- apps/operator-console/src/components/risk/RiskPostureCard.tsx
- apps/operator-console/src/components/shell/InPageTabs.tsx
- apps/operator-console/src/components/shell/OperatorShellLayout.tsx
- apps/operator-console/src/components/shell/ShellStatePanel.tsx
- apps/operator-console/src/components/shell/ShellTopBar.tsx
- apps/operator-console/src/components/timeline/IncidentTimelineCard.tsx
- apps/operator-console/src/lib/shell/read-models.ts
- apps/operator-console/src/styles/tokens.css
- package.json
- tests/api/story-3-1-shell-read-model-api.test.mjs
- tests/e2e/story-3-1-shell-runtime.e2e.test.mjs
- tests/story-3-1/operator-shell.story-3-1.test.mjs

### Change Log

- 2026-04-06: Created Story 3.1 context file and moved lifecycle state from `backlog` to `ready-for-dev`.
- 2026-04-06: Implemented Story 3.1 shell/navigation/token foundations, route-level fallback evidence states, responsive 12/8/4 policy, and story-scoped QA automation.
- 2026-04-06: Completed adversarial code review triage; auto-fixed HIGH/MEDIUM findings, expanded QA coverage, and advanced status to `done`.
- 2026-04-06: Generated Story 3.1 API + E2E QA automation suites, updated story QA command wiring, and revalidated full Story 3.1 quality gates with 17/17 passing checks.
