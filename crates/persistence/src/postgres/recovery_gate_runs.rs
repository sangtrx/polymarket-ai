use domain::recovery::{
    RecoveryContractError, RecoveryGateRunEvidence, RecoveryReasonCode, RecoveryValidationIssue,
    validate_checksum_digest, validate_recovery_gate_run_evidence,
};
use domain::risk::normalize_risk_limit_identifier;
use sqlx::{PgExecutor, Row};
use std::error::Error;
use std::fmt::{Display, Formatter};

const INSERT_RECOVERY_GATE_RUN_SQL: &str = r#"
    INSERT INTO recovery_gate_runs (
        run_id,
        correlation_id,
        profile_key,
        readiness_status,
        reason_code,
        actor_id,
        actor_role,
        reconciliation_run_id,
        approved_checksum,
        computed_checksum,
        requested_at_utc,
        evaluated_at_utc,
        resumed_at_utc,
        evidence
    ) VALUES (
        $1,
        $2,
        $3,
        $4,
        $5,
        $6,
        $7,
        $8,
        $9,
        $10,
        $11::timestamptz,
        $12::timestamptz,
        $13::timestamptz,
        $14
    )
    ON CONFLICT (run_id) DO UPDATE SET
        correlation_id = EXCLUDED.correlation_id,
        profile_key = EXCLUDED.profile_key,
        readiness_status = EXCLUDED.readiness_status,
        reason_code = EXCLUDED.reason_code,
        actor_id = EXCLUDED.actor_id,
        actor_role = EXCLUDED.actor_role,
        reconciliation_run_id = EXCLUDED.reconciliation_run_id,
        approved_checksum = EXCLUDED.approved_checksum,
        computed_checksum = EXCLUDED.computed_checksum,
        requested_at_utc = EXCLUDED.requested_at_utc,
        evaluated_at_utc = EXCLUDED.evaluated_at_utc,
        resumed_at_utc = EXCLUDED.resumed_at_utc,
        evidence = EXCLUDED.evidence,
        recorded_at_utc = NOW()
"#;

const LOAD_RECOVERY_GATE_RUN_BY_RUN_ID_SQL: &str = r#"
    SELECT evidence
    FROM recovery_gate_runs
    WHERE run_id = $1
    ORDER BY evaluated_at_utc DESC, run_id DESC
    LIMIT 1
"#;

const LOAD_LATEST_RECOVERY_GATE_RUN_BY_CORRELATION_SQL: &str = r#"
    SELECT evidence
    FROM recovery_gate_runs
    WHERE correlation_id = $1
    ORDER BY evaluated_at_utc DESC, run_id DESC
    LIMIT 1
"#;

const LOAD_LATEST_RECOVERY_GATE_RUN_SQL: &str = r#"
    SELECT evidence
    FROM recovery_gate_runs
    ORDER BY evaluated_at_utc DESC, run_id DESC
    LIMIT 1
"#;

const LOAD_LATEST_APPROVED_RECOVERY_GATE_RUN_SQL: &str = r#"
    SELECT evidence
    FROM recovery_gate_runs
    WHERE readiness_status = 'approved'
    ORDER BY evaluated_at_utc DESC, run_id DESC
    LIMIT 1
"#;

const RECOVERY_GATE_CONSTRAINT_VIOLATION: &str = "recovery_gate_constraint_violation";
const RECOVERY_GATE_QUERY_FAILED: &str = "recovery_gate_query_failed";
const RECOVERY_GATE_ROW_DECODE_FAILED: &str = "recovery_gate_row_decode_failed";
const RECOVERY_GATE_RUNTIME_UNAVAILABLE: &str = "recovery_gate_runtime_unavailable";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryGateRunPersistenceError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<RecoveryValidationIssue>,
}

impl RecoveryGateRunPersistenceError {
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
            code: RECOVERY_GATE_QUERY_FAILED,
            message: format!("{operation} failed: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn constraint_violation(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: RECOVERY_GATE_CONSTRAINT_VIOLATION,
            message: format!("{operation} rejected by constraint: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn row_decode_failure(column: &'static str, error: impl Display) -> Self {
        Self {
            code: RECOVERY_GATE_ROW_DECODE_FAILED,
            message: format!("unable to decode `{column}`: {error}"),
            field_errors: Vec::new(),
        }
    }

    pub fn runtime_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: RECOVERY_GATE_RUNTIME_UNAVAILABLE,
            message: message.into(),
            field_errors: Vec::new(),
        }
    }
}

impl Display for RecoveryGateRunPersistenceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for RecoveryGateRunPersistenceError {}

pub async fn insert_recovery_gate_run<'e, E>(
    executor: E,
    run: &RecoveryGateRunEvidence,
) -> Result<(), RecoveryGateRunPersistenceError>
where
    E: PgExecutor<'e>,
{
    let canonical = canonicalize_run_evidence(run)?;
    let evidence = serde_json::to_value(&canonical).map_err(|error| {
        RecoveryGateRunPersistenceError::invalid_payload(
            format!("unable to serialize recovery gate run evidence: {error}"),
            Vec::new(),
        )
    })?;
    let result = sqlx::query(INSERT_RECOVERY_GATE_RUN_SQL)
        .bind(&canonical.run_id)
        .bind(&canonical.correlation_id)
        .bind(&canonical.profile_key)
        .bind(canonical.readiness_status.as_str())
        .bind(&canonical.reason_code)
        .bind(&canonical.actor_id)
        .bind(&canonical.actor_role)
        .bind(canonical.reconciliation_run_id.as_deref())
        .bind(canonical.approved_checksum.as_deref())
        .bind(canonical.computed_checksum.as_deref())
        .bind(&canonical.requested_at_utc)
        .bind(&canonical.evaluated_at_utc)
        .bind(canonical.resumed_at_utc.as_deref())
        .bind(evidence)
        .execute(executor)
        .await
        .map_err(|error| classify_query_error("insert_recovery_gate_run", error))?;
    if result.rows_affected() != 1 {
        return Err(RecoveryGateRunPersistenceError::invalid_payload(
            format!(
                "insert_recovery_gate_run expected 1 affected row, got {}",
                result.rows_affected()
            ),
            Vec::new(),
        ));
    }
    Ok(())
}

pub async fn load_recovery_gate_run_by_run_id<'e, E>(
    executor: E,
    run_id: &str,
) -> Result<Option<RecoveryGateRunEvidence>, RecoveryGateRunPersistenceError>
where
    E: PgExecutor<'e>,
{
    let normalized_run_id = normalize_lookup_identifier("run_id", run_id)?;
    let row = sqlx::query(LOAD_RECOVERY_GATE_RUN_BY_RUN_ID_SQL)
        .bind(&normalized_run_id)
        .fetch_optional(executor)
        .await
        .map_err(|error| classify_query_error("load_recovery_gate_run_by_run_id", error))?;
    row.map(decode_recovery_gate_run_row).transpose()
}

pub async fn load_latest_recovery_gate_run_by_correlation<'e, E>(
    executor: E,
    correlation_id: &str,
) -> Result<Option<RecoveryGateRunEvidence>, RecoveryGateRunPersistenceError>
where
    E: PgExecutor<'e>,
{
    let normalized_correlation_id = normalize_lookup_identifier("correlation_id", correlation_id)?;
    let row = sqlx::query(LOAD_LATEST_RECOVERY_GATE_RUN_BY_CORRELATION_SQL)
        .bind(&normalized_correlation_id)
        .fetch_optional(executor)
        .await
        .map_err(|error| {
            classify_query_error("load_latest_recovery_gate_run_by_correlation", error)
        })?;
    row.map(decode_recovery_gate_run_row).transpose()
}

pub async fn load_latest_recovery_gate_run<'e, E>(
    executor: E,
) -> Result<Option<RecoveryGateRunEvidence>, RecoveryGateRunPersistenceError>
where
    E: PgExecutor<'e>,
{
    let row = sqlx::query(LOAD_LATEST_RECOVERY_GATE_RUN_SQL)
        .fetch_optional(executor)
        .await
        .map_err(|error| classify_query_error("load_latest_recovery_gate_run", error))?;
    row.map(decode_recovery_gate_run_row).transpose()
}

pub async fn load_latest_approved_recovery_gate_run<'e, E>(
    executor: E,
) -> Result<Option<RecoveryGateRunEvidence>, RecoveryGateRunPersistenceError>
where
    E: PgExecutor<'e>,
{
    let row = sqlx::query(LOAD_LATEST_APPROVED_RECOVERY_GATE_RUN_SQL)
        .fetch_optional(executor)
        .await
        .map_err(|error| classify_query_error("load_latest_approved_recovery_gate_run", error))?;
    row.map(decode_recovery_gate_run_row).transpose()
}

fn canonicalize_run_evidence(
    run: &RecoveryGateRunEvidence,
) -> Result<RecoveryGateRunEvidence, RecoveryGateRunPersistenceError> {
    let mut canonical = run.clone();
    canonical.run_id = normalize_lookup_identifier("run_id", &canonical.run_id)?;
    canonical.correlation_id =
        normalize_lookup_identifier("correlation_id", &canonical.correlation_id)?;
    canonical.profile_key = normalize_lookup_identifier("profile_key", &canonical.profile_key)?;
    canonical.actor_id = normalize_lookup_identifier("actor_id", &canonical.actor_id)?;
    canonical.actor_role = normalize_lookup_identifier("actor_role", &canonical.actor_role)?;
    canonical.reason_code = RecoveryReasonCode::parse(&canonical.reason_code)
        .map_err(map_contract_error)?
        .code()
        .to_string();
    if let Some(value) = canonical.reconciliation_run_id.as_deref() {
        canonical.reconciliation_run_id =
            Some(normalize_lookup_identifier("reconciliation_run_id", value)?);
    }
    if let Some(value) = canonical.approved_checksum.as_deref() {
        canonical.approved_checksum = Some(
            validate_checksum_digest(value)
                .map_err(map_contract_error)?
                .to_string(),
        );
    }
    if let Some(value) = canonical.computed_checksum.as_deref() {
        canonical.computed_checksum = Some(
            validate_checksum_digest(value)
                .map_err(map_contract_error)?
                .to_string(),
        );
    }
    if let Some(signoff) = canonical.signoff.as_mut() {
        signoff.actor_id = normalize_lookup_identifier("signoff.actor_id", &signoff.actor_id)?;
        signoff.actor_role =
            normalize_lookup_identifier("signoff.actor_role", &signoff.actor_role)?;
    }
    canonical.audit_reference = normalize_optional_field(canonical.audit_reference.as_deref());
    for code in &mut canonical.failing_gate_codes {
        *code = RecoveryReasonCode::parse(code)
            .map_err(map_contract_error)?
            .code()
            .to_string();
    }
    for outcome in &mut canonical.gate_outcomes {
        outcome.reason_code = RecoveryReasonCode::parse(&outcome.reason_code)
            .map_err(map_contract_error)?
            .code()
            .to_string();
    }
    validate_recovery_gate_run_evidence(&canonical).map_err(map_contract_error)?;
    Ok(canonical)
}

fn decode_recovery_gate_run_row(
    row: sqlx::postgres::PgRow,
) -> Result<RecoveryGateRunEvidence, RecoveryGateRunPersistenceError> {
    let evidence: serde_json::Value = row
        .try_get("evidence")
        .map_err(|error| RecoveryGateRunPersistenceError::row_decode_failure("evidence", error))?;
    let run: RecoveryGateRunEvidence = serde_json::from_value(evidence)
        .map_err(|error| RecoveryGateRunPersistenceError::row_decode_failure("evidence", error))?;
    validate_recovery_gate_run_evidence(&run).map_err(map_contract_error)?;
    Ok(run)
}

fn normalize_lookup_identifier(
    field: &'static str,
    value: &str,
) -> Result<String, RecoveryGateRunPersistenceError> {
    let normalized = normalize_risk_limit_identifier(value);
    if normalized.is_empty() {
        return Err(RecoveryGateRunPersistenceError::invalid_payload(
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

fn normalize_optional_field(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|candidate| !candidate.is_empty())
        .map(ToOwned::to_owned)
}

fn map_contract_error(error: RecoveryContractError) -> RecoveryGateRunPersistenceError {
    RecoveryGateRunPersistenceError {
        code: error.code,
        message: error.message,
        field_errors: error.field_errors,
    }
}

fn classify_query_error(
    operation: &'static str,
    error: sqlx::Error,
) -> RecoveryGateRunPersistenceError {
    if is_constraint_error(&error) {
        return RecoveryGateRunPersistenceError::constraint_violation(operation, error);
    }
    RecoveryGateRunPersistenceError::query_failed(operation, error)
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
    use domain::recovery::{
        RecoveryGateName, RecoveryOperatorSignoff, RecoveryReadinessStatus, RecoveryReasonCode,
    };

    const RECOVERY_GATE_MIGRATION_SQL: &str =
        include_str!("../../migrations/20260406223000_recovery_gate_runs.sql");

    #[test]
    fn migration_creates_expected_recovery_gate_schema_scope() {
        assert!(
            RECOVERY_GATE_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS recovery_gate_runs")
        );
        assert!(!RECOVERY_GATE_MIGRATION_SQL.contains("risk_limit_profiles"));
        assert!(!RECOVERY_GATE_MIGRATION_SQL.contains("safety_control_actions"));
        assert!(!RECOVERY_GATE_MIGRATION_SQL.contains("approval_requests"));
    }

    #[test]
    fn migration_enforces_constraints_and_indexes_for_traceability() {
        assert!(
            RECOVERY_GATE_MIGRATION_SQL.contains("readiness_status IN ('approved', 'blocked')")
        );
        assert!(RECOVERY_GATE_MIGRATION_SQL.contains("approved_checksum ~ '^[0-9a-f]{64}$'"));
        assert!(RECOVERY_GATE_MIGRATION_SQL.contains("idx_recovery_gate_runs_correlation_time"));
        assert!(RECOVERY_GATE_MIGRATION_SQL.contains("idx_recovery_gate_runs_status_time"));
        assert!(RECOVERY_GATE_MIGRATION_SQL.contains("idx_recovery_gate_runs_profile_time"));
    }

    #[test]
    fn canonicalization_normalizes_identifiers_and_reason_codes() {
        let mut run = sample_run();
        run.run_id = " RUN-1 ".to_string();
        run.correlation_id = " CORR-1 ".to_string();
        run.reason_code = "recovery_resume_approved".to_string();
        let canonical = canonicalize_run_evidence(&run).expect("canonicalization should succeed");
        assert_eq!(canonical.run_id, "run-1");
        assert_eq!(canonical.correlation_id, "corr-1");
        assert_eq!(
            canonical.reason_code,
            RecoveryReasonCode::ResumeApproved.code().to_string()
        );
        assert_eq!(
            canonical
                .signoff
                .as_ref()
                .expect("signoff required")
                .actor_role,
            "operational_control"
        );
    }

    #[test]
    fn lookup_identifier_normalization_rejects_empty_values() {
        let error = normalize_lookup_identifier("run_id", "  ")
            .expect_err("empty lookup values must be rejected");
        assert_eq!(error.code, RecoveryReasonCode::InvalidPayload.code());
        assert_eq!(error.field_errors[0].field, "run_id");
    }

    #[test]
    fn latest_lookup_queries_remain_deterministic() {
        assert!(LOAD_RECOVERY_GATE_RUN_BY_RUN_ID_SQL.contains("WHERE run_id = $1"));
        assert!(
            LOAD_LATEST_RECOVERY_GATE_RUN_BY_CORRELATION_SQL.contains("WHERE correlation_id = $1")
        );
        assert!(
            LOAD_LATEST_RECOVERY_GATE_RUN_BY_CORRELATION_SQL
                .contains("ORDER BY evaluated_at_utc DESC, run_id DESC")
        );
        assert!(
            LOAD_LATEST_APPROVED_RECOVERY_GATE_RUN_SQL
                .contains("WHERE readiness_status = 'approved'")
        );
        assert!(
            LOAD_LATEST_RECOVERY_GATE_RUN_SQL
                .contains("ORDER BY evaluated_at_utc DESC, run_id DESC")
        );
    }

    fn sample_run() -> RecoveryGateRunEvidence {
        RecoveryGateRunEvidence {
            run_id: "run-1".to_string(),
            correlation_id: "corr-1".to_string(),
            readiness_status: RecoveryReadinessStatus::Approved,
            reason_code: RecoveryReasonCode::ResumeApproved.code().to_string(),
            actor_id: "ops-1".to_string(),
            actor_role: "operational_control".to_string(),
            profile_key: "default".to_string(),
            requested_at_utc: "2026-04-06T12:00:00Z".to_string(),
            evaluated_at_utc: "2026-04-06T12:00:01Z".to_string(),
            resumed_at_utc: Some("2026-04-06T12:00:02Z".to_string()),
            freshness_age_seconds: Some(12.0),
            freshness_observed_at_utc: Some("2026-04-06T11:59:50Z".to_string()),
            reconciliation_run_id: Some("recon-1".to_string()),
            reconciliation_mismatch_rate: Some(0.0002),
            approved_checksum: Some("a".repeat(64)),
            computed_checksum: Some("a".repeat(64)),
            signoff: Some(RecoveryOperatorSignoff {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                signoff_intent: "I approve recovery resume.".to_string(),
                signed_at_utc: "2026-04-06T12:00:00Z".to_string(),
                audit_reference: Some("arb-1".to_string()),
            }),
            gate_outcomes: vec![
                domain::recovery::RecoveryGateOutcome {
                    gate: RecoveryGateName::Freshness,
                    passed: true,
                    reason_code: RecoveryReasonCode::FreshnessPass.code().to_string(),
                    trigger: "freshness <= 30s".to_string(),
                    context: "freshness_age_seconds=12".to_string(),
                    action: "allow freshness gate".to_string(),
                    verification: "freshness telemetry stable".to_string(),
                },
                domain::recovery::RecoveryGateOutcome {
                    gate: RecoveryGateName::Reconciliation,
                    passed: true,
                    reason_code: RecoveryReasonCode::ReconciliationPass.code().to_string(),
                    trigger: "mismatch < 0.1%".to_string(),
                    context: "mismatch_rate=0.02%".to_string(),
                    action: "allow reconciliation gate".to_string(),
                    verification: "reconciliation run validated".to_string(),
                },
                domain::recovery::RecoveryGateOutcome {
                    gate: RecoveryGateName::RiskChecksum,
                    passed: true,
                    reason_code: RecoveryReasonCode::ChecksumMatch.code().to_string(),
                    trigger: "checksum exact match".to_string(),
                    context: "approved and computed digests equal".to_string(),
                    action: "allow checksum gate".to_string(),
                    verification: "bundle hash validated".to_string(),
                },
                domain::recovery::RecoveryGateOutcome {
                    gate: RecoveryGateName::OperatorSignoff,
                    passed: true,
                    reason_code: RecoveryReasonCode::SignoffRecorded.code().to_string(),
                    trigger: "operator signoff recorded".to_string(),
                    context: "actor and audit reference present".to_string(),
                    action: "allow signoff gate".to_string(),
                    verification: "audit trail attached".to_string(),
                },
            ],
            failing_gate_codes: Vec::new(),
            audit_reference: Some("arb-1".to_string()),
        }
    }
}
