# Pre-Trade Gate Pipeline Operations

## Purpose

This runbook describes operational behavior for Story 2.8 pre-trade adjudication:

1. deterministic gate-order evaluation,
2. single machine-readable allow/deny reason per decision,
3. fail-closed handling for missing/unavailable inputs,
4. drawdown protective-mode signaling and incident triage.

## Gate Evaluation Order

The risk engine evaluates gates in this strict order and stops on first failure:

1. `freshness`
2. `stream_health`
3. `exposure_limit_state`
4. `reconciliation_halt`
5. `user_stream_auth`
6. `drawdown_stop`
7. `strategy_approval`
8. `venue_eligibility`
9. `reward_per_risk`
10. `participation_guardrail`

This ordering is deterministic and is enforced in `services/risk-engine/src/gates/mod.rs`.

## Reason-Code Matrix

| Gate | Allow | Deny (state unavailable / invalid) | Deny (policy/breach) |
| --- | --- | --- | --- |
| Freshness | `pretrade_gate_pass` | `pretrade_freshness_state_unavailable` | `pretrade_freshness_stale_breach` |
| Stream health | `pretrade_gate_pass` | `pretrade_stream_health_state_unavailable` | `pretrade_stream_health_degraded` |
| Exposure/limit | `pretrade_gate_pass` | `pretrade_risk_limit_state_unavailable` | `pretrade_risk_limit_state_unavailable` |
| Reconciliation halt | `pretrade_gate_pass` | `pretrade_reconciliation_critical_halt` | `pretrade_reconciliation_critical_halt` |
| User stream auth | `pretrade_gate_pass` | `pretrade_user_stream_auth_expired` | `pretrade_user_stream_auth_expired` |
| Drawdown stop | `pretrade_gate_pass` | `pretrade_drawdown_state_unavailable` | `pretrade_drawdown_stop_triggered` |
| Strategy approval | `pretrade_gate_pass` | `pretrade_strategy_approval_unavailable` | `pretrade_strategy_approval_required` |
| Venue eligibility | `pretrade_gate_pass` | `pretrade_venue_eligibility_unavailable` | `pretrade_venue_ineligible` |
| Reward-per-risk | `pretrade_gate_pass` | `pretrade_reward_risk_state_unavailable` | `pretrade_reward_risk_below_threshold` |
| Participation guardrail (FR41) | `pretrade_gate_pass` | `pretrade_participation_guardrail_unavailable` | `pretrade_participation_guardrail_low_liquidity_pause` / `pretrade_participation_guardrail_inactivity_pause` |

## Fail-Closed Expectations

- Missing or malformed gate state is never treated as healthy.
- Execution submit path requires pre-trade adjudication before persisting submit transitions.
- Adjudication timeout/unreachable outcomes fail closed with:
  - `pretrade_adjudication_timeout` or
  - `pretrade_adjudication_unavailable`.
- Denied or unavailable adjudication must produce **no submit side effects** in order-lifecycle persistence.

## Drawdown Protective-Mode Verification

Use this checklist when drawdown stop is expected:

1. Confirm drawdown input satisfies boundary (`current_drawdown_pct >= configured_stop_threshold_pct`).
2. Confirm risk gate decision is deny with `pretrade_drawdown_stop_triggered`.
3. Confirm safe-state signal event `risk_safe_state_drawdown_protective_mode_v1` is emitted.
4. Confirm persisted pre-trade decision has `protective_mode_active = true`.
5. Confirm execution submit attempts fail closed and do not write submit transitions.

## Evidence Retrieval

Durable evidence is stored in `pretrade_gate_decisions`.

### By intent ID

```sql
SELECT *
FROM pretrade_gate_decisions
WHERE intent_id = 'intent_123'
ORDER BY evaluated_at_utc DESC, decision_id DESC
LIMIT 1;
```

### By correlation ID

```sql
SELECT *
FROM pretrade_gate_decisions
WHERE correlation_id = 'corr_123'
ORDER BY evaluated_at_utc DESC, decision_id DESC
LIMIT 1;
```

### By market and time

```sql
SELECT decision_id, intent_id, outcome, reason_code, protective_mode_active, evaluated_at_utc
FROM pretrade_gate_decisions
WHERE market_id = 'market_yes_no_1'
ORDER BY evaluated_at_utc DESC, decision_id DESC
LIMIT 50;
```

## Recovery and Story 2.9 Handoff

Story 2.8 covers pre-trade deny/protective signaling only. Emergency control orchestration and resume workflows are handled by Story 2.9+.

During incident recovery:

1. Use this runbook to validate gate health inputs and decision evidence.
2. Keep submit flow blocked until gate reason codes return to healthy pass conditions.
3. Execute Story 2.9 emergency-control procedures for control-plane actions (kill-switch/escalation/resume governance).

## Cross-runbook links

1. FR41 participation guardrails: `docs/operations/low-liquidity-overnight-guardrails.md`
2. FR39 reward-per-risk policy operations: `docs/operations/reward-risk-policy-operations.md`
