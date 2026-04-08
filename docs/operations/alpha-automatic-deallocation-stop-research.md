# Alpha Automatic Deallocation and Stop-Research Operations Guide (Story 6.9 / FR47 / FR48 / NFR9 / NFR17)

This runbook covers Story 6.9 lifecycle automation that applies `deallocate` and `stop_research` actions from canonical health and promotion evidence.

## Scope and boundaries

- Dependency surfaces:
  - Story 6.7 alpha-health threshold breaches (`alpha_threshold_breaches`).
  - Story 6.5 promotion decision history (`promotion_decisions`).
  - Story 6.8 readiness read-model lifecycle consumers.
- Schema scope in Story 6.9 is limited to `alpha_lifecycle_actions`.
- Out of scope: redesigning Story 6.8 UI composition or creating parallel promotion/health/replay engines.

## Control-plane routes

1. `POST /control/research/alpha-lifecycle-actions`
2. `GET /control/research/alpha-lifecycle-actions/{action_id}`
3. `GET /control/research/alpha-lifecycle-actions?alpha_id={alpha_id}&limit={n}`

All responses use canonical `data/meta/error` envelopes.

List/read query boundary semantics are deterministic:

1. `acted_after_utc` is inclusive (`>=`).
2. `acted_before_utc` is exclusive (`<`).
3. `acted_before_utc` must be greater than `acted_after_utc`; invalid ordering returns `400` with field-level diagnostics.

## FR47 deallocation trigger contract

- Source of truth: canonical Story 6.7 breach records (`alpha_threshold_breaches`).
- Evaluator behavior:
  - Uses configured deallocation policies (`metric_key`, `comparator`, `threshold_value`).
  - Applies deallocation when canonical breach evidence triggers policy.
  - Persists machine-readable `criterion_keys` and breach evidence in `trigger_evidence`.
- Reflection continuity:
  - Applied deallocation records include `reflected_lifecycle_state = deallocated`.
  - Story 6.8 lifecycle derivation consumes lifecycle actions through the canonical control-plane read surface.

## FR48 stop-research criteria contract

Stop-research triggers when **any** criterion is true:

1. `trade_count_30d < 200`
2. `out_of_sample_sharpe_30d < 0.2`
3. `promotion_failure_rate_last_10 > 0.70`

Boundary behavior is deterministic and allow-path:

1. `trade_count_30d == 200`
2. `out_of_sample_sharpe_30d == 0.2`
3. `promotion_failure_rate_last_10 == 0.70`

When `promotion_failure_rate_last_10` is omitted, it is derived from recent Story 6.5 promotion outcomes.

## Fail-closed status and reason mapping

| Status | Error code families |
| --- | --- |
| `400` | `alpha_lifecycle_action_invalid_payload` |
| `403` | `alpha_lifecycle_action_unauthorized_role` |
| `409` | `alpha_lifecycle_action_not_found`, `alpha_lifecycle_action_denied`, deallocation/stop criteria conflicts, constraint violations |
| `503` | `alpha_lifecycle_action_dependency_unavailable`, `alpha_lifecycle_action_state_unavailable`, `alpha_lifecycle_action_persistence_unavailable`, query/decode unavailable paths |
| `500` | uncategorized internal failures |

No success-shaped fallback is allowed on dependency/state/persistence ambiguity.

## Audit and security-event continuity

Privileged audit records include:

1. actor id / role
2. endpoint + method + action id + alpha id
3. action type + action status + criterion count
4. reason code
5. correlation id
6. UTC timestamp
7. approval reference (when present)

Unauthorized security-signal names:

1. `unauthorized_alpha_lifecycle_action_read_attempt_v1`
2. `unauthorized_alpha_lifecycle_action_mutation_attempt_v1`

## Operator deny/failure playbooks

1. **Invalid payload (400)**  
   Resolve field-level diagnostics and resubmit canonical identifiers/timestamps.
2. **Unauthorized role (403)**  
   Re-run under an authorized role with a fresh correlation id.
3. **Conflict/not-found (409)**  
   Verify action identity, criteria, and approval context before retry.
4. **Dependency/state/persistence unavailable (503)**  
   Restore upstream health; preserve fail-closed posture; re-run evaluation.

## Cross-runbook links

1. Story 6.5 promotion lifecycle governance: `docs/operations/alpha-promotion-lifecycle-governance.md`
2. Story 6.6 counterfactual replay stress gating: `docs/operations/alpha-counterfactual-replay-stress-gating.md`
3. Story 6.7 live alpha health monitoring: `docs/operations/alpha-live-health-monitoring-threshold-breaches.md`
4. Story 6.8 governance readiness card: `docs/operations/alpha-governance-readiness-card.md`
5. Severity alert delivery: `docs/operations/severity-alert-delivery.md`
