use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ValidationGateType {
    ForwardBias,
    DataLeakage,
    RegimeSurvivability,
    DataQuality,
}

impl ValidationGateType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ForwardBias => "forward_bias",
            Self::DataLeakage => "data_leakage",
            Self::RegimeSurvivability => "regime_survivability",
            Self::DataQuality => "data_quality",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ValidationGateContractError> {
        match normalize_research_identifier(value).as_str() {
            "forward_bias" => Ok(Self::ForwardBias),
            "data_leakage" => Ok(Self::DataLeakage),
            "regime_survivability" => Ok(Self::RegimeSurvivability),
            "data_quality" => Ok(Self::DataQuality),
            _ => Err(ValidationGateContractError::invalid_payload(format!(
                "unknown validation gate type `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ValidationGateStageScope {
    Training,
    Promotion,
    TrainingAndPromotion,
}

impl ValidationGateStageScope {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Training => "training",
            Self::Promotion => "promotion",
            Self::TrainingAndPromotion => "training_and_promotion",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ValidationGateContractError> {
        match normalize_research_identifier(value).as_str() {
            "training" => Ok(Self::Training),
            "promotion" => Ok(Self::Promotion),
            "training_and_promotion" => Ok(Self::TrainingAndPromotion),
            _ => Err(ValidationGateContractError::invalid_payload(format!(
                "unknown validation gate stage scope `{value}`"
            ))),
        }
    }

    pub const fn applies_to(self, stage: ValidationGateWorkflowStage) -> bool {
        matches!(
            (self, stage),
            (Self::Training, ValidationGateWorkflowStage::Training)
                | (Self::Promotion, ValidationGateWorkflowStage::Promotion)
                | (Self::TrainingAndPromotion, _)
        )
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ValidationGateWorkflowStage {
    Training,
    Promotion,
}

impl ValidationGateWorkflowStage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Training => "training",
            Self::Promotion => "promotion",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ValidationGateContractError> {
        match normalize_research_identifier(value).as_str() {
            "training" => Ok(Self::Training),
            "promotion" => Ok(Self::Promotion),
            _ => Err(ValidationGateContractError::invalid_payload(format!(
                "unknown validation workflow stage `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ValidationGateComparator {
    Lt,
    Lte,
    Gt,
    Gte,
}

impl ValidationGateComparator {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lt => "lt",
            Self::Lte => "lte",
            Self::Gt => "gt",
            Self::Gte => "gte",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ValidationGateContractError> {
        match normalize_research_identifier(value).as_str() {
            "lt" => Ok(Self::Lt),
            "lte" => Ok(Self::Lte),
            "gt" => Ok(Self::Gt),
            "gte" => Ok(Self::Gte),
            _ => Err(ValidationGateContractError::invalid_payload(format!(
                "unknown validation gate comparator `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ValidationGateReasonCode {
    PolicyRegistered,
    PolicyUpdated,
    PolicyRead,
    PolicyListed,
    EvaluationAllowed,
    EvaluationDenied,
    InvalidPayload,
    UnauthorizedRole,
    PolicyNotFound,
    PolicyUnresolved,
    MissingMandatoryPolicy,
    GateFailed,
    DependencyUnavailable,
    StateUnavailable,
    PersistenceUnavailable,
}

impl ValidationGateReasonCode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::PolicyRegistered => "validation_gate_policy_registered",
            Self::PolicyUpdated => "validation_gate_policy_updated",
            Self::PolicyRead => "validation_gate_policy_read",
            Self::PolicyListed => "validation_gate_policy_listed",
            Self::EvaluationAllowed => "validation_gate_evaluation_allowed",
            Self::EvaluationDenied => "validation_gate_evaluation_denied",
            Self::InvalidPayload => "validation_gate_invalid_payload",
            Self::UnauthorizedRole => "validation_gate_unauthorized_role",
            Self::PolicyNotFound => "validation_gate_policy_not_found",
            Self::PolicyUnresolved => "validation_gate_policy_unresolved",
            Self::MissingMandatoryPolicy => "validation_gate_missing_mandatory_policy",
            Self::GateFailed => "validation_gate_failed",
            Self::DependencyUnavailable => "validation_gate_dependency_unavailable",
            Self::StateUnavailable => "validation_gate_state_unavailable",
            Self::PersistenceUnavailable => "validation_gate_persistence_unavailable",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ValidationGateContractError> {
        match value {
            "validation_gate_policy_registered" => Ok(Self::PolicyRegistered),
            "validation_gate_policy_updated" => Ok(Self::PolicyUpdated),
            "validation_gate_policy_read" => Ok(Self::PolicyRead),
            "validation_gate_policy_listed" => Ok(Self::PolicyListed),
            "validation_gate_evaluation_allowed" => Ok(Self::EvaluationAllowed),
            "validation_gate_evaluation_denied" => Ok(Self::EvaluationDenied),
            "validation_gate_invalid_payload" => Ok(Self::InvalidPayload),
            "validation_gate_unauthorized_role" => Ok(Self::UnauthorizedRole),
            "validation_gate_policy_not_found" => Ok(Self::PolicyNotFound),
            "validation_gate_policy_unresolved" => Ok(Self::PolicyUnresolved),
            "validation_gate_missing_mandatory_policy" => Ok(Self::MissingMandatoryPolicy),
            "validation_gate_failed" => Ok(Self::GateFailed),
            "validation_gate_dependency_unavailable" => Ok(Self::DependencyUnavailable),
            "validation_gate_state_unavailable" => Ok(Self::StateUnavailable),
            "validation_gate_persistence_unavailable" => Ok(Self::PersistenceUnavailable),
            _ => Err(ValidationGateContractError::invalid_payload(format!(
                "unknown validation gate reason code `{value}`"
            ))),
        }
    }
}

pub const FR43_MANDATORY_GATE_TYPES: [ValidationGateType; 3] = [
    ValidationGateType::ForwardBias,
    ValidationGateType::DataLeakage,
    ValidationGateType::RegimeSurvivability,
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValidationGateThreshold {
    pub comparator: ValidationGateComparator,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValidationGatePolicyDefinition {
    pub policy_key: String,
    pub gate_type: ValidationGateType,
    pub stage_scope: ValidationGateStageScope,
    pub metric_key: String,
    pub threshold: ValidationGateThreshold,
    pub mandatory: bool,
    pub diagnostics: Value,
    pub actor_id: String,
    pub correlation_id: String,
    pub updated_at_utc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidationGateValidationIssue {
    pub field: String,
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidationGateContractError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<ValidationGateValidationIssue>,
}

impl ValidationGateContractError {
    pub fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: ValidationGateReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn invalid_payload_with_issues(
        message: impl Into<String>,
        field_errors: Vec<ValidationGateValidationIssue>,
    ) -> Self {
        Self {
            code: ValidationGateReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }
}

pub fn canonicalize_validation_gate_policy_definition(
    policy: &ValidationGatePolicyDefinition,
) -> Result<ValidationGatePolicyDefinition, ValidationGateContractError> {
    let canonical = ValidationGatePolicyDefinition {
        policy_key: normalize_research_identifier(&policy.policy_key),
        gate_type: policy.gate_type,
        stage_scope: policy.stage_scope,
        metric_key: normalize_research_identifier(&policy.metric_key),
        threshold: policy.threshold.clone(),
        mandatory: policy.mandatory,
        diagnostics: policy.diagnostics.clone(),
        actor_id: policy.actor_id.trim().to_string(),
        correlation_id: policy.correlation_id.trim().to_string(),
        updated_at_utc: policy.updated_at_utc.trim().to_string(),
    };

    validate_validation_gate_policy_definition(&canonical)?;
    Ok(canonical)
}

pub fn validate_validation_gate_policy_definition(
    policy: &ValidationGatePolicyDefinition,
) -> Result<(), ValidationGateContractError> {
    let mut field_errors = Vec::new();

    validate_non_empty_validation_gate_field(&mut field_errors, "policy_key", &policy.policy_key);
    validate_non_empty_validation_gate_field(&mut field_errors, "metric_key", &policy.metric_key);
    validate_non_empty_validation_gate_field(&mut field_errors, "actor_id", &policy.actor_id);
    validate_non_empty_validation_gate_field(
        &mut field_errors,
        "correlation_id",
        &policy.correlation_id,
    );
    validate_non_empty_validation_gate_field(
        &mut field_errors,
        "updated_at_utc",
        &policy.updated_at_utc,
    );

    if parse_utc_timestamp(&policy.updated_at_utc).is_err() {
        field_errors.push(ValidationGateValidationIssue {
            field: "updated_at_utc".to_string(),
            code: ValidationGateReasonCode::InvalidPayload.code(),
            message: "updated_at_utc must be RFC3339 UTC".to_string(),
        });
    }

    if !policy.threshold.value.is_finite() {
        field_errors.push(ValidationGateValidationIssue {
            field: "threshold.value".to_string(),
            code: ValidationGateReasonCode::InvalidPayload.code(),
            message: "threshold.value must be finite".to_string(),
        });
    }

    match &policy.diagnostics {
        Value::Object(map) if map.is_empty() => field_errors.push(ValidationGateValidationIssue {
            field: "diagnostics".to_string(),
            code: ValidationGateReasonCode::InvalidPayload.code(),
            message: "diagnostics must include at least one machine-readable field".to_string(),
        }),
        Value::Object(_) => {}
        _ => field_errors.push(ValidationGateValidationIssue {
            field: "diagnostics".to_string(),
            code: ValidationGateReasonCode::InvalidPayload.code(),
            message: "diagnostics must be a JSON object".to_string(),
        }),
    }

    let requires_mandatory = FR43_MANDATORY_GATE_TYPES.contains(&policy.gate_type)
        || policy.gate_type == ValidationGateType::DataQuality;
    if requires_mandatory && !policy.mandatory {
        field_errors.push(ValidationGateValidationIssue {
            field: "mandatory".to_string(),
            code: ValidationGateReasonCode::MissingMandatoryPolicy.code(),
            message: format!(
                "gate type `{}` must remain mandatory under FR43 gate policy rules",
                policy.gate_type.as_str()
            ),
        });
    }

    if !field_errors.is_empty() {
        return Err(ValidationGateContractError::invalid_payload_with_issues(
            "validation gate policy payload failed validation",
            field_errors,
        ));
    }

    Ok(())
}

pub fn evaluate_validation_gate_threshold(
    threshold: &ValidationGateThreshold,
    observed_value: f64,
) -> Result<bool, ValidationGateContractError> {
    if !observed_value.is_finite() {
        return Err(ValidationGateContractError::invalid_payload_with_issues(
            "observed gate metric is unavailable",
            vec![ValidationGateValidationIssue {
                field: "observed_value".to_string(),
                code: ValidationGateReasonCode::StateUnavailable.code(),
                message: "observed_value must be finite".to_string(),
            }],
        ));
    }

    if !threshold.value.is_finite() {
        return Err(ValidationGateContractError::invalid_payload_with_issues(
            "threshold metric is unavailable",
            vec![ValidationGateValidationIssue {
                field: "threshold.value".to_string(),
                code: ValidationGateReasonCode::InvalidPayload.code(),
                message: "threshold.value must be finite".to_string(),
            }],
        ));
    }

    Ok(match threshold.comparator {
        ValidationGateComparator::Lt => observed_value < threshold.value,
        ValidationGateComparator::Lte => observed_value <= threshold.value,
        ValidationGateComparator::Gt => observed_value > threshold.value,
        ValidationGateComparator::Gte => observed_value >= threshold.value,
    })
}

fn validate_non_empty_validation_gate_field(
    field_errors: &mut Vec<ValidationGateValidationIssue>,
    field: &str,
    value: &str,
) {
    if value.trim().is_empty() {
        field_errors.push(ValidationGateValidationIssue {
            field: field.to_string(),
            code: ValidationGateReasonCode::InvalidPayload.code(),
            message: format!("{field} is required"),
        });
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ValidationWorkflowStage {
    Quality,
    Labeling,
    PurgedCv,
    Cpcv,
    OverfitDiagnostics,
}

impl ValidationWorkflowStage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Quality => "quality",
            Self::Labeling => "labeling",
            Self::PurgedCv => "purged_cv",
            Self::Cpcv => "cpcv",
            Self::OverfitDiagnostics => "overfit_diagnostics",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ValidationWorkflowContractError> {
        match normalize_research_identifier(value).as_str() {
            "quality" => Ok(Self::Quality),
            "labeling" => Ok(Self::Labeling),
            "purged_cv" => Ok(Self::PurgedCv),
            "cpcv" => Ok(Self::Cpcv),
            "overfit_diagnostics" => Ok(Self::OverfitDiagnostics),
            _ => Err(ValidationWorkflowContractError::invalid_payload(format!(
                "unknown validation workflow stage `{value}`"
            ))),
        }
    }

    pub const fn stage_index(self) -> i16 {
        match self {
            Self::Quality => 1,
            Self::Labeling => 2,
            Self::PurgedCv => 3,
            Self::Cpcv => 4,
            Self::OverfitDiagnostics => 5,
        }
    }

    pub fn from_stage_index(stage_index: i16) -> Result<Self, ValidationWorkflowContractError> {
        match stage_index {
            1 => Ok(Self::Quality),
            2 => Ok(Self::Labeling),
            3 => Ok(Self::PurgedCv),
            4 => Ok(Self::Cpcv),
            5 => Ok(Self::OverfitDiagnostics),
            _ => Err(ValidationWorkflowContractError::invalid_payload(format!(
                "unknown validation workflow stage index `{stage_index}`"
            ))),
        }
    }

    pub const fn ordered() -> [Self; 5] {
        [
            Self::Quality,
            Self::Labeling,
            Self::PurgedCv,
            Self::Cpcv,
            Self::OverfitDiagnostics,
        ]
    }
}

pub const FR7_VALIDATION_WORKFLOW_STAGES: [ValidationWorkflowStage; 5] =
    ValidationWorkflowStage::ordered();

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ValidationWorkflowStageOutcome {
    Passed,
    Failed,
    Blocked,
}

impl ValidationWorkflowStageOutcome {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Blocked => "blocked",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ValidationWorkflowContractError> {
        match normalize_research_identifier(value).as_str() {
            "passed" => Ok(Self::Passed),
            "failed" => Ok(Self::Failed),
            "blocked" => Ok(Self::Blocked),
            _ => Err(ValidationWorkflowContractError::invalid_payload(format!(
                "unknown validation stage outcome `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ValidationWorkflowRunState {
    Running,
    Completed,
    Blocked,
    Failed,
}

impl ValidationWorkflowRunState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Blocked => "blocked",
            Self::Failed => "failed",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ValidationWorkflowContractError> {
        match normalize_research_identifier(value).as_str() {
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "blocked" => Ok(Self::Blocked),
            "failed" => Ok(Self::Failed),
            _ => Err(ValidationWorkflowContractError::invalid_payload(format!(
                "unknown validation run state `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ValidationWorkflowReasonCode {
    RunStarted,
    RunCompleted,
    RunRead,
    RunListed,
    ArtifactRead,
    ComparisonReady,
    InvalidPayload,
    UnauthorizedRole,
    RunNotFound,
    ArtifactNotFound,
    GateDenied,
    StagePassed,
    StageFailed,
    DependencyUnavailable,
    StateUnavailable,
    PersistenceUnavailable,
}

impl ValidationWorkflowReasonCode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::RunStarted => "validation_run_started",
            Self::RunCompleted => "validation_run_completed",
            Self::RunRead => "validation_run_read",
            Self::RunListed => "validation_run_listed",
            Self::ArtifactRead => "validation_artifact_read",
            Self::ComparisonReady => "validation_comparison_ready",
            Self::InvalidPayload => "validation_run_invalid_payload",
            Self::UnauthorizedRole => "validation_run_unauthorized_role",
            Self::RunNotFound => "validation_run_not_found",
            Self::ArtifactNotFound => "validation_artifact_not_found",
            Self::GateDenied => "validation_run_gate_denied",
            Self::StagePassed => "validation_run_stage_passed",
            Self::StageFailed => "validation_run_stage_failed",
            Self::DependencyUnavailable => "validation_run_dependency_unavailable",
            Self::StateUnavailable => "validation_run_state_unavailable",
            Self::PersistenceUnavailable => "validation_run_persistence_unavailable",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ValidationWorkflowContractError> {
        match value {
            "validation_run_started" => Ok(Self::RunStarted),
            "validation_run_completed" => Ok(Self::RunCompleted),
            "validation_run_read" => Ok(Self::RunRead),
            "validation_run_listed" => Ok(Self::RunListed),
            "validation_artifact_read" => Ok(Self::ArtifactRead),
            "validation_comparison_ready" => Ok(Self::ComparisonReady),
            "validation_run_invalid_payload" => Ok(Self::InvalidPayload),
            "validation_run_unauthorized_role" => Ok(Self::UnauthorizedRole),
            "validation_run_not_found" => Ok(Self::RunNotFound),
            "validation_artifact_not_found" => Ok(Self::ArtifactNotFound),
            "validation_run_gate_denied" => Ok(Self::GateDenied),
            "validation_run_stage_passed" => Ok(Self::StagePassed),
            "validation_run_stage_failed" => Ok(Self::StageFailed),
            "validation_run_dependency_unavailable" => Ok(Self::DependencyUnavailable),
            "validation_run_state_unavailable" => Ok(Self::StateUnavailable),
            "validation_run_persistence_unavailable" => Ok(Self::PersistenceUnavailable),
            _ => Err(ValidationWorkflowContractError::invalid_payload(format!(
                "unknown validation workflow reason code `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidationWorkflowValidationIssue {
    pub field: String,
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidationWorkflowContractError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<ValidationWorkflowValidationIssue>,
}

impl ValidationWorkflowContractError {
    pub fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: ValidationWorkflowReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn invalid_payload_with_issues(
        message: impl Into<String>,
        field_errors: Vec<ValidationWorkflowValidationIssue>,
    ) -> Self {
        Self {
            code: ValidationWorkflowReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValidationDiagnosticsPayload {
    pub out_of_sample_sharpe: f64,
    pub max_drawdown: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brier_score: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_calibration_error: Option<f64>,
    pub overfit_indicator: f64,
    pub overfit_flag: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValidationWorkflowRunRecord {
    pub run_id: String,
    pub candidate_id: String,
    pub run_state: ValidationWorkflowRunState,
    pub reason_code: String,
    pub gate_evaluation: Value,
    pub comparison_ready: bool,
    pub actor_id: String,
    pub correlation_id: String,
    pub started_at_utc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at_utc: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValidationWorkflowArtifactRecord {
    pub artifact_id: String,
    pub run_id: String,
    pub candidate_id: String,
    pub stage: ValidationWorkflowStage,
    pub stage_index: i16,
    pub stage_outcome: ValidationWorkflowStageOutcome,
    pub reason_code: String,
    pub diagnostics: ValidationDiagnosticsPayload,
    pub actor_id: String,
    pub correlation_id: String,
    pub stage_started_at_utc: String,
    pub stage_completed_at_utc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValidationMetricDelta {
    pub metric_key: String,
    pub current_value: f64,
    pub prior_value: f64,
    pub delta: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValidationStageComparison {
    pub stage: ValidationWorkflowStage,
    pub current_run_id: String,
    pub previous_run_id: String,
    pub reason_code: String,
    pub metric_deltas: Vec<ValidationMetricDelta>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ShadowEvaluationState {
    Running,
    Completed,
    Denied,
    Failed,
}

impl ShadowEvaluationState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Denied => "denied",
            Self::Failed => "failed",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ShadowEvaluationContractError> {
        match normalize_research_identifier(value).as_str() {
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "denied" => Ok(Self::Denied),
            "failed" => Ok(Self::Failed),
            _ => Err(ShadowEvaluationContractError::invalid_payload(format!(
                "unknown shadow evaluation state `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ShadowSimulationDecisionSide {
    Buy,
    Sell,
    Hold,
}

impl ShadowSimulationDecisionSide {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Buy => "buy",
            Self::Sell => "sell",
            Self::Hold => "hold",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ShadowEvaluationContractError> {
        match normalize_research_identifier(value).as_str() {
            "buy" => Ok(Self::Buy),
            "sell" => Ok(Self::Sell),
            "hold" => Ok(Self::Hold),
            _ => Err(ShadowEvaluationContractError::invalid_payload(format!(
                "unknown shadow simulation decision side `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ShadowSimulationReasonCode {
    ReadOnlyEnforced,
    DependencyUnavailable,
    StateUnavailable,
}

impl ShadowSimulationReasonCode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::ReadOnlyEnforced => "shadow_simulation_read_only_enforced",
            Self::DependencyUnavailable => "shadow_simulation_dependency_unavailable",
            Self::StateUnavailable => "shadow_simulation_state_unavailable",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ShadowEvaluationContractError> {
        match value {
            "shadow_simulation_read_only_enforced" => Ok(Self::ReadOnlyEnforced),
            "shadow_simulation_dependency_unavailable" => Ok(Self::DependencyUnavailable),
            "shadow_simulation_state_unavailable" => Ok(Self::StateUnavailable),
            _ => Err(ShadowEvaluationContractError::invalid_payload(format!(
                "unknown shadow simulation reason code `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ShadowEvaluationReasonCode {
    EvaluationStarted,
    EvaluationCompleted,
    EvaluationRead,
    EvaluationListed,
    InvalidPayload,
    UnauthorizedRole,
    EvaluationNotFound,
    ValidationRunIneligible,
    DependencyUnavailable,
    StateUnavailable,
    PersistenceUnavailable,
}

impl ShadowEvaluationReasonCode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::EvaluationStarted => "shadow_evaluation_started",
            Self::EvaluationCompleted => "shadow_evaluation_completed",
            Self::EvaluationRead => "shadow_evaluation_read",
            Self::EvaluationListed => "shadow_evaluation_listed",
            Self::InvalidPayload => "shadow_evaluation_invalid_payload",
            Self::UnauthorizedRole => "shadow_evaluation_unauthorized_role",
            Self::EvaluationNotFound => "shadow_evaluation_not_found",
            Self::ValidationRunIneligible => "shadow_evaluation_validation_run_ineligible",
            Self::DependencyUnavailable => "shadow_evaluation_dependency_unavailable",
            Self::StateUnavailable => "shadow_evaluation_state_unavailable",
            Self::PersistenceUnavailable => "shadow_evaluation_persistence_unavailable",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ShadowEvaluationContractError> {
        match value {
            "shadow_evaluation_started" => Ok(Self::EvaluationStarted),
            "shadow_evaluation_completed" => Ok(Self::EvaluationCompleted),
            "shadow_evaluation_read" => Ok(Self::EvaluationRead),
            "shadow_evaluation_listed" => Ok(Self::EvaluationListed),
            "shadow_evaluation_invalid_payload" => Ok(Self::InvalidPayload),
            "shadow_evaluation_unauthorized_role" => Ok(Self::UnauthorizedRole),
            "shadow_evaluation_not_found" => Ok(Self::EvaluationNotFound),
            "shadow_evaluation_validation_run_ineligible" => Ok(Self::ValidationRunIneligible),
            "shadow_evaluation_dependency_unavailable" => Ok(Self::DependencyUnavailable),
            "shadow_evaluation_state_unavailable" => Ok(Self::StateUnavailable),
            "shadow_evaluation_persistence_unavailable" => Ok(Self::PersistenceUnavailable),
            _ => Err(ShadowEvaluationContractError::invalid_payload(format!(
                "unknown shadow evaluation reason code `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShadowEvaluationValidationIssue {
    pub field: String,
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShadowEvaluationContractError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<ShadowEvaluationValidationIssue>,
}

impl ShadowEvaluationContractError {
    pub fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: ShadowEvaluationReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn invalid_payload_with_issues(
        message: impl Into<String>,
        field_errors: Vec<ShadowEvaluationValidationIssue>,
    ) -> Self {
        Self {
            code: ShadowEvaluationReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShadowSimulationOutcome {
    pub decision_side: ShadowSimulationDecisionSide,
    pub intended_size: f64,
    pub simulated_fill_size: f64,
    pub simulated_fill_price: f64,
    pub simulated_slippage_bps: f64,
    pub simulation_reason_code: String,
    pub decision_timestamp_utc: String,
    pub simulated_at_utc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShadowEvaluationRecord {
    pub evaluation_id: String,
    pub candidate_id: String,
    pub validation_run_id: String,
    pub evaluation_state: ShadowEvaluationState,
    pub reason_code: String,
    pub market_context: Value,
    pub signal_decisions: Value,
    pub simulation_outcomes: Vec<ShadowSimulationOutcome>,
    pub actor_id: String,
    pub correlation_id: String,
    pub started_at_utc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at_utc: Option<String>,
}

pub fn parse_shadow_utc_timestamp(
    value: &str,
) -> Result<OffsetDateTime, ShadowEvaluationContractError> {
    let parsed = OffsetDateTime::parse(value, &Rfc3339).map_err(|_| {
        ShadowEvaluationContractError::invalid_payload(format!(
            "timestamp `{value}` must be RFC3339 UTC"
        ))
    })?;
    if parsed.offset() != UtcOffset::UTC {
        return Err(ShadowEvaluationContractError::invalid_payload(
            "timestamps must use UTC `Z` offset",
        ));
    }
    Ok(parsed)
}

pub fn compose_shadow_evaluation_id(
    candidate_id: &str,
    started_at_utc: &str,
) -> Result<String, ShadowEvaluationContractError> {
    let normalized_candidate_id = normalize_research_identifier(candidate_id);
    if normalized_candidate_id.is_empty() {
        return Err(ShadowEvaluationContractError::invalid_payload_with_issues(
            "candidate_id cannot be blank",
            vec![ShadowEvaluationValidationIssue {
                field: "candidate_id".to_string(),
                code: ShadowEvaluationReasonCode::InvalidPayload.code(),
                message: "candidate_id cannot be blank".to_string(),
            }],
        ));
    }
    let started_at = parse_shadow_utc_timestamp(started_at_utc)?;
    Ok(format!(
        "{}::{}",
        normalized_candidate_id,
        started_at.unix_timestamp_nanos()
    ))
}

pub fn canonicalize_shadow_evaluation_record(
    record: &ShadowEvaluationRecord,
) -> Result<ShadowEvaluationRecord, ShadowEvaluationContractError> {
    let mut canonical = ShadowEvaluationRecord {
        evaluation_id: normalize_research_identifier(&record.evaluation_id),
        candidate_id: normalize_research_identifier(&record.candidate_id),
        validation_run_id: normalize_research_identifier(&record.validation_run_id),
        evaluation_state: record.evaluation_state,
        reason_code: normalize_research_identifier(&record.reason_code),
        market_context: record.market_context.clone(),
        signal_decisions: record.signal_decisions.clone(),
        simulation_outcomes: record.simulation_outcomes.clone(),
        actor_id: record.actor_id.trim().to_string(),
        correlation_id: record.correlation_id.trim().to_string(),
        started_at_utc: record.started_at_utc.trim().to_string(),
        completed_at_utc: record
            .completed_at_utc
            .as_ref()
            .map(|value| value.trim().to_string()),
    };
    canonical.simulation_outcomes.sort_by(|left, right| {
        left.decision_timestamp_utc
            .cmp(&right.decision_timestamp_utc)
            .then_with(|| left.decision_side.cmp(&right.decision_side))
            .then_with(|| left.simulated_at_utc.cmp(&right.simulated_at_utc))
    });
    validate_shadow_evaluation_record(&canonical)?;
    Ok(canonical)
}

pub fn validate_shadow_evaluation_record(
    record: &ShadowEvaluationRecord,
) -> Result<(), ShadowEvaluationContractError> {
    let mut field_errors = Vec::new();
    validate_non_empty_shadow_field(&mut field_errors, "evaluation_id", &record.evaluation_id);
    validate_non_empty_shadow_field(&mut field_errors, "candidate_id", &record.candidate_id);
    validate_non_empty_shadow_field(
        &mut field_errors,
        "validation_run_id",
        &record.validation_run_id,
    );
    validate_non_empty_shadow_field(&mut field_errors, "reason_code", &record.reason_code);
    validate_non_empty_shadow_field(&mut field_errors, "actor_id", &record.actor_id);
    validate_non_empty_shadow_field(&mut field_errors, "correlation_id", &record.correlation_id);
    validate_non_empty_shadow_field(&mut field_errors, "started_at_utc", &record.started_at_utc);

    if parse_shadow_utc_timestamp(&record.started_at_utc).is_err() {
        field_errors.push(ShadowEvaluationValidationIssue {
            field: "started_at_utc".to_string(),
            code: ShadowEvaluationReasonCode::InvalidPayload.code(),
            message: "started_at_utc must be RFC3339 UTC".to_string(),
        });
    }
    if let Some(completed_at_utc) = record.completed_at_utc.as_deref() {
        if parse_shadow_utc_timestamp(completed_at_utc).is_err() {
            field_errors.push(ShadowEvaluationValidationIssue {
                field: "completed_at_utc".to_string(),
                code: ShadowEvaluationReasonCode::InvalidPayload.code(),
                message: "completed_at_utc must be RFC3339 UTC".to_string(),
            });
        } else if let (Ok(started), Ok(completed)) = (
            parse_shadow_utc_timestamp(&record.started_at_utc),
            parse_shadow_utc_timestamp(completed_at_utc),
        ) && completed < started
        {
            field_errors.push(ShadowEvaluationValidationIssue {
                field: "completed_at_utc".to_string(),
                code: ShadowEvaluationReasonCode::InvalidPayload.code(),
                message: "completed_at_utc must be >= started_at_utc".to_string(),
            });
        }
    }
    if ShadowEvaluationReasonCode::parse(&record.reason_code).is_err() {
        field_errors.push(ShadowEvaluationValidationIssue {
            field: "reason_code".to_string(),
            code: ShadowEvaluationReasonCode::InvalidPayload.code(),
            message: "reason_code is not a recognized shadow evaluation code".to_string(),
        });
    }
    match &record.market_context {
        Value::Object(map) if !map.is_empty() => {}
        _ => field_errors.push(ShadowEvaluationValidationIssue {
            field: "market_context".to_string(),
            code: ShadowEvaluationReasonCode::InvalidPayload.code(),
            message: "market_context must be a non-empty JSON object".to_string(),
        }),
    }
    match &record.signal_decisions {
        Value::Object(map) if !map.is_empty() => {}
        _ => field_errors.push(ShadowEvaluationValidationIssue {
            field: "signal_decisions".to_string(),
            code: ShadowEvaluationReasonCode::InvalidPayload.code(),
            message: "signal_decisions must be a non-empty JSON object".to_string(),
        }),
    }
    if record.evaluation_state == ShadowEvaluationState::Completed
        && record.simulation_outcomes.is_empty()
    {
        field_errors.push(ShadowEvaluationValidationIssue {
            field: "simulation_outcomes".to_string(),
            code: ShadowEvaluationReasonCode::StateUnavailable.code(),
            message: "completed evaluations must include at least one simulation outcome"
                .to_string(),
        });
    }
    for (index, outcome) in record.simulation_outcomes.iter().enumerate() {
        validate_shadow_simulation_outcome(outcome).map_err(|error| {
            let prefixed = error
                .field_errors
                .into_iter()
                .map(|issue| ShadowEvaluationValidationIssue {
                    field: format!("simulation_outcomes[{index}].{}", issue.field),
                    code: issue.code,
                    message: issue.message,
                })
                .collect::<Vec<_>>();
            ShadowEvaluationContractError::invalid_payload_with_issues(error.message, prefixed)
        })?;
    }
    if !field_errors.is_empty() {
        return Err(ShadowEvaluationContractError::invalid_payload_with_issues(
            "shadow evaluation payload failed validation",
            field_errors,
        ));
    }
    Ok(())
}

pub fn validate_shadow_simulation_outcome(
    outcome: &ShadowSimulationOutcome,
) -> Result<(), ShadowEvaluationContractError> {
    let mut field_errors = Vec::new();
    validate_finite_non_negative_shadow_metric(
        outcome.intended_size,
        "intended_size",
        &mut field_errors,
    );
    validate_finite_non_negative_shadow_metric(
        outcome.simulated_fill_size,
        "simulated_fill_size",
        &mut field_errors,
    );
    validate_finite_non_negative_shadow_metric(
        outcome.simulated_fill_price,
        "simulated_fill_price",
        &mut field_errors,
    );
    validate_finite_non_negative_shadow_metric(
        outcome.simulated_slippage_bps,
        "simulated_slippage_bps",
        &mut field_errors,
    );
    if outcome.simulated_fill_size > outcome.intended_size {
        field_errors.push(ShadowEvaluationValidationIssue {
            field: "simulated_fill_size".to_string(),
            code: ShadowEvaluationReasonCode::InvalidPayload.code(),
            message: "simulated_fill_size cannot exceed intended_size".to_string(),
        });
    }
    if ShadowSimulationReasonCode::parse(&outcome.simulation_reason_code).is_err() {
        field_errors.push(ShadowEvaluationValidationIssue {
            field: "simulation_reason_code".to_string(),
            code: ShadowEvaluationReasonCode::InvalidPayload.code(),
            message: "simulation_reason_code is not recognized".to_string(),
        });
    }
    let decision_ts = parse_shadow_utc_timestamp(&outcome.decision_timestamp_utc);
    if decision_ts.is_err() {
        field_errors.push(ShadowEvaluationValidationIssue {
            field: "decision_timestamp_utc".to_string(),
            code: ShadowEvaluationReasonCode::InvalidPayload.code(),
            message: "decision_timestamp_utc must be RFC3339 UTC".to_string(),
        });
    }
    let simulated_ts = parse_shadow_utc_timestamp(&outcome.simulated_at_utc);
    if simulated_ts.is_err() {
        field_errors.push(ShadowEvaluationValidationIssue {
            field: "simulated_at_utc".to_string(),
            code: ShadowEvaluationReasonCode::InvalidPayload.code(),
            message: "simulated_at_utc must be RFC3339 UTC".to_string(),
        });
    }
    if let (Ok(decision), Ok(simulated)) = (decision_ts, simulated_ts)
        && simulated < decision
    {
        field_errors.push(ShadowEvaluationValidationIssue {
            field: "simulated_at_utc".to_string(),
            code: ShadowEvaluationReasonCode::InvalidPayload.code(),
            message: "simulated_at_utc must be >= decision_timestamp_utc".to_string(),
        });
    }
    if !field_errors.is_empty() {
        return Err(ShadowEvaluationContractError::invalid_payload_with_issues(
            "shadow simulation outcome failed validation",
            field_errors,
        ));
    }
    Ok(())
}

fn validate_non_empty_shadow_field(
    field_errors: &mut Vec<ShadowEvaluationValidationIssue>,
    field: &str,
    value: &str,
) {
    if value.trim().is_empty() {
        field_errors.push(ShadowEvaluationValidationIssue {
            field: field.to_string(),
            code: ShadowEvaluationReasonCode::InvalidPayload.code(),
            message: format!("{field} is required"),
        });
    }
}

fn validate_finite_non_negative_shadow_metric(
    value: f64,
    field: &str,
    field_errors: &mut Vec<ShadowEvaluationValidationIssue>,
) {
    if !value.is_finite() || value < 0.0 {
        field_errors.push(ShadowEvaluationValidationIssue {
            field: field.to_string(),
            code: ShadowEvaluationReasonCode::InvalidPayload.code(),
            message: format!("{field} must be finite and >= 0"),
        });
    }
}

pub const FR45_REQUIRED_PROMOTION_PACKET_FIELDS: [&str; 4] = [
    "data_quality_report",
    "purged_cpcv_results",
    "calibration_report",
    "counterfactual_replay_summary",
];

pub const FR46_STRESSED_SLIPPAGE_MULTIPLIER: f64 = 2.0;
pub const FR46_STRESSED_FILL_RATE_MULTIPLIER: f64 = 0.5;
pub const FR46_DELAYED_EXIT_SECONDS: i64 = 60;
pub const FR46_DEGRADATION_DENY_THRESHOLD_PCT: f64 = -5.0;
pub const FR46_REQUIRED_COUNTERFACTUAL_SCENARIOS: [CounterfactualReplayScenarioKind; 3] = [
    CounterfactualReplayScenarioKind::Baseline,
    CounterfactualReplayScenarioKind::StressedExecution,
    CounterfactualReplayScenarioKind::DelayedExit,
];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum CounterfactualReplayScenarioKind {
    Baseline,
    StressedExecution,
    DelayedExit,
}

impl CounterfactualReplayScenarioKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Baseline => "baseline",
            Self::StressedExecution => "stressed_execution",
            Self::DelayedExit => "delayed_exit",
        }
    }

    pub fn parse(value: &str) -> Result<Self, CounterfactualReplayContractError> {
        match normalize_research_identifier(value).as_str() {
            "baseline" => Ok(Self::Baseline),
            "stressed_execution" => Ok(Self::StressedExecution),
            "delayed_exit" => Ok(Self::DelayedExit),
            _ => Err(CounterfactualReplayContractError::invalid_payload(format!(
                "unknown counterfactual replay scenario `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CounterfactualReplayGateOutcome {
    Allow,
    Deny,
    Unavailable,
}

impl CounterfactualReplayGateOutcome {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
            Self::Unavailable => "unavailable",
        }
    }

    pub fn parse(value: &str) -> Result<Self, CounterfactualReplayContractError> {
        match normalize_research_identifier(value).as_str() {
            "allow" => Ok(Self::Allow),
            "deny" => Ok(Self::Deny),
            "unavailable" => Ok(Self::Unavailable),
            _ => Err(CounterfactualReplayContractError::invalid_payload(format!(
                "unknown counterfactual replay gate outcome `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CounterfactualReplayRunState {
    Running,
    Completed,
    Denied,
    Failed,
}

impl CounterfactualReplayRunState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Denied => "denied",
            Self::Failed => "failed",
        }
    }

    pub fn parse(value: &str) -> Result<Self, CounterfactualReplayContractError> {
        match normalize_research_identifier(value).as_str() {
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "denied" => Ok(Self::Denied),
            "failed" => Ok(Self::Failed),
            _ => Err(CounterfactualReplayContractError::invalid_payload(format!(
                "unknown counterfactual replay run state `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CounterfactualReplayReasonCode {
    RunStarted,
    RunCompleted,
    RunRead,
    RunListed,
    InvalidPayload,
    UnauthorizedRole,
    RunNotFound,
    ScenarioIncomplete,
    ToleranceSatisfied,
    ToleranceBreached,
    BaselineUnavailable,
    PlaceholderRejected,
    DependencyUnavailable,
    StateUnavailable,
    PersistenceUnavailable,
}

impl CounterfactualReplayReasonCode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::RunStarted => "counterfactual_replay_run_started",
            Self::RunCompleted => "counterfactual_replay_run_completed",
            Self::RunRead => "counterfactual_replay_run_read",
            Self::RunListed => "counterfactual_replay_run_listed",
            Self::InvalidPayload => "counterfactual_replay_invalid_payload",
            Self::UnauthorizedRole => "counterfactual_replay_unauthorized_role",
            Self::RunNotFound => "counterfactual_replay_run_not_found",
            Self::ScenarioIncomplete => "counterfactual_replay_scenario_incomplete",
            Self::ToleranceSatisfied => "counterfactual_replay_tolerance_satisfied",
            Self::ToleranceBreached => "counterfactual_replay_tolerance_breached",
            Self::BaselineUnavailable => "counterfactual_replay_baseline_unavailable",
            Self::PlaceholderRejected => "counterfactual_replay_placeholder_rejected",
            Self::DependencyUnavailable => "counterfactual_replay_dependency_unavailable",
            Self::StateUnavailable => "counterfactual_replay_state_unavailable",
            Self::PersistenceUnavailable => "counterfactual_replay_persistence_unavailable",
        }
    }

    pub fn parse(value: &str) -> Result<Self, CounterfactualReplayContractError> {
        match value {
            "counterfactual_replay_run_started" => Ok(Self::RunStarted),
            "counterfactual_replay_run_completed" => Ok(Self::RunCompleted),
            "counterfactual_replay_run_read" => Ok(Self::RunRead),
            "counterfactual_replay_run_listed" => Ok(Self::RunListed),
            "counterfactual_replay_invalid_payload" => Ok(Self::InvalidPayload),
            "counterfactual_replay_unauthorized_role" => Ok(Self::UnauthorizedRole),
            "counterfactual_replay_run_not_found" => Ok(Self::RunNotFound),
            "counterfactual_replay_scenario_incomplete" => Ok(Self::ScenarioIncomplete),
            "counterfactual_replay_tolerance_satisfied" => Ok(Self::ToleranceSatisfied),
            "counterfactual_replay_tolerance_breached" => Ok(Self::ToleranceBreached),
            "counterfactual_replay_baseline_unavailable" => Ok(Self::BaselineUnavailable),
            "counterfactual_replay_placeholder_rejected" => Ok(Self::PlaceholderRejected),
            "counterfactual_replay_dependency_unavailable" => Ok(Self::DependencyUnavailable),
            "counterfactual_replay_state_unavailable" => Ok(Self::StateUnavailable),
            "counterfactual_replay_persistence_unavailable" => Ok(Self::PersistenceUnavailable),
            _ => Err(CounterfactualReplayContractError::invalid_payload(format!(
                "unknown counterfactual replay reason code `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CounterfactualReplayValidationIssue {
    pub field: String,
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CounterfactualReplayContractError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<CounterfactualReplayValidationIssue>,
}

impl CounterfactualReplayContractError {
    pub fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: CounterfactualReplayReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn invalid_payload_with_issues(
        message: impl Into<String>,
        field_errors: Vec<CounterfactualReplayValidationIssue>,
    ) -> Self {
        Self {
            code: CounterfactualReplayReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CounterfactualReplayScenarioParameters {
    pub slippage_multiplier: f64,
    pub fill_rate_multiplier: f64,
    pub exit_delay_seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CounterfactualReplayScenarioResult {
    pub scenario: CounterfactualReplayScenarioKind,
    pub net_pnl: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub degradation_pct: Option<f64>,
    pub gate_outcome: CounterfactualReplayGateOutcome,
    pub reason_code: String,
    pub parameters: CounterfactualReplayScenarioParameters,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CounterfactualReplaySummary {
    pub run_id: String,
    pub gate_outcome: CounterfactualReplayGateOutcome,
    pub reason_code: String,
    pub baseline_net_pnl: f64,
    pub stressed_net_pnl: f64,
    pub delayed_exit_net_pnl: f64,
    pub degradation_pct: f64,
    pub tolerance_threshold_pct: f64,
    pub scenarios: Vec<CounterfactualReplayScenarioResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CounterfactualReplayRunRecord {
    pub run_id: String,
    pub candidate_id: String,
    pub validation_run_id: String,
    pub run_state: CounterfactualReplayRunState,
    pub reason_code: String,
    pub scenario_results: Vec<CounterfactualReplayScenarioResult>,
    pub replay_summary: CounterfactualReplaySummary,
    pub actor_id: String,
    pub correlation_id: String,
    pub started_at_utc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at_utc: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CounterfactualReplayGateEvaluation {
    pub degradation_pct: f64,
    pub gate_outcome: CounterfactualReplayGateOutcome,
    pub reason_code: String,
}

pub fn fr46_scenario_parameters(
    scenario: CounterfactualReplayScenarioKind,
) -> CounterfactualReplayScenarioParameters {
    match scenario {
        CounterfactualReplayScenarioKind::Baseline => CounterfactualReplayScenarioParameters {
            slippage_multiplier: 1.0,
            fill_rate_multiplier: 1.0,
            exit_delay_seconds: 0,
        },
        CounterfactualReplayScenarioKind::StressedExecution => {
            CounterfactualReplayScenarioParameters {
                slippage_multiplier: FR46_STRESSED_SLIPPAGE_MULTIPLIER,
                fill_rate_multiplier: FR46_STRESSED_FILL_RATE_MULTIPLIER,
                exit_delay_seconds: 0,
            }
        }
        CounterfactualReplayScenarioKind::DelayedExit => CounterfactualReplayScenarioParameters {
            slippage_multiplier: 1.0,
            fill_rate_multiplier: 1.0,
            exit_delay_seconds: FR46_DELAYED_EXIT_SECONDS,
        },
    }
}

pub fn parse_counterfactual_replay_utc_timestamp(
    value: &str,
) -> Result<OffsetDateTime, CounterfactualReplayContractError> {
    let parsed = OffsetDateTime::parse(value, &Rfc3339).map_err(|_| {
        CounterfactualReplayContractError::invalid_payload(format!(
            "timestamp `{value}` must be RFC3339 UTC"
        ))
    })?;
    if parsed.offset() != UtcOffset::UTC {
        return Err(CounterfactualReplayContractError::invalid_payload(
            "timestamps must use UTC `Z` offset",
        ));
    }
    Ok(parsed)
}

pub fn compose_counterfactual_replay_run_id(
    candidate_id: &str,
    started_at_utc: &str,
) -> Result<String, CounterfactualReplayContractError> {
    let normalized_candidate_id = normalize_research_identifier(candidate_id);
    if normalized_candidate_id.is_empty() {
        return Err(
            CounterfactualReplayContractError::invalid_payload_with_issues(
                "candidate_id cannot be blank",
                vec![CounterfactualReplayValidationIssue {
                    field: "candidate_id".to_string(),
                    code: CounterfactualReplayReasonCode::InvalidPayload.code(),
                    message: "candidate_id cannot be blank".to_string(),
                }],
            ),
        );
    }
    let started_at = parse_counterfactual_replay_utc_timestamp(started_at_utc)?;
    Ok(format!(
        "{}::{}",
        normalized_candidate_id,
        started_at.unix_timestamp_nanos()
    ))
}

pub fn evaluate_counterfactual_replay_gate(
    baseline_net_pnl: f64,
    stressed_net_pnl: f64,
) -> Result<CounterfactualReplayGateEvaluation, CounterfactualReplayContractError> {
    let mut field_errors = Vec::new();
    if !baseline_net_pnl.is_finite() || baseline_net_pnl <= 0.0 {
        field_errors.push(CounterfactualReplayValidationIssue {
            field: "baseline_net_pnl".to_string(),
            code: CounterfactualReplayReasonCode::BaselineUnavailable.code(),
            message: "baseline_net_pnl must be finite and > 0".to_string(),
        });
    }
    if !stressed_net_pnl.is_finite() {
        field_errors.push(CounterfactualReplayValidationIssue {
            field: "stressed_net_pnl".to_string(),
            code: CounterfactualReplayReasonCode::StateUnavailable.code(),
            message: "stressed_net_pnl must be finite".to_string(),
        });
    }
    if !field_errors.is_empty() {
        return Err(
            CounterfactualReplayContractError::invalid_payload_with_issues(
                "counterfactual replay gate evaluation failed",
                field_errors,
            ),
        );
    }

    let raw_degradation_pct = ((stressed_net_pnl - baseline_net_pnl) / baseline_net_pnl) * 100.0;
    // Normalize to avoid boundary flips from sub-nanopoint floating-point noise.
    let degradation_pct = (raw_degradation_pct * 1_000_000_000.0).round() / 1_000_000_000.0;
    let (gate_outcome, reason_code) = if degradation_pct < FR46_DEGRADATION_DENY_THRESHOLD_PCT {
        (
            CounterfactualReplayGateOutcome::Deny,
            CounterfactualReplayReasonCode::ToleranceBreached
                .code()
                .to_string(),
        )
    } else {
        (
            CounterfactualReplayGateOutcome::Allow,
            CounterfactualReplayReasonCode::ToleranceSatisfied
                .code()
                .to_string(),
        )
    };

    Ok(CounterfactualReplayGateEvaluation {
        degradation_pct,
        gate_outcome,
        reason_code,
    })
}

pub fn validate_counterfactual_replay_summary(
    summary: &CounterfactualReplaySummary,
) -> Result<(), CounterfactualReplayContractError> {
    let mut field_errors = Vec::new();
    if normalize_research_identifier(&summary.run_id).is_empty() {
        field_errors.push(CounterfactualReplayValidationIssue {
            field: "run_id".to_string(),
            code: CounterfactualReplayReasonCode::InvalidPayload.code(),
            message: "run_id is required".to_string(),
        });
    }
    if CounterfactualReplayReasonCode::parse(&summary.reason_code).is_err() {
        field_errors.push(CounterfactualReplayValidationIssue {
            field: "reason_code".to_string(),
            code: CounterfactualReplayReasonCode::InvalidPayload.code(),
            message: "reason_code is not recognized".to_string(),
        });
    }
    if !summary.delayed_exit_net_pnl.is_finite() {
        field_errors.push(CounterfactualReplayValidationIssue {
            field: "delayed_exit_net_pnl".to_string(),
            code: CounterfactualReplayReasonCode::StateUnavailable.code(),
            message: "delayed_exit_net_pnl must be finite".to_string(),
        });
    }
    if !summary.tolerance_threshold_pct.is_finite() {
        field_errors.push(CounterfactualReplayValidationIssue {
            field: "tolerance_threshold_pct".to_string(),
            code: CounterfactualReplayReasonCode::InvalidPayload.code(),
            message: "tolerance_threshold_pct must be finite".to_string(),
        });
    }
    if (summary.tolerance_threshold_pct - FR46_DEGRADATION_DENY_THRESHOLD_PCT).abs() > f64::EPSILON
    {
        field_errors.push(CounterfactualReplayValidationIssue {
            field: "tolerance_threshold_pct".to_string(),
            code: CounterfactualReplayReasonCode::InvalidPayload.code(),
            message: format!(
                "tolerance_threshold_pct must equal {FR46_DEGRADATION_DENY_THRESHOLD_PCT}"
            ),
        });
    }

    let gate_evaluation =
        evaluate_counterfactual_replay_gate(summary.baseline_net_pnl, summary.stressed_net_pnl)?;
    if (gate_evaluation.degradation_pct - summary.degradation_pct).abs() > 1e-9 {
        field_errors.push(CounterfactualReplayValidationIssue {
            field: "degradation_pct".to_string(),
            code: CounterfactualReplayReasonCode::InvalidPayload.code(),
            message: "degradation_pct does not match FR46 formula".to_string(),
        });
    }
    if gate_evaluation.gate_outcome != summary.gate_outcome {
        field_errors.push(CounterfactualReplayValidationIssue {
            field: "gate_outcome".to_string(),
            code: CounterfactualReplayReasonCode::InvalidPayload.code(),
            message: "gate_outcome does not match FR46 tolerance evaluation".to_string(),
        });
    }
    if normalize_research_identifier(&gate_evaluation.reason_code)
        != normalize_research_identifier(&summary.reason_code)
    {
        field_errors.push(CounterfactualReplayValidationIssue {
            field: "reason_code".to_string(),
            code: CounterfactualReplayReasonCode::InvalidPayload.code(),
            message: "reason_code does not match FR46 tolerance evaluation".to_string(),
        });
    }

    let mut scenario_kinds = summary
        .scenarios
        .iter()
        .map(|scenario| scenario.scenario)
        .collect::<Vec<_>>();
    scenario_kinds.sort();
    scenario_kinds.dedup();
    for required in FR46_REQUIRED_COUNTERFACTUAL_SCENARIOS {
        if !scenario_kinds.contains(&required) {
            field_errors.push(CounterfactualReplayValidationIssue {
                field: "scenarios".to_string(),
                code: CounterfactualReplayReasonCode::ScenarioIncomplete.code(),
                message: format!("missing required scenario `{}`", required.as_str()),
            });
        }
        let duplicate_count = summary
            .scenarios
            .iter()
            .filter(|scenario| scenario.scenario == required)
            .count();
        if duplicate_count > 1 {
            field_errors.push(CounterfactualReplayValidationIssue {
                field: "scenarios".to_string(),
                code: CounterfactualReplayReasonCode::InvalidPayload.code(),
                message: format!("scenario `{}` must appear exactly once", required.as_str()),
            });
        }
    }
    if summary.scenarios.len() != FR46_REQUIRED_COUNTERFACTUAL_SCENARIOS.len() {
        field_errors.push(CounterfactualReplayValidationIssue {
            field: "scenarios".to_string(),
            code: CounterfactualReplayReasonCode::InvalidPayload.code(),
            message: format!(
                "scenarios must contain exactly {} entries",
                FR46_REQUIRED_COUNTERFACTUAL_SCENARIOS.len()
            ),
        });
    }

    if let Some(baseline_scenario) = summary
        .scenarios
        .iter()
        .find(|scenario| scenario.scenario == CounterfactualReplayScenarioKind::Baseline)
        && (baseline_scenario.net_pnl - summary.baseline_net_pnl).abs() > 1e-9
    {
        field_errors.push(CounterfactualReplayValidationIssue {
            field: "baseline_net_pnl".to_string(),
            code: CounterfactualReplayReasonCode::InvalidPayload.code(),
            message: "baseline_net_pnl must match baseline scenario net_pnl".to_string(),
        });
    }
    if let Some(stressed_scenario) = summary
        .scenarios
        .iter()
        .find(|scenario| scenario.scenario == CounterfactualReplayScenarioKind::StressedExecution)
    {
        if (stressed_scenario.net_pnl - summary.stressed_net_pnl).abs() > 1e-9 {
            field_errors.push(CounterfactualReplayValidationIssue {
                field: "stressed_net_pnl".to_string(),
                code: CounterfactualReplayReasonCode::InvalidPayload.code(),
                message: "stressed_net_pnl must match stressed_execution scenario net_pnl"
                    .to_string(),
            });
        }
        if stressed_scenario.gate_outcome != summary.gate_outcome {
            field_errors.push(CounterfactualReplayValidationIssue {
                field: "scenarios.stressed_execution.gate_outcome".to_string(),
                code: CounterfactualReplayReasonCode::InvalidPayload.code(),
                message: "stressed_execution scenario gate_outcome must match summary gate_outcome"
                    .to_string(),
            });
        }
        if normalize_research_identifier(&stressed_scenario.reason_code)
            != normalize_research_identifier(&summary.reason_code)
        {
            field_errors.push(CounterfactualReplayValidationIssue {
                field: "scenarios.stressed_execution.reason_code".to_string(),
                code: CounterfactualReplayReasonCode::InvalidPayload.code(),
                message: "stressed_execution scenario reason_code must match summary reason_code"
                    .to_string(),
            });
        }
    }
    if let Some(delayed_exit_scenario) = summary
        .scenarios
        .iter()
        .find(|scenario| scenario.scenario == CounterfactualReplayScenarioKind::DelayedExit)
        && (delayed_exit_scenario.net_pnl - summary.delayed_exit_net_pnl).abs() > 1e-9
    {
        field_errors.push(CounterfactualReplayValidationIssue {
            field: "delayed_exit_net_pnl".to_string(),
            code: CounterfactualReplayReasonCode::InvalidPayload.code(),
            message: "delayed_exit_net_pnl must match delayed_exit scenario net_pnl".to_string(),
        });
    }

    for (index, scenario) in summary.scenarios.iter().enumerate() {
        let expected_parameters = fr46_scenario_parameters(scenario.scenario);
        if !scenario.net_pnl.is_finite() {
            field_errors.push(CounterfactualReplayValidationIssue {
                field: format!("scenarios[{index}].net_pnl"),
                code: CounterfactualReplayReasonCode::StateUnavailable.code(),
                message: "net_pnl must be finite".to_string(),
            });
        }
        if let Some(degradation_pct) = scenario.degradation_pct
            && !degradation_pct.is_finite()
        {
            field_errors.push(CounterfactualReplayValidationIssue {
                field: format!("scenarios[{index}].degradation_pct"),
                code: CounterfactualReplayReasonCode::StateUnavailable.code(),
                message: "degradation_pct must be finite when present".to_string(),
            });
        }
        if CounterfactualReplayReasonCode::parse(&scenario.reason_code).is_err() {
            field_errors.push(CounterfactualReplayValidationIssue {
                field: format!("scenarios[{index}].reason_code"),
                code: CounterfactualReplayReasonCode::InvalidPayload.code(),
                message: "reason_code is not recognized".to_string(),
            });
        }
        if !scenario.parameters.slippage_multiplier.is_finite()
            || scenario.parameters.slippage_multiplier <= 0.0
        {
            field_errors.push(CounterfactualReplayValidationIssue {
                field: format!("scenarios[{index}].parameters.slippage_multiplier"),
                code: CounterfactualReplayReasonCode::InvalidPayload.code(),
                message: "slippage_multiplier must be finite and > 0".to_string(),
            });
        } else if (scenario.parameters.slippage_multiplier
            - expected_parameters.slippage_multiplier)
            .abs()
            > f64::EPSILON
        {
            field_errors.push(CounterfactualReplayValidationIssue {
                field: format!("scenarios[{index}].parameters.slippage_multiplier"),
                code: CounterfactualReplayReasonCode::InvalidPayload.code(),
                message: format!(
                    "slippage_multiplier must equal {} for `{}`",
                    expected_parameters.slippage_multiplier,
                    scenario.scenario.as_str()
                ),
            });
        }
        if !scenario.parameters.fill_rate_multiplier.is_finite()
            || scenario.parameters.fill_rate_multiplier <= 0.0
            || scenario.parameters.fill_rate_multiplier > 1.0
        {
            field_errors.push(CounterfactualReplayValidationIssue {
                field: format!("scenarios[{index}].parameters.fill_rate_multiplier"),
                code: CounterfactualReplayReasonCode::InvalidPayload.code(),
                message: "fill_rate_multiplier must be finite and in (0, 1]".to_string(),
            });
        } else if (scenario.parameters.fill_rate_multiplier
            - expected_parameters.fill_rate_multiplier)
            .abs()
            > f64::EPSILON
        {
            field_errors.push(CounterfactualReplayValidationIssue {
                field: format!("scenarios[{index}].parameters.fill_rate_multiplier"),
                code: CounterfactualReplayReasonCode::InvalidPayload.code(),
                message: format!(
                    "fill_rate_multiplier must equal {} for `{}`",
                    expected_parameters.fill_rate_multiplier,
                    scenario.scenario.as_str()
                ),
            });
        }
        if scenario.parameters.exit_delay_seconds < 0 {
            field_errors.push(CounterfactualReplayValidationIssue {
                field: format!("scenarios[{index}].parameters.exit_delay_seconds"),
                code: CounterfactualReplayReasonCode::InvalidPayload.code(),
                message: "exit_delay_seconds cannot be negative".to_string(),
            });
        } else if scenario.parameters.exit_delay_seconds != expected_parameters.exit_delay_seconds {
            field_errors.push(CounterfactualReplayValidationIssue {
                field: format!("scenarios[{index}].parameters.exit_delay_seconds"),
                code: CounterfactualReplayReasonCode::InvalidPayload.code(),
                message: format!(
                    "exit_delay_seconds must equal {} for `{}`",
                    expected_parameters.exit_delay_seconds,
                    scenario.scenario.as_str()
                ),
            });
        }
    }

    if !field_errors.is_empty() {
        return Err(
            CounterfactualReplayContractError::invalid_payload_with_issues(
                "counterfactual replay summary failed validation",
                field_errors,
            ),
        );
    }
    Ok(())
}

pub fn canonicalize_counterfactual_replay_run_record(
    record: &CounterfactualReplayRunRecord,
) -> Result<CounterfactualReplayRunRecord, CounterfactualReplayContractError> {
    let mut canonical = CounterfactualReplayRunRecord {
        run_id: normalize_research_identifier(&record.run_id),
        candidate_id: normalize_research_identifier(&record.candidate_id),
        validation_run_id: normalize_research_identifier(&record.validation_run_id),
        run_state: record.run_state,
        reason_code: normalize_research_identifier(&record.reason_code),
        scenario_results: record.scenario_results.clone(),
        replay_summary: record.replay_summary.clone(),
        actor_id: record.actor_id.trim().to_string(),
        correlation_id: record.correlation_id.trim().to_string(),
        started_at_utc: record.started_at_utc.trim().to_string(),
        completed_at_utc: record
            .completed_at_utc
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
    };
    canonical.scenario_results.iter_mut().for_each(|scenario| {
        scenario.reason_code = normalize_research_identifier(&scenario.reason_code);
    });
    canonical
        .scenario_results
        .sort_by(|left, right| left.scenario.cmp(&right.scenario));
    canonical.replay_summary.run_id =
        normalize_research_identifier(&canonical.replay_summary.run_id);
    canonical.replay_summary.reason_code =
        normalize_research_identifier(&canonical.replay_summary.reason_code);
    canonical
        .replay_summary
        .scenarios
        .iter_mut()
        .for_each(|scenario| {
            scenario.reason_code = normalize_research_identifier(&scenario.reason_code);
        });
    canonical
        .replay_summary
        .scenarios
        .sort_by(|left, right| left.scenario.cmp(&right.scenario));

    validate_counterfactual_replay_run_record(&canonical)?;
    Ok(canonical)
}

pub fn validate_counterfactual_replay_run_record(
    record: &CounterfactualReplayRunRecord,
) -> Result<(), CounterfactualReplayContractError> {
    let mut field_errors = Vec::new();
    if record.run_id.trim().is_empty() {
        field_errors.push(CounterfactualReplayValidationIssue {
            field: "run_id".to_string(),
            code: CounterfactualReplayReasonCode::InvalidPayload.code(),
            message: "run_id is required".to_string(),
        });
    }
    if record.candidate_id.trim().is_empty() {
        field_errors.push(CounterfactualReplayValidationIssue {
            field: "candidate_id".to_string(),
            code: CounterfactualReplayReasonCode::InvalidPayload.code(),
            message: "candidate_id is required".to_string(),
        });
    }
    if record.validation_run_id.trim().is_empty() {
        field_errors.push(CounterfactualReplayValidationIssue {
            field: "validation_run_id".to_string(),
            code: CounterfactualReplayReasonCode::InvalidPayload.code(),
            message: "validation_run_id is required".to_string(),
        });
    }
    if record.reason_code.trim().is_empty() {
        field_errors.push(CounterfactualReplayValidationIssue {
            field: "reason_code".to_string(),
            code: CounterfactualReplayReasonCode::InvalidPayload.code(),
            message: "reason_code is required".to_string(),
        });
    } else if CounterfactualReplayReasonCode::parse(&record.reason_code).is_err() {
        field_errors.push(CounterfactualReplayValidationIssue {
            field: "reason_code".to_string(),
            code: CounterfactualReplayReasonCode::InvalidPayload.code(),
            message: "reason_code is not recognized".to_string(),
        });
    }
    if record.actor_id.trim().is_empty() {
        field_errors.push(CounterfactualReplayValidationIssue {
            field: "actor_id".to_string(),
            code: CounterfactualReplayReasonCode::InvalidPayload.code(),
            message: "actor_id is required".to_string(),
        });
    }
    if record.correlation_id.trim().is_empty() {
        field_errors.push(CounterfactualReplayValidationIssue {
            field: "correlation_id".to_string(),
            code: CounterfactualReplayReasonCode::InvalidPayload.code(),
            message: "correlation_id is required".to_string(),
        });
    }
    if parse_counterfactual_replay_utc_timestamp(&record.started_at_utc).is_err() {
        field_errors.push(CounterfactualReplayValidationIssue {
            field: "started_at_utc".to_string(),
            code: CounterfactualReplayReasonCode::InvalidPayload.code(),
            message: "started_at_utc must be RFC3339 UTC".to_string(),
        });
    }
    if let Some(completed_at_utc) = record.completed_at_utc.as_deref() {
        if parse_counterfactual_replay_utc_timestamp(completed_at_utc).is_err() {
            field_errors.push(CounterfactualReplayValidationIssue {
                field: "completed_at_utc".to_string(),
                code: CounterfactualReplayReasonCode::InvalidPayload.code(),
                message: "completed_at_utc must be RFC3339 UTC".to_string(),
            });
        } else if let (Ok(started), Ok(completed)) = (
            parse_counterfactual_replay_utc_timestamp(&record.started_at_utc),
            parse_counterfactual_replay_utc_timestamp(completed_at_utc),
        ) && completed < started
        {
            field_errors.push(CounterfactualReplayValidationIssue {
                field: "completed_at_utc".to_string(),
                code: CounterfactualReplayReasonCode::InvalidPayload.code(),
                message: "completed_at_utc must be >= started_at_utc".to_string(),
            });
        }
    }
    if record.scenario_results.is_empty() {
        field_errors.push(CounterfactualReplayValidationIssue {
            field: "scenario_results".to_string(),
            code: CounterfactualReplayReasonCode::ScenarioIncomplete.code(),
            message: "scenario_results must contain required FR46 scenarios".to_string(),
        });
    }

    if let Err(error) = validate_counterfactual_replay_summary(&record.replay_summary) {
        field_errors.extend(error.field_errors);
    }

    let mut scenario_kinds = record
        .scenario_results
        .iter()
        .map(|scenario| scenario.scenario)
        .collect::<Vec<_>>();
    scenario_kinds.sort();
    scenario_kinds.dedup();
    for required in FR46_REQUIRED_COUNTERFACTUAL_SCENARIOS {
        if !scenario_kinds.contains(&required) {
            field_errors.push(CounterfactualReplayValidationIssue {
                field: "scenario_results".to_string(),
                code: CounterfactualReplayReasonCode::ScenarioIncomplete.code(),
                message: format!("scenario_results missing `{}`", required.as_str()),
            });
        }
    }

    if normalize_research_identifier(&record.replay_summary.run_id)
        != normalize_research_identifier(&record.run_id)
    {
        field_errors.push(CounterfactualReplayValidationIssue {
            field: "replay_summary.run_id".to_string(),
            code: CounterfactualReplayReasonCode::InvalidPayload.code(),
            message: "replay_summary.run_id must match run_id".to_string(),
        });
    }
    if record.run_state == CounterfactualReplayRunState::Denied
        && record.replay_summary.gate_outcome != CounterfactualReplayGateOutcome::Deny
    {
        field_errors.push(CounterfactualReplayValidationIssue {
            field: "run_state".to_string(),
            code: CounterfactualReplayReasonCode::InvalidPayload.code(),
            message: "denied run_state requires deny gate_outcome".to_string(),
        });
    }
    if record.run_state == CounterfactualReplayRunState::Completed
        && record.replay_summary.gate_outcome == CounterfactualReplayGateOutcome::Deny
    {
        field_errors.push(CounterfactualReplayValidationIssue {
            field: "run_state".to_string(),
            code: CounterfactualReplayReasonCode::InvalidPayload.code(),
            message: "completed run_state cannot carry deny gate_outcome".to_string(),
        });
    }

    if !field_errors.is_empty() {
        return Err(
            CounterfactualReplayContractError::invalid_payload_with_issues(
                "counterfactual replay run payload failed validation",
                field_errors,
            ),
        );
    }
    Ok(())
}

pub fn validate_counterfactual_replay_summary_for_allow_path(
    summary: &Value,
) -> Result<(), PromotionDecisionContractError> {
    if summary
        .as_object()
        .and_then(|map| map.get("status"))
        .and_then(Value::as_str)
        .map(normalize_research_identifier)
        .as_deref()
        == Some("deferred_to_story_6_6")
    {
        return Err(PromotionDecisionContractError::invalid_payload_with_issues(
            "counterfactual replay summary placeholder is not allowed for promote allow decisions",
            vec![PromotionDecisionValidationIssue {
                field: "counterfactual_replay_summary.status".to_string(),
                code: CounterfactualReplayReasonCode::PlaceholderRejected.code(),
                message: "counterfactual_replay_summary.status cannot be `deferred_to_story_6_6`"
                    .to_string(),
            }],
        ));
    }
    let parsed =
        serde_json::from_value::<CounterfactualReplaySummary>(summary.clone()).map_err(|_| {
            PromotionDecisionContractError::invalid_payload_with_issues(
                "counterfactual_replay_summary must be canonical replay summary payload",
                vec![PromotionDecisionValidationIssue {
                    field: "counterfactual_replay_summary".to_string(),
                    code: CounterfactualReplayReasonCode::InvalidPayload.code(),
                    message:
                        "counterfactual_replay_summary must be structured replay summary object"
                            .to_string(),
                }],
            )
        })?;

    if let Err(error) = validate_counterfactual_replay_summary(&parsed) {
        return Err(PromotionDecisionContractError::invalid_payload_with_issues(
            error.message,
            error
                .field_errors
                .into_iter()
                .map(|issue| PromotionDecisionValidationIssue {
                    field: format!("counterfactual_replay_summary.{}", issue.field),
                    code: issue.code,
                    message: issue.message,
                })
                .collect(),
        ));
    }

    if parsed.gate_outcome != CounterfactualReplayGateOutcome::Allow {
        return Err(PromotionDecisionContractError::invalid_payload_with_issues(
            "counterfactual replay gate outcome must allow before promotion allow-path decisions",
            vec![PromotionDecisionValidationIssue {
                field: "counterfactual_replay_summary.gate_outcome".to_string(),
                code: PromotionDecisionReasonCode::ReplayGateDenied.code(),
                message: "counterfactual replay gate outcome must be `allow`".to_string(),
            }],
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum PromotionLifecycleAction {
    Promote,
    Pause,
    Retire,
}

impl PromotionLifecycleAction {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Promote => "promote",
            Self::Pause => "pause",
            Self::Retire => "retire",
        }
    }

    pub fn parse(value: &str) -> Result<Self, PromotionDecisionContractError> {
        match normalize_research_identifier(value).as_str() {
            "promote" => Ok(Self::Promote),
            "pause" => Ok(Self::Pause),
            "retire" => Ok(Self::Retire),
            _ => Err(PromotionDecisionContractError::invalid_payload_with_issues(
                "unsupported lifecycle action",
                vec![PromotionDecisionValidationIssue {
                    field: "lifecycle_action".to_string(),
                    code: PromotionDecisionReasonCode::UnsupportedAction.code(),
                    message: format!(
                        "lifecycle_action must be one of: promote, pause, retire (received `{value}`)"
                    ),
                }],
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PromotionDecisionState {
    Allowed,
    Denied,
}

impl PromotionDecisionState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allowed => "allowed",
            Self::Denied => "denied",
        }
    }

    pub fn parse(value: &str) -> Result<Self, PromotionDecisionContractError> {
        match normalize_research_identifier(value).as_str() {
            "allowed" => Ok(Self::Allowed),
            "denied" => Ok(Self::Denied),
            _ => Err(PromotionDecisionContractError::invalid_payload(format!(
                "unknown promotion decision state `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PromotionDecisionReasonCode {
    DecisionStarted,
    DecisionAllowed,
    DecisionDenied,
    DecisionRead,
    DecisionListed,
    InvalidPayload,
    UnauthorizedRole,
    DecisionNotFound,
    UnsupportedAction,
    MissingEvidence,
    ThresholdFailed,
    GateDenied,
    ReplayGateDenied,
    ApprovalRequired,
    ApprovalInvalidState,
    DependencyUnavailable,
    StateUnavailable,
    PersistenceUnavailable,
}

impl PromotionDecisionReasonCode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::DecisionStarted => "promotion_decision_started",
            Self::DecisionAllowed => "promotion_decision_allowed",
            Self::DecisionDenied => "promotion_decision_denied",
            Self::DecisionRead => "promotion_decision_read",
            Self::DecisionListed => "promotion_decision_listed",
            Self::InvalidPayload => "promotion_decision_invalid_payload",
            Self::UnauthorizedRole => "promotion_decision_unauthorized_role",
            Self::DecisionNotFound => "promotion_decision_not_found",
            Self::UnsupportedAction => "promotion_decision_unsupported_action",
            Self::MissingEvidence => "promotion_decision_missing_evidence",
            Self::ThresholdFailed => "promotion_decision_threshold_failed",
            Self::GateDenied => "promotion_decision_gate_denied",
            Self::ReplayGateDenied => "promotion_decision_replay_gate_denied",
            Self::ApprovalRequired => "promotion_decision_approval_required",
            Self::ApprovalInvalidState => "promotion_decision_approval_invalid_state",
            Self::DependencyUnavailable => "promotion_decision_dependency_unavailable",
            Self::StateUnavailable => "promotion_decision_state_unavailable",
            Self::PersistenceUnavailable => "promotion_decision_persistence_unavailable",
        }
    }

    pub fn parse(value: &str) -> Result<Self, PromotionDecisionContractError> {
        match value {
            "promotion_decision_started" => Ok(Self::DecisionStarted),
            "promotion_decision_allowed" => Ok(Self::DecisionAllowed),
            "promotion_decision_denied" => Ok(Self::DecisionDenied),
            "promotion_decision_read" => Ok(Self::DecisionRead),
            "promotion_decision_listed" => Ok(Self::DecisionListed),
            "promotion_decision_invalid_payload" => Ok(Self::InvalidPayload),
            "promotion_decision_unauthorized_role" => Ok(Self::UnauthorizedRole),
            "promotion_decision_not_found" => Ok(Self::DecisionNotFound),
            "promotion_decision_unsupported_action" => Ok(Self::UnsupportedAction),
            "promotion_decision_missing_evidence" => Ok(Self::MissingEvidence),
            "promotion_decision_threshold_failed" => Ok(Self::ThresholdFailed),
            "promotion_decision_gate_denied" => Ok(Self::GateDenied),
            "promotion_decision_replay_gate_denied" => Ok(Self::ReplayGateDenied),
            "promotion_decision_approval_required" => Ok(Self::ApprovalRequired),
            "promotion_decision_approval_invalid_state" => Ok(Self::ApprovalInvalidState),
            "promotion_decision_dependency_unavailable" => Ok(Self::DependencyUnavailable),
            "promotion_decision_state_unavailable" => Ok(Self::StateUnavailable),
            "promotion_decision_persistence_unavailable" => Ok(Self::PersistenceUnavailable),
            _ => Err(PromotionDecisionContractError::invalid_payload(format!(
                "unknown promotion decision reason code `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PromotionThresholdReasonCode {
    Passed,
    Failed,
}

impl PromotionThresholdReasonCode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Passed => "promotion_threshold_passed",
            Self::Failed => "promotion_threshold_failed",
        }
    }

    pub fn parse(value: &str) -> Result<Self, PromotionDecisionContractError> {
        match value {
            "promotion_threshold_passed" => Ok(Self::Passed),
            "promotion_threshold_failed" => Ok(Self::Failed),
            _ => Err(PromotionDecisionContractError::invalid_payload(format!(
                "unknown promotion threshold reason code `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PromotionDecisionValidationIssue {
    pub field: String,
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PromotionDecisionContractError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<PromotionDecisionValidationIssue>,
}

impl PromotionDecisionContractError {
    pub fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: PromotionDecisionReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn invalid_payload_with_issues(
        message: impl Into<String>,
        field_errors: Vec<PromotionDecisionValidationIssue>,
    ) -> Self {
        Self {
            code: PromotionDecisionReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PromotionThresholdDefinition {
    pub metric_key: String,
    pub comparator: ValidationGateComparator,
    pub threshold_value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PromotionThresholdOutcome {
    pub metric_key: String,
    pub comparator: ValidationGateComparator,
    pub threshold_value: f64,
    pub observed_value: f64,
    pub passed: bool,
    pub reason_code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PromotionDecisionRecord {
    pub decision_id: String,
    pub candidate_id: String,
    pub validation_run_id: String,
    pub lifecycle_action: PromotionLifecycleAction,
    pub decision_state: PromotionDecisionState,
    pub reason_code: String,
    pub observed_metrics: Value,
    pub evidence_packet: Value,
    pub threshold_results: Vec<PromotionThresholdOutcome>,
    pub missing_evidence_fields: Vec<String>,
    pub gate_evaluation: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow_readiness: Option<Value>,
    pub actor_id: String,
    pub correlation_id: String,
    pub decided_at_utc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_reference: Option<String>,
}

pub fn parse_promotion_utc_timestamp(
    value: &str,
) -> Result<OffsetDateTime, PromotionDecisionContractError> {
    let parsed = OffsetDateTime::parse(value, &Rfc3339).map_err(|_| {
        PromotionDecisionContractError::invalid_payload(format!(
            "timestamp `{value}` must be RFC3339 UTC"
        ))
    })?;
    if parsed.offset() != UtcOffset::UTC {
        return Err(PromotionDecisionContractError::invalid_payload(
            "timestamps must use UTC `Z` offset",
        ));
    }
    Ok(parsed)
}

pub fn compose_promotion_decision_id(
    candidate_id: &str,
    decided_at_utc: &str,
) -> Result<String, PromotionDecisionContractError> {
    let normalized_candidate_id = normalize_research_identifier(candidate_id);
    if normalized_candidate_id.is_empty() {
        return Err(PromotionDecisionContractError::invalid_payload_with_issues(
            "candidate_id cannot be blank",
            vec![PromotionDecisionValidationIssue {
                field: "candidate_id".to_string(),
                code: PromotionDecisionReasonCode::InvalidPayload.code(),
                message: "candidate_id cannot be blank".to_string(),
            }],
        ));
    }
    let decided_at = parse_promotion_utc_timestamp(decided_at_utc)?;
    Ok(format!(
        "{}::{}",
        normalized_candidate_id,
        decided_at.unix_timestamp_nanos()
    ))
}

pub fn evaluate_promotion_thresholds(
    thresholds: &[PromotionThresholdDefinition],
    observed_metrics: &Value,
) -> Result<Vec<PromotionThresholdOutcome>, PromotionDecisionContractError> {
    if thresholds.is_empty() {
        return Err(PromotionDecisionContractError::invalid_payload_with_issues(
            "at least one promotion threshold is required",
            vec![PromotionDecisionValidationIssue {
                field: "thresholds".to_string(),
                code: PromotionDecisionReasonCode::InvalidPayload.code(),
                message: "at least one promotion threshold is required".to_string(),
            }],
        ));
    }
    let Value::Object(observed_map) = observed_metrics else {
        return Err(PromotionDecisionContractError::invalid_payload_with_issues(
            "observed_metrics must be a JSON object",
            vec![PromotionDecisionValidationIssue {
                field: "observed_metrics".to_string(),
                code: PromotionDecisionReasonCode::InvalidPayload.code(),
                message: "observed_metrics must be a JSON object".to_string(),
            }],
        ));
    };

    let mut outcomes = Vec::with_capacity(thresholds.len());
    let mut field_errors = Vec::new();
    for (index, threshold) in thresholds.iter().enumerate() {
        let metric_key = normalize_research_identifier(&threshold.metric_key);
        if metric_key.is_empty() {
            field_errors.push(PromotionDecisionValidationIssue {
                field: format!("thresholds[{index}].metric_key"),
                code: PromotionDecisionReasonCode::InvalidPayload.code(),
                message: "metric_key is required".to_string(),
            });
            continue;
        }
        if !threshold.threshold_value.is_finite() {
            field_errors.push(PromotionDecisionValidationIssue {
                field: format!("thresholds[{index}].threshold_value"),
                code: PromotionDecisionReasonCode::InvalidPayload.code(),
                message: "threshold_value must be finite".to_string(),
            });
            continue;
        }
        let observed_value = observed_map
            .iter()
            .find(|(key, _)| normalize_research_identifier(key) == metric_key)
            .and_then(|(_, value)| value.as_f64());
        let Some(observed_value) = observed_value else {
            field_errors.push(PromotionDecisionValidationIssue {
                field: format!("observed_metrics.{metric_key}"),
                code: PromotionDecisionReasonCode::StateUnavailable.code(),
                message: format!("observed metric `{metric_key}` is unavailable"),
            });
            continue;
        };
        if !observed_value.is_finite() {
            field_errors.push(PromotionDecisionValidationIssue {
                field: format!("observed_metrics.{metric_key}"),
                code: PromotionDecisionReasonCode::StateUnavailable.code(),
                message: format!("observed metric `{metric_key}` must be finite"),
            });
            continue;
        }

        let passed = match threshold.comparator {
            ValidationGateComparator::Lt => observed_value < threshold.threshold_value,
            ValidationGateComparator::Lte => observed_value <= threshold.threshold_value,
            ValidationGateComparator::Gt => observed_value > threshold.threshold_value,
            ValidationGateComparator::Gte => observed_value >= threshold.threshold_value,
        };
        outcomes.push(PromotionThresholdOutcome {
            metric_key,
            comparator: threshold.comparator,
            threshold_value: threshold.threshold_value,
            observed_value,
            passed,
            reason_code: if passed {
                PromotionThresholdReasonCode::Passed.code().to_string()
            } else {
                PromotionThresholdReasonCode::Failed.code().to_string()
            },
        });
    }
    if !field_errors.is_empty() {
        return Err(PromotionDecisionContractError::invalid_payload_with_issues(
            "promotion threshold evaluation failed",
            field_errors,
        ));
    }

    outcomes.sort_by(|left, right| {
        left.metric_key
            .cmp(&right.metric_key)
            .then_with(|| left.comparator.as_str().cmp(right.comparator.as_str()))
    });
    Ok(outcomes)
}

pub fn validate_promotion_evidence_packet(
    action: PromotionLifecycleAction,
    evidence_packet: &Value,
) -> Result<Vec<String>, PromotionDecisionContractError> {
    let Value::Object(map) = evidence_packet else {
        return Err(PromotionDecisionContractError::invalid_payload_with_issues(
            "evidence_packet must be a JSON object",
            vec![PromotionDecisionValidationIssue {
                field: "evidence_packet".to_string(),
                code: PromotionDecisionReasonCode::InvalidPayload.code(),
                message: "evidence_packet must be a JSON object".to_string(),
            }],
        ));
    };
    if action != PromotionLifecycleAction::Promote {
        return Ok(Vec::new());
    }

    let missing = FR45_REQUIRED_PROMOTION_PACKET_FIELDS
        .iter()
        .filter_map(|field| {
            let is_missing = match map.get(*field) {
                None | Some(Value::Null) => true,
                Some(Value::String(text)) => text.trim().is_empty(),
                Some(Value::Object(object)) => object.is_empty(),
                Some(Value::Array(array)) => array.is_empty(),
                Some(_) => false,
            };
            if is_missing {
                Some((*field).to_string())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    Ok(missing)
}

pub fn canonicalize_promotion_decision_record(
    record: &PromotionDecisionRecord,
) -> Result<PromotionDecisionRecord, PromotionDecisionContractError> {
    let mut canonical = PromotionDecisionRecord {
        decision_id: normalize_research_identifier(&record.decision_id),
        candidate_id: normalize_research_identifier(&record.candidate_id),
        validation_run_id: normalize_research_identifier(&record.validation_run_id),
        lifecycle_action: record.lifecycle_action,
        decision_state: record.decision_state,
        reason_code: normalize_research_identifier(&record.reason_code),
        observed_metrics: record.observed_metrics.clone(),
        evidence_packet: record.evidence_packet.clone(),
        threshold_results: record.threshold_results.clone(),
        missing_evidence_fields: record
            .missing_evidence_fields
            .iter()
            .map(|field| normalize_research_identifier(field))
            .filter(|field| !field.is_empty())
            .collect(),
        gate_evaluation: record.gate_evaluation.clone(),
        shadow_readiness: record.shadow_readiness.clone(),
        actor_id: record.actor_id.trim().to_string(),
        correlation_id: record.correlation_id.trim().to_string(),
        decided_at_utc: record.decided_at_utc.trim().to_string(),
        approval_request_id: record
            .approval_request_id
            .as_ref()
            .map(|value| normalize_research_identifier(value))
            .filter(|value| !value.is_empty()),
        approval_reference: record
            .approval_reference
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
    };
    canonical.threshold_results.sort_by(|left, right| {
        left.metric_key
            .cmp(&right.metric_key)
            .then_with(|| left.comparator.as_str().cmp(right.comparator.as_str()))
    });
    canonical.missing_evidence_fields.sort();
    canonical.missing_evidence_fields.dedup();

    validate_promotion_decision_record(&canonical)?;
    Ok(canonical)
}

pub fn validate_promotion_decision_record(
    record: &PromotionDecisionRecord,
) -> Result<(), PromotionDecisionContractError> {
    let mut field_errors = Vec::new();
    validate_non_empty_promotion_field(&mut field_errors, "decision_id", &record.decision_id);
    validate_non_empty_promotion_field(&mut field_errors, "candidate_id", &record.candidate_id);
    validate_non_empty_promotion_field(
        &mut field_errors,
        "validation_run_id",
        &record.validation_run_id,
    );
    validate_non_empty_promotion_field(&mut field_errors, "reason_code", &record.reason_code);
    validate_non_empty_promotion_field(&mut field_errors, "actor_id", &record.actor_id);
    validate_non_empty_promotion_field(&mut field_errors, "correlation_id", &record.correlation_id);
    validate_non_empty_promotion_field(&mut field_errors, "decided_at_utc", &record.decided_at_utc);

    if parse_promotion_utc_timestamp(&record.decided_at_utc).is_err() {
        field_errors.push(PromotionDecisionValidationIssue {
            field: "decided_at_utc".to_string(),
            code: PromotionDecisionReasonCode::InvalidPayload.code(),
            message: "decided_at_utc must be RFC3339 UTC".to_string(),
        });
    }
    if PromotionDecisionReasonCode::parse(&record.reason_code).is_err() {
        field_errors.push(PromotionDecisionValidationIssue {
            field: "reason_code".to_string(),
            code: PromotionDecisionReasonCode::InvalidPayload.code(),
            message: "reason_code is not recognized".to_string(),
        });
    }
    if !matches!(record.observed_metrics, Value::Object(_)) {
        field_errors.push(PromotionDecisionValidationIssue {
            field: "observed_metrics".to_string(),
            code: PromotionDecisionReasonCode::InvalidPayload.code(),
            message: "observed_metrics must be a JSON object".to_string(),
        });
    }
    if !matches!(record.evidence_packet, Value::Object(_)) {
        field_errors.push(PromotionDecisionValidationIssue {
            field: "evidence_packet".to_string(),
            code: PromotionDecisionReasonCode::InvalidPayload.code(),
            message: "evidence_packet must be a JSON object".to_string(),
        });
    }
    if !matches!(record.gate_evaluation, Value::Object(_)) {
        field_errors.push(PromotionDecisionValidationIssue {
            field: "gate_evaluation".to_string(),
            code: PromotionDecisionReasonCode::InvalidPayload.code(),
            message: "gate_evaluation must be a JSON object".to_string(),
        });
    }
    if let Some(shadow_readiness) = record.shadow_readiness.as_ref()
        && !matches!(shadow_readiness, Value::Object(_))
    {
        field_errors.push(PromotionDecisionValidationIssue {
            field: "shadow_readiness".to_string(),
            code: PromotionDecisionReasonCode::InvalidPayload.code(),
            message: "shadow_readiness must be a JSON object when present".to_string(),
        });
    }
    if record.approval_reference.is_some() && record.approval_request_id.is_none() {
        field_errors.push(PromotionDecisionValidationIssue {
            field: "approval_request_id".to_string(),
            code: PromotionDecisionReasonCode::ApprovalInvalidState.code(),
            message: "approval_request_id is required when approval_reference is provided"
                .to_string(),
        });
    }
    if record.lifecycle_action == PromotionLifecycleAction::Promote
        && record.decision_state == PromotionDecisionState::Allowed
        && record.approval_reference.is_none()
    {
        field_errors.push(PromotionDecisionValidationIssue {
            field: "approval_reference".to_string(),
            code: PromotionDecisionReasonCode::ApprovalRequired.code(),
            message: "promotion allow decisions require approval_reference".to_string(),
        });
    }

    for (index, threshold) in record.threshold_results.iter().enumerate() {
        if normalize_research_identifier(&threshold.metric_key).is_empty() {
            field_errors.push(PromotionDecisionValidationIssue {
                field: format!("threshold_results[{index}].metric_key"),
                code: PromotionDecisionReasonCode::InvalidPayload.code(),
                message: "metric_key is required".to_string(),
            });
        }
        if !threshold.threshold_value.is_finite() {
            field_errors.push(PromotionDecisionValidationIssue {
                field: format!("threshold_results[{index}].threshold_value"),
                code: PromotionDecisionReasonCode::InvalidPayload.code(),
                message: "threshold_value must be finite".to_string(),
            });
        }
        if !threshold.observed_value.is_finite() {
            field_errors.push(PromotionDecisionValidationIssue {
                field: format!("threshold_results[{index}].observed_value"),
                code: PromotionDecisionReasonCode::StateUnavailable.code(),
                message: "observed_value must be finite".to_string(),
            });
        }
        if PromotionThresholdReasonCode::parse(&threshold.reason_code).is_err() {
            field_errors.push(PromotionDecisionValidationIssue {
                field: format!("threshold_results[{index}].reason_code"),
                code: PromotionDecisionReasonCode::InvalidPayload.code(),
                message: "threshold reason_code is not recognized".to_string(),
            });
        }
    }

    if record.lifecycle_action == PromotionLifecycleAction::Promote {
        match validate_promotion_evidence_packet(record.lifecycle_action, &record.evidence_packet) {
            Ok(required_missing_fields) => {
                let absent_required_fields = required_missing_fields
                    .into_iter()
                    .filter(|required| {
                        !record
                            .missing_evidence_fields
                            .iter()
                            .any(|present| present == required)
                    })
                    .collect::<Vec<_>>();
                if !absent_required_fields.is_empty() {
                    field_errors.push(PromotionDecisionValidationIssue {
                        field: "missing_evidence_fields".to_string(),
                        code: PromotionDecisionReasonCode::MissingEvidence.code(),
                        message: format!(
                            "missing_evidence_fields must include FR45 evidence packet gaps: {}",
                            absent_required_fields.join(", ")
                        ),
                    });
                }
            }
            Err(error) => field_errors.extend(error.field_errors),
        }
    }

    if record.lifecycle_action == PromotionLifecycleAction::Promote
        && record.decision_state == PromotionDecisionState::Allowed
        && let Some(summary) = record
            .evidence_packet
            .as_object()
            .and_then(|map| map.get("counterfactual_replay_summary"))
        && let Err(error) = validate_counterfactual_replay_summary_for_allow_path(summary)
    {
        field_errors.extend(error.field_errors);
    }

    if record.decision_state == PromotionDecisionState::Allowed
        && record.threshold_results.iter().any(|result| !result.passed)
    {
        field_errors.push(PromotionDecisionValidationIssue {
            field: "threshold_results".to_string(),
            code: PromotionDecisionReasonCode::ThresholdFailed.code(),
            message: "allowed decisions cannot include failed threshold_results".to_string(),
        });
    }
    if record.decision_state == PromotionDecisionState::Allowed
        && !record.missing_evidence_fields.is_empty()
    {
        field_errors.push(PromotionDecisionValidationIssue {
            field: "missing_evidence_fields".to_string(),
            code: PromotionDecisionReasonCode::MissingEvidence.code(),
            message: "allowed decisions cannot contain missing evidence fields".to_string(),
        });
    }

    if !field_errors.is_empty() {
        return Err(PromotionDecisionContractError::invalid_payload_with_issues(
            "promotion decision payload failed validation",
            field_errors,
        ));
    }
    Ok(())
}

fn validate_non_empty_promotion_field(
    field_errors: &mut Vec<PromotionDecisionValidationIssue>,
    field: &str,
    value: &str,
) {
    if value.trim().is_empty() {
        field_errors.push(PromotionDecisionValidationIssue {
            field: field.to_string(),
            code: PromotionDecisionReasonCode::InvalidPayload.code(),
            message: format!("{field} is required"),
        });
    }
}

pub fn parse_validation_utc_timestamp(
    value: &str,
) -> Result<OffsetDateTime, ValidationWorkflowContractError> {
    let parsed = OffsetDateTime::parse(value, &Rfc3339).map_err(|_| {
        ValidationWorkflowContractError::invalid_payload(format!(
            "timestamp `{value}` must be RFC3339 UTC"
        ))
    })?;
    if parsed.offset() != UtcOffset::UTC {
        return Err(ValidationWorkflowContractError::invalid_payload(
            "timestamps must use UTC `Z` offset",
        ));
    }
    Ok(parsed)
}

pub fn compose_validation_run_id(
    candidate_id: &str,
    started_at_utc: &str,
) -> Result<String, ValidationWorkflowContractError> {
    let normalized_candidate_id = normalize_research_identifier(candidate_id);
    if normalized_candidate_id.is_empty() {
        return Err(
            ValidationWorkflowContractError::invalid_payload_with_issues(
                "candidate_id cannot be blank",
                vec![ValidationWorkflowValidationIssue {
                    field: "candidate_id".to_string(),
                    code: ValidationWorkflowReasonCode::InvalidPayload.code(),
                    message: "candidate_id cannot be blank".to_string(),
                }],
            ),
        );
    }
    let started_at = parse_validation_utc_timestamp(started_at_utc)?;
    Ok(format!(
        "{}::{}",
        normalized_candidate_id,
        started_at.unix_timestamp_nanos()
    ))
}

pub fn canonical_validation_stage_sequence(
    stages: &[ValidationWorkflowStage],
) -> Result<Vec<ValidationWorkflowStage>, ValidationWorkflowContractError> {
    let expected = ValidationWorkflowStage::ordered();
    let expected_vec = expected.to_vec();
    if stages != expected_vec.as_slice() {
        return Err(ValidationWorkflowContractError::invalid_payload_with_issues(
            "validation workflow stage sequence must follow FR7 deterministic ordering",
            vec![ValidationWorkflowValidationIssue {
                field: "stages".to_string(),
                code: ValidationWorkflowReasonCode::InvalidPayload.code(),
                message:
                    "expected sequence: quality -> labeling -> purged_cv -> cpcv -> overfit_diagnostics"
                        .to_string(),
            }],
        ));
    }
    Ok(expected_vec)
}

pub fn validate_validation_diagnostics_payload(
    payload: &ValidationDiagnosticsPayload,
) -> Result<(), ValidationWorkflowContractError> {
    let mut field_errors = Vec::new();
    validate_finite_validation_metric(
        payload.out_of_sample_sharpe,
        "out_of_sample_sharpe",
        &mut field_errors,
    );
    validate_finite_validation_metric(payload.max_drawdown, "max_drawdown", &mut field_errors);
    validate_finite_validation_metric(
        payload.overfit_indicator,
        "overfit_indicator",
        &mut field_errors,
    );
    if let Some(brier_score) = payload.brier_score {
        validate_finite_validation_metric(brier_score, "brier_score", &mut field_errors);
    }
    if let Some(expected_calibration_error) = payload.expected_calibration_error {
        validate_finite_validation_metric(
            expected_calibration_error,
            "expected_calibration_error",
            &mut field_errors,
        );
    }
    if payload.brier_score.is_none() && payload.expected_calibration_error.is_none() {
        field_errors.push(ValidationWorkflowValidationIssue {
            field: "calibration_metric".to_string(),
            code: ValidationWorkflowReasonCode::InvalidPayload.code(),
            message: "either brier_score or expected_calibration_error is required".to_string(),
        });
    }
    if !field_errors.is_empty() {
        return Err(
            ValidationWorkflowContractError::invalid_payload_with_issues(
                "validation diagnostics payload failed validation",
                field_errors,
            ),
        );
    }
    Ok(())
}

pub fn build_validation_metric_deltas(
    current: &ValidationDiagnosticsPayload,
    previous: &ValidationDiagnosticsPayload,
) -> Result<Vec<ValidationMetricDelta>, ValidationWorkflowContractError> {
    validate_validation_diagnostics_payload(current)?;
    validate_validation_diagnostics_payload(previous)?;

    let mut deltas = vec![
        ValidationMetricDelta {
            metric_key: "out_of_sample_sharpe".to_string(),
            current_value: current.out_of_sample_sharpe,
            prior_value: previous.out_of_sample_sharpe,
            delta: current.out_of_sample_sharpe - previous.out_of_sample_sharpe,
        },
        ValidationMetricDelta {
            metric_key: "max_drawdown".to_string(),
            current_value: current.max_drawdown,
            prior_value: previous.max_drawdown,
            delta: current.max_drawdown - previous.max_drawdown,
        },
    ];

    match (
        current.brier_score,
        previous.brier_score,
        current.expected_calibration_error,
        previous.expected_calibration_error,
    ) {
        (Some(current_brier), Some(previous_brier), _, _) => deltas.push(ValidationMetricDelta {
            metric_key: "brier_score".to_string(),
            current_value: current_brier,
            prior_value: previous_brier,
            delta: current_brier - previous_brier,
        }),
        (_, _, Some(current_ece), Some(previous_ece)) => deltas.push(ValidationMetricDelta {
            metric_key: "expected_calibration_error".to_string(),
            current_value: current_ece,
            prior_value: previous_ece,
            delta: current_ece - previous_ece,
        }),
        _ => {
            return Err(
                ValidationWorkflowContractError::invalid_payload_with_issues(
                    "diagnostics comparison requires a shared calibration metric",
                    vec![ValidationWorkflowValidationIssue {
                        field: "calibration_metric".to_string(),
                        code: ValidationWorkflowReasonCode::StateUnavailable.code(),
                        message:
                            "runs must include matching brier_score or expected_calibration_error"
                                .to_string(),
                    }],
                ),
            );
        }
    }

    deltas.push(ValidationMetricDelta {
        metric_key: "overfit_indicator".to_string(),
        current_value: current.overfit_indicator,
        prior_value: previous.overfit_indicator,
        delta: current.overfit_indicator - previous.overfit_indicator,
    });
    Ok(deltas)
}

pub fn build_validation_stage_comparison(
    stage: ValidationWorkflowStage,
    current_run_id: impl Into<String>,
    previous_run_id: impl Into<String>,
    current: &ValidationDiagnosticsPayload,
    previous: &ValidationDiagnosticsPayload,
) -> Result<ValidationStageComparison, ValidationWorkflowContractError> {
    Ok(ValidationStageComparison {
        stage,
        current_run_id: current_run_id.into(),
        previous_run_id: previous_run_id.into(),
        reason_code: ValidationWorkflowReasonCode::ComparisonReady
            .code()
            .to_string(),
        metric_deltas: build_validation_metric_deltas(current, previous)?,
    })
}

fn validate_finite_validation_metric(
    value: f64,
    field: &str,
    field_errors: &mut Vec<ValidationWorkflowValidationIssue>,
) {
    if !value.is_finite() {
        field_errors.push(ValidationWorkflowValidationIssue {
            field: field.to_string(),
            code: ValidationWorkflowReasonCode::InvalidPayload.code(),
            message: format!("{field} must be finite"),
        });
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum AlphaHealthMetricWindow {
    OneHour,
    TwentyFourHours,
    ThirtyDays,
}

impl AlphaHealthMetricWindow {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OneHour => "1h",
            Self::TwentyFourHours => "24h",
            Self::ThirtyDays => "30d",
        }
    }

    pub fn parse(value: &str) -> Result<Self, AlphaHealthContractError> {
        match normalize_research_identifier(value).as_str() {
            "1h" => Ok(Self::OneHour),
            "24h" => Ok(Self::TwentyFourHours),
            "30d" => Ok(Self::ThirtyDays),
            _ => Err(AlphaHealthContractError::invalid_payload(format!(
                "unknown alpha-health attribution window `{value}`"
            ))),
        }
    }
}

pub const FR10_ALPHA_HEALTH_REQUIRED_WINDOWS: [AlphaHealthMetricWindow; 3] = [
    AlphaHealthMetricWindow::OneHour,
    AlphaHealthMetricWindow::TwentyFourHours,
    AlphaHealthMetricWindow::ThirtyDays,
];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum AlphaHealthMetricKey {
    RollingSharpe,
    RollingHitRate,
    RollingDrawdown,
    StabilityScore,
}

impl AlphaHealthMetricKey {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RollingSharpe => "rolling_sharpe",
            Self::RollingHitRate => "rolling_hit_rate",
            Self::RollingDrawdown => "rolling_drawdown",
            Self::StabilityScore => "stability_score",
        }
    }

    pub fn parse(value: &str) -> Result<Self, AlphaHealthContractError> {
        match normalize_research_identifier(value).as_str() {
            "rolling_sharpe" => Ok(Self::RollingSharpe),
            "rolling_hit_rate" => Ok(Self::RollingHitRate),
            "rolling_drawdown" => Ok(Self::RollingDrawdown),
            "stability_score" => Ok(Self::StabilityScore),
            _ => Err(AlphaHealthContractError::invalid_payload(format!(
                "unknown alpha-health metric_key `{value}`"
            ))),
        }
    }

    pub const fn is_floor_metric(self) -> bool {
        matches!(
            self,
            Self::RollingSharpe | Self::RollingHitRate | Self::StabilityScore
        )
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AlphaHealthReasonCode {
    MetricRecorded,
    MetricRead,
    MetricListed,
    ThresholdBreachDetected,
    ThresholdBreachRead,
    ThresholdBreachListed,
    ThresholdSatisfied,
    InvalidPayload,
    UnauthorizedRole,
    MetricNotFound,
    BreachNotFound,
    DependencyUnavailable,
    StateUnavailable,
    PersistenceUnavailable,
}

impl AlphaHealthReasonCode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::MetricRecorded => "alpha_health_metric_recorded",
            Self::MetricRead => "alpha_health_metric_read",
            Self::MetricListed => "alpha_health_metric_listed",
            Self::ThresholdBreachDetected => "alpha_health_threshold_breach_detected",
            Self::ThresholdBreachRead => "alpha_health_threshold_breach_read",
            Self::ThresholdBreachListed => "alpha_health_threshold_breach_listed",
            Self::ThresholdSatisfied => "alpha_health_threshold_satisfied",
            Self::InvalidPayload => "alpha_health_invalid_payload",
            Self::UnauthorizedRole => "alpha_health_unauthorized_role",
            Self::MetricNotFound => "alpha_health_metric_not_found",
            Self::BreachNotFound => "alpha_health_breach_not_found",
            Self::DependencyUnavailable => "alpha_health_dependency_unavailable",
            Self::StateUnavailable => "alpha_health_state_unavailable",
            Self::PersistenceUnavailable => "alpha_health_persistence_unavailable",
        }
    }

    pub fn parse(value: &str) -> Result<Self, AlphaHealthContractError> {
        match value {
            "alpha_health_metric_recorded" => Ok(Self::MetricRecorded),
            "alpha_health_metric_read" => Ok(Self::MetricRead),
            "alpha_health_metric_listed" => Ok(Self::MetricListed),
            "alpha_health_threshold_breach_detected" => Ok(Self::ThresholdBreachDetected),
            "alpha_health_threshold_breach_read" => Ok(Self::ThresholdBreachRead),
            "alpha_health_threshold_breach_listed" => Ok(Self::ThresholdBreachListed),
            "alpha_health_threshold_satisfied" => Ok(Self::ThresholdSatisfied),
            "alpha_health_invalid_payload" => Ok(Self::InvalidPayload),
            "alpha_health_unauthorized_role" => Ok(Self::UnauthorizedRole),
            "alpha_health_metric_not_found" => Ok(Self::MetricNotFound),
            "alpha_health_breach_not_found" => Ok(Self::BreachNotFound),
            "alpha_health_dependency_unavailable" => Ok(Self::DependencyUnavailable),
            "alpha_health_state_unavailable" => Ok(Self::StateUnavailable),
            "alpha_health_persistence_unavailable" => Ok(Self::PersistenceUnavailable),
            _ => Err(AlphaHealthContractError::invalid_payload(format!(
                "unknown alpha-health reason code `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AlphaHealthValidationIssue {
    pub field: String,
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AlphaHealthContractError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<AlphaHealthValidationIssue>,
}

impl AlphaHealthContractError {
    pub fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: AlphaHealthReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn invalid_payload_with_issues(
        message: impl Into<String>,
        field_errors: Vec<AlphaHealthValidationIssue>,
    ) -> Self {
        Self {
            code: AlphaHealthReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    pub fn dependency_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: AlphaHealthReasonCode::DependencyUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn state_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: AlphaHealthReasonCode::StateUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn persistence_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: AlphaHealthReasonCode::PersistenceUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlphaHealthAttributionWindowMetrics {
    pub window: AlphaHealthMetricWindow,
    pub net_pnl: f64,
    pub rolling_sharpe: f64,
    pub rolling_hit_rate: f64,
    pub rolling_drawdown: f64,
    pub stability_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlphaHealthMetricRecord {
    pub metric_id: String,
    pub alpha_id: String,
    pub rolling_sharpe: f64,
    pub rolling_hit_rate: f64,
    pub rolling_drawdown: f64,
    pub stability_score: f64,
    pub windows: Vec<AlphaHealthAttributionWindowMetrics>,
    pub reason_code: String,
    pub actor_id: String,
    pub correlation_id: String,
    pub recorded_at_utc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlphaHealthThresholdDefinition {
    pub metric_key: AlphaHealthMetricKey,
    pub comparator: ValidationGateComparator,
    pub threshold_value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlphaThresholdBreachRecord {
    pub breach_id: String,
    pub metric_id: String,
    pub alpha_id: String,
    pub metric_key: AlphaHealthMetricKey,
    pub comparator: ValidationGateComparator,
    pub observed_value: f64,
    pub threshold_value: f64,
    pub breach_reason: String,
    pub reason_code: String,
    pub actor_id: String,
    pub correlation_id: String,
    pub breached_at_utc: String,
}

pub fn parse_alpha_health_utc_timestamp(
    value: &str,
) -> Result<OffsetDateTime, AlphaHealthContractError> {
    let parsed = OffsetDateTime::parse(value, &Rfc3339).map_err(|_| {
        AlphaHealthContractError::invalid_payload(format!(
            "timestamp `{value}` must be RFC3339 UTC"
        ))
    })?;
    if parsed.offset() != UtcOffset::UTC {
        return Err(AlphaHealthContractError::invalid_payload(
            "timestamps must use UTC `Z` offset",
        ));
    }
    Ok(parsed)
}

pub fn compose_alpha_health_metric_id(
    alpha_id: &str,
    recorded_at_utc: &str,
) -> Result<String, AlphaHealthContractError> {
    let normalized_alpha_id = normalize_research_identifier(alpha_id);
    if normalized_alpha_id.is_empty() {
        return Err(AlphaHealthContractError::invalid_payload_with_issues(
            "alpha_id cannot be blank",
            vec![AlphaHealthValidationIssue {
                field: "alpha_id".to_string(),
                code: AlphaHealthReasonCode::InvalidPayload.code(),
                message: "alpha_id cannot be blank".to_string(),
            }],
        ));
    }
    let recorded_at = parse_alpha_health_utc_timestamp(recorded_at_utc)?;
    Ok(format!(
        "{}::{}",
        normalized_alpha_id,
        recorded_at.unix_timestamp_nanos()
    ))
}

pub fn compose_alpha_threshold_breach_id(
    alpha_id: &str,
    metric_key: AlphaHealthMetricKey,
    breached_at_utc: &str,
) -> Result<String, AlphaHealthContractError> {
    let normalized_alpha_id = normalize_research_identifier(alpha_id);
    if normalized_alpha_id.is_empty() {
        return Err(AlphaHealthContractError::invalid_payload_with_issues(
            "alpha_id cannot be blank",
            vec![AlphaHealthValidationIssue {
                field: "alpha_id".to_string(),
                code: AlphaHealthReasonCode::InvalidPayload.code(),
                message: "alpha_id cannot be blank".to_string(),
            }],
        ));
    }
    let breached_at = parse_alpha_health_utc_timestamp(breached_at_utc)?;
    Ok(format!(
        "{}::{}::{}",
        normalized_alpha_id,
        metric_key.as_str(),
        breached_at.unix_timestamp_nanos()
    ))
}

pub fn canonicalize_alpha_health_metric_record(
    record: &AlphaHealthMetricRecord,
) -> Result<AlphaHealthMetricRecord, AlphaHealthContractError> {
    let mut windows = record
        .windows
        .iter()
        .map(canonicalize_alpha_health_window_metrics)
        .collect::<Result<Vec<_>, _>>()?;
    windows.sort_by_key(|window| alpha_health_window_order(window.window));

    let normalized_reason_code = normalize_research_identifier(&record.reason_code);
    let reason_code = AlphaHealthReasonCode::parse(&normalized_reason_code)
        .map(|reason| reason.code().to_string())
        .unwrap_or(normalized_reason_code);

    let normalized = AlphaHealthMetricRecord {
        metric_id: normalize_research_identifier(&record.metric_id),
        alpha_id: normalize_research_identifier(&record.alpha_id),
        rolling_sharpe: record.rolling_sharpe,
        rolling_hit_rate: record.rolling_hit_rate,
        rolling_drawdown: record.rolling_drawdown,
        stability_score: record.stability_score,
        windows,
        reason_code,
        actor_id: record.actor_id.trim().to_string(),
        correlation_id: record.correlation_id.trim().to_string(),
        recorded_at_utc: record.recorded_at_utc.trim().to_string(),
    };
    validate_alpha_health_metric_record(&normalized)?;
    Ok(normalized)
}

pub fn validate_alpha_health_metric_record(
    record: &AlphaHealthMetricRecord,
) -> Result<(), AlphaHealthContractError> {
    let mut field_errors = Vec::new();
    validate_non_empty_alpha_health_field(&mut field_errors, "metric_id", &record.metric_id);
    validate_non_empty_alpha_health_field(&mut field_errors, "alpha_id", &record.alpha_id);
    validate_non_empty_alpha_health_field(&mut field_errors, "reason_code", &record.reason_code);
    validate_non_empty_alpha_health_field(&mut field_errors, "actor_id", &record.actor_id);
    validate_non_empty_alpha_health_field(
        &mut field_errors,
        "correlation_id",
        &record.correlation_id,
    );
    validate_non_empty_alpha_health_field(
        &mut field_errors,
        "recorded_at_utc",
        &record.recorded_at_utc,
    );

    if parse_alpha_health_utc_timestamp(&record.recorded_at_utc).is_err() {
        field_errors.push(AlphaHealthValidationIssue {
            field: "recorded_at_utc".to_string(),
            code: AlphaHealthReasonCode::InvalidPayload.code(),
            message: "recorded_at_utc must be RFC3339 UTC".to_string(),
        });
    }
    if AlphaHealthReasonCode::parse(&record.reason_code).is_err() {
        field_errors.push(AlphaHealthValidationIssue {
            field: "reason_code".to_string(),
            code: AlphaHealthReasonCode::InvalidPayload.code(),
            message: "reason_code is not a supported alpha-health reason".to_string(),
        });
    }

    validate_finite_alpha_health_value(record.rolling_sharpe, "rolling_sharpe", &mut field_errors);
    validate_finite_alpha_health_value(
        record.rolling_hit_rate,
        "rolling_hit_rate",
        &mut field_errors,
    );
    validate_finite_alpha_health_value(
        record.rolling_drawdown,
        "rolling_drawdown",
        &mut field_errors,
    );
    validate_finite_alpha_health_value(
        record.stability_score,
        "stability_score",
        &mut field_errors,
    );
    validate_alpha_health_windows(record.windows.as_slice(), &mut field_errors);

    if !field_errors.is_empty() {
        return Err(AlphaHealthContractError::invalid_payload_with_issues(
            "alpha-health metric record failed validation",
            field_errors,
        ));
    }
    Ok(())
}

pub fn canonicalize_alpha_threshold_breach_record(
    record: &AlphaThresholdBreachRecord,
) -> Result<AlphaThresholdBreachRecord, AlphaHealthContractError> {
    let normalized_reason_code = normalize_research_identifier(&record.reason_code);
    let reason_code = AlphaHealthReasonCode::parse(&normalized_reason_code)
        .map(|reason| reason.code().to_string())
        .unwrap_or(normalized_reason_code);

    let normalized = AlphaThresholdBreachRecord {
        breach_id: normalize_research_identifier(&record.breach_id),
        metric_id: normalize_research_identifier(&record.metric_id),
        alpha_id: normalize_research_identifier(&record.alpha_id),
        metric_key: record.metric_key,
        comparator: record.comparator,
        observed_value: record.observed_value,
        threshold_value: record.threshold_value,
        breach_reason: record.breach_reason.trim().to_string(),
        reason_code,
        actor_id: record.actor_id.trim().to_string(),
        correlation_id: record.correlation_id.trim().to_string(),
        breached_at_utc: record.breached_at_utc.trim().to_string(),
    };
    validate_alpha_threshold_breach_record(&normalized)?;
    Ok(normalized)
}

pub fn validate_alpha_threshold_breach_record(
    record: &AlphaThresholdBreachRecord,
) -> Result<(), AlphaHealthContractError> {
    let mut field_errors = Vec::new();
    validate_non_empty_alpha_health_field(&mut field_errors, "breach_id", &record.breach_id);
    validate_non_empty_alpha_health_field(&mut field_errors, "metric_id", &record.metric_id);
    validate_non_empty_alpha_health_field(&mut field_errors, "alpha_id", &record.alpha_id);
    validate_non_empty_alpha_health_field(
        &mut field_errors,
        "breach_reason",
        &record.breach_reason,
    );
    validate_non_empty_alpha_health_field(&mut field_errors, "reason_code", &record.reason_code);
    validate_non_empty_alpha_health_field(&mut field_errors, "actor_id", &record.actor_id);
    validate_non_empty_alpha_health_field(
        &mut field_errors,
        "correlation_id",
        &record.correlation_id,
    );
    validate_non_empty_alpha_health_field(
        &mut field_errors,
        "breached_at_utc",
        &record.breached_at_utc,
    );

    if parse_alpha_health_utc_timestamp(&record.breached_at_utc).is_err() {
        field_errors.push(AlphaHealthValidationIssue {
            field: "breached_at_utc".to_string(),
            code: AlphaHealthReasonCode::InvalidPayload.code(),
            message: "breached_at_utc must be RFC3339 UTC".to_string(),
        });
    }
    if AlphaHealthReasonCode::parse(&record.reason_code).is_err() {
        field_errors.push(AlphaHealthValidationIssue {
            field: "reason_code".to_string(),
            code: AlphaHealthReasonCode::InvalidPayload.code(),
            message: "reason_code is not a supported alpha-health reason".to_string(),
        });
    }
    if let Err(error) =
        validate_alpha_health_threshold_definition(&AlphaHealthThresholdDefinition {
            metric_key: record.metric_key,
            comparator: record.comparator,
            threshold_value: record.threshold_value,
        })
    {
        field_errors.extend(error.field_errors);
    }
    validate_finite_alpha_health_value(record.observed_value, "observed_value", &mut field_errors);

    if !field_errors.is_empty() {
        return Err(AlphaHealthContractError::invalid_payload_with_issues(
            "alpha-health threshold breach record failed validation",
            field_errors,
        ));
    }
    Ok(())
}

pub fn evaluate_alpha_health_thresholds(
    metric_record: &AlphaHealthMetricRecord,
    thresholds: &[AlphaHealthThresholdDefinition],
) -> Result<Vec<AlphaThresholdBreachRecord>, AlphaHealthContractError> {
    if thresholds.is_empty() {
        return Err(AlphaHealthContractError::invalid_payload_with_issues(
            "thresholds cannot be empty",
            vec![AlphaHealthValidationIssue {
                field: "thresholds".to_string(),
                code: AlphaHealthReasonCode::InvalidPayload.code(),
                message: "at least one threshold definition is required".to_string(),
            }],
        ));
    }

    let metric_record = canonicalize_alpha_health_metric_record(metric_record)?;
    let mut canonical_thresholds = thresholds
        .iter()
        .map(canonicalize_alpha_health_threshold_definition)
        .collect::<Result<Vec<_>, _>>()?;
    canonical_thresholds.sort_by(|left, right| {
        left.metric_key
            .as_str()
            .cmp(right.metric_key.as_str())
            .then_with(|| left.threshold_value.total_cmp(&right.threshold_value))
    });

    let mut seen_metric_keys = std::collections::BTreeSet::new();
    let mut breaches = Vec::new();
    for threshold in canonical_thresholds {
        if !seen_metric_keys.insert(threshold.metric_key) {
            return Err(AlphaHealthContractError::invalid_payload_with_issues(
                "duplicate threshold metric_key entries are not allowed",
                vec![AlphaHealthValidationIssue {
                    field: "thresholds".to_string(),
                    code: AlphaHealthReasonCode::InvalidPayload.code(),
                    message: format!(
                        "duplicate threshold definition for metric_key `{}`",
                        threshold.metric_key.as_str()
                    ),
                }],
            ));
        }
        let observed_value = alpha_health_observed_value(&metric_record, threshold.metric_key);
        let breached = is_alpha_health_threshold_breached(
            threshold.metric_key,
            observed_value,
            threshold.threshold_value,
        );
        if breached {
            let breach_id = compose_alpha_threshold_breach_id(
                &metric_record.alpha_id,
                threshold.metric_key,
                &metric_record.recorded_at_utc,
            )?;
            let breach_reason = format!(
                "{} breached {} {} with observed {}",
                threshold.metric_key.as_str(),
                threshold.comparator.as_str(),
                threshold.threshold_value,
                observed_value
            );
            breaches.push(canonicalize_alpha_threshold_breach_record(
                &AlphaThresholdBreachRecord {
                    breach_id,
                    metric_id: metric_record.metric_id.clone(),
                    alpha_id: metric_record.alpha_id.clone(),
                    metric_key: threshold.metric_key,
                    comparator: threshold.comparator,
                    observed_value,
                    threshold_value: threshold.threshold_value,
                    breach_reason,
                    reason_code: AlphaHealthReasonCode::ThresholdBreachDetected
                        .code()
                        .to_string(),
                    actor_id: metric_record.actor_id.clone(),
                    correlation_id: metric_record.correlation_id.clone(),
                    breached_at_utc: metric_record.recorded_at_utc.clone(),
                },
            )?);
        }
    }
    breaches.sort_by(|left, right| {
        right
            .breached_at_utc
            .cmp(&left.breached_at_utc)
            .then_with(|| left.breach_id.cmp(&right.breach_id))
    });
    Ok(breaches)
}

pub fn is_alpha_health_threshold_breached(
    metric_key: AlphaHealthMetricKey,
    observed_value: f64,
    threshold_value: f64,
) -> bool {
    if metric_key.is_floor_metric() {
        observed_value < threshold_value
    } else {
        observed_value > threshold_value
    }
}

pub fn expected_alpha_health_comparator(
    metric_key: AlphaHealthMetricKey,
) -> ValidationGateComparator {
    if metric_key.is_floor_metric() {
        ValidationGateComparator::Lt
    } else {
        ValidationGateComparator::Gt
    }
}

pub fn canonicalize_alpha_health_threshold_definition(
    definition: &AlphaHealthThresholdDefinition,
) -> Result<AlphaHealthThresholdDefinition, AlphaHealthContractError> {
    let normalized = AlphaHealthThresholdDefinition {
        metric_key: definition.metric_key,
        comparator: definition.comparator,
        threshold_value: definition.threshold_value,
    };
    validate_alpha_health_threshold_definition(&normalized)?;
    Ok(normalized)
}

pub fn validate_alpha_health_threshold_definition(
    definition: &AlphaHealthThresholdDefinition,
) -> Result<(), AlphaHealthContractError> {
    let mut field_errors = Vec::new();
    validate_finite_alpha_health_value(
        definition.threshold_value,
        "threshold_value",
        &mut field_errors,
    );
    let expected = expected_alpha_health_comparator(definition.metric_key);
    if definition.comparator != expected {
        field_errors.push(AlphaHealthValidationIssue {
            field: "comparator".to_string(),
            code: AlphaHealthReasonCode::InvalidPayload.code(),
            message: format!(
                "{} requires `{}` comparator semantics",
                definition.metric_key.as_str(),
                expected.as_str()
            ),
        });
    }
    if !field_errors.is_empty() {
        return Err(AlphaHealthContractError::invalid_payload_with_issues(
            "alpha-health threshold definition failed validation",
            field_errors,
        ));
    }
    Ok(())
}

fn alpha_health_observed_value(
    metric_record: &AlphaHealthMetricRecord,
    metric_key: AlphaHealthMetricKey,
) -> f64 {
    match metric_key {
        AlphaHealthMetricKey::RollingSharpe => metric_record.rolling_sharpe,
        AlphaHealthMetricKey::RollingHitRate => metric_record.rolling_hit_rate,
        AlphaHealthMetricKey::RollingDrawdown => metric_record.rolling_drawdown,
        AlphaHealthMetricKey::StabilityScore => metric_record.stability_score,
    }
}

fn canonicalize_alpha_health_window_metrics(
    window: &AlphaHealthAttributionWindowMetrics,
) -> Result<AlphaHealthAttributionWindowMetrics, AlphaHealthContractError> {
    let normalized = AlphaHealthAttributionWindowMetrics {
        window: window.window,
        net_pnl: window.net_pnl,
        rolling_sharpe: window.rolling_sharpe,
        rolling_hit_rate: window.rolling_hit_rate,
        rolling_drawdown: window.rolling_drawdown,
        stability_score: window.stability_score,
    };
    validate_finite_alpha_health_window_metrics(&normalized, 0)?;
    Ok(normalized)
}

fn validate_alpha_health_windows(
    windows: &[AlphaHealthAttributionWindowMetrics],
    field_errors: &mut Vec<AlphaHealthValidationIssue>,
) {
    if windows.len() != FR10_ALPHA_HEALTH_REQUIRED_WINDOWS.len() {
        field_errors.push(AlphaHealthValidationIssue {
            field: "windows".to_string(),
            code: AlphaHealthReasonCode::InvalidPayload.code(),
            message: "windows must include exactly 1h, 24h, and 30d entries".to_string(),
        });
    }

    let mut seen = Vec::new();
    for (index, window) in windows.iter().enumerate() {
        if let Err(error) = validate_finite_alpha_health_window_metrics(window, index) {
            field_errors.extend(error.field_errors);
        }
        if seen.contains(&window.window) {
            field_errors.push(AlphaHealthValidationIssue {
                field: format!("windows[{index}].window"),
                code: AlphaHealthReasonCode::InvalidPayload.code(),
                message: format!("duplicate window `{}`", window.window.as_str()),
            });
        } else {
            seen.push(window.window);
        }
    }

    for required in FR10_ALPHA_HEALTH_REQUIRED_WINDOWS {
        if !seen.contains(&required) {
            field_errors.push(AlphaHealthValidationIssue {
                field: "windows".to_string(),
                code: AlphaHealthReasonCode::InvalidPayload.code(),
                message: format!("missing required window `{}`", required.as_str()),
            });
        }
    }
}

fn validate_finite_alpha_health_window_metrics(
    window: &AlphaHealthAttributionWindowMetrics,
    index: usize,
) -> Result<(), AlphaHealthContractError> {
    let mut field_errors = Vec::new();
    validate_finite_alpha_health_value(
        window.net_pnl,
        &format!("windows[{index}].net_pnl"),
        &mut field_errors,
    );
    validate_finite_alpha_health_value(
        window.rolling_sharpe,
        &format!("windows[{index}].rolling_sharpe"),
        &mut field_errors,
    );
    validate_finite_alpha_health_value(
        window.rolling_hit_rate,
        &format!("windows[{index}].rolling_hit_rate"),
        &mut field_errors,
    );
    validate_finite_alpha_health_value(
        window.rolling_drawdown,
        &format!("windows[{index}].rolling_drawdown"),
        &mut field_errors,
    );
    validate_finite_alpha_health_value(
        window.stability_score,
        &format!("windows[{index}].stability_score"),
        &mut field_errors,
    );
    if !field_errors.is_empty() {
        return Err(AlphaHealthContractError::invalid_payload_with_issues(
            "alpha-health attribution windows must contain finite values",
            field_errors,
        ));
    }
    Ok(())
}

fn validate_non_empty_alpha_health_field(
    field_errors: &mut Vec<AlphaHealthValidationIssue>,
    field: &str,
    value: &str,
) {
    if value.trim().is_empty() {
        field_errors.push(AlphaHealthValidationIssue {
            field: field.to_string(),
            code: AlphaHealthReasonCode::InvalidPayload.code(),
            message: format!("{field} cannot be blank"),
        });
    }
}

fn validate_finite_alpha_health_value(
    value: f64,
    field: &str,
    field_errors: &mut Vec<AlphaHealthValidationIssue>,
) {
    if !value.is_finite() {
        field_errors.push(AlphaHealthValidationIssue {
            field: field.to_string(),
            code: AlphaHealthReasonCode::InvalidPayload.code(),
            message: format!("{field} must be finite"),
        });
    }
}

fn alpha_health_window_order(window: AlphaHealthMetricWindow) -> i32 {
    match window {
        AlphaHealthMetricWindow::OneHour => 0,
        AlphaHealthMetricWindow::TwentyFourHours => 1,
        AlphaHealthMetricWindow::ThirtyDays => 2,
    }
}

pub const FR48_STOP_RESEARCH_MIN_TRADE_COUNT_30D: i64 = 200;
pub const FR48_STOP_RESEARCH_MIN_OUT_OF_SAMPLE_SHARPE_30D: f64 = 0.2;
pub const FR48_STOP_RESEARCH_MAX_PROMOTION_FAILURE_RATE_LAST_10: f64 = 0.70;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum AlphaLifecycleActionType {
    Deallocate,
    StopResearch,
}

impl AlphaLifecycleActionType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Deallocate => "deallocate",
            Self::StopResearch => "stop_research",
        }
    }

    pub fn parse(value: &str) -> Result<Self, AlphaLifecycleContractError> {
        match normalize_research_identifier(value).as_str() {
            "deallocate" => Ok(Self::Deallocate),
            "stop_research" => Ok(Self::StopResearch),
            _ => Err(AlphaLifecycleContractError::invalid_payload_with_issues(
                "unsupported alpha lifecycle action type",
                vec![AlphaLifecycleValidationIssue {
                    field: "action_type".to_string(),
                    code: AlphaLifecycleReasonCode::InvalidPayload.code(),
                    message: format!(
                        "action_type must be one of: deallocate, stop_research (received `{value}`)"
                    ),
                }],
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AlphaLifecycleActionStatus {
    Applied,
    Denied,
    Unapplied,
}

impl AlphaLifecycleActionStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Applied => "applied",
            Self::Denied => "denied",
            Self::Unapplied => "unapplied",
        }
    }

    pub fn parse(value: &str) -> Result<Self, AlphaLifecycleContractError> {
        match normalize_research_identifier(value).as_str() {
            "applied" => Ok(Self::Applied),
            "denied" => Ok(Self::Denied),
            "unapplied" => Ok(Self::Unapplied),
            _ => Err(AlphaLifecycleContractError::invalid_payload(format!(
                "unknown alpha lifecycle action status `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AlphaLifecycleReasonCode {
    ActionStarted,
    ActionApplied,
    ActionDenied,
    ActionRead,
    ActionListed,
    InvalidPayload,
    UnauthorizedRole,
    ActionNotFound,
    DeallocationThresholdBreached,
    StopResearchCriteriaMet,
    AuthorizationFailed,
    DependencyUnavailable,
    StateUnavailable,
    PersistenceUnavailable,
}

impl AlphaLifecycleReasonCode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::ActionStarted => "alpha_lifecycle_action_started",
            Self::ActionApplied => "alpha_lifecycle_action_applied",
            Self::ActionDenied => "alpha_lifecycle_action_denied",
            Self::ActionRead => "alpha_lifecycle_action_read",
            Self::ActionListed => "alpha_lifecycle_action_listed",
            Self::InvalidPayload => "alpha_lifecycle_action_invalid_payload",
            Self::UnauthorizedRole => "alpha_lifecycle_action_unauthorized_role",
            Self::ActionNotFound => "alpha_lifecycle_action_not_found",
            Self::DeallocationThresholdBreached => {
                "alpha_lifecycle_action_deallocation_threshold_breached"
            }
            Self::StopResearchCriteriaMet => "alpha_lifecycle_action_stop_research_criteria_met",
            Self::AuthorizationFailed => "alpha_lifecycle_action_authorization_failed",
            Self::DependencyUnavailable => "alpha_lifecycle_action_dependency_unavailable",
            Self::StateUnavailable => "alpha_lifecycle_action_state_unavailable",
            Self::PersistenceUnavailable => "alpha_lifecycle_action_persistence_unavailable",
        }
    }

    pub fn parse(value: &str) -> Result<Self, AlphaLifecycleContractError> {
        match value {
            "alpha_lifecycle_action_started" => Ok(Self::ActionStarted),
            "alpha_lifecycle_action_applied" => Ok(Self::ActionApplied),
            "alpha_lifecycle_action_denied" => Ok(Self::ActionDenied),
            "alpha_lifecycle_action_read" => Ok(Self::ActionRead),
            "alpha_lifecycle_action_listed" => Ok(Self::ActionListed),
            "alpha_lifecycle_action_invalid_payload" => Ok(Self::InvalidPayload),
            "alpha_lifecycle_action_unauthorized_role" => Ok(Self::UnauthorizedRole),
            "alpha_lifecycle_action_not_found" => Ok(Self::ActionNotFound),
            "alpha_lifecycle_action_deallocation_threshold_breached" => {
                Ok(Self::DeallocationThresholdBreached)
            }
            "alpha_lifecycle_action_stop_research_criteria_met" => {
                Ok(Self::StopResearchCriteriaMet)
            }
            "alpha_lifecycle_action_authorization_failed" => Ok(Self::AuthorizationFailed),
            "alpha_lifecycle_action_dependency_unavailable" => Ok(Self::DependencyUnavailable),
            "alpha_lifecycle_action_state_unavailable" => Ok(Self::StateUnavailable),
            "alpha_lifecycle_action_persistence_unavailable" => Ok(Self::PersistenceUnavailable),
            _ => Err(AlphaLifecycleContractError::invalid_payload(format!(
                "unknown alpha lifecycle action reason code `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AlphaLifecycleValidationIssue {
    pub field: String,
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AlphaLifecycleContractError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<AlphaLifecycleValidationIssue>,
}

impl AlphaLifecycleContractError {
    pub fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: AlphaLifecycleReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn invalid_payload_with_issues(
        message: impl Into<String>,
        field_errors: Vec<AlphaLifecycleValidationIssue>,
    ) -> Self {
        Self {
            code: AlphaLifecycleReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    pub fn dependency_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: AlphaLifecycleReasonCode::DependencyUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn state_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: AlphaLifecycleReasonCode::StateUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn persistence_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: AlphaLifecycleReasonCode::PersistenceUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn authorization_failed(message: impl Into<String>) -> Self {
        Self {
            code: AlphaLifecycleReasonCode::AuthorizationFailed.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlphaLifecycleDeallocationPolicy {
    pub metric_key: AlphaHealthMetricKey,
    pub comparator: ValidationGateComparator,
    pub threshold_value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StopResearchCriteriaSnapshot {
    pub trade_count_30d: i64,
    pub out_of_sample_sharpe_30d: f64,
    pub promotion_failure_rate_last_10: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub triggered_criteria: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlphaLifecycleActionRecord {
    pub action_id: String,
    pub alpha_id: String,
    pub action_type: AlphaLifecycleActionType,
    pub action_status: AlphaLifecycleActionStatus,
    pub reason_code: String,
    pub trigger_evidence: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_research_criteria: Option<StopResearchCriteriaSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remediation_guidance: Option<String>,
    pub actor_id: String,
    pub correlation_id: String,
    pub acted_at_utc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_reference: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlphaLifecycleDeallocationEvaluation {
    pub triggered: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub triggered_criteria: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger_breach: Option<AlphaThresholdBreachRecord>,
}

pub fn parse_alpha_lifecycle_utc_timestamp(
    value: &str,
) -> Result<OffsetDateTime, AlphaLifecycleContractError> {
    let parsed = OffsetDateTime::parse(value, &Rfc3339).map_err(|_| {
        AlphaLifecycleContractError::invalid_payload(format!(
            "timestamp `{value}` must be RFC3339 UTC"
        ))
    })?;
    if parsed.offset() != UtcOffset::UTC {
        return Err(AlphaLifecycleContractError::invalid_payload(
            "timestamps must use UTC `Z` offset",
        ));
    }
    Ok(parsed)
}

pub fn compose_alpha_lifecycle_action_id(
    alpha_id: &str,
    acted_at_utc: &str,
) -> Result<String, AlphaLifecycleContractError> {
    let normalized_alpha_id = normalize_research_identifier(alpha_id);
    if normalized_alpha_id.is_empty() {
        return Err(AlphaLifecycleContractError::invalid_payload_with_issues(
            "alpha_id cannot be blank",
            vec![AlphaLifecycleValidationIssue {
                field: "alpha_id".to_string(),
                code: AlphaLifecycleReasonCode::InvalidPayload.code(),
                message: "alpha_id cannot be blank".to_string(),
            }],
        ));
    }
    let acted_at = parse_alpha_lifecycle_utc_timestamp(acted_at_utc)?;
    Ok(format!(
        "{}::{}",
        normalized_alpha_id,
        acted_at.unix_timestamp_nanos()
    ))
}

pub fn compose_alpha_lifecycle_action_id_with_context(
    alpha_id: &str,
    action_type: &str,
    correlation_id: &str,
    acted_at_utc: &str,
) -> Result<String, AlphaLifecycleContractError> {
    let normalized_alpha_id = normalize_research_identifier(alpha_id);
    if normalized_alpha_id.is_empty() {
        return Err(AlphaLifecycleContractError::invalid_payload_with_issues(
            "alpha_id cannot be blank",
            vec![AlphaLifecycleValidationIssue {
                field: "alpha_id".to_string(),
                code: AlphaLifecycleReasonCode::InvalidPayload.code(),
                message: "alpha_id cannot be blank".to_string(),
            }],
        ));
    }
    let normalized_action_type = normalize_research_identifier(action_type);
    if normalized_action_type.is_empty() {
        return Err(AlphaLifecycleContractError::invalid_payload_with_issues(
            "action_type cannot be blank",
            vec![AlphaLifecycleValidationIssue {
                field: "action_type".to_string(),
                code: AlphaLifecycleReasonCode::InvalidPayload.code(),
                message: "action_type cannot be blank".to_string(),
            }],
        ));
    }
    let normalized_correlation_id = normalize_research_identifier(correlation_id);
    if normalized_correlation_id.is_empty() {
        return Err(AlphaLifecycleContractError::invalid_payload_with_issues(
            "correlation_id cannot be blank",
            vec![AlphaLifecycleValidationIssue {
                field: "correlation_id".to_string(),
                code: AlphaLifecycleReasonCode::InvalidPayload.code(),
                message: "correlation_id cannot be blank".to_string(),
            }],
        ));
    }
    let acted_at = parse_alpha_lifecycle_utc_timestamp(acted_at_utc)?;
    Ok(format!(
        "{}::{}::{}::{}",
        normalized_alpha_id,
        normalized_action_type,
        normalized_correlation_id,
        acted_at.unix_timestamp_nanos()
    ))
}

pub fn canonicalize_alpha_lifecycle_action_record(
    record: &AlphaLifecycleActionRecord,
) -> Result<AlphaLifecycleActionRecord, AlphaLifecycleContractError> {
    let normalized_reason_code = normalize_research_identifier(&record.reason_code);
    let reason_code = AlphaLifecycleReasonCode::parse(&normalized_reason_code)
        .map(|reason| reason.code().to_string())
        .unwrap_or(normalized_reason_code);

    let normalized = AlphaLifecycleActionRecord {
        action_id: normalize_research_identifier(&record.action_id),
        alpha_id: normalize_research_identifier(&record.alpha_id),
        action_type: record.action_type,
        action_status: record.action_status,
        reason_code,
        trigger_evidence: record.trigger_evidence.clone(),
        stop_research_criteria: record
            .stop_research_criteria
            .as_ref()
            .map(canonicalize_stop_research_criteria_snapshot)
            .transpose()?,
        remediation_guidance: record
            .remediation_guidance
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        actor_id: record.actor_id.trim().to_string(),
        correlation_id: record.correlation_id.trim().to_string(),
        acted_at_utc: record.acted_at_utc.trim().to_string(),
        approval_request_id: record
            .approval_request_id
            .as_ref()
            .map(|value| normalize_research_identifier(value))
            .filter(|value| !value.is_empty()),
        approval_reference: record
            .approval_reference
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
    };
    validate_alpha_lifecycle_action_record(&normalized)?;
    Ok(normalized)
}

pub fn validate_alpha_lifecycle_action_record(
    record: &AlphaLifecycleActionRecord,
) -> Result<(), AlphaLifecycleContractError> {
    let mut field_errors = Vec::new();
    validate_non_empty_alpha_lifecycle_field(&mut field_errors, "action_id", &record.action_id);
    validate_non_empty_alpha_lifecycle_field(&mut field_errors, "alpha_id", &record.alpha_id);
    validate_non_empty_alpha_lifecycle_field(&mut field_errors, "reason_code", &record.reason_code);
    validate_non_empty_alpha_lifecycle_field(&mut field_errors, "actor_id", &record.actor_id);
    validate_non_empty_alpha_lifecycle_field(
        &mut field_errors,
        "correlation_id",
        &record.correlation_id,
    );
    validate_non_empty_alpha_lifecycle_field(
        &mut field_errors,
        "acted_at_utc",
        &record.acted_at_utc,
    );

    if parse_alpha_lifecycle_utc_timestamp(&record.acted_at_utc).is_err() {
        field_errors.push(AlphaLifecycleValidationIssue {
            field: "acted_at_utc".to_string(),
            code: AlphaLifecycleReasonCode::InvalidPayload.code(),
            message: "acted_at_utc must be RFC3339 UTC".to_string(),
        });
    }
    if AlphaLifecycleReasonCode::parse(&record.reason_code).is_err() {
        field_errors.push(AlphaLifecycleValidationIssue {
            field: "reason_code".to_string(),
            code: AlphaLifecycleReasonCode::InvalidPayload.code(),
            message: "reason_code is not recognized".to_string(),
        });
    }
    if !matches!(record.trigger_evidence, Value::Object(_)) {
        field_errors.push(AlphaLifecycleValidationIssue {
            field: "trigger_evidence".to_string(),
            code: AlphaLifecycleReasonCode::InvalidPayload.code(),
            message: "trigger_evidence must be a JSON object".to_string(),
        });
    }
    if record.approval_reference.is_some() && record.approval_request_id.is_none() {
        field_errors.push(AlphaLifecycleValidationIssue {
            field: "approval_request_id".to_string(),
            code: AlphaLifecycleReasonCode::InvalidPayload.code(),
            message: "approval_request_id is required when approval_reference is provided"
                .to_string(),
        });
    }
    if record.action_status == AlphaLifecycleActionStatus::Unapplied
        && record.remediation_guidance.is_none()
    {
        field_errors.push(AlphaLifecycleValidationIssue {
            field: "remediation_guidance".to_string(),
            code: AlphaLifecycleReasonCode::StateUnavailable.code(),
            message: "remediation_guidance is required for unapplied actions".to_string(),
        });
    }
    if record.action_type == AlphaLifecycleActionType::StopResearch
        && record.stop_research_criteria.is_none()
    {
        field_errors.push(AlphaLifecycleValidationIssue {
            field: "stop_research_criteria".to_string(),
            code: AlphaLifecycleReasonCode::InvalidPayload.code(),
            message: "stop_research_criteria is required for stop_research actions".to_string(),
        });
    }
    if record.action_type != AlphaLifecycleActionType::StopResearch
        && record.stop_research_criteria.is_some()
    {
        field_errors.push(AlphaLifecycleValidationIssue {
            field: "stop_research_criteria".to_string(),
            code: AlphaLifecycleReasonCode::InvalidPayload.code(),
            message: "stop_research_criteria is only allowed for stop_research actions".to_string(),
        });
    }

    if let Some(criteria) = record.stop_research_criteria.as_ref()
        && let Err(error) = validate_stop_research_criteria_snapshot(criteria)
    {
        field_errors.extend(error.field_errors);
    }

    if !field_errors.is_empty() {
        return Err(AlphaLifecycleContractError::invalid_payload_with_issues(
            "alpha lifecycle action payload failed validation",
            field_errors,
        ));
    }
    Ok(())
}

pub fn evaluate_fr47_deallocation_trigger(
    breaches: &[AlphaThresholdBreachRecord],
    policies: &[AlphaLifecycleDeallocationPolicy],
) -> Result<AlphaLifecycleDeallocationEvaluation, AlphaLifecycleContractError> {
    if policies.is_empty() {
        return Err(AlphaLifecycleContractError::invalid_payload_with_issues(
            "at least one deallocation policy is required",
            vec![AlphaLifecycleValidationIssue {
                field: "deallocation_policies".to_string(),
                code: AlphaLifecycleReasonCode::InvalidPayload.code(),
                message: "at least one deallocation policy is required".to_string(),
            }],
        ));
    }

    let mut canonical_policies = policies
        .iter()
        .map(canonicalize_alpha_lifecycle_deallocation_policy)
        .collect::<Result<Vec<_>, _>>()?;
    canonical_policies.sort_by(|left, right| {
        left.metric_key
            .as_str()
            .cmp(right.metric_key.as_str())
            .then_with(|| left.threshold_value.total_cmp(&right.threshold_value))
    });
    let mut seen_metric_keys = std::collections::BTreeSet::new();
    for policy in &canonical_policies {
        if !seen_metric_keys.insert(policy.metric_key) {
            return Err(AlphaLifecycleContractError::invalid_payload_with_issues(
                "duplicate metric policy entries are not allowed",
                vec![AlphaLifecycleValidationIssue {
                    field: "deallocation_policies".to_string(),
                    code: AlphaLifecycleReasonCode::InvalidPayload.code(),
                    message: format!(
                        "duplicate deallocation policy for metric_key `{}`",
                        policy.metric_key.as_str()
                    ),
                }],
            ));
        }
    }

    let mut canonical_breaches = breaches
        .iter()
        .map(|breach| {
            canonicalize_alpha_threshold_breach_record(breach)
                .map_err(map_alpha_health_contract_error_to_lifecycle)
        })
        .collect::<Result<Vec<_>, _>>()?;
    canonical_breaches.sort_by(|left, right| {
        right
            .breached_at_utc
            .cmp(&left.breached_at_utc)
            .then_with(|| left.breach_id.cmp(&right.breach_id))
    });

    let mut evaluated_metric_keys = std::collections::BTreeSet::new();
    for breach in canonical_breaches {
        let metric_key = breach.metric_key.as_str().to_string();
        if !evaluated_metric_keys.insert(metric_key.clone()) {
            continue;
        }
        let Some(policy) = canonical_policies
            .iter()
            .find(|policy| policy.metric_key.as_str() == metric_key)
        else {
            continue;
        };
        if compare_threshold(
            policy.comparator,
            breach.observed_value,
            policy.threshold_value,
        ) {
            return Ok(AlphaLifecycleDeallocationEvaluation {
                triggered: true,
                triggered_criteria: vec![format!(
                    "deallocation_threshold:{}",
                    breach.metric_key.as_str()
                )],
                trigger_breach: Some(breach),
            });
        }
    }

    Ok(AlphaLifecycleDeallocationEvaluation {
        triggered: false,
        triggered_criteria: Vec::new(),
        trigger_breach: None,
    })
}

pub fn evaluate_fr48_stop_research_criteria(
    trade_count_30d: i64,
    out_of_sample_sharpe_30d: f64,
    promotion_failure_rate_last_10: f64,
) -> Result<StopResearchCriteriaSnapshot, AlphaLifecycleContractError> {
    let mut field_errors = Vec::new();

    if trade_count_30d < 0 {
        field_errors.push(AlphaLifecycleValidationIssue {
            field: "trade_count_30d".to_string(),
            code: AlphaLifecycleReasonCode::InvalidPayload.code(),
            message: "trade_count_30d must be greater than or equal to 0".to_string(),
        });
    }
    validate_finite_alpha_lifecycle_value(
        out_of_sample_sharpe_30d,
        "out_of_sample_sharpe_30d",
        &mut field_errors,
    );
    validate_finite_alpha_lifecycle_value(
        promotion_failure_rate_last_10,
        "promotion_failure_rate_last_10",
        &mut field_errors,
    );
    if !(0.0..=1.0).contains(&promotion_failure_rate_last_10) {
        field_errors.push(AlphaLifecycleValidationIssue {
            field: "promotion_failure_rate_last_10".to_string(),
            code: AlphaLifecycleReasonCode::InvalidPayload.code(),
            message: "promotion_failure_rate_last_10 must be between 0 and 1".to_string(),
        });
    }
    if !field_errors.is_empty() {
        return Err(AlphaLifecycleContractError::invalid_payload_with_issues(
            "stop-research criteria failed validation",
            field_errors,
        ));
    }

    let mut triggered_criteria = Vec::new();
    if trade_count_30d < FR48_STOP_RESEARCH_MIN_TRADE_COUNT_30D {
        triggered_criteria.push("trade_count_30d_below_minimum".to_string());
    }
    if out_of_sample_sharpe_30d < FR48_STOP_RESEARCH_MIN_OUT_OF_SAMPLE_SHARPE_30D {
        triggered_criteria.push("out_of_sample_sharpe_30d_below_minimum".to_string());
    }
    if promotion_failure_rate_last_10 > FR48_STOP_RESEARCH_MAX_PROMOTION_FAILURE_RATE_LAST_10 {
        triggered_criteria.push("promotion_failure_rate_last_10_above_maximum".to_string());
    }

    Ok(StopResearchCriteriaSnapshot {
        trade_count_30d,
        out_of_sample_sharpe_30d,
        promotion_failure_rate_last_10,
        triggered_criteria,
    })
}

pub fn stop_research_triggered(snapshot: &StopResearchCriteriaSnapshot) -> bool {
    !snapshot.triggered_criteria.is_empty()
}

pub fn build_stop_research_trigger_evidence(snapshot: &StopResearchCriteriaSnapshot) -> Value {
    json!({
        "criterion_keys": snapshot.triggered_criteria,
        "trade_count_30d": snapshot.trade_count_30d,
        "out_of_sample_sharpe_30d": snapshot.out_of_sample_sharpe_30d,
        "promotion_failure_rate_last_10": snapshot.promotion_failure_rate_last_10,
    })
}

fn canonicalize_alpha_lifecycle_deallocation_policy(
    policy: &AlphaLifecycleDeallocationPolicy,
) -> Result<AlphaLifecycleDeallocationPolicy, AlphaLifecycleContractError> {
    let normalized = AlphaLifecycleDeallocationPolicy {
        metric_key: policy.metric_key,
        comparator: policy.comparator,
        threshold_value: policy.threshold_value,
    };
    validate_alpha_lifecycle_deallocation_policy(&normalized)?;
    Ok(normalized)
}

fn validate_alpha_lifecycle_deallocation_policy(
    policy: &AlphaLifecycleDeallocationPolicy,
) -> Result<(), AlphaLifecycleContractError> {
    let mut field_errors = Vec::new();
    validate_finite_alpha_lifecycle_value(
        policy.threshold_value,
        "threshold_value",
        &mut field_errors,
    );
    let expected_comparator = expected_alpha_health_comparator(policy.metric_key);
    if policy.comparator != expected_comparator {
        field_errors.push(AlphaLifecycleValidationIssue {
            field: "comparator".to_string(),
            code: AlphaLifecycleReasonCode::InvalidPayload.code(),
            message: format!(
                "{} requires `{}` comparator semantics",
                policy.metric_key.as_str(),
                expected_comparator.as_str()
            ),
        });
    }
    if !field_errors.is_empty() {
        return Err(AlphaLifecycleContractError::invalid_payload_with_issues(
            "deallocation policy failed validation",
            field_errors,
        ));
    }
    Ok(())
}

fn canonicalize_stop_research_criteria_snapshot(
    snapshot: &StopResearchCriteriaSnapshot,
) -> Result<StopResearchCriteriaSnapshot, AlphaLifecycleContractError> {
    let mut normalized_triggered = snapshot
        .triggered_criteria
        .iter()
        .map(|criterion| normalize_research_identifier(criterion))
        .filter(|criterion| !criterion.is_empty())
        .collect::<Vec<_>>();
    normalized_triggered.sort();
    normalized_triggered.dedup();

    let normalized = StopResearchCriteriaSnapshot {
        trade_count_30d: snapshot.trade_count_30d,
        out_of_sample_sharpe_30d: snapshot.out_of_sample_sharpe_30d,
        promotion_failure_rate_last_10: snapshot.promotion_failure_rate_last_10,
        triggered_criteria: normalized_triggered,
    };
    validate_stop_research_criteria_snapshot(&normalized)?;
    Ok(normalized)
}

fn validate_stop_research_criteria_snapshot(
    snapshot: &StopResearchCriteriaSnapshot,
) -> Result<(), AlphaLifecycleContractError> {
    let mut field_errors = Vec::new();
    if snapshot.trade_count_30d < 0 {
        field_errors.push(AlphaLifecycleValidationIssue {
            field: "stop_research_criteria.trade_count_30d".to_string(),
            code: AlphaLifecycleReasonCode::InvalidPayload.code(),
            message: "trade_count_30d must be greater than or equal to 0".to_string(),
        });
    }
    validate_finite_alpha_lifecycle_value(
        snapshot.out_of_sample_sharpe_30d,
        "stop_research_criteria.out_of_sample_sharpe_30d",
        &mut field_errors,
    );
    validate_finite_alpha_lifecycle_value(
        snapshot.promotion_failure_rate_last_10,
        "stop_research_criteria.promotion_failure_rate_last_10",
        &mut field_errors,
    );
    if snapshot.promotion_failure_rate_last_10 < 0.0
        || snapshot.promotion_failure_rate_last_10 > 1.0
    {
        field_errors.push(AlphaLifecycleValidationIssue {
            field: "stop_research_criteria.promotion_failure_rate_last_10".to_string(),
            code: AlphaLifecycleReasonCode::InvalidPayload.code(),
            message: "promotion_failure_rate_last_10 must be between 0 and 1".to_string(),
        });
    }
    for (index, criterion) in snapshot.triggered_criteria.iter().enumerate() {
        if criterion.trim().is_empty() {
            field_errors.push(AlphaLifecycleValidationIssue {
                field: format!("stop_research_criteria.triggered_criteria[{index}]"),
                code: AlphaLifecycleReasonCode::InvalidPayload.code(),
                message: "triggered criteria keys must be non-empty".to_string(),
            });
        }
    }
    if !field_errors.is_empty() {
        return Err(AlphaLifecycleContractError::invalid_payload_with_issues(
            "stop-research criteria snapshot failed validation",
            field_errors,
        ));
    }
    Ok(())
}

fn validate_non_empty_alpha_lifecycle_field(
    field_errors: &mut Vec<AlphaLifecycleValidationIssue>,
    field: &str,
    value: &str,
) {
    if value.trim().is_empty() {
        field_errors.push(AlphaLifecycleValidationIssue {
            field: field.to_string(),
            code: AlphaLifecycleReasonCode::InvalidPayload.code(),
            message: format!("{field} cannot be blank"),
        });
    }
}

fn validate_finite_alpha_lifecycle_value(
    value: f64,
    field: &str,
    field_errors: &mut Vec<AlphaLifecycleValidationIssue>,
) {
    if !value.is_finite() {
        field_errors.push(AlphaLifecycleValidationIssue {
            field: field.to_string(),
            code: AlphaLifecycleReasonCode::InvalidPayload.code(),
            message: format!("{field} must be finite"),
        });
    }
}

fn compare_threshold(comparator: ValidationGateComparator, observed: f64, threshold: f64) -> bool {
    match comparator {
        ValidationGateComparator::Lt => observed < threshold,
        ValidationGateComparator::Lte => observed <= threshold,
        ValidationGateComparator::Gt => observed > threshold,
        ValidationGateComparator::Gte => observed >= threshold,
    }
}

fn map_alpha_health_contract_error_to_lifecycle(
    error: AlphaHealthContractError,
) -> AlphaLifecycleContractError {
    AlphaLifecycleContractError::invalid_payload_with_issues(
        error.message,
        error
            .field_errors
            .into_iter()
            .map(|issue| AlphaLifecycleValidationIssue {
                field: issue.field,
                code: AlphaLifecycleReasonCode::InvalidPayload.code(),
                message: issue.message,
            })
            .collect(),
    )
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

    fn sample_validation_gate_policy() -> ValidationGatePolicyDefinition {
        ValidationGatePolicyDefinition {
            policy_key: "fr43::forward-bias::primary".to_string(),
            gate_type: ValidationGateType::ForwardBias,
            stage_scope: ValidationGateStageScope::TrainingAndPromotion,
            metric_key: "forward_bias_score".to_string(),
            threshold: ValidationGateThreshold {
                comparator: ValidationGateComparator::Lte,
                value: 0.12,
            },
            mandatory: true,
            diagnostics: json!({
                "failure_reason": "forward_bias_above_limit",
                "operator_action": "review feature windows"
            }),
            actor_id: "ops-1".to_string(),
            correlation_id: "corr-fr43-001".to_string(),
            updated_at_utc: "2026-04-07T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn validation_gate_canonicalization_normalizes_identifiers() {
        let mut policy = sample_validation_gate_policy();
        policy.policy_key = " FR43::Forward-Bias::Primary ".to_string();
        policy.metric_key = " Forward_Bias_Score ".to_string();

        let canonical = canonicalize_validation_gate_policy_definition(&policy)
            .expect("canonical validation gate policy should succeed");
        assert_eq!(canonical.policy_key, "fr43::forward-bias::primary");
        assert_eq!(canonical.metric_key, "forward_bias_score");
    }

    #[test]
    fn validation_gate_rejects_non_mandatory_forward_bias_policy() {
        let mut policy = sample_validation_gate_policy();
        policy.mandatory = false;

        let error = validate_validation_gate_policy_definition(&policy)
            .expect_err("forward_bias policies must remain mandatory");
        assert_eq!(error.code, ValidationGateReasonCode::InvalidPayload.code());
        assert!(error.field_errors.iter().any(|issue| {
            issue.field == "mandatory"
                && issue.code == ValidationGateReasonCode::MissingMandatoryPolicy.code()
        }));
    }

    #[test]
    fn validation_gate_rejects_non_mandatory_data_quality_policy() {
        let mut policy = sample_validation_gate_policy();
        policy.gate_type = ValidationGateType::DataQuality;
        policy.policy_key = "fr43::data-quality::training-core".to_string();
        policy.metric_key = "data_quality_score".to_string();
        policy.mandatory = false;

        let error = validate_validation_gate_policy_definition(&policy)
            .expect_err("data_quality policies must remain mandatory for stage enforcement");
        assert_eq!(error.code, ValidationGateReasonCode::InvalidPayload.code());
        assert!(error.field_errors.iter().any(|issue| {
            issue.field == "mandatory"
                && issue.code == ValidationGateReasonCode::MissingMandatoryPolicy.code()
        }));
    }

    #[test]
    fn validation_gate_threshold_boundaries_are_deterministic() {
        let gt_threshold = ValidationGateThreshold {
            comparator: ValidationGateComparator::Gt,
            value: 1.0,
        };
        let gte_threshold = ValidationGateThreshold {
            comparator: ValidationGateComparator::Gte,
            value: 1.0,
        };
        let lt_threshold = ValidationGateThreshold {
            comparator: ValidationGateComparator::Lt,
            value: 1.0,
        };
        let lte_threshold = ValidationGateThreshold {
            comparator: ValidationGateComparator::Lte,
            value: 1.0,
        };

        assert!(!evaluate_validation_gate_threshold(&gt_threshold, 1.0).expect("gt should parse"));
        assert!(evaluate_validation_gate_threshold(&gte_threshold, 1.0).expect("gte should parse"));
        assert!(!evaluate_validation_gate_threshold(&lt_threshold, 1.0).expect("lt should parse"));
        assert!(evaluate_validation_gate_threshold(&lte_threshold, 1.0).expect("lte should parse"));
    }

    #[test]
    fn validation_gate_stage_scope_training_and_promotion_applies_to_both_workflow_stages() {
        assert!(
            ValidationGateStageScope::TrainingAndPromotion
                .applies_to(ValidationGateWorkflowStage::Training)
        );
        assert!(
            ValidationGateStageScope::TrainingAndPromotion
                .applies_to(ValidationGateWorkflowStage::Promotion)
        );
        assert!(
            !ValidationGateStageScope::Promotion.applies_to(ValidationGateWorkflowStage::Training)
        );
    }

    #[test]
    fn validation_gate_reason_code_parse_accepts_known_values() {
        let code = ValidationGateReasonCode::parse("validation_gate_dependency_unavailable")
            .expect("known reason should parse");
        assert_eq!(code, ValidationGateReasonCode::DependencyUnavailable);
    }

    fn sample_validation_diagnostics() -> ValidationDiagnosticsPayload {
        ValidationDiagnosticsPayload {
            out_of_sample_sharpe: 1.42,
            max_drawdown: -0.18,
            brier_score: Some(0.11),
            expected_calibration_error: None,
            overfit_indicator: 0.23,
            overfit_flag: false,
        }
    }

    #[test]
    fn validation_run_stage_sequence_is_deterministic() {
        let expected = vec![
            ValidationWorkflowStage::Quality,
            ValidationWorkflowStage::Labeling,
            ValidationWorkflowStage::PurgedCv,
            ValidationWorkflowStage::Cpcv,
            ValidationWorkflowStage::OverfitDiagnostics,
        ];
        let canonical = canonical_validation_stage_sequence(&expected)
            .expect("ordered stage sequence should be accepted");
        assert_eq!(canonical, expected);
    }

    #[test]
    fn validation_run_stage_sequence_rejects_misordered_stages() {
        let error = canonical_validation_stage_sequence(&[
            ValidationWorkflowStage::Labeling,
            ValidationWorkflowStage::Quality,
            ValidationWorkflowStage::PurgedCv,
            ValidationWorkflowStage::Cpcv,
            ValidationWorkflowStage::OverfitDiagnostics,
        ])
        .expect_err("misordered stages must be rejected");
        assert_eq!(
            error.code,
            ValidationWorkflowReasonCode::InvalidPayload.code()
        );
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "stages")
        );
    }

    #[test]
    fn validation_run_reason_code_parse_accepts_known_values() {
        let code = ValidationWorkflowReasonCode::parse("validation_run_dependency_unavailable")
            .expect("known reason should parse");
        assert_eq!(code, ValidationWorkflowReasonCode::DependencyUnavailable);
    }

    #[test]
    fn validation_run_compose_id_normalizes_identifier_and_uses_utc_timestamp() {
        let run_id = compose_validation_run_id(" Candidate::Alpha-1 ", "2026-04-07T00:00:00Z")
            .expect("run id composition should succeed");
        assert!(run_id.starts_with("candidate::alpha-1::"));
    }

    #[test]
    fn validation_run_diagnostics_require_shared_calibration_metric() {
        let mut diagnostics = sample_validation_diagnostics();
        diagnostics.brier_score = None;

        let error = validate_validation_diagnostics_payload(&diagnostics)
            .expect_err("missing calibration metric should fail");
        assert_eq!(
            error.code,
            ValidationWorkflowReasonCode::InvalidPayload.code()
        );
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "calibration_metric")
        );
    }

    #[test]
    fn validation_run_metric_delta_ordering_is_deterministic() {
        let current = sample_validation_diagnostics();
        let mut previous = sample_validation_diagnostics();
        previous.out_of_sample_sharpe = 1.12;
        previous.max_drawdown = -0.22;
        previous.brier_score = Some(0.14);
        previous.overfit_indicator = 0.31;

        let deltas = build_validation_metric_deltas(&current, &previous)
            .expect("comparable diagnostics should build deltas");
        assert_eq!(deltas[0].metric_key, "out_of_sample_sharpe");
        assert_eq!(deltas[1].metric_key, "max_drawdown");
        assert_eq!(deltas[2].metric_key, "brier_score");
        assert_eq!(deltas[3].metric_key, "overfit_indicator");
    }

    fn sample_shadow_evaluation_record() -> ShadowEvaluationRecord {
        ShadowEvaluationRecord {
            evaluation_id: "candidate::alpha-1::1712448000".to_string(),
            candidate_id: "candidate::alpha-1".to_string(),
            validation_run_id: "candidate::alpha-1::1712447000".to_string(),
            evaluation_state: ShadowEvaluationState::Completed,
            reason_code: ShadowEvaluationReasonCode::EvaluationCompleted
                .code()
                .to_string(),
            market_context: json!({
                "best_bid": 0.42,
                "best_ask": 0.44
            }),
            signal_decisions: json!({
                "signals": [
                    {
                        "decision_side": "buy",
                        "intended_size": 10.0,
                        "decision_timestamp_utc": "2026-04-07T00:00:01Z"
                    }
                ]
            }),
            simulation_outcomes: vec![ShadowSimulationOutcome {
                decision_side: ShadowSimulationDecisionSide::Buy,
                intended_size: 10.0,
                simulated_fill_size: 10.0,
                simulated_fill_price: 0.43,
                simulated_slippage_bps: 5.0,
                simulation_reason_code: ShadowSimulationReasonCode::ReadOnlyEnforced
                    .code()
                    .to_string(),
                decision_timestamp_utc: "2026-04-07T00:00:01Z".to_string(),
                simulated_at_utc: "2026-04-07T00:00:02Z".to_string(),
            }],
            actor_id: "ops-1".to_string(),
            correlation_id: "corr-shadow-001".to_string(),
            started_at_utc: "2026-04-07T00:00:00Z".to_string(),
            completed_at_utc: Some("2026-04-07T00:00:03Z".to_string()),
        }
    }

    #[test]
    fn shadow_evaluation_reason_code_parse_accepts_known_values() {
        let code = ShadowEvaluationReasonCode::parse("shadow_evaluation_dependency_unavailable")
            .expect("known reason should parse");
        assert_eq!(code, ShadowEvaluationReasonCode::DependencyUnavailable);
    }

    #[test]
    fn shadow_evaluation_compose_id_normalizes_identifier_and_uses_utc_timestamp() {
        let evaluation_id =
            compose_shadow_evaluation_id(" Candidate::Alpha-1 ", "2026-04-07T00:00:00Z")
                .expect("shadow evaluation id composition should succeed");
        assert!(evaluation_id.starts_with("candidate::alpha-1::"));
    }

    #[test]
    fn shadow_evaluation_validation_rejects_non_object_market_context() {
        let mut record = sample_shadow_evaluation_record();
        record.market_context = json!(["not", "object"]);

        let error = validate_shadow_evaluation_record(&record)
            .expect_err("non-object market_context must fail validation");
        assert_eq!(
            error.code,
            ShadowEvaluationReasonCode::InvalidPayload.code()
        );
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "market_context")
        );
    }

    #[test]
    fn shadow_evaluation_canonicalization_orders_outcomes_deterministically() {
        let mut record = sample_shadow_evaluation_record();
        record.simulation_outcomes = vec![
            ShadowSimulationOutcome {
                decision_side: ShadowSimulationDecisionSide::Sell,
                intended_size: 4.0,
                simulated_fill_size: 4.0,
                simulated_fill_price: 0.47,
                simulated_slippage_bps: 8.0,
                simulation_reason_code: ShadowSimulationReasonCode::ReadOnlyEnforced
                    .code()
                    .to_string(),
                decision_timestamp_utc: "2026-04-07T00:00:03Z".to_string(),
                simulated_at_utc: "2026-04-07T00:00:04Z".to_string(),
            },
            ShadowSimulationOutcome {
                decision_side: ShadowSimulationDecisionSide::Buy,
                intended_size: 6.0,
                simulated_fill_size: 6.0,
                simulated_fill_price: 0.41,
                simulated_slippage_bps: 6.0,
                simulation_reason_code: ShadowSimulationReasonCode::ReadOnlyEnforced
                    .code()
                    .to_string(),
                decision_timestamp_utc: "2026-04-07T00:00:02Z".to_string(),
                simulated_at_utc: "2026-04-07T00:00:03Z".to_string(),
            },
        ];

        let canonical = canonicalize_shadow_evaluation_record(&record)
            .expect("canonical shadow evaluation record should validate");
        assert_eq!(
            canonical.simulation_outcomes[0].decision_timestamp_utc,
            "2026-04-07T00:00:02Z"
        );
        assert_eq!(
            canonical.simulation_outcomes[1].decision_timestamp_utc,
            "2026-04-07T00:00:03Z"
        );
    }

    #[test]
    fn shadow_evaluation_outcome_validation_requires_known_read_only_reason_code() {
        let mut record = sample_shadow_evaluation_record();
        record.simulation_outcomes[0].simulation_reason_code =
            "shadow_simulation_unknown".to_string();

        let error = validate_shadow_evaluation_record(&record)
            .expect_err("unknown simulation_reason_code must fail validation");
        assert_eq!(
            error.code,
            ShadowEvaluationReasonCode::InvalidPayload.code()
        );
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field.contains("simulation_reason_code"))
        );
    }

    fn sample_counterfactual_replay_summary() -> CounterfactualReplaySummary {
        CounterfactualReplaySummary {
            run_id: "candidate::alpha-1::1712449000".to_string(),
            gate_outcome: CounterfactualReplayGateOutcome::Allow,
            reason_code: CounterfactualReplayReasonCode::ToleranceSatisfied
                .code()
                .to_string(),
            baseline_net_pnl: 100.0,
            stressed_net_pnl: 95.0,
            delayed_exit_net_pnl: 97.5,
            degradation_pct: -5.0,
            tolerance_threshold_pct: FR46_DEGRADATION_DENY_THRESHOLD_PCT,
            scenarios: vec![
                CounterfactualReplayScenarioResult {
                    scenario: CounterfactualReplayScenarioKind::Baseline,
                    net_pnl: 100.0,
                    degradation_pct: Some(0.0),
                    gate_outcome: CounterfactualReplayGateOutcome::Allow,
                    reason_code: CounterfactualReplayReasonCode::ToleranceSatisfied
                        .code()
                        .to_string(),
                    parameters: fr46_scenario_parameters(
                        CounterfactualReplayScenarioKind::Baseline,
                    ),
                },
                CounterfactualReplayScenarioResult {
                    scenario: CounterfactualReplayScenarioKind::StressedExecution,
                    net_pnl: 95.0,
                    degradation_pct: Some(-5.0),
                    gate_outcome: CounterfactualReplayGateOutcome::Allow,
                    reason_code: CounterfactualReplayReasonCode::ToleranceSatisfied
                        .code()
                        .to_string(),
                    parameters: fr46_scenario_parameters(
                        CounterfactualReplayScenarioKind::StressedExecution,
                    ),
                },
                CounterfactualReplayScenarioResult {
                    scenario: CounterfactualReplayScenarioKind::DelayedExit,
                    net_pnl: 97.5,
                    degradation_pct: Some(-2.5),
                    gate_outcome: CounterfactualReplayGateOutcome::Allow,
                    reason_code: CounterfactualReplayReasonCode::ToleranceSatisfied
                        .code()
                        .to_string(),
                    parameters: fr46_scenario_parameters(
                        CounterfactualReplayScenarioKind::DelayedExit,
                    ),
                },
            ],
        }
    }

    fn sample_counterfactual_replay_run_record() -> CounterfactualReplayRunRecord {
        let summary = sample_counterfactual_replay_summary();
        CounterfactualReplayRunRecord {
            run_id: summary.run_id.clone(),
            candidate_id: "candidate::alpha-1".to_string(),
            validation_run_id: "candidate::alpha-1::1712447000".to_string(),
            run_state: CounterfactualReplayRunState::Completed,
            reason_code: CounterfactualReplayReasonCode::ToleranceSatisfied
                .code()
                .to_string(),
            scenario_results: summary.scenarios.clone(),
            replay_summary: summary,
            actor_id: "ops-1".to_string(),
            correlation_id: "corr-replay-001".to_string(),
            started_at_utc: "2026-04-07T00:15:00Z".to_string(),
            completed_at_utc: Some("2026-04-07T00:16:00Z".to_string()),
        }
    }

    #[test]
    fn counterfactual_replay_gate_enforces_fr46_boundary_deterministically() {
        let boundary = evaluate_counterfactual_replay_gate(100.0, 95.0)
            .expect("boundary stress gate should evaluate");
        assert_eq!(boundary.degradation_pct, -5.0);
        assert_eq!(
            boundary.gate_outcome,
            CounterfactualReplayGateOutcome::Allow
        );

        let deny = evaluate_counterfactual_replay_gate(100.0, 94.99)
            .expect("below-boundary stress gate should evaluate");
        assert!(deny.degradation_pct < -5.0);
        assert_eq!(deny.gate_outcome, CounterfactualReplayGateOutcome::Deny);
        assert_eq!(
            deny.reason_code,
            CounterfactualReplayReasonCode::ToleranceBreached.code()
        );

        let epsilon_below_boundary = evaluate_counterfactual_replay_gate(100.0, 94.9999999)
            .expect("any value below -5.0 should deny");
        assert!(
            epsilon_below_boundary.degradation_pct < -5.0,
            "degradation should remain below boundary"
        );
        assert_eq!(
            epsilon_below_boundary.gate_outcome,
            CounterfactualReplayGateOutcome::Deny
        );
    }

    #[test]
    fn counterfactual_replay_gate_rejects_invalid_baseline_denominator() {
        let error = evaluate_counterfactual_replay_gate(0.0, 100.0)
            .expect_err("baseline <= 0 must fail closed");
        assert_eq!(
            error.code,
            CounterfactualReplayReasonCode::InvalidPayload.code()
        );
        assert!(error.field_errors.iter().any(|issue| {
            issue.field == "baseline_net_pnl"
                && issue.code == CounterfactualReplayReasonCode::BaselineUnavailable.code()
        }));
    }

    #[test]
    fn counterfactual_replay_summary_validation_requires_all_required_scenarios() {
        let mut summary = sample_counterfactual_replay_summary();
        summary
            .scenarios
            .retain(|scenario| scenario.scenario != CounterfactualReplayScenarioKind::DelayedExit);

        let error = validate_counterfactual_replay_summary(&summary)
            .expect_err("missing delayed_exit scenario must fail validation");
        assert!(error.field_errors.iter().any(|issue| {
            issue.field == "scenarios"
                && issue.code == CounterfactualReplayReasonCode::ScenarioIncomplete.code()
        }));
    }

    #[test]
    fn counterfactual_replay_summary_validation_rejects_inconsistent_summary_fields() {
        let mut summary = sample_counterfactual_replay_summary();
        summary.reason_code = CounterfactualReplayReasonCode::ToleranceBreached
            .code()
            .to_string();
        summary.scenarios.push(summary.scenarios[0].clone());
        summary.scenarios[1].net_pnl = 94.0;

        let error = validate_counterfactual_replay_summary(&summary)
            .expect_err("duplicate scenarios and mismatched summary fields must fail validation");
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "scenarios")
        );
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "stressed_net_pnl")
        );
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "reason_code")
        );
    }

    #[test]
    fn counterfactual_replay_allow_path_rejects_deferred_placeholder_summary() {
        let error = validate_counterfactual_replay_summary_for_allow_path(
            &json!({ "status": "deferred_to_story_6_6" }),
        )
        .expect_err("deferred placeholder must be rejected for allow-path promotions");
        assert!(error.field_errors.iter().any(|issue| {
            issue.field == "counterfactual_replay_summary.status"
                && issue.code == CounterfactualReplayReasonCode::PlaceholderRejected.code()
        }));
    }

    #[test]
    fn counterfactual_replay_run_record_canonicalization_normalizes_identifiers() {
        let mut record = sample_counterfactual_replay_run_record();
        record.run_id = " Candidate::Alpha-1::1712449000 ".to_string();
        record.candidate_id = " Candidate::Alpha-1 ".to_string();
        record.validation_run_id = " Candidate::Alpha-1::1712447000 ".to_string();
        record.reason_code = " Counterfactual_Replay_Tolerance_Satisfied ".to_string();
        record.replay_summary.run_id = " Candidate::Alpha-1::1712449000 ".to_string();
        record.replay_summary.reason_code =
            " Counterfactual_Replay_Tolerance_Satisfied ".to_string();
        record.scenario_results.reverse();
        record.replay_summary.scenarios.reverse();

        let canonical = canonicalize_counterfactual_replay_run_record(&record)
            .expect("canonical replay run should validate");
        assert_eq!(canonical.run_id, "candidate::alpha-1::1712449000");
        assert_eq!(canonical.candidate_id, "candidate::alpha-1");
        assert_eq!(
            canonical.validation_run_id,
            "candidate::alpha-1::1712447000"
        );
        assert_eq!(
            canonical.reason_code,
            CounterfactualReplayReasonCode::ToleranceSatisfied.code()
        );
        assert_eq!(
            canonical.replay_summary.scenarios[0].scenario,
            CounterfactualReplayScenarioKind::Baseline
        );
        assert_eq!(
            canonical.replay_summary.scenarios[1].scenario,
            CounterfactualReplayScenarioKind::StressedExecution
        );
        assert_eq!(
            canonical.replay_summary.scenarios[2].scenario,
            CounterfactualReplayScenarioKind::DelayedExit
        );
    }

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
                "out_of_sample_sharpe": 1.28,
                "max_drawdown": -0.17,
                "overfit_indicator": 0.23
            }),
            evidence_packet: json!({
                "data_quality_report": { "artifact_id": "quality::001" },
                "purged_cpcv_results": { "artifact_id": "cpcv::001" },
                "calibration_report": { "artifact_id": "calibration::001" }
            }),
            threshold_results: vec![
                PromotionThresholdOutcome {
                    metric_key: "max_drawdown".to_string(),
                    comparator: ValidationGateComparator::Gte,
                    threshold_value: -0.2,
                    observed_value: -0.17,
                    passed: true,
                    reason_code: PromotionThresholdReasonCode::Passed.code().to_string(),
                },
                PromotionThresholdOutcome {
                    metric_key: "out_of_sample_sharpe".to_string(),
                    comparator: ValidationGateComparator::Gte,
                    threshold_value: 1.0,
                    observed_value: 1.28,
                    passed: true,
                    reason_code: PromotionThresholdReasonCode::Passed.code().to_string(),
                },
            ],
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
    fn promotion_decision_lifecycle_action_parse_rejects_unsupported_actions() {
        assert_eq!(
            PromotionLifecycleAction::parse("promote").expect("promote should parse"),
            PromotionLifecycleAction::Promote
        );
        assert_eq!(
            PromotionLifecycleAction::parse("pause").expect("pause should parse"),
            PromotionLifecycleAction::Pause
        );
        assert_eq!(
            PromotionLifecycleAction::parse("retire").expect("retire should parse"),
            PromotionLifecycleAction::Retire
        );

        let error = PromotionLifecycleAction::parse("deploy")
            .expect_err("unsupported lifecycle action must fail");
        assert_eq!(
            error.code,
            PromotionDecisionReasonCode::InvalidPayload.code()
        );
        assert!(error.field_errors.iter().any(|issue| {
            issue.field == "lifecycle_action"
                && issue.code == PromotionDecisionReasonCode::UnsupportedAction.code()
        }));
    }

    #[test]
    fn promotion_decision_threshold_evaluation_honors_boundary_semantics() {
        let thresholds = vec![
            PromotionThresholdDefinition {
                metric_key: "metric_gt".to_string(),
                comparator: ValidationGateComparator::Gt,
                threshold_value: 1.0,
            },
            PromotionThresholdDefinition {
                metric_key: "metric_gte".to_string(),
                comparator: ValidationGateComparator::Gte,
                threshold_value: 1.0,
            },
            PromotionThresholdDefinition {
                metric_key: "metric_lt".to_string(),
                comparator: ValidationGateComparator::Lt,
                threshold_value: 1.0,
            },
            PromotionThresholdDefinition {
                metric_key: "metric_lte".to_string(),
                comparator: ValidationGateComparator::Lte,
                threshold_value: 1.0,
            },
        ];
        let observed_metrics = json!({
            "metric_gt": 1.0,
            "metric_gte": 1.0,
            "metric_lt": 1.0,
            "metric_lte": 1.0
        });

        let outcomes = evaluate_promotion_thresholds(&thresholds, &observed_metrics)
            .expect("threshold evaluation should succeed");
        let by_metric = outcomes
            .iter()
            .map(|outcome| (outcome.metric_key.as_str(), outcome))
            .collect::<std::collections::BTreeMap<_, _>>();

        assert!(!by_metric["metric_gt"].passed);
        assert_eq!(
            by_metric["metric_gt"].reason_code,
            PromotionThresholdReasonCode::Failed.code()
        );
        assert!(by_metric["metric_gte"].passed);
        assert_eq!(
            by_metric["metric_gte"].reason_code,
            PromotionThresholdReasonCode::Passed.code()
        );
        assert!(!by_metric["metric_lt"].passed);
        assert!(by_metric["metric_lte"].passed);
    }

    #[test]
    fn promotion_decision_evidence_packet_reports_missing_fr45_fields_for_promote() {
        let missing = validate_promotion_evidence_packet(
            PromotionLifecycleAction::Promote,
            &json!({
                "data_quality_report": { "artifact_id": "quality::001" },
                "purged_cpcv_results": { "artifact_id": "cpcv::001" }
            }),
        )
        .expect("packet shape should parse");
        assert_eq!(
            missing,
            vec![
                "calibration_report".to_string(),
                "counterfactual_replay_summary".to_string()
            ]
        );

        let non_promote_missing =
            validate_promotion_evidence_packet(PromotionLifecycleAction::Pause, &json!({}))
                .expect("pause packet should be accepted without FR45 requirements");
        assert!(non_promote_missing.is_empty());
    }

    #[test]
    fn promotion_decision_reason_code_parse_accepts_known_values() {
        let code = PromotionDecisionReasonCode::parse("promotion_decision_threshold_failed")
            .expect("known reason should parse");
        assert_eq!(code, PromotionDecisionReasonCode::ThresholdFailed);
    }

    #[test]
    fn promotion_decision_validation_accepts_additional_missing_evidence_diagnostics() {
        let mut record = sample_promotion_decision_record();
        record
            .missing_evidence_fields
            .push("validation_run_id".to_string());

        validate_promotion_decision_record(&record)
            .expect("validation should allow additional diagnostics beyond FR45 packet gaps");
    }

    #[test]
    fn promotion_decision_validation_requires_fr45_missing_fields_to_be_present() {
        let mut record = sample_promotion_decision_record();
        record.missing_evidence_fields.clear();

        let error = validate_promotion_decision_record(&record)
            .expect_err("validation should fail if FR45 packet gaps are omitted");
        assert!(error.field_errors.iter().any(|issue| {
            issue.field == "missing_evidence_fields"
                && issue.code == PromotionDecisionReasonCode::MissingEvidence.code()
        }));
    }

    #[test]
    fn promotion_decision_canonicalization_normalizes_identifiers_and_orders_thresholds() {
        let mut record = sample_promotion_decision_record();
        record.decision_id = " Candidate::Alpha-1::1712448000 ".to_string();
        record.candidate_id = " Candidate::Alpha-1 ".to_string();
        record.validation_run_id = " Candidate::Alpha-1::1712447000 ".to_string();
        record.reason_code = " Promotion_Decision_Missing_Evidence ".to_string();
        record.threshold_results.reverse();

        let canonical = canonicalize_promotion_decision_record(&record)
            .expect("canonical promotion decision should validate");
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
        assert_eq!(canonical.threshold_results[0].metric_key, "max_drawdown");
        assert_eq!(
            canonical.threshold_results[1].metric_key,
            "out_of_sample_sharpe"
        );
    }

    #[test]
    fn promotion_decision_allow_validation_requires_signoff_and_no_missing_evidence() {
        let mut record = sample_promotion_decision_record();
        record.decision_state = PromotionDecisionState::Allowed;
        record.reason_code = PromotionDecisionReasonCode::DecisionAllowed
            .code()
            .to_string();
        record.approval_reference = None;

        let error = validate_promotion_decision_record(&record)
            .expect_err("allow decision without signoff should fail closed");
        assert!(error.field_errors.iter().any(|issue| {
            issue.field == "approval_reference"
                && issue.code == PromotionDecisionReasonCode::ApprovalRequired.code()
        }));
    }

    fn sample_alpha_health_record() -> AlphaHealthMetricRecord {
        AlphaHealthMetricRecord {
            metric_id: "alpha::mean-reversion::1712534400000000000".to_string(),
            alpha_id: "alpha::mean-reversion".to_string(),
            rolling_sharpe: 1.2,
            rolling_hit_rate: 0.58,
            rolling_drawdown: 0.07,
            stability_score: 0.81,
            windows: vec![
                AlphaHealthAttributionWindowMetrics {
                    window: AlphaHealthMetricWindow::ThirtyDays,
                    net_pnl: 345.0,
                    rolling_sharpe: 1.2,
                    rolling_hit_rate: 0.58,
                    rolling_drawdown: 0.07,
                    stability_score: 0.81,
                },
                AlphaHealthAttributionWindowMetrics {
                    window: AlphaHealthMetricWindow::OneHour,
                    net_pnl: 11.2,
                    rolling_sharpe: 1.25,
                    rolling_hit_rate: 0.62,
                    rolling_drawdown: 0.03,
                    stability_score: 0.84,
                },
                AlphaHealthAttributionWindowMetrics {
                    window: AlphaHealthMetricWindow::TwentyFourHours,
                    net_pnl: 84.3,
                    rolling_sharpe: 1.22,
                    rolling_hit_rate: 0.6,
                    rolling_drawdown: 0.05,
                    stability_score: 0.82,
                },
            ],
            reason_code: AlphaHealthReasonCode::MetricRecorded.code().to_string(),
            actor_id: "ops-1".to_string(),
            correlation_id: "corr-alpha-health-001".to_string(),
            recorded_at_utc: "2026-04-08T01:00:00Z".to_string(),
        }
    }

    #[test]
    fn alpha_health_reason_code_parse_accepts_known_values() {
        assert_eq!(
            AlphaHealthReasonCode::parse("alpha_health_threshold_breach_detected")
                .expect("known alpha-health reason code should parse"),
            AlphaHealthReasonCode::ThresholdBreachDetected
        );
        assert_eq!(
            AlphaHealthReasonCode::parse("alpha_health_metric_not_found")
                .expect("known alpha-health reason code should parse"),
            AlphaHealthReasonCode::MetricNotFound
        );
    }

    #[test]
    fn alpha_health_threshold_boundaries_apply_floor_and_ceiling_semantics() {
        let mut metric = sample_alpha_health_record();
        metric.rolling_sharpe = 1.0;
        metric.rolling_hit_rate = 0.5;
        metric.stability_score = 0.8;
        metric.rolling_drawdown = 0.1;

        let thresholds = vec![
            AlphaHealthThresholdDefinition {
                metric_key: AlphaHealthMetricKey::RollingSharpe,
                comparator: ValidationGateComparator::Lt,
                threshold_value: 1.0,
            },
            AlphaHealthThresholdDefinition {
                metric_key: AlphaHealthMetricKey::RollingHitRate,
                comparator: ValidationGateComparator::Lt,
                threshold_value: 0.5,
            },
            AlphaHealthThresholdDefinition {
                metric_key: AlphaHealthMetricKey::StabilityScore,
                comparator: ValidationGateComparator::Lt,
                threshold_value: 0.8,
            },
            AlphaHealthThresholdDefinition {
                metric_key: AlphaHealthMetricKey::RollingDrawdown,
                comparator: ValidationGateComparator::Gt,
                threshold_value: 0.1,
            },
        ];
        let breaches = evaluate_alpha_health_thresholds(&metric, &thresholds)
            .expect("exact boundary should stay on allow path");
        assert!(breaches.is_empty());

        metric.rolling_sharpe = 0.99;
        metric.rolling_drawdown = 0.1001;
        let breaches = evaluate_alpha_health_thresholds(&metric, &thresholds)
            .expect("strict floor/ceiling boundary breach evaluation should succeed");
        assert_eq!(breaches.len(), 2);
        assert!(breaches.iter().any(|breach| {
            breach.metric_key == AlphaHealthMetricKey::RollingSharpe
                && breach.reason_code == AlphaHealthReasonCode::ThresholdBreachDetected.code()
        }));
        assert!(breaches.iter().any(|breach| {
            breach.metric_key == AlphaHealthMetricKey::RollingDrawdown
                && breach.reason_code == AlphaHealthReasonCode::ThresholdBreachDetected.code()
        }));
    }

    #[test]
    fn alpha_health_threshold_rejects_comparator_mismatch() {
        let threshold = AlphaHealthThresholdDefinition {
            metric_key: AlphaHealthMetricKey::RollingDrawdown,
            comparator: ValidationGateComparator::Lt,
            threshold_value: 0.09,
        };
        let error = validate_alpha_health_threshold_definition(&threshold)
            .expect_err("drawdown threshold should require `gt` comparator");
        assert_eq!(error.code, AlphaHealthReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "comparator")
        );
    }

    #[test]
    fn alpha_health_thresholds_require_non_empty_definition_list() {
        let metric = sample_alpha_health_record();
        let error = evaluate_alpha_health_thresholds(&metric, &[])
            .expect_err("empty thresholds should fail validation");
        assert_eq!(error.code, AlphaHealthReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "thresholds")
        );
    }

    #[test]
    fn alpha_health_thresholds_reject_duplicate_metric_keys() {
        let metric = sample_alpha_health_record();
        let thresholds = vec![
            AlphaHealthThresholdDefinition {
                metric_key: AlphaHealthMetricKey::RollingSharpe,
                comparator: ValidationGateComparator::Lt,
                threshold_value: 1.0,
            },
            AlphaHealthThresholdDefinition {
                metric_key: AlphaHealthMetricKey::RollingSharpe,
                comparator: ValidationGateComparator::Lt,
                threshold_value: 1.1,
            },
        ];
        let error = evaluate_alpha_health_thresholds(&metric, &thresholds)
            .expect_err("duplicate threshold metric keys should fail validation");
        assert_eq!(error.code, AlphaHealthReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "thresholds")
        );
    }

    #[test]
    fn alpha_health_metric_validation_requires_fr10_windows() {
        let mut metric = sample_alpha_health_record();
        metric.windows.pop();

        let error = validate_alpha_health_metric_record(&metric)
            .expect_err("missing required 30d/24h/1h windows should fail");
        assert_eq!(error.code, AlphaHealthReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "windows")
        );
    }

    #[test]
    fn alpha_health_identifier_composition_is_canonical_and_deterministic() {
        let metric_id =
            compose_alpha_health_metric_id(" Alpha::Mean-Reversion ", "2026-04-08T01:00:00Z")
                .expect("metric id composition should succeed");
        assert!(metric_id.starts_with("alpha::mean-reversion::"));

        let breach_id = compose_alpha_threshold_breach_id(
            " Alpha::Mean-Reversion ",
            AlphaHealthMetricKey::RollingSharpe,
            "2026-04-08T01:00:00Z",
        )
        .expect("breach id composition should succeed");
        assert!(breach_id.contains("alpha::mean-reversion::rolling_sharpe::"));
    }

    fn sample_alpha_lifecycle_breach() -> AlphaThresholdBreachRecord {
        AlphaThresholdBreachRecord {
            breach_id: "alpha::mean-reversion::rolling_drawdown::1712534400000000000".to_string(),
            metric_id: "alpha::mean-reversion::1712534400000000000".to_string(),
            alpha_id: "alpha::mean-reversion".to_string(),
            metric_key: AlphaHealthMetricKey::RollingDrawdown,
            comparator: ValidationGateComparator::Gt,
            observed_value: 0.11,
            threshold_value: 0.1,
            breach_reason: "rolling_drawdown exceeded configured ceiling".to_string(),
            reason_code: AlphaHealthReasonCode::ThresholdBreachDetected
                .code()
                .to_string(),
            actor_id: "ops-1".to_string(),
            correlation_id: "corr-alpha-lifecycle-001".to_string(),
            breached_at_utc: "2026-04-08T01:00:00Z".to_string(),
        }
    }

    #[test]
    fn alpha_lifecycle_reason_code_parse_accepts_fail_closed_families() {
        assert_eq!(
            AlphaLifecycleReasonCode::parse("alpha_lifecycle_action_dependency_unavailable")
                .expect("known lifecycle reason code should parse"),
            AlphaLifecycleReasonCode::DependencyUnavailable
        );
        assert_eq!(
            AlphaLifecycleReasonCode::parse("alpha_lifecycle_action_state_unavailable")
                .expect("known lifecycle reason code should parse"),
            AlphaLifecycleReasonCode::StateUnavailable
        );
        assert_eq!(
            AlphaLifecycleReasonCode::parse("alpha_lifecycle_action_persistence_unavailable")
                .expect("known lifecycle reason code should parse"),
            AlphaLifecycleReasonCode::PersistenceUnavailable
        );
    }

    #[test]
    fn alpha_lifecycle_action_identifier_composition_is_canonical_and_deterministic() {
        let action_id =
            compose_alpha_lifecycle_action_id(" Alpha::Mean-Reversion ", "2026-04-08T01:00:00Z")
                .expect("action id composition should succeed");
        assert!(action_id.starts_with("alpha::mean-reversion::"));
    }

    #[test]
    fn alpha_lifecycle_action_identifier_with_context_prevents_collisions() {
        let first = compose_alpha_lifecycle_action_id_with_context(
            " Alpha::Mean-Reversion ",
            "deallocate",
            "Corr-001",
            "2026-04-08T01:00:00Z",
        )
        .expect("contextual action id composition should succeed");
        let second = compose_alpha_lifecycle_action_id_with_context(
            "Alpha::Mean-Reversion",
            "stop_research",
            "Corr-002",
            "2026-04-08T01:00:00Z",
        )
        .expect("contextual action id composition should succeed");
        assert!(first.starts_with("alpha::mean-reversion::deallocate::corr-001::"));
        assert!(second.starts_with("alpha::mean-reversion::stop_research::corr-002::"));
        assert_ne!(first, second);
    }

    #[test]
    fn alpha_lifecycle_fr47_deallocation_trigger_uses_canonical_breach_evidence() {
        let breach = sample_alpha_lifecycle_breach();
        let evaluation = evaluate_fr47_deallocation_trigger(
            std::slice::from_ref(&breach),
            &[AlphaLifecycleDeallocationPolicy {
                metric_key: AlphaHealthMetricKey::RollingDrawdown,
                comparator: ValidationGateComparator::Gt,
                threshold_value: 0.1,
            }],
        )
        .expect("deallocation evaluation should succeed");
        assert!(evaluation.triggered);
        assert_eq!(
            evaluation.triggered_criteria,
            vec!["deallocation_threshold:rolling_drawdown".to_string()]
        );
        assert_eq!(
            evaluation
                .trigger_breach
                .expect("trigger breach should be present")
                .breach_id,
            breach.breach_id
        );
    }

    #[test]
    fn alpha_lifecycle_fr47_deallocation_boundary_equality_is_allow_path() {
        let mut breach = sample_alpha_lifecycle_breach();
        breach.observed_value = 0.1;
        let evaluation = evaluate_fr47_deallocation_trigger(
            &[breach],
            &[AlphaLifecycleDeallocationPolicy {
                metric_key: AlphaHealthMetricKey::RollingDrawdown,
                comparator: ValidationGateComparator::Gt,
                threshold_value: 0.1,
            }],
        )
        .expect("deallocation evaluation should succeed");
        assert!(!evaluation.triggered);
        assert!(evaluation.triggered_criteria.is_empty());
    }

    #[test]
    fn alpha_lifecycle_fr47_uses_latest_breach_per_metric_for_trigger_evaluation() {
        let mut latest_breach = sample_alpha_lifecycle_breach();
        latest_breach.breach_id = "alpha::mean-reversion::rolling_drawdown::latest".to_string();
        latest_breach.observed_value = 0.08;
        latest_breach.breached_at_utc = "2026-04-08T02:00:00Z".to_string();

        let mut older_breach = sample_alpha_lifecycle_breach();
        older_breach.breach_id = "alpha::mean-reversion::rolling_drawdown::older".to_string();
        older_breach.observed_value = 0.12;
        older_breach.breached_at_utc = "2026-04-08T01:00:00Z".to_string();

        let evaluation = evaluate_fr47_deallocation_trigger(
            &[older_breach, latest_breach],
            &[AlphaLifecycleDeallocationPolicy {
                metric_key: AlphaHealthMetricKey::RollingDrawdown,
                comparator: ValidationGateComparator::Gt,
                threshold_value: 0.1,
            }],
        )
        .expect("deallocation evaluation should succeed");

        assert!(!evaluation.triggered);
        assert!(evaluation.triggered_criteria.is_empty());
        assert!(evaluation.trigger_breach.is_none());
    }

    #[test]
    fn alpha_lifecycle_fr48_stop_research_boundaries_keep_equality_on_allow_path() {
        let criteria = evaluate_fr48_stop_research_criteria(200, 0.2, 0.70)
            .expect("boundary criteria should evaluate");
        assert!(criteria.triggered_criteria.is_empty());
        assert!(!stop_research_triggered(&criteria));
    }

    #[test]
    fn alpha_lifecycle_fr48_stop_research_triggers_when_any_criterion_matches() {
        let criteria = evaluate_fr48_stop_research_criteria(199, 0.19, 0.71)
            .expect("criteria evaluation should succeed");
        assert!(stop_research_triggered(&criteria));
        assert_eq!(
            criteria.triggered_criteria,
            vec![
                "trade_count_30d_below_minimum".to_string(),
                "out_of_sample_sharpe_30d_below_minimum".to_string(),
                "promotion_failure_rate_last_10_above_maximum".to_string(),
            ]
        );

        let evidence = build_stop_research_trigger_evidence(&criteria);
        assert_eq!(
            evidence["criterion_keys"],
            serde_json::json!([
                "trade_count_30d_below_minimum",
                "out_of_sample_sharpe_30d_below_minimum",
                "promotion_failure_rate_last_10_above_maximum"
            ])
        );
    }

    #[test]
    fn alpha_lifecycle_record_validation_requires_remediation_for_unapplied_status() {
        let record = AlphaLifecycleActionRecord {
            action_id: "alpha::mean-reversion::1712534400000000000".to_string(),
            alpha_id: "alpha::mean-reversion".to_string(),
            action_type: AlphaLifecycleActionType::Deallocate,
            action_status: AlphaLifecycleActionStatus::Unapplied,
            reason_code: AlphaLifecycleReasonCode::AuthorizationFailed
                .code()
                .to_string(),
            trigger_evidence: serde_json::json!({
                "criterion_keys": ["deallocation_threshold:rolling_drawdown"]
            }),
            stop_research_criteria: None,
            remediation_guidance: None,
            actor_id: "ops-1".to_string(),
            correlation_id: "corr-alpha-lifecycle-001".to_string(),
            acted_at_utc: "2026-04-08T01:00:00Z".to_string(),
            approval_request_id: None,
            approval_reference: None,
        };
        let error = validate_alpha_lifecycle_action_record(&record)
            .expect_err("unapplied actions require remediation guidance");
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "remediation_guidance")
        );
    }

    #[test]
    fn alpha_lifecycle_record_validation_rejects_stop_research_criteria_for_deallocate_actions() {
        let record = AlphaLifecycleActionRecord {
            action_id: "alpha::mean-reversion::1712534400000000000".to_string(),
            alpha_id: "alpha::mean-reversion".to_string(),
            action_type: AlphaLifecycleActionType::Deallocate,
            action_status: AlphaLifecycleActionStatus::Applied,
            reason_code: AlphaLifecycleReasonCode::DeallocationThresholdBreached
                .code()
                .to_string(),
            trigger_evidence: serde_json::json!({
                "criterion_keys": ["deallocation_threshold:rolling_drawdown"]
            }),
            stop_research_criteria: Some(StopResearchCriteriaSnapshot {
                trade_count_30d: 180,
                out_of_sample_sharpe_30d: 0.19,
                promotion_failure_rate_last_10: 0.8,
                triggered_criteria: vec![
                    "trade_count_30d_below_minimum".to_string(),
                    "out_of_sample_sharpe_30d_below_minimum".to_string(),
                ],
            }),
            remediation_guidance: None,
            actor_id: "ops-1".to_string(),
            correlation_id: "corr-alpha-lifecycle-001".to_string(),
            acted_at_utc: "2026-04-08T01:00:00Z".to_string(),
            approval_request_id: None,
            approval_reference: None,
        };

        let error = validate_alpha_lifecycle_action_record(&record)
            .expect_err("deallocate actions must not carry stop_research criteria");
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "stop_research_criteria")
        );
    }
}
