CREATE TABLE IF NOT EXISTS alpha_hypotheses (
    hypothesis_id TEXT PRIMARY KEY,
    feature_set_version TEXT NOT NULL,
    target_regime TEXT NOT NULL,
    expected_edge_source TEXT NOT NULL,
    training_window_start_utc TIMESTAMPTZ NOT NULL,
    training_window_end_utc TIMESTAMPTZ NOT NULL,
    risk_assumptions_json JSONB NOT NULL,
    actor_id TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    created_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (hypothesis_id = lower(trim(hypothesis_id))),
    CHECK (feature_set_version = lower(trim(feature_set_version))),
    CHECK (target_regime = lower(trim(target_regime))),
    CHECK (expected_edge_source = lower(trim(expected_edge_source))),
    CHECK (char_length(trim(hypothesis_id)) > 0),
    CHECK (char_length(trim(feature_set_version)) > 0),
    CHECK (char_length(trim(target_regime)) > 0),
    CHECK (char_length(trim(expected_edge_source)) > 0),
    CHECK (char_length(trim(actor_id)) > 0),
    CHECK (char_length(trim(correlation_id)) > 0),
    CHECK (jsonb_typeof(risk_assumptions_json) = 'object'),
    CHECK (jsonb_object_length(risk_assumptions_json) > 0),
    CHECK (training_window_start_utc < training_window_end_utc),
    CHECK (EXTRACT(TIMEZONE FROM training_window_start_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM training_window_end_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM created_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM updated_at_utc) = 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_alpha_hypotheses_hypothesis_id_canonical_unique
    ON alpha_hypotheses (lower(trim(hypothesis_id)));

CREATE INDEX IF NOT EXISTS idx_alpha_hypotheses_feature_set_lookup
    ON alpha_hypotheses (feature_set_version, updated_at_utc DESC, hypothesis_id ASC);

CREATE INDEX IF NOT EXISTS idx_alpha_hypotheses_correlation_lookup
    ON alpha_hypotheses (correlation_id, updated_at_utc DESC, hypothesis_id ASC);

CREATE INDEX IF NOT EXISTS idx_alpha_hypotheses_actor_lookup
    ON alpha_hypotheses (actor_id, updated_at_utc DESC, hypothesis_id ASC);
