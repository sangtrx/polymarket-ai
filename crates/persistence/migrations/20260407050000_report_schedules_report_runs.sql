CREATE TABLE IF NOT EXISTS report_schedules (
    schedule_id TEXT PRIMARY KEY,
    cadence TEXT NOT NULL,
    status TEXT NOT NULL,
    next_run_at_utc TIMESTAMPTZ NOT NULL,
    actor_id TEXT NOT NULL,
    actor_role TEXT NOT NULL,
    reason_code TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    runbook_url TEXT NOT NULL,
    created_at_utc TIMESTAMPTZ NOT NULL,
    updated_at_utc TIMESTAMPTZ NOT NULL,
    paused_at_utc TIMESTAMPTZ NULL,
    CONSTRAINT report_schedules_schedule_id_check
        CHECK (schedule_id ~ '^[a-z0-9._:-]{3,160}$'),
    CONSTRAINT report_schedules_actor_id_check
        CHECK (actor_id ~ '^[a-z0-9._:-]{3,160}$'),
    CONSTRAINT report_schedules_actor_role_check
        CHECK (actor_role ~ '^[a-z0-9._:-]{3,160}$'),
    CONSTRAINT report_schedules_reason_code_check
        CHECK (reason_code ~ '^[a-z0-9._:-]{3,160}$'),
    CONSTRAINT report_schedules_correlation_id_check
        CHECK (correlation_id ~ '^[a-z0-9._:-]{3,160}$'),
    CONSTRAINT report_schedules_cadence_check
        CHECK (cadence IN ('daily', 'weekly', 'monthly')),
    CONSTRAINT report_schedules_status_check
        CHECK (status IN ('active', 'paused', 'disabled')),
    CONSTRAINT report_schedules_status_paused_check
        CHECK ((status <> 'paused') OR paused_at_utc IS NOT NULL),
    CONSTRAINT report_schedules_runbook_url_check
        CHECK (runbook_url ~* '^https?://'),
    CONSTRAINT report_schedules_update_order_check
        CHECK (updated_at_utc >= created_at_utc)
);

CREATE INDEX IF NOT EXISTS idx_report_schedules_cadence_status_next_run
    ON report_schedules (cadence, status, next_run_at_utc ASC, schedule_id ASC);

CREATE INDEX IF NOT EXISTS idx_report_schedules_correlation_time
    ON report_schedules (correlation_id, updated_at_utc DESC, schedule_id ASC);

CREATE TABLE IF NOT EXISTS report_runs (
    run_id TEXT PRIMARY KEY,
    schedule_id TEXT NOT NULL REFERENCES report_schedules(schedule_id) ON DELETE CASCADE,
    cadence TEXT NOT NULL,
    window_key TEXT NOT NULL,
    window_started_at_utc TIMESTAMPTZ NOT NULL,
    window_ended_at_utc TIMESTAMPTZ NOT NULL,
    status TEXT NOT NULL,
    reason_code TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    source_context TEXT NOT NULL,
    actor_id TEXT NULL,
    run_started_at_utc TIMESTAMPTZ NOT NULL,
    run_finished_at_utc TIMESTAMPTZ NULL,
    alert_emitted_at_utc TIMESTAMPTZ NULL,
    runbook_url TEXT NULL,
    impacted_system TEXT NULL,
    created_at_utc TIMESTAMPTZ NOT NULL,
    updated_at_utc TIMESTAMPTZ NOT NULL,
    CONSTRAINT report_runs_run_id_check
        CHECK (run_id ~ '^[a-z0-9._:-]{3,160}$'),
    CONSTRAINT report_runs_schedule_id_check
        CHECK (schedule_id ~ '^[a-z0-9._:-]{3,160}$'),
    CONSTRAINT report_runs_window_key_check
        CHECK (window_key ~ '^[a-z0-9._:-]{3,200}$'),
    CONSTRAINT report_runs_reason_code_check
        CHECK (reason_code ~ '^[a-z0-9._:-]{3,160}$'),
    CONSTRAINT report_runs_correlation_id_check
        CHECK (correlation_id ~ '^[a-z0-9._:-]{3,160}$'),
    CONSTRAINT report_runs_actor_id_check
        CHECK (
            actor_id IS NULL
            OR actor_id ~ '^[a-z0-9._:-]{3,160}$'
        ),
    CONSTRAINT report_runs_cadence_check
        CHECK (cadence IN ('daily', 'weekly', 'monthly')),
    CONSTRAINT report_runs_status_check
        CHECK (status IN ('pending', 'running', 'succeeded', 'failed', 'missed')),
    CONSTRAINT report_runs_source_context_check
        CHECK (length(trim(source_context)) > 0),
    CONSTRAINT report_runs_window_order_check
        CHECK (window_ended_at_utc > window_started_at_utc),
    CONSTRAINT report_runs_terminal_status_check
        CHECK (
            (status IN ('pending', 'running') AND run_finished_at_utc IS NULL)
            OR (
                status IN ('succeeded', 'failed', 'missed')
                AND run_finished_at_utc IS NOT NULL
            )
        ),
    CONSTRAINT report_runs_finished_after_start_check
        CHECK (
            run_finished_at_utc IS NULL
            OR run_finished_at_utc >= run_started_at_utc
        ),
    CONSTRAINT report_runs_alert_after_start_check
        CHECK (
            alert_emitted_at_utc IS NULL
            OR alert_emitted_at_utc >= run_started_at_utc
        ),
    CONSTRAINT report_runs_runbook_url_check
        CHECK (
            runbook_url IS NULL
            OR runbook_url ~* '^https?://'
        ),
    CONSTRAINT report_runs_update_order_check
        CHECK (updated_at_utc >= created_at_utc),
    CONSTRAINT report_runs_unique_schedule_window_key
        UNIQUE (schedule_id, window_key)
);

CREATE INDEX IF NOT EXISTS idx_report_runs_schedule_window
    ON report_runs (schedule_id, window_started_at_utc DESC, run_id ASC);

CREATE INDEX IF NOT EXISTS idx_report_runs_correlation_time
    ON report_runs (correlation_id, run_started_at_utc DESC, run_id ASC);

CREATE INDEX IF NOT EXISTS idx_report_runs_status_window
    ON report_runs (status, window_started_at_utc DESC, schedule_id ASC);
