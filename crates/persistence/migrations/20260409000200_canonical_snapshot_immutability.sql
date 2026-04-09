CREATE OR REPLACE FUNCTION canonical_snapshot_metadata_immutable_guard()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    IF NEW.commit_sha IS DISTINCT FROM OLD.commit_sha
       OR NEW.ingested_at_utc IS DISTINCT FROM OLD.ingested_at_utc
       OR NEW.file_digests_json IS DISTINCT FROM OLD.file_digests_json
       OR NEW.aggregate_digest IS DISTINCT FROM OLD.aggregate_digest THEN
        RAISE EXCEPTION 'canonical_ingestion_snapshots metadata is immutable after insert'
            USING ERRCODE = '55000';
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS canonical_snapshot_metadata_immutable_trigger
    ON canonical_ingestion_snapshots;

CREATE TRIGGER canonical_snapshot_metadata_immutable_trigger
    BEFORE UPDATE ON canonical_ingestion_snapshots
    FOR EACH ROW
    EXECUTE FUNCTION canonical_snapshot_metadata_immutable_guard();
