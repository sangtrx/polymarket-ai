# Alpha Counterfactual Replay Stress Gating Operations Guide (Story 6.6 / FR46 / NFR14)

This runbook covers Story 6.6 counterfactual replay execution (`baseline`, `stressed_execution`, `delayed_exit`) and deterministic FR46 promotion gating.

## Scope and boundaries

- Dependency surfaces:
  - Story 6.3 validation run + diagnostics evidence.
  - Story 6.4 shadow evaluation evidence.
  - Story 6.5 promotion decision orchestration (replay evidence consumed for `promote` actions).
- Schema scope in Story 6.6 is limited to `counterfactual_replay_runs`.
- Out of scope: Story 6.7 live-health monitoring, Story 6.8 governance-card UX, Story 6.9 automatic deallocation policies.

## Control-plane routes

1. `POST /control/research/counterfactual-replay-runs`
2. `GET /control/research/counterfactual-replay-runs/{replay_run_id}`
3. `GET /control/research/counterfactual-replay-runs?candidate_id={candidate_id}&limit={n}`

All responses use canonical `data/meta/error` envelopes.

## FR46 scenario contract

Each replay run deterministically evaluates and persists all required scenarios under one run id:

1. `baseline`
2. `stressed_execution` (`2x` slippage, `50%` reduced fill rate)
3. `delayed_exit` (`60` second exit delay)

Each scenario includes machine-readable `reason_code`, `gate_outcome`, and scenario parameter metadata.

## FR46 formula and boundary semantics

- Degradation formula: `degradation_pct = ((stressed_net_pnl - baseline_net_pnl) / baseline_net_pnl) * 100`
- Deny threshold: `degradation_pct < -5.0`
- Boundary rule: `degradation_pct == -5.0` is explicitly allow-path
- Promotion integration uses reason code `promotion_decision_replay_gate_denied` when replay gate denies.

## Baseline admissibility and fail-closed rules

- Baseline net PnL must be finite and `> 0`.
- Invalid baseline denominator (`<= 0`, missing, non-finite) fails closed with explicit machine-readable diagnostics.
- Dependency/state/persistence ambiguity fails closed; no success-shaped replay fallback is emitted.

## Story 6.5 integration continuity

- Deferred placeholders (`counterfactual_replay_summary.status = deferred_to_story_6_6`) are no longer valid allow-path evidence.
- Promote decisions materialize canonical replay summary evidence and evaluate replay gate outcomes deterministically.
- Existing approval and threshold contracts remain intact while replay-gate integration is enforced.

## Deterministic status and error mapping

| Status | Error code families |
| --- | --- |
| `400` | `counterfactual_replay_invalid_payload` |
| `403` | `counterfactual_replay_unauthorized_role` |
| `409` | `counterfactual_replay_run_not_found`, `counterfactual_replay_scenario_incomplete`, constraint violations |
| `503` | `counterfactual_replay_dependency_unavailable`, `counterfactual_replay_state_unavailable`, `counterfactual_replay_persistence_unavailable`, query/decode unavailable paths |
| `500` | uncategorized internal failures |

## Telemetry and audit continuity (NFR14)

Allow and deny outcomes preserve:

1. replay run id / candidate id / validation run id
2. action + endpoint + method
3. run state and reason code
4. actor id / role
5. correlation id
6. UTC timestamp

Unauthorized security-signal names:

1. `unauthorized_counterfactual_replay_read_attempt_v1`
2. `unauthorized_counterfactual_replay_mutation_attempt_v1`

## Deny-path playbooks

1. **Invalid payload** (`counterfactual_replay_invalid_payload`, HTTP 400)  
   remediation: correct field-level diagnostics and resubmit canonical payload.
2. **Unauthorized role** (`counterfactual_replay_unauthorized_role`, HTTP 403)  
   remediation: use authorized role and new correlation id.
3. **Conflict/not-found** (`counterfactual_replay_run_not_found`, `counterfactual_replay_scenario_incomplete`, HTTP 409)  
   remediation: verify replay run identity and scenario completeness before retry.
4. **Unavailable dependency/state/persistence** (`counterfactual_replay_*_unavailable`, HTTP 503)  
   remediation: restore dependency health and retry while preserving fail-closed posture.

## Cross-runbook links

1. Promotion lifecycle governance (Story 6.5): `docs/operations/alpha-promotion-lifecycle-governance.md`
2. Validation workflow and diagnostics (Story 6.3): `docs/operations/alpha-validation-workflow-and-diagnostics.md`
3. Shadow-mode evaluation (Story 6.4): `docs/operations/alpha-shadow-mode-evaluation.md`
4. Live alpha health monitoring and threshold breaches (Story 6.7): `docs/operations/alpha-live-health-monitoring-threshold-breaches.md`
5. Automatic deallocation + stop-research (Story 6.9): `docs/operations/alpha-automatic-deallocation-stop-research.md`
6. Governance approval workflow implementation: `services/governance-service/src/approvals/mod.rs`
