# Incident Search and Causal Timeline Forensics (Story 3.5)

## Scope

This runbook covers authenticated incident forensics reads delivered in Story 3.5.

Endpoint:

1. `GET /control/incidents/forensics`

Canonical filters:

1. `market_id`
2. `order_id`
3. `alpha_id`
4. `actor_id`
5. `start_ts` (inclusive)
6. `end_ts` (exclusive)

## Response and evidence contract

Successful responses are machine-readable and timeline-oriented:

1. top-level evidence: `reason_code`, `source`, `correlation_id`, `query_latency_ms`, `p95_latency_target_ms`
2. causal-flow framing: `trigger`, `context`, `action`, `verification`
3. deterministic window bounds: `start_inclusive_utc`, `end_exclusive_utc`
4. event-level evidence: `occurred_at`, `stage`, `source`, `reason_code`, `correlation_id`, plus `run_id` / `snapshot_id` when available

Timeline ordering is deterministic:

1. `occurred_at DESC`
2. stable event id tie-break ordering for equal timestamps

## Operator workflow

1. submit one query with canonical filters and UTC window bounds.
2. review causal flow in order: Trigger -> Context -> Action -> Verification.
3. if severity is `critical` or dependencies are stale/unavailable, trigger containment controls first.
4. use `run_id` and `snapshot_id` to pivot into reconciliation/attribution evidence without manual log stitching.

## Failure-mode playbook

1. **Invalid payload** (`incident_invalid_payload`, HTTP 400)  
   remediation: correct canonical identifier format and provide `start_ts` + `end_ts` together.
2. **Unauthorized incident read** (`incident_unauthorized`, HTTP 403)  
   remediation: use a role authorized for control-plane reads.
3. **Dependency unavailable** (`incident_dependency_unavailable`, HTTP 503)  
   remediation: recover reconciliation/attribution dependency health before retrying.
4. **Stale evidence** (`incident_stale_evidence`, HTTP 503)  
   remediation: wait for fresh evidence projection and rerun the same bounded query.

## Story boundary and handoff

Story 3.5 is limited to incident search and causal timeline forensics.

Out of scope:

1. severity-based alert delivery and channel routing (Story 3.6)
2. controlled recovery readiness gate execution (Story 3.7)

## Related runbooks

1. severity alert delivery and fallback policy: `docs/operations/severity-alert-delivery.md`
2. emergency containment controls: `docs/operations/emergency-safe-state-controls.md`
3. controlled recovery readiness gates and resume verification: `docs/operations/controlled-recovery-readiness-gates.md`
