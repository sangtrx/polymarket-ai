# User Stream Ingestion Operations Guide (Story 2.3)

## Scope

This guide covers authenticated user-order/trade ingest runtime behavior:

1. Canonical user event persistence (`user_stream_events`)
2. Deterministic ordering/auth cursor persistence (`order_event_offsets`)
3. Fail-closed auth-state transitions that gate new intents while auth is uncertain

## Deterministic ordering rules

The runtime enforces ordering with per-partition cursors in `order_event_offsets`:

1. `incoming_offset > last_offset` -> accepted
2. `incoming_offset == last_offset` and same normalized idempotency key -> duplicate (ignored)
3. `incoming_offset == last_offset` with different key -> deterministic lexical tie-break
4. `incoming_offset < last_offset` -> out-of-order (ignored)

`order_event_offsets` includes monotonic offset trigger enforcement and must never regress.

## Auth-expiry fail-safe behavior

Auth-state transitions are persisted with machine-readable reason codes:

1. `user_stream_auth_expired` -> `block_new_intents = true`
2. `user_stream_auth_recovered_pending_event` -> still blocked until first post-recovery event persists
3. `user_stream_authenticated` -> block cleared only after first successful post-recovery event persistence

Risk intent gates must deny while auth block is active.

## Latency and telemetry evidence

Operational targets and evidence requirements:

1. 99th percentile user-event persistence latency `<= 2.0s`
2. Structured telemetry for:
   - accepted events,
   - duplicate suppression,
   - out-of-order rejection,
   - auth-state transitions,
   - persistence-path failures.

Telemetry must include reason codes, UTC timestamp fields, and correlation identifiers.

## Alert conditions

Trigger alerts on:

1. sustained `user_stream_auth_expired` state,
2. repeated reconnect/disconnect failures (`user_stream_disconnected`),
3. elevated duplicate/out-of-order rates (ordering drift signals),
4. persistence outages (`user_stream_persistence_unavailable`),
5. user-stream latency SLO degradation (`user_stream_latency_slo_breached`).

## Runbook: auth-expiry and reconnect incidents

1. Validate API key/secret/passphrase/address configuration and key rotation status.
2. Confirm authenticated user websocket endpoint health and TLS reachability.
3. Keep fail-closed posture while `block_new_intents = true`.
4. Verify transition sequence persisted in `order_event_offsets`:
   - expired -> recovered_pending -> authenticated.
5. Confirm first post-recovery event persistence before manually treating intent flow as healthy.

## Runbook: duplicate/out-of-order spikes

1. Query recent `user_stream_events` by partition and offset progression.
2. Inspect cursor state in `order_event_offsets` for non-regressing monotonic offsets.
3. Confirm idempotency-key normalization remains stable (`trim + lowercase`).
4. Escalate upstream feed anomalies if offset discontinuity is external.

## Runbook: persistence-path failures

1. Validate Postgres connectivity, credentials, and write availability.
2. Check for constraint violations on status/reason/timestamp/offset fields.
3. Treat uncertain persistence as fail-closed; do not clear auth blocks prematurely.
4. Reconcile event/cursor continuity before resuming normal operations.
