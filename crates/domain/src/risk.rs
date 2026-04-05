use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

const MARKET_EXPOSURE_MIN_PCT_NAV: f64 = 0.0;
const MARKET_EXPOSURE_MAX_PCT_NAV: f64 = 100.0;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RiskLimit {
    pub policy_key: String,
    pub max_notional_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MarketPolicyProfile {
    pub profile_id: String,
    pub cluster_id: String,
    pub min_liquidity_usd: f64,
    pub max_spread_bps: f64,
    pub min_reward_score: f64,
    pub max_exposure_pct_nav: f64,
    pub is_active: bool,
    pub actor_id: String,
    pub correlation_id: String,
    pub updated_at_utc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MarketClusterOverride {
    pub cluster_id: String,
    pub is_enabled: bool,
    pub reason_code: String,
    pub actor_id: String,
    pub correlation_id: String,
    pub updated_at_utc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MarketSnapshot {
    pub market_id: String,
    pub cluster_id: String,
    pub liquidity_depth_usd: f64,
    pub spread_bps: f64,
    pub reward_score: f64,
    pub projected_exposure_pct_nav: f64,
    pub observed_at_utc: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MarketEligibilityOutcome {
    Tradable,
    Blocked,
}

impl MarketEligibilityOutcome {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tradable => "tradable",
            Self::Blocked => "blocked",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MarketPolicyReasonCode {
    MarketEligible,
    PolicyStateUnavailable,
    ClusterDisabled,
    UnknownCluster,
    LiquidityBelowMinimum,
    SpreadAboveMaximum,
    RewardBelowMinimum,
    ExposureAboveMaximum,
    InvalidPayload,
    InvalidClusterId,
    InvalidThreshold,
    ProfileUpdated,
    ClusterEnabled,
    ClusterDisabledByOperator,
    PersistenceUnavailable,
}

impl MarketPolicyReasonCode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::MarketEligible => "market_policy_market_eligible",
            Self::PolicyStateUnavailable => "market_policy_state_unavailable",
            Self::ClusterDisabled => "market_policy_cluster_disabled",
            Self::UnknownCluster => "market_policy_unknown_cluster",
            Self::LiquidityBelowMinimum => "market_policy_liquidity_below_minimum",
            Self::SpreadAboveMaximum => "market_policy_spread_above_maximum",
            Self::RewardBelowMinimum => "market_policy_reward_below_minimum",
            Self::ExposureAboveMaximum => "market_policy_exposure_above_maximum",
            Self::InvalidPayload => "market_policy_invalid_payload",
            Self::InvalidClusterId => "market_policy_invalid_cluster_id",
            Self::InvalidThreshold => "market_policy_invalid_threshold",
            Self::ProfileUpdated => "market_policy_profile_updated",
            Self::ClusterEnabled => "market_policy_cluster_enabled",
            Self::ClusterDisabledByOperator => "market_policy_cluster_disabled_by_operator",
            Self::PersistenceUnavailable => "market_policy_persistence_unavailable",
        }
    }

    pub fn parse(value: &str) -> Result<Self, MarketPolicyContractError> {
        match value {
            "market_policy_market_eligible" => Ok(Self::MarketEligible),
            "market_policy_state_unavailable" => Ok(Self::PolicyStateUnavailable),
            "market_policy_cluster_disabled" => Ok(Self::ClusterDisabled),
            "market_policy_unknown_cluster" => Ok(Self::UnknownCluster),
            "market_policy_liquidity_below_minimum" => Ok(Self::LiquidityBelowMinimum),
            "market_policy_spread_above_maximum" => Ok(Self::SpreadAboveMaximum),
            "market_policy_reward_below_minimum" => Ok(Self::RewardBelowMinimum),
            "market_policy_exposure_above_maximum" => Ok(Self::ExposureAboveMaximum),
            "market_policy_invalid_payload" => Ok(Self::InvalidPayload),
            "market_policy_invalid_cluster_id" => Ok(Self::InvalidClusterId),
            "market_policy_invalid_threshold" => Ok(Self::InvalidThreshold),
            "market_policy_profile_updated" => Ok(Self::ProfileUpdated),
            "market_policy_cluster_enabled" => Ok(Self::ClusterEnabled),
            "market_policy_cluster_disabled_by_operator" => Ok(Self::ClusterDisabledByOperator),
            "market_policy_persistence_unavailable" => Ok(Self::PersistenceUnavailable),
            _ => Err(MarketPolicyContractError::invalid_payload(format!(
                "unknown market policy reason code `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MarketEligibilityDecision {
    pub market_id: String,
    pub cluster_id: String,
    pub outcome: MarketEligibilityOutcome,
    pub tradable: bool,
    pub reason_code: String,
    pub correlation_id: String,
    pub evaluated_at_utc: String,
}

impl MarketEligibilityDecision {
    fn tradable(
        market_id: &str,
        cluster_id: &str,
        correlation_id: &str,
        evaluated_at_utc: &str,
    ) -> Self {
        Self {
            market_id: market_id.to_string(),
            cluster_id: cluster_id.to_string(),
            outcome: MarketEligibilityOutcome::Tradable,
            tradable: true,
            reason_code: MarketPolicyReasonCode::MarketEligible.code().to_string(),
            correlation_id: correlation_id.to_string(),
            evaluated_at_utc: evaluated_at_utc.to_string(),
        }
    }

    fn blocked(
        market_id: &str,
        cluster_id: &str,
        reason_code: MarketPolicyReasonCode,
        correlation_id: &str,
        evaluated_at_utc: &str,
    ) -> Self {
        Self {
            market_id: market_id.to_string(),
            cluster_id: cluster_id.to_string(),
            outcome: MarketEligibilityOutcome::Blocked,
            tradable: false,
            reason_code: reason_code.code().to_string(),
            correlation_id: correlation_id.to_string(),
            evaluated_at_utc: evaluated_at_utc.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarketPolicyValidationIssue {
    pub field: &'static str,
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarketPolicyContractError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<MarketPolicyValidationIssue>,
}

impl MarketPolicyContractError {
    pub fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: MarketPolicyReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn invalid_payload_with_issues(
        message: impl Into<String>,
        field_errors: Vec<MarketPolicyValidationIssue>,
    ) -> Self {
        Self {
            code: MarketPolicyReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }
}

pub fn validate_market_policy_profile(
    profile: &MarketPolicyProfile,
) -> Result<(), MarketPolicyContractError> {
    let mut field_errors = Vec::new();

    validate_non_empty_field(&mut field_errors, "profile_id", &profile.profile_id);
    validate_non_empty_field(&mut field_errors, "cluster_id", &profile.cluster_id);
    validate_non_empty_field(&mut field_errors, "actor_id", &profile.actor_id);
    validate_non_empty_field(&mut field_errors, "correlation_id", &profile.correlation_id);
    validate_non_negative_threshold(
        &mut field_errors,
        "min_liquidity_usd",
        profile.min_liquidity_usd,
    );
    validate_non_negative_threshold(&mut field_errors, "max_spread_bps", profile.max_spread_bps);
    validate_non_negative_threshold(
        &mut field_errors,
        "min_reward_score",
        profile.min_reward_score,
    );
    validate_exposure_range(
        &mut field_errors,
        "max_exposure_pct_nav",
        profile.max_exposure_pct_nav,
    );
    validate_utc_timestamp_field(&mut field_errors, "updated_at_utc", &profile.updated_at_utc);

    if !field_errors.is_empty() {
        return Err(MarketPolicyContractError::invalid_payload_with_issues(
            "market policy profile contains invalid threshold fields",
            field_errors,
        ));
    }

    Ok(())
}

pub fn validate_market_cluster_override(
    override_state: &MarketClusterOverride,
) -> Result<(), MarketPolicyContractError> {
    let mut field_errors = Vec::new();
    validate_non_empty_field(&mut field_errors, "cluster_id", &override_state.cluster_id);
    validate_non_empty_field(
        &mut field_errors,
        "reason_code",
        &override_state.reason_code,
    );
    validate_non_empty_field(&mut field_errors, "actor_id", &override_state.actor_id);
    validate_non_empty_field(
        &mut field_errors,
        "correlation_id",
        &override_state.correlation_id,
    );
    validate_utc_timestamp_field(
        &mut field_errors,
        "updated_at_utc",
        &override_state.updated_at_utc,
    );

    if !field_errors.is_empty() {
        return Err(MarketPolicyContractError::invalid_payload_with_issues(
            "market cluster override payload is invalid",
            field_errors,
        ));
    }

    Ok(())
}

pub fn validate_market_snapshot(
    snapshot: &MarketSnapshot,
) -> Result<(), MarketPolicyContractError> {
    let mut field_errors = Vec::new();
    validate_non_empty_field(&mut field_errors, "market_id", &snapshot.market_id);
    validate_non_empty_field(&mut field_errors, "cluster_id", &snapshot.cluster_id);
    validate_non_negative_threshold(
        &mut field_errors,
        "liquidity_depth_usd",
        snapshot.liquidity_depth_usd,
    );
    validate_non_negative_threshold(&mut field_errors, "spread_bps", snapshot.spread_bps);
    validate_non_negative_threshold(&mut field_errors, "reward_score", snapshot.reward_score);
    validate_non_negative_threshold(
        &mut field_errors,
        "projected_exposure_pct_nav",
        snapshot.projected_exposure_pct_nav,
    );
    validate_utc_timestamp_field(
        &mut field_errors,
        "observed_at_utc",
        &snapshot.observed_at_utc,
    );

    if !field_errors.is_empty() {
        return Err(MarketPolicyContractError::invalid_payload_with_issues(
            "market snapshot payload is invalid",
            field_errors,
        ));
    }

    Ok(())
}

pub fn evaluate_market_eligibility(
    snapshot: Option<&MarketSnapshot>,
    profile: Option<&MarketPolicyProfile>,
    cluster_override: Option<&MarketClusterOverride>,
    correlation_id: &str,
    evaluated_at_utc: &str,
) -> MarketEligibilityDecision {
    let (market_id, cluster_id) = snapshot
        .map(|value| (value.market_id.as_str(), value.cluster_id.as_str()))
        .unwrap_or(("unknown_market", "unknown_cluster"));
    let correlation_id = normalize_identifier(correlation_id, "unknown_correlation");

    if parse_utc_timestamp(evaluated_at_utc).is_err() {
        return MarketEligibilityDecision::blocked(
            market_id,
            cluster_id,
            MarketPolicyReasonCode::InvalidPayload,
            correlation_id,
            evaluated_at_utc,
        );
    }

    let Some(snapshot) = snapshot else {
        return MarketEligibilityDecision::blocked(
            market_id,
            cluster_id,
            MarketPolicyReasonCode::PolicyStateUnavailable,
            correlation_id,
            evaluated_at_utc,
        );
    };
    if validate_market_snapshot(snapshot).is_err() {
        return MarketEligibilityDecision::blocked(
            &snapshot.market_id,
            &snapshot.cluster_id,
            MarketPolicyReasonCode::InvalidPayload,
            correlation_id,
            evaluated_at_utc,
        );
    }

    let (Some(profile), Some(cluster_override)) = (profile, cluster_override) else {
        return MarketEligibilityDecision::blocked(
            &snapshot.market_id,
            &snapshot.cluster_id,
            MarketPolicyReasonCode::PolicyStateUnavailable,
            correlation_id,
            evaluated_at_utc,
        );
    };
    if validate_market_policy_profile(profile).is_err()
        || validate_market_cluster_override(cluster_override).is_err()
    {
        return MarketEligibilityDecision::blocked(
            &snapshot.market_id,
            &snapshot.cluster_id,
            MarketPolicyReasonCode::PolicyStateUnavailable,
            correlation_id,
            evaluated_at_utc,
        );
    }

    let snapshot_cluster_id = snapshot.cluster_id.trim();
    if !profile
        .cluster_id
        .trim()
        .eq_ignore_ascii_case(snapshot_cluster_id)
        || !cluster_override
            .cluster_id
            .trim()
            .eq_ignore_ascii_case(snapshot_cluster_id)
    {
        return MarketEligibilityDecision::blocked(
            &snapshot.market_id,
            &snapshot.cluster_id,
            MarketPolicyReasonCode::UnknownCluster,
            correlation_id,
            evaluated_at_utc,
        );
    }
    if !profile.is_active {
        return MarketEligibilityDecision::blocked(
            &snapshot.market_id,
            &snapshot.cluster_id,
            MarketPolicyReasonCode::PolicyStateUnavailable,
            correlation_id,
            evaluated_at_utc,
        );
    }
    if !cluster_override.is_enabled {
        return MarketEligibilityDecision::blocked(
            &snapshot.market_id,
            &snapshot.cluster_id,
            MarketPolicyReasonCode::ClusterDisabled,
            correlation_id,
            evaluated_at_utc,
        );
    }
    if snapshot.liquidity_depth_usd < profile.min_liquidity_usd {
        return MarketEligibilityDecision::blocked(
            &snapshot.market_id,
            &snapshot.cluster_id,
            MarketPolicyReasonCode::LiquidityBelowMinimum,
            correlation_id,
            evaluated_at_utc,
        );
    }
    if snapshot.spread_bps > profile.max_spread_bps {
        return MarketEligibilityDecision::blocked(
            &snapshot.market_id,
            &snapshot.cluster_id,
            MarketPolicyReasonCode::SpreadAboveMaximum,
            correlation_id,
            evaluated_at_utc,
        );
    }
    if snapshot.reward_score < profile.min_reward_score {
        return MarketEligibilityDecision::blocked(
            &snapshot.market_id,
            &snapshot.cluster_id,
            MarketPolicyReasonCode::RewardBelowMinimum,
            correlation_id,
            evaluated_at_utc,
        );
    }
    if snapshot.projected_exposure_pct_nav > profile.max_exposure_pct_nav {
        return MarketEligibilityDecision::blocked(
            &snapshot.market_id,
            &snapshot.cluster_id,
            MarketPolicyReasonCode::ExposureAboveMaximum,
            correlation_id,
            evaluated_at_utc,
        );
    }

    MarketEligibilityDecision::tradable(
        &snapshot.market_id,
        &snapshot.cluster_id,
        correlation_id,
        evaluated_at_utc,
    )
}

fn validate_non_empty_field(
    field_errors: &mut Vec<MarketPolicyValidationIssue>,
    field: &'static str,
    value: &str,
) {
    if value.trim().is_empty() {
        field_errors.push(MarketPolicyValidationIssue {
            field,
            code: MarketPolicyReasonCode::InvalidPayload.code(),
            message: format!("{field} cannot be blank"),
        });
    }
}

fn validate_non_negative_threshold(
    field_errors: &mut Vec<MarketPolicyValidationIssue>,
    field: &'static str,
    value: f64,
) {
    if !value.is_finite() {
        field_errors.push(MarketPolicyValidationIssue {
            field,
            code: MarketPolicyReasonCode::InvalidThreshold.code(),
            message: format!("{field} must be finite"),
        });
        return;
    }
    if value < 0.0 {
        field_errors.push(MarketPolicyValidationIssue {
            field,
            code: MarketPolicyReasonCode::InvalidThreshold.code(),
            message: format!("{field} must be greater than or equal to 0"),
        });
    }
}

fn validate_exposure_range(
    field_errors: &mut Vec<MarketPolicyValidationIssue>,
    field: &'static str,
    value: f64,
) {
    if !value.is_finite() {
        field_errors.push(MarketPolicyValidationIssue {
            field,
            code: MarketPolicyReasonCode::InvalidThreshold.code(),
            message: format!("{field} must be finite"),
        });
        return;
    }
    if !(MARKET_EXPOSURE_MIN_PCT_NAV..=MARKET_EXPOSURE_MAX_PCT_NAV).contains(&value) {
        field_errors.push(MarketPolicyValidationIssue {
            field,
            code: MarketPolicyReasonCode::InvalidThreshold.code(),
            message: format!(
                "{field} must be between {MARKET_EXPOSURE_MIN_PCT_NAV} and {MARKET_EXPOSURE_MAX_PCT_NAV}"
            ),
        });
    }
}

fn validate_utc_timestamp_field(
    field_errors: &mut Vec<MarketPolicyValidationIssue>,
    field: &'static str,
    value: &str,
) {
    if parse_utc_timestamp(value).is_err() {
        field_errors.push(MarketPolicyValidationIssue {
            field,
            code: MarketPolicyReasonCode::InvalidPayload.code(),
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

fn normalize_identifier<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.trim().is_empty() {
        fallback
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_profile() -> MarketPolicyProfile {
        MarketPolicyProfile {
            profile_id: "policy_cluster_alpha".to_string(),
            cluster_id: "cluster_alpha".to_string(),
            min_liquidity_usd: 500.0,
            max_spread_bps: 4.5,
            min_reward_score: 0.4,
            max_exposure_pct_nav: 25.0,
            is_active: true,
            actor_id: "ops-1".to_string(),
            correlation_id: "corr-policy-001".to_string(),
            updated_at_utc: "2026-04-06T00:00:00Z".to_string(),
        }
    }

    fn sample_override() -> MarketClusterOverride {
        MarketClusterOverride {
            cluster_id: "cluster_alpha".to_string(),
            is_enabled: true,
            reason_code: MarketPolicyReasonCode::ClusterEnabled.code().to_string(),
            actor_id: "ops-1".to_string(),
            correlation_id: "corr-policy-001".to_string(),
            updated_at_utc: "2026-04-06T00:00:00Z".to_string(),
        }
    }

    fn sample_snapshot() -> MarketSnapshot {
        MarketSnapshot {
            market_id: "market_yes_no_1".to_string(),
            cluster_id: "cluster_alpha".to_string(),
            liquidity_depth_usd: 700.0,
            spread_bps: 2.5,
            reward_score: 0.7,
            projected_exposure_pct_nav: 12.0,
            observed_at_utc: "2026-04-06T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn profile_validation_rejects_negative_thresholds_and_exposure_gt_100() {
        let mut profile = sample_profile();
        profile.min_liquidity_usd = -1.0;
        profile.max_spread_bps = -0.1;
        profile.max_exposure_pct_nav = 100.1;

        let error =
            validate_market_policy_profile(&profile).expect_err("invalid thresholds must fail");
        let fields: Vec<_> = error.field_errors.iter().map(|issue| issue.field).collect();
        assert!(fields.contains(&"min_liquidity_usd"));
        assert!(fields.contains(&"max_spread_bps"));
        assert!(fields.contains(&"max_exposure_pct_nav"));
    }

    #[test]
    fn profile_validation_accepts_spread_zero_and_exposure_100_boundaries() {
        let mut profile = sample_profile();
        profile.max_spread_bps = 0.0;
        profile.max_exposure_pct_nav = 100.0;

        assert!(validate_market_policy_profile(&profile).is_ok());
    }

    #[test]
    fn evaluate_market_eligibility_allows_when_all_thresholds_are_met() {
        let decision = evaluate_market_eligibility(
            Some(&sample_snapshot()),
            Some(&sample_profile()),
            Some(&sample_override()),
            "corr-eval-001",
            "2026-04-06T00:00:00Z",
        );

        assert!(decision.tradable);
        assert_eq!(decision.outcome, MarketEligibilityOutcome::Tradable);
        assert_eq!(
            decision.reason_code,
            MarketPolicyReasonCode::MarketEligible.code()
        );
    }

    #[test]
    fn evaluate_market_eligibility_denies_when_cluster_is_disabled() {
        let mut override_state = sample_override();
        override_state.is_enabled = false;
        override_state.reason_code = MarketPolicyReasonCode::ClusterDisabledByOperator
            .code()
            .to_string();

        let decision = evaluate_market_eligibility(
            Some(&sample_snapshot()),
            Some(&sample_profile()),
            Some(&override_state),
            "corr-eval-002",
            "2026-04-06T00:00:00Z",
        );

        assert!(!decision.tradable);
        assert_eq!(decision.outcome, MarketEligibilityOutcome::Blocked);
        assert_eq!(
            decision.reason_code,
            MarketPolicyReasonCode::ClusterDisabled.code()
        );
    }

    #[test]
    fn evaluate_market_eligibility_denies_when_policy_state_is_missing() {
        let decision = evaluate_market_eligibility(
            Some(&sample_snapshot()),
            None,
            Some(&sample_override()),
            "corr-eval-003",
            "2026-04-06T00:00:00Z",
        );

        assert!(!decision.tradable);
        assert_eq!(
            decision.reason_code,
            MarketPolicyReasonCode::PolicyStateUnavailable.code()
        );
    }

    #[test]
    fn evaluate_market_eligibility_denies_when_spread_exceeds_threshold() {
        let mut snapshot = sample_snapshot();
        snapshot.spread_bps = 7.0;
        let decision = evaluate_market_eligibility(
            Some(&snapshot),
            Some(&sample_profile()),
            Some(&sample_override()),
            "corr-eval-004",
            "2026-04-06T00:00:00Z",
        );

        assert!(!decision.tradable);
        assert_eq!(
            decision.reason_code,
            MarketPolicyReasonCode::SpreadAboveMaximum.code()
        );
    }

    #[test]
    fn evaluate_market_eligibility_normalizes_cluster_id_whitespace_for_matching() {
        let mut snapshot = sample_snapshot();
        snapshot.cluster_id = " cluster_alpha ".to_string();
        let mut profile = sample_profile();
        profile.cluster_id = "cluster_alpha".to_string();
        let mut override_state = sample_override();
        override_state.cluster_id = "CLUSTER_ALPHA  ".to_string();

        let decision = evaluate_market_eligibility(
            Some(&snapshot),
            Some(&profile),
            Some(&override_state),
            "corr-eval-005",
            "2026-04-06T00:00:00Z",
        );

        assert!(decision.tradable);
        assert_eq!(
            decision.reason_code,
            MarketPolicyReasonCode::MarketEligible.code()
        );
    }
}
