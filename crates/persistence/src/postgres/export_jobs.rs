use domain::reporting_export::{
    DEFAULT_EXPORT_LIST_LIMIT, ExportArtifactRecord, ExportJobRecord, MAX_EXPORT_LIST_LIMIT,
    ReportingExportArtifactType, ReportingExportContractError, ReportingExportJobState,
    ReportingExportReasonCode, ReportingExportTriggerSource, ReportingExportValidationIssue,
    normalize_reporting_export_identifier, validate_export_artifact, validate_export_job,
    validate_export_job_transition,
};
use sqlx::{PgPool, Row};
use std::error::Error;
use std::fmt::{Display, Formatter};

const UPSERT_EXPORT_JOB_SQL: &str = r#"
    INSERT INTO export_jobs (
        job_id,
        trigger_source,
        status,
        reason_code,
        actor_id,
        actor_role,
        correlation_id,
        schedule_id,
        schedule_window_key,
        report_run_id,
        incident_id,
        incident_severity,
        requested_at_utc,
        started_at_utc,
        finished_at_utc,
        package_reference,
        package_checksum,
        failure_metadata,
        created_at_utc,
        updated_at_utc
    ) VALUES (
        $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
        $13::timestamptz, $14::timestamptz, $15::timestamptz, $16, $17, $18, $19::timestamptz, $20::timestamptz
    )
    ON CONFLICT (job_id)
    DO UPDATE SET
        status = EXCLUDED.status,
        reason_code = EXCLUDED.reason_code,
        started_at_utc = EXCLUDED.started_at_utc,
        finished_at_utc = EXCLUDED.finished_at_utc,
        package_reference = EXCLUDED.package_reference,
        package_checksum = EXCLUDED.package_checksum,
        failure_metadata = EXCLUDED.failure_metadata,
        updated_at_utc = EXCLUDED.updated_at_utc
    RETURNING
        job_id,
        trigger_source,
        status,
        reason_code,
        actor_id,
        actor_role,
        correlation_id,
        schedule_id,
        schedule_window_key,
        report_run_id,
        incident_id,
        incident_severity,
        to_char(requested_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS requested_at_utc,
        CASE
            WHEN started_at_utc IS NULL THEN NULL
            ELSE to_char(started_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"')
        END AS started_at_utc,
        CASE
            WHEN finished_at_utc IS NULL THEN NULL
            ELSE to_char(finished_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"')
        END AS finished_at_utc,
        package_reference,
        package_checksum,
        CASE
            WHEN failure_metadata IS NULL THEN NULL
            ELSE failure_metadata::text
        END AS failure_metadata,
        to_char(created_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS created_at_utc,
        to_char(updated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS updated_at_utc
"#;

const LOAD_EXPORT_JOB_BY_ID_SQL: &str = r#"
    SELECT
        job_id,
        trigger_source,
        status,
        reason_code,
        actor_id,
        actor_role,
        correlation_id,
        schedule_id,
        schedule_window_key,
        report_run_id,
        incident_id,
        incident_severity,
        to_char(requested_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS requested_at_utc,
        CASE
            WHEN started_at_utc IS NULL THEN NULL
            ELSE to_char(started_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"')
        END AS started_at_utc,
        CASE
            WHEN finished_at_utc IS NULL THEN NULL
            ELSE to_char(finished_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"')
        END AS finished_at_utc,
        package_reference,
        package_checksum,
        CASE
            WHEN failure_metadata IS NULL THEN NULL
            ELSE failure_metadata::text
        END AS failure_metadata,
        to_char(created_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS created_at_utc,
        to_char(updated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS updated_at_utc
    FROM export_jobs
    WHERE job_id = $1
    LIMIT 1
"#;

const LOAD_EXPORT_JOB_STATUS_SQL: &str = r#"
    SELECT status
    FROM export_jobs
    WHERE job_id = $1
    LIMIT 1
"#;

const LOAD_EXPORT_JOB_BY_WEEKLY_BINDING_SQL: &str = r#"
    SELECT
        job_id,
        trigger_source,
        status,
        reason_code,
        actor_id,
        actor_role,
        correlation_id,
        schedule_id,
        schedule_window_key,
        report_run_id,
        incident_id,
        incident_severity,
        to_char(requested_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS requested_at_utc,
        CASE
            WHEN started_at_utc IS NULL THEN NULL
            ELSE to_char(started_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"')
        END AS started_at_utc,
        CASE
            WHEN finished_at_utc IS NULL THEN NULL
            ELSE to_char(finished_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"')
        END AS finished_at_utc,
        package_reference,
        package_checksum,
        CASE
            WHEN failure_metadata IS NULL THEN NULL
            ELSE failure_metadata::text
        END AS failure_metadata,
        to_char(created_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS created_at_utc,
        to_char(updated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS updated_at_utc
    FROM export_jobs
    WHERE trigger_source = 'scheduled_weekly'
      AND schedule_id = $1
      AND schedule_window_key = $2
      AND report_run_id = $3
    ORDER BY requested_at_utc DESC, job_id ASC
    LIMIT 1
"#;

const LIST_EXPORT_JOBS_SQL: &str = r#"
    SELECT
        job_id,
        trigger_source,
        status,
        reason_code,
        actor_id,
        actor_role,
        correlation_id,
        schedule_id,
        schedule_window_key,
        report_run_id,
        incident_id,
        incident_severity,
        to_char(requested_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS requested_at_utc,
        CASE
            WHEN started_at_utc IS NULL THEN NULL
            ELSE to_char(started_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"')
        END AS started_at_utc,
        CASE
            WHEN finished_at_utc IS NULL THEN NULL
            ELSE to_char(finished_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"')
        END AS finished_at_utc,
        package_reference,
        package_checksum,
        CASE
            WHEN failure_metadata IS NULL THEN NULL
            ELSE failure_metadata::text
        END AS failure_metadata,
        to_char(created_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS created_at_utc,
        to_char(updated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS updated_at_utc
    FROM export_jobs
    ORDER BY requested_at_utc DESC, job_id ASC
    LIMIT $1
"#;

const UPSERT_EXPORT_ARTIFACT_SQL: &str = r#"
    INSERT INTO export_artifacts (
        artifact_id,
        job_id,
        artifact_type,
        source,
        as_of_utc,
        reason_code,
        correlation_id,
        checksum,
        retrieval_reference,
        is_available,
        created_at_utc,
        updated_at_utc
    ) VALUES (
        $1, $2, $3, $4, $5::timestamptz, $6, $7, $8, $9, $10, $11::timestamptz, $12::timestamptz
    )
    ON CONFLICT (artifact_id)
    DO UPDATE SET
        reason_code = EXCLUDED.reason_code,
        correlation_id = EXCLUDED.correlation_id,
        checksum = EXCLUDED.checksum,
        retrieval_reference = EXCLUDED.retrieval_reference,
        is_available = EXCLUDED.is_available,
        updated_at_utc = EXCLUDED.updated_at_utc
    RETURNING
        artifact_id,
        job_id,
        artifact_type,
        source,
        to_char(as_of_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS as_of_utc,
        reason_code,
        correlation_id,
        checksum,
        retrieval_reference,
        is_available,
        to_char(created_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS created_at_utc,
        to_char(updated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS updated_at_utc
"#;

const LOAD_EXPORT_ARTIFACT_BY_ID_SQL: &str = r#"
    SELECT
        artifact_id,
        job_id,
        artifact_type,
        source,
        to_char(as_of_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS as_of_utc,
        reason_code,
        correlation_id,
        checksum,
        retrieval_reference,
        is_available,
        to_char(created_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS created_at_utc,
        to_char(updated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS updated_at_utc
    FROM export_artifacts
    WHERE job_id = $1
      AND artifact_id = $2
    LIMIT 1
"#;

const LOAD_EXPORT_ARTIFACTS_FOR_JOB_SQL: &str = r#"
    SELECT
        artifact_id,
        job_id,
        artifact_type,
        source,
        to_char(as_of_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS as_of_utc,
        reason_code,
        correlation_id,
        checksum,
        retrieval_reference,
        is_available,
        to_char(created_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS created_at_utc,
        to_char(updated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS updated_at_utc
    FROM export_artifacts
    WHERE job_id = $1
    ORDER BY artifact_type ASC, artifact_id ASC
    LIMIT $2
"#;

const REPORT_EXPORT_QUERY_FAILED: &str = "report_export_query_failed";
const REPORT_EXPORT_CONSTRAINT_VIOLATION: &str = "report_export_constraint_violation";
const REPORT_EXPORT_ROW_DECODE_FAILED: &str = "report_export_row_decode_failed";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportExportPersistenceError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<ReportingExportValidationIssue>,
}

impl ReportExportPersistenceError {
    fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<ReportingExportValidationIssue>,
    ) -> Self {
        Self {
            code: ReportingExportReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn query_failure(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: REPORT_EXPORT_QUERY_FAILED,
            message: format!("{operation} failed: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn constraint_violation(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: REPORT_EXPORT_CONSTRAINT_VIOLATION,
            message: format!("{operation} rejected by constraint: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn row_decode_failure(column: &'static str, error: impl Display) -> Self {
        Self {
            code: REPORT_EXPORT_ROW_DECODE_FAILED,
            message: format!("unable to decode `{column}`: {error}"),
            field_errors: Vec::new(),
        }
    }
}

impl Display for ReportExportPersistenceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for ReportExportPersistenceError {}

pub async fn upsert_export_job(
    pool: &PgPool,
    job: &ExportJobRecord,
) -> Result<ExportJobRecord, ReportExportPersistenceError> {
    let canonical = canonicalize_job(job)?;
    let previous_state = load_export_job_status(pool, &canonical.job_id).await?;
    validate_export_job_transition(previous_state, canonical.status).map_err(map_contract_error)?;
    let failure_metadata = parse_optional_json(&canonical.failure_metadata)?;

    let row = sqlx::query(UPSERT_EXPORT_JOB_SQL)
        .bind(&canonical.job_id)
        .bind(canonical.trigger_source.as_str())
        .bind(canonical.status.as_str())
        .bind(&canonical.reason_code)
        .bind(&canonical.actor_id)
        .bind(&canonical.actor_role)
        .bind(&canonical.correlation_id)
        .bind(canonical.schedule_id.as_deref())
        .bind(canonical.schedule_window_key.as_deref())
        .bind(canonical.report_run_id.as_deref())
        .bind(canonical.incident_id.as_deref())
        .bind(canonical.incident_severity.as_deref())
        .bind(&canonical.requested_at_utc)
        .bind(canonical.started_at_utc.as_deref())
        .bind(canonical.finished_at_utc.as_deref())
        .bind(canonical.package_reference.as_deref())
        .bind(canonical.package_checksum.as_deref())
        .bind(failure_metadata)
        .bind(&canonical.created_at_utc)
        .bind(&canonical.updated_at_utc)
        .fetch_one(pool)
        .await
        .map_err(|error| classify_query_error("upsert_export_job", error))?;
    decode_job_row(row)
}

pub async fn load_export_job_by_id(
    pool: &PgPool,
    job_id: &str,
) -> Result<Option<ExportJobRecord>, ReportExportPersistenceError> {
    let normalized_job_id = normalize_lookup_identifier("job_id", job_id, 200)?;
    let row = sqlx::query(LOAD_EXPORT_JOB_BY_ID_SQL)
        .bind(&normalized_job_id)
        .fetch_optional(pool)
        .await
        .map_err(|error| classify_query_error("load_export_job_by_id", error))?;
    row.map(decode_job_row).transpose()
}

pub async fn load_export_job_by_weekly_binding(
    pool: &PgPool,
    schedule_id: &str,
    schedule_window_key: &str,
    report_run_id: &str,
) -> Result<Option<ExportJobRecord>, ReportExportPersistenceError> {
    let normalized_schedule_id = normalize_lookup_identifier("schedule_id", schedule_id, 200)?;
    let normalized_window_key =
        normalize_lookup_identifier("schedule_window_key", schedule_window_key, 240)?;
    let normalized_report_run_id =
        normalize_lookup_identifier("report_run_id", report_run_id, 200)?;
    let row = sqlx::query(LOAD_EXPORT_JOB_BY_WEEKLY_BINDING_SQL)
        .bind(&normalized_schedule_id)
        .bind(&normalized_window_key)
        .bind(&normalized_report_run_id)
        .fetch_optional(pool)
        .await
        .map_err(|error| classify_query_error("load_export_job_by_weekly_binding", error))?;
    row.map(decode_job_row).transpose()
}

pub async fn list_export_jobs(
    pool: &PgPool,
    limit: Option<i64>,
) -> Result<Vec<ExportJobRecord>, ReportExportPersistenceError> {
    let normalized_limit = normalize_limit(limit)?;
    let rows = sqlx::query(LIST_EXPORT_JOBS_SQL)
        .bind(normalized_limit)
        .fetch_all(pool)
        .await
        .map_err(|error| classify_query_error("list_export_jobs", error))?;
    rows.into_iter().map(decode_job_row).collect()
}

pub async fn upsert_export_artifact(
    pool: &PgPool,
    artifact: &ExportArtifactRecord,
) -> Result<ExportArtifactRecord, ReportExportPersistenceError> {
    let canonical = canonicalize_artifact(artifact)?;
    let row = sqlx::query(UPSERT_EXPORT_ARTIFACT_SQL)
        .bind(&canonical.artifact_id)
        .bind(&canonical.job_id)
        .bind(canonical.artifact_type.as_str())
        .bind(&canonical.source)
        .bind(&canonical.as_of_utc)
        .bind(&canonical.reason_code)
        .bind(&canonical.correlation_id)
        .bind(&canonical.checksum)
        .bind(&canonical.retrieval_reference)
        .bind(canonical.is_available)
        .bind(&canonical.created_at_utc)
        .bind(&canonical.updated_at_utc)
        .fetch_one(pool)
        .await
        .map_err(|error| classify_query_error("upsert_export_artifact", error))?;
    decode_artifact_row(row)
}

pub async fn load_export_artifact_by_id(
    pool: &PgPool,
    job_id: &str,
    artifact_id: &str,
) -> Result<Option<ExportArtifactRecord>, ReportExportPersistenceError> {
    let normalized_job_id = normalize_lookup_identifier("job_id", job_id, 200)?;
    let normalized_artifact_id = normalize_lookup_identifier("artifact_id", artifact_id, 200)?;
    let row = sqlx::query(LOAD_EXPORT_ARTIFACT_BY_ID_SQL)
        .bind(&normalized_job_id)
        .bind(&normalized_artifact_id)
        .fetch_optional(pool)
        .await
        .map_err(|error| classify_query_error("load_export_artifact_by_id", error))?;
    row.map(decode_artifact_row).transpose()
}

pub async fn load_export_artifacts_for_job(
    pool: &PgPool,
    job_id: &str,
    limit: Option<i64>,
) -> Result<Vec<ExportArtifactRecord>, ReportExportPersistenceError> {
    let normalized_job_id = normalize_lookup_identifier("job_id", job_id, 200)?;
    let normalized_limit = normalize_limit(limit)?;
    let rows = sqlx::query(LOAD_EXPORT_ARTIFACTS_FOR_JOB_SQL)
        .bind(&normalized_job_id)
        .bind(normalized_limit)
        .fetch_all(pool)
        .await
        .map_err(|error| classify_query_error("load_export_artifacts_for_job", error))?;
    rows.into_iter().map(decode_artifact_row).collect()
}

async fn load_export_job_status(
    pool: &PgPool,
    job_id: &str,
) -> Result<Option<ReportingExportJobState>, ReportExportPersistenceError> {
    let row = sqlx::query(LOAD_EXPORT_JOB_STATUS_SQL)
        .bind(job_id)
        .fetch_optional(pool)
        .await
        .map_err(|error| classify_query_error("load_export_job_status", error))?;
    let Some(row) = row else {
        return Ok(None);
    };
    let status: String = row
        .try_get("status")
        .map_err(|error| ReportExportPersistenceError::row_decode_failure("status", error))?;
    let state = ReportingExportJobState::parse(&status).map_err(map_contract_error)?;
    Ok(Some(state))
}

fn decode_job_row(
    row: sqlx::postgres::PgRow,
) -> Result<ExportJobRecord, ReportExportPersistenceError> {
    let trigger_source: String = row.try_get("trigger_source").map_err(|error| {
        ReportExportPersistenceError::row_decode_failure("trigger_source", error)
    })?;
    let status: String = row
        .try_get("status")
        .map_err(|error| ReportExportPersistenceError::row_decode_failure("status", error))?;
    let job = ExportJobRecord {
        job_id: row
            .try_get("job_id")
            .map_err(|error| ReportExportPersistenceError::row_decode_failure("job_id", error))?,
        trigger_source: ReportingExportTriggerSource::parse(&trigger_source)
            .map_err(map_contract_error)?,
        status: ReportingExportJobState::parse(&status).map_err(map_contract_error)?,
        reason_code: row.try_get("reason_code").map_err(|error| {
            ReportExportPersistenceError::row_decode_failure("reason_code", error)
        })?,
        actor_id: row
            .try_get("actor_id")
            .map_err(|error| ReportExportPersistenceError::row_decode_failure("actor_id", error))?,
        actor_role: row.try_get("actor_role").map_err(|error| {
            ReportExportPersistenceError::row_decode_failure("actor_role", error)
        })?,
        correlation_id: row.try_get("correlation_id").map_err(|error| {
            ReportExportPersistenceError::row_decode_failure("correlation_id", error)
        })?,
        schedule_id: row.try_get("schedule_id").map_err(|error| {
            ReportExportPersistenceError::row_decode_failure("schedule_id", error)
        })?,
        schedule_window_key: row.try_get("schedule_window_key").map_err(|error| {
            ReportExportPersistenceError::row_decode_failure("schedule_window_key", error)
        })?,
        report_run_id: row.try_get("report_run_id").map_err(|error| {
            ReportExportPersistenceError::row_decode_failure("report_run_id", error)
        })?,
        incident_id: row.try_get("incident_id").map_err(|error| {
            ReportExportPersistenceError::row_decode_failure("incident_id", error)
        })?,
        incident_severity: row.try_get("incident_severity").map_err(|error| {
            ReportExportPersistenceError::row_decode_failure("incident_severity", error)
        })?,
        requested_at_utc: row.try_get("requested_at_utc").map_err(|error| {
            ReportExportPersistenceError::row_decode_failure("requested_at_utc", error)
        })?,
        started_at_utc: row.try_get("started_at_utc").map_err(|error| {
            ReportExportPersistenceError::row_decode_failure("started_at_utc", error)
        })?,
        finished_at_utc: row.try_get("finished_at_utc").map_err(|error| {
            ReportExportPersistenceError::row_decode_failure("finished_at_utc", error)
        })?,
        package_reference: row.try_get("package_reference").map_err(|error| {
            ReportExportPersistenceError::row_decode_failure("package_reference", error)
        })?,
        package_checksum: row.try_get("package_checksum").map_err(|error| {
            ReportExportPersistenceError::row_decode_failure("package_checksum", error)
        })?,
        failure_metadata: row.try_get("failure_metadata").map_err(|error| {
            ReportExportPersistenceError::row_decode_failure("failure_metadata", error)
        })?,
        created_at_utc: row.try_get("created_at_utc").map_err(|error| {
            ReportExportPersistenceError::row_decode_failure("created_at_utc", error)
        })?,
        updated_at_utc: row.try_get("updated_at_utc").map_err(|error| {
            ReportExportPersistenceError::row_decode_failure("updated_at_utc", error)
        })?,
    };
    validate_export_job(&job).map_err(map_contract_error)?;
    Ok(job)
}

fn decode_artifact_row(
    row: sqlx::postgres::PgRow,
) -> Result<ExportArtifactRecord, ReportExportPersistenceError> {
    let artifact_type: String = row.try_get("artifact_type").map_err(|error| {
        ReportExportPersistenceError::row_decode_failure("artifact_type", error)
    })?;
    let artifact = ExportArtifactRecord {
        artifact_id: row.try_get("artifact_id").map_err(|error| {
            ReportExportPersistenceError::row_decode_failure("artifact_id", error)
        })?,
        job_id: row
            .try_get("job_id")
            .map_err(|error| ReportExportPersistenceError::row_decode_failure("job_id", error))?,
        artifact_type: ReportingExportArtifactType::parse(&artifact_type)
            .map_err(map_contract_error)?,
        source: row
            .try_get("source")
            .map_err(|error| ReportExportPersistenceError::row_decode_failure("source", error))?,
        as_of_utc: row.try_get("as_of_utc").map_err(|error| {
            ReportExportPersistenceError::row_decode_failure("as_of_utc", error)
        })?,
        reason_code: row.try_get("reason_code").map_err(|error| {
            ReportExportPersistenceError::row_decode_failure("reason_code", error)
        })?,
        correlation_id: row.try_get("correlation_id").map_err(|error| {
            ReportExportPersistenceError::row_decode_failure("correlation_id", error)
        })?,
        checksum: row
            .try_get("checksum")
            .map_err(|error| ReportExportPersistenceError::row_decode_failure("checksum", error))?,
        retrieval_reference: row.try_get("retrieval_reference").map_err(|error| {
            ReportExportPersistenceError::row_decode_failure("retrieval_reference", error)
        })?,
        is_available: row.try_get("is_available").map_err(|error| {
            ReportExportPersistenceError::row_decode_failure("is_available", error)
        })?,
        created_at_utc: row.try_get("created_at_utc").map_err(|error| {
            ReportExportPersistenceError::row_decode_failure("created_at_utc", error)
        })?,
        updated_at_utc: row.try_get("updated_at_utc").map_err(|error| {
            ReportExportPersistenceError::row_decode_failure("updated_at_utc", error)
        })?,
    };
    validate_export_artifact(&artifact).map_err(map_contract_error)?;
    Ok(artifact)
}

fn canonicalize_job(
    job: &ExportJobRecord,
) -> Result<ExportJobRecord, ReportExportPersistenceError> {
    let mut canonical = job.clone();
    canonical.job_id = normalize_lookup_identifier("job_id", &canonical.job_id, 200)?;
    canonical.reason_code = ReportingExportReasonCode::parse(&canonical.reason_code)
        .map_err(map_contract_error)?
        .code()
        .to_string();
    canonical.actor_id = normalize_lookup_identifier("actor_id", &canonical.actor_id, 200)?;
    canonical.actor_role = normalize_lookup_identifier("actor_role", &canonical.actor_role, 200)?;
    canonical.correlation_id =
        normalize_lookup_identifier("correlation_id", &canonical.correlation_id, 200)?;
    canonical.schedule_id =
        normalize_optional_identifier("schedule_id", canonical.schedule_id.as_deref(), 200)?;
    canonical.schedule_window_key = normalize_optional_identifier(
        "schedule_window_key",
        canonical.schedule_window_key.as_deref(),
        240,
    )?;
    canonical.report_run_id =
        normalize_optional_identifier("report_run_id", canonical.report_run_id.as_deref(), 200)?;
    canonical.incident_id =
        normalize_optional_identifier("incident_id", canonical.incident_id.as_deref(), 200)?;
    canonical.incident_severity = normalize_optional_identifier(
        "incident_severity",
        canonical.incident_severity.as_deref(),
        200,
    )?;
    canonical.package_reference = normalize_optional_string(canonical.package_reference.as_deref());
    canonical.package_checksum = canonical
        .package_checksum
        .as_deref()
        .map(str::trim)
        .filter(|candidate| !candidate.is_empty())
        .map(|value| value.to_ascii_lowercase());
    canonical.failure_metadata = normalize_optional_string(canonical.failure_metadata.as_deref());
    let _ = parse_optional_json(&canonical.failure_metadata)?;
    validate_export_job(&canonical).map_err(map_contract_error)?;
    Ok(canonical)
}

fn canonicalize_artifact(
    artifact: &ExportArtifactRecord,
) -> Result<ExportArtifactRecord, ReportExportPersistenceError> {
    let mut canonical = artifact.clone();
    canonical.artifact_id =
        normalize_lookup_identifier("artifact_id", &canonical.artifact_id, 200)?;
    canonical.job_id = normalize_lookup_identifier("job_id", &canonical.job_id, 200)?;
    canonical.source = normalize_lookup_identifier("source", &canonical.source, 200)?;
    canonical.reason_code = ReportingExportReasonCode::parse(&canonical.reason_code)
        .map_err(map_contract_error)?
        .code()
        .to_string();
    canonical.correlation_id =
        normalize_lookup_identifier("correlation_id", &canonical.correlation_id, 200)?;
    canonical.checksum = canonical.checksum.trim().to_ascii_lowercase();
    canonical.retrieval_reference = canonical.retrieval_reference.trim().to_string();
    validate_export_artifact(&canonical).map_err(map_contract_error)?;
    Ok(canonical)
}

fn parse_optional_json(
    value: &Option<String>,
) -> Result<Option<serde_json::Value>, ReportExportPersistenceError> {
    value
        .as_deref()
        .map(|candidate| {
            serde_json::from_str::<serde_json::Value>(candidate).map_err(|error| {
                ReportExportPersistenceError::invalid_payload(
                    format!("failure_metadata must be valid JSON: {error}"),
                    vec![ReportingExportValidationIssue {
                        field: "failure_metadata",
                        code: ReportingExportReasonCode::InvalidPayload.code(),
                        message: "failure_metadata must be valid JSON".to_string(),
                    }],
                )
            })
        })
        .transpose()
}

fn normalize_optional_identifier(
    field: &'static str,
    value: Option<&str>,
    max_len: usize,
) -> Result<Option<String>, ReportExportPersistenceError> {
    value
        .map(|candidate| normalize_lookup_identifier(field, candidate, max_len))
        .transpose()
}

fn normalize_optional_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|candidate| !candidate.is_empty())
        .map(str::to_string)
}

fn normalize_lookup_identifier(
    field: &'static str,
    value: &str,
    max_len: usize,
) -> Result<String, ReportExportPersistenceError> {
    let normalized = normalize_reporting_export_identifier(value);
    if normalized.len() < 3 || normalized.len() > max_len {
        return Err(ReportExportPersistenceError::invalid_payload(
            format!("{field} must contain 3-{max_len} canonical characters"),
            vec![ReportingExportValidationIssue {
                field,
                code: ReportingExportReasonCode::InvalidPayload.code(),
                message: format!("{field} must contain 3-{max_len} canonical characters"),
            }],
        ));
    }
    if !normalized.chars().all(|character| {
        character.is_ascii_lowercase() || character.is_ascii_digit() || "._:-".contains(character)
    }) {
        return Err(ReportExportPersistenceError::invalid_payload(
            format!("{field} contains unsupported characters"),
            vec![ReportingExportValidationIssue {
                field,
                code: ReportingExportReasonCode::InvalidPayload.code(),
                message: format!("{field} contains unsupported characters"),
            }],
        ));
    }
    Ok(normalized)
}

fn normalize_limit(limit: Option<i64>) -> Result<i64, ReportExportPersistenceError> {
    let value = limit.unwrap_or(DEFAULT_EXPORT_LIST_LIMIT);
    if !(1..=MAX_EXPORT_LIST_LIMIT).contains(&value) {
        return Err(ReportExportPersistenceError::invalid_payload(
            format!("limit must be between 1 and {MAX_EXPORT_LIST_LIMIT}"),
            vec![ReportingExportValidationIssue {
                field: "limit",
                code: ReportingExportReasonCode::InvalidPayload.code(),
                message: format!("limit must be between 1 and {MAX_EXPORT_LIST_LIMIT}"),
            }],
        ));
    }
    Ok(value)
}

fn map_contract_error(error: ReportingExportContractError) -> ReportExportPersistenceError {
    ReportExportPersistenceError {
        code: error.code,
        message: error.message,
        field_errors: error.field_errors,
    }
}

fn classify_query_error(
    operation: &'static str,
    error: sqlx::Error,
) -> ReportExportPersistenceError {
    if is_constraint_error(&error) {
        return ReportExportPersistenceError::constraint_violation(operation, error);
    }
    ReportExportPersistenceError::query_failure(operation, error)
}

fn is_constraint_error(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Database(database_error) => database_error
            .code()
            .map(|code| is_constraint_sqlstate(code.as_ref()))
            .unwrap_or(false),
        _ => false,
    }
}

fn is_constraint_sqlstate(sqlstate: &str) -> bool {
    sqlstate.starts_with("23") || sqlstate == "55000"
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXPORT_MIGRATION_SQL: &str =
        include_str!("../../migrations/20260407062000_export_jobs_export_artifacts.sql");

    fn sample_job(state: ReportingExportJobState) -> ExportJobRecord {
        ExportJobRecord {
            job_id: "export-job-001".to_string(),
            trigger_source: ReportingExportTriggerSource::OnDemand,
            status: state,
            reason_code: ReportingExportReasonCode::Ready.code().to_string(),
            actor_id: "ops-1".to_string(),
            actor_role: "operational_control".to_string(),
            correlation_id: "corr-export-001".to_string(),
            schedule_id: None,
            schedule_window_key: None,
            report_run_id: None,
            incident_id: None,
            incident_severity: None,
            requested_at_utc: "2026-04-07T00:00:00Z".to_string(),
            started_at_utc: if state != ReportingExportJobState::Queued {
                Some("2026-04-07T00:00:01Z".to_string())
            } else {
                None
            },
            finished_at_utc: if matches!(
                state,
                ReportingExportJobState::Succeeded | ReportingExportJobState::Failed
            ) {
                Some("2026-04-07T00:00:02Z".to_string())
            } else {
                None
            },
            package_reference: Some(
                "s3://reporting-exports/export-job-001/package.json".to_string(),
            ),
            package_checksum: Some("a".repeat(64)),
            failure_metadata: None,
            created_at_utc: "2026-04-07T00:00:00Z".to_string(),
            updated_at_utc: "2026-04-07T00:00:02Z".to_string(),
        }
    }

    fn sample_artifact() -> ExportArtifactRecord {
        ExportArtifactRecord {
            artifact_id: "export-artifact-001".to_string(),
            job_id: "export-job-001".to_string(),
            artifact_type: ReportingExportArtifactType::PromotionDecisions,
            source: "reporting_read_models".to_string(),
            as_of_utc: "2026-04-07T00:00:00Z".to_string(),
            reason_code: ReportingExportReasonCode::Ready.code().to_string(),
            correlation_id: "corr-export-001".to_string(),
            checksum: "b".repeat(64),
            retrieval_reference: "s3://reporting-exports/export-job-001/promotion.json".to_string(),
            is_available: true,
            created_at_utc: "2026-04-07T00:00:00Z".to_string(),
            updated_at_utc: "2026-04-07T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn migration_scope_is_limited_to_export_jobs_and_artifacts() {
        assert!(EXPORT_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS export_jobs"));
        assert!(EXPORT_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS export_artifacts"));
        assert!(!EXPORT_MIGRATION_SQL.contains("report_schedules"));
        assert!(!EXPORT_MIGRATION_SQL.contains("report_runs"));
    }

    #[test]
    fn migration_enforces_required_checks_and_indexes() {
        assert!(EXPORT_MIGRATION_SQL.contains("export_jobs_trigger_source_check"));
        assert!(EXPORT_MIGRATION_SQL.contains("export_jobs_status_check"));
        assert!(EXPORT_MIGRATION_SQL.contains("uq_export_jobs_weekly_idempotency"));
        assert!(EXPORT_MIGRATION_SQL.contains("export_artifacts_artifact_type_check"));
        assert!(EXPORT_MIGRATION_SQL.contains("export_artifacts_unique_job_category"));
        assert!(EXPORT_MIGRATION_SQL.contains("idx_export_artifacts_as_of_lookup"));
    }

    #[test]
    fn adapter_queries_remain_deterministic_for_job_and_artifact_lookup() {
        assert!(LIST_EXPORT_JOBS_SQL.contains("ORDER BY requested_at_utc DESC, job_id ASC"));
        assert!(
            LOAD_EXPORT_JOB_BY_WEEKLY_BINDING_SQL
                .contains("ORDER BY requested_at_utc DESC, job_id ASC")
        );
        assert!(
            LOAD_EXPORT_ARTIFACTS_FOR_JOB_SQL
                .contains("ORDER BY artifact_type ASC, artifact_id ASC")
        );
    }

    #[test]
    fn canonicalization_normalizes_identifiers_reason_codes_and_checksums() {
        let mut job = sample_job(ReportingExportJobState::Queued);
        job.job_id = " Export-Job-001 ".to_string();
        job.reason_code = "REPORTING_EXPORT_READY".to_string();
        job.actor_role = "Operational_Control".to_string();
        let canonical_job = canonicalize_job(&job).expect("canonicalization should succeed");
        assert_eq!(canonical_job.job_id, "export-job-001");
        assert_eq!(
            canonical_job.reason_code,
            ReportingExportReasonCode::Ready.code()
        );
        assert_eq!(canonical_job.actor_role, "operational_control");

        let mut artifact = sample_artifact();
        artifact.artifact_id = " Export-Artifact-001 ".to_string();
        artifact.reason_code = "REPORTING_EXPORT_READY".to_string();
        let canonical_artifact =
            canonicalize_artifact(&artifact).expect("artifact canonicalization should succeed");
        assert_eq!(canonical_artifact.artifact_id, "export-artifact-001");
        assert_eq!(
            canonical_artifact.reason_code,
            ReportingExportReasonCode::Ready.code()
        );
    }

    #[test]
    fn canonicalization_accepts_readiness_artifact_types() {
        let mut artifact = sample_artifact();
        artifact.artifact_type = ReportingExportArtifactType::ReadinessReportMarkdown;
        artifact.retrieval_reference =
            "s3://reporting-exports/export-job-001/readiness-report.md".to_string();
        let canonical_artifact =
            canonicalize_artifact(&artifact).expect("readiness artifact should validate");
        assert_eq!(
            canonical_artifact.artifact_type,
            ReportingExportArtifactType::ReadinessReportMarkdown
        );
    }

    #[test]
    fn transition_validation_is_fail_closed_for_invalid_jump() {
        let error = validate_export_job_transition(
            Some(ReportingExportJobState::Queued),
            ReportingExportJobState::Succeeded,
        )
        .expect_err("queued -> succeeded should fail");
        assert_eq!(error.code, ReportingExportReasonCode::InvalidPayload.code());
    }

    #[test]
    fn limit_validation_enforces_defaults_and_bounds() {
        assert_eq!(
            normalize_limit(None).expect("default limit should be used"),
            DEFAULT_EXPORT_LIST_LIMIT
        );
        assert_eq!(
            normalize_limit(Some(25)).expect("custom limit in range should pass"),
            25
        );
        let error = normalize_limit(Some(0)).expect_err("lower bound should fail");
        assert_eq!(error.code, ReportingExportReasonCode::InvalidPayload.code());
        let error =
            normalize_limit(Some(MAX_EXPORT_LIST_LIMIT + 1)).expect_err("upper bound should fail");
        assert_eq!(error.code, ReportingExportReasonCode::InvalidPayload.code());
    }

    #[test]
    fn constraint_failure_taxonomy_covers_expected_sqlstates() {
        assert!(is_constraint_sqlstate("23505"));
        assert!(is_constraint_sqlstate("55000"));
        assert!(!is_constraint_sqlstate("22000"));
    }

    #[test]
    fn failure_metadata_must_be_valid_json() {
        let mut job = sample_job(ReportingExportJobState::Failed);
        job.failure_metadata = Some("not-json".to_string());
        let error = canonicalize_job(&job).expect_err("invalid json should fail");
        assert_eq!(error.code, ReportingExportReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "failure_metadata")
        );
    }
}
