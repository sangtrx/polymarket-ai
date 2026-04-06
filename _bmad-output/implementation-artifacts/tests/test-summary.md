# Test Automation Summary

## Story

- 2-7-configure-portfolio-market-and-strategy-limit-policies

## Generated Tests

### API Tests

- [x] `services/control-api/src/routes/mod.rs` — `risk_limit_profile_route_returns_machine_readable_active_evidence` validates accepted profile mutation evidence with machine-readable fields.
- [x] `services/control-api/src/routes/mod.rs` — `risk_limit_profile_route_returns_pending_without_synthetic_approval_reference` validates pending critical-increase behavior with null approval reference.
- [x] `services/control-api/src/routes/mod.rs` — `risk_limit_pending_route_returns_queryable_pending_evidence` validates pending-policy query contract (actor/action/status/reason metadata).
- [x] `services/control-api/src/routes/mod.rs` — unauthorized and service-unavailable risk-limit route tests validate fail-closed error envelopes.

### Integration and Runtime Tests

- [x] `crates/domain/src/risk.rs` — risk-limit profile validation tests cover boundary equality acceptance, strict child-scope `>` rejection, and deterministic reason-code parsing.
- [x] `crates/persistence/src/postgres/risk_limits.rs` — migration scope/constraint/index tests plus bundle validation tests for profile/rule consistency.
- [x] `services/governance-service/src/risk_limits/mod.rs` — orchestration tests cover unauthorized role denial, pending critical increase, approved activation, pending query evidence, and invariant rejection.
- [x] `services/risk-engine/src/limits/mod.rs` — runtime limit-state tests cover available path, missing/stale/unavailable fail-closed behavior, and pending-count snapshot surface.
- [x] `services/risk-engine/src/gates/mod.rs` — gate integration tests validate fail-closed deny when limit state is unavailable and allow when state is available.

## Coverage

- Story 2.7 critical flows covered across domain, persistence, governance service, control API, and risk-engine runtime:
  - scoped profile/inventory contract validation with deterministic boundary semantics,
  - strict schema scope (`risk_limit_profiles`, `inventory_limit_rules`) and index contracts,
  - transactional profile/rule persistence adapter validation,
  - approval-gated critical-increase pending behavior and machine-readable mutation evidence,
  - pending-policy query visibility with actor/action/reason traceability,
  - risk runtime limit-state fail-closed availability surfaces for downstream pre-trade gating.

## Execution Result

- `npm run --silent qa:test:story-2-7` ✅
- `npm run --silent rust:lint` ✅
- `npm run --silent test` ✅
- `npm run --silent rust:build` ✅
- `npm run --silent qa:test:story-2-7` (BMAD QA automation rerun) ✅
