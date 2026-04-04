# Story 1.1: Set Up Initial Project from Starter Template

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As an admin/operator,  
I want the approved Rust + Next.js monorepo scaffold with CI and environment templates,  
so that implementation starts from a consistent, policy-safe baseline.

## Acceptance Criteria

1. **Bootstrap scaffold (story-local BDD):**  
   **Given** a new repository state  
   **When** the bootstrap story is completed  
   **Then** the workspace contains Rust service/crate scaffolds and a Next.js operator console scaffold aligned to architecture structure  
   **And** baseline CI, reproducible build scripts, and `.env.example` placeholders exist without plaintext secrets per architecture starter-template requirement.
2. **UAC-1 Failure handling:** Invalid bootstrap inputs (missing required toolchain versions, missing required environment placeholders, or secret-like literal values in templates) fail with explicit, machine-readable error output and no partial unsafe secret persistence. Error output must include `error_code`, `failed_check`, `remediation_hint`, and `timestamp_utc`.
3. **UAC-2 Boundary behavior:** Bootstrap checks cover deterministic boundary rules for this story: zero plaintext secrets in committed templates, least-privilege runtime defaults, and reproducible build entrypoints that produce the same artifact flow and matching artifact hashes in CI and local runs when using pinned toolchain versions.
4. **UAC-3 Verifiable evidence:** Successful and failed bootstrap checks emit timestamped telemetry/audit-style evidence in CI logs and command output, and CI publishes a retained `bootstrap-evidence` artifact containing command summary plus pass/fail matrix for traceability.
5. **Traceability and dependency constraints:** Story has no dependencies, creates no database tables, and must satisfy FR31 + NFR19 + NFR20 alignment while preparing security/governance groundwork for Stories 1.2+.

## Tasks / Subtasks

- [x] **Task 1: Create monorepo baseline and root manifests** (AC: 1, 3, 5)
  - [x] Add root workspace files expected by architecture baseline (`Cargo.toml`, `Cargo.lock`, `package.json`, `pnpm-workspace.yaml`, `.env.example`, `.gitignore`, root `README.md` updates).
  - [x] Pin and document bootstrap prerequisites for reproducibility (`rust-toolchain.toml` and Node/pnpm version declaration) and fail preflight when local/CI versions do not satisfy requirements.
  - [x] Ensure root scripts expose reproducible bootstrap/build/test entrypoints (for Rust workspace and web app).
  - [x] Add `.env.example` placeholders only (no secrets, no real credentials, no private endpoints with keys).
- [x] **Task 2: Scaffold Rust workspace services and shared crates** (AC: 1, 5)
  - [x] Initialize core Story 1.1 Rust units: `services/control-api`, `services/execution-engine`, `services/risk-engine`, `crates/domain`, `crates/common`.
  - [x] Add placeholder structure for downstream services defined by architecture (`portfolio-engine`, `governance-service`, `reporting-service`, `research-gateway`) and scaffold `crates/persistence` to preserve architecture path contracts without implementing story-forward behavior.
  - [x] Wire workspace membership and confirm per-crate/service build and test invocation works.
- [x] **Task 3: Scaffold Next.js operator console** (AC: 1, 3)
  - [x] Bootstrap `apps/operator-console` using Next.js App Router + TypeScript + Tailwind according to selected starter.
  - [x] Create initial folder boundaries consistent with architecture (`app/(dashboard)`, `app/(incidents)`, `app/(governance)`, `components/{risk,execution,governance,timeline}`, `lib`, `hooks`, `styles`).
  - [x] Seed token-first styling foundation (semantic token placeholders) so future UX stories do not start from raw color literals.
- [x] **Task 4: Add CI baseline and security checks** (AC: 1, 2, 3, 4)
  - [x] Create `.github/workflows/ci-rust.yml` for workspace format/lint/test/build gates.
  - [x] Create `.github/workflows/ci-web.yml` for web lint/type/build gates.
  - [x] Create `.github/workflows/security.yml` for secret scanning and dependency/security baseline checks.
  - [x] Publish `bootstrap-evidence` CI artifact for each baseline run, including timestamped command outputs and explicit pass/fail checks.
  - [x] Ensure CI failures are explicit and block merges on bootstrap baseline breakage.
- [x] **Task 5: Establish policy-safe environment and runtime defaults** (AC: 1, 2, 3)
  - [x] Document least-privilege runtime expectations in bootstrap docs/runbook stubs.
  - [x] Add environment template sections for control API, execution/risk services, and operator console with placeholder values and comments.
  - [x] Add guardrails preventing secret leakage into source/logs/process args in bootstrap scripts and CI checks.
- [x] **Task 6: Evidence and handoff quality gates** (AC: 2, 3, 4, 5)
  - [x] Add bootstrap verification commands to README or docs/operations bootstrap guide.
  - [x] Add negative-path bootstrap checks that assert machine-readable failure output shape (`error_code`, `failed_check`, `remediation_hint`, `timestamp_utc`).
  - [x] Validate local bootstrap path from clean clone to passing CI-equivalent commands.
  - [x] Capture bootstrap completion notes and known gaps for Story 1.2 handoff.

### Review Findings

- [x] [Review][Patch] Expand bootstrap secret scanning to cover repository source/workflow files and report scanned scope [tools/bootstrap/security-scan.mjs:18]
- [x] [Review][Patch] Add explicit least-privilege workflow token permissions for baseline CI jobs [`.github/workflows/ci-rust.yml:8`, `.github/workflows/ci-web.yml:8`, `.github/workflows/security.yml:8`]
- [x] [Review][Defer] `.scripts/bmad-auto/copilot/bmad-progress.log` appears in git status but is outside the story application scope and not part of the implementation File List — deferred, pre-existing

## Dev Notes

### Technical Requirements

- This story is the mandatory starter-template baseline and must ship before any feature implementation stories in Epic 1.  
  [Source: _bmad-output/planning-artifacts/epics.md#Additional Requirements]
- Traceability for Story 1.1 is explicitly FR31, NFR19, NFR20 with no schema creation.  
  [Source: _bmad-output/planning-artifacts/epics.md#Story Traceability & Dependency Index (Compact-Format Stories)]
- Security constraints are strict from day one: no plaintext secrets, least-privilege runtime, authenticated privileged pathways as the foundation for later stories.  
  [Source: _bmad-output/planning-artifacts/prd.md#Technical Constraints]  
  [Source: _bmad-output/planning-artifacts/prd.md#Non-Functional Requirements]

### Architecture Compliance

- Use the selected dual-starter monorepo baseline: Rust Cargo workspace + Next.js App Router operator console.  
  [Source: _bmad-output/planning-artifacts/architecture.md#Selected Starter: Dual-Starter Monorepo Baseline]
- Keep service/crate/UI boundaries aligned to architecture mapping; do not invent alternate top-level layout in Story 1.1.  
  [Source: _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries]
- Enforce naming/format conventions now (snake_case Rust modules, canonical timestamp/ID conventions, explicit error envelopes) to avoid migration churn in Story 1.2+.  
  [Source: _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules]

### Library & Framework Requirements

- Next.js: `16.2.2` (matches architecture and npm latest at execution time).  
  [Source: _bmad-output/planning-artifacts/architecture.md#Frontend Architecture]  
  [Source: https://registry.npmjs.org/next/latest]
- Axum: `0.8.8` (control API baseline).  
  [Source: _bmad-output/planning-artifacts/architecture.md#API & Communication Patterns]  
  [Source: https://crates.io/api/v1/crates/axum]
- OpenTelemetry crate baseline: `0.31.0`.  
  [Source: _bmad-output/planning-artifacts/architecture.md#Infrastructure & Deployment]  
  [Source: https://crates.io/api/v1/crates/opentelemetry]
- Polymarket SDK baseline: `polymarket-client-sdk 0.4.4` with required `clob` + `ws` features for downstream stories.  
  [Source: _bmad-output/planning-artifacts/architecture.md#API & Communication Patterns]  
  [Source: _bmad-output/planning-artifacts/research/market-rust-polymarket-trading-system-lopez-de-prado-research-2026-04-04.md#Technology Stack Analysis]  
  [Source: https://crates.io/api/v1/crates/polymarket-client-sdk]

### File Structure Requirements

- Target root layout must include: `.github/workflows/`, `apps/operator-console/`, `services/`, `crates/`, `infra/`, `tests/`, `docs/` as defined by architecture.
- Rust shared crates must include `crates/persistence` path scaffold in Story 1.1 to match architecture contracts used by later stories.
- For Story 1.1, core scaffolds must exist and compile; future-epic directories may be placeholders but should respect final path contracts now.
- `.env.example` must remain placeholder-only and be committed; real secrets must be injected at runtime through approved mechanisms.

[Source: _bmad-output/planning-artifacts/architecture.md#Complete Project Directory Structure]  
[Source: _bmad-output/planning-artifacts/architecture.md#File Organization Patterns]

### Testing Requirements

- Rust baseline: workspace-level format/lint/test/build checks in CI (`cargo fmt --check`, lint policy, `cargo test`, build).
- Web baseline: lint/type/build checks for `apps/operator-console`.
- Security baseline: secret scanning + dependency checks + policy checks that reject plaintext secret patterns.
- Bootstrap validation must include negative-path assertions for machine-readable error contract fields: `error_code`, `failed_check`, `remediation_hint`, `timestamp_utc`.
- Bootstrap verification should include clean-clone reproducibility and explicit failure behavior tests for missing env placeholders and invalid setup.

[Source: _bmad-output/planning-artifacts/architecture.md#Starter Template Evaluation]  
[Source: _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules]  
[Source: _bmad-output/planning-artifacts/epics.md#Universal Acceptance Criteria Addendum (Applies to Stories 1.1–6.9)]

### Previous Story Intelligence

- Not applicable. This is Story 1.1 and has no prior story dependency context in Epic 1.

### Git Intelligence Summary

- No implementation-story commit lineage exists yet for adaptation; treat architecture + epics as source of truth and avoid speculative divergence in scaffold layout.

### Latest Technical Information

- Version pins used by architecture (Next.js 16.2.2, Axum 0.8.8, polymarket-client-sdk 0.4.4, OpenTelemetry 0.31.0) are still current at execution time.
- Next.js package requires Node `>=20.9.0`; include this in bootstrap prerequisites to prevent local/CI drift.

[Source: _bmad-output/planning-artifacts/architecture.md#Core Architectural Decisions]  
[Source: https://registry.npmjs.org/next/latest]  
[Source: https://crates.io/api/v1/crates/axum]  
[Source: https://crates.io/api/v1/crates/opentelemetry]  
[Source: https://crates.io/api/v1/crates/polymarket-client-sdk]

### Project Context Reference

- No `project-context.md` file was found in repository scope during discovery.
- Relevant context artifacts were sourced from planning artifacts and referenced research documents instead.

### Project Structure Notes

- The repository currently contains planning artifacts but no implementation scaffold; Story 1.1 must establish the canonical structure exactly once to prevent multi-agent drift.
- Do not implement Story 1.2+ behavior (RBAC/auth/audit business logic) here; only scaffold and policy-safe baseline required for downstream stories.

### References

- _bmad-output/planning-artifacts/epics.md#Epic 1: Secure Operator Access & Governance Control Plane  
- _bmad-output/planning-artifacts/epics.md#Story 1.1: Set Up Initial Project from Starter Template  
- _bmad-output/planning-artifacts/epics.md#Story Execution Standards (Applied to All Stories)  
- _bmad-output/planning-artifacts/architecture.md#Starter Template Evaluation  
- _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries  
- _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules  
- _bmad-output/planning-artifacts/prd.md#Technical Constraints  
- _bmad-output/planning-artifacts/prd.md#Non-Functional Requirements  
- _bmad-output/planning-artifacts/ux-design-specification.md#Design System Foundation  
- _bmad-output/planning-artifacts/ux-design-specification.md#Responsive Design & Accessibility  
- _bmad-output/planning-artifacts/research/domain-polymarket-rust-clob-profit-research-2026-04-04.md#Regulatory and Deployment Constraints  
- _bmad-output/planning-artifacts/research/market-rust-polymarket-trading-system-lopez-de-prado-research-2026-04-04.md#Technology Stack Analysis  
- _bmad-output/planning-artifacts/research/technical-polymarket-claim-validity-audit-research-2026-04-04.md#Applicability to Your Polymarket System

## Story Completion Status

- Story context generated with exhaustive artifact analysis (epics, PRD, architecture, UX, research references, and latest-version checks).
- Story is ready for implementation by dev agents.
- Completion note: Ultimate context engine analysis completed - comprehensive developer guide created.
- 2026-04-05 review pass completed: medium-severity security findings auto-fixed, regression coverage added, and baseline checks/tests passing.

## Dev Agent Record

### Agent Model Used

GPT-5.3-Codex (gpt-5.3-codex)

### Debug Log References

- BMAD create-story workflow execution (automated)
- `npm run qa:test:story-1-1`
- `npm run bootstrap:test`
- `npm run bootstrap:verify`
- `npm run ci:rust`
- `npm run ci:web`
- `npm run ci:security`
- `npm test`

### Completion Notes List

- Implemented a Rust + Next.js monorepo bootstrap baseline with pinned toolchains, reproducible scripts, and policy-safe `.env.example` placeholders.
- Added required architecture-aligned service/crate/app boundary scaffolds (`services/*`, `crates/*`, `apps/operator-console/src/*`, `infra/*`, `tests/*`, `docs/*`).
- Added bootstrap guardrail scripts (`preflight`, `check-bootstrap`, `security-scan`) with machine-readable error envelopes and timestamped check telemetry.
- Added negative-path tests that assert failure contract fields (`error_code`, `failed_check`, `remediation_hint`, `timestamp_utc`).
- Added CI workflows (`ci-rust`, `ci-web`, `security`) that publish `bootstrap-evidence` artifacts with check matrix and command logs.
- Review auto-fixes: expanded `security-scan` to inspect repository source/workflow files and added explicit `contents: read` workflow permissions.
- Added `tests/bootstrap/security-scan.test.mjs` to cover private-key literal and sensitive CLI-arg leakage detection across repository files.
- Added story-focused API and E2E QA automation (`tests/api/bootstrap-cli-api.test.mjs`, `tests/e2e/bootstrap-baseline.e2e.test.mjs`) for machine-readable contract validation and bootstrap evidence flow coverage.
- Remediated QA blocker where a committed private-key literal in a negative-path test triggered security scanning by assembling fixture secret content dynamically in test setup.
- Noted discrepancy during review: `.scripts/bmad-auto/copilot/bmad-progress.log` appears in git status but is outside story application scope.
- Known gap intentionally deferred to Story 1.2+: no RBAC/auth/audit business behavior implemented beyond baseline scaffolding and guardrails.

### File List

- .env.example
- .github/workflows/ci-rust.yml
- .github/workflows/ci-web.yml
- .github/workflows/security.yml
- .gitignore
- Cargo.lock
- Cargo.toml
- README.md
- _bmad-output/implementation-artifacts/deferred-work.md
- _bmad-output/implementation-artifacts/sprint-status.yaml
- _bmad-output/implementation-artifacts/stories/1-1-set-up-initial-project-from-starter-template.md
- _bmad-output/implementation-artifacts/tests/test-summary.md
- apps/operator-console/.gitignore
- apps/operator-console/eslint.config.mjs
- apps/operator-console/next.config.ts
- apps/operator-console/package.json
- apps/operator-console/postcss.config.mjs
- apps/operator-console/public/file.svg
- apps/operator-console/public/globe.svg
- apps/operator-console/public/next.svg
- apps/operator-console/public/vercel.svg
- apps/operator-console/public/window.svg
- apps/operator-console/README.md
- apps/operator-console/src/app/(dashboard)/dashboard/page.tsx
- apps/operator-console/src/app/(governance)/governance/page.tsx
- apps/operator-console/src/app/(incidents)/incidents/page.tsx
- apps/operator-console/src/app/api/health/route.ts
- apps/operator-console/src/app/favicon.ico
- apps/operator-console/src/app/globals.css
- apps/operator-console/src/app/layout.tsx
- apps/operator-console/src/app/page.tsx
- apps/operator-console/src/components/execution/ExecutionSummaryCard.tsx
- apps/operator-console/src/components/governance/GovernanceQueueCard.tsx
- apps/operator-console/src/components/risk/RiskPostureCard.tsx
- apps/operator-console/src/components/timeline/IncidentTimelineCard.tsx
- apps/operator-console/src/hooks/useBootstrapTimestamp.ts
- apps/operator-console/src/lib/env.ts
- apps/operator-console/src/styles/tokens.css
- apps/operator-console/tsconfig.json
- crates/common/Cargo.toml
- crates/common/src/config.rs
- crates/common/src/errors.rs
- crates/common/src/lib.rs
- crates/common/src/telemetry.rs
- crates/common/src/time.rs
- crates/domain/Cargo.toml
- crates/domain/src/events.rs
- crates/domain/src/governance.rs
- crates/domain/src/lib.rs
- crates/domain/src/order.rs
- crates/domain/src/risk.rs
- crates/persistence/Cargo.toml
- crates/persistence/migrations/.gitkeep
- crates/persistence/src/lib.rs
- crates/persistence/src/postgres/mod.rs
- docs/architecture/.gitkeep
- docs/governance/.gitkeep
- docs/operations/bootstrap.md
- docs/runbooks/runtime-least-privilege.md
- infra/docker/.gitkeep
- infra/monitoring/.gitkeep
- infra/systemd/.gitkeep
- infra/terraform/.gitkeep
- package.json
- pnpm-lock.yaml
- pnpm-workspace.yaml
- rust-toolchain.toml
- services/control-api/Cargo.toml
- services/control-api/src/handlers/mod.rs
- services/control-api/src/main.rs
- services/control-api/src/middleware/mod.rs
- services/control-api/src/routes/mod.rs
- services/execution-engine/Cargo.toml
- services/execution-engine/src/ingestion/mod.rs
- services/execution-engine/src/main.rs
- services/execution-engine/src/orders/mod.rs
- services/execution-engine/src/reconciliation/mod.rs
- services/governance-service/Cargo.toml
- services/governance-service/src/approvals/mod.rs
- services/governance-service/src/audit/mod.rs
- services/governance-service/src/main.rs
- services/portfolio-engine/Cargo.toml
- services/portfolio-engine/src/allocation/mod.rs
- services/portfolio-engine/src/attribution/mod.rs
- services/portfolio-engine/src/main.rs
- services/reporting-service/Cargo.toml
- services/reporting-service/src/contracts/mod.rs
- services/reporting-service/src/exports/mod.rs
- services/reporting-service/src/main.rs
- services/research-gateway/Cargo.toml
- services/research-gateway/src/main.rs
- services/research-gateway/src/promotion/mod.rs
- services/research-gateway/src/validation/mod.rs
- services/risk-engine/Cargo.toml
- services/risk-engine/src/gates/mod.rs
- services/risk-engine/src/limits/mod.rs
- services/risk-engine/src/main.rs
- services/risk-engine/src/safe_state/mod.rs
- tests/api/bootstrap-cli-api.test.mjs
- tests/bootstrap/bootstrap-negative.test.mjs
- tests/bootstrap/check-bootstrap.test.mjs
- tests/bootstrap/preflight.test.mjs
- tests/bootstrap/security-scan.test.mjs
- tests/chaos/.gitkeep
- tests/contract/.gitkeep
- tests/e2e/.gitkeep
- tests/e2e/bootstrap-baseline.e2e.test.mjs
- tests/fixtures/valid/.env.example
- tests/fixtures/valid/.github/workflows/ci-rust.yml
- tests/fixtures/valid/.github/workflows/ci-web.yml
- tests/fixtures/valid/.github/workflows/security.yml
- tests/fixtures/valid/apps/operator-console/package.json
- tests/fixtures/valid/Cargo.lock
- tests/fixtures/valid/Cargo.toml
- tests/fixtures/valid/package.json
- tests/fixtures/valid/pnpm-workspace.yaml
- tests/fixtures/valid/rust-toolchain.toml
- tests/integration/.gitkeep
- tools/bootstrap/check-bootstrap.mjs
- tools/bootstrap/check-utils.mjs
- tools/bootstrap/preflight.mjs
- tools/bootstrap/security-scan.mjs

## Change Log

- 2026-04-05: Implemented Story 1.1 bootstrap baseline, guardrails, CI evidence workflows, and negative-path validation tests; moved story status to `review`.
- 2026-04-05: Completed adversarial code review, auto-fixed medium security findings (workflow permissions + repository-wide secret scanning), added regression tests, and moved story status to `done`.
- 2026-04-05: Added automated API/E2E QA coverage for Story 1.1 critical bootstrap flows, generated QA summary artifact, and re-validated CI/security/test gates with story status retained as `done`.
