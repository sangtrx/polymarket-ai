# Market Universe Policy Engine Operations Guide (Story 2.1)

## Scope

This guide defines operator-safe controls for market-universe thresholds and runtime cluster enable/disable behavior.

Authenticated control-plane endpoints:

1. `POST /control/market-policy/profiles/{cluster_id}`
2. `POST /control/market-policy/clusters/{cluster_id}/toggle`

## Policy profile field contract

`POST /control/market-policy/profiles/{cluster_id}` accepts:

1. `min_liquidity_usd` (USD notional depth, `>= 0`)
2. `max_spread_bps` (basis points, `>= 0`; `0` allowed)
3. `min_reward_score` (unitless score, `>= 0`)
4. `max_exposure_pct_nav` (percent of NAV, inclusive range `[0, 100]`; `100` allowed, `>100` rejected)

Profile updates are fail-closed and never partially persisted. Invalid payloads return machine-readable `error_code` and `field_errors[]` entries.

## Runtime cluster toggle runbook

Use `POST /control/market-policy/clusters/{cluster_id}/toggle` with:

1. Disable request:
   - `is_enabled: false`
   - `reason_code: "market_policy_cluster_disabled_by_operator"`
2. Enable request:
   - `is_enabled: true`
   - `reason_code: "market_policy_cluster_enabled"`

Expected response evidence:

1. `status`
2. `cluster_id`
3. `is_enabled`
4. `reason_code`
5. `actor_id`
6. `correlation_id`
7. `timestamp_utc`

Runtime behavior is immediate for new intent gating logic: newly submitted order intents are denied when a cluster is disabled and allowed again once re-enabled.

## Failure modes and remediation

1. **Invalid threshold payload** (`market_policy_invalid_payload`, HTTP 400)  
   Remediation: Correct the listed `field_errors` and resubmit. Typical issues: negative liquidity/spread/reward, or `max_exposure_pct_nav > 100`.
2. **Disabled-cluster intent denial** (`market_policy_cluster_disabled`)  
   Remediation: Confirm the disable was intentional. If market activity should resume, submit enable toggle with reason `market_policy_cluster_enabled`.
3. **Missing/ambiguous runtime policy state** (`market_policy_state_unavailable`)  
   Remediation: Reapply cluster toggle and policy profile for the target cluster, then verify subsequent intent-gate decisions.
4. **Persistence/runtime dependency outage** (`market_policy_persistence_unavailable`, HTTP 503)  
   Remediation: Keep fail-closed posture, escalate platform on-call, and retry after persistence health is restored.

## Cross-runbook links

1. Core/satellite market stratification operations (FR42): `docs/operations/core-satellite-market-stratification.md`
