CREATE TABLE IF NOT EXISTS operator_workflow_requests (
    workflow_id UUID PRIMARY KEY,
    operator_id TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    workflow_kind TEXT NOT NULL CHECK (workflow_kind IN ('readiness_snapshot')),
    workflow_status TEXT NOT NULL CHECK (workflow_status IN ('queued', 'running', 'succeeded', 'failed')),
    result JSONB,
    error_code TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT operator_workflow_idempotency UNIQUE (operator_id, workflow_kind, idempotency_key)
);

CREATE INDEX IF NOT EXISTS operator_workflow_requests_status_updated_idx
    ON operator_workflow_requests (workflow_status, updated_at DESC);

COMMENT ON TABLE operator_workflow_requests IS
    'Operator-BFF-owned orchestration/audit state only. Trading, risk, research, and governance domain authority remains in Rust services.';
