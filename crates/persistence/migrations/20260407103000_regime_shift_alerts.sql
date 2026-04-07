CREATE TABLE IF NOT EXISTS regime_shift_alerts (
    alert_id TEXT PRIMARY KEY,
    market_id TEXT NOT NULL,
    cluster_id TEXT NOT NULL,
    reason_code TEXT NOT NULL,
    severity TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    observed_at TIMESTAMPTZ NOT NULL,
    issued_at TIMESTAMPTZ NOT NULL,
    dispatch_status TEXT NOT NULL,
    dispatch_reason_code TEXT NOT NULL,
    recommended_next_action TEXT NOT NULL,
    evidence_link TEXT NOT NULL,
    previous_maker_rebate_bps DOUBLE PRECISION NULL,
    current_maker_rebate_bps DOUBLE PRECISION NULL,
    rebate_delta_bps DOUBLE PRECISION NULL,
    previous_spread_bps DOUBLE PRECISION NULL,
    current_spread_bps DOUBLE PRECISION NULL,
    spread_widening_bps DOUBLE PRECISION NULL,
    previous_eligibility_state TEXT NULL,
    current_eligibility_state TEXT NULL,
    threshold_rebate_delta_bps DOUBLE PRECISION NOT NULL,
    threshold_spread_widening_bps DOUBLE PRECISION NOT NULL,
    created_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (issued_at >= observed_at),
    CHECK (alert_id = lower(trim(alert_id))),
    CHECK (market_id = lower(trim(market_id))),
    CHECK (cluster_id = lower(trim(cluster_id))),
    CHECK (correlation_id = lower(trim(correlation_id))),
    CHECK (char_length(trim(alert_id)) > 0),
    CHECK (char_length(trim(market_id)) > 0),
    CHECK (char_length(trim(cluster_id)) > 0),
    CHECK (char_length(trim(correlation_id)) > 0),
    CHECK (char_length(trim(reason_code)) > 0),
    CHECK (char_length(trim(dispatch_reason_code)) > 0),
    CHECK (char_length(trim(recommended_next_action)) > 0),
    CHECK (char_length(trim(evidence_link)) > 0),
    CHECK (severity IN ('warning', 'critical')),
    CHECK (dispatch_status IN ('pending', 'delivered', 'failed')),
    CHECK (evidence_link ~* '^https?://'),
    CHECK (threshold_rebate_delta_bps >= 0 AND isfinite(threshold_rebate_delta_bps)),
    CHECK (threshold_spread_widening_bps >= 0 AND isfinite(threshold_spread_widening_bps)),
    CHECK (previous_maker_rebate_bps IS NULL OR isfinite(previous_maker_rebate_bps)),
    CHECK (current_maker_rebate_bps IS NULL OR isfinite(current_maker_rebate_bps)),
    CHECK (rebate_delta_bps IS NULL OR isfinite(rebate_delta_bps)),
    CHECK (previous_spread_bps IS NULL OR isfinite(previous_spread_bps)),
    CHECK (current_spread_bps IS NULL OR isfinite(current_spread_bps)),
    CHECK (spread_widening_bps IS NULL OR isfinite(spread_widening_bps)),
    CHECK (
        previous_eligibility_state IS NULL
        OR previous_eligibility_state IN ('eligible', 'restricted', 'ineligible')
    ),
    CHECK (
        current_eligibility_state IS NULL
        OR current_eligibility_state IN ('eligible', 'restricted', 'ineligible')
    )
);

CREATE INDEX IF NOT EXISTS idx_regime_shift_alerts_market_observed_at
    ON regime_shift_alerts (market_id, observed_at DESC, alert_id ASC);

CREATE INDEX IF NOT EXISTS idx_regime_shift_alerts_reason_observed_at
    ON regime_shift_alerts (reason_code, observed_at DESC, alert_id ASC);

CREATE INDEX IF NOT EXISTS idx_regime_shift_alerts_correlation_observed_at
    ON regime_shift_alerts (correlation_id, observed_at DESC, alert_id ASC);
