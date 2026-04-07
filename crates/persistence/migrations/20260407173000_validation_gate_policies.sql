CREATE TABLE IF NOT EXISTS validation_gate_policies (
    policy_key TEXT PRIMARY KEY,
    gate_type TEXT NOT NULL,
    stage_scope TEXT NOT NULL,
    metric_key TEXT NOT NULL,
    comparator TEXT NOT NULL,
    threshold_value DOUBLE PRECISION NOT NULL,
    mandatory BOOLEAN NOT NULL DEFAULT TRUE,
    diagnostics_json JSONB NOT NULL,
    actor_id TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    created_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (policy_key = lower(trim(policy_key))),
    CHECK (metric_key = lower(trim(metric_key))),
    CHECK (char_length(trim(policy_key)) > 0),
    CHECK (char_length(trim(metric_key)) > 0),
    CHECK (char_length(trim(actor_id)) > 0),
    CHECK (char_length(trim(correlation_id)) > 0),
    CHECK (
        gate_type IN (
            'forward_bias',
            'data_leakage',
            'regime_survivability',
            'data_quality'
        )
    ),
    CHECK (
        stage_scope IN ('training', 'promotion', 'training_and_promotion')
    ),
    CHECK (comparator IN ('lt', 'lte', 'gt', 'gte')),
    CHECK (isfinite(threshold_value)),
    CHECK (jsonb_typeof(diagnostics_json) = 'object'),
    CHECK (jsonb_object_length(diagnostics_json) > 0),
    CHECK (
        gate_type IN (
            'forward_bias',
            'data_leakage',
            'regime_survivability',
            'data_quality'
        )
        AND mandatory = TRUE
    ),
    CHECK (EXTRACT(TIMEZONE FROM created_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM updated_at_utc) = 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_validation_gate_policies_policy_key_canonical_unique
    ON validation_gate_policies (lower(trim(policy_key)));

CREATE INDEX IF NOT EXISTS idx_validation_gate_policies_stage_lookup
    ON validation_gate_policies (
        stage_scope,
        gate_type,
        mandatory DESC,
        updated_at_utc DESC,
        policy_key ASC
    );

CREATE INDEX IF NOT EXISTS idx_validation_gate_policies_correlation_lookup
    ON validation_gate_policies (
        correlation_id,
        updated_at_utc DESC,
        policy_key ASC
    );

CREATE INDEX IF NOT EXISTS idx_validation_gate_policies_actor_lookup
    ON validation_gate_policies (
        actor_id,
        updated_at_utc DESC,
        policy_key ASC
    );
