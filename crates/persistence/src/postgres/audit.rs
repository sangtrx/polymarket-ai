use domain::governance::PrivilegedAuditRecord;
use sqlx::PgExecutor;
use std::error::Error;
use std::fmt::{Display, Formatter};
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditPersistenceError {
    pub code: &'static str,
    pub message: String,
}

impl AuditPersistenceError {
    fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: "audit_invalid_payload",
            message: message.into(),
        }
    }

    fn persistence_unavailable(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "audit_persistence_unavailable",
            message: format!("{operation} failed: {error}"),
        }
    }

    fn append_constraint_violation(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "audit_append_constraint_violation",
            message: format!("{operation} rejected by append constraints: {error}"),
        }
    }

    fn unexpected_row_count(operation: &'static str, rows_affected: u64) -> Self {
        Self {
            code: "audit_append_constraint_violation",
            message: format!(
                "{operation} rejected by append constraints: expected 1 row, got {rows_affected}"
            ),
        }
    }
}

impl Display for AuditPersistenceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for AuditPersistenceError {}

pub async fn append_audit_log<'e, E>(
    executor: E,
    record: &PrivilegedAuditRecord,
) -> Result<(), AuditPersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_record(record)?;

    let query_result = sqlx::query(
        r#"
        INSERT INTO audit_log_append (
            actor_id,
            role,
            action_type,
            parameters,
            approval_reference,
            timestamp,
            outcome,
            reason_code,
            correlation_id,
            authentication_outcome
        ) VALUES ($1, $2, $3, $4, $5, $6::timestamptz, $7, $8, $9, $10)
        "#,
    )
    .bind(&record.actor_id)
    .bind(&record.role)
    .bind(&record.action_type)
    .bind(&record.parameters)
    .bind(record.approval_reference.as_deref())
    .bind(&record.timestamp)
    .bind(record.outcome.as_str())
    .bind(&record.reason_code)
    .bind(&record.correlation_id)
    .bind(&record.authentication_outcome)
    .execute(executor)
    .await
    .map_err(|error| classify_query_error("append_audit_log", error))?;

    if query_result.rows_affected() != 1 {
        return Err(AuditPersistenceError::unexpected_row_count(
            "append_audit_log",
            query_result.rows_affected(),
        ));
    }

    Ok(())
}

fn validate_record(record: &PrivilegedAuditRecord) -> Result<(), AuditPersistenceError> {
    if record.actor_id.trim().is_empty() {
        return Err(AuditPersistenceError::invalid_payload(
            "actor_id is required",
        ));
    }

    if record.role.trim().is_empty() {
        return Err(AuditPersistenceError::invalid_payload("role is required"));
    }

    if record.action_type.trim().is_empty() {
        return Err(AuditPersistenceError::invalid_payload(
            "action_type is required",
        ));
    }

    if !record.parameters.is_object() {
        return Err(AuditPersistenceError::invalid_payload(
            "parameters must be a JSON object",
        ));
    }

    if let Some(approval_reference) = &record.approval_reference
        && approval_reference.trim().is_empty()
    {
        return Err(AuditPersistenceError::invalid_payload(
            "approval_reference cannot be blank when provided",
        ));
    }

    if record.reason_code.trim().is_empty() {
        return Err(AuditPersistenceError::invalid_payload(
            "reason_code is required",
        ));
    }

    if record.correlation_id.trim().is_empty() {
        return Err(AuditPersistenceError::invalid_payload(
            "correlation_id is required",
        ));
    }

    if record.authentication_outcome.trim().is_empty() {
        return Err(AuditPersistenceError::invalid_payload(
            "authentication_outcome is required",
        ));
    }

    let timestamp = OffsetDateTime::parse(&record.timestamp, &Rfc3339).map_err(|error| {
        AuditPersistenceError::invalid_payload(format!("timestamp must be RFC3339 UTC: {error}"))
    })?;

    if timestamp.offset() != UtcOffset::UTC {
        return Err(AuditPersistenceError::invalid_payload(
            "timestamp must use UTC offset `Z`",
        ));
    }

    Ok(())
}

fn classify_query_error(operation: &'static str, error: sqlx::Error) -> AuditPersistenceError {
    if is_constraint_error(&error) {
        return AuditPersistenceError::append_constraint_violation(operation, error);
    }

    AuditPersistenceError::persistence_unavailable(operation, error)
}

fn is_constraint_error(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Database(db_error) => db_error
            .code()
            .map(|code| code.starts_with("23") || code == "55000")
            .unwrap_or(false),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::governance::PrivilegedAuditOutcome;
    use serde_json::json;

    const AUDIT_MIGRATION_SQL: &str =
        include_str!("../../migrations/20260405035200_audit_log_append.sql");

    fn sample_record() -> PrivilegedAuditRecord {
        PrivilegedAuditRecord {
            actor_id: "ops-1".to_string(),
            role: "operational_control".to_string(),
            action_type: "execute_control_plane_action".to_string(),
            parameters: json!({
                "endpoint": "/control/rebalance",
                "http_method": "POST",
            }),
            approval_reference: None,
            timestamp: "2026-04-05T00:00:00Z".to_string(),
            outcome: PrivilegedAuditOutcome::Allow,
            reason_code: "authorization_allowed".to_string(),
            authentication_outcome: "authenticated".to_string(),
            correlation_id: "corr-audit-001".to_string(),
        }
    }

    #[test]
    fn migration_creates_expected_audit_append_contract() {
        assert!(AUDIT_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS audit_log_append"));
        assert!(AUDIT_MIGRATION_SQL.contains("parameters JSONB NOT NULL"));
        assert!(AUDIT_MIGRATION_SQL.contains("approval_reference TEXT NULL"));
        assert!(AUDIT_MIGRATION_SQL.contains("timestamp TIMESTAMPTZ NOT NULL"));
        assert!(AUDIT_MIGRATION_SQL.contains("outcome TEXT NOT NULL"));
        assert!(AUDIT_MIGRATION_SQL.contains("reason_code TEXT NOT NULL"));
        assert!(AUDIT_MIGRATION_SQL.contains("correlation_id TEXT NOT NULL"));
        assert!(AUDIT_MIGRATION_SQL.contains("authentication_outcome TEXT NOT NULL"));
    }

    #[test]
    fn migration_enforces_append_only_triggers_and_indexes() {
        assert!(
            AUDIT_MIGRATION_SQL
                .contains("CREATE INDEX IF NOT EXISTS idx_audit_log_append_timestamp")
        );
        assert!(
            AUDIT_MIGRATION_SQL
                .contains("CREATE INDEX IF NOT EXISTS idx_audit_log_append_actor_action")
        );
        assert!(
            AUDIT_MIGRATION_SQL
                .contains("CREATE INDEX IF NOT EXISTS idx_audit_log_append_correlation")
        );
        assert!(
            AUDIT_MIGRATION_SQL
                .contains("CREATE OR REPLACE FUNCTION prevent_audit_log_append_mutation")
        );
        assert!(AUDIT_MIGRATION_SQL.contains("CREATE TRIGGER trg_audit_log_append_no_update"));
        assert!(AUDIT_MIGRATION_SQL.contains("CREATE TRIGGER trg_audit_log_append_no_delete"));
    }

    #[test]
    fn validate_record_accepts_nullable_approval_reference() {
        let record = sample_record();
        assert!(validate_record(&record).is_ok());
    }

    #[test]
    fn validate_record_rejects_non_object_parameters() {
        let mut record = sample_record();
        record.parameters = json!(["not", "an", "object"]);

        let error = validate_record(&record).expect_err("non-object parameters should fail");
        assert_eq!(error.code, "audit_invalid_payload");
    }

    #[test]
    fn validate_record_rejects_invalid_timestamp() {
        let mut record = sample_record();
        record.timestamp = "not-a-timestamp".to_string();

        let error = validate_record(&record).expect_err("invalid timestamp should fail");
        assert_eq!(error.code, "audit_invalid_payload");
    }

    #[test]
    fn validate_record_rejects_non_utc_timestamp() {
        let mut record = sample_record();
        record.timestamp = "2026-04-05T01:00:00+01:00".to_string();

        let error = validate_record(&record).expect_err("non-utc timestamp should fail");
        assert_eq!(error.code, "audit_invalid_payload");
    }
}
