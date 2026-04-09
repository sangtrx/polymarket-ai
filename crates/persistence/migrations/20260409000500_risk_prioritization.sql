CREATE TABLE IF NOT EXISTS risk_snapshots (
    snapshot_id TEXT PRIMARY KEY,
    commit_sha TEXT NOT NULL,
    generated_at_utc TIMESTAMPTZ NOT NULL,
    created_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (snapshot_id = lower(trim(snapshot_id))),
    CHECK (char_length(trim(snapshot_id)) > 0),
    CHECK (char_length(trim(commit_sha)) > 0),
    CHECK (EXTRACT(TIMEZONE FROM generated_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM created_at_utc) = 0),
    UNIQUE (commit_sha, generated_at_utc)
);

CREATE TABLE IF NOT EXISTS risk_rows (
    row_id TEXT PRIMARY KEY,
    snapshot_id TEXT NOT NULL REFERENCES risk_snapshots (snapshot_id) ON DELETE CASCADE,
    canonical_requirement_id TEXT NOT NULL,
    coverage_class TEXT NOT NULL,
    severity TEXT NOT NULL,
    risk_score INTEGER NOT NULL,
    priority_rank INTEGER NOT NULL,
    reason_code TEXT NOT NULL,
    rationale TEXT NOT NULL,
    provenance TEXT NOT NULL,
    code_anchor_count INTEGER NOT NULL,
    test_anchor_count INTEGER NOT NULL,
    ambiguous_anchor_count INTEGER NOT NULL,
    remediation_focus TEXT NOT NULL,
    severity_weight INTEGER NOT NULL,
    reason_weight INTEGER NOT NULL,
    evidence_penalty INTEGER NOT NULL,
    created_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (row_id = lower(trim(row_id))),
    CHECK (char_length(trim(row_id)) > 0),
    CHECK (char_length(trim(canonical_requirement_id)) > 0),
    CHECK (coverage_class IN ('partial', 'missing')),
    CHECK (severity IN ('critical', 'high', 'medium', 'low')),
    CHECK (priority_rank > 0),
    CHECK (char_length(trim(reason_code)) > 0),
    CHECK (char_length(trim(rationale)) > 0),
    CHECK (char_length(trim(provenance)) > 0),
    CHECK (code_anchor_count >= 0),
    CHECK (test_anchor_count >= 0),
    CHECK (ambiguous_anchor_count >= 0),
    CHECK (
        remediation_focus IN (
            'add_evidence',
            'restore_traceability',
            'confirm_semantic_coverage',
            'verify_deterministic_link',
            'review_reason_mapping'
        )
    ),
    CHECK (EXTRACT(TIMEZONE FROM created_at_utc) = 0),
    UNIQUE (snapshot_id, canonical_requirement_id),
    UNIQUE (snapshot_id, priority_rank)
);

CREATE TABLE IF NOT EXISTS risk_row_anchors (
    anchor_id TEXT PRIMARY KEY,
    row_id TEXT NOT NULL REFERENCES risk_rows (row_id) ON DELETE CASCADE,
    anchor_rank INTEGER NOT NULL,
    evidence_type TEXT NOT NULL,
    file_path TEXT NOT NULL,
    symbol TEXT,
    section TEXT,
    line_start INTEGER,
    line_end INTEGER,
    created_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (anchor_id = lower(trim(anchor_id))),
    CHECK (char_length(trim(anchor_id)) > 0),
    CHECK (anchor_rank > 0),
    CHECK (evidence_type IN ('code', 'test')),
    CHECK (char_length(trim(file_path)) > 0),
    CHECK (
        (symbol IS NOT NULL AND char_length(trim(symbol)) > 0)
        OR (section IS NOT NULL AND char_length(trim(section)) > 0)
        OR line_start IS NOT NULL
    ),
    CHECK (line_end IS NULL OR line_start IS NOT NULL),
    CHECK (line_end IS NULL OR line_end >= line_start),
    CHECK (EXTRACT(TIMEZONE FROM created_at_utc) = 0),
    UNIQUE (row_id, anchor_rank)
);

CREATE INDEX IF NOT EXISTS idx_risk_rows_snapshot_priority
    ON risk_rows (snapshot_id, priority_rank, canonical_requirement_id);

CREATE INDEX IF NOT EXISTS idx_risk_rows_snapshot_requirement
    ON risk_rows (snapshot_id, canonical_requirement_id);

CREATE INDEX IF NOT EXISTS idx_risk_row_anchors_row_rank
    ON risk_row_anchors (row_id, anchor_rank);

CREATE OR REPLACE FUNCTION risk_reject_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'risk records are immutable once inserted';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS risk_snapshots_immutable_update ON risk_snapshots;
CREATE TRIGGER risk_snapshots_immutable_update
BEFORE UPDATE OR DELETE ON risk_snapshots
FOR EACH ROW EXECUTE FUNCTION risk_reject_mutation();

DROP TRIGGER IF EXISTS risk_rows_immutable_update ON risk_rows;
CREATE TRIGGER risk_rows_immutable_update
BEFORE UPDATE OR DELETE ON risk_rows
FOR EACH ROW EXECUTE FUNCTION risk_reject_mutation();

DROP TRIGGER IF EXISTS risk_row_anchors_immutable_update ON risk_row_anchors;
CREATE TRIGGER risk_row_anchors_immutable_update
BEFORE UPDATE OR DELETE ON risk_row_anchors
FOR EACH ROW EXECUTE FUNCTION risk_reject_mutation();
