CREATE TABLE IF NOT EXISTS alpha_health_metrics (
    metric_id TEXT PRIMARY KEY,
    alpha_id TEXT NOT NULL,
    rolling_sharpe DOUBLE PRECISION NOT NULL,
    rolling_hit_rate DOUBLE PRECISION NOT NULL,
    rolling_drawdown DOUBLE PRECISION NOT NULL,
    stability_score DOUBLE PRECISION NOT NULL,
    windows_json JSONB NOT NULL,
    reason_code TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    recorded_at_utc TIMESTAMPTZ NOT NULL,
    created_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (metric_id = lower(trim(metric_id))),
    CHECK (alpha_id = lower(trim(alpha_id))),
    CHECK (reason_code = lower(trim(reason_code))),
    CHECK (char_length(trim(metric_id)) > 0),
    CHECK (char_length(trim(alpha_id)) > 0),
    CHECK (char_length(trim(reason_code)) > 0),
    CHECK (char_length(trim(actor_id)) > 0),
    CHECK (char_length(trim(correlation_id)) > 0),
    CHECK (jsonb_typeof(windows_json) = 'object'),
    CHECK (windows_json ? 'windows'),
    CHECK (jsonb_typeof(windows_json -> 'windows') = 'array'),
    CHECK (EXTRACT(TIMEZONE FROM recorded_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM created_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM updated_at_utc) = 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_alpha_health_metrics_id_canonical_unique
    ON alpha_health_metrics (lower(trim(metric_id)));

CREATE INDEX IF NOT EXISTS idx_alpha_health_metrics_alpha_lookup
    ON alpha_health_metrics (alpha_id, recorded_at_utc DESC, metric_id ASC);

CREATE INDEX IF NOT EXISTS idx_alpha_health_metrics_correlation_lookup
    ON alpha_health_metrics (correlation_id, recorded_at_utc DESC, metric_id ASC);

CREATE TABLE IF NOT EXISTS alpha_threshold_breaches (
    breach_id TEXT PRIMARY KEY,
    metric_id TEXT NOT NULL REFERENCES alpha_health_metrics(metric_id) ON DELETE CASCADE,
    alpha_id TEXT NOT NULL,
    metric_key TEXT NOT NULL,
    comparator TEXT NOT NULL,
    observed_value DOUBLE PRECISION NOT NULL,
    threshold_value DOUBLE PRECISION NOT NULL,
    breach_reason TEXT NOT NULL,
    reason_code TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    breached_at_utc TIMESTAMPTZ NOT NULL,
    created_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (breach_id = lower(trim(breach_id))),
    CHECK (metric_id = lower(trim(metric_id))),
    CHECK (alpha_id = lower(trim(alpha_id))),
    CHECK (metric_key = lower(trim(metric_key))),
    CHECK (comparator = lower(trim(comparator))),
    CHECK (reason_code = lower(trim(reason_code))),
    CHECK (char_length(trim(breach_id)) > 0),
    CHECK (char_length(trim(metric_id)) > 0),
    CHECK (char_length(trim(alpha_id)) > 0),
    CHECK (char_length(trim(metric_key)) > 0),
    CHECK (char_length(trim(comparator)) > 0),
    CHECK (char_length(trim(breach_reason)) > 0),
    CHECK (char_length(trim(reason_code)) > 0),
    CHECK (char_length(trim(actor_id)) > 0),
    CHECK (char_length(trim(correlation_id)) > 0),
    CHECK (
        metric_key IN (
            'rolling_sharpe',
            'rolling_hit_rate',
            'rolling_drawdown',
            'stability_score'
        )
    ),
    CHECK (
        (
            metric_key IN ('rolling_sharpe', 'rolling_hit_rate', 'stability_score')
            AND comparator = 'lt'
        )
        OR (metric_key = 'rolling_drawdown' AND comparator = 'gt')
    ),
    CHECK (EXTRACT(TIMEZONE FROM breached_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM created_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM updated_at_utc) = 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_alpha_threshold_breaches_id_canonical_unique
    ON alpha_threshold_breaches (lower(trim(breach_id)));

CREATE INDEX IF NOT EXISTS idx_alpha_threshold_breaches_alpha_lookup
    ON alpha_threshold_breaches (alpha_id, breached_at_utc DESC, breach_id ASC);

CREATE INDEX IF NOT EXISTS idx_alpha_threshold_breaches_metric_lookup
    ON alpha_threshold_breaches (metric_id, breached_at_utc DESC, breach_id ASC);

CREATE INDEX IF NOT EXISTS idx_alpha_threshold_breaches_correlation_lookup
    ON alpha_threshold_breaches (correlation_id, breached_at_utc DESC, breach_id ASC);
