# Normalized Reporting Read Models Operations Guide (Story 4.1)

## Scope

This runbook defines normalized reporting read-model semantics introduced in Story 4.1.

Normalized datasets:

1. `trade` read rows
2. `position` read rows
3. `risk_event` read rows
4. `performance` read rows

## Dataset semantics and source mappings

### Trade read model (`reporting_trade_read_models`)

Primary sources:

1. `user_stream_events` (`event_kind='trade'`)
2. `orders`
3. `order_state_transitions` (latest state tie-break)

Semantics:

1. canonical identifiers: `report_row_id`, `order_id`, `trade_id`, `market_id`, `asset_id`
2. deterministic event ordering: `occurred_at_utc DESC, market_id ASC, order_id ASC, trade_id ASC, report_row_id ASC`
3. evidence metadata always present via `as_of_utc`, `source`, `reason_code`, `correlation_id`

### Position read model (`reporting_position_read_models`)

Primary sources:

1. `exposure_snapshots`
2. `reconciliation_runs`

Semantics:

1. position envelope includes `net_exposure`, `gross_exposure`, `open_order_count`, and reconciliation `run_status`
2. retrieval windows are bounded by `as_of_utc` with stable tie-break ordering
3. run/window evidence stays traceable through `run_id`, `snapshot_id`, and reconciliation timestamps

### Risk-event read model (`reporting_risk_event_read_models`)

Primary sources:

1. `pretrade_gate_decisions`
2. `freshness_gate_events`
3. `safety_control_actions`
4. `recovery_gate_runs`
5. `incident_query_views`

Semantics:

1. heterogeneous risk evidence is normalized into one shape: `event_type`, `severity`, `outcome`, `event_at_utc`
2. severity and outcome remain machine-readable and deterministic
3. retrieval is deterministic: `event_at_utc DESC, event_type ASC, report_row_id ASC`

### Performance read model (`reporting_performance_read_models`)

Primary source:

1. `attribution_snapshots`

Semantics:

1. period-safe metrics: `period_scope`, `period_start_utc`, `period_end_utc`
2. cost-aware fields retained: `fees_usd`, `rebates_usd`, `incentives_usd`
3. deterministic ordering: `period_end_utc DESC, market_id ASC, alpha_id ASC, report_row_id ASC`

## Evidence field contract

Every normalized reporting row must include:

1. `as_of_utc`
2. `source`
3. `reason_code`
4. `correlation_id`
5. `run_id` / `snapshot_id` when upstream evidence provides them

Reads fail closed if required evidence fields cannot be produced deterministically.

## Operational query guidance

1. Always query with explicit `start_inclusive_utc` and `end_exclusive_utc` bounds.
2. Keep limits bounded (`1..=500`) for deterministic retrieval and latency safety.
3. Prefer canonical filters (`market_id`, `alpha_id`, `correlation_id`) to reduce incident triage noise.
4. Treat `reporting_empty_window` as an explicit zero-result state, not an implicit success fallback.

## Failure-mode playbook

1. **Invalid filters** (`reporting_invalid_payload`, HTTP 400)  
   remediation: correct malformed timestamps, identifiers, or out-of-range limits.
2. **Unauthorized read** (`reporting_unauthorized`, HTTP 403)  
   remediation: use a role with `read_analytics` permission.
3. **Dependency unavailable** (`reporting_dependency_unavailable`, HTTP 503)  
   remediation: restore source projection dependencies before retrying.
4. **Stale dependency** (`reporting_stale_dependency`, HTTP 503)  
   remediation: wait for fresh projections and repeat the bounded query.
5. **Persistence unavailable** (`reporting_persistence_unavailable`, HTTP 503)  
   remediation: recover read-model storage/query path before retry.
6. **Evidence unavailable** (`reporting_evidence_unavailable`, HTTP 503)  
   remediation: resolve upstream evidence completeness gaps before serving reads.

## Story 4.2 handoff seam

Story 4.1 exposes normalized internal read-model contracts for future API consumption.

Story 4.2 should consume these internal contracts directly without reshaping core records.

## Explicit non-goals for Story 4.1

1. no endpoint version negotiation
2. no published versioned schema changelog machinery
3. no recurring schedule execution
4. no export job orchestration or artifact publication
