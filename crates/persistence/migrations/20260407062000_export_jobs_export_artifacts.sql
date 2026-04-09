CREATE TABLE IF NOT EXISTS export_jobs (
    job_id TEXT PRIMARY KEY,
    trigger_source TEXT NOT NULL,
    status TEXT NOT NULL,
    reason_code TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    actor_role TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    schedule_id TEXT NULL,
    schedule_window_key TEXT NULL,
    report_run_id TEXT NULL,
    incident_id TEXT NULL,
    incident_severity TEXT NULL,
    requested_at_utc TIMESTAMPTZ NOT NULL,
    started_at_utc TIMESTAMPTZ NULL,
    finished_at_utc TIMESTAMPTZ NULL,
    package_reference TEXT NULL,
    package_checksum TEXT NULL,
    failure_metadata JSONB NULL,
    created_at_utc TIMESTAMPTZ NOT NULL,
    updated_at_utc TIMESTAMPTZ NOT NULL,
    CONSTRAINT export_jobs_job_id_check
        CHECK (job_id ~ '^[a-z0-9._:-]{3,200}$'),
    CONSTRAINT export_jobs_trigger_source_check
        CHECK (trigger_source IN ('scheduled_weekly', 'on_demand', 'incident_triggered')),
    CONSTRAINT export_jobs_status_check
        CHECK (status IN ('queued', 'running', 'succeeded', 'failed')),
    CONSTRAINT export_jobs_reason_code_check
        CHECK (reason_code ~ '^[a-z0-9._:-]{3,200}$'),
    CONSTRAINT export_jobs_actor_id_check
        CHECK (actor_id ~ '^[a-z0-9._:-]{3,200}$'),
    CONSTRAINT export_jobs_actor_role_check
        CHECK (actor_role ~ '^[a-z0-9._:-]{3,200}$'),
    CONSTRAINT export_jobs_correlation_id_check
        CHECK (correlation_id ~ '^[a-z0-9._:-]{3,200}$'),
    CONSTRAINT export_jobs_schedule_id_check
        CHECK (
            schedule_id IS NULL
            OR schedule_id ~ '^[a-z0-9._:-]{3,200}$'
        ),
    CONSTRAINT export_jobs_schedule_window_key_check
        CHECK (
            schedule_window_key IS NULL
            OR schedule_window_key ~ '^[a-z0-9._:-]{3,240}$'
        ),
    CONSTRAINT export_jobs_report_run_id_check
        CHECK (
            report_run_id IS NULL
            OR report_run_id ~ '^[a-z0-9._:-]{3,200}$'
        ),
    CONSTRAINT export_jobs_incident_id_check
        CHECK (
            incident_id IS NULL
            OR incident_id ~ '^[a-z0-9._:-]{3,200}$'
        ),
    CONSTRAINT export_jobs_incident_severity_check
        CHECK (
            incident_severity IS NULL
            OR incident_severity ~ '^[a-z0-9._:-]{3,200}$'
        ),
    CONSTRAINT export_jobs_weekly_linkage_required_check
        CHECK (
            trigger_source <> 'scheduled_weekly'
            OR (
                schedule_id IS NOT NULL
                AND schedule_window_key IS NOT NULL
                AND report_run_id IS NOT NULL
            )
        ),
    CONSTRAINT export_jobs_incident_linkage_required_check
        CHECK (
            trigger_source <> 'incident_triggered'
            OR (incident_id IS NOT NULL AND incident_severity IS NOT NULL)
        ),
    CONSTRAINT export_jobs_lifecycle_timestamps_check
        CHECK (
            (status = 'queued' AND started_at_utc IS NULL AND finished_at_utc IS NULL)
            OR (status = 'running' AND started_at_utc IS NOT NULL AND finished_at_utc IS NULL)
            OR (
                status IN ('succeeded', 'failed')
                AND started_at_utc IS NOT NULL
                AND finished_at_utc IS NOT NULL
            )
        ),
    CONSTRAINT export_jobs_finished_after_started_check
        CHECK (
            finished_at_utc IS NULL
            OR finished_at_utc >= started_at_utc
        ),
    CONSTRAINT export_jobs_package_checksum_check
        CHECK (
            package_checksum IS NULL
            OR package_checksum ~ '^[0-9a-f]{64}$'
        ),
    CONSTRAINT export_jobs_update_order_check
        CHECK (updated_at_utc >= created_at_utc)
);

CREATE INDEX IF NOT EXISTS idx_export_jobs_trigger_status_requested
    ON export_jobs (trigger_source, status, requested_at_utc DESC, job_id ASC);

CREATE INDEX IF NOT EXISTS idx_export_jobs_correlation_requested
    ON export_jobs (correlation_id, requested_at_utc DESC, job_id ASC);

CREATE UNIQUE INDEX IF NOT EXISTS uq_export_jobs_weekly_idempotency
    ON export_jobs (trigger_source, schedule_id, schedule_window_key, report_run_id)
    WHERE trigger_source = 'scheduled_weekly';

CREATE TABLE IF NOT EXISTS export_artifacts (
    artifact_id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL REFERENCES export_jobs(job_id) ON DELETE CASCADE,
    artifact_type TEXT NOT NULL,
    source TEXT NOT NULL,
    as_of_utc TIMESTAMPTZ NOT NULL,
    reason_code TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    checksum TEXT NOT NULL,
    retrieval_reference TEXT NOT NULL,
    is_available BOOLEAN NOT NULL DEFAULT TRUE,
    created_at_utc TIMESTAMPTZ NOT NULL,
    updated_at_utc TIMESTAMPTZ NOT NULL,
    CONSTRAINT export_artifacts_artifact_id_check
        CHECK (artifact_id ~ '^[a-z0-9._:-]{3,200}$'),
    CONSTRAINT export_artifacts_job_id_check
        CHECK (job_id ~ '^[a-z0-9._:-]{3,200}$'),
    CONSTRAINT export_artifacts_artifact_type_check
        CHECK (
            artifact_type IN (
                'promotion_decisions',
                'validation_evidence',
                'reconciliation_summary',
                'access_audits',
                'incident_postmortems',
                'readiness_report_json',
                'readiness_report_markdown'
            )
        ),
    CONSTRAINT export_artifacts_source_check
        CHECK (source ~ '^[a-z0-9._:-]{3,200}$'),
    CONSTRAINT export_artifacts_reason_code_check
        CHECK (reason_code ~ '^[a-z0-9._:-]{3,200}$'),
    CONSTRAINT export_artifacts_correlation_id_check
        CHECK (correlation_id ~ '^[a-z0-9._:-]{3,200}$'),
    CONSTRAINT export_artifacts_checksum_check
        CHECK (
            (is_available = FALSE AND checksum = 'unavailable')
            OR checksum ~ '^[0-9a-f]{64}$'
        ),
    CONSTRAINT export_artifacts_retrieval_reference_check
        CHECK (length(trim(retrieval_reference)) > 0),
    CONSTRAINT export_artifacts_update_order_check
        CHECK (updated_at_utc >= created_at_utc),
    CONSTRAINT export_artifacts_unique_job_category
        UNIQUE (job_id, artifact_type)
);

CREATE INDEX IF NOT EXISTS idx_export_artifacts_job_lookup
    ON export_artifacts (job_id, artifact_type ASC, artifact_id ASC);

CREATE INDEX IF NOT EXISTS idx_export_artifacts_as_of_lookup
    ON export_artifacts (job_id, as_of_utc DESC, artifact_type ASC, artifact_id ASC);
