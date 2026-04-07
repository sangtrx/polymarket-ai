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
}
