# Reconciliation and Exposure Operations Runbook

## Purpose

This runbook covers deterministic reconciliation execution, mismatch triage, safe-state handling, and exposure visibility verification for Story 2.6.

## Reconciliation Threshold Policy

- Critical mismatch threshold is strict: **`mismatch_rate > 0.1%`** (`> 0.001`).
- Boundary-safe runs (`<= 0.1%`) do **not** trigger reconciliation halt.
- Critical runs trigger safe-state deny behavior for new order intents via machine-readable reason code:
  - `reconciliation_critical_mismatch`

## Incident Evidence Queries

Use explicit read paths (no manual log stitching):

1. Reconciliation run summary by `run_id`
2. Reconciliation diffs by `run_id` (deterministic order: `order_id ASC, diff_class ASC`)
3. Latest exposure snapshot (global or market-scoped)

Evidence records include UTC timestamps and `correlation_id` for incident timeline composition.

## Mismatch Triage Procedure

1. Load reconciliation run summary and confirm:
   - `mismatch_count`
   - `mismatch_rate`
   - `critical_halt`
   - `reason_code`
2. Load reconciliation diffs and group by `diff_class`:
   - `missing_internal_record`
   - `missing_venue_record`
   - `market_mismatch`
   - `lifecycle_state_mismatch`
   - `quantity_mismatch`
   - `price_mismatch`
3. Validate correlation linkage across run, diff, and snapshot evidence.
4. If critical halt is active, keep deny-path in place until deterministic follow-up run confirms recovery.

## Halt-State Handling

- During reconciliation halt, new order intents remain blocked.
- Exposure snapshot read paths remain available for operators.
- Do **not** bypass reconciliation deny controls manually.

## Recovery Expectations

1. Resolve upstream mismatch source.
2. Re-run reconciliation on a deterministic internal/venue window.
3. Confirm resulting run has:
   - `critical_halt = false`
   - threshold-safe mismatch rate
4. Confirm latest exposure snapshot updated with fresh timestamp/correlation metadata.

## Failure-Mode Playbooks

### Venue Unavailable

- Reason code: `reconciliation_venue_unavailable`
- Action:
  1. Keep fail-closed posture for reconciliation certainty.
  2. Verify venue connectivity and authentication.
  3. Retry reconciliation once venue truth feed is healthy.

### Persistence Unavailable

- Reason code: `reconciliation_persistence_unavailable`
- Action:
  1. Validate Postgres connectivity, migrations, and transaction health.
  2. Confirm writes for `reconciliation_runs`, `reconciliation_diffs`, `exposure_snapshots`.
  3. Re-run reconciliation only after persistence path is healthy.

### State Hydration Failure

- Reason code: `reconciliation_state_hydration_failed`
- Action:
  1. Validate internal lifecycle data availability from `orders` and `order_state_transitions`.
  2. Confirm reconciliation window bounds are valid and UTC-normalized.
  3. Retry once lifecycle hydration succeeds.
