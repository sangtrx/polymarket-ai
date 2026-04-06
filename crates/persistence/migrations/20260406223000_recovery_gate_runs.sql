CREATE TABLE IF NOT EXISTS recovery_gate_runs (
    run_id TEXT PRIMARY KEY,
    correlation_id TEXT NOT NULL,
    profile_key TEXT NOT NULL,
    readiness_status TEXT NOT NULL,
    reason_code TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    actor_role TEXT NOT NULL,
    reconciliation_run_id TEXT NULL,
    approved_checksum TEXT NULL,
    computed_checksum TEXT NULL,
    requested_at_utc TIMESTAMPTZ NOT NULL,
    evaluated_at_utc TIMESTAMPTZ NOT NULL,
    resumed_at_utc TIMESTAMPTZ NULL,
    recorded_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    evidence JSONB NOT NULL DEFAULT '{}'::jsonb,
    CHECK (run_id = lower(trim(run_id))),
    CHECK (correlation_id = lower(trim(correlation_id))),
    CHECK (profile_key = lower(trim(profile_key))),
    CHECK (actor_id = lower(trim(actor_id))),
    CHECK (actor_role = lower(trim(actor_role))),
    CHECK (
        reconciliation_run_id IS NULL
        OR (
            reconciliation_run_id = lower(trim(reconciliation_run_id))
            AND char_length(trim(reconciliation_run_id)) > 0
        )
    ),
    CHECK (char_length(trim(run_id)) > 0),
    CHECK (char_length(trim(correlation_id)) > 0),
    CHECK (char_length(trim(profile_key)) > 0),
    CHECK (char_length(trim(reason_code)) > 0),
    CHECK (char_length(trim(actor_id)) > 0),
    CHECK (char_length(trim(actor_role)) > 0),
    CHECK (readiness_status IN ('approved', 'blocked')),
    CHECK (
        approved_checksum IS NULL
        OR (
            approved_checksum = lower(trim(approved_checksum))
            AND approved_checksum ~ '^[0-9a-f]{64}$'
        )
    ),
    CHECK (
        computed_checksum IS NULL
        OR (
            computed_checksum = lower(trim(computed_checksum))
            AND computed_checksum ~ '^[0-9a-f]{64}$'
        )
    ),
    CHECK (
        (approved_checksum IS NULL AND computed_checksum IS NULL)
        OR (approved_checksum IS NOT NULL AND computed_checksum IS NOT NULL)
    ),
    CHECK (jsonb_typeof(evidence) = 'object'),
    CHECK (EXTRACT(TIMEZONE FROM requested_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM evaluated_at_utc) = 0),
    CHECK (resumed_at_utc IS NULL OR EXTRACT(TIMEZONE FROM resumed_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM recorded_at_utc) = 0),
    CHECK (evaluated_at_utc >= requested_at_utc),
    CHECK (resumed_at_utc IS NULL OR resumed_at_utc >= evaluated_at_utc),
    CHECK (
        (readiness_status = 'blocked' AND resumed_at_utc IS NULL)
        OR readiness_status = 'approved'
    )
);

CREATE INDEX IF NOT EXISTS idx_recovery_gate_runs_correlation_time
    ON recovery_gate_runs (correlation_id, evaluated_at_utc DESC, run_id DESC);

CREATE INDEX IF NOT EXISTS idx_recovery_gate_runs_status_time
    ON recovery_gate_runs (readiness_status, evaluated_at_utc DESC, run_id DESC);

CREATE INDEX IF NOT EXISTS idx_recovery_gate_runs_profile_time
    ON recovery_gate_runs (profile_key, evaluated_at_utc DESC, run_id DESC);
