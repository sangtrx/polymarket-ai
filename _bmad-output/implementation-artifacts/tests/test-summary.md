# Test Automation Summary

## Story

- 2-6-build-reconciliation-and-exposure-visibility-core

## Generated Tests

### API Tests

- [x] Not applicable for this story scope (Story 2.6 introduces no new HTTP/API route surface).

### E2E Tests

- [x] `services/execution-engine/src/reconciliation/mod.rs` — `execution_run_is_deterministic_for_identical_inputs` validates deterministic reconciliation diff classification for identical windows.
- [x] `services/execution-engine/src/reconciliation/mod.rs` — `critical_mismatch_run_enforces_safe_state_and_keeps_snapshot_readable` validates `mismatch_rate > 0.1%` critical halt transition plus halted-state exposure visibility.
- [x] `services/execution-engine/src/reconciliation/mod.rs` — `unauthorized_read_queries_fail_closed_with_machine_reason_code` validates fail-closed authz behavior on reconciliation/exposure read paths.
- [x] `services/execution-engine/src/reconciliation/mod.rs` — `incident_query_path_satisfies_operability_target` validates representative incident read path stays within `<= 5s`.
- [x] `services/risk-engine/src/gates/mod.rs` — reconciliation halt gate tests validate deny behavior, recovery behavior, and non-halt reason normalization.

## Coverage

- Story 2.6 critical flows covered across domain, persistence, execution-engine, and risk-engine suites:
  - deterministic diff taxonomy + boundary threshold semantics,
  - migration scope/constraints/index contracts,
  - reconciliation evidence persistence/read-path contracts,
  - reconciliation halt deny-path gating with exposure read continuity,
  - authz fail-closed read-path behavior and incident-query operability target checks.

## Execution Result

- `npm run --silent qa:test:story-2-6` ✅
- `npm run --silent rust:lint` ✅
- `npm run --silent rust:build` ✅
