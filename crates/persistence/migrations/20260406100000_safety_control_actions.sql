CREATE TABLE IF NOT EXISTS safety_control_actions (
    action_id TEXT PRIMARY KEY,
    correlation_id TEXT NOT NULL,
    source TEXT NOT NULL,
    action TEXT NOT NULL,
    trigger_source TEXT NOT NULL,
    actor_id TEXT,
    actor_role TEXT,
    resulting_mode TEXT NOT NULL,
    reason_code TEXT NOT NULL,
    audit_reference TEXT NOT NULL,
    dedupe_key TEXT NOT NULL,
    requested_at_utc TIMESTAMPTZ NOT NULL,
    acknowledged_at_utc TIMESTAMPTZ NOT NULL,
    effective_at_utc TIMESTAMPTZ NOT NULL,
    completed_at_utc TIMESTAMPTZ NOT NULL,
    recorded_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    details JSONB NOT NULL DEFAULT '{}'::jsonb,
    CHECK (action_id = lower(trim(action_id))),
    CHECK (correlation_id = lower(trim(correlation_id))),
    CHECK (dedupe_key = lower(trim(dedupe_key))),
    CHECK (actor_id IS NULL OR actor_id = lower(trim(actor_id))),
    CHECK (actor_role IS NULL OR actor_role = lower(trim(actor_role))),
    CHECK (char_length(trim(action_id)) > 0),
    CHECK (char_length(trim(correlation_id)) > 0),
    CHECK (char_length(trim(reason_code)) > 0),
    CHECK (char_length(trim(audit_reference)) > 0),
    CHECK (char_length(trim(dedupe_key)) > 0),
    CHECK (actor_id IS NULL OR char_length(trim(actor_id)) > 0),
    CHECK (actor_role IS NULL OR char_length(trim(actor_role)) > 0),
    CHECK (source IN ('manual', 'automatic')),
    CHECK (action IN ('pause', 'reduce_only', 'cancel_all')),
    CHECK (
        trigger_source IN (
            'operator_command',
            'stale_feed',
            'reconciliation_critical',
            'control_uncertainty'
        )
    ),
    CHECK (resulting_mode IN ('normal', 'paused', 'reduce_only')),
    CHECK (
        reason_code IN (
            'emergency_control_pause_activated',
            'emergency_control_pause_active',
            'emergency_control_reduce_only_activated',
            'emergency_control_reduce_only_active',
            'emergency_control_cancel_all_accepted',
            'emergency_control_stale_feed_triggered',
            'emergency_control_reconciliation_critical_triggered',
            'emergency_control_control_uncertainty_triggered',
            'emergency_control_persistence_unavailable',
            'emergency_control_orchestration_unavailable',
            'emergency_control_unauthorized_role',
            'emergency_control_not_found',
            'emergency_control_invalid_payload'
        )
    ),
    CHECK (jsonb_typeof(details) = 'object'),
    CHECK (EXTRACT(TIMEZONE FROM requested_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM acknowledged_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM effective_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM completed_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM recorded_at_utc) = 0),
    CHECK (acknowledged_at_utc >= requested_at_utc),
    CHECK (effective_at_utc >= requested_at_utc),
    CHECK (completed_at_utc >= effective_at_utc),
    CHECK (EXTRACT(EPOCH FROM acknowledged_at_utc - requested_at_utc) <= 1),
    CHECK (EXTRACT(EPOCH FROM effective_at_utc - requested_at_utc) <= 5),
    CHECK (
        (
            source = 'manual'
            AND trigger_source = 'operator_command'
            AND actor_id IS NOT NULL
            AND actor_role IS NOT NULL
        )
        OR (
            source = 'automatic'
            AND trigger_source IN ('stale_feed', 'reconciliation_critical', 'control_uncertainty')
        )
    ),
    CHECK (
        (action = 'pause' AND resulting_mode = 'paused')
        OR (action = 'reduce_only' AND resulting_mode = 'reduce_only')
        OR (action = 'cancel_all' AND resulting_mode = 'paused')
    )
);

CREATE INDEX IF NOT EXISTS idx_safety_control_actions_action_id
    ON safety_control_actions (action_id);

CREATE UNIQUE INDEX IF NOT EXISTS idx_safety_control_actions_dedupe_key
    ON safety_control_actions (dedupe_key);

CREATE INDEX IF NOT EXISTS idx_safety_control_actions_correlation_reason_source_time
    ON safety_control_actions (
        correlation_id,
        reason_code,
        source,
        effective_at_utc DESC,
        action_id DESC
    );

CREATE INDEX IF NOT EXISTS idx_safety_control_actions_resulting_mode_effective_time
    ON safety_control_actions (resulting_mode, effective_at_utc DESC, action_id DESC);
