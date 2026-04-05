# Test Automation Summary

## Story

- 2-1-configure-market-universe-policy-engine

## Generated Tests

### API Tests

- [x] `services/control-api/src/routes/mod.rs` — `market_policy_profile_route_returns_machine_readable_success_evidence` validates profile update happy-path acceptance and machine-readable evidence (`accepted`, `market_policy_profile_updated`).
- [x] `services/control-api/src/routes/mod.rs` — `market_policy_profile_route_surfaces_service_unavailable_machine_error` validates explicit `503` propagation for persistence-unavailable profile update failures.
- [x] `services/control-api/src/routes/mod.rs` — `market_policy_profile_route_surfaces_unknown_service_error_as_internal_server_error` validates explicit `500` fallback mapping for unclassified orchestration failures.
- [x] `services/control-api/src/routes/mod.rs` — `market_policy_cluster_toggle_route_surfaces_conflict_machine_error` validates explicit `409` conflict behavior for cluster-toggle constraint violations.
- [x] Existing route tests for unauthorized/invalid payload and runtime toggle transitions remain active for `/control/market-policy/profiles/{cluster_id}` and `/control/market-policy/clusters/{cluster_id}/toggle`.

### E2E Tests

- [x] Existing deterministic gate-level E2E in `services/risk-engine/src/gates/mod.rs` validates runtime cluster toggle transitions without process restart (`order_intent_gate_blocks_new_intents_after_runtime_toggle_without_restart`).
- [x] UI E2E not applicable (story scope is backend domain/persistence/governance/risk/control-api policy workflows).

## Coverage

- Market-policy control API endpoints: 2/2 covered with accepted-path and fail-closed machine-error scenarios.
- Market-policy profile route scenarios: 5/5 covered (accepted, invalid payload `400`, unauthorized `403`, persistence unavailable `503`, unclassified failure `500`).
- Cluster-toggle route scenarios: 3/3 covered (disable/enable accepted transitions + conflict `409`).
- Runtime intent-gate behavior: 3/3 covered (enabled allow, runtime disable block, missing-state fail-closed).

## Execution Result

- `npm run qa:test:story-2-1` ✅
- `npm run ci:rust` ✅
