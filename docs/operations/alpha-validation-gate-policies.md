# Alpha Validation Gate Policies Operations Guide (Story 6.2)

## Scope

This runbook covers FR43 control-plane validation-gate policy configuration, read, and evaluation workflows for `validation_gate_policies` with NFR17 evidence continuity.

Authenticated endpoints:

1. `POST /control/research/validation-gate-policies/{policy_key}`
2. `GET /control/research/validation-gate-policies/{policy_key}`
3. `GET /control/research/validation-gate-policies?stage={training|promotion}`
4. `POST /control/research/validation-gate-policies/evaluate/{stage}`

## FR43 mandatory gate catalog contract

Every workflow stage must enforce mandatory policies for:

1. `forward_bias`
2. `data_leakage`
3. `regime_survivability`
4. at least one stage-applicable `data_quality` gate

Policy definition contract:

1. `policy_key` (canonical `trim + lowercase`)
2. `gate_type` (`forward_bias`, `data_leakage`, `regime_survivability`, `data_quality`)
3. `stage_scope` (`training`, `promotion`, `training_and_promotion`)
4. `metric_key`
5. `comparator` (`lt`, `lte`, `gt`, `gte`)
6. `threshold_value` (finite float)
7. `mandatory` (FR43 mandatory gate types must remain `true`)
8. `diagnostics` (non-empty JSON object with machine-readable fail criteria)

## Deterministic boundary semantics

Comparator behavior is deterministic and test-covered:

1. `lt`: `observed < threshold`
2. `lte`: `observed <= threshold`
3. `gt`: `observed > threshold`
4. `gte`: `observed >= threshold`

Boundary equality behavior:

1. `lt`/`gt` reject equality.
2. `lte`/`gte` accept equality.

## Fail-closed evaluation behavior

If policy catalog state or gate inputs are missing/ambiguous, progression is denied.

Primary machine-readable deny/error codes:

1. `validation_gate_missing_mandatory_policy`
2. `validation_gate_policy_unresolved`
3. `validation_gate_failed`
4. `validation_gate_dependency_unavailable`
5. `validation_gate_state_unavailable`
6. `validation_gate_persistence_unavailable`

No success-shaped fallback is emitted for unresolved state.

## Evidence and audit continuity (NFR17)

Allow and deny paths emit telemetry/audit records with:

1. actor and role
2. action (`validation_gate_policy_upsert`, `validation_gate_policy_read`, `validation_gate_policy_list`, `validation_gate_evaluate`)
3. policy/evaluation parameters
4. reason code
5. correlation id
6. UTC timestamp

Audit query examples:

```sql
SELECT policy_key, gate_type, stage_scope, metric_key, comparator, threshold_value, mandatory, actor_id, correlation_id, updated_at_utc
FROM validation_gate_policies
WHERE lower(trim(stage_scope)) IN ('training', 'training_and_promotion')
ORDER BY stage_scope ASC, gate_type ASC, policy_key ASC;
```

```sql
SELECT policy_key, gate_type, stage_scope, correlation_id, updated_at_utc
FROM validation_gate_policies
WHERE lower(trim(policy_key)) = lower(trim('fr43::forward-bias::primary'))
ORDER BY updated_at_utc DESC, policy_key ASC
LIMIT 20;
```

## Failure-mode playbooks

1. **Invalid payload** (`validation_gate_invalid_payload`, HTTP 400)  
   remediation: correct `field_errors[]` and resubmit canonical payload.
2. **Unauthorized role** (`validation_gate_unauthorized_role`, HTTP 403)  
   remediation: use an authorized privileged role and retry with a new correlation id.
3. **Policy conflict/unresolved/threshold failure** (`validation_gate_policy_not_found`, `validation_gate_policy_unresolved`, `validation_gate_missing_mandatory_policy`, `validation_gate_failed`, HTTP 409)  
   remediation: repair policy catalog or gate thresholds before replaying evaluation.
4. **Dependency/state unavailable** (`validation_gate_dependency_unavailable`, `validation_gate_state_unavailable`, `validation_gate_persistence_unavailable`, HTTP 503)  
   remediation: preserve fail-closed posture, restore dependency health, and retry.

## Cross-runbook links

1. Alpha hypothesis registry operations (FR6): `docs/operations/alpha-hypothesis-registry.md`
2. Reward-per-risk policy operations (FR39): `docs/operations/reward-risk-policy-operations.md`
3. Risk-limit policy operations: `docs/operations/risk-limit-policy-operations.md`
4. Report export workflows operations: `docs/operations/report-export-workflows.md`
