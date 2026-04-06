# Emergency Safe-State Controls

## Purpose

This runbook defines operator actions for emergency containment and automatic safe-state behavior in Story 2.9.

## Manual Emergency Controls

All commands require authenticated control-plane access and return auditable machine-readable evidence.

1. **Pause trading**
   - `POST /control/emergency/pause`
   - Effect: blocks new submissions.
2. **Reduce-only mode**
   - `POST /control/emergency/reduce-only`
   - Effect: only `reduce_only` submits are accepted; limit-mode submits are denied.
3. **Cancel-all containment**
   - `POST /control/emergency/cancel-all`
   - Effect: executes emergency cancel fan-out through existing batch-cancel lifecycle seams.

Example payload:

```json
{
  "audit_reference": "incident-2026-04-06-001"
}
```

## Automatic Safe-State Triggers

Risk-engine emits automatic emergency safe-state signals with deterministic reason codes for:

- stale feed (`emergency_control_stale_feed_triggered`)
- reconciliation-critical halt (`emergency_control_reconciliation_critical_triggered`)
- control uncertainty from unavailable critical runtime dependencies (`emergency_control_control_uncertainty_triggered`)

Trigger behavior is fail-closed and keeps new-order flow blocked until recovery gates are healthy.

## Action Verification

Query action evidence by action id:

- `GET /control/emergency/actions/{action_id}`

Expected response evidence fields:

- `timestamp_utc`
- `source` / `trigger_source` and actor context (when manual)
- `resulting_mode`
- `reason_code`
- `audit_reference`
- `correlation_id`

## Recovery Checklist

1. Confirm trigger condition has cleared (freshness, reconciliation, dependency health).
2. Verify safety mode and recent emergency actions through query endpoint.
3. Validate submit behavior matches expected containment mode.
4. Record incident closure in audit trail using final recovery control command reference.

## Related runbooks

1. severity alert delivery and channel fallback: `docs/operations/severity-alert-delivery.md`
2. incident forensics timeline search: `docs/operations/incident-search-causal-timeline-forensics.md`
3. controlled recovery readiness gates and resume verification: `docs/operations/controlled-recovery-readiness-gates.md`
