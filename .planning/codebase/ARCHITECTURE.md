# Architecture

**Analysis Date:** 2026-04-08

## Pattern Overview

**Overall:** Modular monorepo with layered domain-driven services and explicit port/adapter seams.

**Key Characteristics:**
- Keep pure contracts and decision logic in `crates/domain/src/*.rs`, then orchestrate use-cases in `services/*/src/**/mod.rs`.
- Use trait ports for boundaries (for example `ApprovalRepositoryPort` in `services/governance-service/src/approvals/mod.rs` and `MarketStreamStore` in `services/execution-engine/src/ingestion/mod.rs`).
- Bind PostgreSQL adapters in composition roots (`services/control-api/src/main.rs`, `services/risk-engine/src/main.rs`, `services/execution-engine/src/main.rs`) and pass them as `Arc<dyn ...>`.

## Layers

**Domain Contract Layer:**
- Purpose: Own canonical entities, enums, validation, and reason-code contracts.
- Location: `crates/domain/src/`
- Contains: Modules like `risk.rs`, `governance.rs`, `reporting.rs`, `research.rs`, `allocation.rs`.
- Depends on: Primarily `serde` and standard library; no service-level dependencies detected.
- Used by: All service crates and persistence adapters.

**Persistence Adapter Layer:**
- Purpose: Translate domain records to SQL and back.
- Location: `crates/persistence/src/postgres/`
- Contains: Per-capability modules such as `approvals.rs`, `market_stream.rs`, `risk_limits.rs`, `validation_runs.rs`.
- Depends on: `sqlx`, `domain`, `common`.
- Used by: Service orchestrators via repository/store ports in `services/*`.

**Orchestration/Application Layer:**
- Purpose: Implement business workflows behind port traits.
- Location: `services/governance-service/src/**`, `services/reporting-service/src/**`, `services/research-gateway/src/**`, `services/risk-engine/src/**`, `services/execution-engine/src/**`, `services/portfolio-engine/src/**`.
- Contains: `*Service` structs, `*Orchestrator` traits, input/evidence DTOs, telemetry emission.
- Depends on: `domain` contracts and `persistence` adapters.
- Used by: `services/control-api` routes and service `main.rs` composition roots.

**API/Delivery Layer:**
- Purpose: Expose HTTP control surface and reporting endpoints.
- Location: `services/control-api/src/routes/mod.rs`, `services/reporting-service/src/api.rs`.
- Contains: `axum::Router` construction, request parsing, auth middleware integration.
- Depends on: Orchestrator traits in service crates and middleware state in `services/control-api/src/middleware/mod.rs`.
- Used by: Runtime process entry points (`services/control-api/src/main.rs`, `services/reporting-service/src/main.rs`).

**UI Layer:**
- Purpose: Operator shell and workflow surfaces.
- Location: `apps/operator-console/src/app/**`, `apps/operator-console/src/components/**`, `apps/operator-console/src/lib/**`.
- Contains: Next.js App Router pages, client components, typed API clients to `/control/*`.
- Depends on: Browser/server `fetch`, URL-derived shell read-model state, control-api contracts.
- Used by: Operator workflows in dashboard/governance/incidents routes.

## Data Flow

**Control mutation flow (HTTP to domain evidence):**

1. Route handler in `services/control-api/src/routes/mod.rs` authenticates through `require_authenticated_actor` from `services/control-api/src/middleware/mod.rs`.
2. Handler builds typed input and calls orchestrator trait (for example `RiskLimitOrchestrator` or `RecoveryOrchestrator`) stored in `ControlApiState`.
3. Orchestrator implementation (for example `RiskLimitService` in `services/governance-service/src/risk_limits/mod.rs`) validates payload/role, performs workflow, and persists through repository port backed by `crates/persistence/src/postgres/*.rs`.

**Execution + risk gating flow (stream to trade admission):**

1. `services/execution-engine/src/ingestion/mod.rs` and `services/execution-engine/src/ingestion/user_stream.rs` ingest Polymarket WS events and persist stream evidence.
2. Freshness controller in `services/execution-engine/src/ingestion/freshness_gate.rs` emits pause/allow state and persists gate events.
3. `services/risk-engine/src/main.rs` hydrates runtime policy/limit state from Postgres, then evaluates intents with `evaluate_order_intent_gate_with_limit_state_and_safe_state` in `services/risk-engine/src/gates/mod.rs`.

**Operator console read/control flow:**

1. Next.js route pages (for example `apps/operator-console/src/app/(dashboard)/dashboard/page.tsx`) resolve shell state and risk posture from URL/env.
2. Client components call typed clients in `apps/operator-console/src/lib/**` (for example `risk/control-actions.ts`, `portfolio/allocation-policy.ts`).
3. Clients call control-api endpoints (`/control/*`) and update local UI state (for example `apps/operator-console/src/components/risk/RiskCommandSurface.tsx`).

**State Management:**
- Use fail-closed in-memory runtime state for live gating (`InMemoryRuntimePolicyState` in `services/risk-engine/src/gates/mod.rs`, `InMemoryRiskLimitState` in `services/risk-engine/src/limits/mod.rs`, `SharedFreshnessSignals` in `services/execution-engine/src/ingestion/freshness_gate.rs`) and hydrate from PostgreSQL on startup.
- Use immutable evidence records persisted in PostgreSQL via `crates/persistence/src/postgres/*`.

## Key Abstractions

**Port/Adapter Traits:**
- Purpose: Isolate domain workflow from storage and external transports.
- Examples: `services/governance-service/src/approvals/mod.rs`, `services/research-gateway/src/validation/workflow_runs.rs`, `services/execution-engine/src/orders/mod.rs`.
- Pattern: `trait ...Port/Repository/Orchestrator` + `in_memory()` + `postgres(pool)` implementations.

**Control API shared state container:**
- Purpose: Runtime dependency graph for all control endpoints.
- Examples: `services/control-api/src/middleware/mod.rs` (`ControlApiState` and `with_*_orchestrator` builders), wired in `services/control-api/src/main.rs`.
- Pattern: Builder-style dependency injection using `Arc<dyn Trait>`.

**Evidence-first contracts:**
- Purpose: Return machine-readable reason codes, correlation IDs, timestamps.
- Examples: `services/governance-service/src/recovery/mod.rs` (`RecoveryResumeExecutionEvidence`), `services/reporting-service/src/read_models/queries.rs` (`ReportingDatasetResponse`), `services/research-gateway/src/validation/gate_policies.rs`.
- Pattern: Service methods return typed evidence DTOs rather than primitive booleans.

## Entry Points

**Control API binary:**
- Location: `services/control-api/src/main.rs`
- Triggers: Process startup.
- Responsibilities: Open `PgPool`, construct all orchestrators, assemble `ControlApiState`, build router through `routes::app_router`.

**Execution engine binary:**
- Location: `services/execution-engine/src/main.rs`
- Triggers: Process startup.
- Responsibilities: Bootstrap market/user ingestion runtimes, attach freshness/order lifecycle seams, run async loops with fail-closed `tokio::select!`.

**Risk engine binary:**
- Location: `services/risk-engine/src/main.rs`
- Triggers: Process startup.
- Responsibilities: Hydrate runtime policy/limit state from Postgres and env, evaluate bootstrap intent, enforce fail-closed containment defaults.

**Operator console web entry:**
- Location: `apps/operator-console/src/app/layout.tsx` and route pages under `apps/operator-console/src/app/(dashboard|incidents|governance)/**`.
- Triggers: HTTP requests to Next.js app routes.
- Responsibilities: Compose shell layout, tabbed route content, and client components invoking control/reporting clients.

## Error Handling

**Strategy:** Typed error structs with stable machine codes mapped across layers.

**Patterns:**
- Create per-module errors (`ApprovalServiceError`, `MarketIngestionError`, `ReportingReadModelError`) and map lower-level errors (`map_persistence_error` style) in files like `services/governance-service/src/approvals/mod.rs` and `services/reporting-service/src/read_models/queries.rs`.
- Fail closed for runtime risk paths (explicit defaults and panic/deny behavior in `services/risk-engine/src/main.rs` and `services/execution-engine/src/main.rs`).

## Cross-Cutting Concerns

**Logging:** Emit structured JSON/event logs through `println!`/`eprintln!` with reason codes and timestamps (for example `services/risk-engine/src/safe_state/mod.rs`, `services/execution-engine/src/ingestion/freshness_gate.rs`).
**Validation:** Validate inputs at domain and orchestrator boundaries before persistence (for example `validate_risk_limit_profile_version` in `services/governance-service/src/risk_limits/mod.rs`, timestamp checks in `services/execution-engine/src/ingestion/freshness_gate.rs`).
**Authentication:** Enforce bearer-header parsing plus role-based authorization in middleware (`services/control-api/src/middleware/mod.rs`) before privileged control routes in `services/control-api/src/routes/mod.rs`.

---

*Architecture analysis: 2026-04-08*
