# Controlled Recovery Readiness Gates (Story 3.7)

## Scope

This runbook defines controlled recovery evaluation, resume verification, and evidence query workflows introduced in Story 3.7.

Endpoints:

1. `POST /control/recovery/readiness/evaluate`
2. `POST /control/recovery/resume`
3. `GET /control/recovery/runs/{run_id}`
4. `GET /control/recovery/runs?correlation_id=<id>`

## Readiness gates and deterministic thresholds

Resume is blocked unless all gates pass in a single readiness run.

1. **Freshness gate**: passes only when max data age is `<= 30s`.
2. **Reconciliation gate**: passes only when mismatch rate is `< 0.1%` (exactly `0.1%` fails).
3. **Risk checksum gate**: passes only on exact approved vs computed digest equality.
4. **Operator sign-off gate**: passes only when explicit sign-off intent is recorded.

Every run persists full gate evidence (`run_id`, `correlation_id`, per-gate outcomes, reason codes, timestamps, and sign-off metadata).

## Request contracts

### `POST /control/recovery/readiness/evaluate`

```json
{
  "profile_key": "default",
  "reconciliation_run_id": "recon-2026-04-06-001",
  "approved_checksum": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "signoff_intent": "Operator confirms controlled recovery readiness evidence.",
  "audit_reference": "arb-2026-0007"
}
```

### `POST /control/recovery/resume`

```json
{
  "run_id": "recovery::gate::default::corr-123::20260406120000",
  "resumed_at_utc": "2026-04-06T12:00:05Z"
}
```

## Response and evidence expectations

Successful readiness and query responses include:

1. top-level decision (`readiness_status`, `reason_code`, `recommended_next_action`)
2. failed gate list (`failing_gate_codes`) when blocked
3. per-gate `Trigger -> Context -> Action -> Verification` evidence
4. traceability fields (`run_id`, `correlation_id`, `timestamp_utc`, optional `audit_reference`)

Successful resume responses include:

1. gate verdict (`readiness_status`, `reason_code`)
2. verification envelope (`verification_reason_code`, `resumed_at_utc`, `verification_timestamp_utc`)
3. run traceability (`run_id`, `correlation_id`, optional `audit_reference`)

## Failure-mode playbook

1. **Invalid payload** (`recovery_invalid_payload`, HTTP 400)  
   remediation: correct malformed checksum, sign-off intent, or required selector fields.
2. **Unauthorized command** (`recovery_unauthorized`, HTTP 403)  
   remediation: use authorized operational role (`operational_control` or `administrative_actions`).
3. **Run not found** (`recovery_not_found`, HTTP 404)  
   remediation: query by `correlation_id` to locate latest readiness run and retry with canonical `run_id`.
4. **Stale/blocked evidence** (`recovery_stale_evidence`, HTTP 409)  
   remediation: resolve failing gates and rerun readiness evaluation before resume.
5. **Dependency/persistence unavailable** (`recovery_dependency_unavailable` / `recovery_persistence_unavailable`, HTTP 503)  
   remediation: restore freshness/reconciliation/risk-limit dependencies and rerun readiness.

## Operator workflow

1. Stabilize containment preconditions (freshness, reconciliation, risk bundle state).
2. Run readiness evaluation and inspect per-gate outcomes in Trigger -> Context -> Action -> Verification order.
3. If blocked, execute gate-specific remediation and repeat readiness evaluation.
4. If approved, execute resume and capture verification evidence in incident timeline.
5. Confirm recovery traceability by querying latest run by `run_id` or `correlation_id`.

## Related runbooks

1. Emergency containment controls: `docs/operations/emergency-safe-state-controls.md`
2. Incident forensics timeline: `docs/operations/incident-search-causal-timeline-forensics.md`
3. Severity alert delivery and fallback policy: `docs/operations/severity-alert-delivery.md`
