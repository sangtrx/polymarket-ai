CREATE TABLE IF NOT EXISTS credential_rotation_events (
    rotation_id TEXT PRIMARY KEY,
    trigger_type TEXT NOT NULL,
    status TEXT NOT NULL,
    reason_code TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    credential_scope TEXT NOT NULL,
    credential_reference TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    rotation_reference TEXT NULL,
    initiated_at_utc TIMESTAMPTZ NOT NULL,
    deadline_at_utc TIMESTAMPTZ NULL,
    completed_at_utc TIMESTAMPTZ NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (trigger_type IN ('scheduled_cadence', 'emergency_compromise')),
    CHECK (status IN ('pending', 'in_progress', 'succeeded', 'denied', 'failed')),
    CHECK (jsonb_typeof(metadata) = 'object'),
    CHECK (char_length(trim(rotation_id)) > 0),
    CHECK (char_length(trim(reason_code)) > 0),
    CHECK (char_length(trim(actor_id)) > 0),
    CHECK (char_length(trim(credential_scope)) > 0),
    CHECK (char_length(trim(credential_reference)) > 0),
    CHECK (char_length(trim(correlation_id)) > 0),
    CHECK (
        rotation_reference IS NULL
        OR char_length(trim(rotation_reference)) > 0
    ),
    CHECK (
        completed_at_utc IS NULL
        OR completed_at_utc >= initiated_at_utc
    ),
    CHECK (
        deadline_at_utc IS NULL
        OR deadline_at_utc >= initiated_at_utc
    ),
    CHECK (
        status <> 'succeeded'
        OR (
            completed_at_utc IS NOT NULL
            AND rotation_reference IS NOT NULL
            AND char_length(trim(rotation_reference)) > 0
        )
    ),
    CHECK (
        metadata::text !~* '(secret|password|private[_-]?key|token|credential[_-]?value)'
    )
);

CREATE INDEX IF NOT EXISTS idx_credential_rotation_events_trigger_status
    ON credential_rotation_events (trigger_type, status);

CREATE INDEX IF NOT EXISTS idx_credential_rotation_events_initiated_at
    ON credential_rotation_events (initiated_at_utc DESC);

CREATE INDEX IF NOT EXISTS idx_credential_rotation_events_actor_id
    ON credential_rotation_events (actor_id, initiated_at_utc DESC);

CREATE INDEX IF NOT EXISTS idx_credential_rotation_events_correlation_id
    ON credential_rotation_events (correlation_id);

CREATE UNIQUE INDEX IF NOT EXISTS idx_credential_rotation_events_reference_unique
    ON credential_rotation_events (lower(trim(rotation_reference)))
    WHERE rotation_reference IS NOT NULL;
