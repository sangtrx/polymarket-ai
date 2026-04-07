CREATE TABLE IF NOT EXISTS shadow_evaluations (
    evaluation_id TEXT PRIMARY KEY,
    candidate_id TEXT NOT NULL,
    validation_run_id TEXT NOT NULL,
    evaluation_state TEXT NOT NULL,
    reason_code TEXT NOT NULL,
    market_context_json JSONB NOT NULL,
    signal_decisions_json JSONB NOT NULL,
    simulation_outcomes_json JSONB NOT NULL,
    actor_id TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    started_at_utc TIMESTAMPTZ NOT NULL,
    completed_at_utc TIMESTAMPTZ,
    created_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (evaluation_id = lower(trim(evaluation_id))),
    CHECK (candidate_id = lower(trim(candidate_id))),
    CHECK (validation_run_id = lower(trim(validation_run_id))),
    CHECK (reason_code = lower(trim(reason_code))),
    CHECK (char_length(trim(evaluation_id)) > 0),
    CHECK (char_length(trim(candidate_id)) > 0),
    CHECK (char_length(trim(validation_run_id)) > 0),
    CHECK (char_length(trim(reason_code)) > 0),
    CHECK (char_length(trim(actor_id)) > 0),
    CHECK (char_length(trim(correlation_id)) > 0),
    CHECK (evaluation_state IN ('running', 'completed', 'denied', 'failed')),
    CHECK (jsonb_typeof(market_context_json) = 'object'),
    CHECK (jsonb_typeof(signal_decisions_json) = 'object'),
    CHECK (jsonb_typeof(simulation_outcomes_json) = 'object'),
    CHECK (simulation_outcomes_json ? 'outcomes'),
    CHECK (jsonb_typeof(simulation_outcomes_json -> 'outcomes') = 'array'),
    CHECK (completed_at_utc IS NULL OR completed_at_utc >= started_at_utc),
    CHECK (EXTRACT(TIMEZONE FROM started_at_utc) = 0),
    CHECK (completed_at_utc IS NULL OR EXTRACT(TIMEZONE FROM completed_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM created_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM updated_at_utc) = 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_shadow_evaluations_id_canonical_unique
    ON shadow_evaluations (lower(trim(evaluation_id)));

CREATE INDEX IF NOT EXISTS idx_shadow_evaluations_candidate_lookup
    ON shadow_evaluations (candidate_id, started_at_utc DESC, evaluation_id ASC);

CREATE INDEX IF NOT EXISTS idx_shadow_evaluations_validation_run_lookup
    ON shadow_evaluations (validation_run_id, started_at_utc DESC, evaluation_id ASC);

CREATE INDEX IF NOT EXISTS idx_shadow_evaluations_correlation_lookup
    ON shadow_evaluations (correlation_id, started_at_utc DESC, evaluation_id ASC);
