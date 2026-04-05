use domain::governance::{
    CredentialRotationContractError, CredentialRotationDecisionOutcome, CredentialRotationEvidence,
    CredentialRotationReasonCode, CredentialRotationState, CredentialRotationTrigger,
};
use serde_json::Value;
use sqlx::{PgExecutor, Row};
use std::error::Error;
use std::fmt::{Display, Formatter};
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

const CREATE_ROTATION_EVENT_SQL: &str = r#"
    INSERT INTO credential_rotation_events (
        rotation_id,
        trigger_type,
        status,
        reason_code,
        actor_id,
        credential_scope,
        credential_reference,
        correlation_id,
        rotation_reference,
        initiated_at_utc,
        deadline_at_utc,
        completed_at_utc,
        metadata
    ) VALUES (
        $1, $2, $3, $4, $5, $6, $7, $8, $9, $10::timestamptz, $11::timestamptz, $12::timestamptz, $13
    )
"#;

const LOAD_ROTATION_EVENT_SQL: &str = r#"
    SELECT
        rotation_id,
        trigger_type,
        status,
        reason_code,
        actor_id,
        credential_scope,
        credential_reference,
        correlation_id,
        rotation_reference,
        to_char(initiated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS initiated_at_utc,
        to_char(deadline_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS deadline_at_utc,
        to_char(completed_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS completed_at_utc,
        metadata
    FROM credential_rotation_events
    WHERE rotation_id = $1
"#;

const QUERY_ROTATION_EVENTS_SQL: &str = r#"
    SELECT
        rotation_id,
        trigger_type,
        status,
        reason_code,
        actor_id,
        credential_scope,
        credential_reference,
        correlation_id,
        rotation_reference,
        to_char(initiated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS initiated_at_utc,
        to_char(deadline_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS deadline_at_utc,
        to_char(completed_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS completed_at_utc,
        metadata
    FROM credential_rotation_events
    WHERE ($1::text IS NULL OR trigger_type = $1)
      AND ($2::text IS NULL OR status = $2)
    ORDER BY initiated_at_utc DESC, rotation_id ASC
    LIMIT $3
"#;

const LOAD_ROTATION_TRANSITION_GUARD_SQL: &str = r#"
    SELECT status, rotation_reference
    FROM credential_rotation_events
    WHERE rotation_id = $1
"#;

const UPDATE_ROTATION_EVENT_SQL: &str = r#"
    UPDATE credential_rotation_events
    SET
        status = $2,
        reason_code = $3,
        rotation_reference = $4,
        deadline_at_utc = $5::timestamptz,
        completed_at_utc = $6::timestamptz,
        metadata = $7,
        updated_at_utc = NOW()
    WHERE rotation_id = $1
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialRotationPersistenceError {
    pub code: &'static str,
    pub message: String,
}

impl CredentialRotationPersistenceError {
    fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: CredentialRotationReasonCode::InvalidPayload.code(),
            message: message.into(),
        }
    }

    fn invalid_transition(message: impl Into<String>) -> Self {
        Self {
            code: CredentialRotationReasonCode::InvalidStateTransition.code(),
            message: message.into(),
        }
    }

    fn query_failure(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "credential_rotation_query_failed",
            message: format!("{operation} failed: {error}"),
        }
    }

    fn constraint_violation(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "credential_rotation_constraint_violation",
            message: format!("{operation} rejected by constraint: {error}"),
        }
    }

    fn duplicate_rotation(rotation_id: &str) -> Self {
        Self {
            code: "credential_rotation_duplicate_rotation_id",
            message: format!("rotation event `{rotation_id}` already exists"),
        }
    }

    fn duplicate_reference(reference: &str) -> Self {
        Self {
            code: "credential_rotation_duplicate_reference",
            message: format!(
                "rotation reference `{reference}` is already associated to another event"
            ),
        }
    }

    fn row_decode_failure(column: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "credential_rotation_row_decode_failed",
            message: format!("unable to decode `{column}`: {error}"),
        }
    }

    fn not_found(rotation_id: &str) -> Self {
        Self {
            code: "credential_rotation_not_found",
            message: format!("rotation event `{rotation_id}` was not found"),
        }
    }
}

impl Display for CredentialRotationPersistenceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for CredentialRotationPersistenceError {}

pub async fn create_credential_rotation_event<'e, E>(
    executor: E,
    event: &CredentialRotationEvidence,
) -> Result<(), CredentialRotationPersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_rotation_event_for_create(event)?;

    let result = sqlx::query(CREATE_ROTATION_EVENT_SQL)
        .bind(&event.rotation_id)
        .bind(event.trigger_type.as_str())
        .bind(event.status.as_str())
        .bind(&event.reason_code)
        .bind(&event.actor_id)
        .bind(&event.credential_scope)
        .bind(&event.credential_reference)
        .bind(&event.correlation_id)
        .bind(event.rotation_reference.as_deref())
        .bind(&event.initiated_at_utc)
        .bind(event.deadline_at_utc.as_deref())
        .bind(event.completed_at_utc.as_deref())
        .bind(&event.metadata)
        .execute(executor)
        .await
        .map_err(|error| classify_query_error("create_credential_rotation_event", error, event))?;

    if result.rows_affected() != 1 {
        return Err(CredentialRotationPersistenceError::invalid_payload(
            format!(
                "create_credential_rotation_event expected 1 affected row, got {}",
                result.rows_affected()
            ),
        ));
    }

    Ok(())
}

pub async fn load_credential_rotation_event<'e, E>(
    executor: E,
    rotation_id: &str,
) -> Result<Option<CredentialRotationEvidence>, CredentialRotationPersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_non_empty("rotation_id", rotation_id)?;
    let row = sqlx::query(LOAD_ROTATION_EVENT_SQL)
        .bind(rotation_id)
        .fetch_optional(executor)
        .await
        .map_err(|error| {
            CredentialRotationPersistenceError::query_failure(
                "load_credential_rotation_event",
                error,
            )
        })?;

    row.map(decode_rotation_event_row).transpose()
}

pub async fn update_credential_rotation_event<'e, E>(
    executor: E,
    event: &CredentialRotationEvidence,
) -> Result<(), CredentialRotationPersistenceError>
where
    E: PgExecutor<'e> + Copy,
{
    validate_rotation_event_for_update(event)?;

    let current = sqlx::query(LOAD_ROTATION_TRANSITION_GUARD_SQL)
        .bind(&event.rotation_id)
        .fetch_optional(executor)
        .await
        .map_err(|error| {
            CredentialRotationPersistenceError::query_failure(
                "update_credential_rotation_event",
                error,
            )
        })?;

    let Some(current) = current else {
        return Err(CredentialRotationPersistenceError::not_found(
            &event.rotation_id,
        ));
    };

    let current_status: String = current
        .try_get("status")
        .map_err(|error| CredentialRotationPersistenceError::row_decode_failure("status", error))?;
    let current_status = CredentialRotationState::parse(&current_status)
        .map_err(|error| CredentialRotationPersistenceError::invalid_payload(error.message))?;
    if !is_valid_rotation_transition(current_status, event.status) {
        return Err(CredentialRotationPersistenceError::invalid_transition(
            format!(
                "invalid credential-rotation transition from `{}` to `{}`",
                current_status.as_str(),
                event.status.as_str()
            ),
        ));
    }

    let current_reference: Option<String> =
        current.try_get("rotation_reference").map_err(|error| {
            CredentialRotationPersistenceError::row_decode_failure("rotation_reference", error)
        })?;
    if let (Some(existing), Some(next)) = (
        current_reference.as_deref(),
        event.rotation_reference.as_deref(),
    ) && existing.trim() != next.trim()
    {
        return Err(CredentialRotationPersistenceError::invalid_transition(
            "rotation_reference cannot change once assigned",
        ));
    }

    let result = sqlx::query(UPDATE_ROTATION_EVENT_SQL)
        .bind(&event.rotation_id)
        .bind(event.status.as_str())
        .bind(&event.reason_code)
        .bind(event.rotation_reference.as_deref())
        .bind(event.deadline_at_utc.as_deref())
        .bind(event.completed_at_utc.as_deref())
        .bind(&event.metadata)
        .execute(executor)
        .await
        .map_err(|error| classify_query_error("update_credential_rotation_event", error, event))?;

    if result.rows_affected() != 1 {
        return Err(CredentialRotationPersistenceError::not_found(
            &event.rotation_id,
        ));
    }
    Ok(())
}

pub async fn query_credential_rotation_events<'e, E>(
    executor: E,
    trigger_type: Option<CredentialRotationTrigger>,
    status: Option<CredentialRotationState>,
    limit: u32,
) -> Result<Vec<CredentialRotationEvidence>, CredentialRotationPersistenceError>
where
    E: PgExecutor<'e>,
{
    if limit == 0 || limit > 1_000 {
        return Err(CredentialRotationPersistenceError::invalid_payload(
            "`limit` must be between 1 and 1000",
        ));
    }

    let rows = sqlx::query(QUERY_ROTATION_EVENTS_SQL)
        .bind(trigger_type.map(CredentialRotationTrigger::as_str))
        .bind(status.map(CredentialRotationState::as_str))
        .bind(i64::from(limit))
        .fetch_all(executor)
        .await
        .map_err(|error| {
            CredentialRotationPersistenceError::query_failure(
                "query_credential_rotation_events",
                error,
            )
        })?;

    rows.into_iter().map(decode_rotation_event_row).collect()
}

fn decode_rotation_event_row(
    row: sqlx::postgres::PgRow,
) -> Result<CredentialRotationEvidence, CredentialRotationPersistenceError> {
    let trigger_type: String = row.try_get("trigger_type").map_err(|error| {
        CredentialRotationPersistenceError::row_decode_failure("trigger_type", error)
    })?;
    let trigger_type = CredentialRotationTrigger::parse(&trigger_type)
        .map_err(|error| CredentialRotationPersistenceError::invalid_payload(error.message))?;

    let status: String = row
        .try_get("status")
        .map_err(|error| CredentialRotationPersistenceError::row_decode_failure("status", error))?;
    let status = CredentialRotationState::parse(&status)
        .map_err(|error| CredentialRotationPersistenceError::invalid_payload(error.message))?;

    let reason_code: String = row.try_get("reason_code").map_err(|error| {
        CredentialRotationPersistenceError::row_decode_failure("reason_code", error)
    })?;
    CredentialRotationReasonCode::parse(&reason_code)
        .map_err(|error| CredentialRotationPersistenceError::invalid_payload(error.message))?;

    let initiated_at_utc: String = row.try_get("initiated_at_utc").map_err(|error| {
        CredentialRotationPersistenceError::row_decode_failure("initiated_at_utc", error)
    })?;
    parse_utc_timestamp("initiated_at_utc", &initiated_at_utc)?;

    let deadline_at_utc: Option<String> = row.try_get("deadline_at_utc").map_err(|error| {
        CredentialRotationPersistenceError::row_decode_failure("deadline_at_utc", error)
    })?;
    if let Some(deadline) = &deadline_at_utc {
        parse_utc_timestamp("deadline_at_utc", deadline)?;
    }

    let completed_at_utc: Option<String> = row.try_get("completed_at_utc").map_err(|error| {
        CredentialRotationPersistenceError::row_decode_failure("completed_at_utc", error)
    })?;
    if let Some(completed) = &completed_at_utc {
        parse_utc_timestamp("completed_at_utc", completed)?;
    }

    let metadata: Value = row.try_get("metadata").map_err(|error| {
        CredentialRotationPersistenceError::row_decode_failure("metadata", error)
    })?;

    let evidence = CredentialRotationEvidence {
        rotation_id: row.try_get("rotation_id").map_err(|error| {
            CredentialRotationPersistenceError::row_decode_failure("rotation_id", error)
        })?,
        actor_id: row.try_get("actor_id").map_err(|error| {
            CredentialRotationPersistenceError::row_decode_failure("actor_id", error)
        })?,
        trigger_type,
        credential_scope: row.try_get("credential_scope").map_err(|error| {
            CredentialRotationPersistenceError::row_decode_failure("credential_scope", error)
        })?,
        credential_reference: row.try_get("credential_reference").map_err(|error| {
            CredentialRotationPersistenceError::row_decode_failure("credential_reference", error)
        })?,
        outcome: outcome_for_state(status),
        status,
        reason_code,
        correlation_id: row.try_get("correlation_id").map_err(|error| {
            CredentialRotationPersistenceError::row_decode_failure("correlation_id", error)
        })?,
        initiated_at_utc,
        deadline_at_utc,
        completed_at_utc,
        rotation_reference: row.try_get("rotation_reference").map_err(|error| {
            CredentialRotationPersistenceError::row_decode_failure("rotation_reference", error)
        })?,
        metadata,
    };

    evidence
        .validate_contract()
        .map_err(map_domain_contract_error)?;
    Ok(evidence)
}

fn validate_rotation_event_for_create(
    event: &CredentialRotationEvidence,
) -> Result<(), CredentialRotationPersistenceError> {
    event
        .validate_contract()
        .map_err(map_domain_contract_error)?;
    validate_state_shape(event)
}

fn validate_rotation_event_for_update(
    event: &CredentialRotationEvidence,
) -> Result<(), CredentialRotationPersistenceError> {
    event
        .validate_contract()
        .map_err(map_domain_contract_error)?;
    validate_state_shape(event)
}

fn validate_state_shape(
    event: &CredentialRotationEvidence,
) -> Result<(), CredentialRotationPersistenceError> {
    match event.status {
        CredentialRotationState::Succeeded => {
            if event.rotation_reference.is_none() {
                return Err(CredentialRotationPersistenceError::invalid_payload(
                    "rotation_reference is required when status is `succeeded`",
                ));
            }
            if event.completed_at_utc.is_none() {
                return Err(CredentialRotationPersistenceError::invalid_payload(
                    "completed_at_utc is required when status is `succeeded`",
                ));
            }
        }
        CredentialRotationState::Denied | CredentialRotationState::Failed => {
            if event.completed_at_utc.is_none() {
                return Err(CredentialRotationPersistenceError::invalid_payload(
                    "completed_at_utc is required for denied/failed terminal states",
                ));
            }
        }
        CredentialRotationState::Pending | CredentialRotationState::InProgress => {
            if event.completed_at_utc.is_some() {
                return Err(CredentialRotationPersistenceError::invalid_payload(
                    "completed_at_utc must be absent for non-terminal states",
                ));
            }
        }
    }
    Ok(())
}

fn map_domain_contract_error(
    error: CredentialRotationContractError,
) -> CredentialRotationPersistenceError {
    CredentialRotationPersistenceError {
        code: error.code,
        message: error.message,
    }
}

fn validate_non_empty(
    field: &'static str,
    value: &str,
) -> Result<(), CredentialRotationPersistenceError> {
    if value.trim().is_empty() {
        return Err(CredentialRotationPersistenceError::invalid_payload(
            format!("{field} cannot be blank"),
        ));
    }
    Ok(())
}

fn parse_utc_timestamp(
    field: &'static str,
    value: &str,
) -> Result<OffsetDateTime, CredentialRotationPersistenceError> {
    let parsed = OffsetDateTime::parse(value, &Rfc3339).map_err(|_| {
        CredentialRotationPersistenceError::invalid_payload(format!(
            "`{field}` must be RFC3339 UTC timestamp; got `{value}`"
        ))
    })?;
    if parsed.offset() != UtcOffset::UTC {
        return Err(CredentialRotationPersistenceError::invalid_payload(
            format!("`{field}` must use UTC `Z` offset"),
        ));
    }
    Ok(parsed)
}

fn classify_query_error(
    operation: &'static str,
    error: sqlx::Error,
    event: &CredentialRotationEvidence,
) -> CredentialRotationPersistenceError {
    if let Some(kind) = duplicate_constraint_kind(&error) {
        return match kind {
            DuplicateConstraintKind::RotationId => {
                CredentialRotationPersistenceError::duplicate_rotation(&event.rotation_id)
            }
            DuplicateConstraintKind::Reference => {
                CredentialRotationPersistenceError::duplicate_reference(
                    event.rotation_reference.as_deref().unwrap_or("<missing>"),
                )
            }
        };
    }
    if is_constraint_error(&error) {
        return CredentialRotationPersistenceError::constraint_violation(operation, error);
    }
    CredentialRotationPersistenceError::query_failure(operation, error)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DuplicateConstraintKind {
    RotationId,
    Reference,
}

fn duplicate_constraint_kind(error: &sqlx::Error) -> Option<DuplicateConstraintKind> {
    let sqlx::Error::Database(database_error) = error else {
        return None;
    };
    if database_error.code().as_deref() != Some("23505") {
        return None;
    }
    let constraint = database_error.constraint().unwrap_or_default();
    let message = database_error.message();
    if constraint == "credential_rotation_events_pkey"
        || message.contains("credential_rotation_events_pkey")
    {
        return Some(DuplicateConstraintKind::RotationId);
    }
    if constraint == "idx_credential_rotation_events_reference_unique"
        || message.contains("idx_credential_rotation_events_reference_unique")
    {
        return Some(DuplicateConstraintKind::Reference);
    }
    None
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

fn is_valid_rotation_transition(
    current: CredentialRotationState,
    next: CredentialRotationState,
) -> bool {
    match current {
        CredentialRotationState::Pending => matches!(
            next,
            CredentialRotationState::Pending
                | CredentialRotationState::InProgress
                | CredentialRotationState::Succeeded
                | CredentialRotationState::Denied
                | CredentialRotationState::Failed
        ),
        CredentialRotationState::InProgress => matches!(
            next,
            CredentialRotationState::InProgress
                | CredentialRotationState::Succeeded
                | CredentialRotationState::Denied
                | CredentialRotationState::Failed
        ),
        CredentialRotationState::Succeeded => next == CredentialRotationState::Succeeded,
        CredentialRotationState::Denied => next == CredentialRotationState::Denied,
        CredentialRotationState::Failed => next == CredentialRotationState::Failed,
    }
}

fn outcome_for_state(status: CredentialRotationState) -> CredentialRotationDecisionOutcome {
    match status {
        CredentialRotationState::Succeeded => CredentialRotationDecisionOutcome::Allow,
        CredentialRotationState::Pending | CredentialRotationState::InProgress => {
            CredentialRotationDecisionOutcome::Pending
        }
        CredentialRotationState::Denied | CredentialRotationState::Failed => {
            CredentialRotationDecisionOutcome::Deny
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::governance::CredentialRotationTrigger;
    use serde_json::json;

    const ROTATION_MIGRATION_SQL: &str =
        include_str!("../../migrations/20260405103000_credential_rotation_events.sql");

    fn sample_event() -> CredentialRotationEvidence {
        CredentialRotationEvidence {
            rotation_id: "rot-1".to_string(),
            actor_id: "ops-1".to_string(),
            trigger_type: CredentialRotationTrigger::ScheduledCadence,
            credential_scope: "control_api".to_string(),
            credential_reference: "vault://control-api/prod".to_string(),
            outcome: CredentialRotationDecisionOutcome::Pending,
            status: CredentialRotationState::Pending,
            reason_code: CredentialRotationReasonCode::RotationPending
                .code()
                .to_string(),
            correlation_id: "corr-rot-1".to_string(),
            initiated_at_utc: "2026-04-05T00:00:00Z".to_string(),
            deadline_at_utc: None,
            completed_at_utc: None,
            rotation_reference: None,
            metadata: json!({
                "crypto_posture_verified": true,
                "runtime_injection_mode": "runtime_only",
                "provider_ref": "vault://control-api/prod"
            }),
        }
    }

    #[test]
    fn migration_creates_expected_credential_rotation_schema_scope() {
        assert!(
            ROTATION_MIGRATION_SQL
                .contains("CREATE TABLE IF NOT EXISTS credential_rotation_events")
        );
        assert!(
            ROTATION_MIGRATION_SQL
                .contains("trigger_type IN ('scheduled_cadence', 'emergency_compromise')")
        );
        assert!(
            ROTATION_MIGRATION_SQL
                .contains("status IN ('pending', 'in_progress', 'succeeded', 'denied', 'failed')")
        );
        assert!(ROTATION_MIGRATION_SQL.contains("metadata JSONB NOT NULL"));
    }

    #[test]
    fn migration_exposes_required_audit_and_operations_indexes() {
        assert!(ROTATION_MIGRATION_SQL.contains("idx_credential_rotation_events_trigger_status"));
        assert!(ROTATION_MIGRATION_SQL.contains("idx_credential_rotation_events_initiated_at"));
        assert!(ROTATION_MIGRATION_SQL.contains("idx_credential_rotation_events_actor_id"));
        assert!(ROTATION_MIGRATION_SQL.contains("idx_credential_rotation_events_correlation_id"));
        assert!(ROTATION_MIGRATION_SQL.contains("idx_credential_rotation_events_reference_unique"));
    }

    #[test]
    fn transition_validation_is_deterministic_and_fail_closed() {
        assert!(is_valid_rotation_transition(
            CredentialRotationState::Pending,
            CredentialRotationState::InProgress
        ));
        assert!(is_valid_rotation_transition(
            CredentialRotationState::InProgress,
            CredentialRotationState::Succeeded
        ));
        assert!(!is_valid_rotation_transition(
            CredentialRotationState::Succeeded,
            CredentialRotationState::InProgress
        ));
        assert!(!is_valid_rotation_transition(
            CredentialRotationState::Denied,
            CredentialRotationState::Succeeded
        ));
    }

    #[test]
    fn create_validation_rejects_secret_like_metadata() {
        let mut event = sample_event();
        event.metadata = json!({
            "token_value": "abc123"
        });
        let error = validate_rotation_event_for_create(&event)
            .expect_err("secret-like metadata should fail");
        assert_eq!(
            error.code,
            CredentialRotationReasonCode::SecretMaterialRejected.code()
        );
    }

    #[test]
    fn query_sql_orders_by_timestamp_then_rotation_id() {
        assert!(
            QUERY_ROTATION_EVENTS_SQL.contains("ORDER BY initiated_at_utc DESC, rotation_id ASC")
        );
    }

    #[test]
    fn succeeded_state_requires_reference_and_completion_timestamp() {
        let mut event = sample_event();
        event.status = CredentialRotationState::Succeeded;
        event.reason_code = CredentialRotationReasonCode::RotationAllowed
            .code()
            .to_string();
        let error = validate_state_shape(&event)
            .expect_err("succeeded state without completion fields should fail");
        assert_eq!(
            error.code,
            CredentialRotationReasonCode::InvalidPayload.code()
        );
    }
}
