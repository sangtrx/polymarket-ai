# Data Freshness Gating Operations Guide (Story 2.4)

## Scope

This guide covers stale-feed safety controls used by execution and risk layers:

1. Market + user stream freshness age checks.
2. Automatic stale-feed pause activation for new order creation.
3. Stability-window recovery with explicit confirmation evidence.
4. Durable freshness-gate transition evidence in `freshness_gate_events`.

## Freshness thresholds and deterministic boundaries

Runtime policy evaluates both stream ages on every freshness check:

1. `market_data_age_seconds > 30` -> stale breach.
2. `user_data_age_seconds > 30` -> stale breach.
3. Missing market/user freshness signal -> fail-closed (`freshness_gate_state_unavailable`).

Boundary behavior is deterministic:

1. `29s` -> safe (not stale).
2. `30s` -> safe (not stale).
3. `>30s` -> stale pause activation or maintenance.

## Pause and recovery transition model

Persisted transitions (`freshness_gate_events.transition`):

1. `pause_activated` -> stale or unavailable data enters fail-closed pause.
2. `pause_maintained` -> stale condition continues.
3. `recovery_pending` -> data is fresh again, but stability window is still running.
4. `recovery_confirmed` -> full stability window elapsed; pause clears.

Reason codes remain machine-readable and deterministic:

1. `freshness_gate_stale_breach`
2. `freshness_gate_state_unavailable`
3. `freshness_gate_recovery_pending`
4. `freshness_gate_recovery_confirmed`
5. `freshness_gate_boundary_safe` (evaluation outcome when no transition is emitted)

## NFR5 transition latency evidence

For stale breach activation, evidence must support machine verification that breach-to-pause is `<=5s`:

1. `stale_breach_detected_at_utc`
2. `pause_activated_at_utc`
3. `breach_to_pause_latency_seconds` (bounded by policy max)

## Alert conditions

Trigger incident alerts on:

1. sustained `pause_maintained` sequences,
2. repeated `freshness_gate_state_unavailable`,
3. recovery that stays in `recovery_pending` beyond expected duration,
4. `freshness_gate_persistence_unavailable`,
5. any `freshness_gate_evaluation_error`.

## Incident runbook

### 1) Stale breach active

1. Confirm latest market/user update timestamps and computed ages.
2. Validate websocket connectivity and auth state for both streams.
3. Keep fail-closed posture while pause is active.
4. Verify new-order intents are denied with freshness reason codes.

### 2) State unavailable (missing input)

1. Identify which stream freshness signal is missing.
2. Validate runtime health and ingestion telemetry for that stream.
3. Confirm persistence writes for freshness events are healthy.
4. Do not manually clear pause until deterministic fresh signals are restored.

### 3) Recovery monitoring

1. Confirm stream ages remain `<=30s` continuously.
2. Track `recovery_pending` start timestamp.
3. Verify `recovery_confirmed` occurs only at/after stability-window boundary.
4. Confirm risk gate decisions return to normal allow path only after confirmation.
