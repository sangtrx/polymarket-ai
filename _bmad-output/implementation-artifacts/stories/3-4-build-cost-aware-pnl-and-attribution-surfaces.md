# Story 3.4: Build Cost-Aware PnL and Attribution Surfaces

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an operator,  
I want granular realized/unrealized PnL and cost-aware attribution views,  
so that I can understand performance drivers quickly.

## Acceptance Criteria

1. **Cost-aware attribution surface (story-local BDD):**  
   **Given** portfolio and execution data are available  
   **When** attribution views are queried  
   **Then** users can inspect PnL by market, alpha, and period with cost-aware breakdowns  
   **And** modules implement FR24/FR25 with metadata-first cards plus skeleton/empty states per UX-DR12 and UX-DR20.
2. **UAC-1 Failure handling:** Invalid/unsupported period filters, malformed query params, unauthorized attribution reads, unavailable projection dependencies, and stale/unavailable reconciliation inputs return explicit machine-readable errors with no success-shaped UI and no unsafe side effects.
3. **UAC-2 Boundary behavior:** Time-window and cost-decomposition behavior is deterministic and test-covered, including:
   - canonical period windows (`1h`, `24h`, `30d`) with explicit boundary semantics (`start_inclusive`, `end_exclusive`),
   - deterministic ordering and tie-breaks for attribution rows (`period_end_utc DESC`, then stable key ordering),
   - explicit handling of zero-activity windows (empty-state with actionable guidance, not silent zero-filled ambiguity).
4. **UAC-3 Verifiable evidence:** Attribution responses and UI surfaces expose timestamped evidence fields (`as_of_utc`, `source`, `reason_code`, `correlation_id`, snapshot/run identifiers where present) suitable for incident and QA traceability.
5. **Schema/dependency/traceability contract:** Story depends only on `2.6` and `3.1`; creates only `attribution_snapshots`; maps explicitly to `FR24`, `FR25`, `NFR1`, `UX-DR12`, and `UX-DR20`.
6. **Metadata-first card contract (UX-DR12):** PnL/attribution cards render metadata (timestamp/category/status) before narrative detail, with one clear recommended next action when data is warning/critical/degraded.
7. **Loading and empty-state contract (UX-DR20):** Attribution UI uses skeleton states during fetch and actionable next-step messaging for empty/no-data conditions; spinner-only fallback is not allowed.
8. **NFR1 query-path contract:** Dashboard attribution query paths are designed for p95 `<= 2s` operating target at sustained load assumptions and include explicit stale/degraded signaling when freshness or dependency health cannot satisfy target.
9. **Scope boundary contract:** Story 3.4 delivers attribution and cost-aware PnL surfaces only; incident forensic query/timeline workflow depth remains in Story 3.5 and alert/recovery workflows remain in Stories 3.6-3.7.

## Tasks / Subtasks

- [x] **Task 1: Define attribution domain contracts and reason-code taxonomy** (AC: 1, 2, 3, 4, 5, 8)
  - [x] Add attribution domain types in `crates/domain` (new `attribution` module or equivalent) for period scope, realized/unrealized decomposition, fee/rebate/incentive components, and row-level evidence metadata.
  - [x] Define deterministic aggregation and ordering helpers for market/alpha/period breakdowns, including explicit zero-activity window behavior.
  - [x] Add machine-readable attribution reason codes aligned to existing error/evidence conventions (invalid payload, unauthorized, persistence unavailable, projection unavailable, stale source).

- [x] **Task 2: Add forward-only persistence schema and adapter for attribution snapshots** (AC: 1, 2, 3, 4, 5)
  - [x] Add migration for `attribution_snapshots` only under `crates/persistence/migrations/`, with constraints for canonical identifiers, finite numeric fields, and UTC timestamp columns.
  - [x] Implement PostgreSQL adapter module under `crates/persistence/src/postgres/` with deterministic query ordering for latest-by-scope retrieval.
  - [x] Wire module through `crates/persistence/src/postgres/mod.rs` and enforce typed error mapping (no swallowed decode/constraint/tx errors).

- [x] **Task 3: Implement portfolio-engine attribution runtime seam** (AC: 1, 2, 3, 4, 8)
  - [x] Replace Story 3 attribution placeholder in `services/portfolio-engine/src/attribution/mod.rs` with aggregation/read-model builders that consume canonical reconciliation/execution evidence.
  - [x] Reuse Story 2.6 exposure/reconciliation seams as input context; avoid introducing duplicate lifecycle truth sources.
  - [x] Emit attribution read models carrying evidence metadata needed by operator surfaces (`as_of_utc`, `correlation_id`, source identifiers, reason code).

- [x] **Task 4: Extend control-api with attribution read contracts** (AC: 1, 2, 3, 4, 8)
  - [x] Add authenticated read endpoint(s) for attribution query in `services/control-api/src/routes/mod.rs` using canonical response/error envelope patterns.
  - [x] Support market/alpha/period filters with strict validation and deterministic fallback semantics.
  - [x] Keep endpoint behavior read-only; no privileged mutation side effects.

- [x] **Task 5: Add operator-console attribution client and view models** (AC: 1, 2, 3, 4, 6, 7)
  - [x] Add `apps/operator-console/src/lib/portfolio/attribution.ts` for typed API integration and machine-readable error propagation.
  - [x] Normalize response payloads into metadata-first view models and explicit UI state machine (`loading`, `ready`, `empty`, `error`, `critical`).
  - [x] Preserve existing Story 3.1 shell state conventions and Story 3.3 portfolio client patterns.

- [x] **Task 6: Build metadata-first PnL/attribution UI surfaces and integrate dashboard composition** (AC: 1, 2, 4, 6, 7, 8, 9)
  - [x] Replace `PortfolioSummaryCard` placeholder shell copy with implemented PnL/attribution card/table surfaces while preserving existing Story 3.3 allocation/rebalance panels.
  - [x] Add skeleton and empty-state components with actionable next-step messaging; avoid spinner-only fallback.
  - [x] Extend `apps/operator-console/src/app/globals.css` (and token usage) with attribution-specific classes that remain token-driven.
  - [x] Keep integration compatible with persistent risk banner/action-rail behavior from Story 3.2.

- [x] **Task 7: Add Story 3.4 QA automation and command wiring** (AC: 2, 3, 4, 6, 7, 8)
  - [x] Add `qa:test:story-3-4` in root `package.json`, following Story 3.x command conventions.
  - [x] Add story-scoped tests under:
    - `tests/story-3-4/*.test.mjs` (UI composition, metadata-first ordering, skeleton/empty behavior),
    - `tests/api/story-3-4*.test.mjs` (API contract, filter validation, machine-readable errors),
    - `tests/e2e/story-3-4*.test.mjs` (dashboard integration and non-regression with Story 3.2/3.3 surfaces).
  - [x] Update `_bmad-output/implementation-artifacts/tests/test-summary.md` with Story 3.4 evidence.

- [x] **Task 8: Add operator-facing guidance and non-regression safeguards** (AC: 4, 6, 7, 9)
  - [x] Add or update `docs/operations/*attribution*` guidance for interpreting cost-aware decomposition and degraded data states.
  - [x] Document known story boundaries and handoff notes for Story 3.5 forensic timeline dependencies.

### Review Findings

- [x] [Review][Patch] Preserve server-generated timestamps for attribution audit/response envelopes and prevent client-supplied `as_of_utc` from mutating audit event time [services/control-api/src/routes/mod.rs]
- [x] [Review][Patch] Emit explicit machine-readable `attribution_unauthorized` envelopes for denied attribution reads [services/control-api/src/routes/mod.rs]
- [x] [Review][Patch] Prefer persisted attribution snapshot reads when a durable PostgreSQL pool is available; keep synthetic observations only for isolated test-state fallback [services/control-api/Cargo.toml, services/control-api/src/main.rs, services/control-api/src/middleware/mod.rs, services/control-api/src/routes/mod.rs]
- [x] [Review][Patch] Fail closed on malformed required timestamp fields in attribution success payloads (no success-shaped timestamp fallback) [apps/operator-console/src/lib/portfolio/attribution.ts, tests/api/story-3-4-attribution-api.test.mjs]
- [x] [Review][Patch] Expose row-level `snapshot_id` and `run_id` evidence in attribution table rendering [apps/operator-console/src/components/portfolio/PnlAttributionBreakdownTable.tsx, tests/story-3-4/pnl-attribution.story-3-4.test.mjs]
- [x] [Review][Patch] Correct snapshot ID decode mapping, strengthen financial consistency constraints, enforce non-blank optional filters, and preserve row-level period ordering semantics [crates/persistence/src/postgres/attribution_snapshots.rs, crates/persistence/migrations/20260406154000_attribution_snapshots.sql, crates/domain/src/attribution.rs, services/portfolio-engine/src/attribution/mod.rs]
- [x] [Review][Defer] Git working-tree also includes `.scripts/bmad-auto/copilot/bmad-progress.log`; treated as non-application automation artifact and excluded from story code-review scope.

## Dev Notes

### Technical Requirements

- Story dependency and scope are strict:
  - depends on `2.6` and `3.1`,
  - schema scope limited to `attribution_snapshots`,
  - traceability scope: `FR24`, `FR25`, `NFR1`, `UX-DR12`, `UX-DR20`.
- Required outcomes for this story:
  - realized/unrealized PnL visibility by market, alpha, and period (`FR24`),
  - cost-aware net attribution decomposition (`FR25`),
  - dashboard-aligned query-path behavior consistent with p95 `<= 2s` target (`NFR1`),
  - metadata-first presentation and skeleton/empty UX contracts (`UX-DR12`, `UX-DR20`).
- Enforce universal acceptance criteria from epics:
  - explicit machine-readable failure behavior (UAC-1),
  - deterministic boundary semantics (UAC-2),
  - timestamped traceability evidence (UAC-3).
- **Out of scope for Story 3.4:** incident search/timeline forensics (Story 3.5), severity alert delivery (Story 3.6), controlled recovery gates (Story 3.7), and external reporting/export interfaces (Epic 4).

[Source: _bmad-output/planning-artifacts/epics.md#Story 3.4: Build Cost-Aware PnL and Attribution Surfaces]  
[Source: _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)]  
[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1-6.9)]  
[Source: _bmad-output/planning-artifacts/prd.md#Portfolio & Allocation Management]  
[Source: _bmad-output/planning-artifacts/prd.md#Non-Functional Requirements]

### Architecture Compliance

- Preserve architecture boundaries:
  - `services/portfolio-engine` owns attribution aggregation/read-model computation seams.
  - `services/control-api` remains authenticated ingress for operator-facing attribution query contracts.
  - `apps/operator-console` remains presentation/composition layer for dashboard attribution surfaces.
  - `crates/domain` and `crates/persistence` own shared contracts + persistence adapters.
- Preserve data-boundary contracts:
  - PostgreSQL projections/read models are dashboard inputs, not source-of-truth replacements.
  - reuse reconciliation/exposure evidence from Story 2.6; do not create parallel lifecycle ledgers.
- Keep canonical conventions:
  - typed machine-readable error envelopes,
  - UTC ISO-8601 timestamps,
  - explicit `reason_code` and `correlation_id`,
  - deterministic ordering in read queries and UI rendering.
- Maintain FR26 shell composition and Story 3.1 route-group conventions while extending portfolio surfaces.

[Source: _bmad-output/planning-artifacts/architecture.md#Data Architecture]  
[Source: _bmad-output/planning-artifacts/architecture.md#API & Communication Patterns]  
[Source: _bmad-output/planning-artifacts/architecture.md#Frontend Architecture]  
[Source: _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: _bmad-output/implementation-artifacts/stories/2-6-build-reconciliation-and-exposure-visibility-core.md#Project Structure Notes]

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
  - `tokio`: `1.51.0` (newer minor available; defer)
  - `sqlx`: `0.8.6` (latest stable; `0.9.0-alpha.1` pre-release)
  - `axum`: `0.8.8` (matches latest stable)
  - `polymarket-client-sdk`: `0.4.4` (matches latest stable)
  - `time`: `0.3.47` (newer patch available; defer)
- Do not introduce opportunistic dependency upgrades in Story 3.4.

[Source: apps/operator-console/package.json]  
[Source: Cargo.toml]  
[Source: https://registry.npmjs.org/next/latest]  
[Source: https://registry.npmjs.org/react/latest]  
[Source: https://registry.npmjs.org/react-dom/latest]  
[Source: https://registry.npmjs.org/tailwindcss/latest]  
[Source: https://registry.npmjs.org/typescript/latest]  
[Source: https://crates.io/api/v1/crates/tokio]  
[Source: https://crates.io/api/v1/crates/sqlx]  
[Source: https://crates.io/api/v1/crates/axum]  
[Source: https://crates.io/api/v1/crates/polymarket-client-sdk]  
[Source: https://crates.io/api/v1/crates/time]

### File Structure Requirements

- Primary implementation surfaces (expected Story 3.4 seams):
  - `crates/domain/src/lib.rs`
  - `crates/domain/src/attribution.rs`
  - `crates/persistence/migrations/*attribution_snapshots*.sql`
  - `crates/persistence/src/postgres/{mod.rs,attribution_snapshots.rs}`
  - `services/portfolio-engine/src/{main.rs,attribution/mod.rs}`
  - `services/control-api/src/routes/mod.rs`
  - `apps/operator-console/src/components/portfolio/{PortfolioSummaryCard.tsx,PnlAttributionSummaryCard.tsx,PnlAttributionBreakdownTable.tsx}`
  - `apps/operator-console/src/lib/portfolio/{allocation-policy.ts,attribution.ts}`
  - `apps/operator-console/src/app/(dashboard)/dashboard/page.tsx`
  - `apps/operator-console/src/app/globals.css`
  - `tests/story-3-4/*.test.mjs`
  - `tests/api/story-3-4*.test.mjs`
  - `tests/e2e/story-3-4*.test.mjs`
  - `package.json`
  - `_bmad-output/implementation-artifacts/tests/test-summary.md`
  - `docs/operations/*attribution*`
- Preserve existing Story 3 composition:
  - keep allocation/rebalance workflows (Story 3.3) in the portfolio area,
  - keep persistent safety rail/risk banner shell contracts (Story 3.2),
  - keep route-group + tab composition from Story 3.1.
- Reuse established layering pattern: domain -> migration -> persistence -> runtime seam -> API contract -> UI integration -> story QA command.

[Source: _bmad-output/planning-artifacts/architecture.md#Complete Project Directory Structure]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: services/portfolio-engine/src/attribution/mod.rs]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: apps/operator-console/src/components/portfolio/PortfolioSummaryCard.tsx]  
[Source: _bmad-output/implementation-artifacts/stories/3-3-deliver-allocation-policy-and-drift-rebalance-workflows.md#File Structure Requirements]

### Testing Requirements

- Add deterministic coverage for:
  - attribution decomposition and period aggregation correctness (market/alpha/period),
  - explicit time-window boundary behavior (`start_inclusive`, `end_exclusive`),
  - metadata-first rendering order and required evidence fields on cards/tables,
  - skeleton and empty-state behavior with actionable next-step guidance (no spinner-only fallback),
  - machine-readable API failures (`400/401/403/404/500/503`) with no success-shaped UI fallback,
  - stale/degraded data signaling under unmet freshness/performance assumptions,
  - non-regression for Story 3.3 allocation/rebalance surfaces and Story 3.2 risk command surfaces.
- Keep test layering consistent with repository standards:
  - Rust domain + persistence + service tests,
  - operator-console story/API/E2E tests under `tests/`,
  - story-scoped QA command via `qa:test:story-3-4`.

[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1-6.9)]  
[Source: _bmad-output/planning-artifacts/ux-design-specification.md#Feedback Patterns]  
[Source: _bmad-output/planning-artifacts/ux-design-specification.md#Additional Patterns]  
[Source: _bmad-output/planning-artifacts/ux-design-specification.md#Testing Strategy]  
[Source: package.json]

### Previous Story Intelligence

- Story 3.3 already delivered portfolio allocation/rebalance workflows and canonical control-api client patterns; Story 3.4 should extend the same portfolio surface without replacing those workflows.
- Story 3.2 established persistent risk command UX contracts and machine-readable control evidence; attribution surfaces should preserve this safety-first shell behavior.
- Story 3.1 established shell/tab composition, deterministic read-model fallback semantics, and token-first UI conventions; Story 3.4 should build within those contracts.
- Story 2.6 introduced reconciliation runs and `exposure_snapshots` read-paths; Story 3.4 should reuse these data seams for attribution context and incident traceability.
- Existing `services/portfolio-engine/src/attribution/mod.rs` remains a placeholder seam and is the intended runtime extension point for this story.

[Source: _bmad-output/implementation-artifacts/stories/3-3-deliver-allocation-policy-and-drift-rebalance-workflows.md#Project Structure Notes]  
[Source: _bmad-output/implementation-artifacts/stories/3-2-implement-risk-posture-banner-and-persistent-safety-action-rail.md#Project Structure Notes]  
[Source: _bmad-output/implementation-artifacts/stories/3-1-build-token-first-dashboard-shell-and-navigation-model.md#Project Structure Notes]  
[Source: _bmad-output/implementation-artifacts/stories/2-6-build-reconciliation-and-exposure-visibility-core.md#Project Structure Notes]  
[Source: services/portfolio-engine/src/attribution/mod.rs]

### Git Intelligence Summary

- Recent commit sequence (`3-1` -> `3-2` -> `3-3`) follows a stable vertical-slice pattern:
  1. story-scoped contracts/seams,
  2. API/UI integration,
  3. `qa:test:story-*` wiring and evidence updates.
- Changed-file patterns show strong reuse of canonical error envelopes, explicit evidence fields, and story-scoped test suites; Story 3.4 should preserve this pattern.
- Portfolio-related surfaces are already centralized under `apps/operator-console/src/components/portfolio/` and should remain the primary Story 3.4 UI extension seam.

[Source: git --no-pager log --oneline -5]  
[Source: git --no-pager show --name-only --pretty='format:%h %s' 20c50ac]  
[Source: git --no-pager show --name-only --pretty='format:%h %s' 5fd7a57]  
[Source: git --no-pager show --name-only --pretty='format:%h %s' dfa9b3e]

### Latest Technical Information

- Frontend package checks confirm pinned versions for Next.js/React/react-dom remain current, with Tailwind and TypeScript latest versions unchanged from prior Epic 3 context generation.
- Crates.io checks confirm `sqlx`, `axum`, and `polymarket-client-sdk` are still pinned to current stable versions; `tokio` and `time` have newer releases but do not require upgrade for Story 3.4.
- No dependency upgrades are required to deliver Story 3.4 scope safely.

[Source: https://registry.npmjs.org/next/latest]  
[Source: https://registry.npmjs.org/react/latest]  
[Source: https://registry.npmjs.org/react-dom/latest]  
[Source: https://registry.npmjs.org/tailwindcss/latest]  
[Source: https://registry.npmjs.org/typescript/latest]  
[Source: https://crates.io/api/v1/crates/tokio]  
[Source: https://crates.io/api/v1/crates/sqlx]  
[Source: https://crates.io/api/v1/crates/axum]  
[Source: https://crates.io/api/v1/crates/polymarket-client-sdk]  
[Source: https://crates.io/api/v1/crates/time]

### Project Context Reference

- No `project-context.md` file was found in repository scope during discovery.
- Story context was derived from epics, PRD, architecture, UX specification, implementation readiness report, research artifacts, previous story files, git history, and current source seams.

### Project Structure Notes

- `services/portfolio-engine/src/attribution/mod.rs` is currently an explicit placeholder and should become the primary attribution aggregation/read-model seam.
- `apps/operator-console/src/components/portfolio/PortfolioSummaryCard.tsx` currently contains shell-slot placeholder copy and existing Story 3.3 sub-surfaces; Story 3.4 should upgrade this area to implemented attribution cards/tables without regressing allocation/rebalance actions.
- `services/control-api/src/routes/mod.rs` already hosts authenticated allocation/rebalance routes and canonical decision/error envelope patterns suitable for reuse by attribution read endpoints.
- `crates/persistence/src/postgres/reconciliation.rs` already provides deterministic exposure snapshot retrieval and should be reused where attribution context depends on reconciliation evidence.
- Existing CSS already includes Story 3.3 portfolio classes under `globals.css`; attribution styles should extend tokenized conventions rather than introducing ad-hoc style systems.

[Source: services/portfolio-engine/src/attribution/mod.rs]  
[Source: apps/operator-console/src/components/portfolio/PortfolioSummaryCard.tsx]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: crates/persistence/src/postgres/reconciliation.rs]  
[Source: apps/operator-console/src/app/globals.css]

### References

- _bmad-output/planning-artifacts/epics.md#Epic 3: Portfolio Command Center, Alerts & Recovery Operations  
- _bmad-output/planning-artifacts/epics.md#Story 3.4: Build Cost-Aware PnL and Attribution Surfaces  
- _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)  
- _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)  
- _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1-6.9)  
- _bmad-output/planning-artifacts/prd.md#Portfolio & Allocation Management  
- _bmad-output/planning-artifacts/prd.md#Operations Dashboard & Incident Handling  
- _bmad-output/planning-artifacts/prd.md#Non-Functional Requirements  
- _bmad-output/planning-artifacts/prd.md#Journey Requirements Summary  
- _bmad-output/planning-artifacts/architecture.md#Data Architecture  
- _bmad-output/planning-artifacts/architecture.md#API & Communication Patterns  
- _bmad-output/planning-artifacts/architecture.md#Frontend Architecture  
- _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules  
- _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping  
- _bmad-output/planning-artifacts/architecture.md#Complete Project Directory Structure  
- _bmad-output/planning-artifacts/ux-design-specification.md#Critical Success Moments  
- _bmad-output/planning-artifacts/ux-design-specification.md#Flow Optimization Principles  
- _bmad-output/planning-artifacts/ux-design-specification.md#Feedback Patterns  
- _bmad-output/planning-artifacts/ux-design-specification.md#Additional Patterns  
- _bmad-output/planning-artifacts/ux-design-specification.md#Responsive Design & Accessibility  
- _bmad-output/planning-artifacts/implementation-readiness-report-2026-04-05.md#Warnings  
- _bmad-output/planning-artifacts/research/market-rust-polymarket-trading-system-lopez-de-prado-research-2026-04-04.md#Architectural Patterns and Design  
- _bmad-output/implementation-artifacts/stories/2-6-build-reconciliation-and-exposure-visibility-core.md  
- _bmad-output/implementation-artifacts/stories/3-1-build-token-first-dashboard-shell-and-navigation-model.md  
- _bmad-output/implementation-artifacts/stories/3-2-implement-risk-posture-banner-and-persistent-safety-action-rail.md  
- _bmad-output/implementation-artifacts/stories/3-3-deliver-allocation-policy-and-drift-rebalance-workflows.md  
- docs/operations/allocation-rebalance-workflows.md  
- apps/operator-console/package.json  
- Cargo.toml  
- package.json  
- services/portfolio-engine/src/attribution/mod.rs  
- services/control-api/src/routes/mod.rs  
- apps/operator-console/src/components/portfolio/PortfolioSummaryCard.tsx  
- apps/operator-console/src/lib/portfolio/allocation-policy.ts  
- crates/persistence/src/postgres/reconciliation.rs  
- apps/operator-console/src/app/globals.css

## Story Completion Status

- Story 3.4 implementation plus adversarial code-review remediations are complete across domain, persistence, portfolio-engine, control-api, and operator-console attribution surfaces.
- Story-scoped QA command plus repository lint/test/build gates passed after post-review fixes.
- Story artifacts and sprint tracking were updated from `review` to `done`.

## Dev Agent Record

### Agent Model Used

GPT-5.3-Codex (gpt-5.3-codex)

### Debug Log References

- BMAD dev-story workflow execution (automated, non-interactive)
- Sprint backlog discovery and lifecycle updates from `_bmad-output/implementation-artifacts/sprint-status.yaml`
- `export PATH="$HOME/.cargo/bin:$PATH" && node --test tests/api/story-3-4*.test.mjs tests/e2e/story-3-4*.test.mjs`
- `npm run --silent qa:test:story-3-4`
- `npm run --silent rust:lint`
- `npm run --silent rust:test && npm run --silent rust:build && npm run --silent web:lint && npm run --silent web:typecheck && npm run --silent web:build`
- `export PATH="$HOME/.cargo/bin:$PATH" && npm run --silent qa:test:story-3-4 && npm run --silent rust:lint && npm run --silent rust:test && npm run --silent rust:build && npm run --silent web:lint && npm run --silent web:typecheck && npm run --silent web:build`

### Completion Notes List

- Implemented attribution contracts in `crates/domain/src/attribution.rs` with deterministic period semantics, ordering rules, reason-code taxonomy, and boundary-focused unit coverage.
- Added forward-only `attribution_snapshots` migration and persistence adapter wiring with deterministic latest-scope retrieval and typed error mapping.
- Implemented portfolio-engine attribution seam and authenticated control-api attribution read route with metadata-rich success envelopes plus machine-readable degraded dependency failures.
- Delivered operator-console attribution API client, metadata-first summary card, breakdown table, and token-driven styling integrated into `PortfolioSummaryCard` while preserving Story 3.3 allocation/rebalance surfaces.
- Added Story 3.4 QA script wiring and story/api/e2e suites; updated operations runbook + test summary evidence; validated story and repository quality gates.
- Applied adversarial review fixes: strict timestamp contract parsing, unauthorized error taxonomy enforcement, server-timestamped attribution audit envelopes, persisted attribution read-path preference, snapshot/run UI evidence exposure, and financial consistency guardrails.
- Expanded Story 3.4 QA automation coverage with unauthorized attribution API error-evidence assertions plus canonical period boundary and critical dependency escalation E2E checks; re-ran story QA gate successfully.

### File List

- Cargo.lock
- _bmad-output/implementation-artifacts/stories/3-4-build-cost-aware-pnl-and-attribution-surfaces.md
- _bmad-output/implementation-artifacts/sprint-status.yaml
- _bmad-output/implementation-artifacts/deferred-work.md
- _bmad-output/implementation-artifacts/tests/test-summary.md
- crates/domain/src/lib.rs
- crates/domain/src/attribution.rs
- crates/persistence/migrations/20260406154000_attribution_snapshots.sql
- crates/persistence/src/postgres/mod.rs
- crates/persistence/src/postgres/attribution_snapshots.rs
- services/portfolio-engine/src/main.rs
- services/portfolio-engine/src/attribution/mod.rs
- services/control-api/Cargo.toml
- services/control-api/src/main.rs
- services/control-api/src/middleware/mod.rs
- services/control-api/src/routes/mod.rs
- apps/operator-console/src/lib/portfolio/attribution.ts
- apps/operator-console/src/components/portfolio/PnlAttributionSummaryCard.tsx
- apps/operator-console/src/components/portfolio/PnlAttributionBreakdownTable.tsx
- apps/operator-console/src/components/portfolio/PortfolioSummaryCard.tsx
- apps/operator-console/src/app/globals.css
- tests/story-3-4/pnl-attribution.story-3-4.test.mjs
- tests/api/story-3-4-attribution-api.test.mjs
- tests/e2e/story-3-4-attribution-dashboard.e2e.test.mjs
- docs/operations/cost-aware-pnl-attribution-operations.md
- package.json

### Change Log

- 2026-04-06: Created Story 3.4 context file and moved lifecycle state from `backlog` to `ready-for-dev`.
- 2026-04-06: Implemented Story 3.4 cost-aware attribution vertical slice across domain, persistence, portfolio-engine, control-api, and operator-console surfaces.
- 2026-04-06: Added Story 3.4 QA automation/docs evidence updates and promoted story status to `review`.
- 2026-04-06: Completed adversarial review remediations, re-ran full quality gates, and promoted story status from `review` to `done`.
- 2026-04-06: Executed BMAD QA refresh for Story 3.4, expanded API/E2E critical-flow coverage, and confirmed story status remains `done`.
