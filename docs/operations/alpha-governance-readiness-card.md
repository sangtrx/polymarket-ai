# Alpha Governance Readiness Card Operations Guide (Story 6.8 / UX-DR8 / UX-DR12 / UX-DR20)

This runbook explains how to interpret and triage the Alpha Governance Readiness card that composes existing research read models for lifecycle, validation, shadow, and guardrail readiness.

## Scope and boundaries

- Story 6.8 is frontend/read-model composition only.
- No new backend routes, schema entities, or migrations are introduced.
- Card data is read-only and fail-closed for malformed, unauthorized, or unavailable dependencies.

## Composed control-plane reads

1. `GET /control/research/promotion-decisions?candidate_id={candidate_id}&limit={n}`
2. `GET /control/research/shadow-evaluations?candidate_id={candidate_id}&limit={n}`
3. `GET /control/research/validation-runs?candidate_id={candidate_id}&limit={n}`
4. `GET /control/research/validation-runs/{run_id}`
5. `GET /control/research/alpha-health-metrics?alpha_id={alpha_id}&limit={n}`
6. `GET /control/research/alpha-threshold-breaches?alpha_id={alpha_id}&limit={n}`

All reads are interpreted from canonical `data/meta/error` envelopes.

## Deterministic readiness signals

### Lifecycle state

Card lifecycle mapping is deterministic and constrained to:

1. `deallocated` — latest allowed promotion action is `retire`
2. `production` — latest two allowed actions are consecutive `promote` decisions
3. `candidate-live` — latest allowed action is `promote` (without production streak) or `pause`
4. `shadow` — no allowed promote/retire and latest shadow evaluation is `completed`
5. `draft` — no qualifying lifecycle/shadow evidence

### Validation completeness (FR7 + FR45)

Readiness checks all required FR7 stages:

1. `quality`
2. `labeling`
3. `purged_cv`
4. `cpcv`
5. `overfit_diagnostics`

And required FR45 packet fields:

1. `data_quality_report`
2. `purged_cpcv_results`
3. `calibration_report`
4. `counterfactual_replay_summary`

### Guardrail state (FR10)

Guardrail evidence requires windows:

1. `1h`
2. `24h`
3. `30d`

Boundary semantics are explicit: floor metrics breach on strict `<`, drawdown breaches on strict `>`, and equality remains on allow-path.

### Readiness outcome

- `readiness=blocked` when `missingArtifacts[]` is non-empty.
- `readiness=ready` only when all required evidence is complete.

## Blocked-state triage

1. **`validation_stage:*` missing artifacts**
   - Re-run validation workflow and confirm run-detail artifacts for missing FR7 stages.
2. **`promotion_packet_field:*` missing artifacts**
   - Complete FR45 promotion packet fields before promotion decisioning.
3. **`shadow_stability_evidence`**
   - Complete shadow evaluation and persist simulation outcomes.
4. **`alpha_health_window:*`**
   - Publish FR10 windows (`1h`, `24h`, `30d`) in alpha-health metrics.

## Error and critical-state handling

- `error` state: input/contract-level failures that should be corrected and retried.
- `critical` state: unavailable upstream dependencies or fail-closed contract breaks requiring service restoration before operation.
- Card surfaces machine-readable diagnostics (`error_code`, `action`, `endpoint`, `correlation_id`) for incident traceability.

## Next-action playbooks

1. If card is **blocked**, resolve all listed `missingArtifacts[]` entries in order shown.
2. If card is **ready**, continue governed promotion/deallocation review workflows.
3. If card is **critical**, pause lifecycle actions and validate dependency health before proceeding.

## Cross-runbook links

1. Story 6.5 promotion lifecycle governance: `docs/operations/alpha-promotion-lifecycle-governance.md`
2. Story 6.6 counterfactual replay stress gating: `docs/operations/alpha-counterfactual-replay-stress-gating.md`
3. Story 6.7 live alpha health monitoring: `docs/operations/alpha-live-health-monitoring-threshold-breaches.md`
4. Story 6.3 validation workflow and diagnostics: `docs/operations/alpha-validation-workflow-and-diagnostics.md`
5. Story 6.4 shadow-mode evaluation: `docs/operations/alpha-shadow-mode-evaluation.md`
