CREATE TABLE IF NOT EXISTS alpha_lifecycle_actions (
    action_id TEXT PRIMARY KEY,
    alpha_id TEXT NOT NULL,
    action_type TEXT NOT NULL,
    action_status TEXT NOT NULL,
    reason_code TEXT NOT NULL,
    trigger_evidence_json JSONB NOT NULL,
    stop_research_criteria_json JSONB,
    remediation_guidance TEXT,
    actor_id TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    acted_at_utc TIMESTAMPTZ NOT NULL,
    approval_request_id TEXT,
    approval_reference TEXT,
    created_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (action_id = lower(trim(action_id))),
    CHECK (alpha_id = lower(trim(alpha_id))),
    CHECK (action_type = lower(trim(action_type))),
    CHECK (action_status = lower(trim(action_status))),
    CHECK (reason_code = lower(trim(reason_code))),
    CHECK (char_length(trim(action_id)) > 0),
    CHECK (char_length(trim(alpha_id)) > 0),
    CHECK (char_length(trim(action_type)) > 0),
    CHECK (char_length(trim(action_status)) > 0),
    CHECK (char_length(trim(reason_code)) > 0),
    CHECK (char_length(trim(actor_id)) > 0),
    CHECK (char_length(trim(correlation_id)) > 0),
    CHECK (action_type IN ('deallocate', 'stop_research')),
    CHECK (action_status IN ('applied', 'denied', 'unapplied')),
    CHECK (jsonb_typeof(trigger_evidence_json) = 'object'),
    CHECK (
        stop_research_criteria_json IS NULL
        OR jsonb_typeof(stop_research_criteria_json) = 'object'
    ),
    CHECK (
        remediation_guidance IS NULL
        OR char_length(trim(remediation_guidance)) > 0
    ),
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
    CHECK (EXTRACT(TIMEZONE FROM acted_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM created_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM updated_at_utc) = 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_alpha_lifecycle_actions_id_canonical_unique
    ON alpha_lifecycle_actions (lower(trim(action_id)));

CREATE INDEX IF NOT EXISTS idx_alpha_lifecycle_actions_alpha_lookup
    ON alpha_lifecycle_actions (alpha_id, acted_at_utc DESC, action_id ASC);

CREATE INDEX IF NOT EXISTS idx_alpha_lifecycle_actions_type_lookup
    ON alpha_lifecycle_actions (action_type, acted_at_utc DESC, action_id ASC);

CREATE INDEX IF NOT EXISTS idx_alpha_lifecycle_actions_correlation_lookup
    ON alpha_lifecycle_actions (correlation_id, acted_at_utc DESC, action_id ASC);
