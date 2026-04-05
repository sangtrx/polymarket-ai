# Test Automation Summary

## Story

- 1-5-enforce-dual-approval-governance-for-critical-mutations

## Generated Tests

### API Tests

- [x] `services/control-api/src/routes/mod.rs` — `critical_action_submit_denies_expired_window_with_machine_payload` validates expired-window submission denial with machine-readable code/message and alert-compatible security signal fields.
- [x] `services/control-api/src/routes/mod.rs` — `critical_action_vote_unknown_request_returns_bad_request_machine_payload` validates unknown-request voting returns deterministic `400` denial with explicit reason and signal envelope.
- [x] `services/control-api/src/routes/mod.rs` — `critical_action_execute_without_request_returns_bad_request_machine_payload` validates execution without request is fail-closed with explicit machine-readable denial.

### E2E Tests

- [x] `services/control-api/src/routes/mod.rs` — existing critical-action flow tests validate pending submission, self-approval denial, second-approver gating, rate-limit boundary, and approved execution audit linkage (`approval_reference` propagation).
- [x] `services/governance-service/src/approvals/mod.rs` — existing orchestration tests validate FR34 boundary conditions (5-per-hour limit, distinct proposer/approver checks, duplicate vote denial, expiry boundary, and approval reference generation).

## Coverage

- Critical-action API contract: 3/3 workflow endpoints (`submit`, `vote`, `execute`) validated for both accepted and fail-closed machine-denial paths.
- FR34/NFR8 denial behavior: explicit reason-code + security-signal evidence validated across expired-window, unknown-request, missing-request, self-approval, and missing-second-approver classes.
- UI features: N/A (story scope is governance/control-plane API workflow).

## Execution Result

- `npm run qa:test:story-1-5` ✅
- `npm run ci:rust` ✅
