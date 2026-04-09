use domain::readiness::{WaiverRecord, WaiverState, parse_utc_timestamp, validate_waiver};
use sqlx::{Connection, PgConnection, Row};
use std::error::Error;
use std::fmt::{Display, Formatter};

const INSERT_READINESS_WAIVER_SQL: &str = r#"
    INSERT INTO readiness_waivers (
        waiver_id,
        snapshot_id,
        risk_snapshot_id,
        canonical_requirement_id,
        owner,
        reason_code,
        justification,
        approved_by,
        created_at_utc,
        expires_at_utc
    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9::timestamptz, $10::timestamptz)
"#;

const INSERT_READINESS_WAIVER_REVOCATION_SQL: &str = r#"
    INSERT INTO readiness_waiver_revocations (
        revocation_id,
        waiver_id,
        revoked_by,
        revoked_reason_code,
        revoked_at_utc
    ) VALUES ($1, $2, $3, $4, $5::timestamptz)
"#;

const CHECK_UNRESOLVED_REQUIREMENT_SQL: &str = r#"
    SELECT EXISTS (
        SELECT 1
        FROM risk_rows
        WHERE snapshot_id = $1
          AND canonical_requirement_id = $2
          AND coverage_class IN ('partial', 'missing')
    ) AS is_unresolved
"#;

const LIST_ACTIVE_WAIVERS_SQL: &str = r#"
    SELECT
        w.waiver_id,
        w.canonical_requirement_id,
        w.owner,
        w.reason_code,
        w.justification,
        w.approved_by,
        to_char(w.created_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS created_at_utc,
        to_char(w.expires_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS expires_at_utc,
        CASE
            WHEN wr.revoked_at_utc IS NULL THEN NULL
            ELSE to_char(wr.revoked_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"')
        END AS revoked_at_utc
    FROM readiness_waivers w
    LEFT JOIN readiness_waiver_revocations wr ON wr.waiver_id = w.waiver_id
    WHERE w.snapshot_id = $1
      AND wr.waiver_id IS NULL
      AND w.expires_at_utc > $2::timestamptz
    ORDER BY w.expires_at_utc ASC, w.canonical_requirement_id ASC, w.waiver_id ASC
"#;

const LIST_EXPIRING_WAIVERS_SQL: &str = r#"
    SELECT
        w.waiver_id,
        w.canonical_requirement_id,
        w.owner,
        w.reason_code,
        w.justification,
        w.approved_by,
        to_char(w.created_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS created_at_utc,
        to_char(w.expires_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS expires_at_utc,
        CASE
            WHEN wr.revoked_at_utc IS NULL THEN NULL
            ELSE to_char(wr.revoked_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"')
        END AS revoked_at_utc
    FROM readiness_waivers w
    LEFT JOIN readiness_waiver_revocations wr ON wr.waiver_id = w.waiver_id
    WHERE w.snapshot_id = $1
      AND wr.waiver_id IS NULL
      AND w.expires_at_utc > $2::timestamptz
      AND w.expires_at_utc <= $3::timestamptz
    ORDER BY w.expires_at_utc ASC, w.canonical_requirement_id ASC, w.waiver_id ASC
"#;

const LIST_EXPIRED_WAIVERS_SQL: &str = r#"
    SELECT
        w.waiver_id,
        w.canonical_requirement_id,
        w.owner,
        w.reason_code,
        w.justification,
        w.approved_by,
        to_char(w.created_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS created_at_utc,
        to_char(w.expires_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS expires_at_utc,
        CASE
            WHEN wr.revoked_at_utc IS NULL THEN NULL
            ELSE to_char(wr.revoked_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"')
        END AS revoked_at_utc
    FROM readiness_waivers w
    LEFT JOIN readiness_waiver_revocations wr ON wr.waiver_id = w.waiver_id
    WHERE w.snapshot_id = $1
      AND wr.waiver_id IS NULL
      AND w.expires_at_utc <= $2::timestamptz
    ORDER BY w.expires_at_utc ASC, w.canonical_requirement_id ASC, w.waiver_id ASC
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadinessPersistenceError {
    pub code: &'static str,
    pub message: String,
}

impl ReadinessPersistenceError {
    fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: "readiness_invalid_payload",
            message: message.into(),
        }
    }

    fn query_failure(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "readiness_query_failed",
            message: format!("{operation} failed: {error}"),
        }
    }

    fn constraint_violation(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "readiness_constraint_violation",
            message: format!("{operation} rejected by constraint: {error}"),
        }
    }
}

impl Display for ReadinessPersistenceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for ReadinessPersistenceError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaiverRevocationInput {
    pub revocation_id: String,
    pub waiver_id: String,
    pub revoked_by: String,
    pub revoked_reason_code: String,
    pub revoked_at_utc: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaiverListResult {
    pub state: WaiverState,
    pub waivers: Vec<WaiverRecord>,
}

pub async fn insert_waiver(
    executor: &mut PgConnection,
    snapshot_id: &str,
    risk_snapshot_id: &str,
    waiver: &WaiverRecord,
) -> Result<(), ReadinessPersistenceError> {
    if snapshot_id.trim().is_empty() || risk_snapshot_id.trim().is_empty() {
        return Err(ReadinessPersistenceError::invalid_payload(
            "snapshot_id and risk_snapshot_id are required",
        ));
    }
    validate_waiver(waiver)
        .map_err(|error| ReadinessPersistenceError::invalid_payload(error.message))?;

    let mut transaction = executor
        .begin()
        .await
        .map_err(|error| ReadinessPersistenceError::query_failure("insert_waiver.begin", error))?;

    let unresolved: bool = sqlx::query(CHECK_UNRESOLVED_REQUIREMENT_SQL)
        .bind(risk_snapshot_id.trim().to_ascii_lowercase())
        .bind(waiver.canonical_requirement_id.trim().to_ascii_lowercase())
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| classify_query_error("insert_waiver.check_unresolved", error))?
        .try_get("is_unresolved")
        .map_err(|error| ReadinessPersistenceError::query_failure("insert_waiver.decode", error))?;

    if !unresolved {
        return Err(ReadinessPersistenceError::invalid_payload(
            "canonical_requirement_id must reference unresolved partial|missing risk row",
        ));
    }

    sqlx::query(INSERT_READINESS_WAIVER_SQL)
        .bind(waiver.waiver_id.trim().to_ascii_lowercase())
        .bind(snapshot_id.trim().to_ascii_lowercase())
        .bind(risk_snapshot_id.trim().to_ascii_lowercase())
        .bind(waiver.canonical_requirement_id.trim().to_ascii_lowercase())
        .bind(waiver.owner.trim().to_ascii_lowercase())
        .bind(waiver.reason_code.trim().to_ascii_lowercase())
        .bind(waiver.justification.trim())
        .bind(waiver.approved_by.trim().to_ascii_lowercase())
        .bind(waiver.created_at_utc.trim())
        .bind(waiver.expires_at_utc.trim())
        .execute(&mut *transaction)
        .await
        .map_err(|error| classify_query_error("insert_waiver.insert", error))?;

    transaction
        .commit()
        .await
        .map_err(|error| ReadinessPersistenceError::query_failure("insert_waiver.commit", error))?;

    Ok(())
}

pub async fn revoke_waiver(
    executor: &mut PgConnection,
    input: &WaiverRevocationInput,
) -> Result<(), ReadinessPersistenceError> {
    if input.revocation_id.trim().is_empty()
        || input.waiver_id.trim().is_empty()
        || input.revoked_by.trim().is_empty()
        || input.revoked_reason_code.trim().is_empty()
    {
        return Err(ReadinessPersistenceError::invalid_payload(
            "revocation_id, waiver_id, revoked_by, and revoked_reason_code are required",
        ));
    }
    parse_utc_timestamp("revoked_at_utc", &input.revoked_at_utc)
        .map_err(|error| ReadinessPersistenceError::invalid_payload(error.message))?;

    sqlx::query(INSERT_READINESS_WAIVER_REVOCATION_SQL)
        .bind(input.revocation_id.trim().to_ascii_lowercase())
        .bind(input.waiver_id.trim().to_ascii_lowercase())
        .bind(input.revoked_by.trim().to_ascii_lowercase())
        .bind(input.revoked_reason_code.trim().to_ascii_lowercase())
        .bind(input.revoked_at_utc.trim())
        .execute(&mut *executor)
        .await
        .map_err(|error| classify_query_error("revoke_waiver.insert", error))?;
    Ok(())
}

pub async fn list_active_waivers(
    executor: &mut PgConnection,
    snapshot_id: &str,
    as_of_utc: &str,
) -> Result<WaiverListResult, ReadinessPersistenceError> {
    list_waivers_by_query(
        executor,
        LIST_ACTIVE_WAIVERS_SQL,
        snapshot_id,
        as_of_utc,
        Some(as_of_utc),
        WaiverState::Active,
    )
    .await
}

pub async fn list_expiring_waivers(
    executor: &mut PgConnection,
    snapshot_id: &str,
    as_of_utc: &str,
    expiring_before_utc: &str,
) -> Result<WaiverListResult, ReadinessPersistenceError> {
    list_waivers_by_query(
        executor,
        LIST_EXPIRING_WAIVERS_SQL,
        snapshot_id,
        as_of_utc,
        Some(expiring_before_utc),
        WaiverState::Active,
    )
    .await
}

pub async fn list_expired_waivers(
    executor: &mut PgConnection,
    snapshot_id: &str,
    as_of_utc: &str,
) -> Result<WaiverListResult, ReadinessPersistenceError> {
    list_waivers_by_query(
        executor,
        LIST_EXPIRED_WAIVERS_SQL,
        snapshot_id,
        as_of_utc,
        None,
        WaiverState::Expired,
    )
    .await
}

async fn list_waivers_by_query(
    executor: &mut PgConnection,
    sql: &'static str,
    snapshot_id: &str,
    as_of_utc: &str,
    second_timestamp: Option<&str>,
    state: WaiverState,
) -> Result<WaiverListResult, ReadinessPersistenceError> {
    if snapshot_id.trim().is_empty() {
        return Err(ReadinessPersistenceError::invalid_payload(
            "snapshot_id is required",
        ));
    }
    parse_utc_timestamp("as_of_utc", as_of_utc)
        .map_err(|error| ReadinessPersistenceError::invalid_payload(error.message))?;
    if let Some(candidate) = second_timestamp {
        parse_utc_timestamp("expiring_before_utc", candidate)
            .map_err(|error| ReadinessPersistenceError::invalid_payload(error.message))?;
    }

    let mut query = sqlx::query(sql)
        .bind(snapshot_id.trim().to_ascii_lowercase())
        .bind(as_of_utc.trim());
    if let Some(candidate) = second_timestamp
        && sql == LIST_EXPIRING_WAIVERS_SQL
    {
        query = query.bind(candidate.trim());
    }

    let rows = query
        .fetch_all(&mut *executor)
        .await
        .map_err(|error| classify_query_error("list_waivers_by_query.fetch", error))?;
    let mut waivers = Vec::with_capacity(rows.len());

    for row in rows {
        waivers.push(decode_waiver_row(&row)?);
    }

    Ok(WaiverListResult { state, waivers })
}

fn decode_waiver_row(row: &sqlx::postgres::PgRow) -> Result<WaiverRecord, ReadinessPersistenceError> {
    Ok(WaiverRecord {
        waiver_id: row
            .try_get("waiver_id")
            .map_err(|error| ReadinessPersistenceError::query_failure("decode.waiver_id", error))?,
        canonical_requirement_id: row.try_get("canonical_requirement_id").map_err(|error| {
            ReadinessPersistenceError::query_failure("decode.canonical_requirement_id", error)
        })?,
        owner: row
            .try_get("owner")
            .map_err(|error| ReadinessPersistenceError::query_failure("decode.owner", error))?,
        reason_code: row.try_get("reason_code").map_err(|error| {
            ReadinessPersistenceError::query_failure("decode.reason_code", error)
        })?,
        justification: row.try_get("justification").map_err(|error| {
            ReadinessPersistenceError::query_failure("decode.justification", error)
        })?,
        approved_by: row.try_get("approved_by").map_err(|error| {
            ReadinessPersistenceError::query_failure("decode.approved_by", error)
        })?,
        created_at_utc: row.try_get("created_at_utc").map_err(|error| {
            ReadinessPersistenceError::query_failure("decode.created_at_utc", error)
        })?,
        expires_at_utc: row.try_get("expires_at_utc").map_err(|error| {
            ReadinessPersistenceError::query_failure("decode.expires_at_utc", error)
        })?,
        revoked_at_utc: row.try_get("revoked_at_utc").map_err(|error| {
            ReadinessPersistenceError::query_failure("decode.revoked_at_utc", error)
        })?,
    })
}

fn classify_query_error(operation: &'static str, error: sqlx::Error) -> ReadinessPersistenceError {
    if is_constraint_error(&error) {
        return ReadinessPersistenceError::constraint_violation(operation, error);
    }
    ReadinessPersistenceError::query_failure(operation, error)
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

    const READINESS_MIGRATION_SQL: &str =
        include_str!("../../migrations/20260409000600_ci_readiness.sql");

    #[test]
    fn waiver_migration_defines_required_metadata_columns() {
        assert!(READINESS_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS readiness_waivers"));
        assert!(READINESS_MIGRATION_SQL.contains("canonical_requirement_id TEXT NOT NULL"));
        assert!(READINESS_MIGRATION_SQL.contains("owner TEXT NOT NULL"));
        assert!(READINESS_MIGRATION_SQL.contains("reason_code TEXT NOT NULL"));
        assert!(READINESS_MIGRATION_SQL.contains("justification TEXT NOT NULL"));
        assert!(READINESS_MIGRATION_SQL.contains("approved_by TEXT NOT NULL"));
        assert!(READINESS_MIGRATION_SQL.contains("expires_at_utc TIMESTAMPTZ NOT NULL"));
    }

    #[test]
    fn waiver_migration_enforces_immutable_tables_and_revoke_records() {
        assert!(READINESS_MIGRATION_SQL.contains("readiness_reject_mutation"));
        assert!(READINESS_MIGRATION_SQL.contains("readiness_waivers_immutable_update"));
        assert!(READINESS_MIGRATION_SQL.contains("readiness_waiver_revocations_immutable_update"));
        assert!(READINESS_MIGRATION_SQL.contains("readiness_waiver_revocations"));
    }

    #[test]
    fn waiver_sql_rejects_unknown_or_resolved_requirement_ids() {
        assert!(CHECK_UNRESOLVED_REQUIREMENT_SQL.contains("coverage_class IN ('partial', 'missing')"));
        assert!(READINESS_MIGRATION_SQL.contains("readiness_validate_waiver_unresolved"));
    }

    #[test]
    fn waiver_list_queries_exclude_revoked_and_sort_deterministically() {
        assert!(LIST_ACTIVE_WAIVERS_SQL.contains("wr.waiver_id IS NULL"));
        assert!(LIST_ACTIVE_WAIVERS_SQL.contains("ORDER BY w.expires_at_utc ASC"));
        assert!(LIST_EXPIRING_WAIVERS_SQL.contains("ORDER BY w.expires_at_utc ASC"));
        assert!(LIST_EXPIRED_WAIVERS_SQL.contains("ORDER BY w.expires_at_utc ASC"));
        assert!(LIST_ACTIVE_WAIVERS_SQL.contains("w.canonical_requirement_id ASC"));
        assert!(LIST_ACTIVE_WAIVERS_SQL.contains("w.waiver_id ASC"));
    }

    #[test]
    fn waiver_error_mapping_uses_machine_readable_codes() {
        let error = classify_query_error(
            "insert_waiver.insert",
            sqlx::Error::Protocol("boom".into()),
        );
        assert_eq!(error.code, "readiness_query_failed");
        assert!(error.message.contains("insert_waiver.insert"));
    }
}
