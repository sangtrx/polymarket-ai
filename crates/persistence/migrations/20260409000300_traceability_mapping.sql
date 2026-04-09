CREATE TABLE IF NOT EXISTS traceability_snapshots (
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

CREATE TABLE IF NOT EXISTS traceability_links (
    link_id TEXT PRIMARY KEY,
    snapshot_id TEXT NOT NULL REFERENCES traceability_snapshots (snapshot_id) ON DELETE CASCADE,
    canonical_requirement_id TEXT NOT NULL,
    rationale TEXT NOT NULL,
    confidence TEXT NOT NULL,
    outcome TEXT NOT NULL,
    reason_code TEXT,
    created_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (link_id = lower(trim(link_id))),
    CHECK (char_length(trim(link_id)) > 0),
    CHECK (char_length(trim(canonical_requirement_id)) > 0),
    CHECK (char_length(trim(rationale)) > 0),
    CHECK (confidence IN ('high', 'medium', 'low')),
    CHECK (outcome IN ('linked', 'ambiguous', 'missing_evidence', 'stale_evidence')),
    CHECK (reason_code IS NULL OR char_length(trim(reason_code)) > 0),
    CHECK (EXTRACT(TIMEZONE FROM created_at_utc) = 0)
);

CREATE TABLE IF NOT EXISTS traceability_link_anchors (
    anchor_id TEXT PRIMARY KEY,
    link_id TEXT NOT NULL REFERENCES traceability_links (link_id) ON DELETE CASCADE,
    evidence_type TEXT NOT NULL,
    file_path TEXT NOT NULL,
    symbol TEXT,
    section TEXT,
    line_start INTEGER,
    line_end INTEGER,
    created_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (anchor_id = lower(trim(anchor_id))),
    CHECK (char_length(trim(anchor_id)) > 0),
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

CREATE INDEX IF NOT EXISTS idx_traceability_links_requirement_lookup
    ON traceability_links (canonical_requirement_id, snapshot_id, outcome, confidence);

CREATE INDEX IF NOT EXISTS idx_traceability_anchors_link_lookup
    ON traceability_link_anchors (link_id, evidence_type, file_path, line_start);

CREATE OR REPLACE FUNCTION traceability_reject_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'traceability records are immutable once inserted';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS traceability_snapshots_immutable_update ON traceability_snapshots;
CREATE TRIGGER traceability_snapshots_immutable_update
BEFORE UPDATE OR DELETE ON traceability_snapshots
FOR EACH ROW EXECUTE FUNCTION traceability_reject_mutation();

DROP TRIGGER IF EXISTS traceability_links_immutable_update ON traceability_links;
CREATE TRIGGER traceability_links_immutable_update
BEFORE UPDATE OR DELETE ON traceability_links
FOR EACH ROW EXECUTE FUNCTION traceability_reject_mutation();

DROP TRIGGER IF EXISTS traceability_anchors_immutable_update ON traceability_link_anchors;
CREATE TRIGGER traceability_anchors_immutable_update
BEFORE UPDATE OR DELETE ON traceability_link_anchors
FOR EACH ROW EXECUTE FUNCTION traceability_reject_mutation();

CREATE OR REPLACE FUNCTION traceability_require_code_anchor()
RETURNS TRIGGER AS $$
DECLARE
    requires_code_anchor BOOLEAN;
    has_code_anchor BOOLEAN;
BEGIN
    SELECT outcome <> 'missing_evidence'
    INTO requires_code_anchor
    FROM traceability_links
    WHERE link_id = COALESCE(NEW.link_id, OLD.link_id);

    IF requires_code_anchor IS NULL OR requires_code_anchor = FALSE THEN
        RETURN NULL;
    END IF;

    SELECT EXISTS (
        SELECT 1
        FROM traceability_link_anchors
        WHERE link_id = COALESCE(NEW.link_id, OLD.link_id)
          AND evidence_type = 'code'
    )
    INTO has_code_anchor;

    IF NOT has_code_anchor THEN
        RAISE EXCEPTION 'traceability link requires at least one code anchor'
            USING ERRCODE = '23514';
    END IF;

    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS traceability_links_require_code_anchor ON traceability_links;
CREATE CONSTRAINT TRIGGER traceability_links_require_code_anchor
AFTER INSERT ON traceability_links
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION traceability_require_code_anchor();

DROP TRIGGER IF EXISTS traceability_anchors_require_code_anchor ON traceability_link_anchors;
CREATE CONSTRAINT TRIGGER traceability_anchors_require_code_anchor
AFTER INSERT OR DELETE ON traceability_link_anchors
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION traceability_require_code_anchor();
