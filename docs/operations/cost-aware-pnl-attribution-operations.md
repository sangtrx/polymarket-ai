# Cost-Aware PnL Attribution Operations Guide (Story 3.4)

## Scope

This runbook covers operator-facing cost-aware attribution reads for portfolio triage in Story 3.4.

Authenticated read endpoint:

1. `GET /control/portfolio/attribution`

Supported query filters:

1. `period` (`1h`, `24h`, `30d`; defaults to `24h`)
2. optional `market_id`
3. optional `alpha_id`

## Response and evidence contract

Successful responses are machine-readable and metadata-first:

1. top-level evidence: `as_of_utc`, `source`, `reason_code`, `correlation_id`
2. state contract: `data_state` in `{ready, empty}`
3. deterministic window bounds: `start_inclusive_utc`, `end_exclusive_utc`
4. row-level traceability: `snapshot_id` and `run_id` (when available)

`rows` are ordered deterministically for reproducible triage:

1. `period_end_utc DESC`
2. stable key ordering (`market_id`, then `alpha_id`) on ties

## Cost decomposition interpretation

Each row exposes realized/unrealized and net-cost components:

1. `realized_pnl_usd`
2. `unrealized_pnl_usd`
3. `gross_pnl_usd`
4. `fees_usd`, `rebates_usd`, `incentives_usd`
5. `net_cost_impact_usd`
6. `net_pnl_usd`

Recommended operator flow:

1. sort attention by highest absolute `net_pnl_usd`
2. compare `gross_pnl_usd` versus `net_pnl_usd` to identify cost drag
3. inspect `reason_code`, `correlation_id`, and optional `run_id` for incident trace linkage

## Empty/degraded state handling

Empty windows (`data_state=empty`) are expected for no-activity ranges and must not be treated as silent zero-fill success.

Operator-console UX contract:

1. loading uses skeleton state (no spinner-only fallback)
2. empty shows explicit next-step guidance (`recommended_next_action`)
3. critical/degraded failures render machine-readable error details

## Failure-mode playbook

1. **Invalid filters** (`attribution_invalid_payload`, HTTP 400)  
   Remediation: use canonical period values and field-level diagnostics from `field_errors`.
2. **Projection unavailable** (`attribution_projection_unavailable`, HTTP 503)  
   Remediation: restore attribution projection dependency health before retrying.
3. **Stale reconciliation source** (`attribution_stale_source`, HTTP 503)  
   Remediation: wait for fresh reconciliation evidence and re-query.
4. **Reconciliation unavailable / persistence dependency unavailable** (`attribution_persistence_unavailable`, HTTP 503)  
   Remediation: recover reconciliation evidence path, then re-run attribution query.
5. **Unauthorized attribution read** (`attribution_unauthorized`, HTTP 403)  
   Remediation: use a privileged operator role authorized for control-plane reads.

## Story boundary and handoff

Story 3.4 is limited to attribution and cost-aware PnL surfaces for dashboard triage.

Out of scope (handoff to Story 3.5):

1. multi-run forensic timeline reconstruction
2. incident search and historical deep-dive workflows
3. advanced cross-incident attribution drill-through automation
