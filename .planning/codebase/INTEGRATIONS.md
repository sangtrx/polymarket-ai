# External Integrations

**Analysis Date:** 2026-04-08

## APIs & External Services

**Market Data & Trading Streams:**
- Polymarket CLOB WebSocket - market book and user stream ingestion in `services/execution-engine/src/ingestion/mod.rs` and `services/execution-engine/src/ingestion/user_stream.rs`
  - SDK/Client: `polymarket-client-sdk` (`Cargo.toml`, `services/execution-engine/Cargo.toml`)
  - Auth: `EXECUTION_POLYMARKET_API_KEY`, `EXECUTION_POLYMARKET_API_SECRET`, `EXECUTION_POLYMARKET_API_PASSPHRASE`, `EXECUTION_POLYMARKET_ADDRESS` read in `services/execution-engine/src/ingestion/user_stream.rs`

**Internal Service APIs:**
- Control API endpoints under `/control/*` consumed by operator console clients in:
  - `apps/operator-console/src/lib/risk/control-actions.ts`
  - `apps/operator-console/src/lib/portfolio/allocation-policy.ts`
  - `apps/operator-console/src/lib/incidents/alerts.ts`
  - `apps/operator-console/src/lib/governance/readiness.ts`
  - SDK/Client: browser/server `fetch` wrappers in each file above
  - Auth: no frontend auth header injection is implemented in these client wrappers; server-side API auth expects `authorization` and `x-correlation-id` headers in `services/control-api/src/middleware/mod.rs`

## Data Storage

**Databases:**
- PostgreSQL (via SQLx) is the primary durable store
  - Connection: `DATABASE_URL` in `services/control-api/src/main.rs`, `services/execution-engine/src/main.rs`, `services/risk-engine/src/main.rs`, `services/reporting-service/src/main.rs`
  - Client: SQLx Postgres (`Cargo.toml`, `crates/persistence/src/postgres/*.rs`)
  - Schema migrations: `crates/persistence/migrations/*.sql`

**File Storage:**
- Local filesystem for generated contract artifacts and schema files in `services/reporting-service/src/contracts/artifacts/v1/*`
- No external object store (S3/GCS/Azure Blob) integration detected in `services/*` or `crates/*`

**Caching:**
- None detected (no Redis/Memcached integration in `services/*` or `crates/*`)

## Authentication & Identity

**Auth Provider:**
- Custom header-based auth in control API
  - Implementation: `Bearer <actor_id>:<role>:<expires_unix>` parsing and validation in `services/control-api/src/middleware/mod.rs`
  - Authorization: role/action evaluation via `AuthorizationEvaluator` in `services/control-api/src/middleware/mod.rs` and `services/control-api/src/main.rs`

## Monitoring & Observability

**Error Tracking:**
- Dedicated external error tracking service not detected (no Sentry/Datadog SDK usage in `services/*`, `crates/*`, or `apps/operator-console/src/*`)

**Logs:**
- Structured telemetry emitted as JSON to stdout/stderr via `println!`/`eprintln!`, for example:
  - `services/execution-engine/src/ingestion/mod.rs`
  - `services/risk-engine/src/gates/mod.rs`
  - `services/reporting-service/src/api.rs`
  - `services/reporting-service/src/exports/scheduling.rs`
- `opentelemetry` crate is present but current direct usage is minimal in `crates/common/src/telemetry.rs`

## CI/CD & Deployment

**Hosting:**
- Runtime/deployment platform is not declared in repo infra; `infra/docker`, `infra/terraform`, `infra/systemd`, and `infra/monitoring` only contain `.gitkeep`
- Verify deployment target outside repo or in private ops configuration

**CI Pipeline:**
- GitHub Actions workflows:
  - Rust pipeline: `.github/workflows/ci-rust.yml`
  - Web pipeline: `.github/workflows/ci-web.yml`
  - Security pipeline: `.github/workflows/security.yml`

## Environment Configuration

**Required env vars:**
- Database: `DATABASE_URL`
- Operator console API base: `NEXT_PUBLIC_OPERATOR_CONSOLE_API_BASE_URL` or fallback `OPERATOR_CONSOLE_PUBLIC_API_BASE_URL` (`apps/operator-console/src/lib/env.ts`)
- Execution market stream: `EXECUTION_MARKET_STREAM_ASSET_IDS` (+ optional `EXECUTION_MARKET_STREAM_*`) in `services/execution-engine/src/ingestion/mod.rs`
- Execution user stream: `EXECUTION_USER_STREAM_MARKET_IDS`, `EXECUTION_POLYMARKET_API_KEY`, `EXECUTION_POLYMARKET_API_SECRET`, `EXECUTION_POLYMARKET_API_PASSPHRASE`, `EXECUTION_POLYMARKET_ADDRESS` in `services/execution-engine/src/ingestion/user_stream.rs`
- Freshness gate tuning: `EXECUTION_FRESHNESS_*` vars in `services/execution-engine/src/ingestion/freshness_gate.rs`
- Reporting scheduler: `REPORT_SCHEDULER_TICK_SECONDS` in `services/reporting-service/src/main.rs`
- Risk bootstrap toggles: `RISK_ENGINE_BOOTSTRAP_*` vars in `services/risk-engine/src/main.rs`

**Secrets location:**
- `.env` and `.env.example` files are present at repo root; secrets are expected to be runtime-injected (per `README.md`)
- This mapping does not read or expose any secret file contents

## Webhooks & Callbacks

**Incoming:**
- None explicitly detected as webhook endpoints (control API routes are command/query style under `/control/*` in `services/control-api/src/routes/mod.rs`)

**Outgoing:**
- No outbound HTTP webhook dispatch implementation detected in `services/*` or `crates/*`
- Alert channels (`pagerduty`, `slack`, `email`) are represented in domain models (`crates/domain/src/alerts.rs`, `apps/operator-console/src/lib/incidents/alerts.ts`) but provider clients/callback dispatch code is not detected

---

*Integration audit: 2026-04-08*
