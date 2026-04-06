CREATE TABLE IF NOT EXISTS allocation_policies (
    policy_key TEXT NOT NULL,
    version BIGINT NOT NULL,
    portfolio_scope_id TEXT NOT NULL,
    target_exposure_pct_nav DOUBLE PRECISION NOT NULL,
    target_relative_alpha_weight DOUBLE PRECISION NOT NULL,
    exposure_drift_threshold_pct DOUBLE PRECISION NOT NULL DEFAULT 10,
    relative_alpha_drift_threshold_pct DOUBLE PRECISION NOT NULL DEFAULT 15,
    approval_status TEXT NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT FALSE,
    approval_reference TEXT NULL,
    actor_id TEXT NOT NULL,
    reason_code TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    updated_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    advanced_parameters JSONB NOT NULL DEFAULT '{}'::jsonb,
    PRIMARY KEY (policy_key, version),
    CHECK (policy_key = lower(trim(policy_key))),
    CHECK (portfolio_scope_id = lower(trim(portfolio_scope_id))),
    CHECK (char_length(trim(policy_key)) > 0),
    CHECK (char_length(trim(portfolio_scope_id)) > 0),
    CHECK (char_length(trim(actor_id)) > 0),
    CHECK (char_length(trim(reason_code)) > 0),
    CHECK (char_length(trim(correlation_id)) > 0),
    CHECK (version > 0),
    CHECK (
        isfinite(target_exposure_pct_nav)
        AND target_exposure_pct_nav >= 0
        AND target_exposure_pct_nav <= 100
    ),
    CHECK (
        isfinite(target_relative_alpha_weight)
        AND target_relative_alpha_weight >= 0
    ),
    CHECK (
        isfinite(exposure_drift_threshold_pct)
        AND exposure_drift_threshold_pct >= 0
        AND exposure_drift_threshold_pct <= 100
    ),
    CHECK (
        isfinite(relative_alpha_drift_threshold_pct)
        AND relative_alpha_drift_threshold_pct >= 0
        AND relative_alpha_drift_threshold_pct <= 100
    ),
    CHECK (approval_status IN ('active', 'pending', 'denied')),
    CHECK (jsonb_typeof(advanced_parameters) = 'object'),
    CHECK (EXTRACT(TIMEZONE FROM updated_at_utc) = 0),
    CHECK (
        (approval_status = 'active' AND is_active = TRUE)
        OR (approval_status IN ('pending', 'denied') AND is_active = FALSE)
    ),
    CHECK (
        approval_reference IS NULL
        OR char_length(trim(approval_reference)) > 0
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_allocation_policies_active_policy_unique
    ON allocation_policies (policy_key)
    WHERE is_active = TRUE;

CREATE INDEX IF NOT EXISTS idx_allocation_policies_active_lookup
    ON allocation_policies (is_active, policy_key, updated_at_utc DESC);

CREATE INDEX IF NOT EXISTS idx_allocation_policies_actor_correlation
    ON allocation_policies (actor_id, correlation_id, updated_at_utc DESC);

CREATE INDEX IF NOT EXISTS idx_allocation_policies_latest_version
    ON allocation_policies (policy_key, version DESC, updated_at_utc DESC);

CREATE TABLE IF NOT EXISTS rebalance_recommendations (
    recommendation_id TEXT PRIMARY KEY,
    policy_key TEXT NOT NULL,
    policy_version BIGINT NOT NULL,
    status TEXT NOT NULL,
    approval_status TEXT NOT NULL,
    approval_reference TEXT NULL,
    action_type TEXT NOT NULL,
    rationale TEXT NOT NULL,
    reason_code TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    parameters JSONB NOT NULL DEFAULT '{}'::jsonb,
    evidence JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at_utc TIMESTAMPTZ NOT NULL,
    updated_at_utc TIMESTAMPTZ NOT NULL,
    FOREIGN KEY (policy_key, policy_version)
        REFERENCES allocation_policies(policy_key, version)
        ON DELETE CASCADE,
    CHECK (recommendation_id = lower(trim(recommendation_id))),
    CHECK (policy_key = lower(trim(policy_key))),
    CHECK (char_length(trim(recommendation_id)) > 0),
    CHECK (char_length(trim(policy_key)) > 0),
    CHECK (char_length(trim(action_type)) > 0),
    CHECK (char_length(trim(rationale)) > 0),
    CHECK (char_length(trim(reason_code)) > 0),
    CHECK (char_length(trim(actor_id)) > 0),
    CHECK (char_length(trim(correlation_id)) > 0),
    CHECK (policy_version > 0),
    CHECK (status IN ('proposed', 'pending_approval', 'approved', 'executed', 'denied')),
    CHECK (approval_status IN ('not_required', 'pending', 'approved', 'denied')),
    CHECK (action_type IN ('recommend', 'execute')),
    CHECK (jsonb_typeof(parameters) = 'object'),
    CHECK (jsonb_typeof(evidence) = 'object'),
    CHECK (EXTRACT(TIMEZONE FROM created_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM updated_at_utc) = 0),
    CHECK (updated_at_utc >= created_at_utc),
    CHECK (
        (approval_status = 'approved' AND approval_reference IS NOT NULL AND char_length(trim(approval_reference)) > 0)
        OR (approval_status <> 'approved')
    ),
    CHECK (
        (status = 'pending_approval' AND approval_status = 'pending')
        OR status <> 'pending_approval'
    )
);

CREATE INDEX IF NOT EXISTS idx_rebalance_recommendations_pending
    ON rebalance_recommendations (approval_status, updated_at_utc DESC)
    WHERE approval_status = 'pending';

CREATE INDEX IF NOT EXISTS idx_rebalance_recommendations_actor_correlation
    ON rebalance_recommendations (actor_id, correlation_id, created_at_utc DESC);

CREATE INDEX IF NOT EXISTS idx_rebalance_recommendations_policy_latest
    ON rebalance_recommendations (policy_key, created_at_utc DESC, recommendation_id DESC);

CREATE INDEX IF NOT EXISTS idx_rebalance_recommendations_status_policy
    ON rebalance_recommendations (status, policy_key, updated_at_utc DESC);
