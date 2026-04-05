# Market Stream Ingestion Operations Guide (Story 2.2)

## Scope

This guide covers market-data ingest runtime behavior for the execution engine:

1. Canonical market tick persistence (`market_ticks`)
2. Stream-health transition persistence (`market_stream_health`)
3. Quarantine handling for malformed or unsupported payloads

## Stream-health states

`market_stream_health.health_status` is deterministic with machine-readable reason codes:

1. `healthy`
   - `market_stream_healthy` when backlog is below warning threshold.
   - `market_stream_backlog_warning` when backlog is `>= 5s` and `<= 10s`.
2. `degraded`
   - `market_stream_backlog_exceeded` when backlog is `> 10s`.
   - `market_stream_backlog_sustained_exceeded` when backlog is `> 10s` for `> 30s`.
   - `market_stream_heartbeat_timeout` when heartbeat gap exceeds timeout.
   - `market_stream_disconnected` when websocket connectivity drops.

## Backlog and latency boundaries

Operational boundaries:

1. Normal ingestion SLO target: 99th percentile ingest latency `<= 2.0s`
2. Backlog warning threshold: `5.0s`
3. Degrade trigger threshold: `> 10.0s`
4. Sustained degrade gate: backlog `> 10.0s` for `> 30.0s`

At `10.0s` exactly, runtime remains deterministic and non-ambiguous (warning path, not sustained degrade).

## Quarantine behavior

Malformed or unsupported payloads are fail-closed and quarantined:

1. Runtime does not crash.
2. Quarantined payload evidence is persisted into `market_ticks` with `ingest_status = 'quarantined'`.
3. Machine-readable reason codes are attached (`market_stream_invalid_payload`, `market_stream_unsupported_event_shape`, etc.).
4. Valid events continue processing after quarantine.

## Alert conditions

Trigger alerts when any of the following occurs:

1. `market_stream_health.health_status = degraded`
2. Sustained backlog degradation (`market_stream_backlog_sustained_exceeded`)
3. Repeated disconnects (`market_stream_disconnected`)
4. Persistent malformed spikes (`market_stream_quarantined_payload` / invalid payload reasons)
5. Persistence-path failures (`market_stream_persistence_unavailable`)

## Remediation runbook

### 1) Disconnect storm / heartbeat failures

1. Confirm endpoint reachability and DNS/TLS health for market websocket endpoint.
2. Verify execution-engine egress/network ACL changes.
3. Keep fail-closed posture while degraded reasons persist.
4. Resume only after healthy transitions are observed in `market_stream_health`.

### 2) Malformed event spike

1. Query recent quarantined records by correlation/time.
2. Validate payload shape drift against canonical contract requirements.
3. Coordinate with upstream feed provider if schema drift is confirmed.
4. Maintain quarantine path until payloads validate again.

### 3) Persistence outage

1. Verify Postgres connectivity, credentials, and capacity.
2. Check for constraint violations and storage pressure.
3. Treat missing durable writes as degraded/fail-closed; do not mark stream healthy.
4. Reconcile `market_ticks`/`market_stream_health` evidence after restoration.
