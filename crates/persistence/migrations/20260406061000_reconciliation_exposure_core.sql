CREATE TABLE IF NOT EXISTS reconciliation_runs (
    run_id TEXT PRIMARY KEY,
    window_started_at_utc TIMESTAMPTZ NOT NULL,
    window_ended_at_utc TIMESTAMPTZ NOT NULL,
    compared_records BIGINT NOT NULL,
    mismatch_count BIGINT NOT NULL,
    mismatch_rate DOUBLE PRECISION NOT NULL,
    critical_halt BOOLEAN NOT NULL,
    status TEXT NOT NULL,
    reason_code TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    evaluated_at_utc TIMESTAMPTZ NOT NULL,
    recorded_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    evidence JSONB NOT NULL DEFAULT '{}'::jsonb,
    CHECK (char_length(trim(run_id)) > 0),
    CHECK (run_id = lower(trim(run_id))),
    CHECK (char_length(trim(reason_code)) > 0),
    CHECK (char_length(trim(correlation_id)) > 0),
    CHECK (status IN ('succeeded', 'critical_halt', 'failed')),
    CHECK (
        reason_code IN (
            'reconciliation_matched',
            'reconciliation_non_critical_mismatch',
            'reconciliation_critical_mismatch',
            'reconciliation_window_unavailable',
            'reconciliation_venue_unavailable',
            'reconciliation_persistence_unavailable',
            'reconciliation_unauthorized',
            'reconciliation_state_hydration_failed',
            'reconciliation_invalid_payload'
        )
    ),
    CHECK (compared_records > 0),
    CHECK (mismatch_count >= 0),
    CHECK (mismatch_count <= compared_records),
    CHECK (mismatch_rate >= 0),
    CHECK (mismatch_rate <= 1),
    CHECK (
        ABS(mismatch_rate - (mismatch_count::DOUBLE PRECISION / compared_records::DOUBLE PRECISION))
        <= 0.000000000001
    ),
    CHECK (
        (critical_halt = TRUE AND mismatch_rate > 0.001 AND status = 'critical_halt')
        OR (critical_halt = FALSE AND status <> 'critical_halt')
    ),
    CHECK (window_ended_at_utc >= window_started_at_utc),
    CHECK (EXTRACT(TIMEZONE FROM window_started_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM window_ended_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM evaluated_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM recorded_at_utc) = 0),
    UNIQUE (correlation_id, window_started_at_utc, window_ended_at_utc)
);

CREATE INDEX IF NOT EXISTS idx_reconciliation_runs_window_lookup
    ON reconciliation_runs (window_started_at_utc DESC, window_ended_at_utc DESC);

CREATE INDEX IF NOT EXISTS idx_reconciliation_runs_correlation_time
    ON reconciliation_runs (correlation_id, evaluated_at_utc DESC);

CREATE INDEX IF NOT EXISTS idx_reconciliation_runs_reason_time
    ON reconciliation_runs (reason_code, evaluated_at_utc DESC);

CREATE TABLE IF NOT EXISTS reconciliation_diffs (
    diff_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES reconciliation_runs(run_id) ON DELETE CASCADE,
    order_id TEXT NOT NULL,
    market_id TEXT NOT NULL,
    diff_class TEXT NOT NULL,
    reason_code TEXT NOT NULL,
    internal_value TEXT,
    venue_value TEXT,
    observed_at_utc TIMESTAMPTZ NOT NULL,
    correlation_id TEXT NOT NULL,
    recorded_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (char_length(trim(diff_id)) > 0),
    CHECK (diff_id = lower(trim(diff_id))),
    CHECK (char_length(trim(run_id)) > 0),
    CHECK (char_length(trim(order_id)) > 0),
    CHECK (char_length(trim(market_id)) > 0),
    CHECK (char_length(trim(diff_class)) > 0),
    CHECK (char_length(trim(reason_code)) > 0),
    CHECK (char_length(trim(correlation_id)) > 0),
    CHECK (
        diff_class IN (
            'missing_internal_record',
            'missing_venue_record',
            'market_mismatch',
            'lifecycle_state_mismatch',
            'quantity_mismatch',
            'price_mismatch'
        )
    ),
    CHECK (
        reason_code IN (
            'reconciliation_non_critical_mismatch',
            'reconciliation_critical_mismatch',
            'reconciliation_invalid_payload'
        )
    ),
    CHECK (EXTRACT(TIMEZONE FROM observed_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM recorded_at_utc) = 0),
    UNIQUE (run_id, order_id, diff_class)
);

CREATE INDEX IF NOT EXISTS idx_reconciliation_diffs_run_lookup
    ON reconciliation_diffs (run_id, order_id, diff_class);

CREATE INDEX IF NOT EXISTS idx_reconciliation_diffs_run_time
    ON reconciliation_diffs (run_id, observed_at_utc DESC);

CREATE INDEX IF NOT EXISTS idx_reconciliation_diffs_correlation_time
    ON reconciliation_diffs (correlation_id, observed_at_utc DESC);

CREATE TABLE IF NOT EXISTS exposure_snapshots (
    snapshot_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES reconciliation_runs(run_id) ON DELETE CASCADE,
    market_id TEXT,
    net_exposure DOUBLE PRECISION NOT NULL,
    gross_exposure DOUBLE PRECISION NOT NULL,
    open_order_count BIGINT NOT NULL,
    reason_code TEXT NOT NULL,
    captured_at_utc TIMESTAMPTZ NOT NULL,
    correlation_id TEXT NOT NULL,
    recorded_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    evidence JSONB NOT NULL DEFAULT '{}'::jsonb,
    CHECK (char_length(trim(snapshot_id)) > 0),
    CHECK (snapshot_id = lower(trim(snapshot_id))),
    CHECK (char_length(trim(run_id)) > 0),
    CHECK (market_id IS NULL OR char_length(trim(market_id)) > 0),
    CHECK (char_length(trim(reason_code)) > 0),
    CHECK (char_length(trim(correlation_id)) > 0),
    CHECK (
        reason_code IN (
            'reconciliation_matched',
            'reconciliation_non_critical_mismatch',
            'reconciliation_critical_mismatch',
            'reconciliation_window_unavailable',
            'reconciliation_venue_unavailable',
            'reconciliation_persistence_unavailable',
            'reconciliation_unauthorized',
            'reconciliation_state_hydration_failed',
            'reconciliation_invalid_payload'
        )
    ),
    CHECK (gross_exposure >= 0),
    CHECK (open_order_count >= 0),
    CHECK (EXTRACT(TIMEZONE FROM captured_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM recorded_at_utc) = 0)
);

CREATE INDEX IF NOT EXISTS idx_exposure_snapshots_latest_market
    ON exposure_snapshots (market_id, captured_at_utc DESC, snapshot_id DESC);

CREATE INDEX IF NOT EXISTS idx_exposure_snapshots_latest_global
    ON exposure_snapshots (captured_at_utc DESC, snapshot_id DESC)
    WHERE market_id IS NULL;

CREATE INDEX IF NOT EXISTS idx_exposure_snapshots_run_time
    ON exposure_snapshots (run_id, captured_at_utc DESC, snapshot_id DESC);

CREATE INDEX IF NOT EXISTS idx_exposure_snapshots_correlation_time
    ON exposure_snapshots (correlation_id, captured_at_utc DESC);
