# Codebase Structure

**Analysis Date:** 2026-04-08

## Directory Layout

```text
polymarket-ai/
├── apps/operator-console/      # Next.js operator UI (App Router + components + typed clients)
├── crates/common/              # Shared runtime helpers (config/errors/time/telemetry shims)
├── crates/domain/              # Domain contracts, enums, validators, reason codes
├── crates/persistence/         # PostgreSQL adapters and SQL-backed repositories
├── services/control-api/       # Axum control-plane API composition and routing
├── services/execution-engine/  # Market/user stream ingestion and order lifecycle runtime seams
├── services/risk-engine/       # Pre-trade gates, runtime policy state, fail-closed controls
├── services/governance-service/# Governance/recovery/risk-limit orchestration services
├── services/reporting-service/ # Reporting read models, contract metadata, export scheduling
├── services/research-gateway/  # Validation/promotion/alpha-lifecycle orchestration
├── tests/                      # Node test suites (api/e2e/story/bootstrap/contract)
├── tools/bootstrap/            # Environment/security/bootstrap validation scripts
└── docs/                       # Operations/governance/runbook reference docs
```

## Directory Purposes

**`apps/operator-console/src/`:**
- Purpose: UI delivery layer for operator workflows.
- Contains: App routes in `app/`, reusable UI in `components/`, API clients/view-model logic in `lib/`.
- Key files: `apps/operator-console/src/app/(dashboard)/dashboard/page.tsx`, `apps/operator-console/src/components/risk/RiskCommandSurface.tsx`, `apps/operator-console/src/lib/risk/control-actions.ts`.

**`crates/domain/src/`:**
- Purpose: Canonical business contracts and validation logic.
- Contains: Modules grouped by bounded context (`risk.rs`, `governance.rs`, `research.rs`, `reporting.rs`, etc.).
- Key files: `crates/domain/src/risk.rs`, `crates/domain/src/governance.rs`, `crates/domain/src/research.rs`.

**`crates/persistence/src/postgres/`:**
- Purpose: SQL persistence boundary.
- Contains: One adapter file per capability/table family and error mapping.
- Key files: `crates/persistence/src/postgres/approvals.rs`, `crates/persistence/src/postgres/market_stream.rs`, `crates/persistence/src/postgres/validation_runs.rs`.

**`services/control-api/src/`:**
- Purpose: HTTP control surface and dependency wiring.
- Contains: Router in `routes/mod.rs`, middleware and state in `middleware/mod.rs`, composition root in `main.rs`.
- Key files: `services/control-api/src/main.rs`, `services/control-api/src/routes/mod.rs`, `services/control-api/src/middleware/mod.rs`.

**`services/*/src/` (engine/orchestration services):**
- Purpose: Service-specific orchestration and runtime loops.
- Contains: `mod.rs` per capability with ports/services, plus `main.rs` composition entry.
- Key files: `services/execution-engine/src/ingestion/mod.rs`, `services/risk-engine/src/gates/mod.rs`, `services/governance-service/src/approvals/mod.rs`, `services/reporting-service/src/exports/scheduling.rs`, `services/research-gateway/src/validation/workflow_runs.rs`.

## Key File Locations

**Entry Points:**
- `services/control-api/src/main.rs`: Build control API dependencies and router.
- `services/execution-engine/src/main.rs`: Start market/user/freshness runtimes.
- `services/risk-engine/src/main.rs`: Hydrate runtime state and evaluate gate bootstrap.
- `services/reporting-service/src/main.rs`: Build reporting API state and run scheduler loop.
- `apps/operator-console/src/app/layout.tsx`: Root Next.js layout entry.
- `apps/operator-console/src/app/page.tsx`: Redirect root to `/dashboard`.

**Configuration:**
- `Cargo.toml`: Workspace members and shared dependency versions.
- `package.json`: Monorepo scripts for bootstrap, CI, story-level tests.
- `apps/operator-console/next.config.ts`: Next.js app runtime config (verify for route/base-path changes).
- `crates/common/src/config.rs`: Shared runtime identity/config helpers.

**Core Logic:**
- `crates/domain/src/*.rs`: Domain decision and validation logic.
- `services/governance-service/src/*/mod.rs`: Governance, recovery, policy orchestration.
- `services/risk-engine/src/gates/mod.rs`: Pre-trade and safe-state gate evaluation.
- `services/execution-engine/src/orders/mod.rs`: Order lifecycle and adjudication flow.

**Testing:**
- `tests/api/*.test.mjs`: API contract checks by story.
- `tests/e2e/*.e2e.test.mjs`: E2E behavior checks by story.
- `tests/story-*/**/*.test.mjs`: Story acceptance tests.
- Unit tests are colocated in Rust modules under `#[cfg(test)]` blocks (for example `services/risk-engine/src/safe_state/mod.rs`).

## Naming Conventions

**Files:**
- Rust modules use snake_case file names under module directories (example: `services/research-gateway/src/promotion/counterfactual_replay.rs`).
- Service capability modules are typically `mod.rs` inside a snake_case directory (example: `services/governance-service/src/risk_limits/mod.rs`).
- Next.js route files follow App Router conventions (`page.tsx`, `layout.tsx`, `route.ts`) under `apps/operator-console/src/app/**`.

**Directories:**
- Service/crate package directories use kebab-case at top level (example: `services/control-api`, `services/research-gateway`).
- Rust internal module directories use snake_case (example: `services/reporting-service/src/read_models`).

## Where to Add New Code

**New backend control feature:**
- Primary code: Add orchestrator/service logic under the owning service module in `services/governance-service/src/<capability>/mod.rs` or `services/research-gateway/src/<capability>/mod.rs`.
- API route wiring: Add endpoint and handler function in `services/control-api/src/routes/mod.rs`.
- Dependency injection: Add orchestrator field/builder updates in `services/control-api/src/middleware/mod.rs` and wire concrete adapter in `services/control-api/src/main.rs`.
- Tests: Add Rust unit tests in the same module and story/API tests in `tests/api/` and `tests/e2e/`.

**New persistence-backed capability:**
- Implementation: Add SQL adapter module to `crates/persistence/src/postgres/<capability>.rs` and export it in `crates/persistence/src/postgres/mod.rs`.
- Schema: Add migration file in `crates/persistence/migrations/` with timestamp-prefix naming.
- Service hookup: Inject new repository/store into service `postgres(pool)` constructor.

**New UI workflow/module:**
- Route page: Add route under `apps/operator-console/src/app/(dashboard|incidents|governance)/.../page.tsx` (or add a new route group if needed).
- Components: Add reusable UI in `apps/operator-console/src/components/<domain>/`.
- API client/utilities: Add typed fetch client in `apps/operator-console/src/lib/<domain>/`.

**Utilities:**
- Shared Rust helpers: `crates/common/src/`.
- Shared TypeScript helpers for UI: `apps/operator-console/src/lib/`.

## Special Directories

**`crates/persistence/migrations/`:**
- Purpose: Ordered SQL schema migrations.
- Generated: No.
- Committed: Yes.

**`tests/fixtures/valid/`:**
- Purpose: Golden fixture workspace used by bootstrap/security tests.
- Generated: No.
- Committed: Yes.

**`apps/operator-console/.next/`:**
- Purpose: Next.js build artifacts.
- Generated: Yes.
- Committed: Yes in current repository snapshot (verify whether this should remain tracked).

**`target/`:**
- Purpose: Rust build artifacts.
- Generated: Yes.
- Committed: Not typically expected; verify repository policy if cleanup is needed.

---

*Structure analysis: 2026-04-08*
