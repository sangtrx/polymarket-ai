CREATE TABLE IF NOT EXISTS approval_requests (
    request_id TEXT PRIMARY KEY,
    action_id TEXT NOT NULL,
    proposer_actor_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    reason_code TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    approval_reference TEXT NULL,
    created_at_utc TIMESTAMPTZ NOT NULL,
    expires_at_utc TIMESTAMPTZ NOT NULL,
    updated_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (char_length(trim(request_id)) > 0),
    CHECK (char_length(trim(action_id)) > 0),
    CHECK (char_length(trim(proposer_actor_id)) > 0),
    CHECK (char_length(trim(reason_code)) > 0),
    CHECK (char_length(trim(correlation_id)) > 0),
    CHECK (status IN ('pending', 'approved', 'rejected', 'expired')),
    CHECK (
        action_id IN (
            'strategy_promotion_override',
            'risk_limit_increase',
            'kill_switch_disable',
            'production_config_change'
        )
    ),
    CHECK (expires_at_utc > created_at_utc),
    CHECK (
        (
            status = 'approved'
            AND approval_reference IS NOT NULL
            AND char_length(trim(approval_reference)) > 0
        )
        OR (
            status <> 'approved'
            AND approval_reference IS NULL
        )
    )
);

CREATE TABLE IF NOT EXISTS approval_votes (
    request_id TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    decision TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    voted_at_utc TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (request_id, actor_id),
    FOREIGN KEY (request_id) REFERENCES approval_requests(request_id) ON DELETE RESTRICT,
    CHECK (char_length(trim(actor_id)) > 0),
    CHECK (char_length(trim(correlation_id)) > 0),
    CHECK (decision IN ('approve', 'reject'))
);

CREATE INDEX IF NOT EXISTS idx_approval_requests_actor_created_at
    ON approval_requests (proposer_actor_id, created_at_utc DESC);

CREATE INDEX IF NOT EXISTS idx_approval_requests_action_status_created_at
    ON approval_requests (action_id, status, created_at_utc DESC);

CREATE INDEX IF NOT EXISTS idx_approval_requests_expires_at
    ON approval_requests (expires_at_utc);

CREATE INDEX IF NOT EXISTS idx_approval_votes_request_voted_at
    ON approval_votes (request_id, voted_at_utc DESC);

CREATE UNIQUE INDEX IF NOT EXISTS idx_approval_votes_request_actor_canonical_unique
    ON approval_votes (request_id, lower(trim(actor_id)));
