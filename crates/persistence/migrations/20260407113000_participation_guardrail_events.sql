CREATE TABLE IF NOT EXISTS participation_guardrail_events (
    event_id TEXT PRIMARY KEY,
    guardrail_mode TEXT NOT NULL,
    reason_code TEXT NOT NULL,
    market_id TEXT NOT NULL,
    cluster_id TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    observed_at_utc TIMESTAMPTZ NOT NULL,
    evaluated_at_utc TIMESTAMPTZ NOT NULL,
    liquidity_depth_usd DOUBLE PRECISION NOT NULL,
    inactivity_gap_seconds DOUBLE PRECISION NOT NULL,
    threshold_liquidity_depth_usd DOUBLE PRECISION NOT NULL,
    threshold_inactivity_pause_seconds DOUBLE PRECISION NOT NULL,
    threshold_overnight_gap_seconds DOUBLE PRECISION NOT NULL,
    normal_max_order_size_units DOUBLE PRECISION NULL,
    capped_max_order_size_units DOUBLE PRECISION NULL,
    created_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (evaluated_at_utc >= observed_at_utc),
    CHECK (event_id = lower(trim(event_id))),
    CHECK (market_id = lower(trim(market_id))),
    CHECK (cluster_id = lower(trim(cluster_id))),
    CHECK (correlation_id = lower(trim(correlation_id))),
    CHECK (char_length(trim(event_id)) > 0),
    CHECK (char_length(trim(market_id)) > 0),
    CHECK (char_length(trim(cluster_id)) > 0),
    CHECK (char_length(trim(correlation_id)) > 0),
    CHECK (char_length(trim(reason_code)) > 0),
    CHECK (guardrail_mode IN ('pass', 'pause', 'size_cap', 'unavailable')),
    CHECK (isfinite(liquidity_depth_usd) AND liquidity_depth_usd >= 0),
    CHECK (isfinite(inactivity_gap_seconds) AND inactivity_gap_seconds >= 0),
    CHECK (isfinite(threshold_liquidity_depth_usd) AND threshold_liquidity_depth_usd >= 0),
    CHECK (
        isfinite(threshold_inactivity_pause_seconds)
        AND threshold_inactivity_pause_seconds >= 0
    ),
    CHECK (isfinite(threshold_overnight_gap_seconds) AND threshold_overnight_gap_seconds >= 0),
    CHECK (
        normal_max_order_size_units IS NULL
        OR (isfinite(normal_max_order_size_units) AND normal_max_order_size_units > 0)
    ),
    CHECK (
        capped_max_order_size_units IS NULL
        OR (isfinite(capped_max_order_size_units) AND capped_max_order_size_units > 0)
    ),
    CHECK (
        (guardrail_mode = 'size_cap')
        = (
            normal_max_order_size_units IS NOT NULL
            AND capped_max_order_size_units IS NOT NULL
        )
    )
);

CREATE INDEX IF NOT EXISTS idx_participation_guardrail_events_market_observed_at
    ON participation_guardrail_events (market_id, observed_at_utc DESC, event_id ASC);

CREATE INDEX IF NOT EXISTS idx_participation_guardrail_events_reason_observed_at
    ON participation_guardrail_events (reason_code, observed_at_utc DESC, event_id ASC);

CREATE INDEX IF NOT EXISTS idx_participation_guardrail_events_correlation_observed_at
    ON participation_guardrail_events (correlation_id, observed_at_utc DESC, event_id ASC);
