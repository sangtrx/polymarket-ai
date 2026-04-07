use domain::research::{
    PromotionDecisionContractError, PromotionDecisionReasonCode, PromotionDecisionRecord,
    PromotionDecisionState, PromotionDecisionValidationIssue, PromotionLifecycleAction,
    PromotionThresholdOutcome, canonicalize_promotion_decision_record,
    normalize_research_identifier, parse_promotion_utc_timestamp,
};
use serde_json::{Value, json};
use sqlx::{PgExecutor, Row};
use std::error::Error;
use std::fmt::{Display, Formatter};

const UPSERT_PROMOTION_DECISION_SQL: &str = r#"
    INSERT INTO promotion_decisions (
        decision_id,
        candidate_id,
        validation_run_id,
        lifecycle_action,
        decision_state,
        reason_code,
        observed_metrics_json,
        evidence_packet_json,
        threshold_results_json,
        missing_evidence_fields_json,
        gate_evaluation_json,
        shadow_readiness_json,
        actor_id,
        correlation_id,
        decided_at_utc,
        approval_request_id,
        approval_reference,
        updated_at_utc
    ) VALUES (
        $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14,
        $15::timestamptz, $16, $17, NOW()
    )
    ON CONFLICT (decision_id)
    DO UPDATE SET
        candidate_id = EXCLUDED.candidate_id,
        validation_run_id = EXCLUDED.validation_run_id,
        lifecycle_action = EXCLUDED.lifecycle_action,
        decision_state = EXCLUDED.decision_state,
        reason_code = EXCLUDED.reason_code,
        observed_metrics_json = EXCLUDED.observed_metrics_json,
        evidence_packet_json = EXCLUDED.evidence_packet_json,
        threshold_results_json = EXCLUDED.threshold_results_json,
        missing_evidence_fields_json = EXCLUDED.missing_evidence_fields_json,
        gate_evaluation_json = EXCLUDED.gate_evaluation_json,
        shadow_readiness_json = EXCLUDED.shadow_readiness_json,
        actor_id = EXCLUDED.actor_id,
        correlation_id = EXCLUDED.correlation_id,
        decided_at_utc = EXCLUDED.decided_at_utc,
        approval_request_id = EXCLUDED.approval_request_id,
        approval_reference = EXCLUDED.approval_reference,
        updated_at_utc = NOW()
"#;

const LOAD_PROMOTION_DECISION_SQL: &str = r#"
    SELECT
        decision_id,
        candidate_id,
        validation_run_id,
        lifecycle_action,
        decision_state,
        reason_code,
        observed_metrics_json,
        evidence_packet_json,
        threshold_results_json,
        missing_evidence_fields_json,
        gate_evaluation_json,
        shadow_readiness_json,
        actor_id,
        correlation_id,
        to_char(decided_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS decided_at_utc,
        approval_request_id,
        approval_reference
    FROM promotion_decisions
    WHERE lower(trim(decision_id)) = lower(trim($1))
    ORDER BY decided_at_utc DESC, decision_id ASC
    LIMIT 1
"#;

const LIST_PROMOTION_DECISIONS_BY_CANDIDATE_SQL: &str = r#"
    SELECT
        decision_id,
        candidate_id,
        validation_run_id,
        lifecycle_action,
        decision_state,
        reason_code,
        observed_metrics_json,
        evidence_packet_json,
        threshold_results_json,
        missing_evidence_fields_json,
        gate_evaluation_json,
        shadow_readiness_json,
        actor_id,
        correlation_id,
        to_char(decided_at_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS decided_at_utc,
        approval_request_id,
        approval_reference
    FROM promotion_decisions
    WHERE lower(trim(candidate_id)) = lower(trim($1))
      AND ($2::timestamptz IS NULL OR decided_at_utc >= $2::timestamptz)
      AND ($3::timestamptz IS NULL OR decided_at_utc < $3::timestamptz)
    ORDER BY decided_at_utc DESC, decision_id ASC
    LIMIT $4
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromotionDecisionPersistenceError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<PromotionDecisionValidationIssue>,
}

impl PromotionDecisionPersistenceError {
    fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<PromotionDecisionValidationIssue>,
    ) -> Self {
        Self {
            code: PromotionDecisionReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn query_failure(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "promotion_decision_query_failed",
            message: format!("{operation} failed: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn constraint_violation(operation: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "promotion_decision_constraint_violation",
            message: format!("{operation} rejected by constraint: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn row_decode_failure(column: &'static str, error: sqlx::Error) -> Self {
        Self {
            code: "promotion_decision_row_decode_failed",
            message: format!("unable to decode `{column}`: {error}"),
            field_errors: Vec::new(),
        }
    }

    fn row_contract_failure(column: &'static str, error: PromotionDecisionContractError) -> Self {
        Self {
            code: "promotion_decision_row_decode_failed",
            message: format!("invalid persisted value for `{column}`: {}", error.message),
            field_errors: error.field_errors,
        }
    }
}

impl Display for PromotionDecisionPersistenceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for PromotionDecisionPersistenceError {}

pub async fn upsert_promotion_decision<'e, E>(
    executor: E,
    record: &PromotionDecisionRecord,
) -> Result<(), PromotionDecisionPersistenceError>
where
    E: PgExecutor<'e>,
{
    let canonical = validate_record_for_persistence(record)?;
    let threshold_results_json = json!({
        "thresholds": canonical.threshold_results
    });
    let missing_evidence_fields_json = json!({
        "fields": canonical.missing_evidence_fields
    });

    let result = sqlx::query(UPSERT_PROMOTION_DECISION_SQL)
        .bind(&canonical.decision_id)
        .bind(&canonical.candidate_id)
        .bind(&canonical.validation_run_id)
        .bind(canonical.lifecycle_action.as_str())
        .bind(canonical.decision_state.as_str())
        .bind(&canonical.reason_code)
        .bind(&canonical.observed_metrics)
        .bind(&canonical.evidence_packet)
        .bind(threshold_results_json)
        .bind(missing_evidence_fields_json)
        .bind(&canonical.gate_evaluation)
        .bind(canonical.shadow_readiness.as_ref())
        .bind(&canonical.actor_id)
        .bind(&canonical.correlation_id)
        .bind(&canonical.decided_at_utc)
        .bind(canonical.approval_request_id.as_deref())
        .bind(canonical.approval_reference.as_deref())
        .execute(executor)
        .await
        .map_err(|error| classify_query_error("upsert_promotion_decision", error))?;

    if result.rows_affected() != 1 {
        return Err(PromotionDecisionPersistenceError::invalid_payload(
            format!(
                "upsert_promotion_decision expected 1 affected row, got {}",
                result.rows_affected()
            ),
            Vec::new(),
        ));
    }
    Ok(())
}

pub async fn load_promotion_decision<'e, E>(
    executor: E,
    decision_id: &str,
) -> Result<Option<PromotionDecisionRecord>, PromotionDecisionPersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_non_empty("decision_id", decision_id)?;
    let normalized_decision_id = normalize_research_identifier(decision_id);

    let row = sqlx::query(LOAD_PROMOTION_DECISION_SQL)
        .bind(&normalized_decision_id)
        .fetch_optional(executor)
        .await
        .map_err(|error| {
            PromotionDecisionPersistenceError::query_failure("load_promotion_decision", error)
        })?;

    row.map(decode_promotion_decision_row).transpose()
}

pub async fn list_promotion_decisions_by_candidate<'e, E>(
    executor: E,
    candidate_id: &str,
    decided_after_utc: Option<&str>,
    decided_before_utc: Option<&str>,
    limit: i64,
) -> Result<Vec<PromotionDecisionRecord>, PromotionDecisionPersistenceError>
where
    E: PgExecutor<'e>,
{
    validate_non_empty("candidate_id", candidate_id)?;
    if limit <= 0 {
        return Err(PromotionDecisionPersistenceError::invalid_payload(
            "limit must be greater than 0",
            vec![PromotionDecisionValidationIssue {
                field: "limit".to_string(),
                code: PromotionDecisionReasonCode::InvalidPayload.code(),
                message: "limit must be greater than 0".to_string(),
            }],
        ));
    }
    let normalized_candidate_id = normalize_research_identifier(candidate_id);
    let normalized_decided_after =
        normalize_optional_timestamp("decided_after_utc", decided_after_utc)?;
    let normalized_decided_before =
        normalize_optional_timestamp("decided_before_utc", decided_before_utc)?;

    if let (Some(decided_after), Some(decided_before)) = (
        normalized_decided_after.as_deref(),
        normalized_decided_before.as_deref(),
    ) {
        let decided_after_ts =
            parse_promotion_utc_timestamp(decided_after).map_err(map_contract_error)?;
        let decided_before_ts =
            parse_promotion_utc_timestamp(decided_before).map_err(map_contract_error)?;
        if decided_before_ts <= decided_after_ts {
            return Err(PromotionDecisionPersistenceError::invalid_payload(
                "decided_before_utc must be greater than decided_after_utc",
                vec![PromotionDecisionValidationIssue {
                    field: "decided_before_utc".to_string(),
                    code: PromotionDecisionReasonCode::InvalidPayload.code(),
                    message: "decided_before_utc must be greater than decided_after_utc"
                        .to_string(),
                }],
            ));
        }
    }

    let rows = sqlx::query(LIST_PROMOTION_DECISIONS_BY_CANDIDATE_SQL)
        .bind(&normalized_candidate_id)
        .bind(normalized_decided_after.as_deref())
        .bind(normalized_decided_before.as_deref())
        .bind(limit)
        .fetch_all(executor)
        .await
        .map_err(|error| {
            PromotionDecisionPersistenceError::query_failure(
                "list_promotion_decisions_by_candidate",
                error,
            )
        })?;

    rows.into_iter()
        .map(decode_promotion_decision_row)
        .collect()
}

fn decode_promotion_decision_row(
    row: sqlx::postgres::PgRow,
) -> Result<PromotionDecisionRecord, PromotionDecisionPersistenceError> {
    let lifecycle_action_raw: String = row.try_get("lifecycle_action").map_err(|error| {
        PromotionDecisionPersistenceError::row_decode_failure("lifecycle_action", error)
    })?;
    let lifecycle_action =
        PromotionLifecycleAction::parse(&lifecycle_action_raw).map_err(|error| {
            PromotionDecisionPersistenceError::row_contract_failure("lifecycle_action", error)
        })?;

    let decision_state_raw: String = row.try_get("decision_state").map_err(|error| {
        PromotionDecisionPersistenceError::row_decode_failure("decision_state", error)
    })?;
    let decision_state = PromotionDecisionState::parse(&decision_state_raw).map_err(|error| {
        PromotionDecisionPersistenceError::row_contract_failure("decision_state", error)
    })?;

    let reason_code: String = row.try_get("reason_code").map_err(|error| {
        PromotionDecisionPersistenceError::row_decode_failure("reason_code", error)
    })?;
    PromotionDecisionReasonCode::parse(&reason_code).map_err(|error| {
        PromotionDecisionPersistenceError::row_contract_failure("reason_code", error)
    })?;

    let threshold_results_json: Value = row.try_get("threshold_results_json").map_err(|error| {
        PromotionDecisionPersistenceError::row_decode_failure("threshold_results_json", error)
    })?;
    let threshold_results = decode_threshold_results_json(threshold_results_json)?;

    let missing_evidence_fields_json: Value =
        row.try_get("missing_evidence_fields_json")
            .map_err(|error| {
                PromotionDecisionPersistenceError::row_decode_failure(
                    "missing_evidence_fields_json",
                    error,
                )
            })?;
    let missing_evidence_fields =
        decode_missing_evidence_fields_json(missing_evidence_fields_json)?;

    let record = PromotionDecisionRecord {
        decision_id: row.try_get("decision_id").map_err(|error| {
            PromotionDecisionPersistenceError::row_decode_failure("decision_id", error)
        })?,
        candidate_id: row.try_get("candidate_id").map_err(|error| {
            PromotionDecisionPersistenceError::row_decode_failure("candidate_id", error)
        })?,
        validation_run_id: row.try_get("validation_run_id").map_err(|error| {
            PromotionDecisionPersistenceError::row_decode_failure("validation_run_id", error)
        })?,
        lifecycle_action,
        decision_state,
        reason_code,
        observed_metrics: row.try_get("observed_metrics_json").map_err(|error| {
            PromotionDecisionPersistenceError::row_decode_failure("observed_metrics_json", error)
        })?,
        evidence_packet: row.try_get("evidence_packet_json").map_err(|error| {
            PromotionDecisionPersistenceError::row_decode_failure("evidence_packet_json", error)
        })?,
        threshold_results,
        missing_evidence_fields,
        gate_evaluation: row.try_get("gate_evaluation_json").map_err(|error| {
            PromotionDecisionPersistenceError::row_decode_failure("gate_evaluation_json", error)
        })?,
        shadow_readiness: row.try_get("shadow_readiness_json").map_err(|error| {
            PromotionDecisionPersistenceError::row_decode_failure("shadow_readiness_json", error)
        })?,
        actor_id: row.try_get("actor_id").map_err(|error| {
            PromotionDecisionPersistenceError::row_decode_failure("actor_id", error)
        })?,
        correlation_id: row.try_get("correlation_id").map_err(|error| {
            PromotionDecisionPersistenceError::row_decode_failure("correlation_id", error)
        })?,
        decided_at_utc: row.try_get("decided_at_utc").map_err(|error| {
            PromotionDecisionPersistenceError::row_decode_failure("decided_at_utc", error)
        })?,
        approval_request_id: row.try_get("approval_request_id").map_err(|error| {
            PromotionDecisionPersistenceError::row_decode_failure("approval_request_id", error)
        })?,
        approval_reference: row.try_get("approval_reference").map_err(|error| {
            PromotionDecisionPersistenceError::row_decode_failure("approval_reference", error)
        })?,
    };

    validate_record_for_persistence(&record)
}

fn decode_threshold_results_json(
    value: Value,
) -> Result<Vec<PromotionThresholdOutcome>, PromotionDecisionPersistenceError> {
    let Value::Object(map) = value else {
        return Err(PromotionDecisionPersistenceError::invalid_payload(
            "threshold_results_json must be a JSON object",
            vec![PromotionDecisionValidationIssue {
                field: "threshold_results_json".to_string(),
                code: PromotionDecisionReasonCode::InvalidPayload.code(),
                message: "threshold_results_json must be a JSON object".to_string(),
            }],
        ));
    };
    let Some(thresholds) = map.get("thresholds") else {
        return Err(PromotionDecisionPersistenceError::invalid_payload(
            "threshold_results_json must include `thresholds`",
            vec![PromotionDecisionValidationIssue {
                field: "threshold_results_json.thresholds".to_string(),
                code: PromotionDecisionReasonCode::InvalidPayload.code(),
                message: "threshold_results_json must include `thresholds`".to_string(),
            }],
        ));
    };
    let Value::Array(thresholds) = thresholds else {
        return Err(PromotionDecisionPersistenceError::invalid_payload(
            "threshold_results_json.thresholds must be an array",
            vec![PromotionDecisionValidationIssue {
                field: "threshold_results_json.thresholds".to_string(),
                code: PromotionDecisionReasonCode::InvalidPayload.code(),
                message: "threshold_results_json.thresholds must be an array".to_string(),
            }],
        ));
    };

    thresholds
        .iter()
        .enumerate()
        .map(|(index, threshold)| {
            serde_json::from_value::<PromotionThresholdOutcome>(threshold.clone()).map_err(
                |error| {
                    PromotionDecisionPersistenceError::invalid_payload(
                        format!("invalid threshold outcome at index {index}: {error}"),
                        vec![PromotionDecisionValidationIssue {
                            field: format!("threshold_results_json.thresholds[{index}]"),
                            code: PromotionDecisionReasonCode::InvalidPayload.code(),
                            message: "invalid promotion threshold outcome payload".to_string(),
                        }],
                    )
                },
            )
        })
        .collect()
}

fn decode_missing_evidence_fields_json(
    value: Value,
) -> Result<Vec<String>, PromotionDecisionPersistenceError> {
    let Value::Object(map) = value else {
        return Err(PromotionDecisionPersistenceError::invalid_payload(
            "missing_evidence_fields_json must be a JSON object",
            vec![PromotionDecisionValidationIssue {
                field: "missing_evidence_fields_json".to_string(),
                code: PromotionDecisionReasonCode::InvalidPayload.code(),
                message: "missing_evidence_fields_json must be a JSON object".to_string(),
            }],
        ));
    };
    let Some(fields) = map.get("fields") else {
        return Err(PromotionDecisionPersistenceError::invalid_payload(
            "missing_evidence_fields_json must include `fields`",
            vec![PromotionDecisionValidationIssue {
                field: "missing_evidence_fields_json.fields".to_string(),
                code: PromotionDecisionReasonCode::InvalidPayload.code(),
                message: "missing_evidence_fields_json must include `fields`".to_string(),
            }],
        ));
    };
    let Value::Array(fields) = fields else {
        return Err(PromotionDecisionPersistenceError::invalid_payload(
            "missing_evidence_fields_json.fields must be an array",
            vec![PromotionDecisionValidationIssue {
                field: "missing_evidence_fields_json.fields".to_string(),
                code: PromotionDecisionReasonCode::InvalidPayload.code(),
                message: "missing_evidence_fields_json.fields must be an array".to_string(),
            }],
        ));
    };

    fields
        .iter()
        .enumerate()
        .map(|(index, field)| {
            field
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .ok_or_else(|| {
                    PromotionDecisionPersistenceError::invalid_payload(
                        format!("invalid missing evidence field at index {index}"),
                        vec![PromotionDecisionValidationIssue {
                            field: format!("missing_evidence_fields_json.fields[{index}]"),
                            code: PromotionDecisionReasonCode::InvalidPayload.code(),
                            message: "missing evidence field must be a non-empty string"
                                .to_string(),
                        }],
                    )
                })
        })
        .collect()
}

fn validate_record_for_persistence(
    record: &PromotionDecisionRecord,
) -> Result<PromotionDecisionRecord, PromotionDecisionPersistenceError> {
    canonicalize_promotion_decision_record(record).map_err(map_contract_error)
}

fn normalize_optional_timestamp(
    field: &str,
    value: Option<&str>,
) -> Result<Option<String>, PromotionDecisionPersistenceError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    parse_promotion_utc_timestamp(trimmed).map_err(|_| {
        PromotionDecisionPersistenceError::invalid_payload(
            format!("{field} must be an RFC3339 UTC timestamp"),
            vec![PromotionDecisionValidationIssue {
                field: field.to_string(),
                code: PromotionDecisionReasonCode::InvalidPayload.code(),
                message: format!("{field} must be an RFC3339 UTC timestamp"),
            }],
        )
    })?;
    Ok(Some(trimmed.to_string()))
}

fn map_contract_error(error: PromotionDecisionContractError) -> PromotionDecisionPersistenceError {
    PromotionDecisionPersistenceError::invalid_payload(error.message, error.field_errors)
}

fn validate_non_empty(field: &str, value: &str) -> Result<(), PromotionDecisionPersistenceError> {
    if value.trim().is_empty() {
        return Err(PromotionDecisionPersistenceError::invalid_payload(
            format!("{field} cannot be blank"),
            vec![PromotionDecisionValidationIssue {
                field: field.to_string(),
                code: PromotionDecisionReasonCode::InvalidPayload.code(),
                message: format!("{field} cannot be blank"),
            }],
        ));
    }
    Ok(())
}

fn classify_query_error(
    operation: &'static str,
    error: sqlx::Error,
) -> PromotionDecisionPersistenceError {
    if is_constraint_error(&error) {
        return PromotionDecisionPersistenceError::constraint_violation(operation, error);
    }
    PromotionDecisionPersistenceError::query_failure(operation, error)
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

    const PROMOTION_DECISIONS_MIGRATION_SQL: &str =
        include_str!("../../migrations/20260407223000_promotion_decisions.sql");

    fn sample_promotion_decision_record() -> PromotionDecisionRecord {
        PromotionDecisionRecord {
            decision_id: "candidate::alpha-1::1712448000".to_string(),
            candidate_id: "candidate::alpha-1".to_string(),
            validation_run_id: "candidate::alpha-1::1712447000".to_string(),
            lifecycle_action: PromotionLifecycleAction::Promote,
            decision_state: PromotionDecisionState::Denied,
            reason_code: PromotionDecisionReasonCode::MissingEvidence
                .code()
                .to_string(),
            observed_metrics: json!({
                "out_of_sample_sharpe": 1.21,
                "max_drawdown": -0.18
            }),
            evidence_packet: json!({
                "data_quality_report": { "artifact_id": "quality::001" },
                "purged_cpcv_results": { "artifact_id": "cpcv::001" },
                "calibration_report": { "artifact_id": "calibration::001" }
            }),
            threshold_results: vec![PromotionThresholdOutcome {
                metric_key: "out_of_sample_sharpe".to_string(),
                comparator: domain::research::ValidationGateComparator::Gte,
                threshold_value: 1.0,
                observed_value: 1.21,
                passed: true,
                reason_code: "promotion_threshold_passed".to_string(),
            }],
            missing_evidence_fields: vec!["counterfactual_replay_summary".to_string()],
            gate_evaluation: json!({
                "reason_code": "validation_gate_evaluation_allowed",
                "gate_passed": true
            }),
            shadow_readiness: Some(json!({
                "evaluation_id": "candidate::alpha-1::1712447999",
                "reason_code": "shadow_evaluation_completed"
            })),
            actor_id: "ops-1".to_string(),
            correlation_id: "corr-promotion-001".to_string(),
            decided_at_utc: "2026-04-07T00:00:00Z".to_string(),
            approval_request_id: Some("request::promotion-001".to_string()),
            approval_reference: Some("approval::promotion-001".to_string()),
        }
    }

    #[test]
    fn migration_creates_expected_promotion_decision_schema_scope() {
        assert!(
            PROMOTION_DECISIONS_MIGRATION_SQL
                .contains("CREATE TABLE IF NOT EXISTS promotion_decisions")
        );
        assert!(
            !PROMOTION_DECISIONS_MIGRATION_SQL
                .contains("CREATE TABLE IF NOT EXISTS validation_runs")
        );
        assert!(
            !PROMOTION_DECISIONS_MIGRATION_SQL
                .contains("CREATE TABLE IF NOT EXISTS shadow_evaluations")
        );
    }

    #[test]
    fn migration_enforces_promotion_decision_constraints_and_indexes() {
        assert!(
            PROMOTION_DECISIONS_MIGRATION_SQL
                .contains("lifecycle_action IN ('promote', 'pause', 'retire')")
        );
        assert!(
            PROMOTION_DECISIONS_MIGRATION_SQL.contains("decision_state IN ('allowed', 'denied')")
        );
        assert!(
            PROMOTION_DECISIONS_MIGRATION_SQL.contains("idx_promotion_decisions_candidate_lookup")
        );
        assert!(
            PROMOTION_DECISIONS_MIGRATION_SQL
                .contains("idx_promotion_decisions_id_canonical_unique")
        );
    }

    #[test]
    fn promotion_decision_validation_canonicalizes_identifiers() {
        let mut record = sample_promotion_decision_record();
        record.decision_id = " Candidate::Alpha-1::1712448000 ".to_string();
        record.candidate_id = " Candidate::Alpha-1 ".to_string();
        record.validation_run_id = " Candidate::Alpha-1::1712447000 ".to_string();
        record.reason_code = " Promotion_Decision_Missing_Evidence ".to_string();

        let canonical = validate_record_for_persistence(&record)
            .expect("canonical promotion decision should pass");
        assert_eq!(canonical.decision_id, "candidate::alpha-1::1712448000");
        assert_eq!(canonical.candidate_id, "candidate::alpha-1");
        assert_eq!(
            canonical.validation_run_id,
            "candidate::alpha-1::1712447000"
        );
        assert_eq!(
            canonical.reason_code,
            PromotionDecisionReasonCode::MissingEvidence.code()
        );
    }

    #[test]
    fn promotion_decision_validation_rejects_non_object_evidence_packet() {
        let mut record = sample_promotion_decision_record();
        record.evidence_packet = json!(["invalid"]);

        let error = validate_record_for_persistence(&record)
            .expect_err("evidence_packet must be a JSON object");
        assert_eq!(
            error.code,
            PromotionDecisionReasonCode::InvalidPayload.code()
        );
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "evidence_packet")
        );
    }

    #[test]
    fn list_query_orders_promotion_decisions_deterministically() {
        assert!(
            LIST_PROMOTION_DECISIONS_BY_CANDIDATE_SQL
                .contains("ORDER BY decided_at_utc DESC, decision_id ASC")
        );
    }
}
