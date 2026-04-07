# Alpha Hypothesis Registry Operations Guide (Story 6.1)

## Scope

This runbook covers FR6 control-plane registration/read workflows for `alpha_hypotheses` and NFR17 evidence continuity.

Authenticated endpoints:

1. `POST /control/research/alpha-hypotheses/{hypothesis_id}`
2. `GET /control/research/alpha-hypotheses/{hypothesis_id}`

## Required FR6 metadata contract

`POST /control/research/alpha-hypotheses/{hypothesis_id}` requires:

1. `hypothesis_id` (path identifier, canonical `trim + lowercase`)
2. `feature_set_version` (registered dataset snapshot reference)
3. `target_regime`
4. `expected_edge_source`
5. `training_window_start_utc` and `training_window_end_utc` (`start < end`, RFC3339 UTC)
6. `risk_assumptions` (non-empty JSON object; null/blank nested values rejected)

Deterministic boundary behavior:

1. Canonical identifier normalization is enforced across domain/service/persistence.
2. `training_window` boundary enforcement (`training_window_start_utc < training_window_end_utc`) is strict; equality is rejected.
3. Duplicate `hypothesis_id` submissions perform deterministic upsert semantics (no duplicate active-row ambiguity).

## Dataset snapshot registry seam (Story 6.1 assumption)

Story 6.1 intentionally avoids schema expansion beyond `alpha_hypotheses`.  
Dataset registration is verified through `DatasetSnapshotRegistryPort` with the current static source:

- env var `RESEARCH_DATASET_SNAPSHOT_VERSIONS` (comma-separated canonical feature-set versions),
- fallback default `dataset::v1` for local/bootstrap workflows.

Future Stories 6.2/6.3 can replace this seam with a durable registry source without changing the FR6 route contract.

## Evidence and audit contract (NFR17)

Success and denial paths emit telemetry/audit evidence including:

1. actor and role
2. action (`alpha_hypothesis_register` / `alpha_hypothesis_read`)
3. hypothesis parameters and reason code
4. correlation id
5. UTC timestamp

Audit lookup example:

```sql
SELECT hypothesis_id, feature_set_version, target_regime, expected_edge_source, actor_id, correlation_id, updated_at_utc
FROM alpha_hypotheses
WHERE lower(trim(hypothesis_id)) = lower(trim('alpha::mean-reversion'))
ORDER BY updated_at_utc DESC, hypothesis_id ASC
LIMIT 20;
```

## Failure-mode playbook

1. **Invalid payload** (`alpha_hypothesis_invalid_payload`, `alpha_hypothesis_invalid_training_window`, HTTP 400)  
   remediation: correct `field_errors[]` and resubmit canonical UTC payload.
2. **Unauthorized role** (`alpha_hypothesis_unauthorized_role`, HTTP 403)  
   remediation: use privileged mutation/read role and retry with new correlation id.
3. **Dataset reference unresolved or read target missing** (`alpha_hypothesis_dataset_snapshot_unresolved`, `alpha_hypothesis_not_found`, HTTP 409)  
   remediation: register/confirm snapshot reference and hypothesis id before retry.
4. **Dependency unavailable** (`alpha_hypothesis_dataset_snapshot_unavailable`, `alpha_hypothesis_persistence_unavailable`, HTTP 503)  
   remediation: preserve fail-closed posture, recover dependency health, then retry.

## Cross-runbook links

1. Report export workflows: `docs/operations/report-export-workflows.md`
2. Reward-per-risk policy operations: `docs/operations/reward-risk-policy-operations.md`
3. Risk-limit policy operations: `docs/operations/risk-limit-policy-operations.md`
4. Core/satellite market stratification operations: `docs/operations/core-satellite-market-stratification.md`
5. Allocation/rebalance workflows: `docs/operations/allocation-rebalance-workflows.md`
