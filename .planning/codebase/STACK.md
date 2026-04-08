# Technology Stack

**Analysis Date:** 2026-04-08

## Languages

**Primary:**
- Rust 1.91.0 (toolchain pinned) - backend services and shared crates in `services/*` and `crates/*` via `Cargo.toml` and `rust-toolchain.toml`

**Secondary:**
- TypeScript 5.x - Next.js operator UI in `apps/operator-console/src/**` (`apps/operator-console/package.json`, `apps/operator-console/tsconfig.json`)
- JavaScript (Node ESM) - repo tooling and bootstrap/security scripts in `tools/bootstrap/*.mjs`, root scripts in `package.json`

## Runtime

**Environment:**
- Rust async runtime: Tokio (`tokio = "1.48.0"`) in workspace deps at `Cargo.toml`
- Node.js `>=20.9.0` for repo and web workflows in `package.json`
- CI uses Node 22 and Rust 1.91.0 in `.github/workflows/ci-rust.yml`, `.github/workflows/ci-web.yml`, `.github/workflows/security.yml`

**Package Manager:**
- JS package manager: pnpm `10.10.0` (`packageManager` in `package.json`, setup in `.github/workflows/*.yml`)
- Rust package manager/build: Cargo (`Cargo.toml`)
- Lockfile: present (`pnpm-lock.yaml`, `Cargo.lock`)

## Frameworks

**Core:**
- Axum `0.8.8` for HTTP APIs in `services/control-api/src/main.rs`, `services/reporting-service/src/api.rs`, workspace pin in `Cargo.toml`
- Next.js `16.2.2` with React `19.2.4` for operator console in `apps/operator-console/package.json`
- SQLx `0.8.6` with Postgres and `runtime-tokio-rustls` in workspace deps (`Cargo.toml`), used across `crates/persistence/src/postgres/*.rs`

**Testing:**
- Rust built-in test harness via `cargo test` commands in `package.json`
- Node built-in test runner (`node --test`) in root scripts in `package.json`

**Build/Dev:**
- Cargo fmt/clippy/build/test via `rust:*` scripts in `package.json`
- ESLint 9 + `eslint-config-next` in `apps/operator-console/eslint.config.mjs`
- TypeScript `tsc --noEmit` in `apps/operator-console/package.json`
- Next build/dev via `next build` and `next dev` in `apps/operator-console/package.json`

## Key Dependencies

**Critical:**
- `polymarket-client-sdk = 0.4.4` (CLOB + WS features) for market/user stream ingestion in `services/execution-engine/src/ingestion/mod.rs` and `services/execution-engine/src/ingestion/user_stream.rs`
- `sqlx = 0.8.6` for durable state and governance persistence in `services/*/src/main.rs` and `crates/persistence/src/postgres/*.rs`
- `axum = 0.8.8` for control/reporting HTTP surfaces in `services/control-api/src/routes/mod.rs` and `services/reporting-service/src/api.rs`

**Infrastructure:**
- `opentelemetry = 0.31.0` declared in workspace and `crates/common/Cargo.toml` (current direct usage is minimal in `crates/common/src/telemetry.rs`)
- `time = 0.3.44` for RFC3339 timestamps and scheduling logic across `services/*` and `crates/domain/*`
- Frontend runtime deps `next`, `react`, `react-dom` in `apps/operator-console/package.json`

## Configuration

**Environment:**
- Runtime config is environment-variable driven in Rust binaries (`services/control-api/src/main.rs`, `services/execution-engine/src/main.rs`, `services/risk-engine/src/main.rs`, `services/reporting-service/src/main.rs`)
- Frontend API base URL is environment-variable driven in `apps/operator-console/src/lib/env.ts`
- `.env`, `.env.example`, and `tests/fixtures/valid/.env.example` exist but are not read here; verify runtime values there and in deployment secret stores

**Build:**
- Workspace configuration in `Cargo.toml` and `pnpm-workspace.yaml`
- Rust toolchain pin in `rust-toolchain.toml`
- Next/Turbopack config in `apps/operator-console/next.config.ts`
- TypeScript and lint config in `apps/operator-console/tsconfig.json`, `apps/operator-console/eslint.config.mjs`

## Platform Requirements

**Development:**
- Install Rust 1.91.0 with `clippy` and `rustfmt` (`rust-toolchain.toml`)
- Install Node `>=20.9.0` and pnpm `>=10 <11` (`package.json`)
- Provide Postgres connectivity through `DATABASE_URL` for durable service paths (`services/control-api/src/main.rs`, `services/execution-engine/src/main.rs`)

**Production:**
- Backend services are designed as Rust processes with Postgres dependency (`services/*/src/main.rs`, `crates/persistence/src/postgres/*.rs`)
- Operator UI deploy target is a Next.js app (`apps/operator-console/package.json`); concrete hosting platform is not detected in repo config and should be verified from deployment environment

---

*Stack analysis: 2026-04-08*
