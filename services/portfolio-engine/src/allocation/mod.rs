use domain::allocation::{
    AllocationApprovalStatus, AllocationContractError, AllocationPolicyVersion,
    DriftEvaluationOutcome, RebalanceDriftEvaluation, RebalanceReasonCode,
    RebalanceRecommendationStatus, evaluate_rebalance_drift, normalize_allocation_identifier,
};
use serde::Serialize;
use serde_json::json;

#[derive(Debug, Clone)]
pub struct AllocationDriftObservation {
    pub policy_key: String,
    pub exposure_drift_pct: f64,
    pub relative_alpha_drift_pct: f64,
    pub observed_at_utc: String,
    pub stale_after_seconds: f64,
    pub correlation_id: String,
    pub require_execution: bool,
    pub approval_reference: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RebalanceRecommendationReadModel {
    pub recommendation_id: String,
    pub policy_key: String,
    pub policy_version: i64,
    pub recommendation_status: String,
    pub approval_status: String,
    pub action_type: String,
    pub rationale: String,
    pub recommended_next_action: String,
    pub reason_code: String,
    pub correlation_id: String,
    pub observed_at_utc: String,
    pub approval_reference: Option<String>,
    pub evidence: serde_json::Value,
}

pub fn evaluate_drift_boundaries(
    policy: Option<&AllocationPolicyVersion>,
    observation: &AllocationDriftObservation,
) -> Result<RebalanceDriftEvaluation, AllocationContractError> {
    evaluate_rebalance_drift(
        policy,
        observation.exposure_drift_pct,
        observation.relative_alpha_drift_pct,
        &observation.observed_at_utc,
        observation.stale_after_seconds,
    )
}

pub fn build_rebalance_recommendation_read_model(
    policy: Option<&AllocationPolicyVersion>,
    observation: &AllocationDriftObservation,
) -> Result<RebalanceRecommendationReadModel, AllocationContractError> {
    let evaluation = evaluate_drift_boundaries(policy, observation)?;

    if evaluation.outcome == DriftEvaluationOutcome::FailClosed {
        let parsed_reason = RebalanceReasonCode::parse(&evaluation.reason_code)
            .unwrap_or(RebalanceReasonCode::InvalidPayload);
        return Err(AllocationContractError {
            code: parsed_reason.code(),
            message: "rebalance recommendation evaluation failed closed due to unavailable or stale policy state"
                .to_string(),
            field_errors: Vec::new(),
        });
    }

    let Some(policy) = policy else {
        return Err(AllocationContractError {
            code: RebalanceReasonCode::PolicyStateUnavailable.code(),
            message: "allocation policy state is unavailable".to_string(),
            field_errors: Vec::new(),
        });
    };

    let recommendation_id = format!(
        "reco::{}::{}::{}",
        normalize_allocation_identifier(&observation.policy_key),
        policy.version,
        normalize_allocation_identifier(&observation.correlation_id),
    );

    if evaluation.outcome == DriftEvaluationOutcome::InBounds {
        return Ok(RebalanceRecommendationReadModel {
            recommendation_id,
            policy_key: normalize_allocation_identifier(&observation.policy_key),
            policy_version: policy.version,
            recommendation_status: RebalanceRecommendationStatus::Denied.as_str().to_string(),
            approval_status: AllocationApprovalStatus::NotRequired.as_str().to_string(),
            action_type: "recommend".to_string(),
            rationale:
                "Drift remains within configured thresholds; no rebalance recommendation generated."
                    .to_string(),
            recommended_next_action:
                "Continue monitoring drift telemetry; no rebalance action is required.".to_string(),
            reason_code: RebalanceReasonCode::InBounds.code().to_string(),
            correlation_id: observation.correlation_id.clone(),
            observed_at_utc: observation.observed_at_utc.clone(),
            approval_reference: None,
            evidence: json!({
                "thresholds": {
                    "exposure_drift_threshold_pct": evaluation.exposure_threshold_pct,
                    "relative_alpha_drift_threshold_pct": evaluation.relative_alpha_threshold_pct,
                },
                "exceeded_dimensions": evaluation.exceeded_dimensions,
                "require_execution": observation.require_execution,
                "policy_key": policy.policy_key,
                "policy_version": policy.version,
            }),
        });
    }

    let (recommendation_status, approval_status, reason_code, recommended_next_action) =
        if observation.require_execution && observation.approval_reference.is_none() {
            (
                RebalanceRecommendationStatus::PendingApproval.as_str(),
                AllocationApprovalStatus::Pending.as_str(),
                RebalanceReasonCode::ApprovalRequired.code(),
                "Complete dual approval for rebalance execution, then execute through control-api.",
            )
        } else if observation.require_execution {
            (
                RebalanceRecommendationStatus::Approved.as_str(),
                AllocationApprovalStatus::Approved.as_str(),
                RebalanceReasonCode::RecommendationApproved.code(),
                "Execute the approved rebalance recommendation and verify post-trade drift contraction.",
            )
        } else {
            (
                RebalanceRecommendationStatus::Proposed.as_str(),
                AllocationApprovalStatus::NotRequired.as_str(),
                RebalanceReasonCode::RecommendationProposed.code(),
                "Review recommendation rationale and execute rebalance if portfolio intent remains valid.",
            )
        };

    Ok(RebalanceRecommendationReadModel {
        recommendation_id,
        policy_key: normalize_allocation_identifier(&observation.policy_key),
        policy_version: policy.version,
        recommendation_status: recommendation_status.to_string(),
        approval_status: approval_status.to_string(),
        action_type: if observation.require_execution {
            "execute".to_string()
        } else {
            "recommend".to_string()
        },
        rationale: format!(
            "Drift exceeded threshold (exposure: {}%, alpha: {}%; thresholds: {}% / {}%).",
            observation.exposure_drift_pct,
            observation.relative_alpha_drift_pct,
            evaluation.exposure_threshold_pct,
            evaluation.relative_alpha_threshold_pct
        ),
        recommended_next_action: recommended_next_action.to_string(),
        reason_code: reason_code.to_string(),
        correlation_id: observation.correlation_id.clone(),
        observed_at_utc: observation.observed_at_utc.clone(),
        approval_reference: observation.approval_reference.clone(),
        evidence: json!({
            "thresholds": {
                "exposure_drift_threshold_pct": evaluation.exposure_threshold_pct,
                "relative_alpha_drift_threshold_pct": evaluation.relative_alpha_threshold_pct,
            },
            "exceeded_dimensions": evaluation.exceeded_dimensions,
            "require_execution": observation.require_execution,
            "policy_key": policy.policy_key,
            "policy_version": policy.version,
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::allocation::{
        DEFAULT_POLICY_STALE_AFTER_SECONDS, DEFAULT_RELATIVE_ALPHA_DRIFT_THRESHOLD_PCT,
        RebalanceReasonCode,
    };

    fn sample_policy(updated_at_utc: &str) -> AllocationPolicyVersion {
        AllocationPolicyVersion {
            policy_key: "portfolio-default".to_string(),
            version: 3,
            portfolio_scope_id: "portfolio::default".to_string(),
            target_exposure_pct_nav: 30.0,
            target_relative_alpha_weight: 1.2,
            exposure_drift_threshold_pct: 10.0,
            relative_alpha_drift_threshold_pct: DEFAULT_RELATIVE_ALPHA_DRIFT_THRESHOLD_PCT,
            approval_status: AllocationApprovalStatus::Approved,
            approval_reference: Some("apr-allocation-001".to_string()),
            actor_id: "ops-1".to_string(),
            reason_code: RebalanceReasonCode::AllocationPolicyUpdated
                .code()
                .to_string(),
            correlation_id: "corr-policy-001".to_string(),
            updated_at_utc: updated_at_utc.to_string(),
            advanced_parameters: json!({}),
        }
    }

    fn observation() -> AllocationDriftObservation {
        AllocationDriftObservation {
            policy_key: "portfolio-default".to_string(),
            exposure_drift_pct: 12.0,
            relative_alpha_drift_pct: 11.0,
            observed_at_utc: "2026-04-06T12:00:00Z".to_string(),
            stale_after_seconds: DEFAULT_POLICY_STALE_AFTER_SECONDS,
            correlation_id: "corr-rebalance-001".to_string(),
            require_execution: false,
            approval_reference: None,
        }
    }

    #[test]
    fn drift_equal_threshold_stays_in_bounds_deterministically() {
        let mut input = observation();
        input.exposure_drift_pct = 10.0;
        input.relative_alpha_drift_pct = 15.0;
        let policy = sample_policy("2026-04-06T11:59:00Z");

        let model = build_rebalance_recommendation_read_model(Some(&policy), &input)
            .expect("exact threshold should be in bounds");

        assert_eq!(model.recommendation_status, "denied");
        assert_eq!(model.reason_code, RebalanceReasonCode::InBounds.code());
    }

    #[test]
    fn drift_exceedance_generates_proposed_recommendation_read_model() {
        let input = observation();
        let policy = sample_policy("2026-04-06T11:59:00Z");

        let model = build_rebalance_recommendation_read_model(Some(&policy), &input)
            .expect("drift exceedance should generate recommendation");

        assert_eq!(model.recommendation_status, "proposed");
        assert_eq!(model.approval_status, "not_required");
        assert_eq!(
            model.reason_code,
            RebalanceReasonCode::RecommendationProposed.code()
        );
    }

    #[test]
    fn execution_path_without_approval_is_pending() {
        let mut input = observation();
        input.require_execution = true;
        let policy = sample_policy("2026-04-06T11:59:00Z");

        let model = build_rebalance_recommendation_read_model(Some(&policy), &input)
            .expect("execution path without approval should be pending");

        assert_eq!(model.recommendation_status, "pending_approval");
        assert_eq!(model.approval_status, "pending");
        assert_eq!(
            model.reason_code,
            RebalanceReasonCode::ApprovalRequired.code()
        );
    }

    #[test]
    fn execution_path_with_approval_is_approved() {
        let mut input = observation();
        input.require_execution = true;
        input.approval_reference = Some("apr-rebalance-001".to_string());
        let policy = sample_policy("2026-04-06T11:59:00Z");

        let model = build_rebalance_recommendation_read_model(Some(&policy), &input)
            .expect("execution path with approval should be approved");

        assert_eq!(model.recommendation_status, "approved");
        assert_eq!(model.approval_status, "approved");
        assert_eq!(
            model.reason_code,
            RebalanceReasonCode::RecommendationApproved.code()
        );
    }

    #[test]
    fn unavailable_policy_state_fails_closed() {
        let input = observation();
        let error = build_rebalance_recommendation_read_model(None, &input)
            .expect_err("missing policy should fail closed");
        assert_eq!(
            error.code,
            RebalanceReasonCode::PolicyStateUnavailable.code()
        );
    }

    #[test]
    fn stale_policy_state_fails_closed() {
        let input = observation();
        let policy = sample_policy("2026-04-06T11:40:00Z");

        let error = build_rebalance_recommendation_read_model(Some(&policy), &input)
            .expect_err("stale policy should fail closed");
        assert_eq!(error.code, RebalanceReasonCode::PolicyStateStale.code());
    }
}
