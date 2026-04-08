# Testing Patterns

**Analysis Date:** 2026-04-08

## Test Framework

**Runner:**
- Rust: built-in `cargo test`/libtest with async support via `#[tokio::test]` (example: `services/control-api/src/routes/mod.rs`).
- Node: built-in `node:test` runner for `.mjs` suites (example: `tests/bootstrap/preflight.test.mjs`).
- Config: No `jest.config.*` or `vitest.config.*` detected at repo root; test orchestration is script-driven from `package.json`.

**Assertion Library:**
- Rust standard assertions: `assert!`, `assert_eq!`, `expect`, `expect_err` (examples in `crates/domain/src/risk.rs`, `services/governance-service/src/risk_limits/mod.rs`).
- Node strict assertions: `node:assert/strict` (`assert.equal`, `assert.match`, `assert.rejects`, `assert.throws`) in `tests/**`.

**Run Commands:**
```bash
npm run test                 # Node bootstrap suites + Rust workspace tests
npm run rust:test            # cargo test --workspace --all-targets
npm run bootstrap:test       # node --test tests/bootstrap/*.test.mjs tests/api/*.test.mjs tests/e2e/*.test.mjs
```
- Watch mode script: Not configured in `package.json`; verify ad-hoc usage with `node --test --watch` or custom `cargo test` invocations.
- Coverage command: Not detected in `package.json`, `apps/operator-console/package.json`, or `.github/workflows/*.yml`.

## Test File Organization

**Location:**
- Rust tests are mostly co-located inside production modules under `#[cfg(test)]` blocks (examples: `crates/domain/src/risk.rs`, `services/control-api/src/routes/mod.rs`, `crates/persistence/src/postgres/risk_limits.rs`).
- Node tests are centralized under top-level `tests/` by suite type (`tests/bootstrap`, `tests/api`, `tests/story-*`, `tests/e2e`, `tests/contract`).

**Naming:**
- Rust test function names follow behavior-driven snake_case (example: `upsert_rejects_invalid_cross_scope_payload_with_field_errors` in `services/governance-service/src/risk_limits/mod.rs`).
- Node file names are story/scenario scoped: `story-<x>-<name>.test.mjs` and `*.e2e.test.mjs` (examples in `tests/api/` and `tests/e2e/`).

**Structure:**
```
tests/
├── bootstrap/      # Toolchain/bootstrap contract tests
├── api/            # API/client behavior tests (Node test runner)
├── story-*/        # Story-level source contract tests
├── contract/       # Contract artifact tests
└── e2e/            # End-to-end scenario contract tests
```

## Test Structure

**Suite Organization:**
```typescript
test("Story 3.2 API surfaces machine-readable errors with no success-shaped fallback", async () => {
  await assert.rejects(invokeEmergencyControlAction({...}), (error) => {
    assert.equal(error.status, 403);
    assert.equal(error.errorCode, "authorization_denied");
    return true;
  });
});
```
Pattern from `tests/api/story-3-2-safety-control-api.test.mjs`.

```rust
#[tokio::test]
async fn report_schedule_upsert_endpoint_returns_schedule_evidence() {
    let response = test_app_with_report_schedule_orchestrator(Arc::new(
        StubReportScheduleOrchestrator::default(),
    ))
    .oneshot(Request::builder().uri("/control/report-schedules/report-schedule-daily") /* ... */)
    .await
    .expect("request should complete");
    assert_eq!(response.status(), StatusCode::ACCEPTED);
}
```
Pattern from `services/control-api/src/routes/mod.rs`.

**Patterns:**
- Setup pattern: local helper builders/factories per module (examples: `sample_input()` in `services/governance-service/src/risk_limits/mod.rs`, `loadCompiledModules()` in `tests/api/story-3-2-safety-control-api.test.mjs`).
- Teardown pattern: explicit cleanup with `test.after` + file deletion for temp outputs (example: `tests/api/story-3-2-safety-control-api.test.mjs`).
- Assertion pattern: verify machine-readable error contracts and domain reason codes, not only status booleans (examples: `tests/bootstrap/bootstrap-negative.test.mjs`, `services/control-api/src/routes/mod.rs`).

## Mocking

**Framework:** No dedicated mocking framework detected; test doubles are handwritten.

**Patterns:**
```rust
#[derive(Debug, Default)]
struct StubValidationWorkflowOrchestrator { start_error: Option<(&'static str, &'static str)> }
```
from `services/control-api/src/routes/mod.rs`.

```javascript
const fetchImpl = async () => ({ ok: false, status: 403, json: async () => ({ error_code: "authorization_denied" }) });
await assert.rejects(invokeEmergencyControlAction({ baseUrl, action: "pause", fetchImpl }));
```
from `tests/api/story-3-2-safety-control-api.test.mjs`.

**What to Mock:**
- External boundaries: HTTP clients (`fetchImpl`) in Node tests (`tests/api/*.test.mjs`).
- Service orchestrator dependencies through trait implementations in Rust route tests (`services/control-api/src/routes/mod.rs`).

**What NOT to Mock:**
- Pure domain validation logic; test directly with deterministic inputs in `crates/domain/src/*.rs`.
- Canonical SQL/constants checks are validated directly in persistence unit tests (example: migration/query string assertions in `crates/persistence/src/postgres/risk_limits.rs`).

## Fixtures and Factories

**Test Data:**
```rust
fn sample_profile() -> RiskLimitProfileVersion { /* canonical valid fixture */ }
```
in `crates/persistence/src/postgres/risk_limits.rs`.

```javascript
const FIXTURE_ROOT = resolve("tests/fixtures/valid");
cpSync(FIXTURE_ROOT, tempRoot, { recursive: true });
```
in `tests/bootstrap/bootstrap-negative.test.mjs`.

**Location:**
- Node fixture roots: `tests/fixtures/valid/**`.
- Rust fixture-like builders are in-module helper functions under `#[cfg(test)]` blocks.

## Coverage

**Requirements:** None enforced in committed scripts/workflows.

**View Coverage:**
```bash
Not configured in repository scripts; verify external tooling if introduced.
```

## Test Types

**Unit Tests:**
- Domain and service logic tests are co-located in Rust source modules (`crates/domain/src/*.rs`, `services/governance-service/src/*/mod.rs`).
- Bootstrap utility units run via Node test runner (`tests/bootstrap/*.test.mjs`).

**Integration Tests:**
- HTTP route integration-style tests use in-process Axum router + stub orchestrators (`services/control-api/src/routes/mod.rs`).
- `tests/integration/` currently contains `.gitkeep`; no standalone integration test files detected.

**E2E Tests:**
- E2E contract tests exist as Node suites in `tests/e2e/*.e2e.test.mjs`.
- No Playwright/Cypress config detected (`playwright.config.*` and `cypress.config.*` not found).

## Common Patterns

**Async Testing:**
```rust
#[tokio::test]
async fn ... { /* await request */ }
```
in `services/control-api/src/routes/mod.rs`.

```javascript
test("...", async () => { await assert.rejects(asyncCall()); });
```
in `tests/api/story-3-2-safety-control-api.test.mjs`.

**Error Testing:**
```rust
let error = service.upsert_risk_limit_profile(input).expect_err("...must fail");
assert_eq!(error.code, RiskLimitReasonCode::InvalidPayload.code());
```
from `services/governance-service/src/risk_limits/mod.rs`.

```javascript
assert.throws(() => validateEnvironmentTemplate(envEntries), (error) => error.payload.error_code === "BOOTSTRAP_PLAINTEXT_SECRET");
```
from `tests/bootstrap/check-bootstrap.test.mjs`.

## CI/Test Workflows

- Rust CI workflow executes preflight + bootstrap checks + `rust:fmt`, `rust:lint`, `rust:test`, `rust:build` in `.github/workflows/ci-rust.yml`.
- Web CI workflow executes preflight + bootstrap checks + `web:lint`, `web:typecheck`, `web:build` in `.github/workflows/ci-web.yml`.
- Security workflow runs bootstrap checks + `tools/bootstrap/security-scan.mjs` in `.github/workflows/security.yml`.
- Story-specific QA aggregation is script-based in root `package.json` via `qa:test:story-*` commands combining Rust + Node suites.

---

*Testing analysis: 2026-04-08*
