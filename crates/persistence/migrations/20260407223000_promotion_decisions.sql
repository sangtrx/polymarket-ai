CREATE TABLE IF NOT EXISTS promotion_decisions (
    decision_id TEXT PRIMARY KEY,
    candidate_id TEXT NOT NULL,
    validation_run_id TEXT NOT NULL,
    lifecycle_action TEXT NOT NULL,
    decision_state TEXT NOT NULL,
    reason_code TEXT NOT NULL,
    observed_metrics_json JSONB NOT NULL,
    evidence_packet_json JSONB NOT NULL,
    threshold_results_json JSONB NOT NULL,
    missing_evidence_fields_json JSONB NOT NULL,
    gate_evaluation_json JSONB NOT NULL,
    shadow_readiness_json JSONB,
    actor_id TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    decided_at_utc TIMESTAMPTZ NOT NULL,
    approval_request_id TEXT,
    approval_reference TEXT,
    created_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (decision_id = lower(trim(decision_id))),
    CHECK (candidate_id = lower(trim(candidate_id))),
    CHECK (validation_run_id = lower(trim(validation_run_id))),
    CHECK (lifecycle_action = lower(trim(lifecycle_action))),
    CHECK (decision_state = lower(trim(decision_state))),
    CHECK (reason_code = lower(trim(reason_code))),
    CHECK (char_length(trim(decision_id)) > 0),
    CHECK (char_length(trim(candidate_id)) > 0),
    CHECK (char_length(trim(validation_run_id)) > 0),
    CHECK (char_length(trim(reason_code)) > 0),
    CHECK (char_length(trim(actor_id)) > 0),
    CHECK (char_length(trim(correlation_id)) > 0),
    CHECK (lifecycle_action IN ('promote', 'pause', 'retire')),
    CHECK (decision_state IN ('allowed', 'denied')),
    CHECK (jsonb_typeof(observed_metrics_json) = 'object'),
    CHECK (jsonb_typeof(evidence_packet_json) = 'object'),
    CHECK (jsonb_typeof(threshold_results_json) = 'object'),
    CHECK (threshold_results_json ? 'thresholds'),
    CHECK (jsonb_typeof(threshold_results_json -> 'thresholds') = 'array'),
    CHECK (jsonb_typeof(missing_evidence_fields_json) = 'object'),
    CHECK (missing_evidence_fields_json ? 'fields'),
    CHECK (jsonb_typeof(missing_evidence_fields_json -> 'fields') = 'array'),
    CHECK (jsonb_typeof(gate_evaluation_json) = 'object'),
    CHECK (shadow_readiness_json IS NULL OR jsonb_typeof(shadow_readiness_json) = 'object'),
    CHECK (
        approval_request_id IS NULL
        OR char_length(trim(approval_request_id)) > 0
    ),
    CHECK (
        approval_reference IS NULL
        OR char_length(trim(approval_reference)) > 0
    ),
    CHECK (
        approval_reference IS NULL
        OR approval_request_id IS NOT NULL
    ),
    CHECK (EXTRACT(TIMEZONE FROM decided_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM created_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM updated_at_utc) = 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_promotion_decisions_id_canonical_unique
    ON promotion_decisions (lower(trim(decision_id)));

CREATE INDEX IF NOT EXISTS idx_promotion_decisions_candidate_lookup
    ON promotion_decisions (candidate_id, decided_at_utc DESC, decision_id ASC);

CREATE INDEX IF NOT EXISTS idx_promotion_decisions_validation_run_lookup
    ON promotion_decisions (validation_run_id, decided_at_utc DESC, decision_id ASC);

CREATE INDEX IF NOT EXISTS idx_promotion_decisions_correlation_lookup
    ON promotion_decisions (correlation_id, decided_at_utc DESC, decision_id ASC);
