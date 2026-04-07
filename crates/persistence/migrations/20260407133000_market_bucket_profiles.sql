CREATE TABLE IF NOT EXISTS market_bucket_profiles (
    profile_id TEXT PRIMARY KEY,
    market_id TEXT NOT NULL,
    cluster_id TEXT NOT NULL,
    bucket_type TEXT NOT NULL,
    risk_policy_key TEXT NOT NULL,
    allocation_policy_key TEXT NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    actor_id TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    updated_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (profile_id = lower(trim(profile_id))),
    CHECK (market_id = lower(trim(market_id))),
    CHECK (cluster_id = lower(trim(cluster_id))),
    CHECK (bucket_type IN ('core', 'satellite')),
    CHECK (risk_policy_key = lower(trim(risk_policy_key))),
    CHECK (allocation_policy_key = lower(trim(allocation_policy_key))),
    CHECK (char_length(trim(profile_id)) > 0),
    CHECK (char_length(trim(market_id)) > 0),
    CHECK (char_length(trim(cluster_id)) > 0),
    CHECK (char_length(trim(actor_id)) > 0),
    CHECK (char_length(trim(correlation_id)) > 0),
    CHECK (char_length(trim(risk_policy_key)) > 0),
    CHECK (char_length(trim(allocation_policy_key)) > 0),
    CHECK (EXTRACT(TIMEZONE FROM updated_at_utc) = 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_market_bucket_profiles_active_market_cluster_unique
    ON market_bucket_profiles (lower(trim(market_id)), lower(trim(cluster_id)))
    WHERE is_active = TRUE;

CREATE INDEX IF NOT EXISTS idx_market_bucket_profiles_active_lookup
    ON market_bucket_profiles (is_active, market_id, cluster_id, updated_at_utc DESC, profile_id ASC);

CREATE INDEX IF NOT EXISTS idx_market_bucket_profiles_correlation_lookup
    ON market_bucket_profiles (correlation_id, updated_at_utc DESC, profile_id ASC);
