use domain::attribution::{
    AttributionContractError, AttributionPeriod, AttributionReasonCode, AttributionRow,
    AttributionValidationIssue, normalize_attribution_identifier, validate_attribution_row,
};
use serde_json::json;
use sqlx::{PgPool, Row};
use std::error::Error;
use std::fmt::{Display, Formatter};

const UPSERT_ATTRIBUTION_SNAPSHOT_SQL: &str = r#"
    INSERT INTO attribution_snapshots (
        snapshot_id,
        period_scope,
        period_start_utc,
        period_end_utc,
        market_id,
        alpha_id,
        realized_pnl_usd,
        unrealized_pnl_usd,
        fees_usd,
        rebates_usd,
        incentives_usd,
        gross_pnl_usd,
        net_pnl_usd,
        reason_code,
        as_of_utc,
        source,
        correlation_id,
        run_id,
        upstream_snapshot_id,
        evidence
    ) VALUES (
        $1, $2, $3::timestamptz, $4::timestamptz, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14,
        $15::timestamptz, $16, $17, $18, $19, $20::jsonb
    )
    ON CONFLICT (snapshot_id)
    DO UPDATE SET
        period_scope = EXCLUDED.period_scope,
        period_start_utc = EXCLUDED.period_start_utc,
        period_end_utc = EXCLUDED.period_end_utc,
        market_id = EXCLUDED.market_id,
        alpha_id = EXCLUDED.alpha_id,
        realized_pnl_usd = EXCLUDED.realized_pnl_usd,
        unrealized_pnl_usd = EXCLUDED.unrealized_pnl_usd,
        fees_usd = EXCLUDED.fees_usd,
        rebates_usd = EXCLUDED.rebates_usd,
        incentives_usd = EXCLUDED.incentives_usd,
        gross_pnl_usd = EXCLUDED.gross_pnl_usd,
        net_pnl_usd = EXCLUDED.net_pnl_usd,
        reason_code = EXCLUDED.reason_code,
        as_of_utc = EXCLUDED.as_of_utc,
        source = EXCLUDED.source,
        correlation_id = EXCLUDED.correlation_id,
        run_id = EXCLUDED.run_id,
        upstream_snapshot_id = EXCLUDED.upstream_snapshot_id,
        evidence = EXCLUDED.evidence
"#;

const LOAD_LATEST_ATTRIBUTION_SNAPSHOTS_SQL: &str = r#"
    SELECT
        snapshot_id,
        period_scope,
        to_char(period_start_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS period_start_utc,
        to_char(period_end_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS period_end_utc,
        market_id,
        alpha_id,
        realized_pnl_usd,
        unrealized_pnl_usd,
        fees_usd,
        rebates_usd,
        incentives_usd,
        gross_pnl_usd,
        net_pnl_usd,
        reason_code,
        to_char(as_of_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS as_of_utc,
        source,
        correlation_id,
        run_id,
        upstream_snapshot_id
    FROM attribution_snapshots
    WHERE period_scope = $1
      AND ($2::text IS NULL OR market_id = $2)
      AND ($3::text IS NULL OR alpha_id = $3)
    ORDER BY period_end_utc DESC, market_id ASC, alpha_id ASC, snapshot_id ASC
    LIMIT $4
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributionPersistenceError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<AttributionValidationIssue>,
}

impl AttributionPersistenceError {
    fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<AttributionValidationIssue>,
    ) -> Self {
        Self {
            code: AttributionReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn query_failure(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "attribution_query_failed",
            message: format!("{operation} failed: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn constraint_violation(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "attribution_constraint_violation",
            message: format!("{operation} rejected by constraint: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn row_decode_failure(column: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "attribution_row_decode_failed",
            message: format!("unable to decode `{column}`: {error}"),
            field_errors: Vec::new(),
        }
    }
}

impl Display for AttributionPersistenceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for AttributionPersistenceError {}

pub async fn upsert_attribution_snapshot(
    pool: &PgPool,
    row: &AttributionRow,
) -> Result<(), AttributionPersistenceError> {
    validate_attribution_row(row).map_err(map_contract_error)?;

    let snapshot_id = row
        .snapshot_id
        .as_deref()
        .map(normalize_attribution_identifier)
        .unwrap_or_else(|| {
            normalize_attribution_identifier(&format!(
                "attr::{}::{}::{}::{}",
                row.period, row.market_id, row.alpha_id, row.correlation_id
            ))
        });
    let normalized_run_id = row.run_id.as_deref().map(normalize_attribution_identifier);
    let normalized_upstream_snapshot_id = row
        .snapshot_id
        .as_deref()
        .map(normalize_attribution_identifier);

    let result = sqlx::query(UPSERT_ATTRIBUTION_SNAPSHOT_SQL)
        .bind(snapshot_id)
        .bind(&row.period)
        .bind(&row.period_start_utc)
        .bind(&row.period_end_utc)
        .bind(normalize_attribution_identifier(&row.market_id))
        .bind(normalize_attribution_identifier(&row.alpha_id))
        .bind(row.realized_pnl_usd)
        .bind(row.unrealized_pnl_usd)
        .bind(row.costs.fees_usd)
        .bind(row.costs.rebates_usd)
        .bind(row.costs.incentives_usd)
        .bind(row.gross_pnl_usd)
        .bind(row.net_pnl_usd)
        .bind(&row.reason_code)
        .bind(&row.as_of_utc)
        .bind(&row.source)
        .bind(&row.correlation_id)
        .bind(normalized_run_id.as_deref())
        .bind(normalized_upstream_snapshot_id.as_deref())
        .bind(json!({
            "period": row.period,
            "period_start_utc": row.period_start_utc,
            "period_end_utc": row.period_end_utc,
            "cost_breakdown": {
                "fees_usd": row.costs.fees_usd,
                "rebates_usd": row.costs.rebates_usd,
                "incentives_usd": row.costs.incentives_usd,
                "net_cost_impact_usd": row.costs.net_cost_impact_usd
            },
            "source": row.source,
            "reason_code": row.reason_code,
            "correlation_id": row.correlation_id,
            "run_id": row.run_id,
            "upstream_snapshot_id": row.snapshot_id,
        }))
        .execute(pool)
        .await
        .map_err(|error| classify_query_error("upsert_attribution_snapshot", error))?;

    if result.rows_affected() != 1 {
        return Err(AttributionPersistenceError::invalid_payload(
            format!(
                "upsert_attribution_snapshot expected 1 affected row, got {}",
                result.rows_affected()
            ),
            Vec::new(),
        ));
    }

    Ok(())
}

pub async fn load_latest_attribution_snapshots(
    pool: &PgPool,
    period: AttributionPeriod,
    market_id: Option<&str>,
    alpha_id: Option<&str>,
    limit: i64,
) -> Result<Vec<AttributionRow>, AttributionPersistenceError> {
    if limit <= 0 {
        return Err(AttributionPersistenceError::invalid_payload(
            "limit must be greater than zero",
            vec![AttributionValidationIssue {
                field: "limit",
                code: AttributionReasonCode::InvalidPayload.code(),
                message: "limit must be greater than zero".to_string(),
            }],
        ));
    }

    let normalized_market = normalize_query_identifier("market_id", market_id)?;
    let normalized_alpha = normalize_query_identifier("alpha_id", alpha_id)?;

    let rows = sqlx::query(LOAD_LATEST_ATTRIBUTION_SNAPSHOTS_SQL)
        .bind(period.as_str())
        .bind(normalized_market.as_deref())
        .bind(normalized_alpha.as_deref())
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|error| classify_query_error("load_latest_attribution_snapshots", error))?;

    rows.into_iter().map(decode_attribution_row).collect()
}

fn decode_attribution_row(
    row: sqlx::postgres::PgRow,
) -> Result<AttributionRow, AttributionPersistenceError> {
    let reason_code: String = row
        .try_get("reason_code")
        .map_err(|error| AttributionPersistenceError::row_decode_failure("reason_code", error))?;
    AttributionReasonCode::parse(&reason_code).map_err(map_contract_error)?;

    let period: String = row
        .try_get("period_scope")
        .map_err(|error| AttributionPersistenceError::row_decode_failure("period_scope", error))?;
    AttributionPeriod::parse(&period).map_err(map_contract_error)?;
    let fees_usd = row
        .try_get("fees_usd")
        .map_err(|error| AttributionPersistenceError::row_decode_failure("fees_usd", error))?;
    let rebates_usd = row
        .try_get("rebates_usd")
        .map_err(|error| AttributionPersistenceError::row_decode_failure("rebates_usd", error))?;
    let incentives_usd = row.try_get("incentives_usd").map_err(|error| {
        AttributionPersistenceError::row_decode_failure("incentives_usd", error)
    })?;

    let decoded = AttributionRow {
        market_id: row
            .try_get("market_id")
            .map_err(|error| AttributionPersistenceError::row_decode_failure("market_id", error))?,
        alpha_id: row
            .try_get("alpha_id")
            .map_err(|error| AttributionPersistenceError::row_decode_failure("alpha_id", error))?,
        period,
        period_start_utc: row.try_get("period_start_utc").map_err(|error| {
            AttributionPersistenceError::row_decode_failure("period_start_utc", error)
        })?,
        period_end_utc: row.try_get("period_end_utc").map_err(|error| {
            AttributionPersistenceError::row_decode_failure("period_end_utc", error)
        })?,
        realized_pnl_usd: row.try_get("realized_pnl_usd").map_err(|error| {
            AttributionPersistenceError::row_decode_failure("realized_pnl_usd", error)
        })?,
        unrealized_pnl_usd: row.try_get("unrealized_pnl_usd").map_err(|error| {
            AttributionPersistenceError::row_decode_failure("unrealized_pnl_usd", error)
        })?,
        gross_pnl_usd: row.try_get("gross_pnl_usd").map_err(|error| {
            AttributionPersistenceError::row_decode_failure("gross_pnl_usd", error)
        })?,
        net_pnl_usd: row.try_get("net_pnl_usd").map_err(|error| {
            AttributionPersistenceError::row_decode_failure("net_pnl_usd", error)
        })?,
        costs: domain::attribution::AttributionCostBreakdown {
            fees_usd,
            rebates_usd,
            incentives_usd,
            net_cost_impact_usd: fees_usd - rebates_usd - incentives_usd,
        },
        as_of_utc: row
            .try_get("as_of_utc")
            .map_err(|error| AttributionPersistenceError::row_decode_failure("as_of_utc", error))?,
        source: row
            .try_get("source")
            .map_err(|error| AttributionPersistenceError::row_decode_failure("source", error))?,
        reason_code,
        correlation_id: row.try_get("correlation_id").map_err(|error| {
            AttributionPersistenceError::row_decode_failure("correlation_id", error)
        })?,
        snapshot_id: row.try_get("snapshot_id").map_err(|error| {
            AttributionPersistenceError::row_decode_failure("snapshot_id", error)
        })?,
        run_id: row
            .try_get("run_id")
            .map_err(|error| AttributionPersistenceError::row_decode_failure("run_id", error))?,
    };

    validate_attribution_row(&decoded).map_err(map_contract_error)?;
    Ok(decoded)
}

fn normalize_query_identifier(
    field: &'static str,
    value: Option<&str>,
) -> Result<Option<String>, AttributionPersistenceError> {
    match value {
        Some(raw) if raw.trim().is_empty() => Err(AttributionPersistenceError::invalid_payload(
            format!("{field} cannot be blank"),
            vec![AttributionValidationIssue {
                field,
                code: AttributionReasonCode::InvalidPayload.code(),
                message: format!("{field} cannot be blank"),
            }],
        )),
        Some(raw) => {
            let normalized = normalize_attribution_identifier(raw);
            let valid_length = (3..=120).contains(&normalized.len());
            let valid_characters = normalized.chars().all(|character| {
                character.is_ascii_lowercase()
                    || character.is_ascii_digit()
                    || "._:-".contains(character)
            });
            if !valid_length || !valid_characters {
                return Err(AttributionPersistenceError::invalid_payload(
                    format!("{field} must contain 3-120 canonical characters"),
                    vec![AttributionValidationIssue {
                        field,
                        code: AttributionReasonCode::InvalidPayload.code(),
                        message: format!("{field} must contain 3-120 canonical characters"),
                    }],
                ));
            }
            Ok(Some(normalized))
        }
        None => Ok(None),
    }
}

fn map_contract_error(error: AttributionContractError) -> AttributionPersistenceError {
    AttributionPersistenceError {
        code: error.code,
        message: error.message,
        field_errors: error.field_errors,
    }
}

fn classify_query_error(
    operation: &'static str,
    error: sqlx::Error,
) -> AttributionPersistenceError {
    if is_constraint_error(&error) {
        return AttributionPersistenceError::constraint_violation(operation, error);
    }
    AttributionPersistenceError::query_failure(operation, error)
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

    const ATTRIBUTION_MIGRATION_SQL: &str =
        include_str!("../../migrations/20260406154000_attribution_snapshots.sql");

    fn sample_row() -> AttributionRow {
        AttributionRow {
            market_id: "market-btc-election".to_string(),
            alpha_id: "alpha-momentum".to_string(),
            period: AttributionPeriod::TwentyFourHours.as_str().to_string(),
            period_start_utc: "2026-04-05T15:00:00Z".to_string(),
            period_end_utc: "2026-04-06T15:00:00Z".to_string(),
            realized_pnl_usd: 120.0,
            unrealized_pnl_usd: 30.0,
            gross_pnl_usd: 150.0,
            net_pnl_usd: 144.0,
            costs: domain::attribution::AttributionCostBreakdown {
                fees_usd: 8.0,
                rebates_usd: 2.0,
                incentives_usd: 0.0,
                net_cost_impact_usd: 6.0,
            },
            as_of_utc: "2026-04-06T15:00:00Z".to_string(),
            source: "portfolio-engine.attribution.v1".to_string(),
            reason_code: AttributionReasonCode::Ready.code().to_string(),
            correlation_id: "corr-attribution-001".to_string(),
            snapshot_id: Some("snapshot::attribution::001".to_string()),
            run_id: Some("run::reconciliation::001".to_string()),
        }
    }

    #[test]
    fn migration_scope_remains_limited_to_attribution_snapshots() {
        assert!(
            ATTRIBUTION_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS attribution_snapshots")
        );
        assert!(!ATTRIBUTION_MIGRATION_SQL.contains("allocation_policies"));
        assert!(!ATTRIBUTION_MIGRATION_SQL.contains("risk_limit_profiles"));
    }

    #[test]
    fn migration_enforces_reason_codes_and_finite_numeric_constraints() {
        assert!(ATTRIBUTION_MIGRATION_SQL.contains("attribution_projection_unavailable"));
        assert!(ATTRIBUTION_MIGRATION_SQL.contains("attribution_stale_source"));
        assert!(ATTRIBUTION_MIGRATION_SQL.contains("isfinite(realized_pnl_usd)"));
        assert!(ATTRIBUTION_MIGRATION_SQL.contains("isfinite(net_pnl_usd)"));
        assert!(
            ATTRIBUTION_MIGRATION_SQL
                .contains("abs(gross_pnl_usd - (realized_pnl_usd + unrealized_pnl_usd)) <= 1e-9")
        );
        assert!(ATTRIBUTION_MIGRATION_SQL.contains(
            "abs(net_pnl_usd - (gross_pnl_usd - (fees_usd - rebates_usd - incentives_usd))) <= 1e-9"
        ));
        assert!(ATTRIBUTION_MIGRATION_SQL.contains("idx_attribution_snapshots_latest_scope"));
    }

    #[test]
    fn latest_query_ordering_is_deterministic_and_stable() {
        assert!(LOAD_LATEST_ATTRIBUTION_SNAPSHOTS_SQL.contains(
            "ORDER BY period_end_utc DESC, market_id ASC, alpha_id ASC, snapshot_id ASC"
        ));
    }

    #[test]
    fn row_validation_rejects_non_finite_cost_fields() {
        let mut row = sample_row();
        row.costs.fees_usd = f64::NAN;

        let error =
            validate_attribution_row(&row).expect_err("non-finite fees should fail validation");
        assert_eq!(error.code, AttributionReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "fees_usd")
        );
    }

    #[test]
    fn snapshot_id_fallback_normalizes_deterministically() {
        let mut row = sample_row();
        row.snapshot_id = None;
        let snapshot_id = row
            .snapshot_id
            .as_deref()
            .map(normalize_attribution_identifier)
            .unwrap_or_else(|| {
                normalize_attribution_identifier(&format!(
                    "attr::{}::{}::{}::{}",
                    row.period, row.market_id, row.alpha_id, row.correlation_id
                ))
            });
        assert_eq!(
            snapshot_id,
            "attr::24h::market-btc-election::alpha-momentum::corr-attribution-001"
        );
    }
}
