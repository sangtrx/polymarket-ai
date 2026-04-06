# Story 3.5: Implement Incident Search and Causal Timeline Forensics

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As a support analyst,  
I want single-query incident search with causal event timelines,  
so that I can diagnose failures and answer "what happened" rapidly.

## Acceptance Criteria

1. **Incident search + causal timeline workflow (story-local BDD):**  
   **Given** incident filters (market/order/alpha/time) are provided  
   **When** the query executes  
   **Then** correlated signal -> order -> fill -> PnL evidence is returned within target latency  
   **And** timeline UX follows UX-DR7, UX-DR11, and UX-DR22 while satisfying FR27 and FR28.
2. **UAC-1 Failure handling:** Invalid filter payloads, unsupported query combinations, unauthorized incident reads, and unavailable reconciliation/attribution dependencies return explicit machine-readable errors with no unsafe side effects and no success-shaped UI fallback.
3. **UAC-2 Boundary behavior:** Incident query boundaries are deterministic and test-covered, including start-inclusive/end-exclusive time windows, stable tie-break ordering for timeline events, and explicit behavior for empty/no-match query windows.
4. **UAC-3 Verifiable evidence:** Successful and failed incident queries include timestamped, correlation-aware evidence fields (`occurred_at`, `source`, `reason_code`, `correlation_id`, and linked run/snapshot identifiers where available) suitable for incident and QA traceability.
5. **Schema/dependency/traceability contract:** Story depends only on `2.6` and `3.4`, creates only `incident_query_views`, and maps explicitly to `FR27`, `FR28`, `NFR16`, `UX-DR7`, `UX-DR11`, and `UX-DR22`.
6. **Latency/SLO contract (FR27 + NFR16):** Correlated incident evidence retrieval paths are designed and measured for p95 `<= 5s` query responses under representative incident workload assumptions, without manual log stitching.
7. **Filter/search contract (FR28 + UX-DR11):** Single-submit incident search supports canonical filters (`market_id`, `order_id`, `alpha_id`, `actor_id`, `start_ts`, `end_ts`) and returns timeline-oriented output with action-ready summaries.
8. **Causal timeline UX contract (UX-DR7 + UX-DR22):** Timeline surfaces preserve Trigger -> Context -> Action -> Verification flow and expose one clear recommended next action when evidence is warning/critical/degraded.
9. **Scope boundary contract:** Story 3.5 delivers incident search + forensics timeline only; severity-based alerting and channel delivery remain in Story 3.6, and controlled recovery gate execution remains in Story 3.7.

## Tasks / Subtasks

- [x] **Task 1: Define incident forensics domain contracts and reason-code taxonomy** (AC: 1, 2, 3, 4, 5, 7, 8)
  - [x] Add incident query/timeline contract types in `crates/domain` (new module wired through `crates/domain/src/lib.rs`) for filter input, timeline event rows, event stages (signal/order/fill/pnl/risk action), and summary evidence metadata.
  - [x] Define deterministic ordering + boundary helpers for timeline event sequencing (`occurred_at` descending for latest-first views, stable key tie-breaks) and explicit empty-window semantics.
  - [x] Add machine-readable reason codes for invalid payload, unauthorized, dependency unavailable, stale evidence, and empty query windows aligned with existing domain error-envelope patterns.

- [x] **Task 2: Add forward-only persistence schema and adapter for incident query views** (AC: 1, 2, 3, 4, 5, 6, 7)
  - [x] Add migration under `crates/persistence/migrations/` creating only `incident_query_views` (table/materialized-view strategy) with UTC timestamp constraints, canonical identifier checks, and deterministic index coverage for filter columns + time windows.
  - [x] Add PostgreSQL adapter module under `crates/persistence/src/postgres/` (and wire via `mod.rs`) to load incident forensics results using canonical typed error mapping.
  - [x] Reuse existing reconciliation and attribution evidence seams (`reconciliation_runs`, `reconciliation_diffs`, `exposure_snapshots`, `attribution_snapshots`) as sources instead of creating duplicate lifecycle truth stores.

- [x] **Task 3: Implement authenticated control-api incident search endpoint** (AC: 1, 2, 4, 6, 7, 8)
  - [x] Add authenticated read route(s) in `services/control-api/src/routes/mod.rs` for incident search/timeline retrieval using canonical response/error envelope structure.
  - [x] Validate and normalize filter query params (`market_id`, `order_id`, `alpha_id`, `actor_id`, `start_ts`, `end_ts`) with explicit machine-readable field errors.
  - [x] Return timeline payload with summary + evidence metadata (`reason_code`, `correlation_id`, `recommended_next_action`, timestamps) and fail closed on unavailable/stale dependencies.

- [x] **Task 4: Add operator-console incident forensics client and state models** (AC: 1, 2, 3, 4, 7, 8)
  - [x] Add typed incident forensics API client under `apps/operator-console/src/lib/incidents/` following existing `lib/portfolio/attribution.ts` parsing and machine-error propagation patterns.
  - [x] Normalize API payloads into explicit UI state machine values (`loading`, `ready`, `empty`, `error`, `critical`) with canonical timestamp parsing and strict contract validation.
  - [x] Preserve shell-level evidence/freshness conventions from Story 3.1 and avoid implicit success defaults for malformed incident payloads.

- [x] **Task 5: Implement causal timeline UI and integrate incidents route** (AC: 1, 3, 4, 7, 8, 9)
  - [x] Replace shell placeholder behavior in `apps/operator-console/src/components/timeline/IncidentTimelineCard.tsx` with timeline-rendering states (loading/empty/error/critical/ready), filter summary, and action-ready guidance.
  - [x] Extend `apps/operator-console/src/app/(incidents)/incidents/page.tsx` to support single-submit incident search + timeline output while preserving existing shell layout and persistent safety rail behavior.
  - [x] Keep Trigger -> Context -> Action -> Verification framing visible in incidents flow and surface recommended next action for warning/critical outcomes.

- [x] **Task 6: Keep dashboard/incidents composition and shell contracts non-regressive** (AC: 3, 8, 9)
  - [x] Maintain Story 3.1 route-group/tab behavior and deterministic fallback rendering in dashboard and incidents surfaces.
  - [x] Preserve Story 3.2 persistent safety action rail and risk posture evidence in incidents context.
  - [x] Preserve Story 3.4 portfolio attribution composition and avoid cross-surface regressions in dashboard tabs.

- [x] **Task 7: Add Story 3.5 QA automation, docs, and evidence updates** (AC: 2, 3, 4, 6, 7, 8, 9)
  - [x] Add `qa:test:story-3-5` script in root `package.json` following Story 3.x conventions (targeted Rust tests + web checks + story/api/e2e suites).
  - [x] Add story-scoped suites:
    - [x] `tests/story-3-5/*.test.mjs` (timeline UI states, action-ready summaries, shell non-regression),
    - [x] `tests/api/story-3-5*.test.mjs` (endpoint contract, filter validation, machine-readable error paths),
    - [x] `tests/e2e/story-3-5*.test.mjs` (incident route flow, latency/fallback behaviors, safety-rail continuity).
  - [x] Update `_bmad-output/implementation-artifacts/tests/test-summary.md` with Story 3.5 evidence and add `docs/operations/incident-search-causal-timeline-forensics.md` runbook guidance.

## Dev Notes

### Technical Requirements

- Story dependency and scope are strict:
  - depends on `2.6` and `3.4`,
  - schema scope limited to `incident_query_views`,
  - traceability scope: `FR27`, `FR28`, `NFR16`, `UX-DR7`, `UX-DR11`, `UX-DR22`.
- Required outcomes for this story:
  - single-submit incident query over canonical incident filters,
  - correlated evidence linking signal -> order -> fill -> PnL (+ risk/action context where present),
  - timeline-oriented output with action-ready summaries and explicit evidence metadata,
  - p95 `<= 5s` post-incident query performance target for representative workloads (NFR16/FR27).
- Universal acceptance obligations from epics must be implemented:
  - explicit machine-readable failure behavior (UAC-1),
  - deterministic boundary semantics and stable ordering (UAC-2),
  - timestamped, queryable evidence across success and failure (UAC-3).
- Include `actor_id` support in search filters to satisfy FR28 PRD scope even when the compact epic BDD line calls out market/order/alpha/time.
- **Out of scope for Story 3.5:** severity alert dispatch/payload workflows (Story 3.6), controlled recovery gate execution (Story 3.7), and backup restore rehearsal orchestration (Story 3.8).

[Source: _bmad-output/planning-artifacts/epics.md#Story 3.5: Implement Incident Search and Causal Timeline Forensics]  
[Source: _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)]  
[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: _bmad-output/planning-artifacts/prd.md#Operations Dashboard & Incident Handling]  
[Source: _bmad-output/planning-artifacts/prd.md#Non-Functional Requirements]

### Architecture Compliance

- Preserve architecture ownership boundaries:
  - `services/control-api` remains authenticated ingress for incident-search reads,
  - `crates/persistence` owns incident-query view persistence/read adapters,
  - `apps/operator-console` owns rendering/state-machine behavior for timeline surfaces,
  - `services/execution-engine`/`risk-engine` continue to own event production and control-state emission.
- Reuse existing canonical evidence sources and avoid reinvention:
  - reconciliation evidence from Story 2.6 (`load_reconciliation_incident_evidence`),
  - attribution evidence from Story 3.4 (`load_latest_attribution_snapshots`),
  - existing correlation-id and reason-code conventions across API/domain/persistence.
- Keep contract and naming patterns intact:
  - snake_case query params for filters (`start_ts`, `end_ts`, `market_id`, etc.),
  - ISO-8601 UTC timestamps only,
  - machine-readable error envelopes with explicit action/endpoint metadata,
  - no swallowed errors in control/query paths.
- Keep incident forensics read-only: no privileged mutation side effects in Story 3.5 query endpoints.

[Source: _bmad-output/planning-artifacts/architecture.md#Data Architecture]  
[Source: _bmad-output/planning-artifacts/architecture.md#API & Communication Patterns]  
[Source: _bmad-output/planning-artifacts/architecture.md#Communication Patterns]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: crates/persistence/src/postgres/reconciliation.rs]  
[Source: crates/persistence/src/postgres/attribution_snapshots.rs]

### Library & Framework Requirements

- Keep workspace-pinned stack for compatibility:
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
- Latest-version checks at story creation time:
  - `next`: `16.2.2` (matches pinned)
  - `react`: `19.2.4` (matches pinned)
  - `react-dom`: `19.2.4` (matches pinned)
  - `tailwindcss`: `4.2.2` (compatible with `^4`)
  - `typescript`: `6.0.2` (newer major available; defer)
  - `tokio`: `1.50.0` (newer minor available; defer)
  - `sqlx`: `0.8.6` (matches pinned)
  - `axum`: `0.8.8` (matches pinned)
  - `polymarket-client-sdk`: `0.4.4` (matches pinned)
  - `time`: `0.3.47` (newer patch available; defer)
- Do not introduce opportunistic dependency upgrades in Story 3.5.

[Source: apps/operator-console/package.json]  
[Source: Cargo.toml]  
[Source: https://registry.npmjs.org/next/latest]  
[Source: https://registry.npmjs.org/react/latest]  
[Source: https://registry.npmjs.org/react-dom/latest]  
[Source: https://registry.npmjs.org/tailwindcss/latest]  
[Source: https://registry.npmjs.org/typescript/latest]  
[Source: https://docs.rs/crate/tokio/latest]  
[Source: https://docs.rs/crate/sqlx/latest]  
[Source: https://docs.rs/crate/axum/latest]  
[Source: https://docs.rs/crate/polymarket-client-sdk/latest]  
[Source: https://docs.rs/crate/time/latest]

### File Structure Requirements

- Primary implementation surfaces (expected Story 3.5 seams):
  - `crates/domain/src/lib.rs`
  - `crates/domain/src/events.rs` (existing envelope extension point) and/or new incident-forensics domain module
  - `crates/persistence/migrations/*incident_query_views*.sql`
  - `crates/persistence/src/postgres/{mod.rs,reconciliation.rs,attribution_snapshots.rs}`
  - `crates/persistence/src/postgres/incident_query_views.rs` (new adapter module)
  - `services/control-api/src/routes/mod.rs`
  - `apps/operator-console/src/app/(incidents)/incidents/page.tsx`
  - `apps/operator-console/src/components/timeline/IncidentTimelineCard.tsx`
  - `apps/operator-console/src/lib/incidents/*.ts` (new typed client + parsers)
  - `apps/operator-console/src/app/globals.css` (timeline/search state styling additions)
  - `tests/story-3-5/*.test.mjs`
  - `tests/api/story-3-5*.test.mjs`
  - `tests/e2e/story-3-5*.test.mjs`
  - `docs/operations/incident-search-causal-timeline-forensics.md`
  - `package.json`
  - `_bmad-output/implementation-artifacts/tests/test-summary.md`
- Preserve established layering pattern used in Story 3.3/3.4: domain -> migration -> persistence -> control-api read contract -> web client parsing -> UI integration -> story QA command.

[Source: _bmad-output/planning-artifacts/architecture.md#Complete Project Directory Structure]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: apps/operator-console/src/app/(incidents)/incidents/page.tsx]  
[Source: apps/operator-console/src/components/timeline/IncidentTimelineCard.tsx]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: crates/persistence/src/postgres/reconciliation.rs]  
[Source: crates/persistence/src/postgres/attribution_snapshots.rs]

### Testing Requirements

- Add deterministic coverage for:
  - incident search filter normalization/validation (`market_id`, `order_id`, `alpha_id`, `actor_id`, `start_ts`, `end_ts`),
  - start-inclusive/end-exclusive boundary semantics and stable timeline ordering on equal timestamps,
  - empty-window responses with explicit `recommended_next_action` (no silent success),
  - machine-readable failures (`400`, `403`, `404`, `503`) with explicit `error_code`, `reason_code`, `correlation_id`, and timestamp evidence,
  - correlated evidence contract completeness (signal/order/fill/pnl chain with source tags and trace identifiers),
  - p95 `<= 5s` incident-query path assertions for representative fixtures,
  - dashboard/incidents non-regression with Story 3.2 risk rail and Story 3.4 attribution surfaces.
- Keep test layering consistent with repository patterns:
  - Rust targeted tests in domain/persistence/control-api,
  - story/API/E2E suites under `tests/`,
  - story-scoped QA command `qa:test:story-3-5`.

[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: _bmad-output/planning-artifacts/prd.md#Operations Dashboard & Incident Handling]  
[Source: _bmad-output/planning-artifacts/prd.md#Non-Functional Requirements]  
[Source: package.json]

### Previous Story Intelligence

- Story 3.4 already implemented cost-aware attribution read contracts and row-level evidence (`snapshot_id`, `run_id`); Story 3.5 should consume those seams for timeline context, not rebuild attribution logic.
- Story 2.6 already implemented reconciliation incident evidence loading (`load_reconciliation_incident_evidence`) and strict run-scoped snapshot correlation; Story 3.5 should build incident query views on this canonical source.
- Story 3.2 established persistent safety rail and confirmation-evidence patterns; incident timeline UX must keep these controls contextually visible during incident diagnosis.
- Story 3.1 established route-group shell composition, deterministic fallback states, and machine-readable evidence rendering patterns that incidents flow should preserve.
- Implementation-readiness warning already highlighted recommended-next-action metadata and response-time SLO drift risk; Story 3.5 should bake both into query contracts and tests.

[Source: _bmad-output/implementation-artifacts/stories/3-4-build-cost-aware-pnl-and-attribution-surfaces.md#Previous Story Intelligence]  
[Source: _bmad-output/implementation-artifacts/stories/2-6-build-reconciliation-and-exposure-visibility-core.md#Project Structure Notes]  
[Source: _bmad-output/implementation-artifacts/stories/3-2-implement-risk-posture-banner-and-persistent-safety-action-rail.md#Completion Notes List]  
[Source: _bmad-output/implementation-artifacts/stories/3-1-build-token-first-dashboard-shell-and-navigation-model.md#Completion Notes List]  
[Source: _bmad-output/planning-artifacts/implementation-readiness-report-2026-04-05.md#Warnings]

### Git Intelligence Summary

- Recent Epic 3 commits (`3-1` -> `3-2` -> `3-3` -> `3-4`) follow a consistent vertical-slice delivery pattern:
  1. domain/persistence contracts,
  2. control-api route + typed client contract,
  3. UI integration with state-machine clarity,
  4. story-scoped QA command + docs/evidence updates.
- Changed-file history confirms this epic favors extending existing seams (shell, control-api routes, portfolio/timeline components) rather than introducing parallel stacks.
- Story 3.5 should preserve this pattern and avoid introducing ad-hoc endpoint schemas or duplicate evidence stores.

[Source: git --no-pager log --oneline -5]  
[Source: git --no-pager log --name-only --pretty=format:'%h %s' -5]

### Latest Technical Information

- Frontend checks confirm pinned Next.js/React/react-dom versions remain current; Tailwind remains compatible with `^4`; TypeScript `6.x` is available but would require a dedicated upgrade story.
- Rust ecosystem checks confirm `sqlx`, `axum`, and `polymarket-client-sdk` pinned versions match latest docs.rs versions; `tokio` and `time` have newer releases but do not require upgrade for Story 3.5.
- No dependency upgrades are required to deliver Story 3.5 safely.

[Source: https://registry.npmjs.org/next/latest]  
[Source: https://registry.npmjs.org/react/latest]  
[Source: https://registry.npmjs.org/react-dom/latest]  
[Source: https://registry.npmjs.org/tailwindcss/latest]  
[Source: https://registry.npmjs.org/typescript/latest]  
[Source: https://docs.rs/crate/tokio/latest]  
[Source: https://docs.rs/crate/sqlx/latest]  
[Source: https://docs.rs/crate/axum/latest]  
[Source: https://docs.rs/crate/polymarket-client-sdk/latest]  
[Source: https://docs.rs/crate/time/latest]

### Project Context Reference

- No `project-context.md` file was found in repository scope during discovery.
- Story context was derived from epics, PRD, architecture, UX specification, implementation-readiness report, research artifacts, previous story files, git history, and current codebase seams.

### Project Structure Notes

- `apps/operator-console/src/components/timeline/IncidentTimelineCard.tsx` and the incidents route currently remain shell placeholders; Story 3.5 is the first implementation pass for true incident-search + causal timeline behavior.
- `services/control-api/src/routes/mod.rs` currently exposes attribution and control workflows but no incident-forensics query route yet; Story 3.5 should add this using existing envelope/auth conventions.
- `crates/persistence/src/postgres/reconciliation.rs` already provides run/diff/snapshot evidence loading and should be reused for timeline evidence composition.
- `tests/story-3-5`, `tests/api/story-3-5*`, and `tests/e2e/story-3-5*` do not exist yet and should be added with story-scoped QA wiring.

[Source: apps/operator-console/src/components/timeline/IncidentTimelineCard.tsx]  
[Source: apps/operator-console/src/app/(incidents)/incidents/page.tsx]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: crates/persistence/src/postgres/reconciliation.rs]  
[Source: package.json]

### References

- _bmad-output/planning-artifacts/epics.md#Epic 3: Portfolio Command Center, Alerts & Recovery Operations  
- _bmad-output/planning-artifacts/epics.md#Story 3.5: Implement Incident Search and Causal Timeline Forensics  
- _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)  
- _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)  
- _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)  
- _bmad-output/planning-artifacts/prd.md#Operations Dashboard & Incident Handling  
- _bmad-output/planning-artifacts/prd.md#Non-Functional Requirements  
- _bmad-output/planning-artifacts/prd.md#Journey 4 — Support/Troubleshooting User: Arjun, Strategy Support Analyst  
- _bmad-output/planning-artifacts/architecture.md#Data Architecture  
- _bmad-output/planning-artifacts/architecture.md#API & Communication Patterns  
- _bmad-output/planning-artifacts/architecture.md#Communication Patterns  
- _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping  
- _bmad-output/planning-artifacts/architecture.md#Complete Project Directory Structure  
- _bmad-output/planning-artifacts/ux-design-specification.md#Causal Timeline Panel  
- _bmad-output/planning-artifacts/ux-design-specification.md#Effortless Interactions  
- _bmad-output/planning-artifacts/ux-design-specification.md#Component Implementation Strategy  
- _bmad-output/planning-artifacts/ux-design-specification.md#Responsive Design & Accessibility  
- _bmad-output/planning-artifacts/implementation-readiness-report-2026-04-05.md#Warnings  
- _bmad-output/planning-artifacts/research/market-rust-polymarket-trading-system-lopez-de-prado-research-2026-04-04.md#Technical Research Scope Confirmation  
- _bmad-output/planning-artifacts/research/domain-polymarket-rust-clob-profit-research-2026-04-04.md#Domain Research Scope Confirmation  
- _bmad-output/implementation-artifacts/stories/2-6-build-reconciliation-and-exposure-visibility-core.md  
- _bmad-output/implementation-artifacts/stories/3-1-build-token-first-dashboard-shell-and-navigation-model.md  
- _bmad-output/implementation-artifacts/stories/3-2-implement-risk-posture-banner-and-persistent-safety-action-rail.md  
- _bmad-output/implementation-artifacts/stories/3-3-deliver-allocation-policy-and-drift-rebalance-workflows.md  
- _bmad-output/implementation-artifacts/stories/3-4-build-cost-aware-pnl-and-attribution-surfaces.md  
- apps/operator-console/src/app/(incidents)/incidents/page.tsx  
- apps/operator-console/src/components/timeline/IncidentTimelineCard.tsx  
- apps/operator-console/src/lib/shell/read-models.ts  
- apps/operator-console/src/lib/portfolio/attribution.ts  
- services/control-api/src/routes/mod.rs  
- crates/domain/src/reconciliation.rs  
- crates/domain/src/attribution.rs  
- crates/persistence/src/postgres/reconciliation.rs  
- crates/persistence/src/postgres/attribution_snapshots.rs  
- crates/persistence/migrations/20260406061000_reconciliation_exposure_core.sql  
- crates/persistence/migrations/20260406154000_attribution_snapshots.sql  
- docs/operations/cost-aware-pnl-attribution-operations.md  
- docs/operations/emergency-safe-state-controls.md  
- package.json  
- apps/operator-console/package.json  
- Cargo.toml  
- https://registry.npmjs.org/next/latest  
- https://registry.npmjs.org/react/latest  
- https://registry.npmjs.org/react-dom/latest  
- https://registry.npmjs.org/tailwindcss/latest  
- https://registry.npmjs.org/typescript/latest  
- https://docs.rs/crate/tokio/latest  
- https://docs.rs/crate/sqlx/latest  
- https://docs.rs/crate/axum/latest  
- https://docs.rs/crate/polymarket-client-sdk/latest  
- https://docs.rs/crate/time/latest

## Story Completion Status

- Story context generated with exhaustive artifact analysis across epic, PRD, architecture, UX, readiness, research, prior stories, git history, and current source seams.
- Story file is created and ready for implementation by dev agents.
- Completion note: Ultimate context engine analysis completed - comprehensive developer guide created.

## Dev Agent Record

### Agent Model Used

GPT-5.3-Codex (gpt-5.3-codex)

### Debug Log References

- BMAD dev-story workflow execution (automated, non-interactive)
- Sprint status transition and resumed implementation for Story 3.5
- Incident-forensics vertical-slice implementation across domain, persistence, API, and operator-console layers
- Story-scoped QA execution via `npm run --silent qa:test:story-3-5`
- Full regression execution via `npm test`
- QA automation refresh for Story 3.5 critical API/E2E coverage via `npm run --silent qa:test:story-3-5`

### Completion Notes List

- Implemented incident-forensics domain contracts in `crates/domain/src/incidents.rs` and wired exports through `crates/domain/src/lib.rs`.
- Added `incident_query_views` migration and PostgreSQL adapter with canonical constraints, deterministic ordering, and reconciliation/attribution evidence reuse.
- Added authenticated `/control/incidents/forensics` route with strict filter validation, machine-readable field errors, and causal timeline response metadata.
- Added typed operator-console incident client and replaced placeholder timeline with explicit loading/ready/empty/error/critical state handling.
- Added Story 3.5 story/API/E2E test suites, `qa:test:story-3-5` command wiring, and incident forensics operations runbook.
- Fixed malformed success-contract assertion drift in `tests/api/story-3-5-incident-api.test.mjs` to match strict parser behavior.
- Remediated code-review findings: corrected datetime-local to UTC conversion, cleared stale timeline snapshots during reload, rendered effective filter summaries, and gated critical success rendering away from failure UI.
- Hardened incident-forensics contracts by preserving degraded severity end-to-end, scoping causal-flow summaries to a single incident chain, and aligning timeline ordering to parsed timestamp semantics.
- Improved persistence path behavior by selecting attribution period from requested window bounds, parallelizing reconciliation evidence fan-out, and returning explicit dependency-unavailable errors when actor-filter fallback evidence is unavailable.
- Added repeatable p95 incident-query measurement coverage and extended Story 3.5 tests for degraded severity, causal-scope isolation, and timezone/filter UX protections.
- Expanded Story 3.5 API/E2E tests for full canonical filter serialization (`market_id`, `order_id`, `alpha_id`, `actor_id`), explicit empty-window (`data_state=empty`) handling, and unauthorized (`403`) machine-error propagation.

### Review Findings

- [x] [Review][Patch] Critical success timelines no longer render as failure surfaces (`apps/operator-console/src/components/timeline/IncidentTimelineCard.tsx`).
- [x] [Review][Patch] Datetime-local input now converts local wall-clock values to UTC ISO timestamps before API dispatch (`apps/operator-console/src/components/timeline/IncidentTimelineCard.tsx`).
- [x] [Review][Patch] Causal-flow summary extraction is scoped to a single incident chain to avoid cross-incident stitching (`services/control-api/src/routes/mod.rs`).
- [x] [Review][Patch] Degraded severity is preserved across API/client contracts and UI state mapping (`services/control-api/src/routes/mod.rs`, `apps/operator-console/src/lib/incidents/forensics.ts`).
- [x] [Review][Patch] Incident query fallback uses query-window-aware attribution period selection with parallel reconciliation evidence loading (`crates/persistence/src/postgres/incident_query_views.rs`).
- [x] [Review][Patch] Actor-filter fallback now fails closed with explicit machine-readable dependency errors instead of false-empty results (`crates/persistence/src/postgres/incident_query_views.rs`).
- [x] [Review][Patch] Migration removed session-timezone-sensitive timestamptz offset checks (`crates/persistence/migrations/20260406193000_incident_query_views.sql`).
- [x] [Review][Patch] Added repeated-query p95 measurement coverage for incident-forensics route (`services/control-api/src/routes/mod.rs`).
- [x] [Review][Defer] `.scripts/bmad-auto/copilot/bmad-progress.log` differs from story file list but is orchestration metadata outside application-source review scope.

### File List

- Cargo.lock
- _bmad-output/implementation-artifacts/stories/3-5-implement-incident-search-and-causal-timeline-forensics.md
- _bmad-output/implementation-artifacts/sprint-status.yaml
- _bmad-output/implementation-artifacts/tests/test-summary.md
- apps/operator-console/src/app/(dashboard)/dashboard/page.tsx
- apps/operator-console/src/app/(incidents)/incidents/page.tsx
- apps/operator-console/src/app/globals.css
- apps/operator-console/src/components/timeline/IncidentTimelineCard.tsx
- apps/operator-console/src/lib/incidents/forensics.ts
- crates/domain/src/incidents.rs
- crates/domain/src/lib.rs
- crates/persistence/Cargo.toml
- crates/persistence/migrations/20260406193000_incident_query_views.sql
- crates/persistence/src/postgres/incident_query_views.rs
- crates/persistence/src/postgres/mod.rs
- docs/operations/incident-search-causal-timeline-forensics.md
- package.json
- services/control-api/src/routes/mod.rs
- tests/api/story-3-5-incident-api.test.mjs
- tests/e2e/story-3-5-incident-timeline.e2e.test.mjs
- tests/story-3-5/incident-forensics.story-3-5.test.mjs

### Change Log

- 2026-04-06: Moved Story 3.5 into active implementation and delivered incident search + causal timeline forensics across domain, persistence, control-api, and operator console surfaces.
- 2026-04-06: Added Story 3.5 QA automation, incident runbook documentation, and aligned malformed contract assertion messaging in API tests.
- 2026-04-06: Completed adversarial code-review remediation for Story 3.5, auto-fixed identified HIGH/MEDIUM findings, and revalidated Story 3.5 + full regression suites.
- 2026-04-06: Refreshed Story 3.5 QA automation with added API/E2E critical-flow coverage and revalidated `qa:test:story-3-5`.
