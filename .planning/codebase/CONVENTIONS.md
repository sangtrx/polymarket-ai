# Coding Conventions

**Analysis Date:** 2026-04-08

## Naming Patterns

**Files:**
- Use `PascalCase.tsx` for React components in `apps/operator-console/src/components/**` (example: `apps/operator-console/src/components/risk/RiskCommandSurface.tsx`).
- Use `kebab-case.ts` for frontend utility modules in `apps/operator-console/src/lib/**` (example: `apps/operator-console/src/lib/risk/control-actions.ts`).
- Use `snake_case.rs` module files and `mod.rs` directories in Rust services/crates (examples: `services/governance-service/src/risk_limits/mod.rs`, `crates/domain/src/risk.rs`).
- Use `*.test.mjs` and `*.e2e.test.mjs` for Node test suites under `tests/**` (examples: `tests/api/story-3-2-safety-control-api.test.mjs`, `tests/e2e/story-3-2-risk-safety-rail.e2e.test.mjs`).

**Functions:**
- Use `camelCase` in TypeScript/JavaScript (examples: `getOperatorConsoleEnv` in `apps/operator-console/src/lib/env.ts`, `resolveRiskPostureViewModel` in `apps/operator-console/src/lib/risk/posture.ts`).
- Use `snake_case` in Rust (examples: `validate_risk_limit_profile_version` in `crates/domain/src/risk.rs`, `risk_limit_service_error_response` in `services/control-api/src/routes/mod.rs`).
- Use descriptive test names as behavior sentences (examples: `critical_increase_without_approval_reference_returns_pending` in `services/governance-service/src/risk_limits/mod.rs`, `Story 3.2 API surfaces machine-readable errors with no success-shaped fallback` in `tests/api/story-3-2-safety-control-api.test.mjs`).

**Variables:**
- Use `SCREAMING_SNAKE_CASE` constants for both Rust and TS (examples: `EMERGENCY_CONTROL_ACK_MAX_SECONDS` in `crates/domain/src/risk.rs`, `RECOVERY_READINESS_ENDPOINT` in `apps/operator-console/src/lib/risk/control-actions.ts`).
- Use `camelCase` locals/props in TS and JS (examples: `activePosture` in `apps/operator-console/src/components/risk/RiskCommandSurface.tsx`, `compiledOutputDir` in `tests/api/story-3-2-safety-control-api.test.mjs`).

**Types:**
- Use `PascalCase` for Rust structs/enums and TS types/interfaces (examples: `RiskLimitServiceError` in `services/governance-service/src/risk_limits/mod.rs`, `RecoveryReadinessDecision` in `apps/operator-console/src/lib/risk/control-actions.ts`).

## Code Style

**Formatting:**
- Rust formatting is enforced with `cargo fmt --all -- --check` via `rust:fmt` in `package.json`.
- Frontend formatting is lint-driven; no Prettier config was detected in repo root or `apps/operator-console/` (verify by checking for `.prettierrc*` and `prettier` scripts in `package.json` and `apps/operator-console/package.json`).
- TypeScript style aligns with ESLint + Next defaults from `apps/operator-console/eslint.config.mjs`.

**Linting:**
- Rust linting uses `cargo clippy --workspace --all-targets -- -D warnings` via `rust:lint` in `package.json` (warnings are treated as errors).
- Web linting uses `eslint . --max-warnings=0` in `apps/operator-console/package.json`.
- ESLint extends Next core web vitals and TypeScript presets in `apps/operator-console/eslint.config.mjs`.

## Import Organization

**Order:**
1. External libraries/runtime modules first (example: `next/link` in `apps/operator-console/src/components/shell/OperatorShellLayout.tsx`; `axum` in `services/control-api/src/routes/mod.rs`).
2. Internal monorepo aliases/modules second (example: `@/components/...` and `@/lib/...` in `apps/operator-console/src/app/(dashboard)/dashboard/page.tsx`; `domain::...` and `governance_service::...` in `services/control-api/src/routes/mod.rs`).
3. Local relative imports and type-only imports where needed (example: `import type { ... } from "./control-actions";` in `apps/operator-console/src/lib/risk/posture.ts`).

**Path Aliases:**
- Use `@/*` alias for operator-console source imports, configured in `apps/operator-console/tsconfig.json`.

## Error Handling

**Patterns:**
- Use structured, machine-readable error payloads with `code`/`error_code`, `message`, and optional field-level issues (examples: `RiskLimitServiceError` in `services/governance-service/src/risk_limits/mod.rs`; `RiskLimitServiceErrorResponse` returned by `risk_limit_service_error_response` in `services/control-api/src/routes/mod.rs`).
- Map domain/service error codes to HTTP status explicitly with per-domain status mapper functions (example: `risk_limit_service_error_status` in `services/control-api/src/routes/mod.rs`).
- Use custom typed error classes on frontend API clients and throw early on contract validation failures (example: `EmergencyControlClientError` in `apps/operator-console/src/lib/risk/control-actions.ts`).
- Use `expect_err` / `assert.rejects` in tests to enforce negative-path contracts (examples: `services/governance-service/src/risk_limits/mod.rs`, `tests/api/story-3-2-safety-control-api.test.mjs`).

## Logging

**Framework:** `println!` JSON event logging in Rust services; no dedicated logging library detected in sampled files.

**Patterns:**
- Emit structured telemetry events as serialized JSON in orchestrator modules (example: `emit_risk_limit_telemetry` in `services/governance-service/src/risk_limits/mod.rs`).
- Emit startup lifecycle logs in service entrypoints (example: `println!("control-api bootstrap ready...")` in `services/control-api/src/main.rs`).
- Frontend modules rely on explicit error surfaces instead of console logging (example: `apps/operator-console/src/lib/risk/control-actions.ts`).

## Comments

**When to Comment:**
- Keep comments sparse and intent-focused; most code is self-descriptive through naming.
- Use comments mainly in config/override contexts (example: ignore override comments in `apps/operator-console/eslint.config.mjs`).

**JSDoc/TSDoc:**
- Not detected in sampled TS/JS source (`apps/operator-console/src/**`) and test files (`tests/**`).
- Verify if new public APIs need docs in future modules; current baseline does not enforce docblock conventions.

## Function Design

**Size:** Large orchestration functions are accepted when they centralize policy and response mapping (examples: route handlers and mappers in `services/control-api/src/routes/mod.rs`; parser/normalizer pipeline in `apps/operator-console/src/lib/risk/control-actions.ts`).

**Parameters:**
- Prefer typed input structs/interfaces over many positional parameters (examples: `UpsertRiskLimitProfileInput` in `services/governance-service/src/risk_limits/mod.rs`, `RecoveryReadinessRequestInput` in `apps/operator-console/src/lib/risk/control-actions.ts`).
- Use `#[allow(clippy::too_many_arguments)]` only for constructor/wiring boundaries (example: `with_all_orchestrators*` in `services/control-api/src/middleware/mod.rs`).

**Return Values:**
- Return `Result<T, E>` in Rust service/domain layers and propagate typed errors (examples across `crates/domain/src/*.rs`, `services/governance-service/src/risk_limits/mod.rs`).
- Return typed DTOs or throw typed errors in TS API adapters (example: `invokeEmergencyControlAction` and `EmergencyControlClientError` in `apps/operator-console/src/lib/risk/control-actions.ts`).

## Module Design

**Exports:**
- Rust modules expose domain/service surfaces through `pub mod` in crate roots (examples: `crates/domain/src/lib.rs`, `services/governance-service/src/lib.rs`).
- TS frontend favors named exports for shared logic and component functions, with Next route modules using `export default` page handlers (example: `apps/operator-console/src/app/(dashboard)/dashboard/page.tsx`).

**Barrel Files:**
- Rust crate roots act as explicit module barrels via `lib.rs`.
- TypeScript barrel `index.ts` patterns were not detected in `apps/operator-console/src/**`; import from concrete module paths directly.

---

*Convention analysis: 2026-04-08*
