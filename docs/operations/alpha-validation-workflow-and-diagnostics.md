# Alpha Validation Workflow and Diagnostics (FR7/FR44)

This runbook covers Story 6.3 validation-run execution, diagnostics persistence, and control-plane evidence retrieval.

## Scope and dependency boundaries

- Dependency: Story 6.2 validation-gate policies (`evaluate_training_entry_gates(...)` is required at run entry).
- Schema scope in this story is limited to:
  - `validation_runs`
  - `validation_artifacts`
- Out of scope: shadow-mode simulation (Story 6.4), promotion approvals/threshold gating (Story 6.5), lifecycle automation/UX (Stories 6.6+).

## Deterministic FR7 stage workflow

Validation runs execute stages in strict order:

1. `quality`
2. `labeling`
3. `purged_cv`
4. `cpcv`
5. `overfit_diagnostics`

Fail-closed progression rules:

- If FR43 training-entry gates deny or are unresolved, run creation is denied.
- If a stage fails, downstream stages are persisted as `blocked` and are not executed.
- If stage input/executor state is unavailable, the run fails closed with explicit dependency/state reason codes.
- No success-shaped fallback is emitted.

## FR44 diagnostics artifact contract

Each persisted artifact stores deterministic diagnostics:

- `out_of_sample_sharpe`
- `max_drawdown`
- `brier_score` **or** `expected_calibration_error` (at least one required)
- `overfit_indicator`
- `overfit_flag`

Diagnostics are persisted per stage with actor/correlation/timestamps so runs can be compared deterministically.

## Control-plane API surfaces

- `POST /control/research/validation-runs`
- `GET /control/research/validation-runs/{run_id}`
- `GET /control/research/validation-runs?candidate_id={candidate_id}&limit={n}`
- `GET /control/research/validation-runs/{run_id}/artifacts/{stage}`

All responses use canonical envelope shape:

```json
{
  "data": { "kind": "run_detail|runs|artifact", "...": "..." },
  "meta": {
    "action": "validation_run_start|validation_run_read|validation_run_list|validation_artifact_read",
    "actor_id": "ops-1",
    "role": "operational_control",
    "correlation_id": "corr-...",
    "timestamp_utc": "2026-04-07T00:00:00Z",
    "endpoint": "/control/research/validation-runs"
  },
  "error": null
}
```

## Status mapping and machine-readable error codes

| Status | Error code families |
| --- | --- |
| `400` | `validation_run_invalid_payload` |
| `403` | `validation_run_unauthorized_role` |
| `409` | `validation_run_not_found`, `validation_artifact_not_found`, `validation_run_gate_denied`, `validation_run_stage_failed`, constraint violations |
| `503` | `validation_run_dependency_unavailable`, `validation_run_state_unavailable`, `validation_run_persistence_unavailable`, query/decode unavailable paths |
| `500` | any uncategorized internal error |

## Deterministic comparison semantics

- Run comparisons are generated only when the current run reaches `completed`.
- Prior baseline selection is deterministic: same candidate, completed state, latest completed run before current `started_at_utc`.
- Comparison ordering is deterministic by stage index (`quality -> ... -> overfit_diagnostics`) and stable metric key ordering.
- Comparison reason code: `validation_comparison_ready`.

## Telemetry and audit continuity (NFR14/NFR17)

Success and denial paths include:

- actor id / role
- action
- endpoint + method
- run id / candidate id / stage where applicable
- reason code
- correlation id
- UTC timestamp

Unauthorized security-signal names:

- `unauthorized_validation_run_read_attempt_v1`
- `unauthorized_validation_run_mutation_attempt_v1`

## Downstream integration seams (Stories 6.4 / 6.5)

- `validation_run_*` read/list contracts provide stable run-state packets for shadow-mode consumers.
- Artifact diagnostics payloads provide canonical metric fields for promotion checks without embedding promotion decisions in Story 6.3.
- Keep consumers on these contracts; do not fork schema or reason-code taxonomies.

## Related runbooks

- [alpha-hypothesis-registry.md](./alpha-hypothesis-registry.md)
- [alpha-validation-gate-policies.md](./alpha-validation-gate-policies.md)
- [reward-risk-policy-operations.md](./reward-risk-policy-operations.md)
- [risk-limit-policy-operations.md](./risk-limit-policy-operations.md)
- [report-export-workflows.md](./report-export-workflows.md)
