CREATE TABLE IF NOT EXISTS api_contract_versions (
    contract_key TEXT NOT NULL,
    contract_version TEXT NOT NULL,
    lifecycle_status TEXT NOT NULL,
    release_at_utc TIMESTAMPTZ NOT NULL,
    deprecation_notice_at_utc TIMESTAMPTZ,
    backward_compatible_until_utc TIMESTAMPTZ,
    sunset_at_utc TIMESTAMPTZ,
    replacement_contract_version TEXT,
    schema_artifact_path TEXT NOT NULL,
    changelog_artifact_path TEXT NOT NULL,
    schema_checksum_sha256 TEXT NOT NULL,
    changelog_checksum_sha256 TEXT NOT NULL,
    record_checksum_sha256 TEXT NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT FALSE,
    created_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (contract_key, contract_version),
    CONSTRAINT api_contract_versions_lifecycle_status_check
        CHECK (lifecycle_status IN ('active', 'deprecated', 'replaced', 'sunset')),
    CONSTRAINT api_contract_versions_contract_key_check
        CHECK (contract_key ~ '^[a-z0-9._:-]{3,160}$'),
    CONSTRAINT api_contract_versions_contract_version_check
        CHECK (contract_version ~ '^v[0-9]+(?:\\.[0-9]+){0,2}$'),
    CONSTRAINT api_contract_versions_schema_artifact_path_check
        CHECK (length(trim(schema_artifact_path)) > 0),
    CONSTRAINT api_contract_versions_changelog_artifact_path_check
        CHECK (length(trim(changelog_artifact_path)) > 0),
    CONSTRAINT api_contract_versions_schema_checksum_check
        CHECK (schema_checksum_sha256 ~ '^[a-f0-9]{64}$'),
    CONSTRAINT api_contract_versions_changelog_checksum_check
        CHECK (changelog_checksum_sha256 ~ '^[a-f0-9]{64}$'),
    CONSTRAINT api_contract_versions_record_checksum_check
        CHECK (record_checksum_sha256 ~ '^[a-f0-9]{64}$'),
    CONSTRAINT api_contract_versions_deprecation_notice_window_check
        CHECK (
            deprecation_notice_at_utc IS NULL
            OR deprecation_notice_at_utc >= release_at_utc + INTERVAL '90 days'
        ),
    CONSTRAINT api_contract_versions_support_window_check
        CHECK (
            backward_compatible_until_utc IS NULL
            OR (
                deprecation_notice_at_utc IS NOT NULL
                AND backward_compatible_until_utc >= deprecation_notice_at_utc + INTERVAL '180 days'
            )
        ),
    CONSTRAINT api_contract_versions_sunset_window_check
        CHECK (
            sunset_at_utc IS NULL
            OR (
                backward_compatible_until_utc IS NOT NULL
                AND sunset_at_utc >= backward_compatible_until_utc
            )
        ),
    CONSTRAINT api_contract_versions_non_active_requires_notice_check
        CHECK (lifecycle_status = 'active' OR deprecation_notice_at_utc IS NOT NULL),
    CONSTRAINT api_contract_versions_replaced_requires_replacement_check
        CHECK (lifecycle_status <> 'replaced' OR replacement_contract_version IS NOT NULL),
    CONSTRAINT api_contract_versions_active_status_check
        CHECK (NOT is_active OR lifecycle_status = 'active')
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_api_contract_versions_active_contract
    ON api_contract_versions (contract_key)
    WHERE is_active;

CREATE INDEX IF NOT EXISTS idx_api_contract_versions_lookup
    ON api_contract_versions (contract_key, contract_version);

CREATE INDEX IF NOT EXISTS idx_api_contract_versions_active_lookup
    ON api_contract_versions (contract_key, is_active, release_at_utc DESC);

CREATE INDEX IF NOT EXISTS idx_api_contract_versions_lifecycle_window
    ON api_contract_versions (
        contract_key,
        release_at_utc DESC,
        deprecation_notice_at_utc,
        backward_compatible_until_utc,
        sunset_at_utc
    );
