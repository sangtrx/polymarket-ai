CREATE TABLE IF NOT EXISTS freshness_gate_events (
    event_id TEXT PRIMARY KEY,
    transition TEXT NOT NULL,
    reason_code TEXT NOT NULL,
    pause_active BOOLEAN NOT NULL,
    block_new_order_creation BOOLEAN NOT NULL,
    market_data_age_seconds DOUBLE PRECISION,
    user_data_age_seconds DOUBLE PRECISION,
    stale_threshold_seconds DOUBLE PRECISION NOT NULL,
    stability_window_seconds DOUBLE PRECISION NOT NULL,
    max_breach_to_pause_seconds DOUBLE PRECISION NOT NULL,
    stale_breach_detected_at_utc TIMESTAMPTZ,
    pause_activated_at_utc TIMESTAMPTZ,
    recovery_window_started_at_utc TIMESTAMPTZ,
    recovery_confirmed_at_utc TIMESTAMPTZ,
    breach_to_pause_latency_seconds DOUBLE PRECISION,
    evaluated_at_utc TIMESTAMPTZ NOT NULL,
    correlation_id TEXT NOT NULL,
    recorded_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    evidence JSONB NOT NULL DEFAULT '{}'::jsonb,
    CHECK (
        transition IN (
            'pause_activated',
            'pause_maintained',
            'recovery_pending',
            'recovery_confirmed'
        )
    ),
    CHECK (
        reason_code IN (
            'freshness_gate_stale_breach',
            'freshness_gate_boundary_safe',
            'freshness_gate_state_unavailable',
            'freshness_gate_recovery_pending',
            'freshness_gate_recovery_confirmed',
            'freshness_gate_evaluation_error',
            'freshness_gate_persistence_unavailable',
            'freshness_gate_invalid_payload'
        )
    ),
    CHECK (char_length(trim(event_id)) > 0),
    CHECK (char_length(trim(reason_code)) > 0),
    CHECK (char_length(trim(correlation_id)) > 0),
    CHECK (stale_threshold_seconds > 0),
    CHECK (stability_window_seconds > 0),
    CHECK (max_breach_to_pause_seconds > 0),
    CHECK (max_breach_to_pause_seconds <= stale_threshold_seconds),
    CHECK (market_data_age_seconds IS NULL OR market_data_age_seconds >= 0),
    CHECK (user_data_age_seconds IS NULL OR user_data_age_seconds >= 0),
    CHECK (
        breach_to_pause_latency_seconds IS NULL
        OR (
            breach_to_pause_latency_seconds >= 0
            AND breach_to_pause_latency_seconds <= max_breach_to_pause_seconds
        )
    ),
    CHECK (EXTRACT(TIMEZONE FROM evaluated_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM recorded_at_utc) = 0),
    CHECK (
        stale_breach_detected_at_utc IS NULL
        OR EXTRACT(TIMEZONE FROM stale_breach_detected_at_utc) = 0
    ),
    CHECK (
        pause_activated_at_utc IS NULL
        OR EXTRACT(TIMEZONE FROM pause_activated_at_utc) = 0
    ),
    CHECK (
        recovery_window_started_at_utc IS NULL
        OR EXTRACT(TIMEZONE FROM recovery_window_started_at_utc) = 0
    ),
    CHECK (
        recovery_confirmed_at_utc IS NULL
        OR EXTRACT(TIMEZONE FROM recovery_confirmed_at_utc) = 0
    ),
    CHECK (
        (
            transition IN ('pause_activated', 'pause_maintained', 'recovery_pending')
            AND pause_active = TRUE
            AND block_new_order_creation = TRUE
        )
        OR (
            transition = 'recovery_confirmed'
            AND pause_active = FALSE
            AND block_new_order_creation = FALSE
        )
    ),
    CHECK (
        (
            transition = 'pause_activated'
            AND stale_breach_detected_at_utc IS NOT NULL
            AND pause_activated_at_utc IS NOT NULL
        )
        OR (
            transition = 'pause_maintained'
            AND pause_activated_at_utc IS NOT NULL
        )
        OR (
            transition = 'recovery_pending'
            AND recovery_window_started_at_utc IS NOT NULL
        )
        OR (
            transition = 'recovery_confirmed'
            AND recovery_confirmed_at_utc IS NOT NULL
        )
    )
);

CREATE INDEX IF NOT EXISTS idx_freshness_gate_events_latest
    ON freshness_gate_events (evaluated_at_utc DESC, event_id DESC);

CREATE INDEX IF NOT EXISTS idx_freshness_gate_events_transition_time
    ON freshness_gate_events (transition, evaluated_at_utc DESC);

CREATE INDEX IF NOT EXISTS idx_freshness_gate_events_reason_time
    ON freshness_gate_events (reason_code, evaluated_at_utc DESC);

CREATE INDEX IF NOT EXISTS idx_freshness_gate_events_correlation_time
    ON freshness_gate_events (correlation_id, evaluated_at_utc DESC);

CREATE INDEX IF NOT EXISTS idx_freshness_gate_events_pause_state_time
    ON freshness_gate_events (pause_active, evaluated_at_utc DESC);
