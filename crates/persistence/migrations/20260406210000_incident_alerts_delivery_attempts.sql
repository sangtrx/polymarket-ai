CREATE TABLE IF NOT EXISTS incident_alerts (
    alert_id TEXT PRIMARY KEY,
    severity TEXT NOT NULL,
    impacted_subsystem TEXT NOT NULL,
    cause TEXT NOT NULL,
    recommended_next_action TEXT NOT NULL,
    evidence_link TEXT NOT NULL,
    issued_at TIMESTAMPTZ NOT NULL,
    correlation_id TEXT NOT NULL,
    reason_code TEXT NOT NULL,
    status TEXT NOT NULL,
    delivered_at TIMESTAMPTZ NULL,
    failed_at TIMESTAMPTZ NULL,
    created_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (alert_id = lower(trim(alert_id))),
    CHECK (char_length(trim(alert_id)) > 0),
    CHECK (char_length(trim(impacted_subsystem)) > 0),
    CHECK (char_length(trim(cause)) > 0),
    CHECK (char_length(trim(recommended_next_action)) > 0),
    CHECK (char_length(trim(evidence_link)) > 0),
    CHECK (char_length(trim(correlation_id)) > 0),
    CHECK (char_length(trim(reason_code)) > 0),
    CHECK (correlation_id = lower(trim(correlation_id))),
    CHECK (severity IN ('warning', 'critical')),
    CHECK (status IN ('pending', 'delivered', 'failed')),
    CHECK (evidence_link ~* '^https?://'),
    CHECK (
        (status = 'pending' AND delivered_at IS NULL AND failed_at IS NULL)
        OR (status = 'delivered' AND delivered_at IS NOT NULL AND failed_at IS NULL)
        OR (status = 'failed' AND failed_at IS NOT NULL AND delivered_at IS NULL)
    )
);

CREATE TABLE IF NOT EXISTS alert_delivery_attempts (
    alert_id TEXT NOT NULL REFERENCES incident_alerts(alert_id) ON DELETE CASCADE,
    attempt_number INTEGER NOT NULL,
    channel TEXT NOT NULL,
    outcome TEXT NOT NULL,
    reason_code TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    attempted_at TIMESTAMPTZ NOT NULL,
    delivered_at TIMESTAMPTZ NULL,
    failed_at TIMESTAMPTZ NULL,
    created_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (alert_id, attempt_number),
    CHECK (attempt_number > 0),
    CHECK (channel IN ('pagerduty', 'slack', 'email')),
    CHECK (outcome IN ('delivered', 'failed')),
    CHECK (char_length(trim(reason_code)) > 0),
    CHECK (char_length(trim(correlation_id)) > 0),
    CHECK (correlation_id = lower(trim(correlation_id))),
    CHECK (
        (outcome = 'delivered' AND delivered_at IS NOT NULL AND failed_at IS NULL)
        OR (outcome = 'failed' AND failed_at IS NOT NULL AND delivered_at IS NULL)
    )
);

CREATE INDEX IF NOT EXISTS idx_incident_alerts_severity_issued_at
    ON incident_alerts (severity, issued_at DESC, alert_id ASC);

CREATE INDEX IF NOT EXISTS idx_incident_alerts_status_issued_at
    ON incident_alerts (status, issued_at DESC, alert_id ASC);

CREATE INDEX IF NOT EXISTS idx_incident_alerts_correlation_issued_at
    ON incident_alerts (correlation_id, issued_at DESC, alert_id ASC);

CREATE INDEX IF NOT EXISTS idx_alert_delivery_attempts_alert_id_attempt_number
    ON alert_delivery_attempts (alert_id, attempt_number ASC);

CREATE INDEX IF NOT EXISTS idx_alert_delivery_attempts_channel_outcome_attempted_at
    ON alert_delivery_attempts (channel, outcome, attempted_at DESC);
