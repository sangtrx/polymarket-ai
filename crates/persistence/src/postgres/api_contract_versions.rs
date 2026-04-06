use domain::reporting::{
    ReportingContractError, ReportingReasonCode, ReportingValidationIssue,
    normalize_reporting_identifier, parse_utc_timestamp,
};
use sqlx::{PgPool, Row};
use std::error::Error;
use std::fmt::{Display, Formatter};
use time::Duration;

const UPSERT_API_CONTRACT_VERSION_SQL: &str = r#"
    INSERT INTO api_contract_versions (
        contract_key,
        contract_version,
        lifecycle_status,
        release_at_utc,
        deprecation_notice_at_utc,
        backward_compatible_until_utc,
        sunset_at_utc,
        replacement_contract_version,
        schema_artifact_path,
        changelog_artifact_path,
        schema_checksum_sha256,
        changelog_checksum_sha256,
        record_checksum_sha256,
        is_active,
        updated_at_utc
    ) VALUES (
        $1,
        $2,
        $3,
        $4::timestamptz,
        $5::timestamptz,
        $6::timestamptz,
        $7::timestamptz,
        $8,
        $9,
        $10,
        $11,
        $12,
        $13,
        $14,
        NOW()
    )
    ON CONFLICT (contract_key, contract_version)
    DO UPDATE SET
        lifecycle_status = EXCLUDED.lifecycle_status,
        release_at_utc = EXCLUDED.release_at_utc,
        deprecation_notice_at_utc = EXCLUDED.deprecation_notice_at_utc,
        backward_compatible_until_utc = EXCLUDED.backward_compatible_until_utc,
        sunset_at_utc = EXCLUDED.sunset_at_utc,
        replacement_contract_version = EXCLUDED.replacement_contract_version,
        schema_artifact_path = EXCLUDED.schema_artifact_path,
        changelog_artifact_path = EXCLUDED.changelog_artifact_path,
        schema_checksum_sha256 = EXCLUDED.schema_checksum_sha256,
        changelog_checksum_sha256 = EXCLUDED.changelog_checksum_sha256,
        record_checksum_sha256 = EXCLUDED.record_checksum_sha256,
        is_active = EXCLUDED.is_active,
        updated_at_utc = NOW()
    RETURNING
        contract_key,
        contract_version,
        lifecycle_status,
        to_char(release_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS release_at_utc,
        to_char(deprecation_notice_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS deprecation_notice_at_utc,
        to_char(backward_compatible_until_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS backward_compatible_until_utc,
        to_char(sunset_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS sunset_at_utc,
        replacement_contract_version,
        schema_artifact_path,
        changelog_artifact_path,
        schema_checksum_sha256,
        changelog_checksum_sha256,
        record_checksum_sha256,
        is_active,
        to_char(created_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS created_at_utc,
        to_char(updated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS updated_at_utc
"#;

const LOAD_API_CONTRACT_VERSION_SQL: &str = r#"
    SELECT
        contract_key,
        contract_version,
        lifecycle_status,
        to_char(release_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS release_at_utc,
        to_char(deprecation_notice_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS deprecation_notice_at_utc,
        to_char(backward_compatible_until_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS backward_compatible_until_utc,
        to_char(sunset_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS sunset_at_utc,
        replacement_contract_version,
        schema_artifact_path,
        changelog_artifact_path,
        schema_checksum_sha256,
        changelog_checksum_sha256,
        record_checksum_sha256,
        is_active,
        to_char(created_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS created_at_utc,
        to_char(updated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS updated_at_utc
    FROM api_contract_versions
    WHERE contract_key = $1
      AND contract_version = $2
    LIMIT 1
"#;

const LOAD_ACTIVE_API_CONTRACT_VERSION_SQL: &str = r#"
    SELECT
        contract_key,
        contract_version,
        lifecycle_status,
        to_char(release_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS release_at_utc,
        to_char(deprecation_notice_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS deprecation_notice_at_utc,
        to_char(backward_compatible_until_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS backward_compatible_until_utc,
        to_char(sunset_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS sunset_at_utc,
        replacement_contract_version,
        schema_artifact_path,
        changelog_artifact_path,
        schema_checksum_sha256,
        changelog_checksum_sha256,
        record_checksum_sha256,
        is_active,
        to_char(created_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS created_at_utc,
        to_char(updated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS updated_at_utc
    FROM api_contract_versions
    WHERE contract_key = $1
      AND is_active = TRUE
      AND release_at_utc <= $2::timestamptz
      AND (sunset_at_utc IS NULL OR sunset_at_utc > $2::timestamptz)
    ORDER BY release_at_utc DESC, contract_version ASC
    LIMIT 1
"#;

const LIST_API_CONTRACT_VERSIONS_SQL: &str = r#"
    SELECT
        contract_key,
        contract_version,
        lifecycle_status,
        to_char(release_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS release_at_utc,
        to_char(deprecation_notice_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS deprecation_notice_at_utc,
        to_char(backward_compatible_until_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS backward_compatible_until_utc,
        to_char(sunset_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS sunset_at_utc,
        replacement_contract_version,
        schema_artifact_path,
        changelog_artifact_path,
        schema_checksum_sha256,
        changelog_checksum_sha256,
        record_checksum_sha256,
        is_active,
        to_char(created_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS created_at_utc,
        to_char(updated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS updated_at_utc
    FROM api_contract_versions
    WHERE contract_key = $1
    ORDER BY release_at_utc DESC, contract_version ASC
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiContractVersionUpsert {
    pub contract_key: String,
    pub contract_version: String,
    pub lifecycle_status: String,
    pub release_at_utc: String,
    pub deprecation_notice_at_utc: Option<String>,
    pub backward_compatible_until_utc: Option<String>,
    pub sunset_at_utc: Option<String>,
    pub replacement_contract_version: Option<String>,
    pub schema_artifact_path: String,
    pub changelog_artifact_path: String,
    pub schema_checksum_sha256: String,
    pub changelog_checksum_sha256: String,
    pub record_checksum_sha256: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiContractVersionRecord {
    pub contract_key: String,
    pub contract_version: String,
    pub lifecycle_status: String,
    pub release_at_utc: String,
    pub deprecation_notice_at_utc: Option<String>,
    pub backward_compatible_until_utc: Option<String>,
    pub sunset_at_utc: Option<String>,
    pub replacement_contract_version: Option<String>,
    pub schema_artifact_path: String,
    pub changelog_artifact_path: String,
    pub schema_checksum_sha256: String,
    pub changelog_checksum_sha256: String,
    pub record_checksum_sha256: String,
    pub is_active: bool,
    pub created_at_utc: String,
    pub updated_at_utc: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiContractVersionPersistenceError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<ReportingValidationIssue>,
}

impl ApiContractVersionPersistenceError {
    fn invalid_payload(error: ReportingContractError) -> Self {
        Self {
            code: error.code,
            message: error.message,
            field_errors: error.field_errors,
        }
    }

    fn query_failure(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: ReportingReasonCode::PersistenceUnavailable.code(),
            message: format!("{operation} failed: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn constraint_violation(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: ReportingReasonCode::PersistenceUnavailable.code(),
            message: format!("{operation} rejected by constraint: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn row_decode_failure(column: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: ReportingReasonCode::PersistenceUnavailable.code(),
            message: format!("unable to decode `{column}`: {error}"),
            field_errors: Vec::new(),
        }
    }
}

impl Display for ApiContractVersionPersistenceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for ApiContractVersionPersistenceError {}

pub async fn upsert_api_contract_version(
    pool: &PgPool,
    upsert: &ApiContractVersionUpsert,
) -> Result<ApiContractVersionRecord, ApiContractVersionPersistenceError> {
    let normalized =
        normalize_upsert(upsert).map_err(ApiContractVersionPersistenceError::invalid_payload)?;

    let row = sqlx::query(UPSERT_API_CONTRACT_VERSION_SQL)
        .bind(&normalized.contract_key)
        .bind(&normalized.contract_version)
        .bind(&normalized.lifecycle_status)
        .bind(&normalized.release_at_utc)
        .bind(normalized.deprecation_notice_at_utc.as_deref())
        .bind(normalized.backward_compatible_until_utc.as_deref())
        .bind(normalized.sunset_at_utc.as_deref())
        .bind(normalized.replacement_contract_version.as_deref())
        .bind(&normalized.schema_artifact_path)
        .bind(&normalized.changelog_artifact_path)
        .bind(&normalized.schema_checksum_sha256)
        .bind(&normalized.changelog_checksum_sha256)
        .bind(&normalized.record_checksum_sha256)
        .bind(normalized.is_active)
        .fetch_one(pool)
        .await
        .map_err(|error| classify_query_error("upsert_api_contract_version", error))?;
    decode_api_contract_version_row(row)
}

pub async fn load_api_contract_version(
    pool: &PgPool,
    contract_key: &str,
    contract_version: &str,
) -> Result<Option<ApiContractVersionRecord>, ApiContractVersionPersistenceError> {
    let contract_key = normalize_lookup_identifier("contract_key", contract_key)
        .map_err(ApiContractVersionPersistenceError::invalid_payload)?;
    let contract_version = normalize_version(contract_version)
        .map_err(ApiContractVersionPersistenceError::invalid_payload)?;

    let row = sqlx::query(LOAD_API_CONTRACT_VERSION_SQL)
        .bind(&contract_key)
        .bind(&contract_version)
        .fetch_optional(pool)
        .await
        .map_err(|error| classify_query_error("load_api_contract_version", error))?;
    row.map(decode_api_contract_version_row).transpose()
}

pub async fn load_active_api_contract_version(
    pool: &PgPool,
    contract_key: &str,
    as_of_utc: &str,
) -> Result<Option<ApiContractVersionRecord>, ApiContractVersionPersistenceError> {
    let contract_key = normalize_lookup_identifier("contract_key", contract_key)
        .map_err(ApiContractVersionPersistenceError::invalid_payload)?;
    parse_utc_timestamp("as_of_utc", as_of_utc)
        .map_err(ApiContractVersionPersistenceError::invalid_payload)?;

    let row = sqlx::query(LOAD_ACTIVE_API_CONTRACT_VERSION_SQL)
        .bind(&contract_key)
        .bind(as_of_utc)
        .fetch_optional(pool)
        .await
        .map_err(|error| classify_query_error("load_active_api_contract_version", error))?;
    row.map(decode_api_contract_version_row).transpose()
}

pub async fn list_api_contract_versions(
    pool: &PgPool,
    contract_key: &str,
) -> Result<Vec<ApiContractVersionRecord>, ApiContractVersionPersistenceError> {
    let contract_key = normalize_lookup_identifier("contract_key", contract_key)
        .map_err(ApiContractVersionPersistenceError::invalid_payload)?;
    let rows = sqlx::query(LIST_API_CONTRACT_VERSIONS_SQL)
        .bind(contract_key)
        .fetch_all(pool)
        .await
        .map_err(|error| classify_query_error("list_api_contract_versions", error))?;
    rows.into_iter()
        .map(decode_api_contract_version_row)
        .collect()
}

fn decode_api_contract_version_row(
    row: sqlx::postgres::PgRow,
) -> Result<ApiContractVersionRecord, ApiContractVersionPersistenceError> {
    Ok(ApiContractVersionRecord {
        contract_key: row.try_get("contract_key").map_err(|error| {
            ApiContractVersionPersistenceError::row_decode_failure("contract_key", error)
        })?,
        contract_version: row.try_get("contract_version").map_err(|error| {
            ApiContractVersionPersistenceError::row_decode_failure("contract_version", error)
        })?,
        lifecycle_status: row.try_get("lifecycle_status").map_err(|error| {
            ApiContractVersionPersistenceError::row_decode_failure("lifecycle_status", error)
        })?,
        release_at_utc: row.try_get("release_at_utc").map_err(|error| {
            ApiContractVersionPersistenceError::row_decode_failure("release_at_utc", error)
        })?,
        deprecation_notice_at_utc: row.try_get("deprecation_notice_at_utc").map_err(|error| {
            ApiContractVersionPersistenceError::row_decode_failure(
                "deprecation_notice_at_utc",
                error,
            )
        })?,
        backward_compatible_until_utc: row.try_get("backward_compatible_until_utc").map_err(
            |error| {
                ApiContractVersionPersistenceError::row_decode_failure(
                    "backward_compatible_until_utc",
                    error,
                )
            },
        )?,
        sunset_at_utc: row.try_get("sunset_at_utc").map_err(|error| {
            ApiContractVersionPersistenceError::row_decode_failure("sunset_at_utc", error)
        })?,
        replacement_contract_version: row.try_get("replacement_contract_version").map_err(
            |error| {
                ApiContractVersionPersistenceError::row_decode_failure(
                    "replacement_contract_version",
                    error,
                )
            },
        )?,
        schema_artifact_path: row.try_get("schema_artifact_path").map_err(|error| {
            ApiContractVersionPersistenceError::row_decode_failure("schema_artifact_path", error)
        })?,
        changelog_artifact_path: row.try_get("changelog_artifact_path").map_err(|error| {
            ApiContractVersionPersistenceError::row_decode_failure("changelog_artifact_path", error)
        })?,
        schema_checksum_sha256: row.try_get("schema_checksum_sha256").map_err(|error| {
            ApiContractVersionPersistenceError::row_decode_failure("schema_checksum_sha256", error)
        })?,
        changelog_checksum_sha256: row.try_get("changelog_checksum_sha256").map_err(|error| {
            ApiContractVersionPersistenceError::row_decode_failure(
                "changelog_checksum_sha256",
                error,
            )
        })?,
        record_checksum_sha256: row.try_get("record_checksum_sha256").map_err(|error| {
            ApiContractVersionPersistenceError::row_decode_failure("record_checksum_sha256", error)
        })?,
        is_active: row.try_get("is_active").map_err(|error| {
            ApiContractVersionPersistenceError::row_decode_failure("is_active", error)
        })?,
        created_at_utc: row.try_get("created_at_utc").map_err(|error| {
            ApiContractVersionPersistenceError::row_decode_failure("created_at_utc", error)
        })?,
        updated_at_utc: row.try_get("updated_at_utc").map_err(|error| {
            ApiContractVersionPersistenceError::row_decode_failure("updated_at_utc", error)
        })?,
    })
}

fn normalize_upsert(
    upsert: &ApiContractVersionUpsert,
) -> Result<ApiContractVersionUpsert, ReportingContractError> {
    let contract_key = normalize_lookup_identifier("contract_key", &upsert.contract_key)?;
    let contract_version = normalize_version(&upsert.contract_version)?;
    let lifecycle_status = normalize_lifecycle_status(&upsert.lifecycle_status)?;
    let release_at = parse_utc_timestamp("release_at_utc", &upsert.release_at_utc)?;
    let deprecation_notice_at = parse_optional_timestamp(
        "deprecation_notice_at_utc",
        upsert.deprecation_notice_at_utc.as_deref(),
    )?;
    let backward_compatible_until = parse_optional_timestamp(
        "backward_compatible_until_utc",
        upsert.backward_compatible_until_utc.as_deref(),
    )?;
    let sunset_at = parse_optional_timestamp("sunset_at_utc", upsert.sunset_at_utc.as_deref())?;

    if lifecycle_status != "active" && deprecation_notice_at.is_none() {
        return Err(ReportingContractError::invalid_payload_with_issues(
            "deprecated/replaced/sunset lifecycle requires deprecation_notice_at_utc",
            vec![ReportingValidationIssue {
                field: "deprecation_notice_at_utc",
                code: ReportingReasonCode::InvalidPayload.code(),
                message:
                    "deprecation_notice_at_utc is required when lifecycle status is not active"
                        .to_string(),
            }],
        ));
    }
    if let Some(deprecation_notice_at) = deprecation_notice_at
        && deprecation_notice_at < release_at + Duration::days(90)
    {
        return Err(ReportingContractError::invalid_payload_with_issues(
            "deprecation_notice_at_utc must be at least 90 days after release_at_utc",
            vec![ReportingValidationIssue {
                field: "deprecation_notice_at_utc",
                code: ReportingReasonCode::InvalidPayload.code(),
                message: "deprecation_notice_at_utc must be >= release_at_utc + 90 days"
                    .to_string(),
            }],
        ));
    }
    if let (Some(deprecation_notice_at), Some(backward_compatible_until)) =
        (deprecation_notice_at, backward_compatible_until)
        && backward_compatible_until < deprecation_notice_at + Duration::days(180)
    {
        return Err(ReportingContractError::invalid_payload_with_issues(
            "backward_compatible_until_utc must preserve support for at least 6 months after notice",
            vec![ReportingValidationIssue {
                field: "backward_compatible_until_utc",
                code: ReportingReasonCode::InvalidPayload.code(),
                message:
                    "backward_compatible_until_utc must be >= deprecation_notice_at_utc + 180 days"
                        .to_string(),
            }],
        ));
    }
    if let (Some(backward_compatible_until), Some(sunset_at)) =
        (backward_compatible_until, sunset_at)
        && sunset_at < backward_compatible_until
    {
        return Err(ReportingContractError::invalid_payload_with_issues(
            "sunset_at_utc cannot precede backward_compatible_until_utc",
            vec![ReportingValidationIssue {
                field: "sunset_at_utc",
                code: ReportingReasonCode::InvalidPayload.code(),
                message: "sunset_at_utc must be >= backward_compatible_until_utc".to_string(),
            }],
        ));
    }

    let replacement_contract_version = upsert
        .replacement_contract_version
        .as_deref()
        .map(normalize_version)
        .transpose()?;
    if lifecycle_status == "replaced" && replacement_contract_version.is_none() {
        return Err(ReportingContractError::invalid_payload_with_issues(
            "replaced lifecycle requires replacement_contract_version",
            vec![ReportingValidationIssue {
                field: "replacement_contract_version",
                code: ReportingReasonCode::InvalidPayload.code(),
                message: "replacement_contract_version must be set when lifecycle_status=replaced"
                    .to_string(),
            }],
        ));
    }
    if replacement_contract_version.is_some() && backward_compatible_until.is_none() {
        return Err(ReportingContractError::invalid_payload_with_issues(
            "replacement lifecycle metadata requires backward_compatible_until_utc",
            vec![ReportingValidationIssue {
                field: "backward_compatible_until_utc",
                code: ReportingReasonCode::InvalidPayload.code(),
                message:
                    "backward_compatible_until_utc is required when replacement_contract_version is set"
                        .to_string(),
            }],
        ));
    }

    if upsert.is_active && lifecycle_status != "active" {
        return Err(ReportingContractError::invalid_payload_with_issues(
            "is_active=true requires lifecycle_status=active",
            vec![ReportingValidationIssue {
                field: "is_active",
                code: ReportingReasonCode::InvalidPayload.code(),
                message: "active contract rows must have lifecycle_status=active".to_string(),
            }],
        ));
    }

    let schema_artifact_path =
        normalize_artifact_path("schema_artifact_path", &upsert.schema_artifact_path)?;
    let changelog_artifact_path =
        normalize_artifact_path("changelog_artifact_path", &upsert.changelog_artifact_path)?;
    let schema_checksum_sha256 =
        normalize_sha256("schema_checksum_sha256", &upsert.schema_checksum_sha256)?;
    let changelog_checksum_sha256 = normalize_sha256(
        "changelog_checksum_sha256",
        &upsert.changelog_checksum_sha256,
    )?;
    let record_checksum_sha256 =
        normalize_sha256("record_checksum_sha256", &upsert.record_checksum_sha256)?;

    Ok(ApiContractVersionUpsert {
        contract_key,
        contract_version,
        lifecycle_status: lifecycle_status.to_string(),
        release_at_utc: upsert.release_at_utc.trim().to_string(),
        deprecation_notice_at_utc: upsert
            .deprecation_notice_at_utc
            .as_deref()
            .map(str::trim)
            .map(str::to_string),
        backward_compatible_until_utc: upsert
            .backward_compatible_until_utc
            .as_deref()
            .map(str::trim)
            .map(str::to_string),
        sunset_at_utc: upsert
            .sunset_at_utc
            .as_deref()
            .map(str::trim)
            .map(str::to_string),
        replacement_contract_version,
        schema_artifact_path,
        changelog_artifact_path,
        schema_checksum_sha256,
        changelog_checksum_sha256,
        record_checksum_sha256,
        is_active: upsert.is_active,
    })
}

fn normalize_lookup_identifier(
    field: &'static str,
    value: &str,
) -> Result<String, ReportingContractError> {
    let normalized = normalize_reporting_identifier(value);
    if normalized.len() < 3 || normalized.len() > 160 {
        return Err(ReportingContractError::invalid_payload_with_issues(
            format!("{field} must contain 3-160 canonical characters"),
            vec![ReportingValidationIssue {
                field,
                code: ReportingReasonCode::InvalidPayload.code(),
                message: format!("{field} must contain 3-160 canonical characters"),
            }],
        ));
    }
    if !normalized.chars().all(|character| {
        character.is_ascii_lowercase() || character.is_ascii_digit() || "._:-".contains(character)
    }) {
        return Err(ReportingContractError::invalid_payload_with_issues(
            format!("{field} contains unsupported characters"),
            vec![ReportingValidationIssue {
                field,
                code: ReportingReasonCode::InvalidPayload.code(),
                message: format!("{field} contains unsupported characters"),
            }],
        ));
    }
    Ok(normalized)
}

fn normalize_lifecycle_status(value: &str) -> Result<&'static str, ReportingContractError> {
    let normalized = normalize_reporting_identifier(value);
    match normalized.as_str() {
        "active" => Ok("active"),
        "deprecated" => Ok("deprecated"),
        "replaced" => Ok("replaced"),
        "sunset" => Ok("sunset"),
        _ => Err(ReportingContractError::invalid_payload_with_issues(
            "lifecycle_status must be one of: active, deprecated, replaced, sunset",
            vec![ReportingValidationIssue {
                field: "lifecycle_status",
                code: ReportingReasonCode::InvalidPayload.code(),
                message: "lifecycle_status must be one of: active, deprecated, replaced, sunset"
                    .to_string(),
            }],
        )),
    }
}

fn normalize_version(value: &str) -> Result<String, ReportingContractError> {
    let normalized = normalize_reporting_identifier(value);
    if !is_semverish_version(&normalized) {
        return Err(ReportingContractError::invalid_payload_with_issues(
            "contract_version must use version shape like v1, v1.0, or v1.0.1",
            vec![ReportingValidationIssue {
                field: "contract_version",
                code: ReportingReasonCode::InvalidPayload.code(),
                message: "contract_version must use version shape like v1, v1.0, or v1.0.1"
                    .to_string(),
            }],
        ));
    }
    Ok(normalized)
}

fn is_semverish_version(value: &str) -> bool {
    let Some(stripped) = value.strip_prefix('v') else {
        return false;
    };
    if stripped.is_empty() {
        return false;
    }
    let mut segment_count = 0usize;
    for segment in stripped.split('.') {
        if segment.is_empty() || !segment.chars().all(|character| character.is_ascii_digit()) {
            return false;
        }
        segment_count += 1;
    }
    (1..=3).contains(&segment_count)
}

fn normalize_artifact_path(
    field: &'static str,
    value: &str,
) -> Result<String, ReportingContractError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ReportingContractError::invalid_payload_with_issues(
            format!("{field} cannot be blank"),
            vec![ReportingValidationIssue {
                field,
                code: ReportingReasonCode::InvalidPayload.code(),
                message: format!("{field} cannot be blank"),
            }],
        ));
    }
    Ok(trimmed.to_string())
}

fn normalize_sha256(field: &'static str, value: &str) -> Result<String, ReportingContractError> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.len() != 64
        || !normalized
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(ReportingContractError::invalid_payload_with_issues(
            format!("{field} must be a 64-character SHA-256 hex digest"),
            vec![ReportingValidationIssue {
                field,
                code: ReportingReasonCode::InvalidPayload.code(),
                message: format!("{field} must be a 64-character SHA-256 hex digest"),
            }],
        ));
    }
    Ok(normalized)
}

fn parse_optional_timestamp(
    field: &'static str,
    value: Option<&str>,
) -> Result<Option<time::OffsetDateTime>, ReportingContractError> {
    let Some(raw) = value else {
        return Ok(None);
    };
    Ok(Some(parse_utc_timestamp(field, raw)?))
}

fn classify_query_error(
    operation: &'static str,
    error: sqlx::Error,
) -> ApiContractVersionPersistenceError {
    if is_constraint_error(&error) {
        return ApiContractVersionPersistenceError::constraint_violation(operation, error);
    }
    ApiContractVersionPersistenceError::query_failure(operation, error)
}

fn is_constraint_error(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Database(database_error) => database_error
            .code()
            .map(|code| code.starts_with("23") || code == "55000")
            .unwrap_or(false),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const API_CONTRACT_VERSIONS_MIGRATION_SQL: &str =
        include_str!("../../migrations/20260407033000_api_contract_versions.sql");

    fn sample_upsert() -> ApiContractVersionUpsert {
        ApiContractVersionUpsert {
            contract_key: "reporting.trades".to_string(),
            contract_version: "v1".to_string(),
            lifecycle_status: "active".to_string(),
            release_at_utc: "2026-04-07T03:30:11Z".to_string(),
            deprecation_notice_at_utc: None,
            backward_compatible_until_utc: None,
            sunset_at_utc: None,
            replacement_contract_version: None,
            schema_artifact_path: "contracts/v1/trades.schema.json".to_string(),
            changelog_artifact_path: "contracts/v1/changelog.json".to_string(),
            schema_checksum_sha256:
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            changelog_checksum_sha256:
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            record_checksum_sha256:
                "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_string(),
            is_active: true,
        }
    }

    #[test]
    fn migration_scope_is_limited_to_api_contract_versions() {
        assert!(
            API_CONTRACT_VERSIONS_MIGRATION_SQL
                .contains("CREATE TABLE IF NOT EXISTS api_contract_versions")
        );
        assert!(!API_CONTRACT_VERSIONS_MIGRATION_SQL.contains("report_schedules"));
        assert!(!API_CONTRACT_VERSIONS_MIGRATION_SQL.contains("report_runs"));
        assert!(!API_CONTRACT_VERSIONS_MIGRATION_SQL.contains("export_jobs"));
        assert!(!API_CONTRACT_VERSIONS_MIGRATION_SQL.contains("export_artifacts"));
    }

    #[test]
    fn migration_enforces_lifecycle_windows_and_lookup_indexes() {
        assert!(
            API_CONTRACT_VERSIONS_MIGRATION_SQL
                .contains("api_contract_versions_deprecation_notice_window_check")
        );
        assert!(
            API_CONTRACT_VERSIONS_MIGRATION_SQL
                .contains("api_contract_versions_support_window_check")
        );
        assert!(
            API_CONTRACT_VERSIONS_MIGRATION_SQL
                .contains("api_contract_versions_sunset_window_check")
        );
        assert!(
            API_CONTRACT_VERSIONS_MIGRATION_SQL
                .contains("idx_api_contract_versions_active_contract")
        );
        assert!(
            API_CONTRACT_VERSIONS_MIGRATION_SQL.contains("idx_api_contract_versions_active_lookup")
        );
        assert!(API_CONTRACT_VERSIONS_MIGRATION_SQL.contains("idx_api_contract_versions_lookup"));
    }

    #[test]
    fn upsert_validation_accepts_canonical_active_contract_payload() {
        let normalized = normalize_upsert(&sample_upsert()).expect("canonical payload should pass");
        assert_eq!(normalized.contract_key, "reporting.trades");
        assert_eq!(normalized.contract_version, "v1");
        assert_eq!(normalized.lifecycle_status, "active");
        assert!(normalized.is_active);
    }

    #[test]
    fn upsert_validation_rejects_invalid_lifecycle_and_checksum_shapes() {
        let mut invalid = sample_upsert();
        invalid.lifecycle_status = "rolling".to_string();
        let error = normalize_upsert(&invalid).expect_err("invalid lifecycle status must fail");
        assert_eq!(error.code, ReportingReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "lifecycle_status")
        );

        let mut invalid_checksum = sample_upsert();
        invalid_checksum.schema_checksum_sha256 = "short".to_string();
        let checksum_error =
            normalize_upsert(&invalid_checksum).expect_err("short checksum should fail");
        assert!(
            checksum_error
                .field_errors
                .iter()
                .any(|issue| issue.field == "schema_checksum_sha256")
        );
    }

    #[test]
    fn upsert_validation_enforces_nfr13_notice_and_support_windows() {
        let mut invalid_notice = sample_upsert();
        invalid_notice.lifecycle_status = "deprecated".to_string();
        invalid_notice.deprecation_notice_at_utc = Some("2026-05-01T00:00:00Z".to_string());
        let notice_error = normalize_upsert(&invalid_notice)
            .expect_err("deprecation notice earlier than 90 days should fail");
        assert!(
            notice_error
                .field_errors
                .iter()
                .any(|issue| issue.field == "deprecation_notice_at_utc")
        );

        let mut invalid_support = sample_upsert();
        invalid_support.lifecycle_status = "replaced".to_string();
        invalid_support.deprecation_notice_at_utc = Some("2026-07-10T00:00:00Z".to_string());
        invalid_support.backward_compatible_until_utc = Some("2026-11-01T00:00:00Z".to_string());
        invalid_support.replacement_contract_version = Some("v2".to_string());
        let support_error = normalize_upsert(&invalid_support)
            .expect_err("support window under 6 months should fail");
        assert!(
            support_error
                .field_errors
                .iter()
                .any(|issue| issue.field == "backward_compatible_until_utc")
        );
    }

    #[test]
    fn active_lookup_sql_enforces_active_version_fallback_query_shape() {
        assert!(LOAD_ACTIVE_API_CONTRACT_VERSION_SQL.contains("AND is_active = TRUE"));
        assert!(LOAD_ACTIVE_API_CONTRACT_VERSION_SQL.contains("release_at_utc <= $2::timestamptz"));
        assert!(
            LOAD_ACTIVE_API_CONTRACT_VERSION_SQL
                .contains("sunset_at_utc IS NULL OR sunset_at_utc > $2::timestamptz")
        );
        assert!(
            LOAD_ACTIVE_API_CONTRACT_VERSION_SQL
                .contains("ORDER BY release_at_utc DESC, contract_version ASC")
        );
    }
}
