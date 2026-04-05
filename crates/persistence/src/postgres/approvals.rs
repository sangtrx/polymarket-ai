use domain::governance::{
    ApprovalRequest, ApprovalState, ApprovalVoteDecision, ApprovalVoteRecord, CriticalActionId,
};
use sqlx::{PgExecutor, Row};
use std::error::Error;
use std::fmt::{Display, Formatter};
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalPersistenceError {
    pub code: &'static str,
    pub message: String,
}

impl ApprovalPersistenceError {
    fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: "approval_invalid_payload",
            message: message.into(),
        }
    }

    fn query_failure(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "approval_query_failed",
            message: format!("{operation} failed: {error}"),
        }
    }

    fn constraint_violation(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "approval_constraint_violation",
            message: format!("{operation} rejected by constraint: {error}"),
        }
    }

    fn duplicate_request(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "approval_duplicate_request",
            message: format!("{operation} rejected: duplicate approval request ({error})"),
        }
    }

    fn duplicate_vote(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "approval_duplicate_vote",
            message: format!("{operation} rejected: duplicate actor vote ({error})"),
        }
    }

    fn row_decode_failure(column: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "approval_row_decode_failed",
            message: format!("unable to decode `{column}`: {error}"),
        }
    }

    fn unexpected_row_count(operation: &'static str, rows_affected: u64) -> Self {
        Self {
            code: "approval_constraint_violation",
            message: format!(
                "{operation} rejected by constraint: expected 1 row, got {rows_affected}"
            ),
        }
    }

    fn request_not_found(request_id: &str) -> Self {
        Self {
            code: "approval_request_not_found",
            message: format!("approval request `{request_id}` was not found"),
        }
    }
}

impl Display for ApprovalPersistenceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for ApprovalPersistenceError {}

pub async fn create_approval_request<'e, E>(
    executor: E,
    request: &ApprovalRequest,
) -> Result<(), ApprovalPersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_request(request)?;

    let result = sqlx::query(
        r#"
        INSERT INTO approval_requests (
            request_id,
            action_id,
            proposer_actor_id,
            status,
            reason_code,
            correlation_id,
            approval_reference,
            created_at_utc,
            expires_at_utc,
            updated_at_utc
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8::timestamptz, $9::timestamptz, $10::timestamptz)
        "#,
    )
    .bind(&request.request_id)
    .bind(&request.action_id)
    .bind(&request.proposer_actor_id)
    .bind(request.status.as_str())
    .bind(&request.reason_code)
    .bind(&request.correlation_id)
    .bind(request.approval_reference.as_deref())
    .bind(&request.created_at_utc)
    .bind(&request.expires_at_utc)
    .bind(&request.created_at_utc)
    .execute(executor)
    .await
    .map_err(|error| classify_query_error("create_approval_request", error))?;

    if result.rows_affected() != 1 {
        return Err(ApprovalPersistenceError::unexpected_row_count(
            "create_approval_request",
            result.rows_affected(),
        ));
    }
    Ok(())
}

pub async fn record_approval_vote<'e, E>(
    executor: E,
    vote: &ApprovalVoteRecord,
) -> Result<(), ApprovalPersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_vote(vote)?;

    let result = sqlx::query(
        r#"
        INSERT INTO approval_votes (
            request_id,
            actor_id,
            decision,
            correlation_id,
            voted_at_utc
        ) VALUES ($1, $2, $3, $4, $5::timestamptz)
        "#,
    )
    .bind(&vote.request_id)
    .bind(&vote.actor_id)
    .bind(vote.decision.as_str())
    .bind(&vote.correlation_id)
    .bind(&vote.voted_at_utc)
    .execute(executor)
    .await
    .map_err(|error| classify_query_error("record_approval_vote", error))?;

    if result.rows_affected() != 1 {
        return Err(ApprovalPersistenceError::unexpected_row_count(
            "record_approval_vote",
            result.rows_affected(),
        ));
    }
    Ok(())
}

pub async fn load_approval_request<'e, E>(
    executor: E,
    request_id: &str,
) -> Result<Option<ApprovalRequest>, ApprovalPersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_non_empty("request_id", request_id)?;

    let row = sqlx::query(
        r#"
        SELECT
            request_id,
            action_id,
            proposer_actor_id,
            status,
            reason_code,
            correlation_id,
            approval_reference,
            to_char(created_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS created_at_utc,
            to_char(expires_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS expires_at_utc
        FROM approval_requests
        WHERE request_id = $1
        "#,
    )
    .bind(request_id)
    .fetch_optional(executor)
    .await
    .map_err(|error| classify_query_error("load_approval_request", error))?;

    let Some(row) = row else {
        return Ok(None);
    };

    let action_id: String = row
        .try_get("action_id")
        .map_err(|error| ApprovalPersistenceError::row_decode_failure("action_id", error))?;
    CriticalActionId::parse(&action_id)
        .map_err(|error| ApprovalPersistenceError::invalid_payload(error.message))?;

    let status_raw: String = row
        .try_get("status")
        .map_err(|error| ApprovalPersistenceError::row_decode_failure("status", error))?;
    let status = ApprovalState::parse(&status_raw)
        .map_err(|error| ApprovalPersistenceError::invalid_payload(error.message))?;

    Ok(Some(ApprovalRequest {
        request_id: row
            .try_get("request_id")
            .map_err(|error| ApprovalPersistenceError::row_decode_failure("request_id", error))?,
        action_id,
        proposer_actor_id: row.try_get("proposer_actor_id").map_err(|error| {
            ApprovalPersistenceError::row_decode_failure("proposer_actor_id", error)
        })?,
        status,
        reason_code: row
            .try_get("reason_code")
            .map_err(|error| ApprovalPersistenceError::row_decode_failure("reason_code", error))?,
        correlation_id: row.try_get("correlation_id").map_err(|error| {
            ApprovalPersistenceError::row_decode_failure("correlation_id", error)
        })?,
        approval_reference: row.try_get("approval_reference").map_err(|error| {
            ApprovalPersistenceError::row_decode_failure("approval_reference", error)
        })?,
        created_at_utc: row.try_get("created_at_utc").map_err(|error| {
            ApprovalPersistenceError::row_decode_failure("created_at_utc", error)
        })?,
        expires_at_utc: row.try_get("expires_at_utc").map_err(|error| {
            ApprovalPersistenceError::row_decode_failure("expires_at_utc", error)
        })?,
    }))
}

pub async fn load_approval_votes<'e, E>(
    executor: E,
    request_id: &str,
) -> Result<Vec<ApprovalVoteRecord>, ApprovalPersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_non_empty("request_id", request_id)?;

    let rows = sqlx::query(
        r#"
        SELECT
            request_id,
            actor_id,
            decision,
            correlation_id,
            to_char(voted_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS voted_at_utc
        FROM approval_votes
        WHERE request_id = $1
        ORDER BY voted_at_utc ASC
        "#,
    )
    .bind(request_id)
    .fetch_all(executor)
    .await
    .map_err(|error| classify_query_error("load_approval_votes", error))?;

    rows.into_iter()
        .map(|row| {
            let decision_raw: String = row
                .try_get("decision")
                .map_err(|error| ApprovalPersistenceError::row_decode_failure("decision", error))?;
            let decision = ApprovalVoteDecision::parse(&decision_raw)
                .map_err(|error| ApprovalPersistenceError::invalid_payload(error.message))?;
            Ok(ApprovalVoteRecord {
                request_id: row.try_get("request_id").map_err(|error| {
                    ApprovalPersistenceError::row_decode_failure("request_id", error)
                })?,
                actor_id: row.try_get("actor_id").map_err(|error| {
                    ApprovalPersistenceError::row_decode_failure("actor_id", error)
                })?,
                decision,
                correlation_id: row.try_get("correlation_id").map_err(|error| {
                    ApprovalPersistenceError::row_decode_failure("correlation_id", error)
                })?,
                voted_at_utc: row.try_get("voted_at_utc").map_err(|error| {
                    ApprovalPersistenceError::row_decode_failure("voted_at_utc", error)
                })?,
            })
        })
        .collect()
}

pub async fn count_actor_requests_since<'e, E>(
    executor: E,
    actor_id: &str,
    since_utc: &str,
) -> Result<u32, ApprovalPersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_non_empty("actor_id", actor_id)?;
    parse_utc_timestamp("since_utc", since_utc)?;

    let row = sqlx::query(
        r#"
        SELECT COUNT(*) AS request_count
        FROM approval_requests
        WHERE lower(trim(proposer_actor_id)) = lower(trim($1))
          AND created_at_utc >= $2::timestamptz
        "#,
    )
    .bind(actor_id)
    .bind(since_utc)
    .fetch_one(executor)
    .await
    .map_err(|error| classify_query_error("count_actor_requests_since", error))?;

    let count: i64 = row
        .try_get("request_count")
        .map_err(|error| ApprovalPersistenceError::row_decode_failure("request_count", error))?;
    u32::try_from(count)
        .map_err(|_| ApprovalPersistenceError::invalid_payload("request_count exceeded u32 range"))
}

pub async fn update_approval_request<'e, E>(
    executor: E,
    request_id: &str,
    status: ApprovalState,
    reason_code: &str,
    approval_reference: Option<&str>,
) -> Result<(), ApprovalPersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_non_empty("request_id", request_id)?;
    validate_non_empty("reason_code", reason_code)?;
    if let Some(reference) = approval_reference {
        validate_non_empty("approval_reference", reference)?;
    }

    let result = sqlx::query(
        r#"
        UPDATE approval_requests
        SET status = $2,
            reason_code = $3,
            approval_reference = $4,
            updated_at_utc = NOW()
        WHERE request_id = $1
        "#,
    )
    .bind(request_id)
    .bind(status.as_str())
    .bind(reason_code)
    .bind(approval_reference)
    .execute(executor)
    .await
    .map_err(|error| classify_query_error("update_approval_request", error))?;

    if result.rows_affected() != 1 {
        return Err(ApprovalPersistenceError::request_not_found(request_id));
    }
    Ok(())
}

fn validate_request(request: &ApprovalRequest) -> Result<(), ApprovalPersistenceError> {
    validate_non_empty("request_id", &request.request_id)?;
    validate_non_empty("action_id", &request.action_id)?;
    validate_non_empty("proposer_actor_id", &request.proposer_actor_id)?;
    validate_non_empty("reason_code", &request.reason_code)?;
    validate_non_empty("correlation_id", &request.correlation_id)?;
    if let Some(reference) = &request.approval_reference {
        validate_non_empty("approval_reference", reference)?;
    }

    CriticalActionId::parse(&request.action_id)
        .map_err(|error| ApprovalPersistenceError::invalid_payload(error.message))?;

    let created = parse_utc_timestamp("created_at_utc", &request.created_at_utc)?;
    let expires = parse_utc_timestamp("expires_at_utc", &request.expires_at_utc)?;
    if expires <= created {
        return Err(ApprovalPersistenceError::invalid_payload(
            "expires_at_utc must be greater than created_at_utc",
        ));
    }

    Ok(())
}

fn validate_vote(vote: &ApprovalVoteRecord) -> Result<(), ApprovalPersistenceError> {
    validate_non_empty("request_id", &vote.request_id)?;
    validate_non_empty("actor_id", &vote.actor_id)?;
    validate_non_empty("correlation_id", &vote.correlation_id)?;
    parse_utc_timestamp("voted_at_utc", &vote.voted_at_utc)?;
    Ok(())
}

fn validate_non_empty(field: &'static str, value: &str) -> Result<(), ApprovalPersistenceError> {
    if value.trim().is_empty() {
        return Err(ApprovalPersistenceError::invalid_payload(format!(
            "{field} cannot be blank"
        )));
    }
    Ok(())
}

fn parse_utc_timestamp(
    field: &'static str,
    value: &str,
) -> Result<OffsetDateTime, ApprovalPersistenceError> {
    let parsed = OffsetDateTime::parse(value, &Rfc3339).map_err(|_| {
        ApprovalPersistenceError::invalid_payload(format!(
            "`{field}` must be RFC3339 UTC timestamp; got `{value}`"
        ))
    })?;
    if parsed.offset() != UtcOffset::UTC {
        return Err(ApprovalPersistenceError::invalid_payload(format!(
            "`{field}` must use UTC `Z` offset"
        )));
    }
    Ok(parsed)
}

fn classify_query_error(operation: &'static str, error: sqlx::Error) -> ApprovalPersistenceError {
    match duplicate_constraint_kind(&error) {
        Some(DuplicateConstraintKind::Request) => {
            return ApprovalPersistenceError::duplicate_request(operation, error);
        }
        Some(DuplicateConstraintKind::Vote) => {
            return ApprovalPersistenceError::duplicate_vote(operation, error);
        }
        None => {}
    }

    if is_constraint_error(&error) {
        return ApprovalPersistenceError::constraint_violation(operation, error);
    }
    ApprovalPersistenceError::query_failure(operation, error)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DuplicateConstraintKind {
    Request,
    Vote,
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
    if constraint == "approval_requests_pkey" || message.contains("approval_requests_pkey") {
        return Some(DuplicateConstraintKind::Request);
    }
    if constraint == "approval_votes_pkey"
        || constraint == "idx_approval_votes_request_actor_canonical_unique"
        || message.contains("approval_votes_pkey")
        || message.contains("idx_approval_votes_request_actor_canonical_unique")
    {
        return Some(DuplicateConstraintKind::Vote);
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

#[cfg(test)]
mod tests {
    use super::*;

    const APPROVAL_MIGRATION_SQL: &str =
        include_str!("../../migrations/20260405074300_approval_requests_votes.sql");

    fn sample_request() -> ApprovalRequest {
        ApprovalRequest {
            request_id: "req-1".to_string(),
            action_id: "risk_limit_increase".to_string(),
            proposer_actor_id: "ops-1".to_string(),
            status: ApprovalState::Pending,
            reason_code: "approval_pending".to_string(),
            correlation_id: "corr-req-1".to_string(),
            approval_reference: None,
            created_at_utc: "2026-04-05T00:00:00Z".to_string(),
            expires_at_utc: "2026-04-05T01:00:00Z".to_string(),
        }
    }

    fn sample_vote() -> ApprovalVoteRecord {
        ApprovalVoteRecord {
            request_id: "req-1".to_string(),
            actor_id: "admin-1".to_string(),
            decision: ApprovalVoteDecision::Approve,
            correlation_id: "corr-vote-1".to_string(),
            voted_at_utc: "2026-04-05T00:10:00Z".to_string(),
        }
    }

    #[test]
    fn migration_creates_approval_tables_with_expected_schema_scope() {
        assert!(APPROVAL_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS approval_requests"));
        assert!(APPROVAL_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS approval_votes"));
        assert!(APPROVAL_MIGRATION_SQL.contains("expires_at_utc TIMESTAMPTZ NOT NULL"));
        assert!(
            APPROVAL_MIGRATION_SQL
                .contains("status IN ('pending', 'approved', 'rejected', 'expired')")
        );
        assert!(
            APPROVAL_MIGRATION_SQL.contains("status = 'approved'")
                && APPROVAL_MIGRATION_SQL.contains("approval_reference IS NOT NULL")
        );
    }

    #[test]
    fn migration_enforces_vote_uniqueness_and_rate_limit_indexes() {
        assert!(APPROVAL_MIGRATION_SQL.contains("PRIMARY KEY (request_id, actor_id)"));
        assert!(APPROVAL_MIGRATION_SQL.contains("idx_approval_requests_actor_created_at"));
        assert!(APPROVAL_MIGRATION_SQL.contains("idx_approval_requests_action_status_created_at"));
        assert!(APPROVAL_MIGRATION_SQL.contains("idx_approval_votes_request_voted_at"));
        assert!(
            APPROVAL_MIGRATION_SQL.contains("idx_approval_votes_request_actor_canonical_unique")
        );
    }

    #[test]
    fn validate_request_accepts_valid_contract() {
        assert!(validate_request(&sample_request()).is_ok());
    }

    #[test]
    fn validate_request_rejects_unknown_action_identifier() {
        let mut request = sample_request();
        request.action_id = "execute_control_plane_action".to_string();
        let error = validate_request(&request).expect_err("unknown action must fail");
        assert_eq!(error.code, "approval_invalid_payload");
    }

    #[test]
    fn validate_request_rejects_non_utc_timestamp() {
        let mut request = sample_request();
        request.expires_at_utc = "2026-04-05T01:00:00+01:00".to_string();
        let error = validate_request(&request).expect_err("non-utc timestamp must fail");
        assert_eq!(error.code, "approval_invalid_payload");
    }

    #[test]
    fn validate_vote_rejects_blank_actor_id() {
        let mut vote = sample_vote();
        vote.actor_id = " ".to_string();
        let error = validate_vote(&vote).expect_err("blank actor id must fail");
        assert_eq!(error.code, "approval_invalid_payload");
    }
}
