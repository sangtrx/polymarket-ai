# Codebase Concerns

**Analysis Date:** 2026-04-08

## Tech Debt

**Control API route + middleware monolith (Priority: High, Likelihood: High):**
- Issue: Core HTTP behavior is concentrated in very large modules with repeated lint suppressions (`#[allow(clippy::too_many_arguments)]`), which increases merge conflicts and regression risk.
- Files: `services/control-api/src/routes/mod.rs`, `services/control-api/src/middleware/mod.rs`
- Impact: Small changes to auth, routing, or response envelopes can produce wide unintended side effects and slow review velocity.
- Fix approach: Split by bounded context (risk, recovery, reporting, research, governance), move shared response/auth helpers into focused modules, and remove `allow` attributes by reducing function argument count.

**Scaffold-only service entrypoints in production paths (Priority: High, Likelihood: High):**
- Issue: Multiple service `main` binaries are scaffold stubs that only print readiness and return.
- Files: `services/governance-service/src/main.rs`, `services/research-gateway/src/main.rs`, `services/portfolio-engine/src/main.rs`
- Impact: These services cannot process runtime traffic/work despite being first-class workspace members.
- Fix approach: Implement real runtime loops (listener/worker/event-driven) or gate scaffold binaries behind explicit `--example`/feature flags.

## Known Bugs

**HTTP routers are built but not served (Priority: Critical, Likelihood: High):**
- Symptoms: API binaries initialize state and router objects but never bind a socket or call `axum::serve`.
- Files: `services/control-api/src/main.rs`, `services/reporting-service/src/main.rs`
- Trigger: Start the binaries in any environment expecting HTTP availability.
- Workaround: Not applicable in-code; requires implementing listener bootstrap (`TcpListener::bind` + `axum::serve`) with graceful shutdown handling.

**Audit metadata reports wrong endpoint for auth denials (Priority: High, Likelihood: High):**
- Symptoms: Denied auth events always log endpoint `/control/rebalance` regardless of actual route.
- Files: `services/control-api/src/middleware/mod.rs` (denial audit payload in `require_authenticated_actor`)
- Trigger: Any failed authentication on non-rebalance routes.
- Workaround: None currently; set endpoint/method from request context in middleware.

## Security Considerations

**Bearer token format is self-asserted and unsigned (Priority: Critical, Likelihood: High):**
- Risk: `Authorization: Bearer <actor_id>:<role>:<expires_unix>` is parsed locally without cryptographic verification or trusted issuer validation.
- Files: `services/control-api/src/middleware/mod.rs`
- Current mitigation: Basic format checks (`is_valid_identifier`), role parsing, and expiry timestamp checks.
- Recommendations: Replace with signed JWT/OIDC or mTLS-backed identity assertions; verify issuer, audience, signature, and replay protections server-side.

**Privileged audit persistence is in-memory in control-api bootstrap (Priority: Critical, Likelihood: High):**
- Risk: Audit records are lost on process restart and do not provide durable forensic/compliance history.
- Files: `services/control-api/src/main.rs`, `services/governance-service/src/audit/mod.rs`
- Current mitigation: Redaction + validation exists, but persistence adapter is `InMemoryAuditAppendPort`.
- Recommendations: Use a durable append port (Postgres/WORM log sink), add startup health check that fails if audit sink is unavailable.

## Performance Bottlenecks

**Hard-coded small DB pool limits across services (Priority: Medium, Likelihood: Medium):**
- Problem: Connection ceilings are fixed at compile-time (`4`/`5`/`8`) without environment tuning.
- Files: `services/control-api/src/main.rs`, `services/execution-engine/src/main.rs`, `services/risk-engine/src/main.rs`, `services/reporting-service/src/main.rs`, `services/reporting-service/src/read_models/queries.rs`
- Cause: `PgPoolOptions::max_connections(...)` literals are used directly.
- Improvement path: Read pool sizes from env with validated defaults and per-service capacity planning.

## Fragile Areas

**Fail-closed by panic for runtime termination paths (Priority: High, Likelihood: Medium):**
- Files: `services/execution-engine/src/main.rs`
- Why fragile: `tokio::select!` branches call `panic!` on runtime completion/error; transient upstream failures can cause crash loops and thundering-herd restarts.
- Safe modification: Replace panic with supervised restart/backoff + health-state degradation signal.
- Test coverage: Unit tests exist broadly, but resilience behavior for live process supervision should be verified via integration tests under `tests/e2e/`.

**Risk-engine bootstrap is one-shot, no long-lived loop (Priority: High, Likelihood: High):**
- Files: `services/risk-engine/src/main.rs`
- Why fragile: Runtime state is hydrated once and process exits after bootstrap evaluation, leaving no active adjudication process in this binary.
- Safe modification: Introduce a persistent processing loop/subscription and shutdown signaling; preserve fail-closed semantics as policy flags, not process termination.
- Test coverage: Verify expected runtime behavior with executable-level tests; current verification location is unknown and should be confirmed in `tests/e2e/` and `Cargo.toml` test targets.

## Scaling Limits

**Runtime capacity boundaries are not explicit in code/config contracts (Priority: Medium, Likelihood: Medium):**
- Current capacity: Unknown; no documented throughput/SLO constraints were detected in runtime source modules.
- Limit: Practical limits likely bound by fixed pool sizes and single-process loops.
- Scaling path: Add explicit env-configured concurrency controls, queue/backpressure instrumentation, and capacity test scenarios in `tests/e2e/`.

## Dependencies at Risk

**Not detected from source scan with current evidence:**
- Risk: No immediate deprecated/abandoned dependency markers were identified in checked runtime files.
- Impact: Unknown.
- Migration plan: Verify with `cargo audit` and lockfile policy checks referenced by `package.json` script `ci:security`.

## Missing Critical Features

**Production-grade service runtime bootstrap is incomplete (Priority: Critical, Likelihood: High):**
- Problem: Several service binaries are scaffold/no-listener/no-worker implementations.
- Blocks: End-to-end deployment of control, governance, research, portfolio, risk, and reporting operational paths.

## Test Coverage Gaps

**Executable startup/runtime behavior is under-specified (Priority: High, Likelihood: Medium):**
- What's not tested: Guaranteed listener binding, durable audit sink availability, supervised restart behavior, and signed identity verification paths.
- Files: `services/control-api/src/main.rs`, `services/reporting-service/src/main.rs`, `services/governance-service/src/main.rs`, `services/research-gateway/src/main.rs`, `services/portfolio-engine/src/main.rs`, `services/risk-engine/src/main.rs`
- Risk: CI can pass while deployed binaries remain non-serving or security posture remains placeholder-level.
- Priority: High

---

*Concerns audit: 2026-04-08*
