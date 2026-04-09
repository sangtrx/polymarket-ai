CREATE TABLE IF NOT EXISTS readiness_snapshots (
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

CREATE TABLE IF NOT EXISTS readiness_waivers (
    waiver_id TEXT PRIMARY KEY,
    snapshot_id TEXT NOT NULL REFERENCES readiness_snapshots (snapshot_id) ON DELETE CASCADE,
    risk_snapshot_id TEXT NOT NULL REFERENCES risk_snapshots (snapshot_id) ON DELETE RESTRICT,
    canonical_requirement_id TEXT NOT NULL,
    owner TEXT NOT NULL,
    reason_code TEXT NOT NULL,
    justification TEXT NOT NULL,
    approved_by TEXT NOT NULL,
    created_at_utc TIMESTAMPTZ NOT NULL,
    expires_at_utc TIMESTAMPTZ NOT NULL,
    recorded_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (waiver_id = lower(trim(waiver_id))),
    CHECK (char_length(trim(waiver_id)) > 0),
    CHECK (char_length(trim(canonical_requirement_id)) > 0),
    CHECK (char_length(trim(owner)) > 0),
    CHECK (char_length(trim(reason_code)) > 0),
    CHECK (char_length(trim(justification)) > 0),
    CHECK (char_length(trim(approved_by)) > 0),
    CHECK (expires_at_utc > created_at_utc),
    CHECK (EXTRACT(TIMEZONE FROM created_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM expires_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM recorded_at_utc) = 0),
    UNIQUE (snapshot_id, canonical_requirement_id, owner)
);

CREATE TABLE IF NOT EXISTS readiness_waiver_revocations (
    revocation_id TEXT PRIMARY KEY,
    waiver_id TEXT NOT NULL REFERENCES readiness_waivers (waiver_id) ON DELETE CASCADE,
    revoked_by TEXT NOT NULL,
    revoked_reason_code TEXT NOT NULL,
    revoked_at_utc TIMESTAMPTZ NOT NULL,
    recorded_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (revocation_id = lower(trim(revocation_id))),
    CHECK (char_length(trim(revocation_id)) > 0),
    CHECK (char_length(trim(revoked_by)) > 0),
    CHECK (char_length(trim(revoked_reason_code)) > 0),
    CHECK (EXTRACT(TIMEZONE FROM revoked_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM recorded_at_utc) = 0),
    UNIQUE (waiver_id)
);

CREATE INDEX IF NOT EXISTS idx_readiness_waivers_snapshot_order
    ON readiness_waivers (snapshot_id, expires_at_utc, canonical_requirement_id, waiver_id);

CREATE INDEX IF NOT EXISTS idx_readiness_waivers_risk_snapshot_requirement
    ON readiness_waivers (risk_snapshot_id, canonical_requirement_id);

CREATE INDEX IF NOT EXISTS idx_readiness_waiver_revocations_waiver
    ON readiness_waiver_revocations (waiver_id, revoked_at_utc);

CREATE OR REPLACE FUNCTION readiness_validate_waiver_unresolved()
RETURNS TRIGGER AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM risk_rows
        WHERE snapshot_id = NEW.risk_snapshot_id
          AND canonical_requirement_id = NEW.canonical_requirement_id
          AND coverage_class IN ('partial', 'missing')
    ) THEN
        RAISE EXCEPTION 'waiver canonical_requirement_id must reference unresolved risk row'
            USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS readiness_waivers_unresolved_only ON readiness_waivers;
CREATE TRIGGER readiness_waivers_unresolved_only
BEFORE INSERT ON readiness_waivers
FOR EACH ROW EXECUTE FUNCTION readiness_validate_waiver_unresolved();

CREATE OR REPLACE FUNCTION readiness_reject_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'readiness waiver records are immutable once inserted';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS readiness_snapshots_immutable_update ON readiness_snapshots;
CREATE TRIGGER readiness_snapshots_immutable_update
BEFORE UPDATE OR DELETE ON readiness_snapshots
FOR EACH ROW EXECUTE FUNCTION readiness_reject_mutation();

DROP TRIGGER IF EXISTS readiness_waivers_immutable_update ON readiness_waivers;
CREATE TRIGGER readiness_waivers_immutable_update
BEFORE UPDATE OR DELETE ON readiness_waivers
FOR EACH ROW EXECUTE FUNCTION readiness_reject_mutation();

DROP TRIGGER IF EXISTS readiness_waiver_revocations_immutable_update ON readiness_waiver_revocations;
CREATE TRIGGER readiness_waiver_revocations_immutable_update
BEFORE UPDATE OR DELETE ON readiness_waiver_revocations
FOR EACH ROW EXECUTE FUNCTION readiness_reject_mutation();
