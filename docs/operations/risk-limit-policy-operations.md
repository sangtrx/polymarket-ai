# Risk Limit Policy Operations Guide (Story 2.7)

## Scope

This runbook governs control-plane mutation and verification for portfolio, market, and strategy risk-limit policy profiles plus inventory rules.

Authenticated endpoints:

1. `POST /control/risk-limits/profiles/{profile_key}`
2. `GET /control/risk-limits/pending`

## Scope definitions and unit semantics

`POST /control/risk-limits/profiles/{profile_key}` accepts a full scope bundle:

1. `portfolio_scope_id`, `market_scope_id`, `strategy_scope_id` (canonical lower-case identifiers).
2. Notional limits (`*_max_notional_usd`) in USD; values must be finite and `>= 0`.
3. Inventory limits (`*_max_inventory_units`) in position units; values must be finite and `>= 0`.
4. Concentration limits (`*_max_concentration_pct_nav`) in NAV percent; inclusive range `[0, 100]`.
5. `inventory_rules[]` for market/strategy scopes only:
   - `scope` (`market` or `strategy`)
   - `scope_id`
   - `max_position_units` (`>= 0`)
   - `max_order_size_units` (`>= 0`)
   - `max_concentration_pct_nav` (`[0, 100]`)

Cross-scope invariants are strict and deterministic:

1. Equality boundary is allowed (`portfolio == market`, `market == strategy`).
2. Strict child-over-parent increase is rejected (`market > portfolio`, `strategy > market`).

## Approval-gated critical increase workflow

Critical increase detection is fail-closed:

1. If proposed limits strictly increase versus active policy and no valid approval evidence is provided, mutation is persisted as `pending`.
2. Pending mutations return status `pending` with `reason_code = risk_limit_approval_required`.
3. Activation requires dual-approval evidence (`risk_limit_increase`) and returns `active` with a non-null `approval_reference`.
4. Denied approval paths do not emit synthetic approval references.

## Failure-mode playbooks

1. **Invalid invariant payload** (`risk_limit_invalid_payload`, HTTP 400)  
   Remediation: correct returned `field_errors[]`; typical violations are cross-scope ordering or non-finite numeric values.
2. **Missing approval evidence for critical increase** (`risk_limit_approval_required`, HTTP 202 pending)  
   Remediation: complete dual-approval workflow for `risk_limit_increase`, then resubmit with validated approval evidence.
3. **Persistence/runtime dependency unavailable** (`risk_limit_persistence_unavailable`, HTTP 503)  
   Remediation: hold fail-closed posture, escalate platform on-call, and retry once persistence health is restored.

## Operator verification steps

1. Submit profile mutation and confirm response includes actor, reason code, correlation ID, and approval status (`active` or `pending`).
2. Query `GET /control/risk-limits/pending` and verify pending profiles include:
   - actor ID,
   - action type (`risk_limit_profile_update`),
   - approval status,
   - reason code,
   - correlation metadata.
3. For activated critical increases, confirm immutable audit entries include matching `approval_reference`.
4. Correlate control response `correlation_id` with audit retrieval paths before closing incident or change request.

## Cross-runbook links

1. FR41 participation guardrails and overnight cap operations: `docs/operations/low-liquidity-overnight-guardrails.md`
2. Pre-trade pipeline sequencing and reason-code matrix: `docs/operations/pretrade-gate-pipeline.md`
