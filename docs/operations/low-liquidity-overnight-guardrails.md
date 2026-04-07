# Low-Liquidity and Overnight Participation Guardrails (Story 5.3)

## Scope

This runbook covers FR41 participation guardrails that constrain new quote/order participation during fragile market conditions.

Authenticated evidence endpoint:

1. `GET /control/incidents/participation-guardrails`

## FR41 trigger matrix and strict-boundary semantics

FR41 evaluates these conditions in deterministic order:

1. **Low-liquidity pause**: `liquidity_depth_usd < 10_000` -> `pretrade_participation_guardrail_low_liquidity_pause`
2. **Short-gap inactivity pause**: `inactivity_gap_seconds > 900` and `<= 14_400` -> `pretrade_participation_guardrail_inactivity_pause`
3. **Overnight size-cap mode**: `inactivity_gap_seconds > 14_400` and no active pause trigger -> `size_cap`

Exact boundaries are non-triggering for stricter tier transitions:

1. `liquidity_depth_usd == 10_000` is **not** low-liquidity breach.
2. `inactivity_gap_seconds == 900` is **not** inactivity pause.
3. `inactivity_gap_seconds == 14_400` remains inactivity pause window (not overnight cap tier).

## Precedence and cap math contract

1. Pause always supersedes cap when multiple conditions co-occur.
2. Overnight cap mode requires normal order-size baseline from risk-limit inventory rules.
3. Cap formula is deterministic:

`capped_max_order_size_units = normal_max_order_size_units * 0.25`

## Fail-closed behavior

If required dependencies are missing, stale, or malformed (depth, inactivity, or baseline size), FR41 denies participation with fail-closed semantics:

1. pre-trade reason: `pretrade_participation_guardrail_unavailable`
2. evidence reason: `fr41_participation_dependency_unavailable`
3. submit flow writes no lifecycle side effects for denied submissions.

## Evidence retrieval filters and ordering

`GET /control/incidents/participation-guardrails` supports deterministic filters:

1. `market_id`
2. `reason_code`
3. `correlation_id`
4. `start_ts`
5. `end_ts`
6. `limit`

Result ordering is stable:

1. `observed_at_utc DESC`
2. `event_id ASC`

## Failure-mode playbook

1. **Invalid payload/filter** (`fr41_participation_invalid_payload`, HTTP 400)  
   remediation: correct malformed identifiers/timestamps/reason filters and retry with a new correlation ID.
2. **Dependency unavailable / stale evidence** (`alert_dependency_unavailable`, `alert_stale_evidence`, `fr41_participation_dependency_unavailable`, HTTP 503)  
   remediation: restore runtime/persistence dependencies and keep fail-closed posture until healthy.
3. **Persistence query/decode unavailable** (`participation_guardrail_query_failed`, `participation_guardrail_row_decode_failed`, HTTP 503)  
   remediation: restore Postgres availability/index health before re-running incident triage queries.
4. **Constraint violation on evidence write** (`participation_guardrail_constraint_violation`, HTTP 409)  
   remediation: inspect canonical identifier, timestamp, and mode/cap-field constraints in `participation_guardrail_events`.

## Cross-runbook links

1. Pre-trade gate pipeline operations: `docs/operations/pretrade-gate-pipeline.md`
2. Risk-limit policy operations: `docs/operations/risk-limit-policy-operations.md`
3. Incentive regime-shift alert operations: `docs/operations/incentive-regime-shift-alerts.md`
