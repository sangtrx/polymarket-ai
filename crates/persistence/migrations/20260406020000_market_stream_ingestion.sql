CREATE TABLE IF NOT EXISTS market_ticks (
    tick_id TEXT PRIMARY KEY,
    market_id TEXT,
    asset_id TEXT,
    cluster_id TEXT,
    best_bid DOUBLE PRECISION,
    best_ask DOUBLE PRECISION,
    top_bid_depth JSONB NOT NULL DEFAULT '[]'::jsonb,
    top_ask_depth JSONB NOT NULL DEFAULT '[]'::jsonb,
    last_trade_price DOUBLE PRECISION,
    tick_size DOUBLE PRECISION,
    market_status TEXT,
    ingest_status TEXT NOT NULL,
    quarantine_reason_code TEXT,
    raw_payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    ingestion_latency_seconds DOUBLE PRECISION NOT NULL,
    correlation_id TEXT NOT NULL,
    observed_at_utc TIMESTAMPTZ NOT NULL,
    ingested_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (ingest_status IN ('accepted', 'quarantined')),
    CHECK (market_status IS NULL OR market_status IN ('trading', 'halted', 'closed')),
    CHECK (best_bid IS NULL OR best_bid >= 0),
    CHECK (best_ask IS NULL OR best_ask >= 0),
    CHECK (last_trade_price IS NULL OR last_trade_price >= 0),
    CHECK (tick_size IS NULL OR tick_size > 0),
    CHECK (ingestion_latency_seconds >= 0),
    CHECK (char_length(trim(tick_id)) > 0),
    CHECK (market_id IS NULL OR char_length(trim(market_id)) > 0),
    CHECK (asset_id IS NULL OR char_length(trim(asset_id)) > 0),
    CHECK (cluster_id IS NULL OR cluster_id = lower(trim(cluster_id))),
    CHECK (cluster_id IS NULL OR char_length(trim(cluster_id)) > 0),
    CHECK (char_length(trim(correlation_id)) > 0),
    CHECK (EXTRACT(TIMEZONE FROM observed_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM ingested_at_utc) = 0),
    CHECK (
        (
            ingest_status = 'accepted'
            AND market_id IS NOT NULL
            AND asset_id IS NOT NULL
            AND cluster_id IS NOT NULL
            AND best_bid IS NOT NULL
            AND best_ask IS NOT NULL
            AND last_trade_price IS NOT NULL
            AND tick_size IS NOT NULL
            AND market_status IS NOT NULL
            AND quarantine_reason_code IS NULL
        )
        OR (
            ingest_status = 'quarantined'
            AND char_length(trim(coalesce(quarantine_reason_code, ''))) > 0
        )
    )
);

CREATE INDEX IF NOT EXISTS idx_market_ticks_market_time
    ON market_ticks (market_id, observed_at_utc DESC);

CREATE INDEX IF NOT EXISTS idx_market_ticks_cluster_time
    ON market_ticks (cluster_id, observed_at_utc DESC);

CREATE INDEX IF NOT EXISTS idx_market_ticks_ingest_status_time
    ON market_ticks (ingest_status, ingested_at_utc DESC);

CREATE INDEX IF NOT EXISTS idx_market_ticks_correlation_time
    ON market_ticks (correlation_id, ingested_at_utc DESC);

CREATE TABLE IF NOT EXISTS market_stream_health (
    health_event_id TEXT PRIMARY KEY,
    stream_name TEXT NOT NULL,
    health_status TEXT NOT NULL,
    reason_code TEXT NOT NULL,
    backlog_seconds DOUBLE PRECISION NOT NULL,
    sustained_backlog_seconds DOUBLE PRECISION NOT NULL,
    heartbeat_gap_seconds DOUBLE PRECISION NOT NULL,
    correlation_id TEXT NOT NULL,
    observed_at_utc TIMESTAMPTZ NOT NULL,
    recorded_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    evidence JSONB NOT NULL DEFAULT '{}'::jsonb,
    CHECK (health_status IN ('healthy', 'degraded')),
    CHECK (backlog_seconds >= 0),
    CHECK (sustained_backlog_seconds >= 0),
    CHECK (heartbeat_gap_seconds >= 0),
    CHECK (char_length(trim(health_event_id)) > 0),
    CHECK (char_length(trim(stream_name)) > 0),
    CHECK (char_length(trim(reason_code)) > 0),
    CHECK (char_length(trim(correlation_id)) > 0),
    CHECK (EXTRACT(TIMEZONE FROM observed_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM recorded_at_utc) = 0)
);

CREATE INDEX IF NOT EXISTS idx_market_stream_health_stream_time
    ON market_stream_health (stream_name, observed_at_utc DESC);

CREATE INDEX IF NOT EXISTS idx_market_stream_health_status_time
    ON market_stream_health (health_status, observed_at_utc DESC);

CREATE INDEX IF NOT EXISTS idx_market_stream_health_reason_time
    ON market_stream_health (reason_code, recorded_at_utc DESC);
