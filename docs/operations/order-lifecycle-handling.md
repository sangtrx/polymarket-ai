# Order Lifecycle Operations Guide (Story 2.5)

## Scope

This guide covers venue-compatible lifecycle handling for order submission, cancellation, and batch-cancel execution:

1. Canonical order projection in `orders`.
2. Append-only lifecycle evidence in `order_state_transitions`.
3. Deterministic terminal boundaries and idempotent retry behavior.
4. Per-order batch-cancel outcomes with machine-readable reason codes.

## Canonical state model

Canonical lifecycle states:

1. `pending`
2. `live`
3. `partially_filled`
4. `filled` (terminal)
5. `canceled` (terminal)
6. `expired` (terminal)

Deterministic transition guardrails:

1. `pending -> live|canceled|expired`
2. `live -> partially_filled|filled|canceled|expired`
3. `partially_filled -> partially_filled|filled|canceled|expired`
4. Terminal states are immutable (no terminal-to-nonterminal rollback).

## Cancel semantics and batch-cancel behavior

Single-order cancel:

1. Uses normalized idempotency key (`trim + lowercase`).
2. Emits explicit machine-readable result through lifecycle telemetry.
3. Never mutates canonical state on invalid transitions.

Batch-cancel is evaluated per order and returns one of:

1. `canceled`
2. `already_terminal`
3. `retryable_failure`
4. `hard_failure`

Every per-order result includes:

1. `correlation_id`
2. `order_id`
3. `market_id`
4. `reason_code`
5. `timestamp_utc`
6. `idempotency_key`

## Idempotency and replay safety

Deterministic idempotency boundaries:

1. Duplicate keys do not append new transitions.
2. Duplicate/out-of-order lifecycle updates do not mutate canonical order state.
3. `order_state_transitions` enforces strict per-order sequence monotonicity.

Retry guidance:

1. Reuse the same idempotency key for retried submit/cancel requests.
2. For batch-cancel retries, reuse the same `batch_idempotency_key` to preserve deterministic per-order outcomes.
3. Treat `retryable_failure` as retriable only after dependency health is restored.

## Evidence and telemetry contract

Lifecycle transition and batch-cancel evidence includes:

1. `event_id`
2. `correlation_id`
3. `order_id`
4. `market_id`
5. `from_state`
6. `to_state`
7. `reason_code`
8. `timestamp_utc`

This keeps signal -> order -> transition timelines machine-queryable for incident analysis.

## Incident response runbook

### 1) Invalid transition alerts

1. Query latest `orders` row and ordered `order_state_transitions` for the affected `order_id`.
2. Confirm attempted transition violated canonical transition rules.
3. Verify canonical state remained unchanged after rejection.
4. Investigate upstream venue/update ordering anomalies.

### 2) Persistence outage

1. Validate database connectivity and transaction health.
2. Check for constraint-trigger failures (sequence monotonicity, enum/reason-code checks, timestamp UTC checks).
3. Treat uncertain persistence as fail-closed (`order_lifecycle_persistence_unavailable`).
4. Resume normal operation only after write-path and replay consistency are verified.

### 3) Venue mapping mismatch

1. Confirm incoming venue/user-stream lifecycle status was mapped to the expected canonical state.
2. If status is unsupported or ambiguous, keep reject path (`order_lifecycle_unsupported_venue_state`) and escalate.
3. Do not introduce ad-hoc state mappings during incident response; update contracts first.
