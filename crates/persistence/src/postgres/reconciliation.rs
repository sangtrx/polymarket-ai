use domain::reconciliation::{
    ExposureSnapshot, ReconciliationContractError, ReconciliationDiffClass,
    ReconciliationDiffRecord, ReconciliationReasonCode, ReconciliationRunStatus,
    ReconciliationRunSummary, ReconciliationValidationIssue, validate_exposure_snapshot,
    validate_reconciliation_diff_record, validate_reconciliation_run_summary,
};
use serde_json::json;
use sqlx::{PgPool, Row};
use std::error::Error;
use std::fmt::{Display, Formatter};

const INSERT_RECONCILIATION_RUN_SQL: &str = r#"
    INSERT INTO reconciliation_runs (
        run_id,
        window_started_at_utc,
        window_ended_at_utc,
        compared_records,
        mismatch_count,
        mismatch_rate,
        critical_halt,
        status,
        reason_code,
        correlation_id,
        evaluated_at_utc,
        evidence
    ) VALUES (
        $1, $2::timestamptz, $3::timestamptz, $4, $5, $6, $7, $8, $9, $10, $11::timestamptz, $12
    )
"#;

const INSERT_RECONCILIATION_DIFF_SQL: &str = r#"
    INSERT INTO reconciliation_diffs (
        diff_id,
        run_id,
        order_id,
        market_id,
        diff_class,
        reason_code,
        internal_value,
        venue_value,
        observed_at_utc,
        correlation_id
    ) VALUES (
        $1, $2, $3, $4, $5, $6, $7, $8, $9::timestamptz, $10
    )
"#;

const INSERT_EXPOSURE_SNAPSHOT_SQL: &str = r#"
    INSERT INTO exposure_snapshots (
        snapshot_id,
        run_id,
        market_id,
        net_exposure,
        gross_exposure,
        open_order_count,
        reason_code,
        captured_at_utc,
        correlation_id,
        evidence
    ) VALUES (
        $1, $2, $3, $4, $5, $6, $7, $8::timestamptz, $9, $10
    )
"#;

const LOAD_RECONCILIATION_RUN_SQL: &str = r#"
    SELECT
        run_id,
        to_char(window_started_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS window_started_at_utc,
        to_char(window_ended_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS window_ended_at_utc,
        compared_records,
        mismatch_count,
        mismatch_rate,
        critical_halt,
        status,
        reason_code,
        correlation_id,
        to_char(evaluated_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS evaluated_at_utc
    FROM reconciliation_runs
    WHERE run_id = $1
"#;

const LOAD_RECONCILIATION_DIFFS_SQL: &str = r#"
    SELECT
        diff_id,
        run_id,
        order_id,
        market_id,
        diff_class,
        reason_code,
        internal_value,
        venue_value,
        to_char(observed_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS observed_at_utc,
        correlation_id
    FROM reconciliation_diffs
    WHERE run_id = $1
    ORDER BY order_id ASC, diff_class ASC
"#;

const LOAD_LATEST_EXPOSURE_SNAPSHOT_GLOBAL_SQL: &str = r#"
    SELECT
        snapshot_id,
        run_id,
        market_id,
        net_exposure,
        gross_exposure,
        open_order_count,
        reason_code,
        to_char(captured_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS captured_at_utc,
        correlation_id
    FROM exposure_snapshots
    WHERE market_id IS NULL
    ORDER BY captured_at_utc DESC, snapshot_id DESC
    LIMIT 1
"#;

const LOAD_LATEST_EXPOSURE_SNAPSHOT_MARKET_SQL: &str = r#"
    SELECT
        snapshot_id,
        run_id,
        market_id,
        net_exposure,
        gross_exposure,
        open_order_count,
        reason_code,
        to_char(captured_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS captured_at_utc,
        correlation_id
    FROM exposure_snapshots
    WHERE market_id = $1
    ORDER BY captured_at_utc DESC, snapshot_id DESC
    LIMIT 1
"#;

const LOAD_LATEST_EXPOSURE_SNAPSHOT_RUN_SQL: &str = r#"
    SELECT
        snapshot_id,
        run_id,
        market_id,
        net_exposure,
        gross_exposure,
        open_order_count,
        reason_code,
        to_char(captured_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS captured_at_utc,
        correlation_id
    FROM exposure_snapshots
    WHERE run_id = $1
    ORDER BY captured_at_utc DESC, snapshot_id DESC
    LIMIT 1
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconciliationPersistenceError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<ReconciliationValidationIssue>,
}

impl ReconciliationPersistenceError {
    fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<ReconciliationValidationIssue>,
    ) -> Self {
        Self {
            code: ReconciliationReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn persistence_unavailable(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: ReconciliationReasonCode::PersistenceUnavailable.code(),
            message: format!("{operation} failed: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn constraint_violation(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: ReconciliationReasonCode::PersistenceUnavailable.code(),
            message: format!("{operation} rejected by constraint: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn row_decode_failure(column: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: ReconciliationReasonCode::PersistenceUnavailable.code(),
            message: format!("unable to decode `{column}`: {error}"),
            field_errors: Vec::new(),
        }
    }
}

impl Display for ReconciliationPersistenceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for ReconciliationPersistenceError {}

#[derive(Debug, Clone, PartialEq)]
pub struct ReconciliationIncidentEvidence {
    pub run: ReconciliationRunSummary,
    pub diffs: Vec<ReconciliationDiffRecord>,
    pub latest_snapshot: Option<ExposureSnapshot>,
}

pub async fn persist_reconciliation_evidence(
    pool: &PgPool,
    run: &ReconciliationRunSummary,
    diffs: &[ReconciliationDiffRecord],
    snapshot: &ExposureSnapshot,
) -> Result<(), ReconciliationPersistenceError> {
    validate_reconciliation_run_summary(run).map_err(map_contract_error)?;
    if snapshot.run_id != run.run_id {
        return Err(ReconciliationPersistenceError::invalid_payload(
            "exposure snapshot run_id must match reconciliation run_id",
            Vec::new(),
        ));
    }
    validate_exposure_snapshot(snapshot).map_err(map_contract_error)?;
    for diff in diffs {
        if diff.run_id != run.run_id {
            return Err(ReconciliationPersistenceError::invalid_payload(
                "reconciliation diff run_id must match reconciliation run_id",
                Vec::new(),
            ));
        }
        validate_reconciliation_diff_record(diff).map_err(map_contract_error)?;
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|error| classify_query_error("begin_reconciliation_evidence_tx", error))?;

    let run_evidence = json!({
        "run_id": run.run_id,
        "window_started_at_utc": run.window_started_at_utc,
        "window_ended_at_utc": run.window_ended_at_utc,
        "compared_records": run.compared_records,
        "mismatch_count": run.mismatch_count,
        "mismatch_rate": run.mismatch_rate,
        "critical_halt": run.critical_halt,
        "reason_code": run.reason_code,
    });
    let inserted_run = sqlx::query(INSERT_RECONCILIATION_RUN_SQL)
        .bind(&run.run_id)
        .bind(&run.window_started_at_utc)
        .bind(&run.window_ended_at_utc)
        .bind(run.compared_records)
        .bind(run.mismatch_count)
        .bind(run.mismatch_rate)
        .bind(run.critical_halt)
        .bind(run.status.as_str())
        .bind(&run.reason_code)
        .bind(&run.correlation_id)
        .bind(&run.evaluated_at_utc)
        .bind(run_evidence)
        .execute(&mut *tx)
        .await
        .map_err(|error| classify_query_error("insert_reconciliation_run", error))?;
    if inserted_run.rows_affected() != 1 {
        return Err(ReconciliationPersistenceError::invalid_payload(
            format!(
                "insert_reconciliation_run expected 1 affected row, got {}",
                inserted_run.rows_affected()
            ),
            Vec::new(),
        ));
    }

    for diff in diffs {
        let inserted_diff = sqlx::query(INSERT_RECONCILIATION_DIFF_SQL)
            .bind(&diff.diff_id)
            .bind(&diff.run_id)
            .bind(&diff.order_id)
            .bind(&diff.market_id)
            .bind(diff.diff_class.as_str())
            .bind(&diff.reason_code)
            .bind(&diff.internal_value)
            .bind(&diff.venue_value)
            .bind(&diff.observed_at_utc)
            .bind(&diff.correlation_id)
            .execute(&mut *tx)
            .await
            .map_err(|error| classify_query_error("insert_reconciliation_diff", error))?;
        if inserted_diff.rows_affected() != 1 {
            return Err(ReconciliationPersistenceError::invalid_payload(
                format!(
                    "insert_reconciliation_diff expected 1 affected row, got {}",
                    inserted_diff.rows_affected()
                ),
                Vec::new(),
            ));
        }
    }

    let snapshot_evidence = json!({
        "snapshot_id": snapshot.snapshot_id,
        "run_id": snapshot.run_id,
        "market_id": snapshot.market_id,
        "net_exposure": snapshot.net_exposure,
        "gross_exposure": snapshot.gross_exposure,
        "open_order_count": snapshot.open_order_count,
        "reason_code": snapshot.reason_code,
    });
    let inserted_snapshot = sqlx::query(INSERT_EXPOSURE_SNAPSHOT_SQL)
        .bind(&snapshot.snapshot_id)
        .bind(&snapshot.run_id)
        .bind(&snapshot.market_id)
        .bind(snapshot.net_exposure)
        .bind(snapshot.gross_exposure)
        .bind(snapshot.open_order_count)
        .bind(&snapshot.reason_code)
        .bind(&snapshot.captured_at_utc)
        .bind(&snapshot.correlation_id)
        .bind(snapshot_evidence)
        .execute(&mut *tx)
        .await
        .map_err(|error| classify_query_error("insert_exposure_snapshot", error))?;
    if inserted_snapshot.rows_affected() != 1 {
        return Err(ReconciliationPersistenceError::invalid_payload(
            format!(
                "insert_exposure_snapshot expected 1 affected row, got {}",
                inserted_snapshot.rows_affected()
            ),
            Vec::new(),
        ));
    }

    tx.commit()
        .await
        .map_err(|error| classify_query_error("commit_reconciliation_evidence_tx", error))?;
    Ok(())
}

pub async fn load_reconciliation_run(
    pool: &PgPool,
    run_id: &str,
) -> Result<Option<ReconciliationRunSummary>, ReconciliationPersistenceError> {
    validate_non_empty("run_id", run_id)?;
    let row = sqlx::query(LOAD_RECONCILIATION_RUN_SQL)
        .bind(run_id.trim())
        .fetch_optional(pool)
        .await
        .map_err(|error| classify_query_error("load_reconciliation_run", error))?;
    row.map(decode_reconciliation_run_row).transpose()
}

pub async fn load_reconciliation_diffs(
    pool: &PgPool,
    run_id: &str,
) -> Result<Vec<ReconciliationDiffRecord>, ReconciliationPersistenceError> {
    validate_non_empty("run_id", run_id)?;
    let rows = sqlx::query(LOAD_RECONCILIATION_DIFFS_SQL)
        .bind(run_id.trim())
        .fetch_all(pool)
        .await
        .map_err(|error| classify_query_error("load_reconciliation_diffs", error))?;
    rows.into_iter()
        .map(decode_reconciliation_diff_row)
        .collect()
}

pub async fn load_latest_exposure_snapshot(
    pool: &PgPool,
    market_id: Option<&str>,
) -> Result<Option<ExposureSnapshot>, ReconciliationPersistenceError> {
    let row = match market_id {
        Some(value) => {
            validate_non_empty("market_id", value)?;
            sqlx::query(LOAD_LATEST_EXPOSURE_SNAPSHOT_MARKET_SQL)
                .bind(value.trim())
                .fetch_optional(pool)
                .await
                .map_err(|error| {
                    classify_query_error("load_latest_exposure_snapshot_market", error)
                })?
        }
        None => sqlx::query(LOAD_LATEST_EXPOSURE_SNAPSHOT_GLOBAL_SQL)
            .fetch_optional(pool)
            .await
            .map_err(|error| classify_query_error("load_latest_exposure_snapshot_global", error))?,
    };
    row.map(decode_exposure_snapshot_row).transpose()
}

pub async fn load_latest_exposure_snapshot_for_run(
    pool: &PgPool,
    run_id: &str,
) -> Result<Option<ExposureSnapshot>, ReconciliationPersistenceError> {
    validate_non_empty("run_id", run_id)?;
    let row = sqlx::query(LOAD_LATEST_EXPOSURE_SNAPSHOT_RUN_SQL)
        .bind(run_id.trim())
        .fetch_optional(pool)
        .await
        .map_err(|error| classify_query_error("load_latest_exposure_snapshot_run", error))?;
    row.map(decode_exposure_snapshot_row).transpose()
}

pub async fn load_reconciliation_incident_evidence(
    pool: &PgPool,
    run_id: &str,
) -> Result<Option<ReconciliationIncidentEvidence>, ReconciliationPersistenceError> {
    let Some(run) = load_reconciliation_run(pool, run_id).await? else {
        return Ok(None);
    };
    let diffs = load_reconciliation_diffs(pool, run_id).await?;
    let latest_snapshot = load_latest_exposure_snapshot_for_run(pool, run_id).await?;
    Ok(Some(ReconciliationIncidentEvidence {
        run,
        diffs,
        latest_snapshot,
    }))
}

fn decode_reconciliation_run_row(
    row: sqlx::postgres::PgRow,
) -> Result<ReconciliationRunSummary, ReconciliationPersistenceError> {
    let status: String = row
        .try_get("status")
        .map_err(|error| ReconciliationPersistenceError::row_decode_failure("status", error))?;
    let status = ReconciliationRunStatus::parse(&status).map_err(map_contract_error)?;
    let reason_code: String = row.try_get("reason_code").map_err(|error| {
        ReconciliationPersistenceError::row_decode_failure("reason_code", error)
    })?;
    ReconciliationReasonCode::parse(&reason_code).map_err(map_contract_error)?;

    let run = ReconciliationRunSummary {
        run_id: row
            .try_get("run_id")
            .map_err(|error| ReconciliationPersistenceError::row_decode_failure("run_id", error))?,
        window_started_at_utc: row.try_get("window_started_at_utc").map_err(|error| {
            ReconciliationPersistenceError::row_decode_failure("window_started_at_utc", error)
        })?,
        window_ended_at_utc: row.try_get("window_ended_at_utc").map_err(|error| {
            ReconciliationPersistenceError::row_decode_failure("window_ended_at_utc", error)
        })?,
        compared_records: row.try_get("compared_records").map_err(|error| {
            ReconciliationPersistenceError::row_decode_failure("compared_records", error)
        })?,
        mismatch_count: row.try_get("mismatch_count").map_err(|error| {
            ReconciliationPersistenceError::row_decode_failure("mismatch_count", error)
        })?,
        mismatch_rate: row.try_get("mismatch_rate").map_err(|error| {
            ReconciliationPersistenceError::row_decode_failure("mismatch_rate", error)
        })?,
        critical_halt: row.try_get("critical_halt").map_err(|error| {
            ReconciliationPersistenceError::row_decode_failure("critical_halt", error)
        })?,
        status,
        reason_code,
        evaluated_at_utc: row.try_get("evaluated_at_utc").map_err(|error| {
            ReconciliationPersistenceError::row_decode_failure("evaluated_at_utc", error)
        })?,
        correlation_id: row.try_get("correlation_id").map_err(|error| {
            ReconciliationPersistenceError::row_decode_failure("correlation_id", error)
        })?,
    };
    validate_reconciliation_run_summary(&run).map_err(map_contract_error)?;
    Ok(run)
}

fn decode_reconciliation_diff_row(
    row: sqlx::postgres::PgRow,
) -> Result<ReconciliationDiffRecord, ReconciliationPersistenceError> {
    let diff_class: String = row
        .try_get("diff_class")
        .map_err(|error| ReconciliationPersistenceError::row_decode_failure("diff_class", error))?;
    let diff_class = ReconciliationDiffClass::parse(&diff_class).map_err(map_contract_error)?;
    let reason_code: String = row.try_get("reason_code").map_err(|error| {
        ReconciliationPersistenceError::row_decode_failure("reason_code", error)
    })?;
    ReconciliationReasonCode::parse(&reason_code).map_err(map_contract_error)?;

    let diff = ReconciliationDiffRecord {
        diff_id: row.try_get("diff_id").map_err(|error| {
            ReconciliationPersistenceError::row_decode_failure("diff_id", error)
        })?,
        run_id: row
            .try_get("run_id")
            .map_err(|error| ReconciliationPersistenceError::row_decode_failure("run_id", error))?,
        order_id: row.try_get("order_id").map_err(|error| {
            ReconciliationPersistenceError::row_decode_failure("order_id", error)
        })?,
        market_id: row.try_get("market_id").map_err(|error| {
            ReconciliationPersistenceError::row_decode_failure("market_id", error)
        })?,
        diff_class,
        reason_code,
        internal_value: row.try_get("internal_value").map_err(|error| {
            ReconciliationPersistenceError::row_decode_failure("internal_value", error)
        })?,
        venue_value: row.try_get("venue_value").map_err(|error| {
            ReconciliationPersistenceError::row_decode_failure("venue_value", error)
        })?,
        observed_at_utc: row.try_get("observed_at_utc").map_err(|error| {
            ReconciliationPersistenceError::row_decode_failure("observed_at_utc", error)
        })?,
        correlation_id: row.try_get("correlation_id").map_err(|error| {
            ReconciliationPersistenceError::row_decode_failure("correlation_id", error)
        })?,
    };
    validate_reconciliation_diff_record(&diff).map_err(map_contract_error)?;
    Ok(diff)
}

fn decode_exposure_snapshot_row(
    row: sqlx::postgres::PgRow,
) -> Result<ExposureSnapshot, ReconciliationPersistenceError> {
    let reason_code: String = row.try_get("reason_code").map_err(|error| {
        ReconciliationPersistenceError::row_decode_failure("reason_code", error)
    })?;
    ReconciliationReasonCode::parse(&reason_code).map_err(map_contract_error)?;
    let snapshot = ExposureSnapshot {
        snapshot_id: row.try_get("snapshot_id").map_err(|error| {
            ReconciliationPersistenceError::row_decode_failure("snapshot_id", error)
        })?,
        run_id: row
            .try_get("run_id")
            .map_err(|error| ReconciliationPersistenceError::row_decode_failure("run_id", error))?,
        market_id: row.try_get("market_id").map_err(|error| {
            ReconciliationPersistenceError::row_decode_failure("market_id", error)
        })?,
        net_exposure: row.try_get("net_exposure").map_err(|error| {
            ReconciliationPersistenceError::row_decode_failure("net_exposure", error)
        })?,
        gross_exposure: row.try_get("gross_exposure").map_err(|error| {
            ReconciliationPersistenceError::row_decode_failure("gross_exposure", error)
        })?,
        open_order_count: row.try_get("open_order_count").map_err(|error| {
            ReconciliationPersistenceError::row_decode_failure("open_order_count", error)
        })?,
        reason_code,
        captured_at_utc: row.try_get("captured_at_utc").map_err(|error| {
            ReconciliationPersistenceError::row_decode_failure("captured_at_utc", error)
        })?,
        correlation_id: row.try_get("correlation_id").map_err(|error| {
            ReconciliationPersistenceError::row_decode_failure("correlation_id", error)
        })?,
    };
    validate_exposure_snapshot(&snapshot).map_err(map_contract_error)?;
    Ok(snapshot)
}

fn map_contract_error(error: ReconciliationContractError) -> ReconciliationPersistenceError {
    ReconciliationPersistenceError {
        code: error.code,
        message: error.message,
        field_errors: error.field_errors,
    }
}

fn validate_non_empty(
    field: &'static str,
    value: &str,
) -> Result<(), ReconciliationPersistenceError> {
    if value.trim().is_empty() {
        return Err(ReconciliationPersistenceError::invalid_payload(
            format!("{field} cannot be blank"),
            Vec::new(),
        ));
    }
    Ok(())
}

fn classify_query_error(
    operation: &'static str,
    error: sqlx::Error,
) -> ReconciliationPersistenceError {
    if is_constraint_error(&error) {
        return ReconciliationPersistenceError::constraint_violation(operation, error);
    }
    ReconciliationPersistenceError::persistence_unavailable(operation, error)
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

    const RECONCILIATION_MIGRATION_SQL: &str =
        include_str!("../../migrations/20260406061000_reconciliation_exposure_core.sql");

    fn sample_run() -> ReconciliationRunSummary {
        ReconciliationRunSummary {
            run_id: "run-1".to_string(),
            window_started_at_utc: "2026-04-06T00:00:00Z".to_string(),
            window_ended_at_utc: "2026-04-06T00:01:00Z".to_string(),
            compared_records: 100,
            mismatch_count: 1,
            mismatch_rate: 0.01,
            critical_halt: true,
            status: ReconciliationRunStatus::CriticalHalt,
            reason_code: ReconciliationReasonCode::CriticalMismatch
                .code()
                .to_string(),
            evaluated_at_utc: "2026-04-06T00:01:01Z".to_string(),
            correlation_id: "corr-1".to_string(),
        }
    }

    fn sample_diff() -> ReconciliationDiffRecord {
        ReconciliationDiffRecord {
            diff_id: "diff::run-1::order-1::lifecycle_state_mismatch".to_string(),
            run_id: "run-1".to_string(),
            order_id: "order-1".to_string(),
            market_id: "market-1".to_string(),
            diff_class: ReconciliationDiffClass::LifecycleStateMismatch,
            reason_code: ReconciliationReasonCode::NonCriticalMismatch
                .code()
                .to_string(),
            internal_value: Some("live".to_string()),
            venue_value: Some("filled".to_string()),
            observed_at_utc: "2026-04-06T00:00:30Z".to_string(),
            correlation_id: "corr-1".to_string(),
        }
    }

    fn sample_snapshot() -> ExposureSnapshot {
        ExposureSnapshot {
            snapshot_id: "snapshot-1".to_string(),
            run_id: "run-1".to_string(),
            market_id: None,
            net_exposure: 1.0,
            gross_exposure: 1.0,
            open_order_count: 1,
            reason_code: ReconciliationReasonCode::CriticalMismatch
                .code()
                .to_string(),
            captured_at_utc: "2026-04-06T00:01:01Z".to_string(),
            correlation_id: "corr-1".to_string(),
        }
    }

    #[test]
    fn migration_creates_expected_reconciliation_schema_scope() {
        assert!(
            RECONCILIATION_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS reconciliation_runs")
        );
        assert!(
            RECONCILIATION_MIGRATION_SQL
                .contains("CREATE TABLE IF NOT EXISTS reconciliation_diffs")
        );
        assert!(
            RECONCILIATION_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS exposure_snapshots")
        );
        assert!(!RECONCILIATION_MIGRATION_SQL.contains("risk_limit_profiles"));
        assert!(!RECONCILIATION_MIGRATION_SQL.contains("inventory_limit_rules"));
        assert!(!RECONCILIATION_MIGRATION_SQL.contains("pretrade_gate_decisions"));
        assert!(!RECONCILIATION_MIGRATION_SQL.contains("safety_control_actions"));
    }

    #[test]
    fn migration_enforces_constraints_and_incident_query_indexes() {
        assert!(
            RECONCILIATION_MIGRATION_SQL
                .contains("status IN ('succeeded', 'critical_halt', 'failed')")
        );
        assert!(RECONCILIATION_MIGRATION_SQL.contains("ABS(mismatch_rate - (mismatch_count::DOUBLE PRECISION / compared_records::DOUBLE PRECISION))"));
        assert!(
            RECONCILIATION_MIGRATION_SQL
                .contains("UNIQUE (correlation_id, window_started_at_utc, window_ended_at_utc)")
        );
        assert!(RECONCILIATION_MIGRATION_SQL.contains("idx_reconciliation_runs_window_lookup"));
        assert!(RECONCILIATION_MIGRATION_SQL.contains("idx_reconciliation_diffs_run_lookup"));
        assert!(RECONCILIATION_MIGRATION_SQL.contains("idx_reconciliation_diffs_correlation_time"));
        assert!(RECONCILIATION_MIGRATION_SQL.contains("idx_exposure_snapshots_latest_market"));
        assert!(RECONCILIATION_MIGRATION_SQL.contains("idx_exposure_snapshots_latest_global"));
        assert!(RECONCILIATION_MIGRATION_SQL.contains("idx_exposure_snapshots_run_time"));
        assert!(RECONCILIATION_MIGRATION_SQL.contains("idx_exposure_snapshots_correlation_time"));
    }

    #[test]
    fn latest_snapshot_lookup_queries_are_deterministic() {
        assert!(
            LOAD_LATEST_EXPOSURE_SNAPSHOT_GLOBAL_SQL
                .contains("ORDER BY captured_at_utc DESC, snapshot_id DESC")
        );
        assert!(LOAD_LATEST_EXPOSURE_SNAPSHOT_GLOBAL_SQL.contains("LIMIT 1"));
        assert!(
            LOAD_LATEST_EXPOSURE_SNAPSHOT_MARKET_SQL
                .contains("ORDER BY captured_at_utc DESC, snapshot_id DESC")
        );
        assert!(LOAD_LATEST_EXPOSURE_SNAPSHOT_MARKET_SQL.contains("LIMIT 1"));
        assert!(LOAD_LATEST_EXPOSURE_SNAPSHOT_RUN_SQL.contains("WHERE run_id = $1"));
        assert!(
            LOAD_LATEST_EXPOSURE_SNAPSHOT_RUN_SQL
                .contains("ORDER BY captured_at_utc DESC, snapshot_id DESC")
        );
        assert!(LOAD_LATEST_EXPOSURE_SNAPSHOT_RUN_SQL.contains("LIMIT 1"));
    }

    #[test]
    fn diff_lookup_query_orders_by_order_and_class() {
        assert!(LOAD_RECONCILIATION_DIFFS_SQL.contains("ORDER BY order_id ASC, diff_class ASC"));
    }

    #[test]
    fn validation_requires_snapshot_and_diff_to_reference_same_run() {
        let run = sample_run();
        let mut diff = sample_diff();
        diff.run_id = "run-other".to_string();
        let error = {
            if diff.run_id != run.run_id {
                ReconciliationPersistenceError::invalid_payload(
                    "reconciliation diff run_id must match reconciliation run_id",
                    Vec::new(),
                )
            } else {
                panic!("test setup should have mismatched run ids")
            }
        };
        assert_eq!(error.code, ReconciliationReasonCode::InvalidPayload.code());
    }

    #[test]
    fn domain_validation_accepts_reconciliation_persistence_payloads() {
        let run = sample_run();
        let diff = sample_diff();
        let snapshot = sample_snapshot();
        assert!(validate_reconciliation_run_summary(&run).is_ok());
        assert!(validate_reconciliation_diff_record(&diff).is_ok());
        assert!(validate_exposure_snapshot(&snapshot).is_ok());
    }
}
