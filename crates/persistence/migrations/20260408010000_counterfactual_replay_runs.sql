CREATE TABLE IF NOT EXISTS counterfactual_replay_runs (
    run_id TEXT PRIMARY KEY,
    candidate_id TEXT NOT NULL,
    validation_run_id TEXT NOT NULL,
    run_state TEXT NOT NULL,
    reason_code TEXT NOT NULL,
    scenario_results_json JSONB NOT NULL,
    replay_summary_json JSONB NOT NULL,
    actor_id TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    started_at_utc TIMESTAMPTZ NOT NULL,
    completed_at_utc TIMESTAMPTZ,
    created_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (run_id = lower(trim(run_id))),
    CHECK (candidate_id = lower(trim(candidate_id))),
    CHECK (validation_run_id = lower(trim(validation_run_id))),
    CHECK (run_state = lower(trim(run_state))),
    CHECK (reason_code = lower(trim(reason_code))),
    CHECK (char_length(trim(run_id)) > 0),
    CHECK (char_length(trim(candidate_id)) > 0),
    CHECK (char_length(trim(validation_run_id)) > 0),
    CHECK (char_length(trim(reason_code)) > 0),
    CHECK (char_length(trim(actor_id)) > 0),
    CHECK (char_length(trim(correlation_id)) > 0),
    CHECK (run_state IN ('running', 'completed', 'denied', 'failed')),
    CHECK (jsonb_typeof(scenario_results_json) = 'object'),
    CHECK (scenario_results_json ? 'scenarios'),
    CHECK (jsonb_typeof(scenario_results_json -> 'scenarios') = 'array'),
    CHECK (jsonb_typeof(replay_summary_json) = 'object'),
    CHECK (completed_at_utc IS NULL OR completed_at_utc >= started_at_utc),
    CHECK (EXTRACT(TIMEZONE FROM started_at_utc) = 0),
    CHECK (completed_at_utc IS NULL OR EXTRACT(TIMEZONE FROM completed_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM created_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM updated_at_utc) = 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_counterfactual_replay_runs_id_canonical_unique
    ON counterfactual_replay_runs (lower(trim(run_id)));

CREATE INDEX IF NOT EXISTS idx_counterfactual_replay_runs_candidate_lookup
    ON counterfactual_replay_runs (candidate_id, started_at_utc DESC, run_id ASC);

CREATE INDEX IF NOT EXISTS idx_counterfactual_replay_runs_validation_lookup
    ON counterfactual_replay_runs (validation_run_id, started_at_utc DESC, run_id ASC);

CREATE INDEX IF NOT EXISTS idx_counterfactual_replay_runs_correlation_lookup
    ON counterfactual_replay_runs (correlation_id, started_at_utc DESC, run_id ASC);
