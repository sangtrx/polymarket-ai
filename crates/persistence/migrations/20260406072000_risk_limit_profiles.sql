CREATE TABLE IF NOT EXISTS risk_limit_profiles (
    profile_key TEXT NOT NULL,
    version BIGINT NOT NULL,
    scope_type TEXT NOT NULL,
    scope_id TEXT NOT NULL,
    max_notional_usd DOUBLE PRECISION NOT NULL,
    max_inventory_units DOUBLE PRECISION NOT NULL,
    max_concentration_pct_nav DOUBLE PRECISION NOT NULL,
    approval_status TEXT NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT FALSE,
    approval_reference TEXT NULL,
    actor_id TEXT NOT NULL,
    reason_code TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    updated_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (profile_key, version, scope_type),
    CHECK (profile_key = lower(trim(profile_key))),
    CHECK (scope_id = lower(trim(scope_id))),
    CHECK (char_length(trim(profile_key)) > 0),
    CHECK (char_length(trim(scope_type)) > 0),
    CHECK (char_length(trim(scope_id)) > 0),
    CHECK (char_length(trim(actor_id)) > 0),
    CHECK (char_length(trim(reason_code)) > 0),
    CHECK (char_length(trim(correlation_id)) > 0),
    CHECK (scope_type IN ('portfolio', 'market', 'strategy')),
    CHECK (version > 0),
    CHECK (isfinite(max_notional_usd) AND max_notional_usd >= 0),
    CHECK (isfinite(max_inventory_units) AND max_inventory_units >= 0),
    CHECK (
        isfinite(max_concentration_pct_nav)
        AND max_concentration_pct_nav >= 0
        AND max_concentration_pct_nav <= 100
    ),
    CHECK (approval_status IN ('active', 'pending', 'denied')),
    CHECK (EXTRACT(TIMEZONE FROM updated_at_utc) = 0),
    CHECK (
        (approval_status = 'active' AND is_active = TRUE)
        OR (approval_status IN ('pending', 'denied') AND is_active = FALSE)
    ),
    CHECK (
        (approval_status = 'active' AND approval_reference IS NOT NULL AND char_length(trim(approval_reference)) > 0)
        OR (approval_status <> 'active')
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_risk_limit_profiles_active_scope_unique
    ON risk_limit_profiles (profile_key, scope_type)
    WHERE is_active = TRUE;

CREATE INDEX IF NOT EXISTS idx_risk_limit_profiles_active_lookup
    ON risk_limit_profiles (is_active, profile_key, scope_type, updated_at_utc DESC);

CREATE INDEX IF NOT EXISTS idx_risk_limit_profiles_scope_version
    ON risk_limit_profiles (scope_type, scope_id, version DESC);

CREATE INDEX IF NOT EXISTS idx_risk_limit_profiles_pending_approval
    ON risk_limit_profiles (approval_status, updated_at_utc DESC)
    WHERE approval_status = 'pending';

CREATE INDEX IF NOT EXISTS idx_risk_limit_profiles_actor_correlation
    ON risk_limit_profiles (actor_id, correlation_id, updated_at_utc DESC);

CREATE INDEX IF NOT EXISTS idx_risk_limit_profiles_latest_effective
    ON risk_limit_profiles (profile_key, scope_type, is_active DESC, version DESC, updated_at_utc DESC);

CREATE TABLE IF NOT EXISTS inventory_limit_rules (
    rule_id TEXT PRIMARY KEY,
    profile_key TEXT NOT NULL,
    profile_version BIGINT NOT NULL,
    scope_type TEXT NOT NULL,
    scope_id TEXT NOT NULL,
    max_position_units DOUBLE PRECISION NOT NULL,
    max_order_size_units DOUBLE PRECISION NOT NULL,
    max_concentration_pct_nav DOUBLE PRECISION NOT NULL,
    actor_id TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    updated_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    FOREIGN KEY (profile_key, profile_version, scope_type)
        REFERENCES risk_limit_profiles(profile_key, version, scope_type)
        ON DELETE CASCADE,
    CHECK (rule_id = lower(trim(rule_id))),
    CHECK (profile_key = lower(trim(profile_key))),
    CHECK (scope_id = lower(trim(scope_id))),
    CHECK (char_length(trim(rule_id)) > 0),
    CHECK (char_length(trim(profile_key)) > 0),
    CHECK (char_length(trim(scope_type)) > 0),
    CHECK (char_length(trim(scope_id)) > 0),
    CHECK (char_length(trim(actor_id)) > 0),
    CHECK (char_length(trim(correlation_id)) > 0),
    CHECK (scope_type IN ('market', 'strategy')),
    CHECK (profile_version > 0),
    CHECK (isfinite(max_position_units) AND max_position_units >= 0),
    CHECK (isfinite(max_order_size_units) AND max_order_size_units >= 0),
    CHECK (
        isfinite(max_concentration_pct_nav)
        AND max_concentration_pct_nav >= 0
        AND max_concentration_pct_nav <= 100
    ),
    CHECK (EXTRACT(TIMEZONE FROM updated_at_utc) = 0)
);

CREATE INDEX IF NOT EXISTS idx_inventory_limit_rules_profile_lookup
    ON inventory_limit_rules (profile_key, profile_version, scope_type, scope_id);

CREATE INDEX IF NOT EXISTS idx_inventory_limit_rules_actor_correlation
    ON inventory_limit_rules (actor_id, correlation_id, updated_at_utc DESC);
