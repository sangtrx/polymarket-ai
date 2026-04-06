CREATE TABLE IF NOT EXISTS attribution_snapshots (
    snapshot_id TEXT PRIMARY KEY,
    period_scope TEXT NOT NULL,
    period_start_utc TIMESTAMPTZ NOT NULL,
    period_end_utc TIMESTAMPTZ NOT NULL,
    market_id TEXT NOT NULL,
    alpha_id TEXT NOT NULL,
    realized_pnl_usd DOUBLE PRECISION NOT NULL,
    unrealized_pnl_usd DOUBLE PRECISION NOT NULL,
    fees_usd DOUBLE PRECISION NOT NULL DEFAULT 0,
    rebates_usd DOUBLE PRECISION NOT NULL DEFAULT 0,
    incentives_usd DOUBLE PRECISION NOT NULL DEFAULT 0,
    gross_pnl_usd DOUBLE PRECISION NOT NULL,
    net_pnl_usd DOUBLE PRECISION NOT NULL,
    reason_code TEXT NOT NULL,
    as_of_utc TIMESTAMPTZ NOT NULL,
    source TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    run_id TEXT NULL,
    upstream_snapshot_id TEXT NULL,
    evidence JSONB NOT NULL DEFAULT '{}'::jsonb,
    recorded_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (snapshot_id = lower(trim(snapshot_id))),
    CHECK (char_length(trim(snapshot_id)) > 0),
    CHECK (period_scope IN ('1h', '24h', '30d')),
    CHECK (char_length(trim(market_id)) > 0),
    CHECK (char_length(trim(alpha_id)) > 0),
    CHECK (char_length(trim(reason_code)) > 0),
    CHECK (char_length(trim(source)) > 0),
    CHECK (char_length(trim(correlation_id)) > 0),
    CHECK (run_id IS NULL OR (run_id = lower(trim(run_id)) AND char_length(trim(run_id)) > 0)),
    CHECK (
        upstream_snapshot_id IS NULL
        OR (
            upstream_snapshot_id = lower(trim(upstream_snapshot_id))
            AND char_length(trim(upstream_snapshot_id)) > 0
        )
    ),
    CHECK (
        reason_code IN (
            'attribution_ready',
            'attribution_empty_window',
            'attribution_invalid_payload',
            'attribution_unauthorized',
            'attribution_projection_unavailable',
            'attribution_stale_source',
            'attribution_persistence_unavailable'
        )
    ),
    CHECK (isfinite(realized_pnl_usd)),
    CHECK (isfinite(unrealized_pnl_usd)),
    CHECK (isfinite(fees_usd)),
    CHECK (isfinite(rebates_usd)),
    CHECK (isfinite(incentives_usd)),
    CHECK (isfinite(gross_pnl_usd)),
    CHECK (isfinite(net_pnl_usd)),
    CHECK (abs(gross_pnl_usd - (realized_pnl_usd + unrealized_pnl_usd)) <= 1e-9),
    CHECK (abs(net_pnl_usd - (gross_pnl_usd - (fees_usd - rebates_usd - incentives_usd))) <= 1e-9),
    CHECK (period_end_utc > period_start_utc),
    CHECK (as_of_utc >= period_end_utc),
    CHECK (EXTRACT(TIMEZONE FROM period_start_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM period_end_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM as_of_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM recorded_at_utc) = 0),
    CHECK (jsonb_typeof(evidence) = 'object'),
    UNIQUE (period_scope, period_end_utc, market_id, alpha_id, correlation_id)
);

CREATE INDEX IF NOT EXISTS idx_attribution_snapshots_latest_scope
    ON attribution_snapshots (period_scope, period_end_utc DESC, market_id ASC, alpha_id ASC, snapshot_id ASC);

CREATE INDEX IF NOT EXISTS idx_attribution_snapshots_market_alpha_period
    ON attribution_snapshots (market_id, alpha_id, period_end_utc DESC, snapshot_id ASC);

CREATE INDEX IF NOT EXISTS idx_attribution_snapshots_correlation_time
    ON attribution_snapshots (correlation_id, as_of_utc DESC);
