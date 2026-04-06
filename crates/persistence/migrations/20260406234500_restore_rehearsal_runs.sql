CREATE TABLE IF NOT EXISTS restore_rehearsal_runs (
    run_id TEXT PRIMARY KEY,
    correlation_id TEXT NOT NULL,
    incident_correlation_id TEXT NULL,
    incident_severity TEXT NULL,
    artifact_id TEXT NOT NULL,
    artifact_checksum TEXT NOT NULL,
    observed_checksum TEXT NOT NULL,
    restore_target TEXT NOT NULL,
    rehearsal_status TEXT NOT NULL,
    reason_code TEXT NOT NULL,
    reconciliation_run_id TEXT NOT NULL,
    reconciliation_mismatch_rate DOUBLE PRECISION NULL,
    reconciliation_passed BOOLEAN NOT NULL,
    deterministic_signature TEXT NOT NULL,
    prior_deterministic_signature TEXT NULL,
    deterministic_signature_match BOOLEAN NULL,
    deterministic_mismatch_summary TEXT NULL,
    requested_at_utc TIMESTAMPTZ NOT NULL,
    started_at_utc TIMESTAMPTZ NOT NULL,
    completed_at_utc TIMESTAMPTZ NOT NULL,
    recorded_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    audit_reference TEXT NULL,
    evidence JSONB NOT NULL DEFAULT '{}'::jsonb,
    CHECK (run_id = lower(trim(run_id))),
    CHECK (correlation_id = lower(trim(correlation_id))),
    CHECK (artifact_id = lower(trim(artifact_id))),
    CHECK (restore_target = lower(trim(restore_target))),
    CHECK (reconciliation_run_id = lower(trim(reconciliation_run_id))),
    CHECK (
        incident_correlation_id IS NULL
        OR (
            incident_correlation_id = lower(trim(incident_correlation_id))
            AND char_length(trim(incident_correlation_id)) > 0
        )
    ),
    CHECK (
        incident_severity IS NULL
        OR incident_severity IN ('severity_1', 'severity_2', 'severity_3', 'severity_4')
    ),
    CHECK (char_length(trim(run_id)) > 0),
    CHECK (char_length(trim(correlation_id)) > 0),
    CHECK (char_length(trim(artifact_id)) > 0),
    CHECK (char_length(trim(restore_target)) > 0),
    CHECK (char_length(trim(reason_code)) > 0),
    CHECK (char_length(trim(reconciliation_run_id)) > 0),
    CHECK (rehearsal_status IN ('passed', 'failed')),
    CHECK (
        artifact_checksum = lower(trim(artifact_checksum))
        AND artifact_checksum ~ '^[0-9a-f]{64}$'
    ),
    CHECK (
        observed_checksum = lower(trim(observed_checksum))
        AND observed_checksum ~ '^[0-9a-f]{64}$'
    ),
    CHECK (
        deterministic_signature = lower(trim(deterministic_signature))
        AND deterministic_signature ~ '^[0-9a-f]{64}$'
    ),
    CHECK (
        prior_deterministic_signature IS NULL
        OR (
            prior_deterministic_signature = lower(trim(prior_deterministic_signature))
            AND prior_deterministic_signature ~ '^[0-9a-f]{64}$'
        )
    ),
    CHECK (
        deterministic_signature_match IS NULL
        OR prior_deterministic_signature IS NOT NULL
    ),
    CHECK (
        deterministic_signature_match IS DISTINCT FROM FALSE
        OR (
            deterministic_mismatch_summary IS NOT NULL
            AND char_length(trim(deterministic_mismatch_summary)) > 0
        )
    ),
    CHECK (
        deterministic_mismatch_summary IS NULL
        OR char_length(trim(deterministic_mismatch_summary)) > 0
    ),
    CHECK (
        reconciliation_mismatch_rate IS NULL
        OR (
            isfinite(reconciliation_mismatch_rate)
            AND reconciliation_mismatch_rate >= 0
            AND reconciliation_mismatch_rate <= 1
        )
    ),
    CHECK (jsonb_typeof(evidence) = 'object'),
    CHECK (EXTRACT(TIMEZONE FROM requested_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM started_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM completed_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM recorded_at_utc) = 0),
    CHECK (started_at_utc >= requested_at_utc),
    CHECK (completed_at_utc >= started_at_utc)
);

CREATE TABLE IF NOT EXISTS backup_integrity_checks (
    check_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES restore_rehearsal_runs(run_id) ON DELETE CASCADE,
    check_name TEXT NOT NULL,
    passed BOOLEAN NOT NULL,
    reason_code TEXT NOT NULL,
    expected_value TEXT NULL,
    observed_value TEXT NULL,
    details TEXT NOT NULL,
    recorded_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (check_id = lower(trim(check_id))),
    CHECK (run_id = lower(trim(run_id))),
    CHECK (char_length(trim(check_id)) > 0),
    CHECK (char_length(trim(run_id)) > 0),
    CHECK (char_length(trim(check_name)) > 0),
    CHECK (char_length(trim(reason_code)) > 0),
    CHECK (char_length(trim(details)) > 0),
    CHECK (EXTRACT(TIMEZONE FROM recorded_at_utc) = 0)
);

CREATE INDEX IF NOT EXISTS idx_restore_rehearsal_runs_artifact_time
    ON restore_rehearsal_runs (artifact_id, completed_at_utc DESC, run_id DESC);

CREATE INDEX IF NOT EXISTS idx_restore_rehearsal_runs_correlation_time
    ON restore_rehearsal_runs (correlation_id, completed_at_utc DESC, run_id DESC);

CREATE INDEX IF NOT EXISTS idx_restore_rehearsal_runs_incident_correlation_time
    ON restore_rehearsal_runs (incident_correlation_id, completed_at_utc DESC, run_id DESC);

CREATE INDEX IF NOT EXISTS idx_restore_rehearsal_runs_status_time
    ON restore_rehearsal_runs (rehearsal_status, completed_at_utc DESC, run_id DESC);

CREATE INDEX IF NOT EXISTS idx_restore_rehearsal_runs_reason_time
    ON restore_rehearsal_runs (reason_code, completed_at_utc DESC, run_id DESC);

CREATE INDEX IF NOT EXISTS idx_backup_integrity_checks_run_time
    ON backup_integrity_checks (run_id, recorded_at_utc DESC, check_id DESC);

CREATE INDEX IF NOT EXISTS idx_backup_integrity_checks_run_passed
    ON backup_integrity_checks (run_id, passed, check_name);
