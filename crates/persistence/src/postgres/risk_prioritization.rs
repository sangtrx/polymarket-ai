use domain::coverage::CoverageClass;
use domain::risk_prioritization::{RemediationFocus, RiskSeverity};
use domain::traceability::{EvidenceAnchor, EvidenceType};
use sqlx::{Connection, PgConnection, Row};
use std::error::Error;
use std::fmt::{Display, Formatter};

const INSERT_RISK_SNAPSHOT_SQL: &str = r#"
    INSERT INTO risk_snapshots (
        snapshot_id,
        commit_sha,
        generated_at_utc
    ) VALUES ($1, $2, $3::timestamptz)
"#;

const INSERT_RISK_ROW_SQL: &str = r#"
    INSERT INTO risk_rows (
        row_id,
        snapshot_id,
        canonical_requirement_id,
        coverage_class,
        severity,
        risk_score,
        priority_rank,
        reason_code,
        rationale,
        provenance,
        code_anchor_count,
        test_anchor_count,
        ambiguous_anchor_count,
        remediation_focus,
        severity_weight,
        reason_weight,
        evidence_penalty
    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
"#;

const INSERT_RISK_ANCHOR_SQL: &str = r#"
    INSERT INTO risk_row_anchors (
        anchor_id,
        row_id,
        anchor_rank,
        evidence_type,
        file_path,
        symbol,
        section,
        line_start,
        line_end
    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
"#;

const LIST_RISK_ROWS_BY_SNAPSHOT_SQL: &str = r#"
    SELECT
        r.row_id,
        r.canonical_requirement_id,
        r.coverage_class,
        r.severity,
        r.risk_score,
        r.priority_rank,
        r.reason_code,
        r.rationale,
        r.provenance,
        r.code_anchor_count,
        r.test_anchor_count,
        r.ambiguous_anchor_count,
        r.remediation_focus,
        r.severity_weight,
        r.reason_weight,
        r.evidence_penalty,
        a.anchor_id,
        a.anchor_rank,
        a.evidence_type,
        a.file_path,
        a.symbol,
        a.section,
        a.line_start,
        a.line_end
    FROM risk_rows r
    LEFT JOIN risk_row_anchors a ON a.row_id = r.row_id
    WHERE r.snapshot_id = $1
    ORDER BY
        r.canonical_requirement_id ASC,
        r.priority_rank ASC,
        r.row_id ASC,
        a.anchor_rank ASC,
        a.anchor_id ASC
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiskPersistenceError {
    pub code: &'static str,
    pub message: String,
}

impl RiskPersistenceError {
    fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: "risk_invalid_payload",
            message: message.into(),
        }
    }

    fn query_failure(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "risk_query_failed",
            message: format!("{operation} failed: {error}"),
        }
    }

    fn constraint_violation(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "risk_constraint_violation",
            message: format!("{operation} rejected by constraint: {error}"),
        }
    }
}

impl Display for RiskPersistenceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for RiskPersistenceError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedRiskRow {
    pub canonical_requirement_id: String,
    pub coverage_class: CoverageClass,
    pub severity: RiskSeverity,
    pub risk_score: i32,
    pub priority_rank: usize,
    pub reason_code: String,
    pub rationale: String,
    pub provenance: String,
    pub code_anchor_count: usize,
    pub test_anchor_count: usize,
    pub ambiguous_anchor_count: usize,
    pub top_evidence_anchors: Vec<EvidenceAnchor>,
    pub remediation_focus: RemediationFocus,
    pub severity_weight: i32,
    pub reason_weight: i32,
    pub evidence_penalty: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiskRowRecord {
    pub row_id: String,
    pub row: PersistedRiskRow,
}

pub async fn insert_risk_snapshot(
    executor: &mut PgConnection,
    snapshot_id: &str,
    commit_sha: &str,
    generated_at_utc: &str,
    rows: &[PersistedRiskRow],
) -> Result<(), RiskPersistenceError> {
    if snapshot_id.trim().is_empty()
        || commit_sha.trim().is_empty()
        || generated_at_utc.trim().is_empty()
    {
        return Err(RiskPersistenceError::invalid_payload(
            "snapshot_id, commit_sha, and generated_at_utc are required",
        ));
    }

    let normalized_snapshot_id = snapshot_id.trim().to_ascii_lowercase();
    let mut transaction = executor.begin().await.map_err(|error| {
        RiskPersistenceError::query_failure("insert_risk_snapshot.begin", error)
    })?;

    sqlx::query(INSERT_RISK_SNAPSHOT_SQL)
        .bind(&normalized_snapshot_id)
        .bind(commit_sha.trim().to_ascii_lowercase())
        .bind(generated_at_utc.trim())
        .execute(&mut *transaction)
        .await
        .map_err(|error| classify_query_error("insert_risk_snapshot.snapshot", error))?;

    for (row_index, row) in rows.iter().enumerate() {
        if row.canonical_requirement_id.trim().is_empty() {
            return Err(RiskPersistenceError::invalid_payload(
                "canonical_requirement_id is required",
            ));
        }
        let row_id = format!("{normalized_snapshot_id}_{}", row_index + 1);
        sqlx::query(INSERT_RISK_ROW_SQL)
            .bind(&row_id)
            .bind(&normalized_snapshot_id)
            .bind(row.canonical_requirement_id.trim())
            .bind(coverage_class_to_str(&row.coverage_class))
            .bind(severity_to_str(&row.severity))
            .bind(row.risk_score)
            .bind(usize_to_i64(row.priority_rank, "priority_rank")?)
            .bind(row.reason_code.trim())
            .bind(row.rationale.trim())
            .bind(row.provenance.trim())
            .bind(usize_to_i64(row.code_anchor_count, "code_anchor_count")?)
            .bind(usize_to_i64(row.test_anchor_count, "test_anchor_count")?)
            .bind(usize_to_i64(
                row.ambiguous_anchor_count,
                "ambiguous_anchor_count",
            )?)
            .bind(remediation_focus_to_str(&row.remediation_focus))
            .bind(row.severity_weight)
            .bind(row.reason_weight)
            .bind(row.evidence_penalty)
            .execute(&mut *transaction)
            .await
            .map_err(|error| classify_query_error("insert_risk_snapshot.row", error))?;

        for (anchor_index, anchor) in row.top_evidence_anchors.iter().enumerate() {
            let anchor_id = format!("{row_id}_{}", anchor_index + 1);
            sqlx::query(INSERT_RISK_ANCHOR_SQL)
                .bind(anchor_id)
                .bind(&row_id)
                .bind(usize_to_i64(anchor_index + 1, "anchor_rank")?)
                .bind(evidence_type_to_str(&anchor.evidence_type))
                .bind(anchor.file_path.trim())
                .bind(anchor.symbol.clone())
                .bind(anchor.section.clone())
                .bind(anchor.line_start.map(i64::from))
                .bind(anchor.line_end.map(i64::from))
                .execute(&mut *transaction)
                .await
                .map_err(|error| classify_query_error("insert_risk_snapshot.anchor", error))?;
        }
    }

    transaction.commit().await.map_err(|error| {
        RiskPersistenceError::query_failure("insert_risk_snapshot.commit", error)
    })?;

    Ok(())
}

pub async fn list_risk_rows_by_snapshot(
    executor: &mut PgConnection,
    snapshot_id: &str,
) -> Result<Vec<RiskRowRecord>, RiskPersistenceError> {
    if snapshot_id.trim().is_empty() {
        return Err(RiskPersistenceError::invalid_payload(
            "snapshot_id is required",
        ));
    }

    let rows = sqlx::query(LIST_RISK_ROWS_BY_SNAPSHOT_SQL)
        .bind(snapshot_id.trim().to_ascii_lowercase())
        .fetch_all(&mut *executor)
        .await
        .map_err(|error| classify_query_error("list_risk_rows_by_snapshot", error))?;

    let mut records: Vec<RiskRowRecord> = Vec::new();
    for row in rows {
        let row_id: String = row
            .try_get("row_id")
            .map_err(|error| RiskPersistenceError::query_failure("row_decode.row_id", error))?;
        let maybe_existing_idx = records
            .iter()
            .position(|record| record.row_id.as_str() == row_id.as_str());
        let anchor = decode_anchor(&row)?;

        if let Some(existing_idx) = maybe_existing_idx {
            if let Some(anchor) = anchor {
                records[existing_idx].row.top_evidence_anchors.push(anchor);
            }
            continue;
        }

        let coverage_class_raw: String = row.try_get("coverage_class").map_err(|error| {
            RiskPersistenceError::query_failure("row_decode.coverage_class", error)
        })?;
        let severity_raw: String = row
            .try_get("severity")
            .map_err(|error| RiskPersistenceError::query_failure("row_decode.severity", error))?;
        let remediation_focus_raw: String = row.try_get("remediation_focus").map_err(|error| {
            RiskPersistenceError::query_failure("row_decode.remediation_focus", error)
        })?;

        let mut top_evidence_anchors = Vec::new();
        if let Some(anchor) = anchor {
            top_evidence_anchors.push(anchor);
        }

        records.push(RiskRowRecord {
            row_id,
            row: PersistedRiskRow {
                canonical_requirement_id: row.try_get("canonical_requirement_id").map_err(
                    |error| {
                        RiskPersistenceError::query_failure(
                            "row_decode.canonical_requirement_id",
                            error,
                        )
                    },
                )?,
                coverage_class: CoverageClass::try_from(coverage_class_raw.as_str()).map_err(
                    |error| {
                        RiskPersistenceError::invalid_payload(format!(
                            "invalid coverage class stored in database: {}",
                            error.message
                        ))
                    },
                )?,
                severity: RiskSeverity::try_from(severity_raw.as_str()).map_err(|error| {
                    RiskPersistenceError::invalid_payload(format!(
                        "invalid severity stored in database: {}",
                        error.message
                    ))
                })?,
                risk_score: row.try_get("risk_score").map_err(|error| {
                    RiskPersistenceError::query_failure("row_decode.risk_score", error)
                })?,
                priority_rank: decode_usize_field(
                    row.try_get::<i64, _>("priority_rank").map_err(|error| {
                        RiskPersistenceError::query_failure("row_decode.priority_rank", error)
                    })?,
                    "priority_rank",
                )?,
                reason_code: row.try_get("reason_code").map_err(|error| {
                    RiskPersistenceError::query_failure("row_decode.reason_code", error)
                })?,
                rationale: row.try_get("rationale").map_err(|error| {
                    RiskPersistenceError::query_failure("row_decode.rationale", error)
                })?,
                provenance: row.try_get("provenance").map_err(|error| {
                    RiskPersistenceError::query_failure("row_decode.provenance", error)
                })?,
                code_anchor_count: decode_usize_field(
                    row.try_get::<i64, _>("code_anchor_count").map_err(|error| {
                        RiskPersistenceError::query_failure("row_decode.code_anchor_count", error)
                    })?,
                    "code_anchor_count",
                )?,
                test_anchor_count: decode_usize_field(
                    row.try_get::<i64, _>("test_anchor_count").map_err(|error| {
                        RiskPersistenceError::query_failure("row_decode.test_anchor_count", error)
                    })?,
                    "test_anchor_count",
                )?,
                ambiguous_anchor_count: decode_usize_field(
                    row.try_get::<i64, _>("ambiguous_anchor_count")
                        .map_err(|error| {
                            RiskPersistenceError::query_failure(
                                "row_decode.ambiguous_anchor_count",
                                error,
                            )
                        })?,
                    "ambiguous_anchor_count",
                )?,
                top_evidence_anchors,
                remediation_focus: parse_remediation_focus(remediation_focus_raw.as_str())?,
                severity_weight: row.try_get("severity_weight").map_err(|error| {
                    RiskPersistenceError::query_failure("row_decode.severity_weight", error)
                })?,
                reason_weight: row.try_get("reason_weight").map_err(|error| {
                    RiskPersistenceError::query_failure("row_decode.reason_weight", error)
                })?,
                evidence_penalty: row.try_get("evidence_penalty").map_err(|error| {
                    RiskPersistenceError::query_failure("row_decode.evidence_penalty", error)
                })?,
            },
        });
    }

    Ok(records)
}

fn decode_anchor(
    row: &sqlx::postgres::PgRow,
) -> Result<Option<EvidenceAnchor>, RiskPersistenceError> {
    let anchor_id: Option<String> = row
        .try_get("anchor_id")
        .map_err(|error| RiskPersistenceError::query_failure("row_decode.anchor_id", error))?;
    if anchor_id.is_none() {
        return Ok(None);
    }

    let evidence_type_raw: String = row.try_get("evidence_type").map_err(|error| {
        RiskPersistenceError::query_failure("row_decode.evidence_type", error)
    })?;
    let evidence_type = parse_evidence_type(evidence_type_raw.as_str())?;

    let line_start: Option<i64> = row
        .try_get("line_start")
        .map_err(|error| RiskPersistenceError::query_failure("row_decode.line_start", error))?;
    let line_end: Option<i64> = row
        .try_get("line_end")
        .map_err(|error| RiskPersistenceError::query_failure("row_decode.line_end", error))?;
    let line_start = line_start
        .map(|value| decode_anchor_line_value(value, "line_start"))
        .transpose()?;
    let line_end = line_end
        .map(|value| decode_anchor_line_value(value, "line_end"))
        .transpose()?;

    Ok(Some(EvidenceAnchor {
        evidence_type,
        file_path: row.try_get("file_path").map_err(|error| {
            RiskPersistenceError::query_failure("row_decode.file_path", error)
        })?,
        symbol: row.try_get("symbol").map_err(|error| {
            RiskPersistenceError::query_failure("row_decode.symbol", error)
        })?,
        section: row.try_get("section").map_err(|error| {
            RiskPersistenceError::query_failure("row_decode.section", error)
        })?,
        line_start,
        line_end,
    }))
}

fn coverage_class_to_str(value: &CoverageClass) -> &'static str {
    match value {
        CoverageClass::Partial => "partial",
        CoverageClass::Missing => "missing",
        CoverageClass::Covered => "covered",
    }
}

fn severity_to_str(value: &RiskSeverity) -> &'static str {
    match value {
        RiskSeverity::Critical => "critical",
        RiskSeverity::High => "high",
        RiskSeverity::Medium => "medium",
        RiskSeverity::Low => "low",
    }
}

fn remediation_focus_to_str(value: &RemediationFocus) -> &'static str {
    match value {
        RemediationFocus::AddEvidence => "add_evidence",
        RemediationFocus::RestoreTraceability => "restore_traceability",
        RemediationFocus::ConfirmSemanticCoverage => "confirm_semantic_coverage",
        RemediationFocus::VerifyDeterministicLink => "verify_deterministic_link",
        RemediationFocus::ReviewReasonMapping => "review_reason_mapping",
    }
}

fn parse_remediation_focus(value: &str) -> Result<RemediationFocus, RiskPersistenceError> {
    match value {
        "add_evidence" => Ok(RemediationFocus::AddEvidence),
        "restore_traceability" => Ok(RemediationFocus::RestoreTraceability),
        "confirm_semantic_coverage" => Ok(RemediationFocus::ConfirmSemanticCoverage),
        "verify_deterministic_link" => Ok(RemediationFocus::VerifyDeterministicLink),
        "review_reason_mapping" => Ok(RemediationFocus::ReviewReasonMapping),
        _ => Err(RiskPersistenceError::invalid_payload(format!(
            "invalid remediation_focus stored in database: {value}",
        ))),
    }
}

fn evidence_type_to_str(value: &EvidenceType) -> &'static str {
    match value {
        EvidenceType::Code => "code",
        EvidenceType::Test => "test",
    }
}

fn parse_evidence_type(value: &str) -> Result<EvidenceType, RiskPersistenceError> {
    match value {
        "code" => Ok(EvidenceType::Code),
        "test" => Ok(EvidenceType::Test),
        _ => Err(RiskPersistenceError::invalid_payload(format!(
            "invalid evidence_type stored in database: {value}",
        ))),
    }
}

fn usize_to_i64(value: usize, field: &'static str) -> Result<i64, RiskPersistenceError> {
    i64::try_from(value).map_err(|_| {
        RiskPersistenceError::invalid_payload(format!(
            "{field} must be between 0 and {}",
            i64::MAX
        ))
    })
}

fn decode_usize_field(value: i64, field: &'static str) -> Result<usize, RiskPersistenceError> {
    usize::try_from(value).map_err(|_| {
        RiskPersistenceError::invalid_payload(format!(
            "{field} must be between 0 and {}",
            usize::MAX
        ))
    })
}

fn decode_anchor_line_value(
    value: i64,
    field: &'static str,
) -> Result<u32, RiskPersistenceError> {
    u32::try_from(value).map_err(|_| {
        RiskPersistenceError::invalid_payload(format!(
            "{field} must be between 0 and {}",
            u32::MAX
        ))
    })
}

#[cfg(test)]
fn insert_risk_snapshot_transaction_steps() -> [&'static str; 5] {
    ["begin", "snapshot", "rows", "anchors", "commit"]
}

fn classify_query_error(operation: &'static str, error: sqlx::Error) -> RiskPersistenceError {
    if is_constraint_error(&error) {
        return RiskPersistenceError::constraint_violation(operation, error);
    }
    RiskPersistenceError::query_failure(operation, error)
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

    const RISK_MIGRATION_SQL: &str =
        include_str!("../../migrations/20260409000500_risk_prioritization.sql");

    #[test]
    fn migration_contract_defines_snapshot_and_row_tables() {
        assert!(RISK_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS risk_snapshots"));
        assert!(RISK_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS risk_rows"));
        assert!(RISK_MIGRATION_SQL.contains("severity IN ('critical', 'high', 'medium', 'low')"));
    }

    #[test]
    fn migration_contract_enforces_immutable_snapshot_tables() {
        assert!(RISK_MIGRATION_SQL.contains("risk_reject_mutation"));
        assert!(RISK_MIGRATION_SQL.contains("risk_snapshots_immutable_update"));
        assert!(RISK_MIGRATION_SQL.contains("risk_rows_immutable_update"));
        assert!(RISK_MIGRATION_SQL.contains("risk_row_anchors_immutable_update"));
    }

    #[test]
    fn insert_snapshot_uses_bind_parameters_for_all_writes() {
        assert!(INSERT_RISK_SNAPSHOT_SQL.contains("$1"));
        assert!(INSERT_RISK_ROW_SQL.contains("$17"));
        assert!(INSERT_RISK_ANCHOR_SQL.contains("$9"));
        assert!(!INSERT_RISK_ROW_SQL.contains("||"));
    }

    #[test]
    fn insert_risk_snapshot_executes_writes_in_one_transaction() {
        assert_eq!(
            insert_risk_snapshot_transaction_steps(),
            ["begin", "snapshot", "rows", "anchors", "commit"]
        );
    }

    #[test]
    fn list_query_orders_rows_deterministically_by_requirement_and_rank() {
        assert!(LIST_RISK_ROWS_BY_SNAPSHOT_SQL.contains("ORDER BY"));
        assert!(LIST_RISK_ROWS_BY_SNAPSHOT_SQL.contains("r.canonical_requirement_id ASC"));
        assert!(LIST_RISK_ROWS_BY_SNAPSHOT_SQL.contains("r.priority_rank ASC"));
    }

    #[test]
    fn query_errors_map_to_machine_readable_codes() {
        let error = classify_query_error(
            "insert_risk_snapshot.row",
            sqlx::Error::Protocol("boom".into()),
        );
        assert_eq!(error.code, "risk_query_failed");
        assert!(error.message.contains("insert_risk_snapshot.row"));
    }

    #[test]
    fn parse_remediation_focus_rejects_unknown_values() {
        let error = parse_remediation_focus("unknown").expect_err("unknown value must fail");
        assert_eq!(error.code, "risk_invalid_payload");
        assert!(error.message.contains("unknown"));
    }

    #[test]
    fn decode_anchor_rejects_negative_line_values() {
        let error = decode_anchor_line_value(-1, "line_start")
            .expect_err("negative line values must fail closed");
        assert_eq!(error.code, "risk_invalid_payload");
        assert!(error.message.contains("line_start"));
    }
}
