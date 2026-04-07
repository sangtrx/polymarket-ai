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
        match (self, stage) {
            (Self::Training, ValidationGateWorkflowStage::Training)
            | (Self::Promotion, ValidationGateWorkflowStage::Promotion)
            | (Self::TrainingAndPromotion, _) => true,
            _ => false,
        }
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

    fn sample_promotion_decision_record() -> PromotionDecisionRecord {
        PromotionDecisionRecord {
            decision_id: "candidate::alpha-1::1712448000".to_string(),
            candidate_id: "candidate::alpha-1".to_string(),
            validation_run_id: "candidate::alpha-1::1712447000".to_string(),
            lifecycle_action: PromotionLifecycleAction::Promote,
            decision_state: PromotionDecisionState::Denied,
            reason_code: PromotionDecisionReasonCode::MissingEvidence.code().to_string(),
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
        assert_eq!(error.code, PromotionDecisionReasonCode::InvalidPayload.code());
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

        let non_promote_missing = validate_promotion_evidence_packet(
            PromotionLifecycleAction::Pause,
            &json!({}),
        )
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
        assert_eq!(canonical.threshold_results[1].metric_key, "out_of_sample_sharpe");
    }

    #[test]
    fn promotion_decision_allow_validation_requires_signoff_and_no_missing_evidence() {
        let mut record = sample_promotion_decision_record();
        record.decision_state = PromotionDecisionState::Allowed;
        record.reason_code = PromotionDecisionReasonCode::DecisionAllowed.code().to_string();
        record.approval_reference = None;

        let error = validate_promotion_decision_record(&record)
            .expect_err("allow decision without signoff should fail closed");
        assert!(error.field_errors.iter().any(|issue| {
            issue.field == "approval_reference"
                && issue.code == PromotionDecisionReasonCode::ApprovalRequired.code()
        }));
    }
}
