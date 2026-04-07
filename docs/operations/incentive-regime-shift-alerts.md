# Incentive Regime-Shift Alert Operations (Story 5.2)

## Scope

This runbook covers FR40 regime-shift detection, dispatch, and evidence retrieval for materially changing venue economics.

Endpoints:

1. `GET /control/incidents/regime-shifts`
2. `POST /control/incidents/regime-shifts/dispatch`

## FR40 trigger matrix and boundary semantics

Regime-shift detection uses strict `>` thresholds:

1. rebate-rate delta: `abs(current_maker_rebate_bps - previous_maker_rebate_bps) > 20 bps` -> `fr40_regime_rebate_delta_exceeded`
2. spread widening: `(current_spread_bps - previous_spread_bps) > 50 bps` -> `fr40_regime_spread_widening_exceeded`
3. eligibility transition: `eligible` <-> (`restricted` or `ineligible`) -> `fr40_regime_eligibility_transition`

Exact boundary values (`20 bps`, `50 bps`) do **not** trigger.

## Alert payload and dispatch contract

Regime-shift dispatch reuses the established incident-alert delivery pipeline (primary/fallback channels, SLA checks, and attempt evidence).

Required operator payload context:

1. `reason_code`
2. `severity`
3. `market_id`
4. `cluster_id`
5. `observed_at`
6. `correlation_id`
7. `recommended_next_action`
8. `evidence_link`

Additional FR40 evidence context includes threshold values plus before/after rebate, spread, and eligibility fields.

## Dedupe and fallback behavior

1. Dispatch uses `should_emit_alert(...)` with a bounded dedupe window (`dedupe_window_seconds`, default `300`).
2. Duplicate reason/correlation candidates inside the window are suppressed (`alert_duplicate_suppressed`).
3. Primary delivery failure and SLA breach paths reuse existing Story 3.6 fallback behavior and machine-readable reason codes.

## Evidence retrieval and triage filters

`GET /control/incidents/regime-shifts` supports deterministic retrieval by:

1. `market_id`
2. `reason_code`
3. `correlation_id`
4. `start_ts`
5. `end_ts`
6. `limit`

Rows are ordered by `observed_at DESC, alert_id ASC` for stable operator triage.

## Failure-mode playbook

1. **Invalid payload / invalid FR40 contract** (`alert_invalid_payload` or `fr40_regime_invalid_payload`, HTTP 400)  
   remediation: correct malformed identifiers/timestamps/state fields and retry with new correlation.
2. **Unavailable dependency or stale evidence** (`alert_dependency_unavailable`, `alert_stale_evidence`, `fr40_regime_dependency_unavailable`, HTTP 503)  
   remediation: restore persistence/runtime dependencies, keep fail-closed posture, and re-run dispatch.
3. **Persistence query/decode unavailable** (`regime_shift_query_failed`, `regime_shift_row_decode_failed`, HTTP 503)  
   remediation: restore Postgres path/index health before repeating reads.
4. **Constraint violation on evidence write** (`regime_shift_constraint_violation`, HTTP 409)  
   remediation: inspect canonical-id/timestamp/value constraints and correct producer payload.
5. **No threshold trigger** (`alert_no_trigger`, HTTP 400)  
   remediation: confirm expected baseline/current deltas; no dispatch occurs when thresholds are not breached.

## Cross-runbook links

1. Reward-per-risk policy operations: `docs/operations/reward-risk-policy-operations.md`
2. Severity alert delivery and fallback: `docs/operations/severity-alert-delivery.md`
3. Incident forensics timeline: `docs/operations/incident-search-causal-timeline-forensics.md`
4. FR41 participation guardrails: `docs/operations/low-liquidity-overnight-guardrails.md`
