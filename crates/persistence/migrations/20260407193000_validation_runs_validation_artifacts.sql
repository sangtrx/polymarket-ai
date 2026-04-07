CREATE TABLE IF NOT EXISTS validation_runs (
    run_id TEXT PRIMARY KEY,
    candidate_id TEXT NOT NULL,
    run_state TEXT NOT NULL,
    reason_code TEXT NOT NULL,
    gate_evaluation_json JSONB NOT NULL,
    comparison_ready BOOLEAN NOT NULL DEFAULT FALSE,
    actor_id TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    started_at_utc TIMESTAMPTZ NOT NULL,
    completed_at_utc TIMESTAMPTZ,
    created_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (run_id = lower(trim(run_id))),
    CHECK (candidate_id = lower(trim(candidate_id))),
    CHECK (reason_code = lower(trim(reason_code))),
    CHECK (char_length(trim(run_id)) > 0),
    CHECK (char_length(trim(candidate_id)) > 0),
    CHECK (char_length(trim(reason_code)) > 0),
    CHECK (char_length(trim(actor_id)) > 0),
    CHECK (char_length(trim(correlation_id)) > 0),
    CHECK (run_state IN ('running', 'completed', 'blocked', 'failed')),
    CHECK (jsonb_typeof(gate_evaluation_json) = 'object'),
    CHECK (completed_at_utc IS NULL OR completed_at_utc >= started_at_utc),
    CHECK (EXTRACT(TIMEZONE FROM started_at_utc) = 0),
    CHECK (completed_at_utc IS NULL OR EXTRACT(TIMEZONE FROM completed_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM created_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM updated_at_utc) = 0)
);

CREATE TABLE IF NOT EXISTS validation_artifacts (
    artifact_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES validation_runs (run_id) ON DELETE CASCADE,
    candidate_id TEXT NOT NULL,
    stage TEXT NOT NULL,
    stage_index SMALLINT NOT NULL,
    stage_outcome TEXT NOT NULL,
    reason_code TEXT NOT NULL,
    diagnostics_json JSONB NOT NULL,
    actor_id TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    stage_started_at_utc TIMESTAMPTZ NOT NULL,
    stage_completed_at_utc TIMESTAMPTZ NOT NULL,
    created_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (artifact_id = lower(trim(artifact_id))),
    CHECK (run_id = lower(trim(run_id))),
    CHECK (candidate_id = lower(trim(candidate_id))),
    CHECK (reason_code = lower(trim(reason_code))),
    CHECK (char_length(trim(artifact_id)) > 0),
    CHECK (char_length(trim(run_id)) > 0),
    CHECK (char_length(trim(candidate_id)) > 0),
    CHECK (char_length(trim(reason_code)) > 0),
    CHECK (char_length(trim(actor_id)) > 0),
    CHECK (char_length(trim(correlation_id)) > 0),
    CHECK (
        stage IN ('quality', 'labeling', 'purged_cv', 'cpcv', 'overfit_diagnostics')
    ),
    CHECK (stage_index BETWEEN 1 AND 5),
    CHECK (stage_outcome IN ('passed', 'failed', 'blocked')),
    CHECK (jsonb_typeof(diagnostics_json) = 'object'),
    CHECK (jsonb_object_length(diagnostics_json) > 0),
    CHECK (diagnostics_json ? 'out_of_sample_sharpe'),
    CHECK (diagnostics_json ? 'max_drawdown'),
    CHECK (diagnostics_json ? 'overfit_indicator'),
    CHECK (
        diagnostics_json ? 'brier_score'
        OR diagnostics_json ? 'expected_calibration_error'
    ),
    CHECK (stage_completed_at_utc >= stage_started_at_utc),
    CHECK (EXTRACT(TIMEZONE FROM stage_started_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM stage_completed_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM created_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM updated_at_utc) = 0),
    UNIQUE (run_id, stage_index)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_validation_runs_run_id_canonical_unique
    ON validation_runs (lower(trim(run_id)));

CREATE INDEX IF NOT EXISTS idx_validation_runs_candidate_lookup
    ON validation_runs (candidate_id, started_at_utc DESC, run_id ASC);

CREATE INDEX IF NOT EXISTS idx_validation_runs_correlation_lookup
    ON validation_runs (correlation_id, started_at_utc DESC, run_id ASC);

CREATE UNIQUE INDEX IF NOT EXISTS idx_validation_artifacts_run_stage_unique
    ON validation_artifacts (run_id, stage_index);

CREATE INDEX IF NOT EXISTS idx_validation_artifacts_run_lookup
    ON validation_artifacts (run_id, stage_index ASC, artifact_id ASC);

CREATE INDEX IF NOT EXISTS idx_validation_artifacts_candidate_lookup
    ON validation_artifacts (candidate_id, stage_index ASC, stage_completed_at_utc DESC);

CREATE INDEX IF NOT EXISTS idx_validation_artifacts_correlation_lookup
    ON validation_artifacts (correlation_id, stage_completed_at_utc DESC, artifact_id ASC);
