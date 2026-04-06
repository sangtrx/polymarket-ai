use domain::recovery::{
    RecoveryContractError, RecoveryReasonCode, RecoveryValidationIssue, validate_checksum_digest,
};
use domain::recovery_rehearsal::{
    RestoreRehearsalRunEvidence, normalize_incident_severity, validate_backup_integrity_check_item,
    validate_restore_rehearsal_run_evidence,
};
use domain::risk::normalize_risk_limit_identifier;
use sqlx::{PgPool, Row};
use std::error::Error;
use std::fmt::{Display, Formatter};

const UPSERT_RESTORE_REHEARSAL_RUN_SQL: &str = r#"
    INSERT INTO restore_rehearsal_runs (
        run_id,
        correlation_id,
        incident_correlation_id,
        incident_severity,
        artifact_id,
        artifact_checksum,
        observed_checksum,
        restore_target,
        rehearsal_status,
        reason_code,
        reconciliation_run_id,
        reconciliation_mismatch_rate,
        reconciliation_passed,
        deterministic_signature,
        prior_deterministic_signature,
        deterministic_signature_match,
        deterministic_mismatch_summary,
        requested_at_utc,
        started_at_utc,
        completed_at_utc,
        audit_reference,
        evidence
    ) VALUES (
        $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
        $11, $12, $13, $14, $15, $16, $17,
        $18::timestamptz, $19::timestamptz, $20::timestamptz, $21, $22
    )
    ON CONFLICT (run_id) DO UPDATE SET
        correlation_id = EXCLUDED.correlation_id,
        incident_correlation_id = EXCLUDED.incident_correlation_id,
        incident_severity = EXCLUDED.incident_severity,
        artifact_id = EXCLUDED.artifact_id,
        artifact_checksum = EXCLUDED.artifact_checksum,
        observed_checksum = EXCLUDED.observed_checksum,
        restore_target = EXCLUDED.restore_target,
        rehearsal_status = EXCLUDED.rehearsal_status,
        reason_code = EXCLUDED.reason_code,
        reconciliation_run_id = EXCLUDED.reconciliation_run_id,
        reconciliation_mismatch_rate = EXCLUDED.reconciliation_mismatch_rate,
        reconciliation_passed = EXCLUDED.reconciliation_passed,
        deterministic_signature = EXCLUDED.deterministic_signature,
        prior_deterministic_signature = EXCLUDED.prior_deterministic_signature,
        deterministic_signature_match = EXCLUDED.deterministic_signature_match,
        deterministic_mismatch_summary = EXCLUDED.deterministic_mismatch_summary,
        requested_at_utc = EXCLUDED.requested_at_utc,
        started_at_utc = EXCLUDED.started_at_utc,
        completed_at_utc = EXCLUDED.completed_at_utc,
        audit_reference = EXCLUDED.audit_reference,
        evidence = EXCLUDED.evidence,
        recorded_at_utc = NOW()
"#;

const DELETE_BACKUP_INTEGRITY_CHECKS_SQL: &str = r#"
    DELETE FROM backup_integrity_checks
    WHERE run_id = $1
"#;

const INSERT_BACKUP_INTEGRITY_CHECK_SQL: &str = r#"
    INSERT INTO backup_integrity_checks (
        check_id,
        run_id,
        check_name,
        passed,
        reason_code,
        expected_value,
        observed_value,
        details
    ) VALUES (
        $1, $2, $3, $4, $5, $6, $7, $8
    )
"#;

const LOAD_REHEARSAL_BY_RUN_ID_SQL: &str = r#"
    SELECT evidence
    FROM restore_rehearsal_runs
    WHERE run_id = $1
    ORDER BY completed_at_utc DESC, run_id DESC
    LIMIT 1
"#;

const LOAD_REHEARSALS_BY_ARTIFACT_SQL: &str = r#"
    SELECT evidence
    FROM restore_rehearsal_runs
    WHERE artifact_id = $1
    ORDER BY completed_at_utc DESC, run_id DESC
    LIMIT $2
"#;

const LOAD_REHEARSALS_BY_CORRELATION_SQL: &str = r#"
    SELECT evidence
    FROM restore_rehearsal_runs
    WHERE correlation_id = $1 OR incident_correlation_id = $1
    ORDER BY completed_at_utc DESC, run_id DESC
    LIMIT $2
"#;

const LOAD_LATEST_SUCCESSFUL_REHEARSAL_BY_ARTIFACT_SQL: &str = r#"
    SELECT evidence
    FROM restore_rehearsal_runs
    WHERE artifact_id = $1
      AND rehearsal_status = 'passed'
    ORDER BY completed_at_utc DESC, run_id DESC
    LIMIT 1
"#;

const LOAD_LATEST_SUCCESSFUL_REHEARSAL_BY_CORRELATION_SQL: &str = r#"
    SELECT evidence
    FROM restore_rehearsal_runs
    WHERE (correlation_id = $1 OR incident_correlation_id = $1)
      AND rehearsal_status = 'passed'
    ORDER BY completed_at_utc DESC, run_id DESC
    LIMIT 1
"#;

const REHEARSAL_QUERY_FAILED: &str = "restore_rehearsal_query_failed";
const REHEARSAL_CONSTRAINT_VIOLATION: &str = "restore_rehearsal_constraint_violation";
const REHEARSAL_ROW_DECODE_FAILED: &str = "restore_rehearsal_row_decode_failed";
const REHEARSAL_RUNTIME_UNAVAILABLE: &str = "restore_rehearsal_runtime_unavailable";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreRehearsalPersistenceError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<RecoveryValidationIssue>,
}

impl RestoreRehearsalPersistenceError {
    fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<RecoveryValidationIssue>,
    ) -> Self {
        Self {
            code: RecoveryReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn query_failed(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: REHEARSAL_QUERY_FAILED,
            message: format!("{operation} failed: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn constraint_violation(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: REHEARSAL_CONSTRAINT_VIOLATION,
            message: format!("{operation} rejected by constraint: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn row_decode_failure(column: &'static str, error: impl Display) -> Self {
        Self {
            code: REHEARSAL_ROW_DECODE_FAILED,
            message: format!("unable to decode `{column}`: {error}"),
            field_errors: Vec::new(),
        }
    }

    pub fn runtime_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: REHEARSAL_RUNTIME_UNAVAILABLE,
            message: message.into(),
            field_errors: Vec::new(),
        }
    }
}

impl Display for RestoreRehearsalPersistenceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for RestoreRehearsalPersistenceError {}

pub async fn upsert_restore_rehearsal_run(
    pool: &PgPool,
    run: &RestoreRehearsalRunEvidence,
) -> Result<(), RestoreRehearsalPersistenceError> {
    let canonical = canonicalize_rehearsal_evidence(run)?;
    let evidence = serde_json::to_value(&canonical).map_err(|error| {
        RestoreRehearsalPersistenceError::invalid_payload(
            format!("unable to serialize restore rehearsal evidence: {error}"),
            Vec::new(),
        )
    })?;

    let mut tx = pool
        .begin()
        .await
        .map_err(|error| classify_query_error("begin_restore_rehearsal_tx", error))?;

    let run_result = sqlx::query(UPSERT_RESTORE_REHEARSAL_RUN_SQL)
        .bind(&canonical.run_id)
        .bind(&canonical.correlation_id)
        .bind(canonical.incident_correlation_id.as_deref())
        .bind(canonical.incident_severity.as_deref())
        .bind(&canonical.artifact_id)
        .bind(&canonical.artifact_checksum)
        .bind(&canonical.observed_checksum)
        .bind(&canonical.restore_target)
        .bind(canonical.status.as_str())
        .bind(&canonical.reason_code)
        .bind(&canonical.reconciliation_run_id)
        .bind(canonical.reconciliation_mismatch_rate)
        .bind(canonical.reconciliation_passed)
        .bind(&canonical.deterministic_signature.deterministic_signature)
        .bind(
            canonical
                .deterministic_signature
                .prior_deterministic_signature
                .as_deref(),
        )
        .bind(canonical.deterministic_signature.deterministic_match)
        .bind(
            canonical
                .deterministic_signature
                .mismatch_summary
                .as_deref(),
        )
        .bind(&canonical.requested_at_utc)
        .bind(&canonical.started_at_utc)
        .bind(&canonical.completed_at_utc)
        .bind(canonical.audit_reference.as_deref())
        .bind(evidence)
        .execute(&mut *tx)
        .await
        .map_err(|error| classify_query_error("upsert_restore_rehearsal_run", error))?;

    if run_result.rows_affected() != 1 {
        return Err(RestoreRehearsalPersistenceError::invalid_payload(
            format!(
                "upsert_restore_rehearsal_run expected 1 affected row, got {}",
                run_result.rows_affected()
            ),
            Vec::new(),
        ));
    }

    sqlx::query(DELETE_BACKUP_INTEGRITY_CHECKS_SQL)
        .bind(&canonical.run_id)
        .execute(&mut *tx)
        .await
        .map_err(|error| classify_query_error("delete_backup_integrity_checks", error))?;

    for check in &canonical.integrity_checks {
        let check_id = compose_check_identifier(&canonical.run_id, &check.check_name);
        let check_result = sqlx::query(INSERT_BACKUP_INTEGRITY_CHECK_SQL)
            .bind(&check_id)
            .bind(&canonical.run_id)
            .bind(&check.check_name)
            .bind(check.passed)
            .bind(&check.reason_code)
            .bind(check.expected_value.as_deref())
            .bind(check.observed_value.as_deref())
            .bind(&check.details)
            .execute(&mut *tx)
            .await
            .map_err(|error| classify_query_error("insert_backup_integrity_check", error))?;
        if check_result.rows_affected() != 1 {
            return Err(RestoreRehearsalPersistenceError::invalid_payload(
                format!(
                    "insert_backup_integrity_check expected 1 affected row, got {}",
                    check_result.rows_affected()
                ),
                Vec::new(),
            ));
        }
    }

    tx.commit()
        .await
        .map_err(|error| classify_query_error("commit_restore_rehearsal_tx", error))?;
    Ok(())
}

pub async fn load_restore_rehearsal_run_by_run_id(
    pool: &PgPool,
    run_id: &str,
) -> Result<Option<RestoreRehearsalRunEvidence>, RestoreRehearsalPersistenceError> {
    let normalized_run_id = normalize_lookup_identifier("run_id", run_id)?;
    let row = sqlx::query(LOAD_REHEARSAL_BY_RUN_ID_SQL)
        .bind(normalized_run_id)
        .fetch_optional(pool)
        .await
        .map_err(|error| classify_query_error("load_restore_rehearsal_run_by_run_id", error))?;
    row.map(decode_rehearsal_row).transpose()
}

pub async fn load_latest_restore_rehearsals_by_artifact(
    pool: &PgPool,
    artifact_id: &str,
    limit: i64,
) -> Result<Vec<RestoreRehearsalRunEvidence>, RestoreRehearsalPersistenceError> {
    let normalized_artifact_id = normalize_lookup_identifier("artifact_id", artifact_id)?;
    let normalized_limit = validate_limit(limit)?;
    let rows = sqlx::query(LOAD_REHEARSALS_BY_ARTIFACT_SQL)
        .bind(normalized_artifact_id)
        .bind(normalized_limit)
        .fetch_all(pool)
        .await
        .map_err(|error| {
            classify_query_error("load_latest_restore_rehearsals_by_artifact", error)
        })?;
    decode_rehearsal_rows(rows)
}

pub async fn load_latest_restore_rehearsals_by_correlation(
    pool: &PgPool,
    correlation_id: &str,
    limit: i64,
) -> Result<Vec<RestoreRehearsalRunEvidence>, RestoreRehearsalPersistenceError> {
    let normalized_correlation_id = normalize_lookup_identifier("correlation_id", correlation_id)?;
    let normalized_limit = validate_limit(limit)?;
    let rows = sqlx::query(LOAD_REHEARSALS_BY_CORRELATION_SQL)
        .bind(normalized_correlation_id)
        .bind(normalized_limit)
        .fetch_all(pool)
        .await
        .map_err(|error| {
            classify_query_error("load_latest_restore_rehearsals_by_correlation", error)
        })?;
    decode_rehearsal_rows(rows)
}

pub async fn load_latest_successful_restore_rehearsal_by_artifact(
    pool: &PgPool,
    artifact_id: &str,
) -> Result<Option<RestoreRehearsalRunEvidence>, RestoreRehearsalPersistenceError> {
    let normalized_artifact_id = normalize_lookup_identifier("artifact_id", artifact_id)?;
    let row = sqlx::query(LOAD_LATEST_SUCCESSFUL_REHEARSAL_BY_ARTIFACT_SQL)
        .bind(normalized_artifact_id)
        .fetch_optional(pool)
        .await
        .map_err(|error| {
            classify_query_error(
                "load_latest_successful_restore_rehearsal_by_artifact",
                error,
            )
        })?;
    row.map(decode_rehearsal_row).transpose()
}

pub async fn load_latest_successful_restore_rehearsal_by_correlation(
    pool: &PgPool,
    correlation_id: &str,
) -> Result<Option<RestoreRehearsalRunEvidence>, RestoreRehearsalPersistenceError> {
    let normalized_correlation_id = normalize_lookup_identifier("correlation_id", correlation_id)?;
    let row = sqlx::query(LOAD_LATEST_SUCCESSFUL_REHEARSAL_BY_CORRELATION_SQL)
        .bind(normalized_correlation_id)
        .fetch_optional(pool)
        .await
        .map_err(|error| {
            classify_query_error(
                "load_latest_successful_restore_rehearsal_by_correlation",
                error,
            )
        })?;
    row.map(decode_rehearsal_row).transpose()
}

fn decode_rehearsal_rows(
    rows: Vec<sqlx::postgres::PgRow>,
) -> Result<Vec<RestoreRehearsalRunEvidence>, RestoreRehearsalPersistenceError> {
    rows.into_iter().map(decode_rehearsal_row).collect()
}

fn decode_rehearsal_row(
    row: sqlx::postgres::PgRow,
) -> Result<RestoreRehearsalRunEvidence, RestoreRehearsalPersistenceError> {
    let evidence: serde_json::Value = row
        .try_get("evidence")
        .map_err(|error| RestoreRehearsalPersistenceError::row_decode_failure("evidence", error))?;
    let run: RestoreRehearsalRunEvidence = serde_json::from_value(evidence)
        .map_err(|error| RestoreRehearsalPersistenceError::row_decode_failure("evidence", error))?;
    validate_restore_rehearsal_run_evidence(&run).map_err(map_contract_error)?;
    Ok(run)
}

fn canonicalize_rehearsal_evidence(
    run: &RestoreRehearsalRunEvidence,
) -> Result<RestoreRehearsalRunEvidence, RestoreRehearsalPersistenceError> {
    let mut canonical = run.clone();
    canonical.run_id = normalize_lookup_identifier("run_id", &canonical.run_id)?;
    canonical.correlation_id =
        normalize_lookup_identifier("correlation_id", &canonical.correlation_id)?;
    canonical.artifact_id = normalize_lookup_identifier("artifact_id", &canonical.artifact_id)?;
    canonical.restore_target =
        normalize_lookup_identifier("restore_target", &canonical.restore_target)?;
    canonical.reconciliation_run_id =
        normalize_lookup_identifier("reconciliation_run_id", &canonical.reconciliation_run_id)?;
    canonical.artifact_checksum = validate_checksum_digest(&canonical.artifact_checksum)
        .map_err(map_contract_error)?
        .to_string();
    canonical.observed_checksum = validate_checksum_digest(&canonical.observed_checksum)
        .map_err(map_contract_error)?
        .to_string();
    canonical.reason_code = RecoveryReasonCode::parse(&canonical.reason_code)
        .map_err(map_contract_error)?
        .code()
        .to_string();
    canonical.incident_correlation_id = canonical
        .incident_correlation_id
        .as_deref()
        .map(|value| normalize_lookup_identifier("incident_correlation_id", value))
        .transpose()?;
    canonical.incident_severity = canonical
        .incident_severity
        .as_deref()
        .map(|value| {
            normalize_incident_severity(value).ok_or_else(|| {
                RestoreRehearsalPersistenceError::invalid_payload(
                    "incident_severity must normalize to severity_1, severity_2, severity_3, or severity_4",
                    vec![RecoveryValidationIssue {
                        field: "incident_severity",
                        code: RecoveryReasonCode::InvalidPayload.code(),
                        message: "incident_severity must be one of severity_1, severity_2, severity_3, or severity_4".to_string(),
                    }],
                )
            })
        })
        .transpose()?;
    canonical.audit_reference = normalize_optional(canonical.audit_reference.as_deref());
    canonical.deterministic_signature.deterministic_signature =
        validate_checksum_digest(&canonical.deterministic_signature.deterministic_signature)
            .map_err(map_contract_error)?
            .to_string();
    if let Some(prior_signature) = canonical
        .deterministic_signature
        .prior_deterministic_signature
        .as_deref()
    {
        canonical
            .deterministic_signature
            .prior_deterministic_signature = Some(
            validate_checksum_digest(prior_signature)
                .map_err(map_contract_error)?
                .to_string(),
        );
    }
    for check in &mut canonical.integrity_checks {
        validate_backup_integrity_check_item(check).map_err(map_contract_error)?;
        check.check_name = normalize_risk_limit_identifier(&check.check_name);
        check.reason_code = RecoveryReasonCode::parse(&check.reason_code)
            .map_err(map_contract_error)?
            .code()
            .to_string();
    }
    validate_restore_rehearsal_run_evidence(&canonical).map_err(map_contract_error)?;
    Ok(canonical)
}

fn compose_check_identifier(run_id: &str, check_name: &str) -> String {
    format!(
        "{}::{}",
        normalize_risk_limit_identifier(run_id),
        normalize_risk_limit_identifier(check_name)
    )
}

fn normalize_lookup_identifier(
    field: &'static str,
    value: &str,
) -> Result<String, RestoreRehearsalPersistenceError> {
    let normalized = normalize_risk_limit_identifier(value);
    if normalized.is_empty() {
        return Err(RestoreRehearsalPersistenceError::invalid_payload(
            format!("{field} must not be empty"),
            vec![RecoveryValidationIssue {
                field,
                code: RecoveryReasonCode::InvalidPayload.code(),
                message: format!("{field} must not be empty"),
            }],
        ));
    }
    Ok(normalized)
}

fn validate_limit(limit: i64) -> Result<i64, RestoreRehearsalPersistenceError> {
    if !(1..=200).contains(&limit) {
        return Err(RestoreRehearsalPersistenceError::invalid_payload(
            "limit must be within 1..=200".to_string(),
            vec![RecoveryValidationIssue {
                field: "limit",
                code: RecoveryReasonCode::InvalidPayload.code(),
                message: "limit must be within 1..=200".to_string(),
            }],
        ));
    }
    Ok(limit)
}

fn normalize_optional(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|candidate| !candidate.is_empty())
        .map(ToOwned::to_owned)
}

fn map_contract_error(error: RecoveryContractError) -> RestoreRehearsalPersistenceError {
    RestoreRehearsalPersistenceError {
        code: error.code,
        message: error.message,
        field_errors: error.field_errors,
    }
}

fn classify_query_error(
    operation: &'static str,
    error: sqlx::Error,
) -> RestoreRehearsalPersistenceError {
    if is_constraint_error(&error) {
        return RestoreRehearsalPersistenceError::constraint_violation(operation, error);
    }
    RestoreRehearsalPersistenceError::query_failed(operation, error)
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
    use domain::recovery_rehearsal::{
        BackupIntegrityCheckItem, DeterministicReplaySignatureEvidence, RestoreRehearsalStatus,
    };

    const REHEARSAL_MIGRATION_SQL: &str =
        include_str!("../../migrations/20260406234500_restore_rehearsal_runs.sql");

    #[test]
    fn migration_creates_only_story_3_8_schema_scope() {
        assert!(
            REHEARSAL_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS restore_rehearsal_runs")
        );
        assert!(
            REHEARSAL_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS backup_integrity_checks")
        );
        assert!(!REHEARSAL_MIGRATION_SQL.contains("recovery_gate_runs"));
        assert!(!REHEARSAL_MIGRATION_SQL.contains("approval_requests"));
        assert!(!REHEARSAL_MIGRATION_SQL.contains("incident_alerts"));
    }

    #[test]
    fn migration_enforces_checksum_and_deterministic_constraints_and_indexes() {
        assert!(REHEARSAL_MIGRATION_SQL.contains("artifact_checksum ~ '^[0-9a-f]{64}$'"));
        assert!(REHEARSAL_MIGRATION_SQL.contains("deterministic_signature ~ '^[0-9a-f]{64}$'"));
        assert!(REHEARSAL_MIGRATION_SQL.contains("rehearsal_status IN ('passed', 'failed')"));
        assert!(REHEARSAL_MIGRATION_SQL.contains("idx_restore_rehearsal_runs_artifact_time"));
        assert!(REHEARSAL_MIGRATION_SQL.contains("idx_restore_rehearsal_runs_correlation_time"));
        assert!(
            REHEARSAL_MIGRATION_SQL
                .contains("idx_restore_rehearsal_runs_incident_correlation_time")
        );
        assert!(REHEARSAL_MIGRATION_SQL.contains("idx_restore_rehearsal_runs_status_time"));
        assert!(REHEARSAL_MIGRATION_SQL.contains("idx_backup_integrity_checks_run_time"));
    }

    #[test]
    fn adapter_queries_preserve_deterministic_ordering_contract() {
        assert!(LOAD_REHEARSAL_BY_RUN_ID_SQL.contains("WHERE run_id = $1"));
        assert!(
            LOAD_REHEARSALS_BY_ARTIFACT_SQL.contains("ORDER BY completed_at_utc DESC, run_id DESC")
        );
        assert!(
            LOAD_REHEARSALS_BY_CORRELATION_SQL
                .contains("ORDER BY completed_at_utc DESC, run_id DESC")
        );
        assert!(
            LOAD_LATEST_SUCCESSFUL_REHEARSAL_BY_ARTIFACT_SQL
                .contains("rehearsal_status = 'passed'")
        );
        assert!(
            LOAD_LATEST_SUCCESSFUL_REHEARSAL_BY_CORRELATION_SQL
                .contains("rehearsal_status = 'passed'")
        );
    }

    #[test]
    fn canonicalization_normalizes_identifiers_reason_codes_and_severity() {
        let mut run = sample_run();
        run.run_id = " RUN-1 ".to_string();
        run.artifact_id = " ARTIFACT-1 ".to_string();
        run.incident_severity = Some("Severity-1".to_string());
        run.reason_code = RecoveryReasonCode::RehearsalSuccess.code().to_string();
        let canonical =
            canonicalize_rehearsal_evidence(&run).expect("canonicalization should pass");
        assert_eq!(canonical.run_id, "run-1");
        assert_eq!(canonical.artifact_id, "artifact-1");
        assert_eq!(canonical.incident_severity.as_deref(), Some("severity_1"));
        assert_eq!(
            canonical.reason_code,
            RecoveryReasonCode::RehearsalSuccess.code().to_string()
        );
    }

    #[test]
    fn canonicalization_rejects_invalid_incident_severity() {
        let mut run = sample_run();
        run.incident_severity = Some("severity_9".to_string());
        let error = canonicalize_rehearsal_evidence(&run)
            .expect_err("invalid incident severity must be rejected");
        assert_eq!(error.code, RecoveryReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "incident_severity")
        );
    }

    #[test]
    fn lookup_identifier_rejects_empty_values() {
        let error = normalize_lookup_identifier("artifact_id", "   ")
            .expect_err("empty lookup values must fail");
        assert_eq!(error.code, RecoveryReasonCode::InvalidPayload.code());
        assert_eq!(error.field_errors[0].field, "artifact_id");
    }

    fn sample_run() -> RestoreRehearsalRunEvidence {
        RestoreRehearsalRunEvidence {
            run_id: "run-1".to_string(),
            correlation_id: "corr-1".to_string(),
            artifact_id: "artifact-1".to_string(),
            artifact_checksum: "a".repeat(64),
            observed_checksum: "a".repeat(64),
            restore_target: "sandbox-a".to_string(),
            reconciliation_run_id: "recon-1".to_string(),
            reconciliation_mismatch_rate: Some(0.0002),
            reconciliation_passed: true,
            status: RestoreRehearsalStatus::Passed,
            reason_code: RecoveryReasonCode::RehearsalSuccess.code().to_string(),
            requested_at_utc: "2026-04-06T12:00:00Z".to_string(),
            started_at_utc: "2026-04-06T12:00:00Z".to_string(),
            completed_at_utc: "2026-04-06T12:00:02Z".to_string(),
            integrity_checks: vec![BackupIntegrityCheckItem {
                check_name: "checksum_match".to_string(),
                passed: true,
                reason_code: RecoveryReasonCode::RehearsalSuccess.code().to_string(),
                expected_value: Some("a".repeat(64)),
                observed_value: Some("a".repeat(64)),
                details: "checksum matched".to_string(),
            }],
            deterministic_signature: DeterministicReplaySignatureEvidence {
                deterministic_signature: "b".repeat(64),
                prior_deterministic_signature: Some("b".repeat(64)),
                deterministic_match: Some(true),
                mismatch_summary: None,
            },
            incident_correlation_id: Some("incident-corr-1".to_string()),
            incident_severity: Some("severity_2".to_string()),
            audit_reference: Some("arb-2026-3008".to_string()),
        }
    }
}
