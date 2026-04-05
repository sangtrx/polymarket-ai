CREATE TABLE IF NOT EXISTS user_stream_events (
    event_id TEXT PRIMARY KEY,
    event_kind TEXT NOT NULL,
    event_status TEXT NOT NULL,
    market_id TEXT NOT NULL,
    asset_id TEXT NOT NULL,
    order_id TEXT NOT NULL,
    trade_id TEXT,
    partition_key TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    event_offset BIGINT NOT NULL,
    reason_code TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    event_timestamp_utc TIMESTAMPTZ NOT NULL,
    observed_at_utc TIMESTAMPTZ NOT NULL,
    ingested_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ingestion_latency_seconds DOUBLE PRECISION NOT NULL,
    raw_payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    CHECK (event_kind IN ('order', 'trade')),
    CHECK (
        event_status IN (
            'placement',
            'update',
            'cancellation',
            'matched',
            'mined',
            'confirmed'
        )
    ),
    CHECK (
        reason_code IN (
            'user_stream_event_accepted',
            'user_stream_duplicate_event',
            'user_stream_out_of_order_event',
            'user_stream_equal_offset_tie_break_accepted',
            'user_stream_equal_offset_tie_break_rejected',
            'user_stream_auth_expired',
            'user_stream_auth_recovered_pending_event',
            'user_stream_authenticated',
            'user_stream_persistence_unavailable',
            'user_stream_disconnected',
            'user_stream_invalid_payload',
            'user_stream_latency_slo_breached'
        )
    ),
    CHECK (event_offset >= 0),
    CHECK (ingestion_latency_seconds >= 0),
    CHECK (char_length(trim(event_id)) > 0),
    CHECK (char_length(trim(market_id)) > 0),
    CHECK (char_length(trim(asset_id)) > 0),
    CHECK (char_length(trim(order_id)) > 0),
    CHECK (trade_id IS NULL OR char_length(trim(trade_id)) > 0),
    CHECK (char_length(trim(partition_key)) > 0),
    CHECK (partition_key = lower(trim(partition_key))),
    CHECK (char_length(trim(idempotency_key)) > 0),
    CHECK (idempotency_key = lower(trim(idempotency_key))),
    CHECK (char_length(trim(correlation_id)) > 0),
    CHECK (EXTRACT(TIMEZONE FROM event_timestamp_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM observed_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM ingested_at_utc) = 0),
    CHECK (observed_at_utc >= event_timestamp_utc),
    CHECK (ingested_at_utc >= observed_at_utc),
    CHECK (
        (
            event_kind = 'trade'
            AND char_length(trim(coalesce(trade_id, ''))) > 0
        )
        OR event_kind = 'order'
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_user_stream_events_partition_offset_key
    ON user_stream_events (partition_key, event_offset, idempotency_key);

CREATE INDEX IF NOT EXISTS idx_user_stream_events_order_time
    ON user_stream_events (order_id, observed_at_utc DESC);

CREATE INDEX IF NOT EXISTS idx_user_stream_events_market_time
    ON user_stream_events (market_id, observed_at_utc DESC);

CREATE INDEX IF NOT EXISTS idx_user_stream_events_offset_lookup
    ON user_stream_events (partition_key, event_offset DESC);

CREATE TABLE IF NOT EXISTS order_event_offsets (
    partition_key TEXT PRIMARY KEY,
    last_event_id TEXT NOT NULL,
    last_event_key TEXT NOT NULL,
    last_event_offset BIGINT NOT NULL,
    auth_state TEXT NOT NULL,
    block_new_intents BOOLEAN NOT NULL DEFAULT FALSE,
    reason_code TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    updated_at_utc TIMESTAMPTZ NOT NULL,
    recorded_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (char_length(trim(partition_key)) > 0),
    CHECK (partition_key = lower(trim(partition_key))),
    CHECK (char_length(trim(last_event_id)) > 0),
    CHECK (char_length(trim(last_event_key)) > 0),
    CHECK (char_length(trim(reason_code)) > 0),
    CHECK (char_length(trim(correlation_id)) > 0),
    CHECK (last_event_offset >= 0),
    CHECK (auth_state IN ('authenticated', 'auth_expired')),
    CHECK (
        reason_code IN (
            'user_stream_event_accepted',
            'user_stream_duplicate_event',
            'user_stream_out_of_order_event',
            'user_stream_equal_offset_tie_break_accepted',
            'user_stream_equal_offset_tie_break_rejected',
            'user_stream_auth_expired',
            'user_stream_auth_recovered_pending_event',
            'user_stream_authenticated',
            'user_stream_persistence_unavailable',
            'user_stream_disconnected',
            'user_stream_invalid_payload',
            'user_stream_latency_slo_breached'
        )
    ),
    CHECK (
        (auth_state = 'auth_expired' AND block_new_intents = TRUE)
        OR auth_state = 'authenticated'
    ),
    CHECK (EXTRACT(TIMEZONE FROM updated_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM recorded_at_utc) = 0)
);

CREATE INDEX IF NOT EXISTS idx_order_event_offsets_updated
    ON order_event_offsets (updated_at_utc DESC);

CREATE INDEX IF NOT EXISTS idx_order_event_offsets_blocking_state
    ON order_event_offsets (block_new_intents, updated_at_utc DESC);

CREATE OR REPLACE FUNCTION enforce_order_event_offset_monotonicity()
RETURNS trigger AS $$
BEGIN
    IF NEW.last_event_offset < OLD.last_event_offset THEN
        RAISE EXCEPTION
            'order_event_offsets.last_event_offset must be monotonic (old %, new %)',
            OLD.last_event_offset,
            NEW.last_event_offset;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_order_event_offsets_monotonicity ON order_event_offsets;

CREATE TRIGGER trg_order_event_offsets_monotonicity
BEFORE UPDATE ON order_event_offsets
FOR EACH ROW
EXECUTE FUNCTION enforce_order_event_offset_monotonicity();
