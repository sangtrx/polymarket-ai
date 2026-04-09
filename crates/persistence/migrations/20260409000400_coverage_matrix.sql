CREATE TABLE IF NOT EXISTS coverage_snapshots (
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

CREATE TABLE IF NOT EXISTS coverage_rows (
    row_id TEXT PRIMARY KEY,
    snapshot_id TEXT NOT NULL REFERENCES coverage_snapshots (snapshot_id) ON DELETE CASCADE,
    canonical_requirement_id TEXT NOT NULL,
    coverage_class TEXT NOT NULL,
    reason_code TEXT NOT NULL,
    rationale TEXT NOT NULL,
    provenance TEXT NOT NULL,
    created_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (row_id = lower(trim(row_id))),
    CHECK (char_length(trim(row_id)) > 0),
    CHECK (char_length(trim(canonical_requirement_id)) > 0),
    CHECK (coverage_class IN ('covered', 'partial', 'missing')),
    CHECK (char_length(trim(reason_code)) > 0),
    CHECK (char_length(trim(rationale)) > 0),
    CHECK (char_length(trim(provenance)) > 0),
    CHECK (
        coverage_class = 'covered'
        OR (
            char_length(trim(reason_code)) > 0
            AND char_length(trim(rationale)) > 0
        )
    ),
    CHECK (EXTRACT(TIMEZONE FROM created_at_utc) = 0),
    UNIQUE (snapshot_id, canonical_requirement_id)
);

CREATE TABLE IF NOT EXISTS coverage_row_anchors (
    anchor_id TEXT PRIMARY KEY,
    row_id TEXT NOT NULL REFERENCES coverage_rows (row_id) ON DELETE CASCADE,
    bucket TEXT NOT NULL,
    evidence_type TEXT NOT NULL,
    file_path TEXT NOT NULL,
    symbol TEXT,
    section TEXT,
    line_start INTEGER,
    line_end INTEGER,
    created_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (anchor_id = lower(trim(anchor_id))),
    CHECK (char_length(trim(anchor_id)) > 0),
    CHECK (bucket IN ('code', 'test', 'ambiguous')),
    CHECK (evidence_type IN ('code', 'test')),
    CHECK (char_length(trim(file_path)) > 0),
    CHECK (
        (symbol IS NOT NULL AND char_length(trim(symbol)) > 0)
        OR (section IS NOT NULL AND char_length(trim(section)) > 0)
        OR line_start IS NOT NULL
    ),
    CHECK (line_end IS NULL OR line_start IS NOT NULL),
    CHECK (line_end IS NULL OR line_end >= line_start),
    CHECK (EXTRACT(TIMEZONE FROM created_at_utc) = 0)
);

CREATE INDEX IF NOT EXISTS idx_coverage_rows_snapshot_requirement
    ON coverage_rows (snapshot_id, canonical_requirement_id, coverage_class);

CREATE INDEX IF NOT EXISTS idx_coverage_anchors_row_lookup
    ON coverage_row_anchors (row_id, bucket, file_path, line_start);

CREATE OR REPLACE FUNCTION coverage_reject_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'coverage records are immutable once inserted';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS coverage_snapshots_immutable_update ON coverage_snapshots;
CREATE TRIGGER coverage_snapshots_immutable_update
BEFORE UPDATE OR DELETE ON coverage_snapshots
FOR EACH ROW EXECUTE FUNCTION coverage_reject_mutation();

DROP TRIGGER IF EXISTS coverage_rows_immutable_update ON coverage_rows;
CREATE TRIGGER coverage_rows_immutable_update
BEFORE UPDATE OR DELETE ON coverage_rows
FOR EACH ROW EXECUTE FUNCTION coverage_reject_mutation();

DROP TRIGGER IF EXISTS coverage_anchors_immutable_update ON coverage_row_anchors;
CREATE TRIGGER coverage_anchors_immutable_update
BEFORE UPDATE OR DELETE ON coverage_row_anchors
FOR EACH ROW EXECUTE FUNCTION coverage_reject_mutation();
