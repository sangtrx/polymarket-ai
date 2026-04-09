CREATE TABLE IF NOT EXISTS canonical_ingestion_snapshots (
    snapshot_id TEXT PRIMARY KEY,
    commit_sha TEXT NOT NULL,
    ingested_at_utc TIMESTAMPTZ NOT NULL,
    file_digests_json JSONB NOT NULL,
    aggregate_digest TEXT NOT NULL,
    created_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (snapshot_id = lower(trim(snapshot_id))),
    CHECK (char_length(trim(snapshot_id)) > 0),
    CHECK (char_length(trim(commit_sha)) > 0),
    CHECK (char_length(trim(aggregate_digest)) > 0),
    CHECK (jsonb_typeof(file_digests_json) = 'object'),
    CHECK (EXTRACT(TIMEZONE FROM ingested_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM created_at_utc) = 0),
    UNIQUE (commit_sha, ingested_at_utc)
);

CREATE TABLE IF NOT EXISTS canonical_artifact_items (
    canonical_requirement_id TEXT PRIMARY KEY,
    snapshot_id TEXT NOT NULL REFERENCES canonical_ingestion_snapshots (snapshot_id) ON DELETE CASCADE,
    artifact_type TEXT NOT NULL,
    artifact_path TEXT NOT NULL,
    heading_slug TEXT NOT NULL,
    item_index INTEGER NOT NULL,
    source_item_id TEXT,
    body TEXT NOT NULL,
    created_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (canonical_requirement_id = lower(trim(canonical_requirement_id))),
    CHECK (
        canonical_requirement_id ~ '^[a-z0-9_]+\\.[a-z0-9][a-z0-9_-]*\\.[0-9]+$'
    ),
    CHECK (artifact_type IN ('prd', 'architecture', 'story', 'roadmap')),
    CHECK (char_length(trim(artifact_path)) > 0),
    CHECK (char_length(trim(heading_slug)) > 0),
    CHECK (item_index > 0),
    CHECK (char_length(trim(body)) > 0),
    CHECK (EXTRACT(TIMEZONE FROM created_at_utc) = 0)
);

CREATE TABLE IF NOT EXISTS canonical_item_equivalences (
    equivalence_id TEXT PRIMARY KEY,
    snapshot_id TEXT NOT NULL REFERENCES canonical_ingestion_snapshots (snapshot_id) ON DELETE CASCADE,
    left_canonical_requirement_id TEXT NOT NULL REFERENCES canonical_artifact_items (canonical_requirement_id) ON DELETE CASCADE,
    right_canonical_requirement_id TEXT NOT NULL REFERENCES canonical_artifact_items (canonical_requirement_id) ON DELETE CASCADE,
    rationale TEXT NOT NULL,
    created_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (equivalence_id = lower(trim(equivalence_id))),
    CHECK (char_length(trim(rationale)) > 0),
    CHECK (left_canonical_requirement_id <> right_canonical_requirement_id),
    CHECK (EXTRACT(TIMEZONE FROM created_at_utc) = 0),
    UNIQUE (snapshot_id, left_canonical_requirement_id, right_canonical_requirement_id)
);

CREATE TABLE IF NOT EXISTS canonical_item_conflicts (
    conflict_id TEXT PRIMARY KEY,
    snapshot_id TEXT NOT NULL REFERENCES canonical_ingestion_snapshots (snapshot_id) ON DELETE CASCADE,
    left_canonical_requirement_id TEXT NOT NULL REFERENCES canonical_artifact_items (canonical_requirement_id) ON DELETE CASCADE,
    right_canonical_requirement_id TEXT NOT NULL REFERENCES canonical_artifact_items (canonical_requirement_id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'unresolved',
    rationale TEXT NOT NULL,
    created_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (conflict_id = lower(trim(conflict_id))),
    CHECK (status = 'unresolved'),
    CHECK (char_length(trim(rationale)) > 0),
    CHECK (left_canonical_requirement_id <> right_canonical_requirement_id),
    CHECK (EXTRACT(TIMEZONE FROM created_at_utc) = 0)
);

CREATE INDEX IF NOT EXISTS idx_canonical_snapshots_commit_lookup
    ON canonical_ingestion_snapshots (commit_sha, ingested_at_utc DESC, snapshot_id ASC);

CREATE INDEX IF NOT EXISTS idx_canonical_items_snapshot_lookup
    ON canonical_artifact_items (snapshot_id, artifact_type, heading_slug, item_index ASC);

CREATE INDEX IF NOT EXISTS idx_canonical_equivalences_snapshot_lookup
    ON canonical_item_equivalences (
        snapshot_id,
        left_canonical_requirement_id,
        right_canonical_requirement_id
    );

CREATE INDEX IF NOT EXISTS idx_canonical_conflicts_snapshot_lookup
    ON canonical_item_conflicts (
        snapshot_id,
        status,
        left_canonical_requirement_id,
        right_canonical_requirement_id
    );
