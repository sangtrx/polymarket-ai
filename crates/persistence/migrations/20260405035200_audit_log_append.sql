CREATE TABLE IF NOT EXISTS audit_log_append (
    audit_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    actor_id TEXT NOT NULL,
    role TEXT NOT NULL,
    action_type TEXT NOT NULL,
    parameters JSONB NOT NULL DEFAULT '{}'::jsonb,
    approval_reference TEXT NULL,
    timestamp TIMESTAMPTZ NOT NULL,
    outcome TEXT NOT NULL,
    reason_code TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    authentication_outcome TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (jsonb_typeof(parameters) = 'object'),
    CHECK (
        outcome IN (
            'allow',
            'authorization_denied',
            'authentication_denied'
        )
    ),
    CHECK (char_length(trim(actor_id)) > 0),
    CHECK (char_length(trim(role)) > 0),
    CHECK (char_length(trim(action_type)) > 0),
    CHECK (char_length(trim(reason_code)) > 0),
    CHECK (char_length(trim(correlation_id)) > 0),
    CHECK (char_length(trim(authentication_outcome)) > 0),
    CHECK (
        approval_reference IS NULL
        OR char_length(trim(approval_reference)) > 0
    )
);

CREATE INDEX IF NOT EXISTS idx_audit_log_append_timestamp
    ON audit_log_append (timestamp DESC);

CREATE INDEX IF NOT EXISTS idx_audit_log_append_actor_action
    ON audit_log_append (actor_id, action_type);

CREATE INDEX IF NOT EXISTS idx_audit_log_append_correlation
    ON audit_log_append (correlation_id);

CREATE OR REPLACE FUNCTION prevent_audit_log_append_mutation()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION 'audit_log_append is append-only'
        USING ERRCODE = '55000';
END;
$$;

DROP TRIGGER IF EXISTS trg_audit_log_append_no_update ON audit_log_append;
CREATE TRIGGER trg_audit_log_append_no_update
    BEFORE UPDATE ON audit_log_append
    FOR EACH ROW
    EXECUTE FUNCTION prevent_audit_log_append_mutation();

DROP TRIGGER IF EXISTS trg_audit_log_append_no_delete ON audit_log_append;
CREATE TRIGGER trg_audit_log_append_no_delete
    BEFORE DELETE ON audit_log_append
    FOR EACH ROW
    EXECUTE FUNCTION prevent_audit_log_append_mutation();
