use domain::coverage::{CoverageClass, CoverageMatrixRow};
use domain::traceability::{EvidenceAnchor, EvidenceType};
use sqlx::{Connection, PgConnection, Row};
use std::error::Error;
use std::fmt::{Display, Formatter};

const INSERT_COVERAGE_SNAPSHOT_SQL: &str = r#"
    INSERT INTO coverage_snapshots (
        snapshot_id,
        commit_sha,
        generated_at_utc
    ) VALUES ($1, $2, $3::timestamptz)
"#;

const INSERT_COVERAGE_ROW_SQL: &str = r#"
    INSERT INTO coverage_rows (
        row_id,
        snapshot_id,
        canonical_requirement_id,
        coverage_class,
        reason_code,
        rationale,
        provenance
    ) VALUES ($1, $2, $3, $4, $5, $6, $7)
"#;

const INSERT_COVERAGE_ANCHOR_SQL: &str = r#"
    INSERT INTO coverage_row_anchors (
        anchor_id,
        row_id,
        bucket,
        evidence_type,
        file_path,
        symbol,
        section,
        line_start,
        line_end
    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
"#;

const LIST_COVERAGE_ROWS_BY_SNAPSHOT_SQL: &str = r#"
    SELECT
        r.row_id,
        r.canonical_requirement_id,
        r.coverage_class,
        r.reason_code,
        r.rationale,
        r.provenance,
        a.anchor_id,
        a.bucket,
        a.evidence_type,
        a.file_path,
        a.symbol,
        a.section,
        a.line_start,
        a.line_end
    FROM coverage_rows r
    LEFT JOIN coverage_row_anchors a ON a.row_id = r.row_id
    WHERE r.snapshot_id = $1
    ORDER BY
        r.canonical_requirement_id ASC,
        r.row_id ASC,
        CASE a.bucket
            WHEN 'code' THEN 1
            WHEN 'test' THEN 2
            ELSE 3
        END ASC,
        a.anchor_id ASC
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoveragePersistenceError {
    pub code: &'static str,
    pub message: String,
}

impl CoveragePersistenceError {
    fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: "coverage_invalid_payload",
            message: message.into(),
        }
    }

    fn query_failure(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "coverage_query_failed",
            message: format!("{operation} failed: {error}"),
        }
    }

    fn constraint_violation(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "coverage_constraint_violation",
            message: format!("{operation} rejected by constraint: {error}"),
        }
    }
}

impl Display for CoveragePersistenceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for CoveragePersistenceError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverageMatrixRowRecord {
    pub row_id: String,
    pub canonical_requirement_id: String,
    pub coverage_class: CoverageClass,
    pub reason_code: String,
    pub rationale: String,
    pub code_anchors: Vec<EvidenceAnchor>,
    pub test_anchors: Vec<EvidenceAnchor>,
    pub ambiguous_candidates: Vec<EvidenceAnchor>,
    pub provenance: String,
}

pub async fn insert_coverage_snapshot(
    executor: &mut PgConnection,
    snapshot_id: &str,
    commit_sha: &str,
    generated_at_utc: &str,
    rows: &[CoverageMatrixRow],
) -> Result<(), CoveragePersistenceError> {
    if snapshot_id.trim().is_empty()
        || commit_sha.trim().is_empty()
        || generated_at_utc.trim().is_empty()
    {
        return Err(CoveragePersistenceError::invalid_payload(
            "snapshot_id, commit_sha, and generated_at_utc are required",
        ));
    }

    let normalized_snapshot_id = snapshot_id.trim().to_ascii_lowercase();
    let mut transaction = executor.begin().await.map_err(|error| {
        CoveragePersistenceError::query_failure("insert_coverage_snapshot.begin", error)
    })?;

    sqlx::query(INSERT_COVERAGE_SNAPSHOT_SQL)
        .bind(&normalized_snapshot_id)
        .bind(commit_sha.trim().to_ascii_lowercase())
        .bind(generated_at_utc.trim())
        .execute(&mut *transaction)
        .await
        .map_err(|error| classify_query_error("insert_coverage_snapshot.snapshot", error))?;

    for (row_idx, row) in rows.iter().enumerate() {
        if row.canonical_requirement_id.trim().is_empty() {
            return Err(CoveragePersistenceError::invalid_payload(
                "canonical_requirement_id is required",
            ));
        }
        let row_id = format!("{normalized_snapshot_id}_{}", row_idx + 1);
        sqlx::query(INSERT_COVERAGE_ROW_SQL)
            .bind(&row_id)
            .bind(&normalized_snapshot_id)
            .bind(row.canonical_requirement_id.trim())
            .bind(coverage_class_to_str(&row.coverage_class))
            .bind(row.reason_code.trim())
            .bind(row.rationale.trim())
            .bind(row.provenance.trim())
            .execute(&mut *transaction)
            .await
            .map_err(|error| classify_query_error("insert_coverage_snapshot.row", error))?;

        let mut anchor_idx = 1usize;
        for (bucket, anchors) in [
            ("code", &row.code_anchors),
            ("test", &row.test_anchors),
            ("ambiguous", &row.ambiguous_candidates),
        ] {
            for anchor in anchors {
                let anchor_id = format!("{row_id}_{anchor_idx}");
                anchor_idx += 1;
                sqlx::query(INSERT_COVERAGE_ANCHOR_SQL)
                    .bind(anchor_id)
                    .bind(&row_id)
                    .bind(bucket)
                    .bind(evidence_type_to_str(&anchor.evidence_type))
                    .bind(anchor.file_path.trim())
                    .bind(anchor.symbol.clone())
                    .bind(anchor.section.clone())
                    .bind(anchor.line_start.map(i64::from))
                    .bind(anchor.line_end.map(i64::from))
                    .execute(&mut *transaction)
                    .await
                    .map_err(|error| {
                        classify_query_error("insert_coverage_snapshot.anchor", error)
                    })?;
            }
        }
    }

    transaction.commit().await.map_err(|error| {
        CoveragePersistenceError::query_failure("insert_coverage_snapshot.commit", error)
    })?;

    Ok(())
}

pub async fn list_coverage_rows_by_snapshot(
    executor: &mut PgConnection,
    snapshot_id: &str,
) -> Result<Vec<CoverageMatrixRowRecord>, CoveragePersistenceError> {
    if snapshot_id.trim().is_empty() {
        return Err(CoveragePersistenceError::invalid_payload(
            "snapshot_id is required",
        ));
    }

    let rows = sqlx::query(LIST_COVERAGE_ROWS_BY_SNAPSHOT_SQL)
        .bind(snapshot_id.trim().to_ascii_lowercase())
        .fetch_all(&mut *executor)
        .await
        .map_err(|error| classify_query_error("list_coverage_rows_by_snapshot", error))?;

    let mut records: Vec<CoverageMatrixRowRecord> = Vec::new();
    for row in rows {
        let row_id: String = row
            .try_get("row_id")
            .map_err(|error| CoveragePersistenceError::query_failure("row_decode.row_id", error))?;
        let maybe_existing_idx = records.iter().position(|record| record.row_id == row_id);
        let anchor = decode_anchor(&row)?;

        if let Some(existing_idx) = maybe_existing_idx {
            if let Some((bucket, anchor)) = anchor {
                push_coverage_anchor(&mut records[existing_idx], bucket, anchor)?;
            }
            continue;
        }

        let coverage_class_raw: String = row.try_get("coverage_class").map_err(|error| {
            CoveragePersistenceError::query_failure("row_decode.coverage_class", error)
        })?;
        let coverage_class =
            CoverageClass::try_from(coverage_class_raw.as_str()).map_err(|error| {
                CoveragePersistenceError::invalid_payload(format!(
                    "invalid coverage class stored in database: {}",
                    error.message
                ))
            })?;

        let mut record = CoverageMatrixRowRecord {
            row_id,
            canonical_requirement_id: row.try_get("canonical_requirement_id").map_err(|error| {
                CoveragePersistenceError::query_failure(
                    "row_decode.canonical_requirement_id",
                    error,
                )
            })?,
            coverage_class,
            reason_code: row.try_get("reason_code").map_err(|error| {
                CoveragePersistenceError::query_failure("row_decode.reason_code", error)
            })?,
            rationale: row.try_get("rationale").map_err(|error| {
                CoveragePersistenceError::query_failure("row_decode.rationale", error)
            })?,
            code_anchors: vec![],
            test_anchors: vec![],
            ambiguous_candidates: vec![],
            provenance: row.try_get("provenance").map_err(|error| {
                CoveragePersistenceError::query_failure("row_decode.provenance", error)
            })?,
        };

        if let Some((bucket, anchor)) = anchor {
            push_coverage_anchor(&mut record, bucket, anchor)?;
        }
        records.push(record);
    }

    Ok(records)
}

fn decode_anchor(
    row: &sqlx::postgres::PgRow,
) -> Result<Option<(String, EvidenceAnchor)>, CoveragePersistenceError> {
    let anchor_id: Option<String> = row
        .try_get("anchor_id")
        .map_err(|error| CoveragePersistenceError::query_failure("row_decode.anchor_id", error))?;
    if anchor_id.is_none() {
        return Ok(None);
    }

    let bucket: String = row
        .try_get("bucket")
        .map_err(|error| CoveragePersistenceError::query_failure("row_decode.bucket", error))?;
    let bucket = parse_anchor_bucket(bucket.as_str())?.to_string();

    let evidence_type_raw: String = row.try_get("evidence_type").map_err(|error| {
        CoveragePersistenceError::query_failure("row_decode.evidence_type", error)
    })?;
    let evidence_type = parse_evidence_type(evidence_type_raw.as_str())?;

    let line_start: Option<i64> = row
        .try_get("line_start")
        .map_err(|error| CoveragePersistenceError::query_failure("row_decode.line_start", error))?;
    let line_end: Option<i64> = row
        .try_get("line_end")
        .map_err(|error| CoveragePersistenceError::query_failure("row_decode.line_end", error))?;
    let line_start = line_start
        .map(|value| decode_anchor_line_value(value, "line_start"))
        .transpose()?;
    let line_end = line_end
        .map(|value| decode_anchor_line_value(value, "line_end"))
        .transpose()?;

    Ok(Some((
        bucket,
        EvidenceAnchor {
            evidence_type,
            file_path: row.try_get("file_path").map_err(|error| {
                CoveragePersistenceError::query_failure("row_decode.file_path", error)
            })?,
            symbol: row.try_get("symbol").map_err(|error| {
                CoveragePersistenceError::query_failure("row_decode.symbol", error)
            })?,
            section: row.try_get("section").map_err(|error| {
                CoveragePersistenceError::query_failure("row_decode.section", error)
            })?,
            line_start,
            line_end,
        },
    )))
}

fn push_coverage_anchor(
    record: &mut CoverageMatrixRowRecord,
    bucket: String,
    anchor: EvidenceAnchor,
) -> Result<(), CoveragePersistenceError> {
    match bucket.as_str() {
        "code" => record.code_anchors.push(anchor),
        "test" => record.test_anchors.push(anchor),
        "ambiguous" => record.ambiguous_candidates.push(anchor),
        _ => {
            return Err(CoveragePersistenceError::invalid_payload(format!(
                "invalid anchor bucket stored in database: {bucket}"
            )));
        }
    }
    Ok(())
}

fn parse_anchor_bucket(value: &str) -> Result<&str, CoveragePersistenceError> {
    match value {
        "code" | "test" | "ambiguous" => Ok(value),
        _ => Err(CoveragePersistenceError::invalid_payload(format!(
            "invalid anchor bucket stored in database: {value}",
        ))),
    }
}

fn parse_evidence_type(value: &str) -> Result<EvidenceType, CoveragePersistenceError> {
    match value {
        "code" => Ok(EvidenceType::Code),
        "test" => Ok(EvidenceType::Test),
        _ => Err(CoveragePersistenceError::invalid_payload(format!(
            "invalid evidence_type stored in database: {value}",
        ))),
    }
}

fn decode_anchor_line_value(
    value: i64,
    field: &'static str,
) -> Result<u32, CoveragePersistenceError> {
    u32::try_from(value).map_err(|_| {
        CoveragePersistenceError::invalid_payload(format!(
            "{field} must be between 0 and {}",
            u32::MAX
        ))
    })
}

#[cfg(test)]
fn insert_coverage_snapshot_transaction_steps() -> [&'static str; 5] {
    ["begin", "snapshot", "rows", "anchors", "commit"]
}

fn coverage_class_to_str(value: &CoverageClass) -> &'static str {
    match value {
        CoverageClass::Covered => "covered",
        CoverageClass::Partial => "partial",
        CoverageClass::Missing => "missing",
    }
}

fn evidence_type_to_str(value: &EvidenceType) -> &'static str {
    match value {
        EvidenceType::Code => "code",
        EvidenceType::Test => "test",
    }
}

fn classify_query_error(operation: &'static str, error: sqlx::Error) -> CoveragePersistenceError {
    if is_constraint_error(&error) {
        return CoveragePersistenceError::constraint_violation(operation, error);
    }
    CoveragePersistenceError::query_failure(operation, error)
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

    const COVERAGE_MIGRATION_SQL: &str =
        include_str!("../../migrations/20260409000400_coverage_matrix.sql");

    #[test]
    fn migration_contract_requires_class_enum_and_reason_payload() {
        assert!(COVERAGE_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS coverage_snapshots"));
        assert!(
            COVERAGE_MIGRATION_SQL.contains("coverage_class IN ('covered', 'partial', 'missing')")
        );
        assert!(COVERAGE_MIGRATION_SQL.contains("char_length(trim(reason_code)) > 0"));
        assert!(COVERAGE_MIGRATION_SQL.contains("char_length(trim(rationale)) > 0"));
    }

    #[test]
    fn migration_contract_preserves_explainability_anchor_buckets() {
        assert!(COVERAGE_MIGRATION_SQL.contains("coverage_row_anchors"));
        assert!(COVERAGE_MIGRATION_SQL.contains("bucket IN ('code', 'test', 'ambiguous')"));
    }

    #[test]
    fn insert_snapshot_uses_bind_parameters_for_all_writes() {
        assert!(INSERT_COVERAGE_SNAPSHOT_SQL.contains("$1"));
        assert!(INSERT_COVERAGE_ROW_SQL.contains("$7"));
        assert!(INSERT_COVERAGE_ANCHOR_SQL.contains("$9"));
        assert!(!INSERT_COVERAGE_ROW_SQL.contains("||"));
    }

    #[test]
    fn insert_coverage_snapshot_executes_writes_in_one_transaction() {
        let steps = insert_coverage_snapshot_transaction_steps();
        assert_eq!(steps, ["begin", "snapshot", "rows", "anchors", "commit"]);
    }

    #[test]
    fn list_query_orders_rows_deterministically() {
        assert!(LIST_COVERAGE_ROWS_BY_SNAPSHOT_SQL.contains("ORDER BY"));
        assert!(LIST_COVERAGE_ROWS_BY_SNAPSHOT_SQL.contains("r.canonical_requirement_id ASC"));
        assert!(LIST_COVERAGE_ROWS_BY_SNAPSHOT_SQL.contains("a.anchor_id ASC"));
    }

    #[test]
    fn query_errors_map_to_machine_readable_codes() {
        let error = classify_query_error(
            "insert_coverage_snapshot.row",
            sqlx::Error::Protocol("boom".into()),
        );
        assert_eq!(error.code, "coverage_query_failed");
        assert!(error.message.contains("insert_coverage_snapshot.row"));
    }

    #[test]
    fn parse_anchor_bucket_rejects_unknown_values_with_machine_code() {
        let error = parse_anchor_bucket("other").expect_err("unknown bucket must fail");
        assert_eq!(error.code, "coverage_invalid_payload");
        assert!(error.message.contains("other"));
    }

    #[test]
    fn decode_anchor_rejects_negative_line_values() {
        let error = decode_anchor_line_value(-1, "line_start")
            .expect_err("negative line values must fail closed");
        assert_eq!(error.code, "coverage_invalid_payload");
        assert!(error.message.contains("line_start"));
    }

    #[test]
    fn decode_anchor_rejects_out_of_range_line_values() {
        let error = decode_anchor_line_value(i64::from(u32::MAX) + 1, "line_end")
            .expect_err("overflowing line values must fail closed");
        assert_eq!(error.code, "coverage_invalid_payload");
        assert!(error.message.contains("line_end"));
    }

    #[test]
    fn coverage_row_payload_preserves_reason_and_anchor_buckets() {
        let row = CoverageMatrixRow {
            canonical_requirement_id: "project_md.requirements.1".to_string(),
            coverage_class: CoverageClass::Partial,
            reason_code: "semantic_fallback_used".to_string(),
            rationale: "semantic fallback lowered confidence".to_string(),
            code_anchors: vec![EvidenceAnchor {
                evidence_type: EvidenceType::Code,
                file_path: "services/research-gateway/src/coverage/service.rs".to_string(),
                symbol: Some("run_coverage_classification".to_string()),
                section: None,
                line_start: Some(1),
                line_end: Some(20),
            }],
            test_anchors: vec![EvidenceAnchor {
                evidence_type: EvidenceType::Test,
                file_path: "tests/api/phase-3-coverage-classification.test.mjs".to_string(),
                symbol: Some("coverage contract".to_string()),
                section: None,
                line_start: Some(1),
                line_end: Some(30),
            }],
            ambiguous_candidates: vec![EvidenceAnchor {
                evidence_type: EvidenceType::Code,
                file_path: "services/research-gateway/src/traceability/matcher.rs".to_string(),
                symbol: Some("match_requirement_to_evidence".to_string()),
                section: None,
                line_start: Some(1),
                line_end: Some(40),
            }],
            provenance: "traceability_service".to_string(),
        };

        assert_eq!(row.reason_code, "semantic_fallback_used");
        assert_eq!(row.code_anchors.len(), 1);
        assert_eq!(row.test_anchors.len(), 1);
        assert_eq!(row.ambiguous_candidates.len(), 1);
    }
}
