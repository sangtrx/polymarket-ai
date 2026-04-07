# Reward-Per-Risk Policy Operations Guide (Story 5.1)

## Scope

This runbook governs control-plane mutation and verification for FR39 reward-per-risk policy thresholds used by pre-trade gating.

Authenticated endpoints:

1. `POST /control/reward-risk/policies/{policy_key}`
2. `GET /control/reward-risk/policies/{policy_key}`

## FR39 inputs and formula semantics

Reward-per-risk scoring uses these market snapshot fields (all values in basis points):

1. `expected_reward_bps`
2. `maker_rebate_bps`
3. `expected_cost_bps`
4. `expected_volatility_bps`

Formula (deterministic):

`(expected_reward_bps + maker_rebate_bps - expected_cost_bps) / expected_volatility_bps`

Operational guardrails:

1. Inputs must be finite.
2. `expected_volatility_bps` must be strictly greater than the near-zero floor (`0.000001`).
3. Missing or invalid score inputs are fail-closed and return `pretrade_reward_risk_state_unavailable`.

## Threshold defaults and override behavior

1. Policy thresholds are keyed by canonical `policy_key`/`strategy_key` identifiers.
2. If no policy override is found, default threshold `1.2` is applied.
3. Boundary behavior is deterministic:
   - `score == threshold` passes
   - `score < threshold` denies with `pretrade_reward_risk_below_threshold`
4. Policy reads that use fallback return `reason_code = reward_risk_default_threshold_applied`.

## Rollout and rollback workflow

1. **Rollout**
   1. Submit `POST /control/reward-risk/policies/{policy_key}` with `strategy_key` and `min_reward_per_risk`.
   2. Confirm accepted evidence includes `policy_key`, `strategy_key`, `min_reward_per_risk`, `reason_code`, `actor_id`, `correlation_id`, and `timestamp_utc`.
   3. Monitor pre-trade decisions for expected transition from `pretrade_reward_risk_state_unavailable`/`pretrade_reward_risk_below_threshold` to `pretrade_gate_pass` where appropriate.
2. **Rollback**
   1. Re-submit the same policy key with prior known-good threshold.
   2. Confirm evidence reason `reward_risk_policy_updated` and matching correlation/audit trail.

## Failure-mode playbooks

1. **Invalid mutation payload** (`reward_risk_invalid_payload`, HTTP 400)  
   Remediation: correct `field_errors[]` and resubmit (most common: invalid threshold or malformed identifiers/timestamps).
2. **Unauthorized mutation role** (`reward_risk_unauthorized_role`, HTTP 403)  
   Remediation: use an authorized privileged role and re-run with a new correlation ID.
3. **Persistence dependency unavailable** (`reward_risk_persistence_unavailable`, HTTP 503)  
   Remediation: preserve fail-closed posture, escalate platform on-call, and retry after persistence recovers.
4. **Runtime scoring/policy unavailable** (`pretrade_reward_risk_state_unavailable`)  
   Remediation: verify market snapshot FR39 fields are populated, verify policy read endpoint, and confirm risk-engine bootstrap hydration from `reward_risk_policies`.

## Cross-runbook links

1. Incentive regime-shift alerts (FR40): `docs/operations/incentive-regime-shift-alerts.md`
2. Severity alert delivery and fallback (Story 3.6): `docs/operations/severity-alert-delivery.md`
3. Core/satellite market stratification operations (FR42): `docs/operations/core-satellite-market-stratification.md`
4. Alpha hypothesis registry operations (FR6): `docs/operations/alpha-hypothesis-registry.md`
