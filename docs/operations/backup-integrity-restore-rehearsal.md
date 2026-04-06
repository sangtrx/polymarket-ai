# Backup Integrity and Deterministic Restore Rehearsal (Story 3.8)

## Scope

This runbook defines backup-integrity rehearsal execution, deterministic replay validation, and rehearsal evidence query workflows delivered in Story 3.8.

Endpoints:

1. `POST /control/recovery/rehearsals`
2. `GET /control/recovery/rehearsals/{run_id}`
3. `GET /control/recovery/rehearsals?artifact_id=<id>&limit=<n>`
4. `GET /control/recovery/rehearsals?correlation_id=<id>&limit=<n>`

## Deterministic integrity checks

A rehearsal run is `passed` only when all checks pass:

1. **Checksum match**: expected and observed checksums must be exact lowercase 64-hex equality.
2. **Reconciliation sanity**: mismatch rate must remain below the deterministic threshold.
3. **Deterministic replay**: canonicalized restore output signature must match prior evidence (except allowed run metadata variance).

Any failed check marks run status as `failed` and emits explicit reason codes:

1. `recovery_rehearsal_checksum_mismatch`
2. `recovery_rehearsal_reconciliation_sanity_failure`
3. `recovery_rehearsal_deterministic_replay_mismatch`

## Request contract

### `POST /control/recovery/rehearsals`

```json
{
  "artifact_id": "backup-artifact-2026-04-06",
  "artifact_checksum": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "restore_target": "sandbox-restore-target",
  "reconciliation_run_id": "recon-2026-04-06-001",
  "restore_output": {
    "positions": 120,
    "balances": {
      "usd": "500000.00"
    }
  },
  "observed_checksum": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "incident_correlation_id": "incident-corr-001",
  "incident_severity": "severity_1",
  "audit_reference": "arb-2026-0010"
}
```

## Evidence and query contract

Each run persists verifiable evidence:

1. run traceability (`run_id`, `correlation_id`, `artifact_id`, timestamps)
2. integrity evidence (`artifact_checksum`, `observed_checksum`, per-check outcomes)
3. deterministic replay evidence (`deterministic_signature`, optional prior signature, mismatch summary)
4. reconciliation evidence (`reconciliation_run_id`, mismatch summary, pass/fail)
5. final verdict (`status`, `reason_code`, recommended next action)

## Severe-incident resume gating

For Severity-1/Severity-2 paths, controlled resume is blocked unless latest relevant rehearsal evidence is successful.

Selector priority:

1. explicit `rehearsal_run_id` override (audit replay)
2. latest successful evidence by incident correlation selector
3. latest successful evidence by artifact selector

Blocked severe resume reason:

1. `recovery_rehearsal_missing_or_failed`

## Failure-mode playbook

1. **Invalid payload** (`recovery_invalid_payload`, HTTP 400)  
   remediation: fix malformed artifact/checksum/selector fields.
2. **Unauthorized command/query** (`recovery_unauthorized`, HTTP 403)  
   remediation: use authorized operational role context.
3. **Rehearsal not found** (`recovery_not_found`, HTTP 404)  
   remediation: query by artifact/correlation and retry with canonical `run_id`.
4. **Constraint violation / stale evidence** (`restore_rehearsal_constraint_violation` or `recovery_stale_evidence`, HTTP 409)  
   remediation: rerun rehearsal with corrected restore inputs and deterministic evidence.
5. **Dependency or persistence unavailable** (`recovery_dependency_unavailable`, `recovery_persistence_unavailable`, HTTP 503)  
   remediation: restore reconciliation/restore dependencies and rerun rehearsal.

## Related runbooks

1. Controlled recovery readiness gates: `docs/operations/controlled-recovery-readiness-gates.md`
2. Incident forensics timeline: `docs/operations/incident-search-causal-timeline-forensics.md`
3. Severity alert delivery and fallback policy: `docs/operations/severity-alert-delivery.md`
