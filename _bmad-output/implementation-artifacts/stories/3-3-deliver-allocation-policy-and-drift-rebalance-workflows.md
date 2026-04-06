# Story 3.3: Deliver Allocation Policy and Drift-Rebalance Workflows

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an operator,  
I want to configure allocations and review rebalance rationale,  
so that portfolio drift can be corrected with auditable intent.

## Acceptance Criteria

1. **Allocation policy + drift-rebalance workflow (story-local BDD):**  
   **Given** allocation policy and drift thresholds are set  
   **When** drift exceeds policy limits  
   **Then** the system proposes or executes rebalance with visible rationale and approval context  
   **And** workflow fulfills FR22 and FR23 with progressive-disclosure controls (UX-DR15).
2. **UAC-1 Failure handling:** Invalid payloads, unauthorized mutation attempts, unavailable policy state, and unavailable persistence/dependency paths return explicit machine-readable errors with no unsafe side effects and no success-shaped UI.
3. **UAC-2 Boundary behavior:** Drift-threshold behavior is deterministic and test-covered, including:
   - default exposure drift threshold = `10%`,
   - default relative alpha drift threshold = `15%`,
   - exact-threshold semantics are explicit and test-covered (`drift == threshold` stays in-bounds; `drift > threshold` enters recommendation flow),
   - fail-closed behavior when policy/recommendation state is stale or unavailable.
4. **UAC-3 Verifiable evidence:** Policy updates and rebalance recommendation/execution surfaces expose timestamped evidence (`actor_id`/source, action type, parameters, reason code, correlation ID, approval status/reference) and remain queryable within the NFR17 audit-query expectation.
5. **Schema/dependency/traceability contract:** Story depends only on `2.7` and `3.1`; creates only `allocation_policies` and `rebalance_recommendations`; maps explicitly to `FR22`, `FR23`, `NFR17`, and `UX-DR15`.
6. **Progressive-disclosure form contract:** Allocation forms use progressive disclosure for advanced parameters, inline validation on blur, and risk-impact guidance text with one clear recommended next action for warning/critical outcomes.
7. **Approval-context contract:** Any recommendation path that implies critical risk-limit increase follows existing dual-approval flow conventions and exposes `pending/approved/denied` context without bypassing governance controls.
8. **Control/API contract alignment:** Rebalance and allocation-policy workflows reuse canonical control-api authorization, audit, and machine-error envelope patterns; no duplicate privileged ingress or ad-hoc response schema is introduced.

## Tasks / Subtasks

- [x] **Task 1: Define allocation-policy and rebalance recommendation domain contracts** (AC: 1, 2, 3, 4, 5, 7)
  - [x] Add canonical domain types and reason-code taxonomy for allocation policies, drift thresholds, recommendation states, and recommendation rationale/evidence fields.
  - [x] Define deterministic drift-evaluation boundary rules (`drift == threshold` stays in-bounds, `drift > threshold` triggers recommendation flow; stale/unavailable handling fail-closed).
  - [x] Keep identifier normalization, UTC timestamp, and machine-readable error conventions consistent with existing risk/governance domain contracts.

- [x] **Task 2: Add forward-only persistence schema and adapters for Story 3.3 entities** (AC: 2, 3, 4, 5)
  - [x] Add migration for `allocation_policies` and `rebalance_recommendations` only, with strict checks for lowercase identifiers, finite numeric thresholds, UTC timestamps, and status constraints.
  - [x] Add persistence adapter module(s) under `crates/persistence/src/postgres/` with transactional upsert/query behavior and explicit typed error mapping.
  - [x] Add indexes for active-policy lookup, pending recommendation retrieval, actor/correlation traceability, and latest recommendation-by-policy queries.

- [x] **Task 3: Implement governance/service orchestration for allocation policy mutation and recommendation lifecycle** (AC: 1, 2, 4, 5, 7, 8)
  - [x] Add governance-service orchestration module for allocation policy workflows with role checks and explicit machine errors.
  - [x] Integrate approval-context behavior for critical increase scenarios using existing approval orchestration conventions (no parallel approval mechanism).
  - [x] Emit deterministic telemetry and evidence fields required for NFR17 auditability.

- [x] **Task 4: Extend control-api contracts for allocation policy and rebalance workflows** (AC: 1, 2, 4, 6, 7, 8)
  - [x] Add/extend authenticated control-api routes for allocation policy upsert and recommendation query/execution paths, reusing `authorize_critical_action` and audit append patterns.
  - [x] Ensure response envelopes include machine-readable `error_code`, `reason_code`, `correlation_id`, timestamps, and approval context where applicable.
  - [x] Extend existing `/control/rebalance` behavior to carry recommendation/rationale context instead of introducing a duplicate privileged endpoint.

- [x] **Task 5: Implement portfolio-engine allocation/drift evaluation seam** (AC: 1, 2, 3, 5)
  - [x] Replace allocation scaffold placeholder with deterministic drift evaluation + recommendation generation primitives.
  - [x] Keep execution-state mutation ownership outside portfolio-engine; produce recommendation/read-model outputs only unless explicitly approved via control path.
  - [x] Surface recommendation rationale fields suitable for operator review and audit correlation.

- [x] **Task 6: Build operator-console allocation policy and rebalance rationale UX surfaces** (AC: 1, 2, 3, 4, 6, 7)
  - [x] Implement portfolio-allocation UX components with progressive disclosure for advanced controls and inline blur validation.
  - [x] Add recommendation review surface showing drift evidence, rationale text, approval context, and explicit recommended next action.
  - [x] Integrate surfaces into dashboard route/tab composition without violating Story 3.1 shell conventions or Story 3.2 safety-rail behavior.

- [x] **Task 7: Add Story 3.3 QA automation, command wiring, and runbook evidence updates** (AC: 2, 3, 4, 6, 7, 8)
  - [x] Add deterministic tests across domain, persistence, governance-service, control-api, portfolio-engine, and operator-console (component/API/E2E) surfaces.
  - [x] Add root script `qa:test:story-3-3` in `package.json`, following existing story QA command patterns.
  - [x] Update `_bmad-output/implementation-artifacts/tests/test-summary.md` with Story 3.3 coverage/evidence and add/refresh operations runbook guidance for allocation/rebalance workflows.

## Dev Notes

### Technical Requirements

- Story dependency and scope are strict:
  - depends on `2.7` and `3.1`,
  - schema scope limited to `allocation_policies` and `rebalance_recommendations`,
  - traceability scope: `FR22`, `FR23`, `NFR17`, `UX-DR15`.
- Required story outcomes:
  - operator-configurable allocation policies across alpha sleeves/market segments (FR22),
  - deterministic drift-triggered rebalance recommendation/execution readiness with visible rationale (FR23),
  - audit-query-ready evidence for allocation-impacting actions (NFR17).
- Default drift thresholds from requirements:
  - exposure drift threshold `10%`,
  - relative alpha drift threshold `15%`.
  - boundary semantics: equality to threshold does not trigger rebalance; only strict exceedance does.
- UX obligations for this story:
  - progressive disclosure for advanced parameters,
  - inline validation on blur,
  - risk-impact guidance with one clear recommended next action in warning/critical paths.
- **Out of scope for Story 3.3:** cost-aware PnL attribution surfaces (Story 3.4), incident forensics search/timeline depth (Story 3.5), severity alert delivery workflows (Story 3.6), and controlled recovery gate execution (Story 3.7).

[Source: _bmad-output/planning-artifacts/epics.md#Story 3.3: Deliver Allocation Policy and Drift-Rebalance Workflows]  
[Source: _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)]  
[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: _bmad-output/planning-artifacts/prd.md#Portfolio & Allocation Management]  
[Source: _bmad-output/planning-artifacts/prd.md#Compliance & Auditability]  
[Source: _bmad-output/planning-artifacts/ux-design-specification.md#Form Patterns]

### Architecture Compliance

- Preserve ownership boundaries:
  - `control-api` remains authenticated privileged ingress,
  - `governance-service` owns approval/audit-oriented mutation orchestration,
  - `portfolio-engine` owns allocation/drift recommendation logic seams,
  - `execution-engine` must not mutate policy state directly.
- Keep data-boundary contracts intact:
  - PostgreSQL remains persisted source of truth for policy/recommendation records,
  - dashboard/operator surfaces consume projections and API responses, not direct writes.
- Reuse canonical patterns:
  - machine-readable typed error envelopes,
  - UTC-only timestamps,
  - explicit `reason_code` + `correlation_id`,
  - append-only audit compatibility for privileged actions.
- Treat stale or uncertain policy state as fail-closed for rebalance execution readiness.

[Source: _bmad-output/planning-artifacts/architecture.md#Architectural Boundaries]  
[Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping]  
[Source: _bmad-output/planning-artifacts/architecture.md#API & Communication Patterns]  
[Source: _bmad-output/planning-artifacts/architecture.md#Data Architecture]  
[Source: services/control-api/src/routes/mod.rs]

### Library & Framework Requirements

- Keep workspace-pinned stack for story compatibility:
  - `next = 16.2.2`
  - `react = 19.2.4`
  - `react-dom = 19.2.4`
  - `tailwindcss = ^4`
  - `typescript = ^5`
  - `axum = 0.8.8`
  - `sqlx = 0.8.6`
  - `tokio = 1.48.0`
  - `polymarket-client-sdk = 0.4.4`
- Latest-version checks at story creation time:
  - `next`: `16.2.2` (matches pinned)
  - `react`: `19.2.4` (matches pinned)
  - `react-dom`: `19.2.4` (matches pinned)
  - `tailwindcss`: `4.2.2` (compatible with `^4`)
  - `typescript`: `6.0.2` (newer major available; defer)
  - `tokio`: `1.51.0` (newer minor than pinned; defer)
  - `sqlx`: `0.8.6` (matches stable pinned)
  - `axum`: `0.8.8` (matches stable pinned)
  - `polymarket-client-sdk`: `0.4.4` (matches stable pinned)
- Do not introduce opportunistic dependency upgrades in Story 3.3.

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

### File Structure Requirements

- Primary implementation surfaces (expected Story 3.3 seams):
  - `crates/domain/src/risk.rs` (or `crates/domain/src/allocation.rs` with `mod` wiring)
  - `crates/persistence/migrations/*allocation*rebalance*.sql`
  - `crates/persistence/src/postgres/{mod.rs,allocation_policies.rs}`
  - `services/governance-service/src/{lib.rs,allocation_policy/mod.rs}`
  - `services/control-api/src/{main.rs,middleware/mod.rs,routes/mod.rs}`
  - `services/portfolio-engine/src/{main.rs,allocation/mod.rs}`
  - `apps/operator-console/src/components/portfolio/{PortfolioSummaryCard.tsx,AllocationPolicyForm.tsx,RebalanceRecommendationCard.tsx}`
  - `apps/operator-console/src/app/(dashboard)/dashboard/page.tsx`
  - `apps/operator-console/src/lib/portfolio/allocation-policy.ts`
  - `tests/story-3-3/*.test.mjs`
  - `tests/api/story-3-3*.test.mjs`
  - `tests/e2e/story-3-3*.test.mjs`
  - `docs/operations/*allocation*rebalance*.md`
  - `package.json`
  - `_bmad-output/implementation-artifacts/tests/test-summary.md`
- Preserve existing route-group and shell composition conventions from Story 3.1/3.2.
- Reuse Story 2.7 layering pattern: domain → migration → persistence adapter → service orchestration → API integration → UI integration → tests/runbook.

[Source: _bmad-output/planning-artifacts/architecture.md#Complete Project Directory Structure]  
[Source: services/portfolio-engine/src/allocation/mod.rs]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: apps/operator-console/src/app/(dashboard)/dashboard/page.tsx]  
[Source: _bmad-output/implementation-artifacts/stories/2-7-configure-portfolio-market-and-strategy-limit-policies.md#File Structure Requirements]

### Testing Requirements

- Add deterministic coverage for:
  - drift-threshold boundary evaluation (`drift == threshold` yields no recommendation; `drift > threshold` yields recommendation flow),
  - progressive disclosure + blur validation + risk-impact guidance rendering in allocation forms,
  - recommendation lifecycle transitions (`proposed/pending/approved/executed/denied`) and approval-context projection behavior,
  - fail-closed behavior on unauthorized input and unavailable policy/persistence dependencies,
  - machine-readable response envelope contracts (`error_code`, `reason_code`, `correlation_id`, `timestamp_utc`) across API/UI paths,
  - audit-evidence payload coverage required by NFR17.
- Keep test layering consistent with repository standards:
  - domain (`crates/domain`),
  - persistence (`crates/persistence`),
  - governance/control-api (`services/governance-service`, `services/control-api`),
  - runtime engine (`services/portfolio-engine`, `services/risk-engine` where integration applies),
  - web/API/E2E story suites under `tests/`.
- Add and wire `qa:test:story-3-3` in root `package.json`.

[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]  
[Source: _bmad-output/planning-artifacts/ux-design-specification.md#Testing Strategy]  
[Source: package.json]

### Previous Story Intelligence

- Story 3.2 established canonical control-path UX and machine-readable control evidence; Story 3.3 should reuse these evidence and error-handling conventions for rebalance workflows.
- Story 3.1 established shell route-group structure, in-page tabs, deterministic read-model/fallback behavior, and tokenized UI conventions that Story 3.3 portfolio surfaces must extend (not replace).
- Story 2.7 provides the policy-configuration blueprint for schema discipline, pending/active approval status handling, and risk-limit increase approval-context integration.
- Existing `/control/rebalance` route and control-api authorization/audit helper seams should be extended rather than duplicated.

[Source: _bmad-output/implementation-artifacts/stories/3-2-implement-risk-posture-banner-and-persistent-safety-action-rail.md#Project Structure Notes]  
[Source: _bmad-output/implementation-artifacts/stories/3-1-build-token-first-dashboard-shell-and-navigation-model.md#Project Structure Notes]  
[Source: _bmad-output/implementation-artifacts/stories/2-7-configure-portfolio-market-and-strategy-limit-policies.md#Technical Requirements]  
[Source: services/control-api/src/routes/mod.rs]

### Git Intelligence Summary

- Recent commit progression (`2-7` policy orchestration, `3-1` shell foundation, `3-2` safety controls) indicates a stable pattern of story-scoped vertical slices with explicit QA command wiring.
- Story 3.3 should preserve this sequencing discipline: backend contracts + persistence + API + UI + story-scoped QA command in a single coherent slice.
- Commit file-change patterns show strong reuse of machine-readable error envelopes, audit metadata, and source-of-truth boundaries; Story 3.3 should maintain these conventions.

[Source: git --no-pager log --oneline -5]  
[Source: git --no-pager show --name-only --pretty='format:%h %s' 5fd7a57]  
[Source: git --no-pager show --name-only --pretty='format:%h %s' dfa9b3e]  
[Source: git --no-pager show --name-only --pretty='format:%h %s' 0b76e9a]

### Latest Technical Information

- Frontend package checks confirm pinned stack remains compatible with latest stable Next.js/React/react-dom and Tailwind major range.
- Rust crate checks confirm `sqlx`, `axum`, and `polymarket-client-sdk` pinned versions match latest stable; `tokio` has a newer minor release and can be considered in a dedicated dependency-upgrade story.
- No dependency upgrades are required to deliver Story 3.3 scope.

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

### Project Context Reference

- No `project-context.md` file was found in repository scope during discovery.
- Story context was derived from epics, PRD, architecture, UX specification, implementation readiness artifacts, prior stories, git history, and current source seams.

### Project Structure Notes

- `services/portfolio-engine/src/allocation/mod.rs` is currently a Story 3 scaffold seam and should become the primary home for allocation/drift recommendation logic.
- `apps/operator-console/src/components/portfolio/PortfolioSummaryCard.tsx` is still a shell placeholder and is the natural UX extension point for Story 3.3 allocation/rebalance surfaces.
- `services/control-api/src/routes/mod.rs` already includes privileged route, authorization helper, and policy workflow response patterns suitable for Story 3.3 reuse.
- Existing Story 2.7 risk-limit policy and runbook conventions should be mirrored for allocation-policy operational guidance and approval-context evidence.

[Source: services/portfolio-engine/src/allocation/mod.rs]  
[Source: apps/operator-console/src/components/portfolio/PortfolioSummaryCard.tsx]  
[Source: services/control-api/src/routes/mod.rs]  
[Source: docs/operations/risk-limit-policy-operations.md]

### References

- _bmad-output/planning-artifacts/epics.md#Epic 3: Portfolio Command Center, Alerts & Recovery Operations  
- _bmad-output/planning-artifacts/epics.md#Story 3.3: Deliver Allocation Policy and Drift-Rebalance Workflows  
- _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)  
- _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)  
- _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)  
- _bmad-output/planning-artifacts/prd.md#Portfolio & Allocation Management  
- _bmad-output/planning-artifacts/prd.md#Compliance & Auditability  
- _bmad-output/planning-artifacts/prd.md#Non-Functional Requirements  
- _bmad-output/planning-artifacts/architecture.md#Data Architecture  
- _bmad-output/planning-artifacts/architecture.md#API & Communication Patterns  
- _bmad-output/planning-artifacts/architecture.md#Frontend Architecture  
- _bmad-output/planning-artifacts/architecture.md#Architectural Boundaries  
- _bmad-output/planning-artifacts/architecture.md#Requirements to Structure Mapping  
- _bmad-output/planning-artifacts/architecture.md#Complete Project Directory Structure  
- _bmad-output/planning-artifacts/ux-design-specification.md#Form Patterns  
- _bmad-output/planning-artifacts/ux-design-specification.md#Flow Optimization Principles  
- _bmad-output/planning-artifacts/ux-design-specification.md#Action Rail (Safety Controls)  
- _bmad-output/planning-artifacts/ux-design-specification.md#Feedback Patterns  
- _bmad-output/planning-artifacts/ux-design-specification.md#Responsive Design & Accessibility  
- _bmad-output/planning-artifacts/implementation-readiness-report-2026-04-05.md#Warnings  
- _bmad-output/implementation-artifacts/stories/2-7-configure-portfolio-market-and-strategy-limit-policies.md  
- _bmad-output/implementation-artifacts/stories/3-1-build-token-first-dashboard-shell-and-navigation-model.md  
- _bmad-output/implementation-artifacts/stories/3-2-implement-risk-posture-banner-and-persistent-safety-action-rail.md  
- apps/operator-console/package.json  
- apps/operator-console/src/app/(dashboard)/dashboard/page.tsx  
- apps/operator-console/src/components/portfolio/PortfolioSummaryCard.tsx  
- apps/operator-console/src/components/shell/OperatorShellLayout.tsx  
- apps/operator-console/src/lib/shell/read-models.ts  
- apps/operator-console/src/lib/risk/posture.ts  
- services/control-api/src/routes/mod.rs  
- services/portfolio-engine/src/allocation/mod.rs  
- services/governance-service/src/risk_limits/mod.rs  
- crates/domain/src/risk.rs  
- crates/persistence/src/postgres/risk_limits.rs  
- docs/operations/risk-limit-policy-operations.md

## Story Completion Status

- Story implementation completed across domain, persistence, governance-service, control-api, portfolio-engine, and operator-console surfaces with Story 3.3 scope boundaries preserved (`allocation_policies` + `rebalance_recommendations` only).
- Story status moved from `ready-for-dev` → `in-progress` → `review` → `done` with all acceptance criteria and universal acceptance criteria mapped to deterministic tests.
- Completion note: Allocation policy mutation, drift recommendation/execution context, progressive-disclosure UX, and evidence/runbook updates are validated and complete.
- Code review note: approval workflow enforcement was hardened to block client-supplied `approval_reference` bypass paths and preserve dual-approval execution contracts.

## Dev Agent Record

### Agent Model Used

GPT-5.3-Codex (gpt-5.3-codex)

### Debug Log References

- BMAD create-story workflow execution (automated)
- Sprint backlog discovery from `_bmad-output/implementation-artifacts/sprint-status.yaml`
- Artifact discovery across planning, implementation, and source surfaces
- Latest frontend package checks via npm registry endpoints
- Latest crate checks via crates.io API endpoints

### Completion Notes List

- Implemented Story 3.3 vertical slice: domain contracts, persistence migration/adapters, governance orchestration, control-api routes/envelopes, portfolio-engine seam, operator-console UX/client, and story-scoped QA suites.
- Extended `/control/rebalance` with recommendation/rationale context while preserving canonical privileged ingress, authorization, and audit patterns.
- Added progressive-disclosure allocation form + recommendation review card with blur validation, risk-impact guidance, approval-context visibility, and explicit next-action guidance.
- Added Story 3.3 QA command wiring, runbook guidance, and test-summary evidence updates; full suite/lint/build checks pass.
- Post-review hardening: blocked direct `approval_reference` client input on allocation/rebalance routes, mapped `rebalance_approval_required` to conflict semantics, and aligned operator-console request payloads with approval-request-driven evidence flow.
- QA refresh expanded Story 3.3 API/E2E suites to cover `400/404/500` machine-error flows, field-level validation evidence, and rebalance evaluate/load/execute control-loop affordances; `npm run --silent qa:test:story-3-3` passes with 14 story-scoped tests.

### File List

- _bmad-output/implementation-artifacts/stories/3-3-deliver-allocation-policy-and-drift-rebalance-workflows.md
- _bmad-output/implementation-artifacts/sprint-status.yaml
- crates/domain/src/lib.rs
- crates/domain/src/reconciliation.rs
- crates/domain/src/allocation.rs
- crates/persistence/migrations/20260406113000_allocation_policies_rebalance_recommendations.sql
- crates/persistence/src/postgres/mod.rs
- crates/persistence/src/postgres/reconciliation.rs
- crates/persistence/src/postgres/allocation_policies.rs
- services/governance-service/src/lib.rs
- services/governance-service/src/allocation_policy/mod.rs
- services/control-api/src/main.rs
- services/control-api/src/middleware/mod.rs
- services/control-api/src/routes/mod.rs
- services/execution-engine/src/ingestion/user_stream.rs
- services/execution-engine/src/reconciliation/mod.rs
- services/portfolio-engine/Cargo.toml
- services/portfolio-engine/src/main.rs
- services/portfolio-engine/src/allocation/mod.rs
- apps/operator-console/src/components/portfolio/PortfolioSummaryCard.tsx
- apps/operator-console/src/components/portfolio/AllocationPolicyForm.tsx
- apps/operator-console/src/components/portfolio/RebalanceRecommendationCard.tsx
- apps/operator-console/src/lib/portfolio/allocation-policy.ts
- apps/operator-console/src/app/globals.css
- tests/story-3-3/allocation-workflows.story-3-3.test.mjs
- tests/api/story-3-3-allocation-api.test.mjs
- tests/e2e/story-3-3-allocation-rebalance.e2e.test.mjs
- docs/operations/allocation-rebalance-workflows.md
- package.json
- Cargo.lock
- _bmad-output/implementation-artifacts/tests/test-summary.md

### Change Log

- 2026-04-06: Created Story 3.3 context file and moved lifecycle state from `backlog` to `ready-for-dev`.
- 2026-04-06: Implemented Story 3.3 allocation-policy and drift-rebalance workflows across domain, persistence, governance-service, control-api, portfolio-engine, and operator-console.
- 2026-04-06: Added Story 3.3 QA suites/command wiring, runbook updates, and promoted story status to `review`.
- 2026-04-06: Completed adversarial code review hardening (approval-reference bypass mitigation + machine-error mapping updates), synced file-list discrepancies, and promoted story status to `done`.
- 2026-04-06: Refreshed Story 3.3 QA automation coverage for API status/error contracts (`400/404/500`) and rebalance control-loop E2E affordances; reran `qa:test:story-3-3` successfully while keeping story status `done`.
