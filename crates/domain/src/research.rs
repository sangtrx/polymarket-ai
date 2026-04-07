use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AlphaHypothesisReasonCode {
    Registered,
    Updated,
    Read,
    NotFound,
    InvalidPayload,
    InvalidTrainingWindow,
    DatasetSnapshotUnresolved,
    DatasetSnapshotUnavailable,
    UnauthorizedRole,
    PersistenceUnavailable,
}

impl AlphaHypothesisReasonCode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Registered => "alpha_hypothesis_registered",
            Self::Updated => "alpha_hypothesis_updated",
            Self::Read => "alpha_hypothesis_read",
            Self::NotFound => "alpha_hypothesis_not_found",
            Self::InvalidPayload => "alpha_hypothesis_invalid_payload",
            Self::InvalidTrainingWindow => "alpha_hypothesis_invalid_training_window",
            Self::DatasetSnapshotUnresolved => "alpha_hypothesis_dataset_snapshot_unresolved",
            Self::DatasetSnapshotUnavailable => "alpha_hypothesis_dataset_snapshot_unavailable",
            Self::UnauthorizedRole => "alpha_hypothesis_unauthorized_role",
            Self::PersistenceUnavailable => "alpha_hypothesis_persistence_unavailable",
        }
    }

    pub fn parse(value: &str) -> Result<Self, AlphaHypothesisContractError> {
        match value {
            "alpha_hypothesis_registered" => Ok(Self::Registered),
            "alpha_hypothesis_updated" => Ok(Self::Updated),
            "alpha_hypothesis_read" => Ok(Self::Read),
            "alpha_hypothesis_not_found" => Ok(Self::NotFound),
            "alpha_hypothesis_invalid_payload" => Ok(Self::InvalidPayload),
            "alpha_hypothesis_invalid_training_window" => Ok(Self::InvalidTrainingWindow),
            "alpha_hypothesis_dataset_snapshot_unresolved" => Ok(Self::DatasetSnapshotUnresolved),
            "alpha_hypothesis_dataset_snapshot_unavailable" => Ok(Self::DatasetSnapshotUnavailable),
            "alpha_hypothesis_unauthorized_role" => Ok(Self::UnauthorizedRole),
            "alpha_hypothesis_persistence_unavailable" => Ok(Self::PersistenceUnavailable),
            _ => Err(AlphaHypothesisContractError::invalid_payload(format!(
                "unknown alpha hypothesis reason code `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlphaHypothesisRegistration {
    pub hypothesis_id: String,
    pub feature_set_version: String,
    pub target_regime: String,
    pub expected_edge_source: String,
    pub training_window_start_utc: String,
    pub training_window_end_utc: String,
    pub risk_assumptions: Value,
    pub actor_id: String,
    pub correlation_id: String,
    pub updated_at_utc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AlphaHypothesisValidationIssue {
    pub field: String,
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AlphaHypothesisContractError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<AlphaHypothesisValidationIssue>,
}

impl AlphaHypothesisContractError {
    pub fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: AlphaHypothesisReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn invalid_payload_with_issues(
        message: impl Into<String>,
        field_errors: Vec<AlphaHypothesisValidationIssue>,
    ) -> Self {
        Self {
            code: AlphaHypothesisReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }
}

pub fn normalize_research_identifier(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}

pub fn parse_utc_timestamp(value: &str) -> Result<OffsetDateTime, AlphaHypothesisContractError> {
    let parsed = OffsetDateTime::parse(value, &Rfc3339).map_err(|_| {
        AlphaHypothesisContractError::invalid_payload(format!(
            "timestamp `{value}` must be RFC3339 UTC"
        ))
    })?;

    if parsed.offset() != UtcOffset::UTC {
        return Err(AlphaHypothesisContractError::invalid_payload(
            "timestamps must use UTC `Z` offset",
        ));
    }

    Ok(parsed)
}

pub fn canonicalize_alpha_hypothesis_registration(
    registration: &AlphaHypothesisRegistration,
) -> Result<AlphaHypothesisRegistration, AlphaHypothesisContractError> {
    let normalized = AlphaHypothesisRegistration {
        hypothesis_id: normalize_research_identifier(&registration.hypothesis_id),
        feature_set_version: normalize_research_identifier(&registration.feature_set_version),
        target_regime: normalize_research_identifier(&registration.target_regime),
        expected_edge_source: normalize_research_identifier(&registration.expected_edge_source),
        training_window_start_utc: registration.training_window_start_utc.trim().to_string(),
        training_window_end_utc: registration.training_window_end_utc.trim().to_string(),
        risk_assumptions: registration.risk_assumptions.clone(),
        actor_id: registration.actor_id.trim().to_string(),
        correlation_id: registration.correlation_id.trim().to_string(),
        updated_at_utc: registration.updated_at_utc.trim().to_string(),
    };

    validate_alpha_hypothesis_registration(&normalized)?;
    Ok(normalized)
}

pub fn validate_alpha_hypothesis_registration(
    registration: &AlphaHypothesisRegistration,
) -> Result<(), AlphaHypothesisContractError> {
    let mut field_errors = Vec::new();

    validate_non_empty_field(
        &mut field_errors,
        "hypothesis_id",
        &registration.hypothesis_id,
    );
    validate_non_empty_field(
        &mut field_errors,
        "feature_set_version",
        &registration.feature_set_version,
    );
    validate_non_empty_field(
        &mut field_errors,
        "target_regime",
        &registration.target_regime,
    );
    validate_non_empty_field(
        &mut field_errors,
        "expected_edge_source",
        &registration.expected_edge_source,
    );
    validate_non_empty_field(&mut field_errors, "actor_id", &registration.actor_id);
    validate_non_empty_field(
        &mut field_errors,
        "correlation_id",
        &registration.correlation_id,
    );
    validate_non_empty_field(
        &mut field_errors,
        "updated_at_utc",
        &registration.updated_at_utc,
    );
    validate_non_empty_field(
        &mut field_errors,
        "training_window_start_utc",
        &registration.training_window_start_utc,
    );
    validate_non_empty_field(
        &mut field_errors,
        "training_window_end_utc",
        &registration.training_window_end_utc,
    );

    let start_ts = parse_timestamp_field(
        &mut field_errors,
        "training_window_start_utc",
        &registration.training_window_start_utc,
    );
    let end_ts = parse_timestamp_field(
        &mut field_errors,
        "training_window_end_utc",
        &registration.training_window_end_utc,
    );
    parse_timestamp_field(
        &mut field_errors,
        "updated_at_utc",
        &registration.updated_at_utc,
    );

    if let (Some(start_ts), Some(end_ts)) = (start_ts, end_ts)
        && start_ts >= end_ts
    {
        field_errors.push(AlphaHypothesisValidationIssue {
            field: "training_window".to_string(),
            code: AlphaHypothesisReasonCode::InvalidTrainingWindow.code(),
            message: "training window must satisfy start < end".to_string(),
        });
    }

    validate_risk_assumptions(&registration.risk_assumptions, &mut field_errors);

    if !field_errors.is_empty() {
        return Err(AlphaHypothesisContractError::invalid_payload_with_issues(
            "alpha hypothesis registration payload failed validation",
            field_errors,
        ));
    }

    Ok(())
}

fn validate_non_empty_field(
    field_errors: &mut Vec<AlphaHypothesisValidationIssue>,
    field: &str,
    value: &str,
) {
    if value.trim().is_empty() {
        field_errors.push(AlphaHypothesisValidationIssue {
            field: field.to_string(),
            code: AlphaHypothesisReasonCode::InvalidPayload.code(),
            message: format!("{field} is required"),
        });
    }
}

fn parse_timestamp_field(
    field_errors: &mut Vec<AlphaHypothesisValidationIssue>,
    field: &str,
    value: &str,
) -> Option<OffsetDateTime> {
    match parse_utc_timestamp(value) {
        Ok(parsed) => Some(parsed),
        Err(error) => {
            field_errors.push(AlphaHypothesisValidationIssue {
                field: field.to_string(),
                code: AlphaHypothesisReasonCode::InvalidPayload.code(),
                message: error.message,
            });
            None
        }
    }
}

fn validate_risk_assumptions(
    value: &Value,
    field_errors: &mut Vec<AlphaHypothesisValidationIssue>,
) {
    match value {
        Value::Object(map) => {
            if map.is_empty() {
                field_errors.push(AlphaHypothesisValidationIssue {
                    field: "risk_assumptions".to_string(),
                    code: AlphaHypothesisReasonCode::InvalidPayload.code(),
                    message: "risk_assumptions must include at least one entry".to_string(),
                });
                return;
            }

            for (key, nested_value) in map {
                if key.trim().is_empty() {
                    field_errors.push(AlphaHypothesisValidationIssue {
                        field: "risk_assumptions".to_string(),
                        code: AlphaHypothesisReasonCode::InvalidPayload.code(),
                        message: "risk_assumptions keys cannot be blank".to_string(),
                    });
                }
                validate_nested_assumption_value(
                    nested_value,
                    &format!("risk_assumptions.{key}"),
                    field_errors,
                );
            }
        }
        _ => field_errors.push(AlphaHypothesisValidationIssue {
            field: "risk_assumptions".to_string(),
            code: AlphaHypothesisReasonCode::InvalidPayload.code(),
            message: "risk_assumptions must be a JSON object".to_string(),
        }),
    }
}

fn validate_nested_assumption_value(
    value: &Value,
    path: &str,
    field_errors: &mut Vec<AlphaHypothesisValidationIssue>,
) {
    match value {
        Value::Null => field_errors.push(AlphaHypothesisValidationIssue {
            field: path.to_string(),
            code: AlphaHypothesisReasonCode::InvalidPayload.code(),
            message: "risk assumption values cannot be null".to_string(),
        }),
        Value::String(text) if text.trim().is_empty() => {
            field_errors.push(AlphaHypothesisValidationIssue {
                field: path.to_string(),
                code: AlphaHypothesisReasonCode::InvalidPayload.code(),
                message: "risk assumption string values cannot be blank".to_string(),
            })
        }
        Value::Array(items) => {
            if items.is_empty() {
                field_errors.push(AlphaHypothesisValidationIssue {
                    field: path.to_string(),
                    code: AlphaHypothesisReasonCode::InvalidPayload.code(),
                    message: "risk assumption arrays cannot be empty".to_string(),
                });
            }
            for (index, item) in items.iter().enumerate() {
                validate_nested_assumption_value(item, &format!("{path}[{index}]"), field_errors);
            }
        }
        Value::Object(map) => {
            if map.is_empty() {
                field_errors.push(AlphaHypothesisValidationIssue {
                    field: path.to_string(),
                    code: AlphaHypothesisReasonCode::InvalidPayload.code(),
                    message: "risk assumption objects cannot be empty".to_string(),
                });
            }
            for (key, item) in map {
                if key.trim().is_empty() {
                    field_errors.push(AlphaHypothesisValidationIssue {
                        field: path.to_string(),
                        code: AlphaHypothesisReasonCode::InvalidPayload.code(),
                        message: "risk assumption object keys cannot be blank".to_string(),
                    });
                }
                validate_nested_assumption_value(item, &format!("{path}.{key}"), field_errors);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_registration() -> AlphaHypothesisRegistration {
        AlphaHypothesisRegistration {
            hypothesis_id: "alpha::mean-reversion".to_string(),
            feature_set_version: "dataset::v1".to_string(),
            target_regime: "overnight".to_string(),
            expected_edge_source: "liquidity_dislocation".to_string(),
            training_window_start_utc: "2026-04-01T00:00:00Z".to_string(),
            training_window_end_utc: "2026-04-02T00:00:00Z".to_string(),
            risk_assumptions: json!({
                "max_drawdown_pct": 2.5,
                "min_depth_usd": 5000.0
            }),
            actor_id: "ops-1".to_string(),
            correlation_id: "corr-hypothesis-001".to_string(),
            updated_at_utc: "2026-04-07T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn canonicalization_normalizes_identifier_fields() {
        let mut registration = sample_registration();
        registration.hypothesis_id = " Alpha::Mean-Reversion ".to_string();
        registration.feature_set_version = " Dataset::V1 ".to_string();
        registration.target_regime = " Overnight ".to_string();
        registration.expected_edge_source = " Liquidity_Dislocation ".to_string();

        let canonical = canonicalize_alpha_hypothesis_registration(&registration)
            .expect("canonicalization should succeed");
        assert_eq!(canonical.hypothesis_id, "alpha::mean-reversion");
        assert_eq!(canonical.feature_set_version, "dataset::v1");
        assert_eq!(canonical.target_regime, "overnight");
        assert_eq!(canonical.expected_edge_source, "liquidity_dislocation");
    }

    #[test]
    fn validation_rejects_equal_training_window_boundaries() {
        let mut registration = sample_registration();
        registration.training_window_end_utc = registration.training_window_start_utc.clone();

        let error = validate_alpha_hypothesis_registration(&registration)
            .expect_err("equal training window boundaries must fail");
        assert_eq!(error.code, AlphaHypothesisReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "training_window")
        );
    }

    #[test]
    fn validation_rejects_non_object_risk_assumptions() {
        let mut registration = sample_registration();
        registration.risk_assumptions = json!(["free_form_text"]);

        let error = validate_alpha_hypothesis_registration(&registration)
            .expect_err("non-object assumptions should fail");
        assert_eq!(error.code, AlphaHypothesisReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "risk_assumptions")
        );
    }

    #[test]
    fn validation_rejects_null_nested_assumption_values() {
        let mut registration = sample_registration();
        registration.risk_assumptions = json!({
            "execution": {
                "max_slippage_bps": null
            }
        });

        let error = validate_alpha_hypothesis_registration(&registration)
            .expect_err("null nested assumptions should fail");
        assert!(error.field_errors.iter().any(|issue| {
            issue
                .field
                .contains("risk_assumptions.execution.max_slippage_bps")
        }));
    }

    #[test]
    fn parse_utc_timestamp_rejects_non_utc_offsets() {
        let error = parse_utc_timestamp("2026-04-07T01:00:00+01:00")
            .expect_err("non-UTC timestamp should fail");
        assert_eq!(error.code, AlphaHypothesisReasonCode::InvalidPayload.code());
    }

    #[test]
    fn reason_code_parse_accepts_known_values() {
        let code = AlphaHypothesisReasonCode::parse("alpha_hypothesis_dataset_snapshot_unresolved")
            .expect("known reason should parse");
        assert_eq!(code, AlphaHypothesisReasonCode::DatasetSnapshotUnresolved);
    }
}
