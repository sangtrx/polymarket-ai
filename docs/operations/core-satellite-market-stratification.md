# Core/Satellite Market Stratification Operations Guide (Story 5.4)

## Scope

This runbook covers FR42 control-plane mutation/read workflows and pre-trade runtime behavior for market-bucket stratification.

Authenticated endpoints:

1. `POST /control/market-policy/buckets/{market_id}/{cluster_id}`
2. `GET /control/market-policy/buckets/{market_id}/{cluster_id}`

## Canonical bucket and policy-link contract

`POST /control/market-policy/buckets/{market_id}/{cluster_id}` accepts:

1. `bucket_type` (`core` or `satellite` only)
2. `risk_policy_key` (canonical lower-case identifier)
3. `allocation_policy_key` (canonical lower-case identifier)

Normalization and boundary invariants are deterministic:

1. identifiers are `trim + lowercase` across control, governance, persistence, and runtime layers.
2. `bucket_type` must be one of `core`, `satellite` (bucket_type must be one of `core`, `satellite`).
3. only one active mapping is allowed per `(market_id, cluster_id)`.

## Evidence and audit contract (NFR17)

Successful read/update responses include:

1. `profile_id`
2. `market_id`
3. `cluster_id`
4. `bucket_type`
5. `risk_policy_key`
6. `allocation_policy_key`
7. `reason_code`
8. `actor_id`
9. `correlation_id`
10. `timestamp_utc`

Control-plane paths append privileged audit records with:

1. `action_type` (`market_bucket_profile_update` / `market_bucket_profile_read`)
2. actor/role/authentication metadata
3. request parameters (endpoint, bucket, linked policy keys, active state)
4. `reason_code` and correlation/timestamp traceability

## Runtime application and fail-closed behavior

Pre-trade evaluation resolves policy context from active bucket mappings before exposure-limit, reward-risk, and FR41 participation checks.

When mapping or linked policy context is unavailable/invalid:

1. deny reason is `pretrade_stratification_state_unavailable`
2. safe-state escalation uses control-uncertainty trigger semantics
3. no success-shaped fallback to unrelated non-bucket policy keys is emitted

Resolution telemetry emits `risk_market_bucket_resolution_v1` with resolved `risk_policy_key` and `allocation_policy_key`.

## Operator verification queries

Use deterministic ordering for audit and triage checks:

```sql
SELECT profile_id, market_id, cluster_id, bucket_type, risk_policy_key, allocation_policy_key, actor_id, correlation_id, updated_at_utc
FROM market_bucket_profiles
WHERE lower(trim(market_id)) = lower(trim('market_yes_no_1'))
  AND lower(trim(cluster_id)) = lower(trim('cluster_alpha'))
  AND is_active = TRUE
ORDER BY updated_at_utc DESC, profile_id ASC
LIMIT 20;
```

## Failure-mode playbook

1. **Invalid payload / unsupported bucket** (`market_bucket_invalid_payload`, `market_bucket_unsupported_bucket_type`, HTTP 400)  
   remediation: correct `field_errors[]` and re-submit canonical identifiers and supported bucket value.
2. **Unauthorized control role** (`market_policy_unauthorized_role`, HTTP 403)  
   remediation: use an authorized privileged role (`operational_control` or `administrative_actions`) and retry with new correlation ID.
3. **Mapping conflict** (`market_bucket_mapping_conflict`, `market_bucket_constraint_violation`, HTTP 409)  
   remediation: inspect active mapping uniqueness for the same market+cluster pair, then re-apply intended active mapping.
4. **Dependency unavailable / read failure** (`market_bucket_mapping_unavailable`, `market_bucket_persistence_unavailable`, `market_bucket_query_failed`, `market_bucket_row_decode_failed`, HTTP 503)  
   remediation: preserve fail-closed posture, recover persistence dependency health, and retry read/update flow.
5. **Runtime stratification unavailable** (`pretrade_stratification_state_unavailable`)  
   remediation: restore active mapping and linked policy state, then re-validate pre-trade decisions before resuming normal execution volume.

## Cross-runbook links

1. Market policy engine operations: `docs/operations/market-policy-engine.md`
2. Risk-limit policy operations: `docs/operations/risk-limit-policy-operations.md`
3. Allocation/rebalance workflows: `docs/operations/allocation-rebalance-workflows.md`
4. Reward-per-risk policy operations: `docs/operations/reward-risk-policy-operations.md`
5. Incentive regime-shift alerts: `docs/operations/incentive-regime-shift-alerts.md`
6. Pre-trade gate pipeline operations: `docs/operations/pretrade-gate-pipeline.md`
