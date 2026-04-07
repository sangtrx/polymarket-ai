# Alpha Live Health Monitoring and Threshold Breach Operations Guide (Story 6.7 / FR10 / FR47 / NFR14 / NFR15)

This runbook covers Story 6.7 live alpha health telemetry ingestion, deterministic threshold evaluation, breach persistence, and operator-alert escalation.

## Scope and boundaries

- Dependency surfaces:
  - Story 6.4 shadow-evaluation evidence.
  - Story 6.6 counterfactual replay evidence.
  - Story 6.5 promotion decision evidence.
- Schema scope in Story 6.7 is limited to:
  - `alpha_health_metrics`
  - `alpha_threshold_breaches`
- Out of scope: Story 6.8 governance-card UX and Story 6.9 automatic deallocation actions.

## Control-plane routes

1. `POST /control/research/alpha-health-metrics`
2. `GET /control/research/alpha-health-metrics/{metric_id}`
3. `GET /control/research/alpha-health-metrics?alpha_id={alpha_id}&limit={n}`
4. `GET /control/research/alpha-threshold-breaches/{breach_id}`
5. `GET /control/research/alpha-threshold-breaches?alpha_id={alpha_id}&limit={n}`

All responses use canonical `data/meta/error` envelopes.

## FR10 telemetry and attribution window contract

Each metric record persists:

1. `rolling_sharpe`
2. `rolling_hit_rate`
3. `rolling_drawdown`
4. `stability_score`

Each record must also include all required windows:

1. `1h`
2. `24h`
3. `30d`

Window payloads must include `net_pnl`, `rolling_sharpe`, `rolling_hit_rate`, `rolling_drawdown`, and `stability_score`.

## Deterministic threshold semantics (FR47 continuity)

- Floor metrics breach on strict less-than:
  - `rolling_sharpe < threshold_value`
  - `rolling_hit_rate < threshold_value`
  - `stability_score < threshold_value`
- Ceiling metrics breach on strict greater-than:
  - `rolling_drawdown > threshold_value`
- Boundary behavior: equality follows allow-path.

## Breach record and alert contract

Each persisted breach includes:

1. `alpha_id`
2. `metric_key`
3. `observed_value`
4. `threshold_value`
5. `comparator`
6. `breach_reason`
7. canonical UTC timestamps and correlation evidence

Breach alerts reuse the incident-alert contract and include:

- severity
- impacted_subsystem
- cause
- recommended_next_action
- evidence_link
- reason_code (`alert_alpha_health_threshold_breach`)
- correlation_id

## Fail-closed status and reason mapping

| Status | Error code families |
| --- | --- |
| `400` | `alpha_health_invalid_payload` |
| `403` | `alpha_health_unauthorized_role` |
| `409` | `alpha_health_metric_not_found`, `alpha_health_breach_not_found`, constraint violations |
| `503` | `alpha_health_dependency_unavailable`, `alpha_health_state_unavailable`, `alpha_health_persistence_unavailable`, query/decode unavailable paths |
| `500` | uncategorized internal failures |

## Incident and observability continuity

Allow and deny paths preserve:

1. alpha id / metric id / breach id
2. metric key
3. reason code
4. actor id / role
5. correlation id
6. UTC timestamp

Unauthorized security-signal names:

1. `unauthorized_alpha_health_read_attempt_v1`
2. `unauthorized_alpha_health_mutation_attempt_v1`

## Operator triage workflow

1. Confirm breach evidence from `GET /control/research/alpha-threshold-breaches/{breach_id}`.
2. Validate latest health trend using `GET /control/research/alpha-health-metrics?alpha_id={alpha_id}&limit=20`.
3. Cross-check upstream seams (shadow/replay/promotion) before escalation:
   - verify latest shadow-readiness evidence,
   - verify latest replay gate outcome,
   - verify latest promotion decision state.
4. Follow incident response playbook in `docs/operations/severity-alert-delivery.md`.

## Cross-runbook links

1. Shadow-mode evaluation (Story 6.4): `docs/operations/alpha-shadow-mode-evaluation.md`
2. Counterfactual replay stress gating (Story 6.6): `docs/operations/alpha-counterfactual-replay-stress-gating.md`
3. Promotion lifecycle governance (Story 6.5): `docs/operations/alpha-promotion-lifecycle-governance.md`
4. Validation workflow and diagnostics (Story 6.3): `docs/operations/alpha-validation-workflow-and-diagnostics.md`
5. Severity alert delivery: `docs/operations/severity-alert-delivery.md`
