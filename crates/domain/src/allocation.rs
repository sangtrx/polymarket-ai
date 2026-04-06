use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

pub const DEFAULT_EXPOSURE_DRIFT_THRESHOLD_PCT: f64 = 10.0;
pub const DEFAULT_RELATIVE_ALPHA_DRIFT_THRESHOLD_PCT: f64 = 15.0;
pub const DEFAULT_POLICY_STALE_AFTER_SECONDS: f64 = 60.0;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AllocationApprovalStatus {
    NotRequired,
    Pending,
    Approved,
    Denied,
}

impl AllocationApprovalStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotRequired => "not_required",
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Denied => "denied",
        }
    }

    pub fn parse(value: &str) -> Result<Self, AllocationContractError> {
        match value {
            "not_required" => Ok(Self::NotRequired),
            "pending" => Ok(Self::Pending),
            "approved" => Ok(Self::Approved),
            "denied" => Ok(Self::Denied),
            _ => Err(AllocationContractError::invalid_payload(format!(
                "unknown allocation approval status `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RebalanceRecommendationStatus {
    Proposed,
    PendingApproval,
    Approved,
    Executed,
    Denied,
}

impl RebalanceRecommendationStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::PendingApproval => "pending_approval",
            Self::Approved => "approved",
            Self::Executed => "executed",
            Self::Denied => "denied",
        }
    }

    pub fn parse(value: &str) -> Result<Self, AllocationContractError> {
        match value {
            "proposed" => Ok(Self::Proposed),
            "pending_approval" => Ok(Self::PendingApproval),
            "approved" => Ok(Self::Approved),
            "executed" => Ok(Self::Executed),
            "denied" => Ok(Self::Denied),
            _ => Err(AllocationContractError::invalid_payload(format!(
                "unknown recommendation status `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RebalanceReasonCode {
    AllocationPolicyUpdated,
    AllocationPolicyPendingApproval,
    AllocationPolicyDenied,
    InBounds,
    DriftThresholdExceeded,
    PolicyStateUnavailable,
    PolicyStateStale,
    InvalidPayload,
    InvalidThreshold,
    ApprovalRequired,
    RecommendationProposed,
    RecommendationApproved,
    RecommendationExecuted,
    RecommendationDenied,
    RecommendationNotFound,
    PersistenceUnavailable,
}

impl RebalanceReasonCode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::AllocationPolicyUpdated => "allocation_policy_updated",
            Self::AllocationPolicyPendingApproval => "allocation_policy_pending_approval",
            Self::AllocationPolicyDenied => "allocation_policy_denied",
            Self::InBounds => "rebalance_in_bounds",
            Self::DriftThresholdExceeded => "rebalance_drift_threshold_exceeded",
            Self::PolicyStateUnavailable => "rebalance_policy_state_unavailable",
            Self::PolicyStateStale => "rebalance_policy_state_stale",
            Self::InvalidPayload => "rebalance_invalid_payload",
            Self::InvalidThreshold => "rebalance_invalid_threshold",
            Self::ApprovalRequired => "rebalance_approval_required",
            Self::RecommendationProposed => "rebalance_recommendation_proposed",
            Self::RecommendationApproved => "rebalance_recommendation_approved",
            Self::RecommendationExecuted => "rebalance_recommendation_executed",
            Self::RecommendationDenied => "rebalance_recommendation_denied",
            Self::RecommendationNotFound => "rebalance_recommendation_not_found",
            Self::PersistenceUnavailable => "rebalance_persistence_unavailable",
        }
    }

    pub fn parse(value: &str) -> Result<Self, AllocationContractError> {
        match value {
            "allocation_policy_updated" => Ok(Self::AllocationPolicyUpdated),
            "allocation_policy_pending_approval" => Ok(Self::AllocationPolicyPendingApproval),
            "allocation_policy_denied" => Ok(Self::AllocationPolicyDenied),
            "rebalance_in_bounds" => Ok(Self::InBounds),
            "rebalance_drift_threshold_exceeded" => Ok(Self::DriftThresholdExceeded),
            "rebalance_policy_state_unavailable" => Ok(Self::PolicyStateUnavailable),
            "rebalance_policy_state_stale" => Ok(Self::PolicyStateStale),
            "rebalance_invalid_payload" => Ok(Self::InvalidPayload),
            "rebalance_invalid_threshold" => Ok(Self::InvalidThreshold),
            "rebalance_approval_required" => Ok(Self::ApprovalRequired),
            "rebalance_recommendation_proposed" => Ok(Self::RecommendationProposed),
            "rebalance_recommendation_approved" => Ok(Self::RecommendationApproved),
            "rebalance_recommendation_executed" => Ok(Self::RecommendationExecuted),
            "rebalance_recommendation_denied" => Ok(Self::RecommendationDenied),
            "rebalance_recommendation_not_found" => Ok(Self::RecommendationNotFound),
            "rebalance_persistence_unavailable" => Ok(Self::PersistenceUnavailable),
            _ => Err(AllocationContractError::invalid_payload(format!(
                "unknown rebalance reason code `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AllocationPolicyVersion {
    pub policy_key: String,
    pub version: i64,
    pub portfolio_scope_id: String,
    pub target_exposure_pct_nav: f64,
    pub target_relative_alpha_weight: f64,
    pub exposure_drift_threshold_pct: f64,
    pub relative_alpha_drift_threshold_pct: f64,
    pub approval_status: AllocationApprovalStatus,
    pub approval_reference: Option<String>,
    pub actor_id: String,
    pub reason_code: String,
    pub correlation_id: String,
    pub updated_at_utc: String,
    #[serde(default = "empty_object")]
    pub advanced_parameters: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RebalanceRecommendation {
    pub recommendation_id: String,
    pub policy_key: String,
    pub policy_version: i64,
    pub status: RebalanceRecommendationStatus,
    pub approval_status: AllocationApprovalStatus,
    pub approval_reference: Option<String>,
    pub action_type: String,
    pub rationale: String,
    pub reason_code: String,
    pub actor_id: String,
    pub correlation_id: String,
    pub created_at_utc: String,
    pub updated_at_utc: String,
    #[serde(default = "empty_object")]
    pub parameters: Value,
    #[serde(default = "empty_object")]
    pub evidence: Value,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DriftEvaluationOutcome {
    InBounds,
    RecommendationRequired,
    FailClosed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RebalanceDriftEvaluation {
    pub policy_key: String,
    pub outcome: DriftEvaluationOutcome,
    pub reason_code: String,
    pub exposure_drift_pct: f64,
    pub relative_alpha_drift_pct: f64,
    pub exposure_threshold_pct: f64,
    pub relative_alpha_threshold_pct: f64,
    pub exceeded_dimensions: Vec<String>,
    pub evaluated_at_utc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AllocationValidationIssue {
    pub field: &'static str,
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AllocationContractError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<AllocationValidationIssue>,
}

impl AllocationContractError {
    pub fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: RebalanceReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn invalid_payload_with_issues(
        message: impl Into<String>,
        field_errors: Vec<AllocationValidationIssue>,
    ) -> Self {
        Self {
            code: RebalanceReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }
}

pub fn normalize_allocation_identifier(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}

pub fn validate_allocation_policy_version(
    policy: &AllocationPolicyVersion,
) -> Result<(), AllocationContractError> {
    let mut field_errors = Vec::new();
    validate_canonical_identifier(&mut field_errors, "policy_key", &policy.policy_key);
    validate_canonical_identifier(
        &mut field_errors,
        "portfolio_scope_id",
        &policy.portfolio_scope_id,
    );
    validate_non_empty_field(&mut field_errors, "actor_id", &policy.actor_id);
    validate_non_empty_field(&mut field_errors, "correlation_id", &policy.correlation_id);
    validate_non_empty_field(&mut field_errors, "reason_code", &policy.reason_code);
    validate_utc_timestamp_field(&mut field_errors, "updated_at_utc", &policy.updated_at_utc);

    if policy.version <= 0 {
        field_errors.push(AllocationValidationIssue {
            field: "version",
            code: RebalanceReasonCode::InvalidPayload.code(),
            message: "version must be greater than 0".to_string(),
        });
    }

    if RebalanceReasonCode::parse(&policy.reason_code).is_err() {
        field_errors.push(AllocationValidationIssue {
            field: "reason_code",
            code: RebalanceReasonCode::InvalidPayload.code(),
            message: "reason_code must be a known rebalance reason".to_string(),
        });
    }

    validate_percent_threshold(
        &mut field_errors,
        "target_exposure_pct_nav",
        policy.target_exposure_pct_nav,
    );
    validate_non_negative_threshold(
        &mut field_errors,
        "target_relative_alpha_weight",
        policy.target_relative_alpha_weight,
    );
    validate_percent_threshold(
        &mut field_errors,
        "exposure_drift_threshold_pct",
        policy.exposure_drift_threshold_pct,
    );
    validate_percent_threshold(
        &mut field_errors,
        "relative_alpha_drift_threshold_pct",
        policy.relative_alpha_drift_threshold_pct,
    );

    if !policy.advanced_parameters.is_object() {
        field_errors.push(AllocationValidationIssue {
            field: "advanced_parameters",
            code: RebalanceReasonCode::InvalidPayload.code(),
            message: "advanced_parameters must be a JSON object".to_string(),
        });
    }

    match policy.approval_status {
        AllocationApprovalStatus::Pending => {
            if policy
                .approval_reference
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty())
            {
                field_errors.push(AllocationValidationIssue {
                    field: "approval_reference",
                    code: RebalanceReasonCode::InvalidPayload.code(),
                    message: "pending allocation policy versions cannot include approval_reference"
                        .to_string(),
                });
            }
        }
        AllocationApprovalStatus::Approved => {
            if policy
                .approval_reference
                .as_deref()
                .is_some_and(|value| value.trim().is_empty())
            {
                field_errors.push(AllocationValidationIssue {
                    field: "approval_reference",
                    code: RebalanceReasonCode::InvalidPayload.code(),
                    message: "approval_reference cannot be blank when provided".to_string(),
                });
            }
        }
        AllocationApprovalStatus::Denied | AllocationApprovalStatus::NotRequired => {
            if policy
                .approval_reference
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty())
            {
                field_errors.push(AllocationValidationIssue {
                    field: "approval_reference",
                    code: RebalanceReasonCode::InvalidPayload.code(),
                    message:
                        "approval_reference is only permitted for approved allocation policies"
                            .to_string(),
                });
            }
        }
    }

    if !field_errors.is_empty() {
        return Err(AllocationContractError::invalid_payload_with_issues(
            "allocation policy payload is invalid",
            field_errors,
        ));
    }
    Ok(())
}

pub fn validate_rebalance_recommendation(
    recommendation: &RebalanceRecommendation,
) -> Result<(), AllocationContractError> {
    let mut field_errors = Vec::new();
    validate_canonical_identifier(
        &mut field_errors,
        "recommendation_id",
        &recommendation.recommendation_id,
    );
    validate_canonical_identifier(&mut field_errors, "policy_key", &recommendation.policy_key);
    validate_non_empty_field(
        &mut field_errors,
        "action_type",
        &recommendation.action_type,
    );
    validate_non_empty_field(&mut field_errors, "rationale", &recommendation.rationale);
    validate_non_empty_field(
        &mut field_errors,
        "reason_code",
        &recommendation.reason_code,
    );
    validate_non_empty_field(&mut field_errors, "actor_id", &recommendation.actor_id);
    validate_non_empty_field(
        &mut field_errors,
        "correlation_id",
        &recommendation.correlation_id,
    );
    validate_utc_timestamp_field(
        &mut field_errors,
        "created_at_utc",
        &recommendation.created_at_utc,
    );
    validate_utc_timestamp_field(
        &mut field_errors,
        "updated_at_utc",
        &recommendation.updated_at_utc,
    );

    if recommendation.policy_version <= 0 {
        field_errors.push(AllocationValidationIssue {
            field: "policy_version",
            code: RebalanceReasonCode::InvalidPayload.code(),
            message: "policy_version must be greater than 0".to_string(),
        });
    }

    if RebalanceReasonCode::parse(&recommendation.reason_code).is_err() {
        field_errors.push(AllocationValidationIssue {
            field: "reason_code",
            code: RebalanceReasonCode::InvalidPayload.code(),
            message: "reason_code must be a known rebalance reason".to_string(),
        });
    }

    if recommendation.action_type != "recommend" && recommendation.action_type != "execute" {
        field_errors.push(AllocationValidationIssue {
            field: "action_type",
            code: RebalanceReasonCode::InvalidPayload.code(),
            message: "action_type must be `recommend` or `execute`".to_string(),
        });
    }

    if !recommendation.parameters.is_object() {
        field_errors.push(AllocationValidationIssue {
            field: "parameters",
            code: RebalanceReasonCode::InvalidPayload.code(),
            message: "parameters must be a JSON object".to_string(),
        });
    }
    if !recommendation.evidence.is_object() {
        field_errors.push(AllocationValidationIssue {
            field: "evidence",
            code: RebalanceReasonCode::InvalidPayload.code(),
            message: "evidence must be a JSON object".to_string(),
        });
    }

    match recommendation.status {
        RebalanceRecommendationStatus::PendingApproval => {
            if recommendation.approval_status != AllocationApprovalStatus::Pending {
                field_errors.push(AllocationValidationIssue {
                    field: "approval_status",
                    code: RebalanceReasonCode::InvalidPayload.code(),
                    message:
                        "pending_approval recommendations must carry approval_status `pending`"
                            .to_string(),
                });
            }
        }
        RebalanceRecommendationStatus::Approved | RebalanceRecommendationStatus::Executed => {
            if recommendation.approval_status == AllocationApprovalStatus::Pending {
                field_errors.push(AllocationValidationIssue {
                    field: "approval_status",
                    code: RebalanceReasonCode::InvalidPayload.code(),
                    message:
                        "approved or executed recommendations cannot keep approval_status `pending`"
                            .to_string(),
                });
            }
        }
        RebalanceRecommendationStatus::Proposed | RebalanceRecommendationStatus::Denied => {}
    }

    match recommendation.approval_status {
        AllocationApprovalStatus::Approved => {
            if recommendation
                .approval_reference
                .as_deref()
                .map(str::trim)
                .is_none_or(str::is_empty)
            {
                field_errors.push(AllocationValidationIssue {
                    field: "approval_reference",
                    code: RebalanceReasonCode::InvalidPayload.code(),
                    message: "approval_reference is required when approval_status is `approved`"
                        .to_string(),
                });
            }
        }
        AllocationApprovalStatus::Pending
        | AllocationApprovalStatus::Denied
        | AllocationApprovalStatus::NotRequired => {
            if recommendation
                .approval_reference
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty())
            {
                field_errors.push(AllocationValidationIssue {
                    field: "approval_reference",
                    code: RebalanceReasonCode::InvalidPayload.code(),
                    message: "approval_reference is only valid when approval_status is `approved`"
                        .to_string(),
                });
            }
        }
    }

    if !field_errors.is_empty() {
        return Err(AllocationContractError::invalid_payload_with_issues(
            "rebalance recommendation payload is invalid",
            field_errors,
        ));
    }
    Ok(())
}

pub fn requires_allocation_critical_increase_approval(
    current: Option<&AllocationPolicyVersion>,
    proposed: &AllocationPolicyVersion,
) -> bool {
    let Some(current) = current else {
        return false;
    };
    if normalize_allocation_identifier(&current.policy_key)
        != normalize_allocation_identifier(&proposed.policy_key)
    {
        return false;
    }

    proposed.target_exposure_pct_nav > current.target_exposure_pct_nav
        || proposed.target_relative_alpha_weight > current.target_relative_alpha_weight
        || proposed.exposure_drift_threshold_pct > current.exposure_drift_threshold_pct
        || proposed.relative_alpha_drift_threshold_pct > current.relative_alpha_drift_threshold_pct
}

pub fn evaluate_rebalance_drift(
    policy: Option<&AllocationPolicyVersion>,
    exposure_drift_pct: f64,
    relative_alpha_drift_pct: f64,
    observed_at_utc: &str,
    stale_after_seconds: f64,
) -> Result<RebalanceDriftEvaluation, AllocationContractError> {
    let mut field_errors = Vec::new();
    validate_non_negative_threshold(&mut field_errors, "exposure_drift_pct", exposure_drift_pct);
    validate_non_negative_threshold(
        &mut field_errors,
        "relative_alpha_drift_pct",
        relative_alpha_drift_pct,
    );
    validate_utc_timestamp_field(&mut field_errors, "observed_at_utc", observed_at_utc);
    if !stale_after_seconds.is_finite() || stale_after_seconds <= 0.0 {
        field_errors.push(AllocationValidationIssue {
            field: "stale_after_seconds",
            code: RebalanceReasonCode::InvalidPayload.code(),
            message: "stale_after_seconds must be finite and greater than 0".to_string(),
        });
    }
    if !field_errors.is_empty() {
        return Err(AllocationContractError::invalid_payload_with_issues(
            "rebalance drift input is invalid",
            field_errors,
        ));
    }

    let observed_timestamp = parse_utc_timestamp(observed_at_utc).map_err(|_| {
        AllocationContractError::invalid_payload_with_issues(
            "rebalance drift input is invalid",
            vec![AllocationValidationIssue {
                field: "observed_at_utc",
                code: RebalanceReasonCode::InvalidPayload.code(),
                message: "observed_at_utc must be an RFC3339 UTC timestamp".to_string(),
            }],
        )
    })?;

    let Some(policy) = policy else {
        return Ok(RebalanceDriftEvaluation {
            policy_key: "unavailable_policy".to_string(),
            outcome: DriftEvaluationOutcome::FailClosed,
            reason_code: RebalanceReasonCode::PolicyStateUnavailable
                .code()
                .to_string(),
            exposure_drift_pct,
            relative_alpha_drift_pct,
            exposure_threshold_pct: DEFAULT_EXPOSURE_DRIFT_THRESHOLD_PCT,
            relative_alpha_threshold_pct: DEFAULT_RELATIVE_ALPHA_DRIFT_THRESHOLD_PCT,
            exceeded_dimensions: Vec::new(),
            evaluated_at_utc: observed_at_utc.to_string(),
        });
    };

    validate_allocation_policy_version(policy)?;
    let policy_timestamp = parse_utc_timestamp(&policy.updated_at_utc).map_err(|_| {
        AllocationContractError::invalid_payload_with_issues(
            "allocation policy payload is invalid",
            vec![AllocationValidationIssue {
                field: "updated_at_utc",
                code: RebalanceReasonCode::InvalidPayload.code(),
                message: "updated_at_utc must be an RFC3339 UTC timestamp".to_string(),
            }],
        )
    })?;

    let age_seconds = elapsed_seconds(policy_timestamp, observed_timestamp);
    if age_seconds > stale_after_seconds {
        return Ok(RebalanceDriftEvaluation {
            policy_key: policy.policy_key.clone(),
            outcome: DriftEvaluationOutcome::FailClosed,
            reason_code: RebalanceReasonCode::PolicyStateStale.code().to_string(),
            exposure_drift_pct,
            relative_alpha_drift_pct,
            exposure_threshold_pct: policy.exposure_drift_threshold_pct,
            relative_alpha_threshold_pct: policy.relative_alpha_drift_threshold_pct,
            exceeded_dimensions: Vec::new(),
            evaluated_at_utc: observed_at_utc.to_string(),
        });
    }

    let mut exceeded_dimensions = Vec::new();
    if exposure_drift_pct > policy.exposure_drift_threshold_pct {
        exceeded_dimensions.push("exposure_drift_pct".to_string());
    }
    if relative_alpha_drift_pct > policy.relative_alpha_drift_threshold_pct {
        exceeded_dimensions.push("relative_alpha_drift_pct".to_string());
    }

    let (outcome, reason_code) = if exceeded_dimensions.is_empty() {
        (
            DriftEvaluationOutcome::InBounds,
            RebalanceReasonCode::InBounds.code(),
        )
    } else {
        (
            DriftEvaluationOutcome::RecommendationRequired,
            RebalanceReasonCode::DriftThresholdExceeded.code(),
        )
    };

    Ok(RebalanceDriftEvaluation {
        policy_key: policy.policy_key.clone(),
        outcome,
        reason_code: reason_code.to_string(),
        exposure_drift_pct,
        relative_alpha_drift_pct,
        exposure_threshold_pct: policy.exposure_drift_threshold_pct,
        relative_alpha_threshold_pct: policy.relative_alpha_drift_threshold_pct,
        exceeded_dimensions,
        evaluated_at_utc: observed_at_utc.to_string(),
    })
}

fn empty_object() -> Value {
    Value::Object(serde_json::Map::new())
}

fn validate_non_empty_field(
    field_errors: &mut Vec<AllocationValidationIssue>,
    field: &'static str,
    value: &str,
) {
    if value.trim().is_empty() {
        field_errors.push(AllocationValidationIssue {
            field,
            code: RebalanceReasonCode::InvalidPayload.code(),
            message: format!("{field} cannot be blank"),
        });
    }
}

fn validate_canonical_identifier(
    field_errors: &mut Vec<AllocationValidationIssue>,
    field: &'static str,
    value: &str,
) {
    validate_non_empty_field(field_errors, field, value);
    let normalized = normalize_allocation_identifier(value);
    if !value.trim().eq(normalized.as_str()) {
        field_errors.push(AllocationValidationIssue {
            field,
            code: RebalanceReasonCode::InvalidPayload.code(),
            message: format!("{field} must be lowercase and whitespace-trimmed"),
        });
    }
}

fn validate_non_negative_threshold(
    field_errors: &mut Vec<AllocationValidationIssue>,
    field: &'static str,
    value: f64,
) {
    if !value.is_finite() {
        field_errors.push(AllocationValidationIssue {
            field,
            code: RebalanceReasonCode::InvalidThreshold.code(),
            message: format!("{field} must be finite"),
        });
        return;
    }
    if value < 0.0 {
        field_errors.push(AllocationValidationIssue {
            field,
            code: RebalanceReasonCode::InvalidThreshold.code(),
            message: format!("{field} must be greater than or equal to 0"),
        });
    }
}

fn validate_percent_threshold(
    field_errors: &mut Vec<AllocationValidationIssue>,
    field: &'static str,
    value: f64,
) {
    validate_non_negative_threshold(field_errors, field, value);
    if value.is_finite() && !(0.0..=100.0).contains(&value) {
        field_errors.push(AllocationValidationIssue {
            field,
            code: RebalanceReasonCode::InvalidThreshold.code(),
            message: format!("{field} must be between 0 and 100"),
        });
    }
}

fn validate_utc_timestamp_field(
    field_errors: &mut Vec<AllocationValidationIssue>,
    field: &'static str,
    value: &str,
) {
    if parse_utc_timestamp(value).is_err() {
        field_errors.push(AllocationValidationIssue {
            field,
            code: RebalanceReasonCode::InvalidPayload.code(),
            message: format!("{field} must be an RFC3339 UTC timestamp"),
        });
    }
}

fn parse_utc_timestamp(value: &str) -> Result<OffsetDateTime, ()> {
    let parsed = OffsetDateTime::parse(value, &Rfc3339).map_err(|_| ())?;
    if parsed.offset() != UtcOffset::UTC {
        return Err(());
    }
    Ok(parsed)
}

fn elapsed_seconds(start: OffsetDateTime, end: OffsetDateTime) -> f64 {
    let millis = (end - start).whole_milliseconds();
    if millis <= 0 {
        0.0
    } else {
        millis as f64 / 1_000.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_policy() -> AllocationPolicyVersion {
        AllocationPolicyVersion {
            policy_key: "portfolio-default".to_string(),
            version: 1,
            portfolio_scope_id: "portfolio::default".to_string(),
            target_exposure_pct_nav: 40.0,
            target_relative_alpha_weight: 1.2,
            exposure_drift_threshold_pct: DEFAULT_EXPOSURE_DRIFT_THRESHOLD_PCT,
            relative_alpha_drift_threshold_pct: DEFAULT_RELATIVE_ALPHA_DRIFT_THRESHOLD_PCT,
            approval_status: AllocationApprovalStatus::NotRequired,
            approval_reference: None,
            actor_id: "ops-1".to_string(),
            reason_code: RebalanceReasonCode::AllocationPolicyUpdated
                .code()
                .to_string(),
            correlation_id: "corr-alloc-001".to_string(),
            updated_at_utc: "2026-04-06T07:00:00Z".to_string(),
            advanced_parameters: serde_json::json!({
                "rebalance_window_minutes": 15
            }),
        }
    }

    fn sample_recommendation() -> RebalanceRecommendation {
        RebalanceRecommendation {
            recommendation_id: "reco::portfolio-default::1::corr-alloc-001".to_string(),
            policy_key: "portfolio-default".to_string(),
            policy_version: 1,
            status: RebalanceRecommendationStatus::Proposed,
            approval_status: AllocationApprovalStatus::NotRequired,
            approval_reference: None,
            action_type: "recommend".to_string(),
            rationale: "Exposure drift exceeded configured threshold.".to_string(),
            reason_code: RebalanceReasonCode::RecommendationProposed
                .code()
                .to_string(),
            actor_id: "ops-1".to_string(),
            correlation_id: "corr-alloc-001".to_string(),
            created_at_utc: "2026-04-06T07:00:00Z".to_string(),
            updated_at_utc: "2026-04-06T07:00:00Z".to_string(),
            parameters: serde_json::json!({
                "target_exposure_pct_nav": 40.0,
                "target_relative_alpha_weight": 1.2
            }),
            evidence: serde_json::json!({
                "exposure_drift_pct": 10.5,
                "relative_alpha_drift_pct": 12.0
            }),
        }
    }

    #[test]
    fn policy_validation_accepts_default_threshold_contract() {
        let policy = sample_policy();
        assert!(validate_allocation_policy_version(&policy).is_ok());
    }

    #[test]
    fn policy_validation_rejects_non_canonical_identifier() {
        let mut policy = sample_policy();
        policy.policy_key = "Portfolio-Default".to_string();
        let error = validate_allocation_policy_version(&policy)
            .expect_err("non-canonical policy identifiers should fail");
        assert_eq!(error.code, RebalanceReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "policy_key")
        );
    }

    #[test]
    fn drift_evaluation_exact_threshold_stays_in_bounds() {
        let policy = sample_policy();
        let evaluation = evaluate_rebalance_drift(
            Some(&policy),
            policy.exposure_drift_threshold_pct,
            policy.relative_alpha_drift_threshold_pct,
            "2026-04-06T07:00:15Z",
            DEFAULT_POLICY_STALE_AFTER_SECONDS,
        )
        .expect("equal threshold should remain in-bounds");
        assert_eq!(evaluation.outcome, DriftEvaluationOutcome::InBounds);
        assert_eq!(evaluation.reason_code, RebalanceReasonCode::InBounds.code());
    }

    #[test]
    fn drift_evaluation_strict_exceedance_requires_recommendation() {
        let policy = sample_policy();
        let evaluation = evaluate_rebalance_drift(
            Some(&policy),
            policy.exposure_drift_threshold_pct + 0.01,
            policy.relative_alpha_drift_threshold_pct,
            "2026-04-06T07:00:15Z",
            DEFAULT_POLICY_STALE_AFTER_SECONDS,
        )
        .expect("strict exceedance should trigger recommendation flow");
        assert_eq!(
            evaluation.outcome,
            DriftEvaluationOutcome::RecommendationRequired
        );
        assert_eq!(
            evaluation.reason_code,
            RebalanceReasonCode::DriftThresholdExceeded.code()
        );
        assert!(
            evaluation
                .exceeded_dimensions
                .iter()
                .any(|value| value == "exposure_drift_pct")
        );
    }

    #[test]
    fn drift_evaluation_fail_closed_when_policy_state_is_unavailable() {
        let evaluation = evaluate_rebalance_drift(
            None,
            20.0,
            25.0,
            "2026-04-06T07:00:15Z",
            DEFAULT_POLICY_STALE_AFTER_SECONDS,
        )
        .expect("unavailable policy must fail closed with deterministic reason");
        assert_eq!(evaluation.outcome, DriftEvaluationOutcome::FailClosed);
        assert_eq!(
            evaluation.reason_code,
            RebalanceReasonCode::PolicyStateUnavailable.code()
        );
    }

    #[test]
    fn drift_evaluation_fail_closed_when_policy_state_is_stale() {
        let policy = sample_policy();
        let evaluation =
            evaluate_rebalance_drift(Some(&policy), 2.0, 2.0, "2026-04-06T07:10:15Z", 30.0)
                .expect("stale policy state must fail closed");
        assert_eq!(evaluation.outcome, DriftEvaluationOutcome::FailClosed);
        assert_eq!(
            evaluation.reason_code,
            RebalanceReasonCode::PolicyStateStale.code()
        );
    }

    #[test]
    fn critical_approval_only_required_for_strict_policy_increase() {
        let baseline = sample_policy();
        let mut equal = baseline.clone();
        equal.version = 2;
        assert!(!requires_allocation_critical_increase_approval(
            Some(&baseline),
            &equal
        ));

        let mut increase = baseline.clone();
        increase.version = 2;
        increase.exposure_drift_threshold_pct += 1.0;
        assert!(requires_allocation_critical_increase_approval(
            Some(&baseline),
            &increase
        ));
    }

    #[test]
    fn recommendation_validation_enforces_approval_context_invariants() {
        let mut recommendation = sample_recommendation();
        recommendation.status = RebalanceRecommendationStatus::PendingApproval;
        recommendation.approval_status = AllocationApprovalStatus::Pending;
        recommendation.approval_reference = Some("apr-should-not-exist".to_string());

        let error = validate_rebalance_recommendation(&recommendation)
            .expect_err("pending approval recommendation cannot carry approval reference");
        assert_eq!(error.code, RebalanceReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "approval_reference")
        );
    }

    #[test]
    fn rebalance_reason_code_round_trip_is_deterministic() {
        let codes = [
            RebalanceReasonCode::InBounds,
            RebalanceReasonCode::DriftThresholdExceeded,
            RebalanceReasonCode::PolicyStateUnavailable,
            RebalanceReasonCode::PolicyStateStale,
            RebalanceReasonCode::ApprovalRequired,
        ];

        for code in codes {
            let parsed = RebalanceReasonCode::parse(code.code())
                .expect("known rebalance reason code should parse");
            assert_eq!(parsed, code);
        }
    }
}
