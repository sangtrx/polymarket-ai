CREATE TABLE IF NOT EXISTS reward_risk_policies (
    policy_key TEXT PRIMARY KEY,
    strategy_key TEXT NOT NULL,
    min_reward_per_risk DOUBLE PRECISION NOT NULL,
    actor_id TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    updated_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (policy_key = lower(trim(policy_key))),
    CHECK (strategy_key = lower(trim(strategy_key))),
    CHECK (char_length(trim(policy_key)) > 0),
    CHECK (char_length(trim(strategy_key)) > 0),
    CHECK (char_length(trim(actor_id)) > 0),
    CHECK (char_length(trim(correlation_id)) > 0),
    CHECK (isfinite(min_reward_per_risk) AND min_reward_per_risk >= 0),
    CHECK (EXTRACT(TIMEZONE FROM updated_at_utc) = 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_reward_risk_policies_policy_key_canonical_unique
    ON reward_risk_policies (lower(trim(policy_key)));

CREATE INDEX IF NOT EXISTS idx_reward_risk_policies_strategy_lookup
    ON reward_risk_policies (strategy_key, updated_at_utc DESC);

CREATE INDEX IF NOT EXISTS idx_reward_risk_policies_actor_correlation
    ON reward_risk_policies (actor_id, correlation_id, updated_at_utc DESC);
