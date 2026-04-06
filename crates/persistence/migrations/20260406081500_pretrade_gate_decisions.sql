CREATE TABLE IF NOT EXISTS pretrade_gate_decisions (
    decision_id TEXT PRIMARY KEY,
    intent_id TEXT NOT NULL,
    market_id TEXT NOT NULL,
    cluster_id TEXT NOT NULL,
    profile_key TEXT NOT NULL,
    outcome TEXT NOT NULL,
    reason_code TEXT NOT NULL,
    protective_mode_active BOOLEAN NOT NULL DEFAULT FALSE,
    gate_results JSONB NOT NULL DEFAULT '[]'::jsonb,
    correlation_id TEXT NOT NULL,
    evaluated_at_utc TIMESTAMPTZ NOT NULL,
    recorded_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    evidence JSONB NOT NULL DEFAULT '{}'::jsonb,
    CHECK (decision_id = lower(trim(decision_id))),
    CHECK (intent_id = lower(trim(intent_id))),
    CHECK (market_id = lower(trim(market_id))),
    CHECK (cluster_id = lower(trim(cluster_id))),
    CHECK (profile_key = lower(trim(profile_key))),
    CHECK (correlation_id = lower(trim(correlation_id))),
    CHECK (char_length(trim(decision_id)) > 0),
    CHECK (char_length(trim(intent_id)) > 0),
    CHECK (char_length(trim(market_id)) > 0),
    CHECK (char_length(trim(cluster_id)) > 0),
    CHECK (char_length(trim(profile_key)) > 0),
    CHECK (char_length(trim(reason_code)) > 0),
    CHECK (char_length(trim(correlation_id)) > 0),
    CHECK (outcome IN ('allow', 'deny')),
    CHECK (
        reason_code IN (
            'pretrade_gate_pass',
            'pretrade_freshness_state_unavailable',
            'pretrade_freshness_stale_breach',
            'pretrade_stream_health_state_unavailable',
            'pretrade_stream_health_degraded',
            'pretrade_risk_limit_state_unavailable',
            'pretrade_reconciliation_critical_halt',
            'pretrade_user_stream_auth_expired',
            'pretrade_drawdown_state_unavailable',
            'pretrade_drawdown_stop_triggered',
            'pretrade_strategy_approval_unavailable',
            'pretrade_strategy_approval_required',
            'pretrade_venue_eligibility_unavailable',
            'pretrade_venue_ineligible',
            'pretrade_adjudication_unavailable',
            'pretrade_adjudication_timeout',
            'pretrade_persistence_unavailable',
            'pretrade_invalid_payload'
        )
    ),
    CHECK (jsonb_typeof(gate_results) = 'array'),
    CHECK (jsonb_array_length(gate_results) > 0),
    CHECK (EXTRACT(TIMEZONE FROM evaluated_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM recorded_at_utc) = 0),
    CHECK (
        (outcome = 'allow' AND reason_code = 'pretrade_gate_pass' AND protective_mode_active = FALSE)
        OR outcome = 'deny'
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_pretrade_gate_decisions_intent_unique
    ON pretrade_gate_decisions (intent_id);

CREATE INDEX IF NOT EXISTS idx_pretrade_gate_decisions_market_time
    ON pretrade_gate_decisions (market_id, evaluated_at_utc DESC, decision_id DESC);

CREATE INDEX IF NOT EXISTS idx_pretrade_gate_decisions_reason_time
    ON pretrade_gate_decisions (reason_code, evaluated_at_utc DESC, decision_id DESC);

CREATE INDEX IF NOT EXISTS idx_pretrade_gate_decisions_correlation_time
    ON pretrade_gate_decisions (correlation_id, evaluated_at_utc DESC, decision_id DESC);
