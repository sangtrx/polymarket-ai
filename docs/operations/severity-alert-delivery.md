# Severity Alert Delivery and Fallback Operations (Story 3.6)

## Scope

This runbook covers warning/critical incident alert retrieval and dispatch workflows delivered in Story 3.6.

Endpoints:

1. `GET /control/incidents/alerts`
2. `POST /control/incidents/alerts/dispatch`

## Trigger matrix (FR29 coverage)

Alert triggers are deterministic and use strict `>` semantics:

1. drawdown breach: `drawdown_pct_of_daily_limit > 80` -> `alert_drawdown_limit_exceeded` (critical)
2. stream disconnect: `stream_disconnect_seconds > 300` -> `alert_stream_disconnect_exceeded` (critical)
3. reconciliation lag: `reconciliation_lag_seconds > 60` -> `alert_reconciliation_lag_exceeded` (warning)
4. stale data: `stale_data_detected = true` -> `alert_stale_data_detected` (critical)
5. policy bypass attempt: `policy_bypass_attempt = true` -> `alert_policy_bypass_attempt` (critical)

Boundary events at exact thresholds are not dispatched.

## Alert payload contract

Each warning/critical alert must include:

1. `alert_id`
2. `severity`
3. `impacted_subsystem`
4. `cause`
5. `recommended_next_action`
6. `evidence_link`
7. `issued_at`
8. `correlation_id`
9. `reason_code`

## Delivery policy and fallback behavior

Dispatch uses bounded primary -> fallback handling with auditable attempts:

1. attempt primary channel (`pagerduty` by default)
2. if primary fails or critical SLA is breached, attempt fallback channel (`slack` by default)
3. persist each attempt with machine-readable reason code and timestamp evidence
4. mark final status as `delivered` or `failed` (no success-shaped fallback)

Critical SLA target:

1. dispatch completion within `<= 30s` from `issued_at`
2. SLA breach emits `alert_delivery_sla_breached`

## Required evidence fields

For each alert:

1. `alert_id`
2. `reason_code`
3. `severity`
4. `impacted_subsystem`
5. `correlation_id`
6. `issued_at`
7. `delivered_at`/`failed_at`

For each delivery attempt:

1. `alert_id`
2. `attempt_number`
3. `channel`
4. `outcome`
5. `reason_code`
6. `attempted_at`
7. `delivered_at`/`failed_at`

## Failure-mode playbook

1. **Invalid payload** (`alert_invalid_payload`, HTTP 400)  
   remediation: fix malformed fields (threshold values, evidence link, timestamps, required metadata).
2. **Unauthorized alert operation** (`alert_unauthorized`, HTTP 403)  
   remediation: use an `operational_control` credential context.
3. **Dependency unavailable** (`alert_dependency_unavailable`, HTTP 503)  
   remediation: restore dependent incident/reconciliation projections before dispatch retry.
4. **Stale evidence** (`alert_stale_evidence`, HTTP 503)  
   remediation: refresh upstream evidence and re-run dispatch.
5. **Fallback delivery failure** (`alert_delivery_fallback_failed`, HTTP 503)  
   remediation: escalate to manual containment and page on-call through secondary channel.

## Cross-runbook links

1. Incident forensics timeline: `docs/operations/incident-search-causal-timeline-forensics.md`
2. Emergency containment controls: `docs/operations/emergency-safe-state-controls.md`
