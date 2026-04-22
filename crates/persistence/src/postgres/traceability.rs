use domain::traceability::{
    EvidenceAnchor, EvidenceType, LinkConfidence, LinkOutcome, TraceabilityLink,
};
use sqlx::{Connection, PgConnection, Row};
use std::error::Error;
use std::fmt::{Display, Formatter};

const INSERT_TRACEABILITY_SNAPSHOT_SQL: &str = r#"
    INSERT INTO traceability_snapshots (
        snapshot_id,
        commit_sha,
        generated_at_utc
    ) VALUES ($1, $2, $3::timestamptz)
"#;

const INSERT_TRACEABILITY_LINK_SQL: &str = r#"
    INSERT INTO traceability_links (
        link_id,
        snapshot_id,
        canonical_requirement_id,
        rationale,
        confidence,
        outcome,
        reason_code
    ) VALUES ($1, $2, $3, $4, $5, $6, $7)
"#;

const INSERT_TRACEABILITY_ANCHOR_SQL: &str = r#"
    INSERT INTO traceability_link_anchors (
        anchor_id,
        link_id,
        evidence_type,
        file_path,
        symbol,
        section,
        line_start,
        line_end
    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
"#;

const LIST_LINKS_BY_REQUIREMENT_SQL: &str = r#"
    SELECT
        l.link_id,
        l.canonical_requirement_id,
        l.rationale,
        l.confidence,
        l.outcome,
        l.reason_code,
        a.anchor_id,
        a.evidence_type,
        a.file_path,
        a.symbol,
        a.section,
        a.line_start,
        a.line_end
    FROM traceability_links l
    LEFT JOIN traceability_link_anchors a ON a.link_id = l.link_id
    WHERE l.snapshot_id = $1 AND l.canonical_requirement_id = $2
    ORDER BY
        l.canonical_requirement_id ASC,
        CASE l.confidence
            WHEN 'high' THEN 1
            WHEN 'medium' THEN 2
            ELSE 3
        END ASC,
        l.link_id ASC,
        a.anchor_id ASC
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceabilityPersistenceError {
    pub code: &'static str,
    pub message: String,
}

impl TraceabilityPersistenceError {
    fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: "traceability_invalid_payload",
            message: message.into(),
        }
    }

    fn query_failure(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "traceability_query_failed",
            message: format!("{operation} failed: {error}"),
        }
    }

    fn constraint_violation(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "traceability_constraint_violation",
            message: format!("{operation} rejected by constraint: {error}"),
        }
    }
}

impl Display for TraceabilityPersistenceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for TraceabilityPersistenceError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceabilityLinkRecord {
    pub link_id: String,
    pub canonical_requirement_id: String,
    pub rationale: String,
    pub confidence: LinkConfidence,
    pub outcome: LinkOutcome,
    pub reason_code: Option<String>,
    pub anchors: Vec<EvidenceAnchor>,
}

pub async fn insert_traceability_snapshot<'e, E>(
    executor: &mut PgConnection,
    snapshot_id: &str,
    commit_sha: &str,
    generated_at_utc: &str,
    links: &[TraceabilityLink],
) -> Result<(), TraceabilityPersistenceError> {
    if snapshot_id.trim().is_empty()
        || commit_sha.trim().is_empty()
        || generated_at_utc.trim().is_empty()
    {
        return Err(TraceabilityPersistenceError::invalid_payload(
            "snapshot_id, commit_sha, and generated_at_utc are required",
        ));
    }

    let mut transaction = executor.begin().await.map_err(|error| {
        TraceabilityPersistenceError::query_failure("insert_traceability_snapshot.begin", error)
    })?;

    sqlx::query(INSERT_TRACEABILITY_SNAPSHOT_SQL)
        .bind(snapshot_id.trim().to_ascii_lowercase())
        .bind(commit_sha.trim().to_ascii_lowercase())
        .bind(generated_at_utc.trim())
        .execute(&mut *transaction)
        .await
        .map_err(|error| classify_query_error("insert_traceability_snapshot.snapshot", error))?;

    for (link_idx, link) in links.iter().enumerate() {
        let link_id = format!(
            "{}_{}",
            snapshot_id.trim().to_ascii_lowercase(),
            link_idx + 1
        );
        sqlx::query(INSERT_TRACEABILITY_LINK_SQL)
            .bind(&link_id)
            .bind(snapshot_id.trim().to_ascii_lowercase())
            .bind(link.canonical_requirement_id.trim())
            .bind(link.rationale.trim())
            .bind(confidence_to_str(&link.confidence))
            .bind(outcome_to_str(&link.outcome))
            .bind(
                link.reason_code
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToString::to_string),
            )
            .execute(&mut *transaction)
            .await
            .map_err(|error| classify_query_error("insert_traceability_snapshot.link", error))?;

        for (anchor_idx, anchor) in link.anchors.iter().enumerate() {
            let anchor_id = format!("{link_id}_{}", anchor_idx + 1);
            sqlx::query(INSERT_TRACEABILITY_ANCHOR_SQL)
                .bind(anchor_id)
                .bind(&link_id)
                .bind(evidence_type_to_str(&anchor.evidence_type))
                .bind(anchor.file_path.trim())
                .bind(anchor.symbol.clone())
                .bind(anchor.section.clone())
                .bind(anchor.line_start.map(i64::from))
                .bind(anchor.line_end.map(i64::from))
                .execute(&mut *transaction)
                .await
                .map_err(|error| {
                    classify_query_error("insert_traceability_snapshot.anchor", error)
                })?;
        }
    }

    transaction.commit().await.map_err(|error| {
        TraceabilityPersistenceError::query_failure("insert_traceability_snapshot.commit", error)
    })?;

    Ok(())
}

pub async fn list_links_by_requirement(
    executor: &mut PgConnection,
    snapshot_id: &str,
    canonical_requirement_id: &str,
) -> Result<Vec<TraceabilityLinkRecord>, TraceabilityPersistenceError> {
    if snapshot_id.trim().is_empty() || canonical_requirement_id.trim().is_empty() {
        return Err(TraceabilityPersistenceError::invalid_payload(
            "snapshot_id and canonical_requirement_id are required",
        ));
    }

    let rows = sqlx::query(LIST_LINKS_BY_REQUIREMENT_SQL)
        .bind(snapshot_id.trim().to_ascii_lowercase())
        .bind(canonical_requirement_id.trim())
        .fetch_all(&mut *executor)
        .await
        .map_err(|error| classify_query_error("list_links_by_requirement", error))?;

    let mut records: Vec<TraceabilityLinkRecord> = Vec::new();
    for row in rows {
        let link_id: String = row.try_get("link_id").map_err(|error| {
            TraceabilityPersistenceError::query_failure("row_decode.link_id", error)
        })?;

        let maybe_existing_idx = records.iter().position(|record| record.link_id == link_id);
        let anchor = decode_anchor(&row)?;

        if let Some(existing_idx) = maybe_existing_idx {
            if let Some(anchor) = anchor {
                records[existing_idx].anchors.push(anchor);
            }
            continue;
        }

        let confidence_raw: String = row.try_get("confidence").map_err(|error| {
            TraceabilityPersistenceError::query_failure("row_decode.confidence", error)
        })?;
        let outcome_raw: String = row.try_get("outcome").map_err(|error| {
            TraceabilityPersistenceError::query_failure("row_decode.outcome", error)
        })?;

        let mut anchors = Vec::new();
        if let Some(anchor) = anchor {
            anchors.push(anchor);
        }

        records.push(TraceabilityLinkRecord {
            link_id,
            canonical_requirement_id: row.try_get("canonical_requirement_id").map_err(|error| {
                TraceabilityPersistenceError::query_failure(
                    "row_decode.canonical_requirement_id",
                    error,
                )
            })?,
            rationale: row.try_get("rationale").map_err(|error| {
                TraceabilityPersistenceError::query_failure("row_decode.rationale", error)
            })?,
            confidence: LinkConfidence::try_from(confidence_raw.as_str()).map_err(|error| {
                TraceabilityPersistenceError::invalid_payload(format!(
                    "invalid confidence stored in database: {}",
                    error.message
                ))
            })?,
            outcome: parse_outcome(outcome_raw.as_str())?,
            reason_code: row.try_get("reason_code").map_err(|error| {
                TraceabilityPersistenceError::query_failure("row_decode.reason_code", error)
            })?,
            anchors,
        });
    }

    Ok(records)
}

fn decode_anchor(
    row: &sqlx::postgres::PgRow,
) -> Result<Option<EvidenceAnchor>, TraceabilityPersistenceError> {
    let anchor_id: Option<String> = row.try_get("anchor_id").map_err(|error| {
        TraceabilityPersistenceError::query_failure("row_decode.anchor_id", error)
    })?;
    if anchor_id.is_none() {
        return Ok(None);
    }

    let evidence_type_raw: String = row.try_get("evidence_type").map_err(|error| {
        TraceabilityPersistenceError::query_failure("row_decode.evidence_type", error)
    })?;
    let evidence_type = parse_evidence_type(evidence_type_raw.as_str())?;
    let line_start: Option<i64> = row.try_get("line_start").map_err(|error| {
        TraceabilityPersistenceError::query_failure("row_decode.line_start", error)
    })?;
    let line_end: Option<i64> = row.try_get("line_end").map_err(|error| {
        TraceabilityPersistenceError::query_failure("row_decode.line_end", error)
    })?;
    let line_start = line_start
        .map(|value| decode_anchor_line_value(value, "line_start"))
        .transpose()?;
    let line_end = line_end
        .map(|value| decode_anchor_line_value(value, "line_end"))
        .transpose()?;

    Ok(Some(EvidenceAnchor {
        evidence_type,
        file_path: row.try_get("file_path").map_err(|error| {
            TraceabilityPersistenceError::query_failure("row_decode.file_path", error)
        })?,
        symbol: row.try_get("symbol").map_err(|error| {
            TraceabilityPersistenceError::query_failure("row_decode.symbol", error)
        })?,
        section: row.try_get("section").map_err(|error| {
            TraceabilityPersistenceError::query_failure("row_decode.section", error)
        })?,
        line_start,
        line_end,
    }))
}

fn parse_outcome(value: &str) -> Result<LinkOutcome, TraceabilityPersistenceError> {
    match value {
        "linked" => Ok(LinkOutcome::Linked),
        "ambiguous" => Ok(LinkOutcome::Ambiguous),
        "missing_evidence" => Ok(LinkOutcome::MissingEvidence),
        "stale_evidence" => Ok(LinkOutcome::StaleEvidence),
        _ => Err(TraceabilityPersistenceError::invalid_payload(format!(
            "invalid outcome stored in database: {value}",
        ))),
    }
}

fn parse_evidence_type(value: &str) -> Result<EvidenceType, TraceabilityPersistenceError> {
    match value {
        "code" => Ok(EvidenceType::Code),
        "test" => Ok(EvidenceType::Test),
        _ => Err(TraceabilityPersistenceError::invalid_payload(format!(
            "invalid evidence_type stored in database: {value}",
        ))),
    }
}

fn decode_anchor_line_value(
    value: i64,
    field: &'static str,
) -> Result<u32, TraceabilityPersistenceError> {
    u32::try_from(value).map_err(|_| {
        TraceabilityPersistenceError::invalid_payload(format!(
            "{field} must be between 0 and {}",
            u32::MAX
        ))
    })
}

#[cfg(test)]
fn insert_traceability_snapshot_transaction_steps() -> [&'static str; 5] {
    ["begin", "snapshot", "links", "anchors", "commit"]
}

fn confidence_to_str(confidence: &LinkConfidence) -> &'static str {
    match confidence {
        LinkConfidence::High => "high",
        LinkConfidence::Medium => "medium",
        LinkConfidence::Low => "low",
    }
}

fn outcome_to_str(outcome: &LinkOutcome) -> &'static str {
    match outcome {
        LinkOutcome::Linked => "linked",
        LinkOutcome::Ambiguous => "ambiguous",
        LinkOutcome::MissingEvidence => "missing_evidence",
        LinkOutcome::StaleEvidence => "stale_evidence",
    }
}

fn evidence_type_to_str(value: &EvidenceType) -> &'static str {
    match value {
        EvidenceType::Code => "code",
        EvidenceType::Test => "test",
    }
}

fn classify_query_error(
    operation: &'static str,
    error: sqlx::Error,
) -> TraceabilityPersistenceError {
    if is_constraint_error(&error) {
        return TraceabilityPersistenceError::constraint_violation(operation, error);
    }
    TraceabilityPersistenceError::query_failure(operation, error)
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
    use domain::traceability::{
        EvidenceAnchor, EvidenceType, LinkConfidence, LinkOutcome, TraceabilityLink,
    };

    const TRACEABILITY_MIGRATION_SQL: &str =
        include_str!("../../migrations/20260409000300_traceability_mapping.sql");

    #[test]
    fn migration_contract_requires_rationale_and_confidence_enum() {
        assert!(TRACEABILITY_MIGRATION_SQL.contains("traceability_links"));
        assert!(TRACEABILITY_MIGRATION_SQL.contains("char_length(trim(rationale)) > 0"));
        assert!(TRACEABILITY_MIGRATION_SQL.contains("confidence IN ('high', 'medium', 'low')"));
    }

    #[test]
    fn migration_contract_preserves_many_to_many_evidence_links() {
        assert!(TRACEABILITY_MIGRATION_SQL.contains("traceability_link_anchors"));
        assert!(TRACEABILITY_MIGRATION_SQL.contains("evidence_type"));
    }

    #[test]
    fn migration_contract_supports_missing_evidence_outcomes() {
        assert!(TRACEABILITY_MIGRATION_SQL.contains("missing_evidence"));
        assert!(TRACEABILITY_MIGRATION_SQL.contains("stale_evidence"));
    }

    #[test]
    fn insert_snapshot_uses_bind_parameters_for_all_writes() {
        assert!(INSERT_TRACEABILITY_SNAPSHOT_SQL.contains("$1"));
        assert!(INSERT_TRACEABILITY_LINK_SQL.contains("$7"));
        assert!(INSERT_TRACEABILITY_ANCHOR_SQL.contains("$8"));
        assert!(!INSERT_TRACEABILITY_LINK_SQL.contains("||"));
    }

    #[test]
    fn insert_traceability_snapshot_executes_writes_in_one_transaction() {
        let steps = insert_traceability_snapshot_transaction_steps();
        assert_eq!(steps, ["begin", "snapshot", "links", "anchors", "commit"]);
    }

    #[test]
    fn list_query_orders_records_deterministically() {
        assert!(LIST_LINKS_BY_REQUIREMENT_SQL.contains("ORDER BY"));
        assert!(LIST_LINKS_BY_REQUIREMENT_SQL.contains("l.link_id ASC"));
        assert!(LIST_LINKS_BY_REQUIREMENT_SQL.contains("a.anchor_id ASC"));
    }

    #[test]
    fn query_errors_map_to_machine_readable_codes() {
        let error = classify_query_error(
            "insert_traceability_snapshot.link",
            sqlx::Error::Protocol("boom".into()),
        );
        assert_eq!(error.code, "traceability_query_failed");
        assert!(error.message.contains("insert_traceability_snapshot.link"));
    }

    #[test]
    fn parse_outcome_accepts_missing_and_stale_records() {
        assert_eq!(
            parse_outcome("missing_evidence"),
            Ok(LinkOutcome::MissingEvidence)
        );
        assert_eq!(
            parse_outcome("stale_evidence"),
            Ok(LinkOutcome::StaleEvidence)
        );
    }

    #[test]
    fn parse_outcome_rejects_unknown_values_with_machine_code() {
        let error = parse_outcome("unknown").expect_err("unknown outcome must fail");
        assert_eq!(error.code, "traceability_invalid_payload");
        assert!(error.message.contains("unknown"));
    }

    #[test]
    fn list_payload_preserves_ambiguity_with_multiple_anchors() {
        let link = TraceabilityLink {
            canonical_requirement_id: "project_md.requirements.1".to_string(),
            anchors: vec![
                EvidenceAnchor {
                    evidence_type: EvidenceType::Code,
                    file_path: "crates/domain/src/traceability.rs".to_string(),
                    symbol: Some("build_traceability_link".to_string()),
                    section: None,
                    line_start: Some(1),
                    line_end: Some(20),
                },
                EvidenceAnchor {
                    evidence_type: EvidenceType::Test,
                    file_path: "tests/api/phase-2-traceability.test.mjs".to_string(),
                    symbol: Some("traceability".to_string()),
                    section: None,
                    line_start: Some(1),
                    line_end: Some(50),
                },
            ],
            rationale: "multiple plausible evidence anchors are retained".to_string(),
            confidence: LinkConfidence::Medium,
            outcome: LinkOutcome::Ambiguous,
            reason_code: Some("ambiguous_multiple_candidates".to_string()),
        };

        assert_eq!(link.anchors.len(), 2);
        assert!(matches!(link.outcome, LinkOutcome::Ambiguous));
    }

    #[test]
    fn decode_anchor_rejects_negative_line_values() {
        let error = decode_anchor_line_value(-1, "line_start")
            .expect_err("negative lines must fail closed");
        assert_eq!(error.code, "traceability_invalid_payload");
        assert!(error.message.contains("line_start"));
    }

    #[test]
    fn decode_anchor_rejects_out_of_range_line_values() {
        let error = decode_anchor_line_value(i64::from(u32::MAX) + 1, "line_end")
            .expect_err("out-of-range lines must fail closed");
        assert_eq!(error.code, "traceability_invalid_payload");
        assert!(error.message.contains("line_end"));
    }
}
