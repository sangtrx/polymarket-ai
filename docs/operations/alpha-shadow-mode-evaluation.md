# Alpha Shadow-Mode Evaluation Operations Guide (Story 6.4 / FR9)

This runbook covers Story 6.4 shadow-evaluation execution for validated candidates using read-only simulation evidence.

## Scope and boundaries

- Dependency: Story 6.3 validation workflow evidence (`validation_runs` + `validation_artifacts`) must exist and be queryable.
- Schema scope in this story is limited to `shadow_evaluations`.
- Story 6.4 scope is read-only evaluation and queryability only; it does not implement Story 6.5 promotion decisions.

## FR9 control-plane routes

1. `POST /control/research/shadow-evaluations`
2. `GET /control/research/shadow-evaluations/{evaluation_id}`
3. `GET /control/research/shadow-evaluations?candidate_id={candidate_id}&limit={n}`

All responses use canonical `data/meta/error` envelopes.

## Read-only simulation guarantee

- Shadow evaluation performs deterministic simulation only.
- There are no live order placement/cancel pathways in shadow workflow orchestration.
- Simulation outcomes must include machine-readable proof code `shadow_simulation_read_only_enforced`.

## Start prechecks and fail-closed behavior

Start requests are denied unless:

1. `validation_run_id` resolves to a completed Story 6.3 run for the same candidate.
2. Validation diagnostics artifacts are available for that run.
3. Market-context and simulation dependencies are available.

No success-shaped fallback is emitted for unresolved state.

## Deterministic status and error mapping

| Status | Error code families |
| --- | --- |
| `400` | `shadow_evaluation_invalid_payload` |
| `403` | `shadow_evaluation_unauthorized_role` |
| `409` | `shadow_evaluation_validation_run_ineligible`, `shadow_evaluation_not_found`, constraint violations |
| `503` | `shadow_evaluation_dependency_unavailable`, `shadow_evaluation_state_unavailable`, `shadow_evaluation_persistence_unavailable` |
| `500` | uncategorized internal failures |

## Evidence continuity (NFR14)

Allow and deny outcomes preserve:

1. actor id / role
2. action
3. candidate id / evaluation id
4. reason code
5. correlation id
6. UTC timestamp

Unauthorized security-signal names:

1. `unauthorized_shadow_evaluation_read_attempt_v1`
2. `unauthorized_shadow_evaluation_mutation_attempt_v1`

## Cross-runbook links

1. Validation workflow and diagnostics (Story 6.3): `docs/operations/alpha-validation-workflow-and-diagnostics.md`
2. Validation gate policies (Story 6.2): `docs/operations/alpha-validation-gate-policies.md`
3. Counterfactual replay stress gating (Story 6.6): `docs/operations/alpha-counterfactual-replay-stress-gating.md`
