CREATE TABLE IF NOT EXISTS incident_query_views (
    view_id TEXT PRIMARY KEY,
    occurred_at TIMESTAMPTZ NOT NULL,
    market_id TEXT NULL,
    order_id TEXT NULL,
    alpha_id TEXT NULL,
    actor_id TEXT NULL,
    stage TEXT NOT NULL,
    source TEXT NOT NULL,
    reason_code TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    run_id TEXT NULL,
    snapshot_id TEXT NULL,
    summary TEXT NOT NULL,
    severity TEXT NOT NULL,
    recommended_next_action TEXT NOT NULL,
    evidence JSONB NOT NULL DEFAULT '{}'::jsonb,
    recorded_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (char_length(trim(view_id)) > 0),
    CHECK (view_id = lower(trim(view_id))),
    CHECK (
        market_id IS NULL
        OR (
            market_id = lower(trim(market_id))
            AND char_length(trim(market_id)) > 0
        )
    ),
    CHECK (
        order_id IS NULL
        OR (
            order_id = lower(trim(order_id))
            AND char_length(trim(order_id)) > 0
        )
    ),
    CHECK (
        alpha_id IS NULL
        OR (
            alpha_id = lower(trim(alpha_id))
            AND char_length(trim(alpha_id)) > 0
        )
    ),
    CHECK (
        actor_id IS NULL
        OR (
            actor_id = lower(trim(actor_id))
            AND char_length(trim(actor_id)) > 0
        )
    ),
    CHECK (
        run_id IS NULL
        OR (
            run_id = lower(trim(run_id))
            AND char_length(trim(run_id)) > 0
        )
    ),
    CHECK (
        snapshot_id IS NULL
        OR (
            snapshot_id = lower(trim(snapshot_id))
            AND char_length(trim(snapshot_id)) > 0
        )
    ),
    CHECK (char_length(trim(stage)) > 0),
    CHECK (char_length(trim(source)) > 0),
    CHECK (char_length(trim(reason_code)) > 0),
    CHECK (char_length(trim(correlation_id)) > 0),
    CHECK (char_length(trim(summary)) > 0),
    CHECK (char_length(trim(recommended_next_action)) > 0),
    CHECK (stage IN ('signal', 'order', 'fill', 'pnl', 'risk_action')),
    CHECK (severity IN ('normal', 'warning', 'critical', 'degraded')),
    CHECK (jsonb_typeof(evidence) = 'object')
);

CREATE INDEX IF NOT EXISTS idx_incident_query_views_window_lookup
    ON incident_query_views (occurred_at DESC, view_id ASC);

CREATE INDEX IF NOT EXISTS idx_incident_query_views_market_window
    ON incident_query_views (market_id, occurred_at DESC, view_id ASC)
    WHERE market_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_incident_query_views_order_window
    ON incident_query_views (order_id, occurred_at DESC, view_id ASC)
    WHERE order_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_incident_query_views_alpha_window
    ON incident_query_views (alpha_id, occurred_at DESC, view_id ASC)
    WHERE alpha_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_incident_query_views_actor_window
    ON incident_query_views (actor_id, occurred_at DESC, view_id ASC)
    WHERE actor_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_incident_query_views_correlation_time
    ON incident_query_views (correlation_id, occurred_at DESC);
