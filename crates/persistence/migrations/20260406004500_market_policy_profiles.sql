CREATE TABLE IF NOT EXISTS market_policy_profiles (
    profile_id TEXT PRIMARY KEY,
    cluster_id TEXT NOT NULL,
    min_liquidity_usd DOUBLE PRECISION NOT NULL CHECK (min_liquidity_usd >= 0),
    max_spread_bps DOUBLE PRECISION NOT NULL CHECK (max_spread_bps >= 0),
    min_reward_score DOUBLE PRECISION NOT NULL CHECK (min_reward_score >= 0),
    max_exposure_pct_nav DOUBLE PRECISION NOT NULL CHECK (
        max_exposure_pct_nav >= 0
        AND max_exposure_pct_nav <= 100
    ),
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    actor_id TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    updated_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (cluster_id = lower(trim(cluster_id))),
    CHECK (char_length(trim(profile_id)) > 0),
    CHECK (char_length(trim(cluster_id)) > 0),
    CHECK (char_length(trim(actor_id)) > 0),
    CHECK (char_length(trim(correlation_id)) > 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_market_policy_profiles_active_cluster_unique
    ON market_policy_profiles (lower(trim(cluster_id)))
    WHERE is_active = TRUE;

CREATE INDEX IF NOT EXISTS idx_market_policy_profiles_active_lookup
    ON market_policy_profiles (is_active, lower(trim(cluster_id)), updated_at_utc DESC);

CREATE TABLE IF NOT EXISTS market_cluster_overrides (
    cluster_id TEXT PRIMARY KEY,
    is_enabled BOOLEAN NOT NULL,
    reason_code TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    updated_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (cluster_id = lower(trim(cluster_id))),
    CHECK (char_length(trim(cluster_id)) > 0),
    CHECK (char_length(trim(reason_code)) > 0),
    CHECK (char_length(trim(actor_id)) > 0),
    CHECK (char_length(trim(correlation_id)) > 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_market_cluster_overrides_cluster_canonical_unique
    ON market_cluster_overrides (lower(trim(cluster_id)));

CREATE INDEX IF NOT EXISTS idx_market_cluster_overrides_runtime_state
    ON market_cluster_overrides (is_enabled, updated_at_utc DESC);
