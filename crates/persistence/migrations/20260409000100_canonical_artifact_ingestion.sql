CREATE TABLE IF NOT EXISTS canonical_ingestion_snapshots (
    snapshot_id TEXT PRIMARY KEY,
    commit_sha TEXT NOT NULL,
    ingested_at_utc TIMESTAMPTZ NOT NULL,
    aggregate_digest TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS canonical_artifact_items (
    canonical_requirement_id TEXT PRIMARY KEY,
    snapshot_id TEXT NOT NULL REFERENCES canonical_ingestion_snapshots (snapshot_id) ON DELETE CASCADE,
    artifact_type TEXT NOT NULL,
    artifact_path TEXT NOT NULL,
    heading_slug TEXT NOT NULL,
    item_index INTEGER NOT NULL
);
