# Alpha Promotion Lifecycle Governance Operations Guide (Story 6.5 / FR8 / FR11 / FR45)

This runbook covers governed alpha lifecycle decisions (`promote`, `pause`, `retire`) with deterministic threshold checks, FR45 evidence completeness, and approval enforcement.

## Scope and boundaries

- Dependency surfaces:
  - Story 6.2 promotion-stage gate evaluation (`evaluate_promotion_entry_gates(...)`).
  - Story 6.3 validation run + artifact evidence.
  - Story 6.4 shadow-readiness context (read-only consumption only).
- Schema scope in Story 6.5 is limited to `promotion_decisions`.
- Out of scope for Story 6.5 implementation: Story 6.7 live-health monitors, Story 6.8 governance-card UX, Story 6.9 automatic deallocation. (Story 6.6 replay execution is now covered in `alpha-counterfactual-replay-stress-gating.md`.)

## Control-plane routes

1. `POST /control/research/promotion-decisions`
2. `GET /control/research/promotion-decisions/{decision_id}`
3. `GET /control/research/promotion-decisions?candidate_id={candidate_id}&limit={n}`

All responses use canonical `data/meta/error` envelopes.

## FR8 lifecycle action contract

- Accepted actions are canonical-only: `promote`, `pause`, `retire`.
- Unsupported action values fail closed with `promotion_decision_unsupported_action`.
- Actions remain auditable with actor/correlation/timestamp continuity.

## FR45 promotion packet completeness

For `promote` actions, the evidence packet must include all required fields:

1. `data_quality_report`
2. `purged_cpcv_results`
3. `calibration_report`
4. `counterfactual_replay_summary`

Missing fields deny progression with `promotion_decision_missing_evidence` and explicit field diagnostics. No success-shaped fallback is emitted.

## Deterministic threshold semantics

Comparator behavior is deterministic and test-covered:

1. `lt`: observed `<` threshold
2. `lte`: observed `<=` threshold
3. `gt`: observed `>` threshold
4. `gte`: observed `>=` threshold

Boundary equality is rejected for strict comparators (`lt`, `gt`) and accepted for inclusive comparators (`lte`, `gte`).

## Governed sign-off and approval workflow

- Promotion actions that include approval evidence use governance execution checks with `action_id = strategy_promotion_override`.
- Control-plane mutation handling accepts `approval_request_id` and derives `approval_reference` from approved workflow evidence.
- Direct client-supplied `approval_reference` without a governed request is rejected.
- Missing, pending, denied, or invalid approval state denies action with deterministic reason codes (`promotion_decision_approval_required`, `promotion_decision_approval_invalid_state`).

## Fail-closed status and reason mapping

| Status | Error code families |
| --- | --- |
| `400` | `promotion_decision_invalid_payload` |
| `403` | `promotion_decision_unauthorized_role` |
| `409` | `promotion_decision_not_found`, `promotion_decision_missing_evidence`, `promotion_decision_threshold_failed`, `promotion_decision_gate_denied`, `promotion_decision_approval_required`, `promotion_decision_approval_invalid_state`, constraint violations |
| `503` | `promotion_decision_dependency_unavailable`, `promotion_decision_state_unavailable`, `promotion_decision_persistence_unavailable`, query/decode unavailable paths |
| `500` | uncategorized internal failures |

## Deny-path playbooks

1. **Invalid payload** (`promotion_decision_invalid_payload`, HTTP 400)  
   remediation: correct `field_errors[]`, keep canonical casing/identifiers, and resubmit.
2. **Unauthorized role** (`promotion_decision_unauthorized_role`, HTTP 403)  
   remediation: use an authorized role and a new correlation id.
3. **Promotion conflict** (`promotion_decision_missing_evidence`, `promotion_decision_threshold_failed`, `promotion_decision_gate_denied`, `promotion_decision_approval_required`, `promotion_decision_approval_invalid_state`, HTTP 409)  
   remediation: resolve evidence/threshold/gate/approval requirements before retry.
4. **Dependency/state/persistence unavailable** (`promotion_decision_dependency_unavailable`, `promotion_decision_state_unavailable`, `promotion_decision_persistence_unavailable`, HTTP 503)  
   remediation: preserve fail-closed posture, restore dependency health, and retry.

## Evidence continuity (NFR8 / NFR17)

Allow and deny outcomes preserve:

1. actor id / role
2. action + endpoint + method
3. candidate id / decision id
4. reason code
5. approval request/reference evidence (when present)
6. correlation id
7. UTC timestamp

Unauthorized security-signal names:

1. `unauthorized_promotion_decision_read_attempt_v1`
2. `unauthorized_promotion_decision_mutation_attempt_v1`

## Cross-runbook links

1. Validation gate policies (Story 6.2): `docs/operations/alpha-validation-gate-policies.md`
2. Validation workflow and diagnostics (Story 6.3): `docs/operations/alpha-validation-workflow-and-diagnostics.md`
3. Shadow-mode evaluation (Story 6.4): `docs/operations/alpha-shadow-mode-evaluation.md`
4. Counterfactual replay stress gating (Story 6.6): `docs/operations/alpha-counterfactual-replay-stress-gating.md`
5. Live alpha health monitoring and threshold breaches (Story 6.7): `docs/operations/alpha-live-health-monitoring-threshold-breaches.md`
6. Governance approval workflow implementation: `services/governance-service/src/approvals/mod.rs`
7. Role model and privileged controls: `docs/governance/rbac-role-model.md`
