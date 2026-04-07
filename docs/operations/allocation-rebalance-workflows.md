# Allocation Policy and Drift-Rebalance Operations Guide (Story 3.3)

## Scope

This runbook governs privileged allocation-policy mutation plus drift-driven rebalance recommendation and execution workflows.

Authenticated endpoints:

1. `POST /control/allocation-policies/{policy_key}`
2. `POST /control/rebalance`
3. `GET /control/rebalance/recommendations/pending`
4. `POST /control/rebalance/recommendations/{recommendation_id}/execute`

## Allocation policy mutation contract

`POST /control/allocation-policies/{policy_key}` accepts:

1. `version` (positive integer)
2. `portfolio_scope_id` (canonical lower-case identifier)
3. `target_exposure_pct_nav` (`[0, 100]`, finite)
4. `target_relative_alpha_weight` (`>0`, finite)
5. `exposure_drift_threshold_pct` and `relative_alpha_drift_threshold_pct` (`(0, 100]`, finite)
6. Optional `approval_request_id` (required when dual-approval evidence is needed)
7. Optional `advanced_parameters` JSON object

`approval_reference` is system-derived from approved dual-approval requests and is not accepted as a direct client input.

Critical increase behavior is fail-closed:

1. Increase without approval evidence returns `status=pending` and `reason_code=allocation_policy_pending_approval`.
2. Approved paths return `status=accepted` with non-null `approval_reference` sourced from approval workflow evidence.

## Drift evaluation and recommendation lifecycle

`POST /control/rebalance` evaluates drift and emits rationale context:

1. `drift == threshold` is deterministic in-bounds (`recommendation_status=denied`, `reason_code=rebalance_in_bounds`).
2. `drift > threshold` enters recommendation flow:
   - `require_execution=false` → `recommendation_status=proposed`
   - `require_execution=true` without approval → `recommendation_status=pending_approval`
   - `require_execution=true` with approval context → `recommendation_status=approved`
3. Recommendation execution occurs only through `POST /control/rebalance/recommendations/{recommendation_id}/execute`.

## Query and evidence expectations (NFR17)

1. Pending execution-ready recommendations are queryable through `GET /control/rebalance/recommendations/pending`.
2. Responses preserve machine-readable evidence fields:
   - `reason_code`
   - `correlation_id`
   - `timestamp_utc`
   - `approval_status` and `approval_reference`
   - rationale and recommended next action
3. Privileged actions append audit records using canonical control-api authorization/audit patterns.

## Failure-mode playbooks

1. **Invalid payload** (`rebalance_invalid_payload`, HTTP 400)  
   Remediation: correct returned `field_errors[]` and resubmit canonical identifiers/numeric values.
2. **Approval required** (`rebalance_approval_required`, pending recommendation state)  
   Remediation: complete dual-approval workflow, supply `approval_request_id`, and retry execution.
3. **Unavailable policy state / stale policy state** (`rebalance_policy_state_unavailable`, `rebalance_policy_state_stale`, HTTP 503)  
   Remediation: fail closed, recover policy persistence state, then re-evaluate drift.
4. **Persistence unavailable** (`rebalance_persistence_unavailable`, HTTP 503)  
   Remediation: escalate platform on-call and retry once dependency health is restored.

## Cross-runbook links

1. Risk-limit policy operations: `docs/operations/risk-limit-policy-operations.md`
2. Pre-trade gate pipeline operations: `docs/operations/pretrade-gate-pipeline.md`
3. Core/satellite market stratification operations (FR42): `docs/operations/core-satellite-market-stratification.md`
