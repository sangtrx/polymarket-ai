use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

const MARKET_EXPOSURE_MIN_PCT_NAV: f64 = 0.0;
const MARKET_EXPOSURE_MAX_PCT_NAV: f64 = 100.0;
const RISK_LIMIT_PERCENT_MIN_PCT_NAV: f64 = 0.0;
const RISK_LIMIT_PERCENT_MAX_PCT_NAV: f64 = 100.0;
pub const MARKET_STREAM_LATENCY_TARGET_SECONDS: f64 = 2.0;
pub const USER_STREAM_LATENCY_TARGET_SECONDS: f64 = 2.0;
pub const MARKET_STREAM_BACKLOG_WARNING_SECONDS: f64 = 5.0;
pub const MARKET_STREAM_BACKLOG_DEGRADED_SECONDS: f64 = 10.0;
pub const MARKET_STREAM_BACKLOG_SUSTAINED_DURATION_SECONDS: f64 = 30.0;
pub const MARKET_STREAM_MAX_DEPTH_LEVELS: usize = 5;
pub const USER_STREAM_AUTH_STATE_PARTITION_KEY: &str = "__user_stream_auth_state__";
pub const FRESHNESS_STALE_THRESHOLD_SECONDS: f64 = 30.0;
pub const FRESHNESS_RECOVERY_STABILITY_WINDOW_SECONDS: f64 = 10.0;
pub const FRESHNESS_MAX_BREACH_TO_PAUSE_SECONDS: f64 = 5.0;
pub const REWARD_RISK_DEFAULT_THRESHOLD: f64 = 1.2;
pub const REWARD_RISK_MIN_VOLATILITY_BPS: f64 = 0.000_001;
pub const FR40_REBATE_DELTA_THRESHOLD_BPS: f64 = 20.0;
pub const FR40_SPREAD_WIDENING_THRESHOLD_BPS: f64 = 50.0;
pub const FR41_LOW_LIQUIDITY_DEPTH_USD_THRESHOLD: f64 = 10_000.0;
pub const FR41_INACTIVITY_PAUSE_THRESHOLD_SECONDS: f64 = 900.0;
pub const FR41_OVERNIGHT_CAP_THRESHOLD_SECONDS: f64 = 14_400.0;
pub const FR41_OVERNIGHT_CAP_FACTOR: f64 = 0.25;
pub const EMERGENCY_CONTROL_ACK_MAX_SECONDS: f64 = 1.0;
pub const EMERGENCY_CONTROL_STATE_REFLECTION_MAX_SECONDS: f64 = 5.0;
pub const EMERGENCY_CONTROL_SAFE_STATE_MAX_SECONDS: f64 = 5.0;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RiskLimit {
    pub policy_key: String,
    pub max_notional_usd: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RiskLimitScope {
    Portfolio,
    Market,
    Strategy,
}

impl RiskLimitScope {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Portfolio => "portfolio",
            Self::Market => "market",
            Self::Strategy => "strategy",
        }
    }

    pub fn parse(value: &str) -> Result<Self, RiskLimitContractError> {
        match value {
            "portfolio" => Ok(Self::Portfolio),
            "market" => Ok(Self::Market),
            "strategy" => Ok(Self::Strategy),
            _ => Err(RiskLimitContractError::invalid_payload(format!(
                "unknown risk limit scope `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RiskLimitProfileStatus {
    Active,
    Pending,
    Denied,
}

impl RiskLimitProfileStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Pending => "pending",
            Self::Denied => "denied",
        }
    }

    pub fn parse(value: &str) -> Result<Self, RiskLimitContractError> {
        match value {
            "active" => Ok(Self::Active),
            "pending" => Ok(Self::Pending),
            "denied" => Ok(Self::Denied),
            _ => Err(RiskLimitContractError::invalid_payload(format!(
                "unknown risk limit profile status `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RiskLimitReasonCode {
    ProfileApplied,
    ProfilePendingApproval,
    ProfileDenied,
    ApprovalRequired,
    InvalidPayload,
    InvalidThreshold,
    InvalidScopeInvariant,
    PolicyStateUnavailable,
    PersistenceUnavailable,
}

impl RiskLimitReasonCode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::ProfileApplied => "risk_limit_profile_applied",
            Self::ProfilePendingApproval => "risk_limit_profile_pending_approval",
            Self::ProfileDenied => "risk_limit_profile_denied",
            Self::ApprovalRequired => "risk_limit_approval_required",
            Self::InvalidPayload => "risk_limit_invalid_payload",
            Self::InvalidThreshold => "risk_limit_invalid_threshold",
            Self::InvalidScopeInvariant => "risk_limit_invalid_scope_invariant",
            Self::PolicyStateUnavailable => "risk_limit_policy_state_unavailable",
            Self::PersistenceUnavailable => "risk_limit_persistence_unavailable",
        }
    }

    pub fn parse(value: &str) -> Result<Self, RiskLimitContractError> {
        match value {
            "risk_limit_profile_applied" => Ok(Self::ProfileApplied),
            "risk_limit_profile_pending_approval" => Ok(Self::ProfilePendingApproval),
            "risk_limit_profile_denied" => Ok(Self::ProfileDenied),
            "risk_limit_approval_required" => Ok(Self::ApprovalRequired),
            "risk_limit_invalid_payload" => Ok(Self::InvalidPayload),
            "risk_limit_invalid_threshold" => Ok(Self::InvalidThreshold),
            "risk_limit_invalid_scope_invariant" => Ok(Self::InvalidScopeInvariant),
            "risk_limit_policy_state_unavailable" => Ok(Self::PolicyStateUnavailable),
            "risk_limit_persistence_unavailable" => Ok(Self::PersistenceUnavailable),
            _ => Err(RiskLimitContractError::invalid_payload(format!(
                "unknown risk limit reason code `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RiskScopeLimit {
    pub scope: RiskLimitScope,
    pub scope_id: String,
    pub max_notional_usd: f64,
    pub max_inventory_units: f64,
    pub max_concentration_pct_nav: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RiskLimitProfileVersion {
    pub profile_key: String,
    pub version: i64,
    pub portfolio: RiskScopeLimit,
    pub market: RiskScopeLimit,
    pub strategy: RiskScopeLimit,
    pub status: RiskLimitProfileStatus,
    pub approval_reference: Option<String>,
    pub actor_id: String,
    pub reason_code: String,
    pub correlation_id: String,
    pub updated_at_utc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InventoryLimitRule {
    pub rule_id: String,
    pub profile_key: String,
    pub profile_version: i64,
    pub scope: RiskLimitScope,
    pub scope_id: String,
    pub max_position_units: f64,
    pub max_order_size_units: f64,
    pub max_concentration_pct_nav: f64,
    pub actor_id: String,
    pub correlation_id: String,
    pub updated_at_utc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RiskLimitValidationIssue {
    pub field: &'static str,
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RiskLimitContractError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<RiskLimitValidationIssue>,
}

impl RiskLimitContractError {
    pub fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: RiskLimitReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn invalid_payload_with_issues(
        message: impl Into<String>,
        field_errors: Vec<RiskLimitValidationIssue>,
    ) -> Self {
        Self {
            code: RiskLimitReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }
}

pub fn validate_risk_limit_profile_version(
    profile: &RiskLimitProfileVersion,
) -> Result<(), RiskLimitContractError> {
    let mut field_errors = Vec::new();
    validate_non_empty_risk_limit_field(&mut field_errors, "profile_key", &profile.profile_key);
    validate_non_empty_risk_limit_field(&mut field_errors, "actor_id", &profile.actor_id);
    validate_non_empty_risk_limit_field(
        &mut field_errors,
        "correlation_id",
        &profile.correlation_id,
    );
    validate_non_empty_risk_limit_field(&mut field_errors, "reason_code", &profile.reason_code);
    validate_risk_limit_timestamp_field(
        &mut field_errors,
        "updated_at_utc",
        &profile.updated_at_utc,
    );

    if profile.version <= 0 {
        field_errors.push(RiskLimitValidationIssue {
            field: "version",
            code: RiskLimitReasonCode::InvalidPayload.code(),
            message: "version must be greater than 0".to_string(),
        });
    }

    if RiskLimitReasonCode::parse(&profile.reason_code).is_err() {
        field_errors.push(RiskLimitValidationIssue {
            field: "reason_code",
            code: RiskLimitReasonCode::InvalidPayload.code(),
            message: "reason_code must be a known risk limit reason".to_string(),
        });
    }

    if profile.status == RiskLimitProfileStatus::Pending
        && profile
            .approval_reference
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
    {
        field_errors.push(RiskLimitValidationIssue {
            field: "approval_reference",
            code: RiskLimitReasonCode::InvalidScopeInvariant.code(),
            message: "pending profiles cannot carry approval_reference until activated".to_string(),
        });
    }

    validate_scope_limit(
        &mut field_errors,
        &profile.portfolio,
        RiskLimitScope::Portfolio,
        "portfolio.scope",
        "portfolio.scope_id",
        "portfolio.max_notional_usd",
        "portfolio.max_inventory_units",
        "portfolio.max_concentration_pct_nav",
    );
    validate_scope_limit(
        &mut field_errors,
        &profile.market,
        RiskLimitScope::Market,
        "market.scope",
        "market.scope_id",
        "market.max_notional_usd",
        "market.max_inventory_units",
        "market.max_concentration_pct_nav",
    );
    validate_scope_limit(
        &mut field_errors,
        &profile.strategy,
        RiskLimitScope::Strategy,
        "strategy.scope",
        "strategy.scope_id",
        "strategy.max_notional_usd",
        "strategy.max_inventory_units",
        "strategy.max_concentration_pct_nav",
    );

    validate_child_not_above_parent(
        &mut field_errors,
        "market.max_notional_usd",
        profile.market.max_notional_usd,
        "portfolio.max_notional_usd",
        profile.portfolio.max_notional_usd,
    );
    validate_child_not_above_parent(
        &mut field_errors,
        "strategy.max_notional_usd",
        profile.strategy.max_notional_usd,
        "market.max_notional_usd",
        profile.market.max_notional_usd,
    );
    validate_child_not_above_parent(
        &mut field_errors,
        "market.max_inventory_units",
        profile.market.max_inventory_units,
        "portfolio.max_inventory_units",
        profile.portfolio.max_inventory_units,
    );
    validate_child_not_above_parent(
        &mut field_errors,
        "strategy.max_inventory_units",
        profile.strategy.max_inventory_units,
        "market.max_inventory_units",
        profile.market.max_inventory_units,
    );
    validate_child_not_above_parent(
        &mut field_errors,
        "market.max_concentration_pct_nav",
        profile.market.max_concentration_pct_nav,
        "portfolio.max_concentration_pct_nav",
        profile.portfolio.max_concentration_pct_nav,
    );
    validate_child_not_above_parent(
        &mut field_errors,
        "strategy.max_concentration_pct_nav",
        profile.strategy.max_concentration_pct_nav,
        "market.max_concentration_pct_nav",
        profile.market.max_concentration_pct_nav,
    );

    if !field_errors.is_empty() {
        return Err(RiskLimitContractError::invalid_payload_with_issues(
            "risk limit profile contains invalid scope thresholds",
            field_errors,
        ));
    }

    Ok(())
}

pub fn validate_inventory_limit_rule(
    rule: &InventoryLimitRule,
) -> Result<(), RiskLimitContractError> {
    let mut field_errors = Vec::new();
    validate_non_empty_risk_limit_field(&mut field_errors, "rule_id", &rule.rule_id);
    validate_non_empty_risk_limit_field(&mut field_errors, "profile_key", &rule.profile_key);
    validate_non_empty_risk_limit_field(&mut field_errors, "scope_id", &rule.scope_id);
    validate_non_empty_risk_limit_field(&mut field_errors, "actor_id", &rule.actor_id);
    validate_non_empty_risk_limit_field(&mut field_errors, "correlation_id", &rule.correlation_id);
    validate_risk_limit_timestamp_field(&mut field_errors, "updated_at_utc", &rule.updated_at_utc);
    validate_non_negative_risk_limit_value(
        &mut field_errors,
        "max_position_units",
        rule.max_position_units,
    );
    validate_non_negative_risk_limit_value(
        &mut field_errors,
        "max_order_size_units",
        rule.max_order_size_units,
    );
    validate_risk_limit_percent_range(
        &mut field_errors,
        "max_concentration_pct_nav",
        rule.max_concentration_pct_nav,
    );

    if rule.profile_version <= 0 {
        field_errors.push(RiskLimitValidationIssue {
            field: "profile_version",
            code: RiskLimitReasonCode::InvalidPayload.code(),
            message: "profile_version must be greater than 0".to_string(),
        });
    }

    if rule.scope == RiskLimitScope::Portfolio {
        field_errors.push(RiskLimitValidationIssue {
            field: "scope",
            code: RiskLimitReasonCode::InvalidScopeInvariant.code(),
            message: "inventory rules are only valid for market or strategy scopes".to_string(),
        });
    }

    if !field_errors.is_empty() {
        return Err(RiskLimitContractError::invalid_payload_with_issues(
            "inventory limit rule payload is invalid",
            field_errors,
        ));
    }

    Ok(())
}

pub fn requires_risk_limit_increase_approval(
    current: Option<&RiskLimitProfileVersion>,
    proposed: &RiskLimitProfileVersion,
) -> bool {
    let Some(current) = current else {
        return false;
    };
    if current.profile_key.trim() != proposed.profile_key.trim() {
        return false;
    }

    proposed.portfolio.max_notional_usd > current.portfolio.max_notional_usd
        || proposed.market.max_notional_usd > current.market.max_notional_usd
        || proposed.strategy.max_notional_usd > current.strategy.max_notional_usd
        || proposed.portfolio.max_inventory_units > current.portfolio.max_inventory_units
        || proposed.market.max_inventory_units > current.market.max_inventory_units
        || proposed.strategy.max_inventory_units > current.strategy.max_inventory_units
        || proposed.portfolio.max_concentration_pct_nav
            > current.portfolio.max_concentration_pct_nav
        || proposed.market.max_concentration_pct_nav > current.market.max_concentration_pct_nav
        || proposed.strategy.max_concentration_pct_nav > current.strategy.max_concentration_pct_nav
}

pub fn normalize_risk_limit_identifier(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
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
pub struct MarketBucketProfile {
    pub profile_id: String,
    pub market_id: String,
    pub cluster_id: String,
    pub bucket_type: String,
    pub risk_policy_key: String,
    pub allocation_policy_key: String,
    pub is_active: bool,
    pub actor_id: String,
    pub correlation_id: String,
    pub updated_at_utc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResolvedMarketPolicyLinks {
    pub profile_id: String,
    pub market_id: String,
    pub cluster_id: String,
    pub bucket_type: String,
    pub risk_policy_key: String,
    pub allocation_policy_key: String,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inactivity_gap_seconds: Option<f64>,
    pub spread_bps: f64,
    pub reward_score: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_reward_bps: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maker_rebate_bps: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_cost_bps: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_volatility_bps: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub venue_eligibility_state: Option<VenueEligibilityState>,
    pub projected_exposure_pct_nav: f64,
    pub observed_at_utc: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VenueEligibilityState {
    Eligible,
    Restricted,
    Ineligible,
}

impl VenueEligibilityState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Eligible => "eligible",
            Self::Restricted => "restricted",
            Self::Ineligible => "ineligible",
        }
    }

    pub fn parse(value: &str) -> Result<Self, RegimeShiftContractError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "eligible" => Ok(Self::Eligible),
            "restricted" => Ok(Self::Restricted),
            "ineligible" => Ok(Self::Ineligible),
            _ => Err(RegimeShiftContractError::invalid_payload_with_issues(
                format!("unsupported eligibility_state `{value}`"),
                vec![RegimeShiftValidationIssue {
                    field: "eligibility_state",
                    code: RegimeShiftReasonCode::InvalidPayload.code(),
                    message: "eligibility_state must be one of: eligible, restricted, ineligible"
                        .to_string(),
                }],
            )),
        }
    }

    pub const fn is_eligible(self) -> bool {
        matches!(self, Self::Eligible)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RegimeShiftReasonCode {
    RebateDeltaExceeded,
    SpreadWideningExceeded,
    EligibilityTransition,
    InvalidPayload,
    DependencyUnavailable,
    PersistenceUnavailable,
    EvidencePersisted,
    EvidenceRead,
}

impl RegimeShiftReasonCode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::RebateDeltaExceeded => "fr40_regime_rebate_delta_exceeded",
            Self::SpreadWideningExceeded => "fr40_regime_spread_widening_exceeded",
            Self::EligibilityTransition => "fr40_regime_eligibility_transition",
            Self::InvalidPayload => "fr40_regime_invalid_payload",
            Self::DependencyUnavailable => "fr40_regime_dependency_unavailable",
            Self::PersistenceUnavailable => "fr40_regime_persistence_unavailable",
            Self::EvidencePersisted => "fr40_regime_evidence_persisted",
            Self::EvidenceRead => "fr40_regime_evidence_read",
        }
    }

    pub fn parse(value: &str) -> Result<Self, RegimeShiftContractError> {
        match value {
            "fr40_regime_rebate_delta_exceeded" => Ok(Self::RebateDeltaExceeded),
            "fr40_regime_spread_widening_exceeded" => Ok(Self::SpreadWideningExceeded),
            "fr40_regime_eligibility_transition" => Ok(Self::EligibilityTransition),
            "fr40_regime_invalid_payload" => Ok(Self::InvalidPayload),
            "fr40_regime_dependency_unavailable" => Ok(Self::DependencyUnavailable),
            "fr40_regime_persistence_unavailable" => Ok(Self::PersistenceUnavailable),
            "fr40_regime_evidence_persisted" => Ok(Self::EvidencePersisted),
            "fr40_regime_evidence_read" => Ok(Self::EvidenceRead),
            _ => Err(RegimeShiftContractError::invalid_payload(format!(
                "unknown FR40 regime-shift reason code `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegimeShiftValidationIssue {
    pub field: &'static str,
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegimeShiftContractError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<RegimeShiftValidationIssue>,
}

impl RegimeShiftContractError {
    pub fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: RegimeShiftReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn invalid_payload_with_issues(
        message: impl Into<String>,
        field_errors: Vec<RegimeShiftValidationIssue>,
    ) -> Self {
        Self {
            code: RegimeShiftReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    pub fn dependency_unavailable_with_issues(
        message: impl Into<String>,
        field_errors: Vec<RegimeShiftValidationIssue>,
    ) -> Self {
        Self {
            code: RegimeShiftReasonCode::DependencyUnavailable.code(),
            message: message.into(),
            field_errors,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct RegimeShiftThresholds {
    pub rebate_delta_bps: f64,
    pub spread_widening_bps: f64,
}

impl Default for RegimeShiftThresholds {
    fn default() -> Self {
        Self {
            rebate_delta_bps: FR40_REBATE_DELTA_THRESHOLD_BPS,
            spread_widening_bps: FR40_SPREAD_WIDENING_THRESHOLD_BPS,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RegimeShiftDetection {
    pub market_id: String,
    pub cluster_id: String,
    pub reason_code: String,
    pub severity: String,
    pub correlation_id: String,
    pub observed_at_utc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_maker_rebate_bps: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_maker_rebate_bps: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rebate_delta_bps: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_spread_bps: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_spread_bps: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spread_widening_bps: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_eligibility_state: Option<VenueEligibilityState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_eligibility_state: Option<VenueEligibilityState>,
    pub threshold_rebate_delta_bps: f64,
    pub threshold_spread_widening_bps: f64,
}

pub fn evaluate_fr40_regime_shift(
    previous: &MarketSnapshot,
    current: &MarketSnapshot,
    correlation_id: &str,
    thresholds: Option<&RegimeShiftThresholds>,
) -> Result<Vec<RegimeShiftDetection>, RegimeShiftContractError> {
    let thresholds = thresholds.copied().unwrap_or_default();
    validate_regime_shift_thresholds(&thresholds)?;
    validate_regime_shift_identifiers(previous, current, correlation_id)?;
    let previous_observed_at = validate_regime_shift_observed_timestamp(
        "previous.observed_at_utc",
        &previous.observed_at_utc,
    )?;
    let current_observed_at = validate_regime_shift_observed_timestamp(
        "current.observed_at_utc",
        &current.observed_at_utc,
    )?;
    if current_observed_at < previous_observed_at {
        return Err(RegimeShiftContractError::invalid_payload_with_issues(
            "current.observed_at_utc must be greater than or equal to previous.observed_at_utc",
            vec![RegimeShiftValidationIssue {
                field: "current.observed_at_utc",
                code: RegimeShiftReasonCode::InvalidPayload.code(),
                message:
                    "current.observed_at_utc must be greater than or equal to previous.observed_at_utc"
                        .to_string(),
            }],
        ));
    }

    let mut dependency_issues = Vec::new();
    let previous_rebate = require_regime_shift_f64_option(
        &mut dependency_issues,
        "previous.maker_rebate_bps",
        previous.maker_rebate_bps,
    );
    let current_rebate = require_regime_shift_f64_option(
        &mut dependency_issues,
        "current.maker_rebate_bps",
        current.maker_rebate_bps,
    );
    let previous_eligibility = require_regime_shift_eligibility_state(
        &mut dependency_issues,
        "previous.venue_eligibility_state",
        previous.venue_eligibility_state,
    );
    let current_eligibility = require_regime_shift_eligibility_state(
        &mut dependency_issues,
        "current.venue_eligibility_state",
        current.venue_eligibility_state,
    );

    if !dependency_issues.is_empty() {
        return Err(
            RegimeShiftContractError::dependency_unavailable_with_issues(
                "required FR40 baseline dependencies are unavailable",
                dependency_issues,
            ),
        );
    }

    let previous_rebate = previous_rebate.expect("dependency validation should guarantee rebate");
    let current_rebate = current_rebate.expect("dependency validation should guarantee rebate");
    let previous_eligibility =
        previous_eligibility.expect("dependency validation should guarantee eligibility");
    let current_eligibility =
        current_eligibility.expect("dependency validation should guarantee eligibility");
    let normalized_correlation = correlation_id.trim().to_ascii_lowercase();
    let normalized_market_id = current.market_id.trim().to_ascii_lowercase();
    let normalized_cluster_id = current.cluster_id.trim().to_ascii_lowercase();

    let mut detections = Vec::new();
    let rebate_delta_bps = (current_rebate - previous_rebate).abs();
    let spread_widening_bps = (current.spread_bps - previous.spread_bps).max(0.0);
    if rebate_delta_bps > thresholds.rebate_delta_bps {
        detections.push(RegimeShiftDetection {
            market_id: normalized_market_id.clone(),
            cluster_id: normalized_cluster_id.clone(),
            reason_code: RegimeShiftReasonCode::RebateDeltaExceeded
                .code()
                .to_string(),
            severity: "critical".to_string(),
            correlation_id: normalized_correlation.clone(),
            observed_at_utc: current.observed_at_utc.clone(),
            previous_maker_rebate_bps: Some(previous_rebate),
            current_maker_rebate_bps: Some(current_rebate),
            rebate_delta_bps: Some(rebate_delta_bps),
            previous_spread_bps: Some(previous.spread_bps),
            current_spread_bps: Some(current.spread_bps),
            spread_widening_bps: Some(spread_widening_bps),
            previous_eligibility_state: Some(previous_eligibility),
            current_eligibility_state: Some(current_eligibility),
            threshold_rebate_delta_bps: thresholds.rebate_delta_bps,
            threshold_spread_widening_bps: thresholds.spread_widening_bps,
        });
    }

    if spread_widening_bps > thresholds.spread_widening_bps {
        detections.push(RegimeShiftDetection {
            market_id: normalized_market_id.clone(),
            cluster_id: normalized_cluster_id.clone(),
            reason_code: RegimeShiftReasonCode::SpreadWideningExceeded
                .code()
                .to_string(),
            severity: "critical".to_string(),
            correlation_id: normalized_correlation.clone(),
            observed_at_utc: current.observed_at_utc.clone(),
            previous_maker_rebate_bps: Some(previous_rebate),
            current_maker_rebate_bps: Some(current_rebate),
            rebate_delta_bps: Some(rebate_delta_bps),
            previous_spread_bps: Some(previous.spread_bps),
            current_spread_bps: Some(current.spread_bps),
            spread_widening_bps: Some(spread_widening_bps),
            previous_eligibility_state: Some(previous_eligibility),
            current_eligibility_state: Some(current_eligibility),
            threshold_rebate_delta_bps: thresholds.rebate_delta_bps,
            threshold_spread_widening_bps: thresholds.spread_widening_bps,
        });
    }

    if previous_eligibility.is_eligible() != current_eligibility.is_eligible() {
        detections.push(RegimeShiftDetection {
            market_id: normalized_market_id,
            cluster_id: normalized_cluster_id,
            reason_code: RegimeShiftReasonCode::EligibilityTransition
                .code()
                .to_string(),
            severity: "critical".to_string(),
            correlation_id: normalized_correlation,
            observed_at_utc: current.observed_at_utc.clone(),
            previous_maker_rebate_bps: Some(previous_rebate),
            current_maker_rebate_bps: Some(current_rebate),
            rebate_delta_bps: Some(rebate_delta_bps),
            previous_spread_bps: Some(previous.spread_bps),
            current_spread_bps: Some(current.spread_bps),
            spread_widening_bps: Some(spread_widening_bps),
            previous_eligibility_state: Some(previous_eligibility),
            current_eligibility_state: Some(current_eligibility),
            threshold_rebate_delta_bps: thresholds.rebate_delta_bps,
            threshold_spread_widening_bps: thresholds.spread_widening_bps,
        });
    }

    Ok(detections)
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct RewardRiskScoreInput {
    pub expected_reward_bps: f64,
    pub maker_rebate_bps: f64,
    pub expected_cost_bps: f64,
    pub expected_volatility_bps: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RewardRiskReasonCode {
    ScoreEligible,
    ScoreBelowThreshold,
    PolicyStateUnavailable,
    ScoreStateUnavailable,
    InvalidPayload,
    InvalidPolicyKey,
    InvalidThreshold,
    PolicyUpdated,
    PolicyRead,
    DefaultThresholdApplied,
    PersistenceUnavailable,
}

impl RewardRiskReasonCode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::ScoreEligible => "reward_risk_score_eligible",
            Self::ScoreBelowThreshold => "reward_risk_score_below_threshold",
            Self::PolicyStateUnavailable => "reward_risk_policy_state_unavailable",
            Self::ScoreStateUnavailable => "reward_risk_score_state_unavailable",
            Self::InvalidPayload => "reward_risk_invalid_payload",
            Self::InvalidPolicyKey => "reward_risk_invalid_policy_key",
            Self::InvalidThreshold => "reward_risk_invalid_threshold",
            Self::PolicyUpdated => "reward_risk_policy_updated",
            Self::PolicyRead => "reward_risk_policy_read",
            Self::DefaultThresholdApplied => "reward_risk_default_threshold_applied",
            Self::PersistenceUnavailable => "reward_risk_persistence_unavailable",
        }
    }

    pub fn parse(value: &str) -> Result<Self, RewardRiskContractError> {
        match value {
            "reward_risk_score_eligible" => Ok(Self::ScoreEligible),
            "reward_risk_score_below_threshold" => Ok(Self::ScoreBelowThreshold),
            "reward_risk_policy_state_unavailable" => Ok(Self::PolicyStateUnavailable),
            "reward_risk_score_state_unavailable" => Ok(Self::ScoreStateUnavailable),
            "reward_risk_invalid_payload" => Ok(Self::InvalidPayload),
            "reward_risk_invalid_policy_key" => Ok(Self::InvalidPolicyKey),
            "reward_risk_invalid_threshold" => Ok(Self::InvalidThreshold),
            "reward_risk_policy_updated" => Ok(Self::PolicyUpdated),
            "reward_risk_policy_read" => Ok(Self::PolicyRead),
            "reward_risk_default_threshold_applied" => Ok(Self::DefaultThresholdApplied),
            "reward_risk_persistence_unavailable" => Ok(Self::PersistenceUnavailable),
            _ => Err(RewardRiskContractError::invalid_payload(format!(
                "unknown reward-risk reason code `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RewardRiskValidationIssue {
    pub field: &'static str,
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RewardRiskContractError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<RewardRiskValidationIssue>,
}

impl RewardRiskContractError {
    pub fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: RewardRiskReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn invalid_payload_with_issues(
        message: impl Into<String>,
        field_errors: Vec<RewardRiskValidationIssue>,
    ) -> Self {
        Self {
            code: RewardRiskReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    pub fn state_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: RewardRiskReasonCode::ScoreStateUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardRiskPolicy {
    pub policy_key: String,
    pub strategy_key: String,
    pub min_reward_per_risk: f64,
    pub actor_id: String,
    pub correlation_id: String,
    pub updated_at_utc: String,
}

pub fn normalize_reward_risk_identifier(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}

pub fn validate_reward_risk_policy(
    policy: &RewardRiskPolicy,
) -> Result<(), RewardRiskContractError> {
    let mut field_errors = Vec::new();
    validate_reward_risk_non_empty_field(&mut field_errors, "policy_key", &policy.policy_key);
    validate_reward_risk_non_empty_field(&mut field_errors, "strategy_key", &policy.strategy_key);
    validate_reward_risk_non_empty_field(&mut field_errors, "actor_id", &policy.actor_id);
    validate_reward_risk_non_empty_field(
        &mut field_errors,
        "correlation_id",
        &policy.correlation_id,
    );
    validate_reward_risk_threshold_field(
        &mut field_errors,
        "min_reward_per_risk",
        policy.min_reward_per_risk,
    );
    validate_reward_risk_timestamp_field(
        &mut field_errors,
        "updated_at_utc",
        &policy.updated_at_utc,
    );

    if !field_errors.is_empty() {
        return Err(RewardRiskContractError::invalid_payload_with_issues(
            "reward-risk policy payload is invalid",
            field_errors,
        ));
    }
    Ok(())
}

pub fn validate_reward_risk_score_input(
    input: &RewardRiskScoreInput,
) -> Result<(), RewardRiskContractError> {
    let mut field_errors = Vec::new();
    validate_reward_risk_finite_field(
        &mut field_errors,
        "expected_reward_bps",
        input.expected_reward_bps,
    );
    validate_reward_risk_finite_field(
        &mut field_errors,
        "maker_rebate_bps",
        input.maker_rebate_bps,
    );
    validate_reward_risk_finite_field(
        &mut field_errors,
        "expected_cost_bps",
        input.expected_cost_bps,
    );
    validate_reward_risk_volatility_field(
        &mut field_errors,
        "expected_volatility_bps",
        input.expected_volatility_bps,
    );

    if !field_errors.is_empty() {
        return Err(RewardRiskContractError::invalid_payload_with_issues(
            "reward-risk score inputs are invalid",
            field_errors,
        ));
    }
    Ok(())
}

pub fn compute_reward_per_risk_score(
    input: &RewardRiskScoreInput,
) -> Result<f64, RewardRiskContractError> {
    validate_reward_risk_score_input(input)?;
    let score = (input.expected_reward_bps + input.maker_rebate_bps - input.expected_cost_bps)
        / input.expected_volatility_bps;
    if !score.is_finite() {
        return Err(RewardRiskContractError::invalid_payload(
            "computed reward-risk score must be finite",
        ));
    }
    Ok(score)
}

pub fn reward_risk_threshold_for_policy(policy: Option<&RewardRiskPolicy>) -> f64 {
    policy
        .map(|value| value.min_reward_per_risk)
        .unwrap_or(REWARD_RISK_DEFAULT_THRESHOLD)
}

pub fn reward_risk_score_input_from_snapshot(
    snapshot: &MarketSnapshot,
) -> Result<RewardRiskScoreInput, RewardRiskContractError> {
    let expected_reward_bps = snapshot.expected_reward_bps.ok_or_else(|| {
        RewardRiskContractError::state_unavailable(
            "market snapshot is missing `expected_reward_bps` input",
        )
    })?;
    let maker_rebate_bps = snapshot.maker_rebate_bps.ok_or_else(|| {
        RewardRiskContractError::state_unavailable(
            "market snapshot is missing `maker_rebate_bps` input",
        )
    })?;
    let expected_cost_bps = snapshot.expected_cost_bps.ok_or_else(|| {
        RewardRiskContractError::state_unavailable(
            "market snapshot is missing `expected_cost_bps` input",
        )
    })?;
    let expected_volatility_bps = snapshot.expected_volatility_bps.ok_or_else(|| {
        RewardRiskContractError::state_unavailable(
            "market snapshot is missing `expected_volatility_bps` input",
        )
    })?;

    let input = RewardRiskScoreInput {
        expected_reward_bps,
        maker_rebate_bps,
        expected_cost_bps,
        expected_volatility_bps,
    };
    validate_reward_risk_score_input(&input)?;
    Ok(input)
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MarketBucketType {
    Core,
    Satellite,
}

impl MarketBucketType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Satellite => "satellite",
        }
    }

    pub fn parse(value: &str) -> Result<Self, MarketBucketContractError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "core" => Ok(Self::Core),
            "satellite" => Ok(Self::Satellite),
            _ => Err(MarketBucketContractError::invalid_payload_with_issues(
                format!("unsupported bucket_type `{value}`"),
                vec![MarketBucketValidationIssue {
                    field: "bucket_type",
                    code: MarketBucketReasonCode::UnsupportedBucketType.code(),
                    message: "bucket_type must be one of: core, satellite".to_string(),
                }],
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MarketBucketReasonCode {
    MappingResolved,
    MappingUnavailable,
    MappingConflict,
    InvalidPayload,
    UnsupportedBucketType,
    ProfileUpdated,
    ProfileRead,
    PersistenceUnavailable,
}

impl MarketBucketReasonCode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::MappingResolved => "market_bucket_mapping_resolved",
            Self::MappingUnavailable => "market_bucket_mapping_unavailable",
            Self::MappingConflict => "market_bucket_mapping_conflict",
            Self::InvalidPayload => "market_bucket_invalid_payload",
            Self::UnsupportedBucketType => "market_bucket_unsupported_bucket_type",
            Self::ProfileUpdated => "market_bucket_profile_updated",
            Self::ProfileRead => "market_bucket_profile_read",
            Self::PersistenceUnavailable => "market_bucket_persistence_unavailable",
        }
    }

    pub fn parse(value: &str) -> Result<Self, MarketBucketContractError> {
        match value {
            "market_bucket_mapping_resolved" => Ok(Self::MappingResolved),
            "market_bucket_mapping_unavailable" => Ok(Self::MappingUnavailable),
            "market_bucket_mapping_conflict" => Ok(Self::MappingConflict),
            "market_bucket_invalid_payload" => Ok(Self::InvalidPayload),
            "market_bucket_unsupported_bucket_type" => Ok(Self::UnsupportedBucketType),
            "market_bucket_profile_updated" => Ok(Self::ProfileUpdated),
            "market_bucket_profile_read" => Ok(Self::ProfileRead),
            "market_bucket_persistence_unavailable" => Ok(Self::PersistenceUnavailable),
            _ => Err(MarketBucketContractError::invalid_payload(format!(
                "unknown market bucket reason code `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarketBucketValidationIssue {
    pub field: &'static str,
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarketBucketContractError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<MarketBucketValidationIssue>,
}

impl MarketBucketContractError {
    pub fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: MarketBucketReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn invalid_payload_with_issues(
        message: impl Into<String>,
        field_errors: Vec<MarketBucketValidationIssue>,
    ) -> Self {
        Self {
            code: MarketBucketReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    pub fn mapping_unavailable_with_issues(
        message: impl Into<String>,
        field_errors: Vec<MarketBucketValidationIssue>,
    ) -> Self {
        Self {
            code: MarketBucketReasonCode::MappingUnavailable.code(),
            message: message.into(),
            field_errors,
        }
    }

    pub fn mapping_conflict_with_issues(
        message: impl Into<String>,
        field_errors: Vec<MarketBucketValidationIssue>,
    ) -> Self {
        Self {
            code: MarketBucketReasonCode::MappingConflict.code(),
            message: message.into(),
            field_errors,
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

pub fn normalize_market_bucket_identifier(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}

pub fn canonical_market_bucket_profile_id(market_id: &str, cluster_id: &str) -> String {
    format!(
        "bucket::{}::{}",
        normalize_market_bucket_identifier(market_id),
        normalize_market_bucket_identifier(cluster_id)
    )
}

pub fn validate_market_bucket_profile(
    profile: &MarketBucketProfile,
) -> Result<(), MarketBucketContractError> {
    let mut field_errors = Vec::new();
    validate_market_bucket_non_empty_field(&mut field_errors, "profile_id", &profile.profile_id);
    validate_market_bucket_non_empty_field(&mut field_errors, "market_id", &profile.market_id);
    validate_market_bucket_non_empty_field(&mut field_errors, "cluster_id", &profile.cluster_id);
    validate_market_bucket_non_empty_field(&mut field_errors, "bucket_type", &profile.bucket_type);
    validate_market_bucket_non_empty_field(
        &mut field_errors,
        "risk_policy_key",
        &profile.risk_policy_key,
    );
    validate_market_bucket_non_empty_field(
        &mut field_errors,
        "allocation_policy_key",
        &profile.allocation_policy_key,
    );
    validate_market_bucket_non_empty_field(&mut field_errors, "actor_id", &profile.actor_id);
    validate_market_bucket_non_empty_field(
        &mut field_errors,
        "correlation_id",
        &profile.correlation_id,
    );
    validate_market_bucket_timestamp_field(
        &mut field_errors,
        "updated_at_utc",
        &profile.updated_at_utc,
    );
    if let Err(error) = MarketBucketType::parse(&profile.bucket_type) {
        field_errors.extend(error.field_errors);
    }

    if !field_errors.is_empty() {
        return Err(MarketBucketContractError::invalid_payload_with_issues(
            "market bucket profile payload is invalid",
            field_errors,
        ));
    }

    Ok(())
}

pub fn canonicalize_market_bucket_profile(
    profile: &MarketBucketProfile,
) -> Result<MarketBucketProfile, MarketBucketContractError> {
    validate_market_bucket_profile(profile)?;
    let bucket_type = MarketBucketType::parse(&profile.bucket_type)?;
    Ok(MarketBucketProfile {
        profile_id: normalize_market_bucket_identifier(&profile.profile_id),
        market_id: normalize_market_bucket_identifier(&profile.market_id),
        cluster_id: normalize_market_bucket_identifier(&profile.cluster_id),
        bucket_type: bucket_type.as_str().to_string(),
        risk_policy_key: normalize_market_bucket_identifier(&profile.risk_policy_key),
        allocation_policy_key: normalize_market_bucket_identifier(&profile.allocation_policy_key),
        is_active: profile.is_active,
        actor_id: normalize_market_bucket_identifier(&profile.actor_id),
        correlation_id: normalize_market_bucket_identifier(&profile.correlation_id),
        updated_at_utc: profile.updated_at_utc.clone(),
    })
}

pub fn resolve_market_bucket_policy_links(
    market_id: &str,
    cluster_id: &str,
    profiles: &[MarketBucketProfile],
) -> Result<ResolvedMarketPolicyLinks, MarketBucketContractError> {
    let normalized_market_id = normalize_market_bucket_identifier(market_id);
    let normalized_cluster_id = normalize_market_bucket_identifier(cluster_id);
    if normalized_market_id.is_empty() || normalized_cluster_id.is_empty() {
        return Err(MarketBucketContractError::invalid_payload_with_issues(
            "market_id and cluster_id are required to resolve market bucket policy links",
            vec![
                MarketBucketValidationIssue {
                    field: "market_id",
                    code: MarketBucketReasonCode::InvalidPayload.code(),
                    message: "market_id cannot be blank".to_string(),
                },
                MarketBucketValidationIssue {
                    field: "cluster_id",
                    code: MarketBucketReasonCode::InvalidPayload.code(),
                    message: "cluster_id cannot be blank".to_string(),
                },
            ],
        ));
    }

    let mut matches = Vec::new();
    for profile in profiles {
        if !profile.is_active {
            continue;
        }
        let canonical = canonicalize_market_bucket_profile(profile)?;
        if canonical.market_id == normalized_market_id && canonical.cluster_id == normalized_cluster_id
        {
            matches.push(canonical);
        }
    }

    match matches.len() {
        0 => Err(MarketBucketContractError::mapping_unavailable_with_issues(
            "no active market bucket mapping exists for the requested market/cluster",
            vec![MarketBucketValidationIssue {
                field: "market_id",
                code: MarketBucketReasonCode::MappingUnavailable.code(),
                message: format!(
                    "no active mapping for market_id `{normalized_market_id}` and cluster_id `{normalized_cluster_id}`"
                ),
            }],
        )),
        1 => {
            let profile = matches
                .pop()
                .expect("single-element vector should contain profile");
            Ok(ResolvedMarketPolicyLinks {
                profile_id: profile.profile_id,
                market_id: profile.market_id,
                cluster_id: profile.cluster_id,
                bucket_type: profile.bucket_type,
                risk_policy_key: profile.risk_policy_key,
                allocation_policy_key: profile.allocation_policy_key,
            })
        }
        _ => Err(MarketBucketContractError::mapping_conflict_with_issues(
            "multiple active market bucket mappings exist for the requested market/cluster",
            vec![MarketBucketValidationIssue {
                field: "profile_id",
                code: MarketBucketReasonCode::MappingConflict.code(),
                message: format!(
                    "expected one active mapping for market_id `{normalized_market_id}` and cluster_id `{normalized_cluster_id}`"
                ),
            }],
        )),
    }
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MarketStatus {
    Trading,
    Halted,
    Closed,
}

impl MarketStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Trading => "trading",
            Self::Halted => "halted",
            Self::Closed => "closed",
        }
    }

    pub fn parse(value: &str) -> Result<Self, MarketStreamContractError> {
        match value {
            "trading" => Ok(Self::Trading),
            "halted" => Ok(Self::Halted),
            "closed" => Ok(Self::Closed),
            _ => Err(MarketStreamContractError::invalid_payload(format!(
                "unknown market status `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MarketStreamHealthStatus {
    Healthy,
    Degraded,
}

impl MarketStreamHealthStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Healthy => "healthy",
            Self::Degraded => "degraded",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MarketStreamReasonCode {
    TickAccepted,
    InvalidPayload,
    UnsupportedEventShape,
    QuarantinedPayload,
    PersistenceUnavailable,
    StreamDisconnected,
    HeartbeatTimeout,
    BacklogWarning,
    BacklogExceeded,
    BacklogSustainedExceeded,
    Healthy,
    Degraded,
    LatencySloBreached,
}

impl MarketStreamReasonCode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::TickAccepted => "market_stream_tick_accepted",
            Self::InvalidPayload => "market_stream_invalid_payload",
            Self::UnsupportedEventShape => "market_stream_unsupported_event_shape",
            Self::QuarantinedPayload => "market_stream_quarantined_payload",
            Self::PersistenceUnavailable => "market_stream_persistence_unavailable",
            Self::StreamDisconnected => "market_stream_disconnected",
            Self::HeartbeatTimeout => "market_stream_heartbeat_timeout",
            Self::BacklogWarning => "market_stream_backlog_warning",
            Self::BacklogExceeded => "market_stream_backlog_exceeded",
            Self::BacklogSustainedExceeded => "market_stream_backlog_sustained_exceeded",
            Self::Healthy => "market_stream_healthy",
            Self::Degraded => "market_stream_degraded",
            Self::LatencySloBreached => "market_stream_latency_slo_breached",
        }
    }

    pub fn parse(value: &str) -> Result<Self, MarketStreamContractError> {
        match value {
            "market_stream_tick_accepted" => Ok(Self::TickAccepted),
            "market_stream_invalid_payload" => Ok(Self::InvalidPayload),
            "market_stream_unsupported_event_shape" => Ok(Self::UnsupportedEventShape),
            "market_stream_quarantined_payload" => Ok(Self::QuarantinedPayload),
            "market_stream_persistence_unavailable" => Ok(Self::PersistenceUnavailable),
            "market_stream_disconnected" => Ok(Self::StreamDisconnected),
            "market_stream_heartbeat_timeout" => Ok(Self::HeartbeatTimeout),
            "market_stream_backlog_warning" => Ok(Self::BacklogWarning),
            "market_stream_backlog_exceeded" => Ok(Self::BacklogExceeded),
            "market_stream_backlog_sustained_exceeded" => Ok(Self::BacklogSustainedExceeded),
            "market_stream_healthy" => Ok(Self::Healthy),
            "market_stream_degraded" => Ok(Self::Degraded),
            "market_stream_latency_slo_breached" => Ok(Self::LatencySloBreached),
            _ => Err(MarketStreamContractError::invalid_payload(format!(
                "unknown market stream reason code `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MarketDepthLevel {
    pub price: f64,
    pub size: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MarketStreamTick {
    pub tick_id: String,
    pub market_id: String,
    pub asset_id: String,
    pub cluster_id: String,
    pub best_bid: f64,
    pub best_ask: f64,
    pub top_bid_depth: Vec<MarketDepthLevel>,
    pub top_ask_depth: Vec<MarketDepthLevel>,
    pub last_trade_price: f64,
    pub tick_size: f64,
    pub market_status: MarketStatus,
    pub correlation_id: String,
    pub observed_at_utc: String,
    pub ingested_at_utc: String,
    pub ingestion_latency_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuarantinedMarketEvent {
    pub event_id: String,
    pub reason_code: String,
    pub correlation_id: String,
    pub observed_at_utc: String,
    pub quarantined_at_utc: String,
    pub ingestion_latency_seconds: f64,
    pub raw_payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MarketStreamHealth {
    pub health_event_id: String,
    pub stream_name: String,
    pub status: MarketStreamHealthStatus,
    pub reason_code: String,
    pub backlog_seconds: f64,
    pub sustained_backlog_seconds: f64,
    pub heartbeat_gap_seconds: f64,
    pub correlation_id: String,
    pub observed_at_utc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarketStreamValidationIssue {
    pub field: &'static str,
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarketStreamContractError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<MarketStreamValidationIssue>,
}

impl MarketStreamContractError {
    pub fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: MarketStreamReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn invalid_payload_with_issues(
        message: impl Into<String>,
        field_errors: Vec<MarketStreamValidationIssue>,
    ) -> Self {
        Self {
            code: MarketStreamReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarketStreamHealthAssessment {
    pub status: MarketStreamHealthStatus,
    pub reason_code: String,
}

pub fn validate_market_stream_tick(
    tick: &MarketStreamTick,
) -> Result<(), MarketStreamContractError> {
    let mut field_errors = Vec::new();
    validate_non_empty_stream_field(&mut field_errors, "tick_id", &tick.tick_id);
    validate_non_empty_stream_field(&mut field_errors, "market_id", &tick.market_id);
    validate_non_empty_stream_field(&mut field_errors, "asset_id", &tick.asset_id);
    validate_non_empty_stream_field(&mut field_errors, "cluster_id", &tick.cluster_id);
    validate_non_empty_stream_field(&mut field_errors, "correlation_id", &tick.correlation_id);
    validate_non_negative_stream_value(&mut field_errors, "best_bid", tick.best_bid);
    validate_non_negative_stream_value(&mut field_errors, "best_ask", tick.best_ask);
    validate_non_negative_stream_value(
        &mut field_errors,
        "last_trade_price",
        tick.last_trade_price,
    );
    validate_positive_stream_value(&mut field_errors, "tick_size", tick.tick_size);
    validate_non_negative_stream_value(
        &mut field_errors,
        "ingestion_latency_seconds",
        tick.ingestion_latency_seconds,
    );
    validate_market_depth_levels(&mut field_errors, "top_bid_depth", &tick.top_bid_depth);
    validate_market_depth_levels(&mut field_errors, "top_ask_depth", &tick.top_ask_depth);
    validate_stream_timestamp_field(&mut field_errors, "observed_at_utc", &tick.observed_at_utc);
    validate_stream_timestamp_field(&mut field_errors, "ingested_at_utc", &tick.ingested_at_utc);

    if tick.best_ask < tick.best_bid {
        field_errors.push(MarketStreamValidationIssue {
            field: "best_ask",
            code: MarketStreamReasonCode::InvalidPayload.code(),
            message: "best_ask must be greater than or equal to best_bid".to_string(),
        });
    }

    if let (Ok(observed_at), Ok(ingested_at)) = (
        parse_utc_timestamp(&tick.observed_at_utc),
        parse_utc_timestamp(&tick.ingested_at_utc),
    ) && ingested_at < observed_at
    {
        field_errors.push(MarketStreamValidationIssue {
            field: "ingested_at_utc",
            code: MarketStreamReasonCode::InvalidPayload.code(),
            message: "ingested_at_utc cannot be earlier than observed_at_utc".to_string(),
        });
    }

    if !field_errors.is_empty() {
        return Err(MarketStreamContractError::invalid_payload_with_issues(
            "market stream tick payload is invalid",
            field_errors,
        ));
    }

    Ok(())
}

pub fn validate_quarantined_market_event(
    event: &QuarantinedMarketEvent,
) -> Result<(), MarketStreamContractError> {
    let mut field_errors = Vec::new();
    validate_non_empty_stream_field(&mut field_errors, "event_id", &event.event_id);
    validate_non_empty_stream_field(&mut field_errors, "reason_code", &event.reason_code);
    validate_non_empty_stream_field(&mut field_errors, "correlation_id", &event.correlation_id);
    validate_stream_timestamp_field(&mut field_errors, "observed_at_utc", &event.observed_at_utc);
    validate_stream_timestamp_field(
        &mut field_errors,
        "quarantined_at_utc",
        &event.quarantined_at_utc,
    );
    validate_non_negative_stream_value(
        &mut field_errors,
        "ingestion_latency_seconds",
        event.ingestion_latency_seconds,
    );
    if event.raw_payload.is_null() {
        field_errors.push(MarketStreamValidationIssue {
            field: "raw_payload",
            code: MarketStreamReasonCode::InvalidPayload.code(),
            message: "raw_payload cannot be null".to_string(),
        });
    }

    if MarketStreamReasonCode::parse(&event.reason_code).is_err() {
        field_errors.push(MarketStreamValidationIssue {
            field: "reason_code",
            code: MarketStreamReasonCode::InvalidPayload.code(),
            message: "reason_code must be a known market stream reason".to_string(),
        });
    }

    if !field_errors.is_empty() {
        return Err(MarketStreamContractError::invalid_payload_with_issues(
            "quarantined market event payload is invalid",
            field_errors,
        ));
    }

    Ok(())
}

pub fn validate_market_stream_health(
    health: &MarketStreamHealth,
) -> Result<(), MarketStreamContractError> {
    let mut field_errors = Vec::new();
    validate_non_empty_stream_field(
        &mut field_errors,
        "health_event_id",
        &health.health_event_id,
    );
    validate_non_empty_stream_field(&mut field_errors, "stream_name", &health.stream_name);
    validate_non_empty_stream_field(&mut field_errors, "reason_code", &health.reason_code);
    validate_non_empty_stream_field(&mut field_errors, "correlation_id", &health.correlation_id);
    validate_stream_timestamp_field(
        &mut field_errors,
        "observed_at_utc",
        &health.observed_at_utc,
    );
    validate_non_negative_stream_value(
        &mut field_errors,
        "backlog_seconds",
        health.backlog_seconds,
    );
    validate_non_negative_stream_value(
        &mut field_errors,
        "sustained_backlog_seconds",
        health.sustained_backlog_seconds,
    );
    validate_non_negative_stream_value(
        &mut field_errors,
        "heartbeat_gap_seconds",
        health.heartbeat_gap_seconds,
    );

    if MarketStreamReasonCode::parse(&health.reason_code).is_err() {
        field_errors.push(MarketStreamValidationIssue {
            field: "reason_code",
            code: MarketStreamReasonCode::InvalidPayload.code(),
            message: "reason_code must be a known market stream reason".to_string(),
        });
    }

    if !field_errors.is_empty() {
        return Err(MarketStreamContractError::invalid_payload_with_issues(
            "market stream health payload is invalid",
            field_errors,
        ));
    }

    Ok(())
}

pub fn assess_market_stream_health(
    backlog_seconds: f64,
    sustained_backlog_seconds: f64,
    heartbeat_gap_seconds: f64,
    heartbeat_timeout_seconds: f64,
) -> Result<MarketStreamHealthAssessment, MarketStreamContractError> {
    let mut field_errors = Vec::new();
    validate_non_negative_stream_value(&mut field_errors, "backlog_seconds", backlog_seconds);
    validate_non_negative_stream_value(
        &mut field_errors,
        "sustained_backlog_seconds",
        sustained_backlog_seconds,
    );
    validate_non_negative_stream_value(
        &mut field_errors,
        "heartbeat_gap_seconds",
        heartbeat_gap_seconds,
    );
    validate_positive_stream_value(
        &mut field_errors,
        "heartbeat_timeout_seconds",
        heartbeat_timeout_seconds,
    );

    if !field_errors.is_empty() {
        return Err(MarketStreamContractError::invalid_payload_with_issues(
            "market stream health metrics are invalid",
            field_errors,
        ));
    }

    let (status, reason_code) = if heartbeat_gap_seconds > heartbeat_timeout_seconds {
        (
            MarketStreamHealthStatus::Degraded,
            MarketStreamReasonCode::HeartbeatTimeout.code(),
        )
    } else if backlog_seconds > MARKET_STREAM_BACKLOG_DEGRADED_SECONDS
        && sustained_backlog_seconds > MARKET_STREAM_BACKLOG_SUSTAINED_DURATION_SECONDS
    {
        (
            MarketStreamHealthStatus::Degraded,
            MarketStreamReasonCode::BacklogSustainedExceeded.code(),
        )
    } else if backlog_seconds > MARKET_STREAM_BACKLOG_DEGRADED_SECONDS {
        (
            MarketStreamHealthStatus::Degraded,
            MarketStreamReasonCode::BacklogExceeded.code(),
        )
    } else if backlog_seconds >= MARKET_STREAM_BACKLOG_WARNING_SECONDS {
        (
            MarketStreamHealthStatus::Healthy,
            MarketStreamReasonCode::BacklogWarning.code(),
        )
    } else {
        (
            MarketStreamHealthStatus::Healthy,
            MarketStreamReasonCode::Healthy.code(),
        )
    };

    Ok(MarketStreamHealthAssessment {
        status,
        reason_code: reason_code.to_string(),
    })
}

pub fn ingestion_latency_within_target(latency_seconds: f64) -> bool {
    latency_seconds.is_finite()
        && (0.0..=MARKET_STREAM_LATENCY_TARGET_SECONDS).contains(&latency_seconds)
}

pub fn ingestion_latency_slo_met(latencies_seconds: &[f64]) -> bool {
    if latencies_seconds.is_empty()
        || latencies_seconds
            .iter()
            .any(|latency| !latency.is_finite() || *latency < 0.0)
    {
        return false;
    }

    let mut ordered_latencies = latencies_seconds.to_vec();
    ordered_latencies.sort_by(f64::total_cmp);
    let nearest_rank = ((ordered_latencies.len() as f64) * 0.99).ceil() as usize;
    let percentile_index = nearest_rank
        .saturating_sub(1)
        .min(ordered_latencies.len() - 1);
    ordered_latencies[percentile_index] <= MARKET_STREAM_LATENCY_TARGET_SECONDS
}

pub fn market_stream_tick_to_snapshot(
    tick: &MarketStreamTick,
    reward_score: f64,
    projected_exposure_pct_nav: f64,
) -> Result<MarketSnapshot, MarketStreamContractError> {
    validate_market_stream_tick(tick)?;
    if !reward_score.is_finite() || reward_score < 0.0 {
        return Err(MarketStreamContractError::invalid_payload(
            "reward_score must be finite and non-negative",
        ));
    }
    if !projected_exposure_pct_nav.is_finite() || projected_exposure_pct_nav < 0.0 {
        return Err(MarketStreamContractError::invalid_payload(
            "projected_exposure_pct_nav must be finite and non-negative",
        ));
    }
    if tick.best_bid <= 0.0 {
        return Err(MarketStreamContractError::invalid_payload(
            "best_bid must be greater than 0 for spread conversion",
        ));
    }

    let liquidity_depth_usd = tick
        .top_bid_depth
        .iter()
        .chain(tick.top_ask_depth.iter())
        .map(|level| level.price * level.size)
        .sum::<f64>();
    let spread_bps = ((tick.best_ask - tick.best_bid) / tick.best_bid) * 10_000.0;

    Ok(MarketSnapshot {
        market_id: tick.market_id.clone(),
        cluster_id: tick.cluster_id.clone(),
        liquidity_depth_usd,
        spread_bps,
        reward_score,
        expected_reward_bps: Some(reward_score),
        maker_rebate_bps: Some(0.0),
        expected_cost_bps: Some(0.0),
        expected_volatility_bps: Some(1.0),
        venue_eligibility_state: None,
        projected_exposure_pct_nav,
        inactivity_gap_seconds: None,
        observed_at_utc: tick.observed_at_utc.clone(),
    })
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UserStreamEventKind {
    Order,
    Trade,
}

impl UserStreamEventKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Order => "order",
            Self::Trade => "trade",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UserStreamEventStatus {
    Placement,
    Update,
    Cancellation,
    Matched,
    Mined,
    Confirmed,
}

impl UserStreamEventStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Placement => "placement",
            Self::Update => "update",
            Self::Cancellation => "cancellation",
            Self::Matched => "matched",
            Self::Mined => "mined",
            Self::Confirmed => "confirmed",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UserStreamAuthState {
    Authenticated,
    AuthExpired,
}

impl UserStreamAuthState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Authenticated => "authenticated",
            Self::AuthExpired => "auth_expired",
        }
    }

    pub fn parse(value: &str) -> Result<Self, UserStreamContractError> {
        match value {
            "authenticated" => Ok(Self::Authenticated),
            "auth_expired" => Ok(Self::AuthExpired),
            _ => Err(UserStreamContractError::invalid_payload(format!(
                "unknown user stream auth state `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UserStreamReasonCode {
    EventAccepted,
    DuplicateEvent,
    OutOfOrderEvent,
    EqualOffsetTieBreakAccepted,
    EqualOffsetTieBreakRejected,
    AuthExpired,
    AuthRecoveredPendingEvent,
    Authenticated,
    PersistenceUnavailable,
    StreamDisconnected,
    InvalidPayload,
    LatencySloBreached,
}

impl UserStreamReasonCode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::EventAccepted => "user_stream_event_accepted",
            Self::DuplicateEvent => "user_stream_duplicate_event",
            Self::OutOfOrderEvent => "user_stream_out_of_order_event",
            Self::EqualOffsetTieBreakAccepted => "user_stream_equal_offset_tie_break_accepted",
            Self::EqualOffsetTieBreakRejected => "user_stream_equal_offset_tie_break_rejected",
            Self::AuthExpired => "user_stream_auth_expired",
            Self::AuthRecoveredPendingEvent => "user_stream_auth_recovered_pending_event",
            Self::Authenticated => "user_stream_authenticated",
            Self::PersistenceUnavailable => "user_stream_persistence_unavailable",
            Self::StreamDisconnected => "user_stream_disconnected",
            Self::InvalidPayload => "user_stream_invalid_payload",
            Self::LatencySloBreached => "user_stream_latency_slo_breached",
        }
    }

    pub fn parse(value: &str) -> Result<Self, UserStreamContractError> {
        match value {
            "user_stream_event_accepted" => Ok(Self::EventAccepted),
            "user_stream_duplicate_event" => Ok(Self::DuplicateEvent),
            "user_stream_out_of_order_event" => Ok(Self::OutOfOrderEvent),
            "user_stream_equal_offset_tie_break_accepted" => Ok(Self::EqualOffsetTieBreakAccepted),
            "user_stream_equal_offset_tie_break_rejected" => Ok(Self::EqualOffsetTieBreakRejected),
            "user_stream_auth_expired" => Ok(Self::AuthExpired),
            "user_stream_auth_recovered_pending_event" => Ok(Self::AuthRecoveredPendingEvent),
            "user_stream_authenticated" => Ok(Self::Authenticated),
            "user_stream_persistence_unavailable" => Ok(Self::PersistenceUnavailable),
            "user_stream_disconnected" => Ok(Self::StreamDisconnected),
            "user_stream_invalid_payload" => Ok(Self::InvalidPayload),
            "user_stream_latency_slo_breached" => Ok(Self::LatencySloBreached),
            _ => Err(UserStreamContractError::invalid_payload(format!(
                "unknown user stream reason code `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserStreamEvent {
    pub event_id: String,
    pub event_kind: UserStreamEventKind,
    pub event_status: UserStreamEventStatus,
    pub market_id: String,
    pub asset_id: String,
    pub order_id: String,
    pub trade_id: Option<String>,
    pub partition_key: String,
    pub idempotency_key: String,
    pub event_offset: i64,
    pub event_timestamp_utc: String,
    pub observed_at_utc: String,
    pub ingested_at_utc: String,
    pub ingestion_latency_seconds: f64,
    pub correlation_id: String,
    pub reason_code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrderEventOffsetCursor {
    pub partition_key: String,
    pub last_event_id: String,
    pub last_event_key: String,
    pub last_event_offset: i64,
    pub auth_state: UserStreamAuthState,
    pub block_new_intents: bool,
    pub reason_code: String,
    pub correlation_id: String,
    pub updated_at_utc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserStreamAuthTransition {
    pub transition_id: String,
    pub transition_offset: i64,
    pub auth_state: UserStreamAuthState,
    pub block_new_intents: bool,
    pub reason_code: String,
    pub correlation_id: String,
    pub observed_at_utc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserStreamValidationIssue {
    pub field: &'static str,
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserStreamContractError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<UserStreamValidationIssue>,
}

impl UserStreamContractError {
    pub fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: UserStreamReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn invalid_payload_with_issues(
        message: impl Into<String>,
        field_errors: Vec<UserStreamValidationIssue>,
    ) -> Self {
        Self {
            code: UserStreamReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserStreamOrderingDecision {
    Accept { reason_code: &'static str },
    Duplicate { reason_code: &'static str },
    OutOfOrder { reason_code: &'static str },
}

pub fn normalize_user_stream_idempotency_key(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}

pub fn user_stream_latency_slo_met(latencies_seconds: &[f64]) -> bool {
    ingestion_latency_slo_met(latencies_seconds)
}

pub fn validate_user_stream_event(event: &UserStreamEvent) -> Result<(), UserStreamContractError> {
    let mut field_errors = Vec::new();
    validate_non_empty_user_stream_field(&mut field_errors, "event_id", &event.event_id);
    validate_non_empty_user_stream_field(&mut field_errors, "market_id", &event.market_id);
    validate_non_empty_user_stream_field(&mut field_errors, "asset_id", &event.asset_id);
    validate_non_empty_user_stream_field(&mut field_errors, "order_id", &event.order_id);
    validate_non_empty_user_stream_field(&mut field_errors, "partition_key", &event.partition_key);
    validate_non_empty_user_stream_field(
        &mut field_errors,
        "idempotency_key",
        &event.idempotency_key,
    );
    validate_non_empty_user_stream_field(
        &mut field_errors,
        "correlation_id",
        &event.correlation_id,
    );
    validate_non_empty_user_stream_field(&mut field_errors, "reason_code", &event.reason_code);
    validate_user_stream_timestamp_field(
        &mut field_errors,
        "event_timestamp_utc",
        &event.event_timestamp_utc,
    );
    validate_user_stream_timestamp_field(
        &mut field_errors,
        "observed_at_utc",
        &event.observed_at_utc,
    );
    validate_user_stream_timestamp_field(
        &mut field_errors,
        "ingested_at_utc",
        &event.ingested_at_utc,
    );
    validate_non_negative_user_stream_offset(&mut field_errors, "event_offset", event.event_offset);
    validate_non_negative_user_stream_value(
        &mut field_errors,
        "ingestion_latency_seconds",
        event.ingestion_latency_seconds,
    );

    if event.event_kind == UserStreamEventKind::Trade
        && event
            .trade_id
            .as_ref()
            .map(|value| value.trim().is_empty())
            .unwrap_or(true)
    {
        field_errors.push(UserStreamValidationIssue {
            field: "trade_id",
            code: UserStreamReasonCode::InvalidPayload.code(),
            message: "trade events require non-empty trade_id".to_string(),
        });
    }
    if event.event_kind == UserStreamEventKind::Order
        && event
            .trade_id
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
    {
        field_errors.push(UserStreamValidationIssue {
            field: "trade_id",
            code: UserStreamReasonCode::InvalidPayload.code(),
            message: "order events cannot carry blank trade_id".to_string(),
        });
    }
    if UserStreamReasonCode::parse(&event.reason_code).is_err() {
        field_errors.push(UserStreamValidationIssue {
            field: "reason_code",
            code: UserStreamReasonCode::InvalidPayload.code(),
            message: "reason_code must be a known user stream reason".to_string(),
        });
    }
    if let (Ok(event_ts), Ok(observed_at), Ok(ingested_at)) = (
        parse_utc_timestamp(&event.event_timestamp_utc),
        parse_utc_timestamp(&event.observed_at_utc),
        parse_utc_timestamp(&event.ingested_at_utc),
    ) {
        if observed_at < event_ts {
            field_errors.push(UserStreamValidationIssue {
                field: "observed_at_utc",
                code: UserStreamReasonCode::InvalidPayload.code(),
                message: "observed_at_utc cannot be earlier than event_timestamp_utc".to_string(),
            });
        }
        if ingested_at < observed_at {
            field_errors.push(UserStreamValidationIssue {
                field: "ingested_at_utc",
                code: UserStreamReasonCode::InvalidPayload.code(),
                message: "ingested_at_utc cannot be earlier than observed_at_utc".to_string(),
            });
        }
    }

    if !field_errors.is_empty() {
        return Err(UserStreamContractError::invalid_payload_with_issues(
            "user stream event payload is invalid",
            field_errors,
        ));
    }
    Ok(())
}

pub fn validate_order_event_offset_cursor(
    cursor: &OrderEventOffsetCursor,
) -> Result<(), UserStreamContractError> {
    let mut field_errors = Vec::new();
    validate_non_empty_user_stream_field(&mut field_errors, "partition_key", &cursor.partition_key);
    validate_non_empty_user_stream_field(&mut field_errors, "last_event_id", &cursor.last_event_id);
    validate_non_empty_user_stream_field(
        &mut field_errors,
        "last_event_key",
        &cursor.last_event_key,
    );
    validate_non_empty_user_stream_field(&mut field_errors, "reason_code", &cursor.reason_code);
    validate_non_empty_user_stream_field(
        &mut field_errors,
        "correlation_id",
        &cursor.correlation_id,
    );
    validate_user_stream_timestamp_field(
        &mut field_errors,
        "updated_at_utc",
        &cursor.updated_at_utc,
    );
    validate_non_negative_user_stream_offset(
        &mut field_errors,
        "last_event_offset",
        cursor.last_event_offset,
    );
    if UserStreamReasonCode::parse(&cursor.reason_code).is_err() {
        field_errors.push(UserStreamValidationIssue {
            field: "reason_code",
            code: UserStreamReasonCode::InvalidPayload.code(),
            message: "reason_code must be a known user stream reason".to_string(),
        });
    }
    if cursor.auth_state == UserStreamAuthState::AuthExpired && !cursor.block_new_intents {
        field_errors.push(UserStreamValidationIssue {
            field: "block_new_intents",
            code: UserStreamReasonCode::InvalidPayload.code(),
            message: "auth_expired state must set block_new_intents to true".to_string(),
        });
    }

    if !field_errors.is_empty() {
        return Err(UserStreamContractError::invalid_payload_with_issues(
            "order event offset cursor is invalid",
            field_errors,
        ));
    }
    Ok(())
}

pub fn validate_user_stream_auth_transition(
    transition: &UserStreamAuthTransition,
) -> Result<(), UserStreamContractError> {
    let mut field_errors = Vec::new();
    validate_non_empty_user_stream_field(
        &mut field_errors,
        "transition_id",
        &transition.transition_id,
    );
    validate_non_empty_user_stream_field(&mut field_errors, "reason_code", &transition.reason_code);
    validate_non_empty_user_stream_field(
        &mut field_errors,
        "correlation_id",
        &transition.correlation_id,
    );
    validate_user_stream_timestamp_field(
        &mut field_errors,
        "observed_at_utc",
        &transition.observed_at_utc,
    );
    validate_non_negative_user_stream_offset(
        &mut field_errors,
        "transition_offset",
        transition.transition_offset,
    );
    if UserStreamReasonCode::parse(&transition.reason_code).is_err() {
        field_errors.push(UserStreamValidationIssue {
            field: "reason_code",
            code: UserStreamReasonCode::InvalidPayload.code(),
            message: "reason_code must be a known user stream reason".to_string(),
        });
    }
    if transition.auth_state == UserStreamAuthState::AuthExpired && !transition.block_new_intents {
        field_errors.push(UserStreamValidationIssue {
            field: "block_new_intents",
            code: UserStreamReasonCode::InvalidPayload.code(),
            message: "auth_expired transitions must enforce block_new_intents=true".to_string(),
        });
    }

    if !field_errors.is_empty() {
        return Err(UserStreamContractError::invalid_payload_with_issues(
            "user stream auth transition is invalid",
            field_errors,
        ));
    }
    Ok(())
}

pub fn evaluate_user_stream_ordering(
    cursor: Option<&OrderEventOffsetCursor>,
    event: &UserStreamEvent,
) -> Result<UserStreamOrderingDecision, UserStreamContractError> {
    validate_user_stream_event(event)?;
    if let Some(cursor) = cursor {
        validate_order_event_offset_cursor(cursor)?;
        if event.event_offset < cursor.last_event_offset {
            return Ok(UserStreamOrderingDecision::OutOfOrder {
                reason_code: UserStreamReasonCode::OutOfOrderEvent.code(),
            });
        }
        if event.event_offset > cursor.last_event_offset {
            return Ok(UserStreamOrderingDecision::Accept {
                reason_code: UserStreamReasonCode::EventAccepted.code(),
            });
        }

        let normalized_event_key = normalize_user_stream_idempotency_key(&event.idempotency_key);
        let normalized_cursor_key = normalize_user_stream_idempotency_key(&cursor.last_event_key);
        if normalized_event_key == normalized_cursor_key {
            return Ok(UserStreamOrderingDecision::Duplicate {
                reason_code: UserStreamReasonCode::DuplicateEvent.code(),
            });
        }
        if normalized_event_key > normalized_cursor_key {
            return Ok(UserStreamOrderingDecision::Accept {
                reason_code: UserStreamReasonCode::EqualOffsetTieBreakAccepted.code(),
            });
        }
        return Ok(UserStreamOrderingDecision::OutOfOrder {
            reason_code: UserStreamReasonCode::EqualOffsetTieBreakRejected.code(),
        });
    }

    Ok(UserStreamOrderingDecision::Accept {
        reason_code: UserStreamReasonCode::EventAccepted.code(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FreshnessGatePolicy {
    pub stale_threshold_seconds: f64,
    pub stability_window_seconds: f64,
    pub max_breach_to_pause_seconds: f64,
}

impl Default for FreshnessGatePolicy {
    fn default() -> Self {
        Self {
            stale_threshold_seconds: FRESHNESS_STALE_THRESHOLD_SECONDS,
            stability_window_seconds: FRESHNESS_RECOVERY_STABILITY_WINDOW_SECONDS,
            max_breach_to_pause_seconds: FRESHNESS_MAX_BREACH_TO_PAUSE_SECONDS,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FreshnessGateTransition {
    PauseActivated,
    PauseMaintained,
    RecoveryPending,
    RecoveryConfirmed,
}

impl FreshnessGateTransition {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PauseActivated => "pause_activated",
            Self::PauseMaintained => "pause_maintained",
            Self::RecoveryPending => "recovery_pending",
            Self::RecoveryConfirmed => "recovery_confirmed",
        }
    }

    pub fn parse(value: &str) -> Result<Self, FreshnessGateContractError> {
        match value {
            "pause_activated" => Ok(Self::PauseActivated),
            "pause_maintained" => Ok(Self::PauseMaintained),
            "recovery_pending" => Ok(Self::RecoveryPending),
            "recovery_confirmed" => Ok(Self::RecoveryConfirmed),
            _ => Err(FreshnessGateContractError::invalid_payload(format!(
                "unknown freshness gate transition `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FreshnessGateReasonCode {
    StaleBreach,
    BoundarySafe,
    StateUnavailable,
    RecoveryPending,
    RecoveryConfirmed,
    EvaluationError,
    PersistenceUnavailable,
    InvalidPayload,
}

impl FreshnessGateReasonCode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::StaleBreach => "freshness_gate_stale_breach",
            Self::BoundarySafe => "freshness_gate_boundary_safe",
            Self::StateUnavailable => "freshness_gate_state_unavailable",
            Self::RecoveryPending => "freshness_gate_recovery_pending",
            Self::RecoveryConfirmed => "freshness_gate_recovery_confirmed",
            Self::EvaluationError => "freshness_gate_evaluation_error",
            Self::PersistenceUnavailable => "freshness_gate_persistence_unavailable",
            Self::InvalidPayload => "freshness_gate_invalid_payload",
        }
    }

    pub fn parse(value: &str) -> Result<Self, FreshnessGateContractError> {
        match value {
            "freshness_gate_stale_breach" => Ok(Self::StaleBreach),
            "freshness_gate_boundary_safe" => Ok(Self::BoundarySafe),
            "freshness_gate_state_unavailable" => Ok(Self::StateUnavailable),
            "freshness_gate_recovery_pending" => Ok(Self::RecoveryPending),
            "freshness_gate_recovery_confirmed" => Ok(Self::RecoveryConfirmed),
            "freshness_gate_evaluation_error" => Ok(Self::EvaluationError),
            "freshness_gate_persistence_unavailable" => Ok(Self::PersistenceUnavailable),
            "freshness_gate_invalid_payload" => Ok(Self::InvalidPayload),
            _ => Err(FreshnessGateContractError::invalid_payload(format!(
                "unknown freshness gate reason code `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FreshnessSnapshot {
    pub market_data_age_seconds: Option<f64>,
    pub user_data_age_seconds: Option<f64>,
    pub sampled_at_utc: String,
    pub correlation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FreshnessGateState {
    pub pause_active: bool,
    pub reason_code: String,
    pub stale_breach_detected_at_utc: Option<String>,
    pub pause_activated_at_utc: Option<String>,
    pub recovery_window_started_at_utc: Option<String>,
    pub last_evaluated_at_utc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FreshnessGateEvaluationOutcome {
    pub transition: Option<FreshnessGateTransition>,
    pub reason_code: String,
    pub pause_active: bool,
    pub block_new_order_creation: bool,
    pub market_data_age_seconds: Option<f64>,
    pub user_data_age_seconds: Option<f64>,
    pub stale_breach_detected_at_utc: Option<String>,
    pub pause_activated_at_utc: Option<String>,
    pub recovery_window_started_at_utc: Option<String>,
    pub recovery_confirmed_at_utc: Option<String>,
    pub breach_to_pause_latency_seconds: Option<f64>,
    pub evaluated_at_utc: String,
    pub correlation_id: String,
}

impl FreshnessGateEvaluationOutcome {
    pub fn to_state(&self) -> FreshnessGateState {
        FreshnessGateState {
            pause_active: self.pause_active,
            reason_code: self.reason_code.clone(),
            stale_breach_detected_at_utc: self.stale_breach_detected_at_utc.clone(),
            pause_activated_at_utc: self.pause_activated_at_utc.clone(),
            recovery_window_started_at_utc: self.recovery_window_started_at_utc.clone(),
            last_evaluated_at_utc: self.evaluated_at_utc.clone(),
        }
    }

    pub fn to_event(
        &self,
        event_id: impl Into<String>,
        policy: &FreshnessGatePolicy,
    ) -> Option<FreshnessGateEvent> {
        self.transition.map(|transition| FreshnessGateEvent {
            event_id: event_id.into(),
            transition,
            reason_code: self.reason_code.clone(),
            pause_active: self.pause_active,
            block_new_order_creation: self.block_new_order_creation,
            market_data_age_seconds: self.market_data_age_seconds,
            user_data_age_seconds: self.user_data_age_seconds,
            stale_threshold_seconds: policy.stale_threshold_seconds,
            stability_window_seconds: policy.stability_window_seconds,
            max_breach_to_pause_seconds: policy.max_breach_to_pause_seconds,
            stale_breach_detected_at_utc: self.stale_breach_detected_at_utc.clone(),
            pause_activated_at_utc: self.pause_activated_at_utc.clone(),
            recovery_window_started_at_utc: self.recovery_window_started_at_utc.clone(),
            recovery_confirmed_at_utc: self.recovery_confirmed_at_utc.clone(),
            breach_to_pause_latency_seconds: self.breach_to_pause_latency_seconds,
            evaluated_at_utc: self.evaluated_at_utc.clone(),
            correlation_id: self.correlation_id.clone(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FreshnessGateEvent {
    pub event_id: String,
    pub transition: FreshnessGateTransition,
    pub reason_code: String,
    pub pause_active: bool,
    pub block_new_order_creation: bool,
    pub market_data_age_seconds: Option<f64>,
    pub user_data_age_seconds: Option<f64>,
    pub stale_threshold_seconds: f64,
    pub stability_window_seconds: f64,
    pub max_breach_to_pause_seconds: f64,
    pub stale_breach_detected_at_utc: Option<String>,
    pub pause_activated_at_utc: Option<String>,
    pub recovery_window_started_at_utc: Option<String>,
    pub recovery_confirmed_at_utc: Option<String>,
    pub breach_to_pause_latency_seconds: Option<f64>,
    pub evaluated_at_utc: String,
    pub correlation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FreshnessGateValidationIssue {
    pub field: &'static str,
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FreshnessGateContractError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<FreshnessGateValidationIssue>,
}

impl FreshnessGateContractError {
    pub fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: FreshnessGateReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn invalid_payload_with_issues(
        message: impl Into<String>,
        field_errors: Vec<FreshnessGateValidationIssue>,
    ) -> Self {
        Self {
            code: FreshnessGateReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }
}

pub fn validate_freshness_gate_policy(
    policy: &FreshnessGatePolicy,
) -> Result<(), FreshnessGateContractError> {
    let mut field_errors = Vec::new();
    validate_non_negative_freshness_age(
        &mut field_errors,
        "stale_threshold_seconds",
        Some(policy.stale_threshold_seconds),
        true,
    );
    validate_non_negative_freshness_age(
        &mut field_errors,
        "stability_window_seconds",
        Some(policy.stability_window_seconds),
        true,
    );
    validate_non_negative_freshness_age(
        &mut field_errors,
        "max_breach_to_pause_seconds",
        Some(policy.max_breach_to_pause_seconds),
        true,
    );

    if policy.max_breach_to_pause_seconds > policy.stale_threshold_seconds {
        field_errors.push(FreshnessGateValidationIssue {
            field: "max_breach_to_pause_seconds",
            code: FreshnessGateReasonCode::InvalidPayload.code(),
            message: "max_breach_to_pause_seconds cannot exceed stale_threshold_seconds"
                .to_string(),
        });
    }

    if !field_errors.is_empty() {
        return Err(FreshnessGateContractError::invalid_payload_with_issues(
            "freshness gate policy is invalid",
            field_errors,
        ));
    }
    Ok(())
}

pub fn validate_freshness_snapshot(
    snapshot: &FreshnessSnapshot,
) -> Result<(), FreshnessGateContractError> {
    let mut field_errors = Vec::new();
    validate_non_negative_freshness_age(
        &mut field_errors,
        "market_data_age_seconds",
        snapshot.market_data_age_seconds,
        false,
    );
    validate_non_negative_freshness_age(
        &mut field_errors,
        "user_data_age_seconds",
        snapshot.user_data_age_seconds,
        false,
    );
    validate_freshness_timestamp_field(
        &mut field_errors,
        "sampled_at_utc",
        &snapshot.sampled_at_utc,
    );
    validate_freshness_non_empty(
        &mut field_errors,
        "correlation_id",
        &snapshot.correlation_id,
    );

    if !field_errors.is_empty() {
        return Err(FreshnessGateContractError::invalid_payload_with_issues(
            "freshness snapshot is invalid",
            field_errors,
        ));
    }
    Ok(())
}

pub fn validate_freshness_gate_state(
    state: &FreshnessGateState,
) -> Result<(), FreshnessGateContractError> {
    let mut field_errors = Vec::new();
    validate_freshness_non_empty(&mut field_errors, "reason_code", &state.reason_code);
    if FreshnessGateReasonCode::parse(&state.reason_code).is_err() {
        field_errors.push(FreshnessGateValidationIssue {
            field: "reason_code",
            code: FreshnessGateReasonCode::InvalidPayload.code(),
            message: "reason_code must be a known freshness gate reason".to_string(),
        });
    }
    validate_freshness_timestamp_field(
        &mut field_errors,
        "last_evaluated_at_utc",
        &state.last_evaluated_at_utc,
    );
    if let Some(value) = state.stale_breach_detected_at_utc.as_ref() {
        validate_freshness_timestamp_field(
            &mut field_errors,
            "stale_breach_detected_at_utc",
            value,
        );
    }
    if let Some(value) = state.pause_activated_at_utc.as_ref() {
        validate_freshness_timestamp_field(&mut field_errors, "pause_activated_at_utc", value);
    }
    if let Some(value) = state.recovery_window_started_at_utc.as_ref() {
        validate_freshness_timestamp_field(
            &mut field_errors,
            "recovery_window_started_at_utc",
            value,
        );
    }

    if state.pause_active && state.pause_activated_at_utc.is_none() {
        field_errors.push(FreshnessGateValidationIssue {
            field: "pause_activated_at_utc",
            code: FreshnessGateReasonCode::InvalidPayload.code(),
            message: "pause_active state requires pause_activated_at_utc".to_string(),
        });
    }

    if !field_errors.is_empty() {
        return Err(FreshnessGateContractError::invalid_payload_with_issues(
            "freshness gate state is invalid",
            field_errors,
        ));
    }
    Ok(())
}

pub fn validate_freshness_gate_event(
    event: &FreshnessGateEvent,
) -> Result<(), FreshnessGateContractError> {
    let mut field_errors = Vec::new();
    validate_freshness_non_empty(&mut field_errors, "event_id", &event.event_id);
    validate_freshness_non_empty(&mut field_errors, "reason_code", &event.reason_code);
    validate_freshness_non_empty(&mut field_errors, "correlation_id", &event.correlation_id);
    validate_non_negative_freshness_age(
        &mut field_errors,
        "market_data_age_seconds",
        event.market_data_age_seconds,
        false,
    );
    validate_non_negative_freshness_age(
        &mut field_errors,
        "user_data_age_seconds",
        event.user_data_age_seconds,
        false,
    );
    validate_non_negative_freshness_age(
        &mut field_errors,
        "stale_threshold_seconds",
        Some(event.stale_threshold_seconds),
        true,
    );
    validate_non_negative_freshness_age(
        &mut field_errors,
        "stability_window_seconds",
        Some(event.stability_window_seconds),
        true,
    );
    validate_non_negative_freshness_age(
        &mut field_errors,
        "max_breach_to_pause_seconds",
        Some(event.max_breach_to_pause_seconds),
        true,
    );
    validate_freshness_timestamp_field(
        &mut field_errors,
        "evaluated_at_utc",
        &event.evaluated_at_utc,
    );
    if let Some(value) = event.stale_breach_detected_at_utc.as_ref() {
        validate_freshness_timestamp_field(
            &mut field_errors,
            "stale_breach_detected_at_utc",
            value,
        );
    }
    if let Some(value) = event.pause_activated_at_utc.as_ref() {
        validate_freshness_timestamp_field(&mut field_errors, "pause_activated_at_utc", value);
    }
    if let Some(value) = event.recovery_window_started_at_utc.as_ref() {
        validate_freshness_timestamp_field(
            &mut field_errors,
            "recovery_window_started_at_utc",
            value,
        );
    }
    if let Some(value) = event.recovery_confirmed_at_utc.as_ref() {
        validate_freshness_timestamp_field(&mut field_errors, "recovery_confirmed_at_utc", value);
    }
    if let Some(latency) = event.breach_to_pause_latency_seconds {
        if !latency.is_finite() || latency < 0.0 {
            field_errors.push(FreshnessGateValidationIssue {
                field: "breach_to_pause_latency_seconds",
                code: FreshnessGateReasonCode::InvalidPayload.code(),
                message: "breach_to_pause_latency_seconds must be finite and non-negative"
                    .to_string(),
            });
        } else if latency > event.max_breach_to_pause_seconds {
            field_errors.push(FreshnessGateValidationIssue {
                field: "breach_to_pause_latency_seconds",
                code: FreshnessGateReasonCode::InvalidPayload.code(),
                message: "breach_to_pause_latency_seconds exceeds configured maximum".to_string(),
            });
        }
    }
    if FreshnessGateReasonCode::parse(&event.reason_code).is_err() {
        field_errors.push(FreshnessGateValidationIssue {
            field: "reason_code",
            code: FreshnessGateReasonCode::InvalidPayload.code(),
            message: "reason_code must be a known freshness gate reason".to_string(),
        });
    }

    match event.transition {
        FreshnessGateTransition::PauseActivated | FreshnessGateTransition::PauseMaintained => {
            if !event.pause_active || !event.block_new_order_creation {
                field_errors.push(FreshnessGateValidationIssue {
                    field: "pause_active",
                    code: FreshnessGateReasonCode::InvalidPayload.code(),
                    message:
                        "pause transitions must keep pause_active and block_new_order_creation true"
                            .to_string(),
                });
            }
            if event.pause_activated_at_utc.is_none() {
                field_errors.push(FreshnessGateValidationIssue {
                    field: "pause_activated_at_utc",
                    code: FreshnessGateReasonCode::InvalidPayload.code(),
                    message: "pause transitions require pause_activated_at_utc".to_string(),
                });
            }
            if event.transition == FreshnessGateTransition::PauseActivated
                && event.stale_breach_detected_at_utc.is_none()
            {
                field_errors.push(FreshnessGateValidationIssue {
                    field: "stale_breach_detected_at_utc",
                    code: FreshnessGateReasonCode::InvalidPayload.code(),
                    message: "pause_activated transition requires stale_breach_detected_at_utc"
                        .to_string(),
                });
            }
        }
        FreshnessGateTransition::RecoveryPending => {
            if !event.pause_active || !event.block_new_order_creation {
                field_errors.push(FreshnessGateValidationIssue {
                    field: "pause_active",
                    code: FreshnessGateReasonCode::InvalidPayload.code(),
                    message:
                        "recovery_pending transition must remain fail-closed while stability window runs"
                            .to_string(),
                });
            }
            if event.recovery_window_started_at_utc.is_none() {
                field_errors.push(FreshnessGateValidationIssue {
                    field: "recovery_window_started_at_utc",
                    code: FreshnessGateReasonCode::InvalidPayload.code(),
                    message: "recovery_pending transition requires recovery_window_started_at_utc"
                        .to_string(),
                });
            }
        }
        FreshnessGateTransition::RecoveryConfirmed => {
            if event.pause_active || event.block_new_order_creation {
                field_errors.push(FreshnessGateValidationIssue {
                    field: "pause_active",
                    code: FreshnessGateReasonCode::InvalidPayload.code(),
                    message:
                        "recovery_confirmed transition must clear pause_active and un-block orders"
                            .to_string(),
                });
            }
            if event.recovery_confirmed_at_utc.is_none() {
                field_errors.push(FreshnessGateValidationIssue {
                    field: "recovery_confirmed_at_utc",
                    code: FreshnessGateReasonCode::InvalidPayload.code(),
                    message: "recovery_confirmed transition requires recovery_confirmed_at_utc"
                        .to_string(),
                });
            }
        }
    }

    if !field_errors.is_empty() {
        return Err(FreshnessGateContractError::invalid_payload_with_issues(
            "freshness gate event is invalid",
            field_errors,
        ));
    }
    Ok(())
}

pub fn evaluate_freshness_gate(
    snapshot: &FreshnessSnapshot,
    previous_state: Option<&FreshnessGateState>,
    policy: &FreshnessGatePolicy,
) -> Result<FreshnessGateEvaluationOutcome, FreshnessGateContractError> {
    validate_freshness_snapshot(snapshot)?;
    validate_freshness_gate_policy(policy)?;
    if let Some(state) = previous_state {
        validate_freshness_gate_state(state)?;
    }

    let missing_inputs =
        snapshot.market_data_age_seconds.is_none() || snapshot.user_data_age_seconds.is_none();
    let market_stale = snapshot
        .market_data_age_seconds
        .is_some_and(|age| age > policy.stale_threshold_seconds);
    let user_stale = snapshot
        .user_data_age_seconds
        .is_some_and(|age| age > policy.stale_threshold_seconds);
    let stale_breach = market_stale || user_stale;

    if missing_inputs || stale_breach {
        let was_paused = previous_state.is_some_and(|state| state.pause_active);
        let stale_breach_detected_at_utc = if was_paused {
            previous_state
                .and_then(|state| state.stale_breach_detected_at_utc.clone())
                .unwrap_or_else(|| snapshot.sampled_at_utc.clone())
        } else {
            snapshot.sampled_at_utc.clone()
        };
        let pause_activated_at_utc = if was_paused {
            previous_state
                .and_then(|state| state.pause_activated_at_utc.clone())
                .unwrap_or_else(|| snapshot.sampled_at_utc.clone())
        } else {
            snapshot.sampled_at_utc.clone()
        };
        let transition = if was_paused {
            FreshnessGateTransition::PauseMaintained
        } else {
            FreshnessGateTransition::PauseActivated
        };
        let breach_to_pause_latency_seconds =
            if transition == FreshnessGateTransition::PauseActivated {
                Some(compute_elapsed_seconds(
                    &stale_breach_detected_at_utc,
                    &pause_activated_at_utc,
                )?)
            } else {
                None
            };
        let reason_code = if missing_inputs {
            FreshnessGateReasonCode::StateUnavailable.code()
        } else {
            FreshnessGateReasonCode::StaleBreach.code()
        };

        return Ok(FreshnessGateEvaluationOutcome {
            transition: Some(transition),
            reason_code: reason_code.to_string(),
            pause_active: true,
            block_new_order_creation: true,
            market_data_age_seconds: snapshot.market_data_age_seconds,
            user_data_age_seconds: snapshot.user_data_age_seconds,
            stale_breach_detected_at_utc: Some(stale_breach_detected_at_utc),
            pause_activated_at_utc: Some(pause_activated_at_utc),
            recovery_window_started_at_utc: None,
            recovery_confirmed_at_utc: None,
            breach_to_pause_latency_seconds,
            evaluated_at_utc: snapshot.sampled_at_utc.clone(),
            correlation_id: snapshot.correlation_id.clone(),
        });
    }

    if previous_state.is_some_and(|state| state.pause_active) {
        let recovery_window_started_at_utc = previous_state
            .and_then(|state| state.recovery_window_started_at_utc.clone())
            .unwrap_or_else(|| snapshot.sampled_at_utc.clone());
        let elapsed_recovery_seconds =
            compute_elapsed_seconds(&recovery_window_started_at_utc, &snapshot.sampled_at_utc)?;

        if elapsed_recovery_seconds >= policy.stability_window_seconds {
            return Ok(FreshnessGateEvaluationOutcome {
                transition: Some(FreshnessGateTransition::RecoveryConfirmed),
                reason_code: FreshnessGateReasonCode::RecoveryConfirmed
                    .code()
                    .to_string(),
                pause_active: false,
                block_new_order_creation: false,
                market_data_age_seconds: snapshot.market_data_age_seconds,
                user_data_age_seconds: snapshot.user_data_age_seconds,
                stale_breach_detected_at_utc: previous_state
                    .and_then(|state| state.stale_breach_detected_at_utc.clone()),
                pause_activated_at_utc: previous_state
                    .and_then(|state| state.pause_activated_at_utc.clone()),
                recovery_window_started_at_utc: Some(recovery_window_started_at_utc),
                recovery_confirmed_at_utc: Some(snapshot.sampled_at_utc.clone()),
                breach_to_pause_latency_seconds: None,
                evaluated_at_utc: snapshot.sampled_at_utc.clone(),
                correlation_id: snapshot.correlation_id.clone(),
            });
        }

        return Ok(FreshnessGateEvaluationOutcome {
            transition: Some(FreshnessGateTransition::RecoveryPending),
            reason_code: FreshnessGateReasonCode::RecoveryPending.code().to_string(),
            pause_active: true,
            block_new_order_creation: true,
            market_data_age_seconds: snapshot.market_data_age_seconds,
            user_data_age_seconds: snapshot.user_data_age_seconds,
            stale_breach_detected_at_utc: previous_state
                .and_then(|state| state.stale_breach_detected_at_utc.clone()),
            pause_activated_at_utc: previous_state
                .and_then(|state| state.pause_activated_at_utc.clone()),
            recovery_window_started_at_utc: Some(recovery_window_started_at_utc),
            recovery_confirmed_at_utc: None,
            breach_to_pause_latency_seconds: None,
            evaluated_at_utc: snapshot.sampled_at_utc.clone(),
            correlation_id: snapshot.correlation_id.clone(),
        });
    }

    Ok(FreshnessGateEvaluationOutcome {
        transition: None,
        reason_code: FreshnessGateReasonCode::BoundarySafe.code().to_string(),
        pause_active: false,
        block_new_order_creation: false,
        market_data_age_seconds: snapshot.market_data_age_seconds,
        user_data_age_seconds: snapshot.user_data_age_seconds,
        stale_breach_detected_at_utc: None,
        pause_activated_at_utc: None,
        recovery_window_started_at_utc: None,
        recovery_confirmed_at_utc: None,
        breach_to_pause_latency_seconds: None,
        evaluated_at_utc: snapshot.sampled_at_utc.clone(),
        correlation_id: snapshot.correlation_id.clone(),
    })
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ParticipationGuardrailMode {
    Pass,
    Pause,
    SizeCap,
    Unavailable,
}

impl ParticipationGuardrailMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Pause => "pause",
            Self::SizeCap => "size_cap",
            Self::Unavailable => "unavailable",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ParticipationGuardrailContractError> {
        match value {
            "pass" => Ok(Self::Pass),
            "pause" => Ok(Self::Pause),
            "size_cap" => Ok(Self::SizeCap),
            "unavailable" => Ok(Self::Unavailable),
            _ => Err(ParticipationGuardrailContractError::invalid_payload(
                format!("unknown FR41 guardrail mode `{value}`"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ParticipationGuardrailReasonCode {
    NoTrigger,
    LowLiquidityPause,
    InactivityPause,
    OvernightSizeCapActive,
    DependencyUnavailable,
    InvalidPayload,
    PersistenceUnavailable,
    EvidencePersisted,
    EvidenceRead,
}

impl ParticipationGuardrailReasonCode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::NoTrigger => "fr41_participation_no_trigger",
            Self::LowLiquidityPause => "fr41_participation_low_liquidity_pause",
            Self::InactivityPause => "fr41_participation_inactivity_pause",
            Self::OvernightSizeCapActive => "fr41_participation_overnight_size_cap_active",
            Self::DependencyUnavailable => "fr41_participation_dependency_unavailable",
            Self::InvalidPayload => "fr41_participation_invalid_payload",
            Self::PersistenceUnavailable => "fr41_participation_persistence_unavailable",
            Self::EvidencePersisted => "fr41_participation_evidence_persisted",
            Self::EvidenceRead => "fr41_participation_evidence_read",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ParticipationGuardrailContractError> {
        match value {
            "fr41_participation_no_trigger" => Ok(Self::NoTrigger),
            "fr41_participation_low_liquidity_pause" => Ok(Self::LowLiquidityPause),
            "fr41_participation_inactivity_pause" => Ok(Self::InactivityPause),
            "fr41_participation_overnight_size_cap_active" => Ok(Self::OvernightSizeCapActive),
            "fr41_participation_dependency_unavailable" => Ok(Self::DependencyUnavailable),
            "fr41_participation_invalid_payload" => Ok(Self::InvalidPayload),
            "fr41_participation_persistence_unavailable" => Ok(Self::PersistenceUnavailable),
            "fr41_participation_evidence_persisted" => Ok(Self::EvidencePersisted),
            "fr41_participation_evidence_read" => Ok(Self::EvidenceRead),
            _ => Err(ParticipationGuardrailContractError::invalid_payload(
                format!("unknown FR41 reason code `{value}`"),
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParticipationGuardrailValidationIssue {
    pub field: &'static str,
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParticipationGuardrailContractError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<ParticipationGuardrailValidationIssue>,
}

impl ParticipationGuardrailContractError {
    pub fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: ParticipationGuardrailReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn invalid_payload_with_issues(
        message: impl Into<String>,
        field_errors: Vec<ParticipationGuardrailValidationIssue>,
    ) -> Self {
        Self {
            code: ParticipationGuardrailReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    pub fn dependency_unavailable_with_issues(
        message: impl Into<String>,
        field_errors: Vec<ParticipationGuardrailValidationIssue>,
    ) -> Self {
        Self {
            code: ParticipationGuardrailReasonCode::DependencyUnavailable.code(),
            message: message.into(),
            field_errors,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParticipationGuardrailEvaluationInput {
    pub market_id: String,
    pub cluster_id: String,
    pub correlation_id: String,
    pub observed_at_utc: String,
    pub liquidity_depth_usd: Option<f64>,
    pub inactivity_gap_seconds: Option<f64>,
    pub normal_max_order_size_units: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParticipationGuardrailEvaluationOutcome {
    pub market_id: String,
    pub cluster_id: String,
    pub correlation_id: String,
    pub observed_at_utc: String,
    pub guardrail_mode: ParticipationGuardrailMode,
    pub reason_code: String,
    pub liquidity_depth_usd: f64,
    pub inactivity_gap_seconds: f64,
    pub threshold_liquidity_depth_usd: f64,
    pub threshold_inactivity_pause_seconds: f64,
    pub threshold_overnight_gap_seconds: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normal_max_order_size_units: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capped_max_order_size_units: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PreTradeParticipationGuardrailEvidence {
    pub event_id: String,
    pub guardrail_mode: String,
    pub reason_code: String,
    pub market_id: String,
    pub cluster_id: String,
    pub correlation_id: String,
    pub observed_at_utc: String,
    pub evaluated_at_utc: String,
    pub liquidity_depth_usd: f64,
    pub inactivity_gap_seconds: f64,
    pub threshold_liquidity_depth_usd: f64,
    pub threshold_inactivity_pause_seconds: f64,
    pub threshold_overnight_gap_seconds: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normal_max_order_size_units: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capped_max_order_size_units: Option<f64>,
}

pub fn evaluate_fr41_participation_guardrail(
    input: &ParticipationGuardrailEvaluationInput,
) -> Result<ParticipationGuardrailEvaluationOutcome, ParticipationGuardrailContractError> {
    let mut field_errors = Vec::new();
    validate_participation_guardrail_identifier_field(
        &mut field_errors,
        "market_id",
        &input.market_id,
    );
    validate_participation_guardrail_identifier_field(
        &mut field_errors,
        "cluster_id",
        &input.cluster_id,
    );
    validate_participation_guardrail_identifier_field(
        &mut field_errors,
        "correlation_id",
        &input.correlation_id,
    );
    validate_participation_guardrail_timestamp_field(
        &mut field_errors,
        "observed_at_utc",
        &input.observed_at_utc,
    );
    if !field_errors.is_empty() {
        return Err(
            ParticipationGuardrailContractError::invalid_payload_with_issues(
                "FR41 participation guardrail payload is invalid",
                field_errors,
            ),
        );
    }

    let mut dependency_issues = Vec::new();
    let liquidity_depth_usd = require_participation_guardrail_signal(
        &mut dependency_issues,
        "liquidity_depth_usd",
        input.liquidity_depth_usd,
        false,
    );
    let inactivity_gap_seconds = require_participation_guardrail_signal(
        &mut dependency_issues,
        "inactivity_gap_seconds",
        input.inactivity_gap_seconds,
        false,
    );
    if !dependency_issues.is_empty() {
        return Err(
            ParticipationGuardrailContractError::dependency_unavailable_with_issues(
                "required FR41 dependencies are unavailable",
                dependency_issues,
            ),
        );
    }
    let liquidity_depth_usd = liquidity_depth_usd.expect("dependency checks guarantee liquidity");
    let inactivity_gap_seconds =
        inactivity_gap_seconds.expect("dependency checks guarantee inactivity");

    let mut normal_max_order_size_units = None;
    let mut capped_max_order_size_units = None;
    let (guardrail_mode, reason_code) =
        if liquidity_depth_usd < FR41_LOW_LIQUIDITY_DEPTH_USD_THRESHOLD {
            (
                ParticipationGuardrailMode::Pause,
                ParticipationGuardrailReasonCode::LowLiquidityPause,
            )
        } else if inactivity_gap_seconds > FR41_INACTIVITY_PAUSE_THRESHOLD_SECONDS
            && inactivity_gap_seconds <= FR41_OVERNIGHT_CAP_THRESHOLD_SECONDS
        {
            (
                ParticipationGuardrailMode::Pause,
                ParticipationGuardrailReasonCode::InactivityPause,
            )
        } else if inactivity_gap_seconds > FR41_OVERNIGHT_CAP_THRESHOLD_SECONDS {
            let normal_baseline = require_participation_guardrail_signal(
                &mut dependency_issues,
                "normal_max_order_size_units",
                input.normal_max_order_size_units,
                true,
            );
            if !dependency_issues.is_empty() {
                return Err(
                    ParticipationGuardrailContractError::dependency_unavailable_with_issues(
                        "required FR41 size-cap baseline is unavailable",
                        dependency_issues,
                    ),
                );
            }
            let normal_baseline = normal_baseline
                .expect("dependency checks guarantee size-cap baseline availability");
            let capped = normal_baseline * FR41_OVERNIGHT_CAP_FACTOR;
            if !capped.is_finite() || capped <= 0.0 {
                return Err(
                    ParticipationGuardrailContractError::dependency_unavailable_with_issues(
                        "required FR41 size-cap baseline is unavailable",
                        vec![ParticipationGuardrailValidationIssue {
                        field: "normal_max_order_size_units",
                        code: ParticipationGuardrailReasonCode::DependencyUnavailable.code(),
                        message:
                            "normal_max_order_size_units must produce a finite capped order size"
                                .to_string(),
                    }],
                    ),
                );
            }
            normal_max_order_size_units = Some(normal_baseline);
            capped_max_order_size_units = Some(capped);
            (
                ParticipationGuardrailMode::SizeCap,
                ParticipationGuardrailReasonCode::OvernightSizeCapActive,
            )
        } else {
            (
                ParticipationGuardrailMode::Pass,
                ParticipationGuardrailReasonCode::NoTrigger,
            )
        };

    Ok(ParticipationGuardrailEvaluationOutcome {
        market_id: input.market_id.trim().to_ascii_lowercase(),
        cluster_id: input.cluster_id.trim().to_ascii_lowercase(),
        correlation_id: input.correlation_id.trim().to_ascii_lowercase(),
        observed_at_utc: input.observed_at_utc.clone(),
        guardrail_mode,
        reason_code: reason_code.code().to_string(),
        liquidity_depth_usd,
        inactivity_gap_seconds,
        threshold_liquidity_depth_usd: FR41_LOW_LIQUIDITY_DEPTH_USD_THRESHOLD,
        threshold_inactivity_pause_seconds: FR41_INACTIVITY_PAUSE_THRESHOLD_SECONDS,
        threshold_overnight_gap_seconds: FR41_OVERNIGHT_CAP_THRESHOLD_SECONDS,
        normal_max_order_size_units,
        capped_max_order_size_units,
    })
}

pub fn validate_pretrade_participation_guardrail_evidence(
    evidence: &PreTradeParticipationGuardrailEvidence,
) -> Result<(), ParticipationGuardrailContractError> {
    let mut field_errors = Vec::new();
    validate_participation_guardrail_identifier_field(
        &mut field_errors,
        "event_id",
        &evidence.event_id,
    );
    validate_participation_guardrail_identifier_field(
        &mut field_errors,
        "market_id",
        &evidence.market_id,
    );
    validate_participation_guardrail_identifier_field(
        &mut field_errors,
        "cluster_id",
        &evidence.cluster_id,
    );
    validate_participation_guardrail_identifier_field(
        &mut field_errors,
        "correlation_id",
        &evidence.correlation_id,
    );
    validate_participation_guardrail_non_empty_field(
        &mut field_errors,
        "reason_code",
        &evidence.reason_code,
    );
    validate_participation_guardrail_non_empty_field(
        &mut field_errors,
        "guardrail_mode",
        &evidence.guardrail_mode,
    );
    validate_participation_guardrail_timestamp_field(
        &mut field_errors,
        "observed_at_utc",
        &evidence.observed_at_utc,
    );
    validate_participation_guardrail_timestamp_field(
        &mut field_errors,
        "evaluated_at_utc",
        &evidence.evaluated_at_utc,
    );
    validate_participation_guardrail_finite_field(
        &mut field_errors,
        "liquidity_depth_usd",
        evidence.liquidity_depth_usd,
        false,
    );
    validate_participation_guardrail_finite_field(
        &mut field_errors,
        "inactivity_gap_seconds",
        evidence.inactivity_gap_seconds,
        false,
    );
    validate_participation_guardrail_finite_field(
        &mut field_errors,
        "threshold_liquidity_depth_usd",
        evidence.threshold_liquidity_depth_usd,
        false,
    );
    validate_participation_guardrail_finite_field(
        &mut field_errors,
        "threshold_inactivity_pause_seconds",
        evidence.threshold_inactivity_pause_seconds,
        false,
    );
    validate_participation_guardrail_finite_field(
        &mut field_errors,
        "threshold_overnight_gap_seconds",
        evidence.threshold_overnight_gap_seconds,
        false,
    );
    if let Some(value) = evidence.normal_max_order_size_units {
        validate_participation_guardrail_finite_field(
            &mut field_errors,
            "normal_max_order_size_units",
            value,
            true,
        );
    }
    if let Some(value) = evidence.capped_max_order_size_units {
        validate_participation_guardrail_finite_field(
            &mut field_errors,
            "capped_max_order_size_units",
            value,
            true,
        );
    }
    if ParticipationGuardrailReasonCode::parse(&evidence.reason_code).is_err() {
        field_errors.push(ParticipationGuardrailValidationIssue {
            field: "reason_code",
            code: ParticipationGuardrailReasonCode::InvalidPayload.code(),
            message: "reason_code must be a supported FR41 reason code".to_string(),
        });
    }
    let mode = match ParticipationGuardrailMode::parse(&evidence.guardrail_mode) {
        Ok(mode) => Some(mode),
        Err(_) => {
            field_errors.push(ParticipationGuardrailValidationIssue {
                field: "guardrail_mode",
                code: ParticipationGuardrailReasonCode::InvalidPayload.code(),
                message: "guardrail_mode must be one of: pass, pause, size_cap, unavailable"
                    .to_string(),
            });
            None
        }
    };
    if evidence.observed_at_utc > evidence.evaluated_at_utc {
        field_errors.push(ParticipationGuardrailValidationIssue {
            field: "evaluated_at_utc",
            code: ParticipationGuardrailReasonCode::InvalidPayload.code(),
            message: "evaluated_at_utc must be greater than or equal to observed_at_utc"
                .to_string(),
        });
    }
    if mode == Some(ParticipationGuardrailMode::SizeCap) {
        if evidence.normal_max_order_size_units.is_none() {
            field_errors.push(ParticipationGuardrailValidationIssue {
                field: "normal_max_order_size_units",
                code: ParticipationGuardrailReasonCode::InvalidPayload.code(),
                message: "size_cap mode requires normal_max_order_size_units".to_string(),
            });
        }
        if evidence.capped_max_order_size_units.is_none() {
            field_errors.push(ParticipationGuardrailValidationIssue {
                field: "capped_max_order_size_units",
                code: ParticipationGuardrailReasonCode::InvalidPayload.code(),
                message: "size_cap mode requires capped_max_order_size_units".to_string(),
            });
        }
        if let (Some(normal), Some(capped)) = (
            evidence.normal_max_order_size_units,
            evidence.capped_max_order_size_units,
        ) {
            let expected_capped = normal * FR41_OVERNIGHT_CAP_FACTOR;
            if (capped - expected_capped).abs() > 0.000_000_1 {
                field_errors.push(ParticipationGuardrailValidationIssue {
                    field: "capped_max_order_size_units",
                    code: ParticipationGuardrailReasonCode::InvalidPayload.code(),
                    message: format!(
                        "capped_max_order_size_units must equal normal_max_order_size_units * {FR41_OVERNIGHT_CAP_FACTOR}"
                    ),
                });
            }
        }
    }
    if field_errors.is_empty() {
        return Ok(());
    }
    Err(
        ParticipationGuardrailContractError::invalid_payload_with_issues(
            "FR41 participation guardrail evidence is invalid",
            field_errors,
        ),
    )
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum PreTradeGateDimension {
    Freshness,
    StreamHealth,
    ExposureLimitState,
    ReconciliationHalt,
    UserStreamAuth,
    DrawdownStop,
    StrategyApproval,
    VenueEligibility,
    RewardPerRisk,
    ParticipationGuardrail,
}

impl PreTradeGateDimension {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Freshness => "freshness",
            Self::StreamHealth => "stream_health",
            Self::ExposureLimitState => "exposure_limit_state",
            Self::ReconciliationHalt => "reconciliation_halt",
            Self::UserStreamAuth => "user_stream_auth",
            Self::DrawdownStop => "drawdown_stop",
            Self::StrategyApproval => "strategy_approval",
            Self::VenueEligibility => "venue_eligibility",
            Self::RewardPerRisk => "reward_per_risk",
            Self::ParticipationGuardrail => "participation_guardrail",
        }
    }

    pub fn parse(value: &str) -> Result<Self, PreTradeGateContractError> {
        match value {
            "freshness" => Ok(Self::Freshness),
            "stream_health" => Ok(Self::StreamHealth),
            "exposure_limit_state" => Ok(Self::ExposureLimitState),
            "reconciliation_halt" => Ok(Self::ReconciliationHalt),
            "user_stream_auth" => Ok(Self::UserStreamAuth),
            "drawdown_stop" => Ok(Self::DrawdownStop),
            "strategy_approval" => Ok(Self::StrategyApproval),
            "venue_eligibility" => Ok(Self::VenueEligibility),
            "reward_per_risk" => Ok(Self::RewardPerRisk),
            "participation_guardrail" => Ok(Self::ParticipationGuardrail),
            _ => Err(PreTradeGateContractError::invalid_payload(format!(
                "unknown pre-trade gate dimension `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PreTradeDecisionOutcome {
    Allow,
    Deny,
}

impl PreTradeDecisionOutcome {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
        }
    }

    pub fn parse(value: &str) -> Result<Self, PreTradeGateContractError> {
        match value {
            "allow" => Ok(Self::Allow),
            "deny" => Ok(Self::Deny),
            _ => Err(PreTradeGateContractError::invalid_payload(format!(
                "unknown pre-trade decision outcome `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PreTradeReasonCode {
    Pass,
    FreshnessStateUnavailable,
    FreshnessStaleBreach,
    StreamHealthStateUnavailable,
    StreamHealthDegraded,
    RiskLimitStateUnavailable,
    StratificationStateUnavailable,
    ReconciliationCriticalHalt,
    UserStreamAuthExpired,
    DrawdownStateUnavailable,
    DrawdownStopTriggered,
    StrategyApprovalUnavailable,
    StrategyApprovalRequired,
    VenueEligibilityUnavailable,
    VenueIneligible,
    RewardRiskStateUnavailable,
    RewardRiskBelowThreshold,
    ParticipationGuardrailUnavailable,
    ParticipationGuardrailLowLiquidityPause,
    ParticipationGuardrailInactivityPause,
    ParticipationGuardrailOvernightCapExceeded,
    AdjudicationUnavailable,
    AdjudicationTimeout,
    PersistenceUnavailable,
    InvalidPayload,
}

impl PreTradeReasonCode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Pass => "pretrade_gate_pass",
            Self::FreshnessStateUnavailable => "pretrade_freshness_state_unavailable",
            Self::FreshnessStaleBreach => "pretrade_freshness_stale_breach",
            Self::StreamHealthStateUnavailable => "pretrade_stream_health_state_unavailable",
            Self::StreamHealthDegraded => "pretrade_stream_health_degraded",
            Self::RiskLimitStateUnavailable => "pretrade_risk_limit_state_unavailable",
            Self::StratificationStateUnavailable => "pretrade_stratification_state_unavailable",
            Self::ReconciliationCriticalHalt => "pretrade_reconciliation_critical_halt",
            Self::UserStreamAuthExpired => "pretrade_user_stream_auth_expired",
            Self::DrawdownStateUnavailable => "pretrade_drawdown_state_unavailable",
            Self::DrawdownStopTriggered => "pretrade_drawdown_stop_triggered",
            Self::StrategyApprovalUnavailable => "pretrade_strategy_approval_unavailable",
            Self::StrategyApprovalRequired => "pretrade_strategy_approval_required",
            Self::VenueEligibilityUnavailable => "pretrade_venue_eligibility_unavailable",
            Self::VenueIneligible => "pretrade_venue_ineligible",
            Self::RewardRiskStateUnavailable => "pretrade_reward_risk_state_unavailable",
            Self::RewardRiskBelowThreshold => "pretrade_reward_risk_below_threshold",
            Self::ParticipationGuardrailUnavailable => {
                "pretrade_participation_guardrail_unavailable"
            }
            Self::ParticipationGuardrailLowLiquidityPause => {
                "pretrade_participation_guardrail_low_liquidity_pause"
            }
            Self::ParticipationGuardrailInactivityPause => {
                "pretrade_participation_guardrail_inactivity_pause"
            }
            Self::ParticipationGuardrailOvernightCapExceeded => {
                "pretrade_participation_guardrail_overnight_cap_exceeded"
            }
            Self::AdjudicationUnavailable => "pretrade_adjudication_unavailable",
            Self::AdjudicationTimeout => "pretrade_adjudication_timeout",
            Self::PersistenceUnavailable => "pretrade_persistence_unavailable",
            Self::InvalidPayload => "pretrade_invalid_payload",
        }
    }

    pub fn parse(value: &str) -> Result<Self, PreTradeGateContractError> {
        match value {
            "pretrade_gate_pass" => Ok(Self::Pass),
            "pretrade_freshness_state_unavailable" => Ok(Self::FreshnessStateUnavailable),
            "pretrade_freshness_stale_breach" => Ok(Self::FreshnessStaleBreach),
            "pretrade_stream_health_state_unavailable" => Ok(Self::StreamHealthStateUnavailable),
            "pretrade_stream_health_degraded" => Ok(Self::StreamHealthDegraded),
            "pretrade_risk_limit_state_unavailable" => Ok(Self::RiskLimitStateUnavailable),
            "pretrade_stratification_state_unavailable" => Ok(Self::StratificationStateUnavailable),
            "pretrade_reconciliation_critical_halt" => Ok(Self::ReconciliationCriticalHalt),
            "pretrade_user_stream_auth_expired" => Ok(Self::UserStreamAuthExpired),
            "pretrade_drawdown_state_unavailable" => Ok(Self::DrawdownStateUnavailable),
            "pretrade_drawdown_stop_triggered" => Ok(Self::DrawdownStopTriggered),
            "pretrade_strategy_approval_unavailable" => Ok(Self::StrategyApprovalUnavailable),
            "pretrade_strategy_approval_required" => Ok(Self::StrategyApprovalRequired),
            "pretrade_venue_eligibility_unavailable" => Ok(Self::VenueEligibilityUnavailable),
            "pretrade_venue_ineligible" => Ok(Self::VenueIneligible),
            "pretrade_reward_risk_state_unavailable" => Ok(Self::RewardRiskStateUnavailable),
            "pretrade_reward_risk_below_threshold" => Ok(Self::RewardRiskBelowThreshold),
            "pretrade_participation_guardrail_unavailable" => {
                Ok(Self::ParticipationGuardrailUnavailable)
            }
            "pretrade_participation_guardrail_low_liquidity_pause" => {
                Ok(Self::ParticipationGuardrailLowLiquidityPause)
            }
            "pretrade_participation_guardrail_inactivity_pause" => {
                Ok(Self::ParticipationGuardrailInactivityPause)
            }
            "pretrade_participation_guardrail_overnight_cap_exceeded" => {
                Ok(Self::ParticipationGuardrailOvernightCapExceeded)
            }
            "pretrade_adjudication_unavailable" => Ok(Self::AdjudicationUnavailable),
            "pretrade_adjudication_timeout" => Ok(Self::AdjudicationTimeout),
            "pretrade_persistence_unavailable" => Ok(Self::PersistenceUnavailable),
            "pretrade_invalid_payload" => Ok(Self::InvalidPayload),
            _ => Err(PreTradeGateContractError::invalid_payload(format!(
                "unknown pre-trade reason code `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PreTradeGateValidationIssue {
    pub field: &'static str,
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PreTradeGateContractError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<PreTradeGateValidationIssue>,
}

impl PreTradeGateContractError {
    pub fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: PreTradeReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn invalid_payload_with_issues(
        message: impl Into<String>,
        field_errors: Vec<PreTradeGateValidationIssue>,
    ) -> Self {
        Self {
            code: PreTradeReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PreTradeGateResult {
    pub gate: PreTradeGateDimension,
    pub passed: bool,
    pub reason_code: String,
    pub evaluated_at_utc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PreTradeGateDecision {
    pub decision_id: String,
    pub intent_id: String,
    pub market_id: String,
    pub cluster_id: String,
    pub profile_key: String,
    pub outcome: PreTradeDecisionOutcome,
    pub reason_code: String,
    pub protective_mode_active: bool,
    pub gate_results: Vec<PreTradeGateResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub participation_guardrail: Option<PreTradeParticipationGuardrailEvidence>,
    pub correlation_id: String,
    pub evaluated_at_utc: String,
}

pub fn normalize_pretrade_identifier(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}

pub fn drawdown_stop_triggered(
    current_drawdown_pct: f64,
    configured_stop_threshold_pct: f64,
) -> Result<bool, PreTradeGateContractError> {
    let mut field_errors = Vec::new();
    validate_non_negative_pretrade_numeric(
        &mut field_errors,
        "current_drawdown_pct",
        current_drawdown_pct,
    );
    validate_non_negative_pretrade_numeric(
        &mut field_errors,
        "configured_stop_threshold_pct",
        configured_stop_threshold_pct,
    );
    if !field_errors.is_empty() {
        return Err(PreTradeGateContractError::invalid_payload_with_issues(
            "drawdown stop inputs are invalid",
            field_errors,
        ));
    }
    Ok(current_drawdown_pct >= configured_stop_threshold_pct)
}

pub fn validate_pretrade_gate_result(
    result: &PreTradeGateResult,
) -> Result<(), PreTradeGateContractError> {
    let mut field_errors = Vec::new();
    validate_non_empty_pretrade_field(&mut field_errors, "reason_code", &result.reason_code);
    validate_pretrade_timestamp_field(
        &mut field_errors,
        "evaluated_at_utc",
        &result.evaluated_at_utc,
    );
    if PreTradeReasonCode::parse(&result.reason_code).is_err() {
        field_errors.push(PreTradeGateValidationIssue {
            field: "reason_code",
            code: PreTradeReasonCode::InvalidPayload.code(),
            message: "reason_code must be a known pre-trade reason".to_string(),
        });
    }
    if result.passed && result.reason_code != PreTradeReasonCode::Pass.code() {
        field_errors.push(PreTradeGateValidationIssue {
            field: "reason_code",
            code: PreTradeReasonCode::InvalidPayload.code(),
            message: "passed gate results must use pretrade_gate_pass reason_code".to_string(),
        });
    }
    if !result.passed && result.reason_code == PreTradeReasonCode::Pass.code() {
        field_errors.push(PreTradeGateValidationIssue {
            field: "reason_code",
            code: PreTradeReasonCode::InvalidPayload.code(),
            message: "failed gate results cannot use pretrade_gate_pass reason_code".to_string(),
        });
    }

    if !field_errors.is_empty() {
        return Err(PreTradeGateContractError::invalid_payload_with_issues(
            "pre-trade gate result payload is invalid",
            field_errors,
        ));
    }
    Ok(())
}

pub fn validate_pretrade_gate_decision(
    decision: &PreTradeGateDecision,
) -> Result<(), PreTradeGateContractError> {
    let mut field_errors = Vec::new();
    validate_non_empty_pretrade_field(&mut field_errors, "decision_id", &decision.decision_id);
    validate_non_empty_pretrade_field(&mut field_errors, "intent_id", &decision.intent_id);
    validate_non_empty_pretrade_field(&mut field_errors, "market_id", &decision.market_id);
    validate_non_empty_pretrade_field(&mut field_errors, "cluster_id", &decision.cluster_id);
    validate_non_empty_pretrade_field(&mut field_errors, "profile_key", &decision.profile_key);
    validate_non_empty_pretrade_field(
        &mut field_errors,
        "correlation_id",
        &decision.correlation_id,
    );
    validate_non_empty_pretrade_field(&mut field_errors, "reason_code", &decision.reason_code);
    validate_pretrade_timestamp_field(
        &mut field_errors,
        "evaluated_at_utc",
        &decision.evaluated_at_utc,
    );
    validate_normalized_pretrade_identifier(
        &mut field_errors,
        "decision_id",
        &decision.decision_id,
    );
    validate_normalized_pretrade_identifier(&mut field_errors, "intent_id", &decision.intent_id);
    validate_normalized_pretrade_identifier(&mut field_errors, "market_id", &decision.market_id);
    validate_normalized_pretrade_identifier(&mut field_errors, "cluster_id", &decision.cluster_id);
    validate_normalized_pretrade_identifier(
        &mut field_errors,
        "profile_key",
        &decision.profile_key,
    );
    validate_normalized_pretrade_identifier(
        &mut field_errors,
        "correlation_id",
        &decision.correlation_id,
    );

    if PreTradeReasonCode::parse(&decision.reason_code).is_err() {
        field_errors.push(PreTradeGateValidationIssue {
            field: "reason_code",
            code: PreTradeReasonCode::InvalidPayload.code(),
            message: "reason_code must be a known pre-trade reason".to_string(),
        });
    }
    if decision.gate_results.is_empty() {
        field_errors.push(PreTradeGateValidationIssue {
            field: "gate_results",
            code: PreTradeReasonCode::InvalidPayload.code(),
            message: "gate_results must include at least one gate entry".to_string(),
        });
    }

    for result in &decision.gate_results {
        if let Err(error) = validate_pretrade_gate_result(result) {
            field_errors.extend(error.field_errors);
        }
    }
    if let Some(participation_guardrail) = decision.participation_guardrail.as_ref() {
        if let Err(error) =
            validate_pretrade_participation_guardrail_evidence(participation_guardrail)
        {
            field_errors.extend(error.field_errors.into_iter().map(|issue| {
                PreTradeGateValidationIssue {
                    field: issue.field,
                    code: issue.code,
                    message: issue.message,
                }
            }));
        }
        if normalize_pretrade_identifier(&participation_guardrail.market_id) != decision.market_id {
            field_errors.push(PreTradeGateValidationIssue {
                field: "participation_guardrail.market_id",
                code: PreTradeReasonCode::InvalidPayload.code(),
                message: "participation_guardrail.market_id must match decision market_id"
                    .to_string(),
            });
        }
        if normalize_pretrade_identifier(&participation_guardrail.cluster_id) != decision.cluster_id
        {
            field_errors.push(PreTradeGateValidationIssue {
                field: "participation_guardrail.cluster_id",
                code: PreTradeReasonCode::InvalidPayload.code(),
                message: "participation_guardrail.cluster_id must match decision cluster_id"
                    .to_string(),
            });
        }
        if normalize_pretrade_identifier(&participation_guardrail.correlation_id)
            != decision.correlation_id
        {
            field_errors.push(PreTradeGateValidationIssue {
                field: "participation_guardrail.correlation_id",
                code: PreTradeReasonCode::InvalidPayload.code(),
                message:
                    "participation_guardrail.correlation_id must match decision correlation_id"
                        .to_string(),
            });
        }
        if participation_guardrail.evaluated_at_utc != decision.evaluated_at_utc {
            field_errors.push(PreTradeGateValidationIssue {
                field: "participation_guardrail.evaluated_at_utc",
                code: PreTradeReasonCode::InvalidPayload.code(),
                message:
                    "participation_guardrail.evaluated_at_utc must match decision evaluated_at_utc"
                        .to_string(),
            });
        }
    }

    let failed = decision
        .gate_results
        .iter()
        .filter(|result| !result.passed)
        .count();
    match decision.outcome {
        PreTradeDecisionOutcome::Allow => {
            if failed != 0 {
                field_errors.push(PreTradeGateValidationIssue {
                    field: "outcome",
                    code: PreTradeReasonCode::InvalidPayload.code(),
                    message: "allow outcome cannot include failed gate results".to_string(),
                });
            }
            if decision.reason_code != PreTradeReasonCode::Pass.code() {
                field_errors.push(PreTradeGateValidationIssue {
                    field: "reason_code",
                    code: PreTradeReasonCode::InvalidPayload.code(),
                    message: "allow outcome must use pretrade_gate_pass reason_code".to_string(),
                });
            }
            if decision.protective_mode_active {
                field_errors.push(PreTradeGateValidationIssue {
                    field: "protective_mode_active",
                    code: PreTradeReasonCode::InvalidPayload.code(),
                    message: "allow outcome cannot assert protective_mode_active".to_string(),
                });
            }
        }
        PreTradeDecisionOutcome::Deny => {
            if failed != 1 {
                field_errors.push(PreTradeGateValidationIssue {
                    field: "gate_results",
                    code: PreTradeReasonCode::InvalidPayload.code(),
                    message:
                        "deny outcome must contain exactly one failed gate for deterministic reason precedence"
                            .to_string(),
                });
            }
            if let Some(failed_result) = decision.gate_results.iter().find(|result| !result.passed)
                && decision.reason_code != failed_result.reason_code
            {
                field_errors.push(PreTradeGateValidationIssue {
                    field: "reason_code",
                    code: PreTradeReasonCode::InvalidPayload.code(),
                    message: "deny outcome reason_code must match the failed gate reason_code"
                        .to_string(),
                });
            }
        }
    }
    if decision.protective_mode_active
        && decision.reason_code != PreTradeReasonCode::DrawdownStopTriggered.code()
    {
        field_errors.push(PreTradeGateValidationIssue {
            field: "protective_mode_active",
            code: PreTradeReasonCode::InvalidPayload.code(),
            message:
                "protective_mode_active is only valid for drawdown-stop-triggered deny decisions"
                    .to_string(),
        });
    }

    if !field_errors.is_empty() {
        return Err(PreTradeGateContractError::invalid_payload_with_issues(
            "pre-trade gate decision payload is invalid",
            field_errors,
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn adjudicate_pretrade_gate_results(
    intent_id: &str,
    market_id: &str,
    cluster_id: &str,
    profile_key: &str,
    correlation_id: &str,
    evaluated_at_utc: &str,
    gate_results: Vec<PreTradeGateResult>,
    participation_guardrail: Option<PreTradeParticipationGuardrailEvidence>,
    protective_mode_active: bool,
) -> Result<PreTradeGateDecision, PreTradeGateContractError> {
    if gate_results.is_empty() {
        return Err(PreTradeGateContractError::invalid_payload(
            "gate_results must include at least one entry",
        ));
    }
    for gate_result in &gate_results {
        validate_pretrade_gate_result(gate_result)?;
    }
    if parse_utc_timestamp(evaluated_at_utc).is_err() {
        return Err(PreTradeGateContractError::invalid_payload(
            "evaluated_at_utc must be an RFC3339 UTC timestamp",
        ));
    }

    let failed_gate_reason = gate_results
        .iter()
        .find(|result| !result.passed)
        .map(|result| result.reason_code.clone());
    let (outcome, reason_code) = match failed_gate_reason {
        Some(reason_code) => (PreTradeDecisionOutcome::Deny, reason_code),
        None => (
            PreTradeDecisionOutcome::Allow,
            PreTradeReasonCode::Pass.code().to_string(),
        ),
    };
    let normalized_intent_id = normalize_pretrade_identifier(intent_id);
    let decision = PreTradeGateDecision {
        decision_id: format!(
            "pretrade::{}::{}",
            normalized_intent_id,
            compact_utc_timestamp_token(evaluated_at_utc)
        ),
        intent_id: normalized_intent_id,
        market_id: normalize_pretrade_identifier(market_id),
        cluster_id: normalize_pretrade_identifier(cluster_id),
        profile_key: normalize_pretrade_identifier(profile_key),
        outcome,
        reason_code,
        protective_mode_active,
        gate_results,
        participation_guardrail,
        correlation_id: normalize_pretrade_identifier(correlation_id),
        evaluated_at_utc: evaluated_at_utc.to_string(),
    };
    validate_pretrade_gate_decision(&decision)?;
    Ok(decision)
}

fn validate_non_empty_pretrade_field(
    field_errors: &mut Vec<PreTradeGateValidationIssue>,
    field: &'static str,
    value: &str,
) {
    if value.trim().is_empty() {
        field_errors.push(PreTradeGateValidationIssue {
            field,
            code: PreTradeReasonCode::InvalidPayload.code(),
            message: format!("{field} cannot be blank"),
        });
    }
}

fn validate_pretrade_timestamp_field(
    field_errors: &mut Vec<PreTradeGateValidationIssue>,
    field: &'static str,
    value: &str,
) {
    if parse_utc_timestamp(value).is_err() {
        field_errors.push(PreTradeGateValidationIssue {
            field,
            code: PreTradeReasonCode::InvalidPayload.code(),
            message: format!("{field} must be an RFC3339 UTC timestamp"),
        });
    }
}

fn validate_non_negative_pretrade_numeric(
    field_errors: &mut Vec<PreTradeGateValidationIssue>,
    field: &'static str,
    value: f64,
) {
    if !value.is_finite() {
        field_errors.push(PreTradeGateValidationIssue {
            field,
            code: PreTradeReasonCode::InvalidPayload.code(),
            message: format!("{field} must be finite"),
        });
    } else if value < 0.0 {
        field_errors.push(PreTradeGateValidationIssue {
            field,
            code: PreTradeReasonCode::InvalidPayload.code(),
            message: format!("{field} must be greater than or equal to 0"),
        });
    }
}

fn validate_normalized_pretrade_identifier(
    field_errors: &mut Vec<PreTradeGateValidationIssue>,
    field: &'static str,
    value: &str,
) {
    if normalize_pretrade_identifier(value) != value {
        field_errors.push(PreTradeGateValidationIssue {
            field,
            code: PreTradeReasonCode::InvalidPayload.code(),
            message: format!("{field} must be normalized (trimmed lowercase)"),
        });
    }
}

fn compact_utc_timestamp_token(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_digit())
        .collect()
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EmergencyControlAction {
    Pause,
    ReduceOnly,
    CancelAll,
}

impl EmergencyControlAction {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pause => "pause",
            Self::ReduceOnly => "reduce_only",
            Self::CancelAll => "cancel_all",
        }
    }

    pub fn parse(value: &str) -> Result<Self, EmergencyControlContractError> {
        match value {
            "pause" => Ok(Self::Pause),
            "reduce_only" => Ok(Self::ReduceOnly),
            "cancel_all" => Ok(Self::CancelAll),
            _ => Err(EmergencyControlContractError::invalid_payload(format!(
                "unknown emergency control action `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EmergencyControlSource {
    Manual,
    Automatic,
}

impl EmergencyControlSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Automatic => "automatic",
        }
    }

    pub fn parse(value: &str) -> Result<Self, EmergencyControlContractError> {
        match value {
            "manual" => Ok(Self::Manual),
            "automatic" => Ok(Self::Automatic),
            _ => Err(EmergencyControlContractError::invalid_payload(format!(
                "unknown emergency control source `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EmergencyControlTriggerSource {
    OperatorCommand,
    StaleFeed,
    ReconciliationCritical,
    ControlUncertainty,
}

impl EmergencyControlTriggerSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OperatorCommand => "operator_command",
            Self::StaleFeed => "stale_feed",
            Self::ReconciliationCritical => "reconciliation_critical",
            Self::ControlUncertainty => "control_uncertainty",
        }
    }

    pub fn parse(value: &str) -> Result<Self, EmergencyControlContractError> {
        match value {
            "operator_command" => Ok(Self::OperatorCommand),
            "stale_feed" => Ok(Self::StaleFeed),
            "reconciliation_critical" => Ok(Self::ReconciliationCritical),
            "control_uncertainty" => Ok(Self::ControlUncertainty),
            _ => Err(EmergencyControlContractError::invalid_payload(format!(
                "unknown emergency trigger source `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EmergencyControlMode {
    Normal,
    Paused,
    ReduceOnly,
}

impl EmergencyControlMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Paused => "paused",
            Self::ReduceOnly => "reduce_only",
        }
    }

    pub fn parse(value: &str) -> Result<Self, EmergencyControlContractError> {
        match value {
            "normal" => Ok(Self::Normal),
            "paused" => Ok(Self::Paused),
            "reduce_only" => Ok(Self::ReduceOnly),
            _ => Err(EmergencyControlContractError::invalid_payload(format!(
                "unknown emergency control mode `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EmergencyControlReasonCode {
    PauseActivated,
    PauseActive,
    ReduceOnlyActivated,
    ReduceOnlyActive,
    CancelAllAccepted,
    StaleFeedTriggered,
    ReconciliationCriticalTriggered,
    ControlUncertaintyTriggered,
    PersistenceUnavailable,
    OrchestrationUnavailable,
    UnauthorizedRole,
    NotFound,
    InvalidPayload,
}

impl EmergencyControlReasonCode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::PauseActivated => "emergency_control_pause_activated",
            Self::PauseActive => "emergency_control_pause_active",
            Self::ReduceOnlyActivated => "emergency_control_reduce_only_activated",
            Self::ReduceOnlyActive => "emergency_control_reduce_only_active",
            Self::CancelAllAccepted => "emergency_control_cancel_all_accepted",
            Self::StaleFeedTriggered => "emergency_control_stale_feed_triggered",
            Self::ReconciliationCriticalTriggered => {
                "emergency_control_reconciliation_critical_triggered"
            }
            Self::ControlUncertaintyTriggered => "emergency_control_control_uncertainty_triggered",
            Self::PersistenceUnavailable => "emergency_control_persistence_unavailable",
            Self::OrchestrationUnavailable => "emergency_control_orchestration_unavailable",
            Self::UnauthorizedRole => "emergency_control_unauthorized_role",
            Self::NotFound => "emergency_control_not_found",
            Self::InvalidPayload => "emergency_control_invalid_payload",
        }
    }

    pub fn parse(value: &str) -> Result<Self, EmergencyControlContractError> {
        match value {
            "emergency_control_pause_activated" => Ok(Self::PauseActivated),
            "emergency_control_pause_active" => Ok(Self::PauseActive),
            "emergency_control_reduce_only_activated" => Ok(Self::ReduceOnlyActivated),
            "emergency_control_reduce_only_active" => Ok(Self::ReduceOnlyActive),
            "emergency_control_cancel_all_accepted" => Ok(Self::CancelAllAccepted),
            "emergency_control_stale_feed_triggered" => Ok(Self::StaleFeedTriggered),
            "emergency_control_reconciliation_critical_triggered" => {
                Ok(Self::ReconciliationCriticalTriggered)
            }
            "emergency_control_control_uncertainty_triggered" => {
                Ok(Self::ControlUncertaintyTriggered)
            }
            "emergency_control_persistence_unavailable" => Ok(Self::PersistenceUnavailable),
            "emergency_control_orchestration_unavailable" => Ok(Self::OrchestrationUnavailable),
            "emergency_control_unauthorized_role" => Ok(Self::UnauthorizedRole),
            "emergency_control_not_found" => Ok(Self::NotFound),
            "emergency_control_invalid_payload" => Ok(Self::InvalidPayload),
            _ => Err(EmergencyControlContractError::invalid_payload(format!(
                "unknown emergency control reason code `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmergencyControlValidationIssue {
    pub field: &'static str,
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmergencyControlContractError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<EmergencyControlValidationIssue>,
}

impl EmergencyControlContractError {
    pub fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: EmergencyControlReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn invalid_payload_with_issues(
        message: impl Into<String>,
        field_errors: Vec<EmergencyControlValidationIssue>,
    ) -> Self {
        Self {
            code: EmergencyControlReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmergencyControlCommand {
    pub action_id: String,
    pub action: EmergencyControlAction,
    pub source: EmergencyControlSource,
    pub trigger_source: EmergencyControlTriggerSource,
    pub actor_id: Option<String>,
    pub actor_role: Option<String>,
    pub correlation_id: String,
    pub requested_at_utc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SafetyControlActionRecord {
    pub action_id: String,
    pub source: EmergencyControlSource,
    pub action: EmergencyControlAction,
    pub trigger_source: EmergencyControlTriggerSource,
    pub actor_id: Option<String>,
    pub actor_role: Option<String>,
    pub resulting_mode: EmergencyControlMode,
    pub reason_code: String,
    pub correlation_id: String,
    pub audit_reference: String,
    pub dedupe_key: String,
    pub requested_at_utc: String,
    pub acknowledged_at_utc: String,
    pub effective_at_utc: String,
    pub completed_at_utc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EmergencyControlLatencyEvaluation {
    pub acknowledgement_latency_seconds: f64,
    pub state_reflection_latency_seconds: f64,
    pub acknowledgement_within_boundary: bool,
    pub state_reflection_within_boundary: bool,
}

pub fn normalize_emergency_control_identifier(raw: &str) -> String {
    normalize_pretrade_identifier(raw)
}

pub fn emergency_mode_allows_order_mode(
    mode: EmergencyControlMode,
    order_mode: crate::order::OrderMode,
) -> bool {
    match mode {
        EmergencyControlMode::Normal => true,
        EmergencyControlMode::Paused => false,
        EmergencyControlMode::ReduceOnly => {
            matches!(order_mode, crate::order::OrderMode::ReduceOnly)
        }
    }
}

pub fn stale_feed_triggers_safe_state(
    feed_age_seconds: f64,
) -> Result<bool, EmergencyControlContractError> {
    if !feed_age_seconds.is_finite() || feed_age_seconds < 0.0 {
        return Err(EmergencyControlContractError::invalid_payload(
            "feed_age_seconds must be finite and greater than or equal to 0",
        ));
    }
    Ok(feed_age_seconds > FRESHNESS_STALE_THRESHOLD_SECONDS)
}

pub fn evaluate_emergency_control_latency_boundaries(
    acknowledgement_latency_seconds: f64,
    state_reflection_latency_seconds: f64,
) -> Result<EmergencyControlLatencyEvaluation, EmergencyControlContractError> {
    if !acknowledgement_latency_seconds.is_finite() || acknowledgement_latency_seconds < 0.0 {
        return Err(EmergencyControlContractError::invalid_payload(
            "acknowledgement_latency_seconds must be finite and greater than or equal to 0",
        ));
    }
    if !state_reflection_latency_seconds.is_finite() || state_reflection_latency_seconds < 0.0 {
        return Err(EmergencyControlContractError::invalid_payload(
            "state_reflection_latency_seconds must be finite and greater than or equal to 0",
        ));
    }
    Ok(EmergencyControlLatencyEvaluation {
        acknowledgement_latency_seconds,
        state_reflection_latency_seconds,
        acknowledgement_within_boundary: acknowledgement_latency_seconds
            <= EMERGENCY_CONTROL_ACK_MAX_SECONDS,
        state_reflection_within_boundary: state_reflection_latency_seconds
            <= EMERGENCY_CONTROL_STATE_REFLECTION_MAX_SECONDS,
    })
}

pub fn validate_emergency_control_command(
    command: &EmergencyControlCommand,
) -> Result<(), EmergencyControlContractError> {
    let mut field_errors = Vec::new();
    validate_non_empty_emergency_field(&mut field_errors, "action_id", &command.action_id);
    validate_non_empty_emergency_field(
        &mut field_errors,
        "correlation_id",
        &command.correlation_id,
    );
    validate_emergency_timestamp_field(
        &mut field_errors,
        "requested_at_utc",
        &command.requested_at_utc,
    );
    validate_normalized_emergency_identifier(&mut field_errors, "action_id", &command.action_id);
    validate_normalized_emergency_identifier(
        &mut field_errors,
        "correlation_id",
        &command.correlation_id,
    );

    match command.source {
        EmergencyControlSource::Manual => {
            if command.trigger_source != EmergencyControlTriggerSource::OperatorCommand {
                field_errors.push(EmergencyControlValidationIssue {
                    field: "trigger_source",
                    code: EmergencyControlReasonCode::InvalidPayload.code(),
                    message: "manual controls must use trigger_source `operator_command`"
                        .to_string(),
                });
            }
            validate_optional_non_empty_emergency_field(
                &mut field_errors,
                "actor_id",
                &command.actor_id,
            );
            validate_optional_non_empty_emergency_field(
                &mut field_errors,
                "actor_role",
                &command.actor_role,
            );
        }
        EmergencyControlSource::Automatic => {
            if command.trigger_source == EmergencyControlTriggerSource::OperatorCommand {
                field_errors.push(EmergencyControlValidationIssue {
                    field: "trigger_source",
                    code: EmergencyControlReasonCode::InvalidPayload.code(),
                    message: "automatic controls must use non-manual trigger source".to_string(),
                });
            }
        }
    }

    if !field_errors.is_empty() {
        return Err(EmergencyControlContractError::invalid_payload_with_issues(
            "emergency control command payload is invalid",
            field_errors,
        ));
    }
    Ok(())
}

pub fn validate_safety_control_action_record(
    record: &SafetyControlActionRecord,
) -> Result<(), EmergencyControlContractError> {
    let command = EmergencyControlCommand {
        action_id: record.action_id.clone(),
        action: record.action,
        source: record.source,
        trigger_source: record.trigger_source,
        actor_id: record.actor_id.clone(),
        actor_role: record.actor_role.clone(),
        correlation_id: record.correlation_id.clone(),
        requested_at_utc: record.requested_at_utc.clone(),
    };
    validate_emergency_control_command(&command)?;

    let mut field_errors = Vec::new();
    validate_non_empty_emergency_field(&mut field_errors, "reason_code", &record.reason_code);
    validate_non_empty_emergency_field(
        &mut field_errors,
        "audit_reference",
        &record.audit_reference,
    );
    validate_non_empty_emergency_field(&mut field_errors, "dedupe_key", &record.dedupe_key);
    validate_emergency_timestamp_field(
        &mut field_errors,
        "acknowledged_at_utc",
        &record.acknowledged_at_utc,
    );
    validate_emergency_timestamp_field(
        &mut field_errors,
        "effective_at_utc",
        &record.effective_at_utc,
    );
    validate_emergency_timestamp_field(
        &mut field_errors,
        "completed_at_utc",
        &record.completed_at_utc,
    );
    validate_normalized_emergency_identifier(&mut field_errors, "dedupe_key", &record.dedupe_key);
    if EmergencyControlReasonCode::parse(&record.reason_code).is_err() {
        field_errors.push(EmergencyControlValidationIssue {
            field: "reason_code",
            code: EmergencyControlReasonCode::InvalidPayload.code(),
            message: "reason_code must be a known emergency control reason".to_string(),
        });
    }

    match record.action {
        EmergencyControlAction::Pause if record.resulting_mode != EmergencyControlMode::Paused => {
            field_errors.push(EmergencyControlValidationIssue {
                field: "resulting_mode",
                code: EmergencyControlReasonCode::InvalidPayload.code(),
                message: "pause actions must set resulting_mode to `paused`".to_string(),
            });
        }
        EmergencyControlAction::ReduceOnly
            if record.resulting_mode != EmergencyControlMode::ReduceOnly =>
        {
            field_errors.push(EmergencyControlValidationIssue {
                field: "resulting_mode",
                code: EmergencyControlReasonCode::InvalidPayload.code(),
                message: "reduce-only actions must set resulting_mode to `reduce_only`".to_string(),
            });
        }
        EmergencyControlAction::CancelAll
            if record.resulting_mode != EmergencyControlMode::Paused =>
        {
            field_errors.push(EmergencyControlValidationIssue {
                field: "resulting_mode",
                code: EmergencyControlReasonCode::InvalidPayload.code(),
                message: "cancel-all actions must set resulting_mode to `paused`".to_string(),
            });
        }
        _ => {}
    }

    if record.source == EmergencyControlSource::Automatic
        && record.resulting_mode != EmergencyControlMode::Paused
    {
        field_errors.push(EmergencyControlValidationIssue {
            field: "resulting_mode",
            code: EmergencyControlReasonCode::InvalidPayload.code(),
            message: "automatic triggers must transition to `paused` mode".to_string(),
        });
    }

    match emergency_timestamp_delta_seconds(&record.requested_at_utc, &record.acknowledged_at_utc) {
        Ok(acknowledgement_latency_seconds) => {
            if acknowledgement_latency_seconds > EMERGENCY_CONTROL_ACK_MAX_SECONDS {
                field_errors.push(EmergencyControlValidationIssue {
                    field: "acknowledged_at_utc",
                    code: EmergencyControlReasonCode::InvalidPayload.code(),
                    message: format!(
                        "acknowledgement latency must be <= {EMERGENCY_CONTROL_ACK_MAX_SECONDS}s"
                    ),
                });
            }
        }
        Err(error) => field_errors.extend(error.field_errors),
    }

    match emergency_timestamp_delta_seconds(&record.requested_at_utc, &record.effective_at_utc) {
        Ok(reflection_latency_seconds) => {
            if reflection_latency_seconds > EMERGENCY_CONTROL_STATE_REFLECTION_MAX_SECONDS {
                field_errors.push(EmergencyControlValidationIssue {
                    field: "effective_at_utc",
                    code: EmergencyControlReasonCode::InvalidPayload.code(),
                    message: format!(
                        "state reflection latency must be <= {EMERGENCY_CONTROL_STATE_REFLECTION_MAX_SECONDS}s"
                    ),
                });
            }
            if record.source == EmergencyControlSource::Automatic
                && reflection_latency_seconds > EMERGENCY_CONTROL_SAFE_STATE_MAX_SECONDS
            {
                field_errors.push(EmergencyControlValidationIssue {
                    field: "effective_at_utc",
                    code: EmergencyControlReasonCode::InvalidPayload.code(),
                    message: format!(
                        "automatic safe-state transition latency must be <= {EMERGENCY_CONTROL_SAFE_STATE_MAX_SECONDS}s"
                    ),
                });
            }
        }
        Err(error) => field_errors.extend(error.field_errors),
    }

    if emergency_timestamp_delta_seconds(&record.effective_at_utc, &record.completed_at_utc)
        .is_err()
    {
        field_errors.push(EmergencyControlValidationIssue {
            field: "completed_at_utc",
            code: EmergencyControlReasonCode::InvalidPayload.code(),
            message: "completed_at_utc must be greater than or equal to effective_at_utc"
                .to_string(),
        });
    }

    if !field_errors.is_empty() {
        return Err(EmergencyControlContractError::invalid_payload_with_issues(
            "safety control action record is invalid",
            field_errors,
        ));
    }
    Ok(())
}

fn validate_non_empty_emergency_field(
    field_errors: &mut Vec<EmergencyControlValidationIssue>,
    field: &'static str,
    value: &str,
) {
    if value.trim().is_empty() {
        field_errors.push(EmergencyControlValidationIssue {
            field,
            code: EmergencyControlReasonCode::InvalidPayload.code(),
            message: format!("{field} cannot be blank"),
        });
    }
}

fn validate_optional_non_empty_emergency_field(
    field_errors: &mut Vec<EmergencyControlValidationIssue>,
    field: &'static str,
    value: &Option<String>,
) {
    if value
        .as_deref()
        .map(str::trim)
        .is_none_or(|candidate| candidate.is_empty())
    {
        field_errors.push(EmergencyControlValidationIssue {
            field,
            code: EmergencyControlReasonCode::InvalidPayload.code(),
            message: format!("{field} is required"),
        });
    }
}

fn validate_emergency_timestamp_field(
    field_errors: &mut Vec<EmergencyControlValidationIssue>,
    field: &'static str,
    value: &str,
) {
    if parse_utc_timestamp(value).is_err() {
        field_errors.push(EmergencyControlValidationIssue {
            field,
            code: EmergencyControlReasonCode::InvalidPayload.code(),
            message: format!("{field} must be an RFC3339 UTC timestamp"),
        });
    }
}

fn validate_normalized_emergency_identifier(
    field_errors: &mut Vec<EmergencyControlValidationIssue>,
    field: &'static str,
    value: &str,
) {
    if normalize_emergency_control_identifier(value) != value {
        field_errors.push(EmergencyControlValidationIssue {
            field,
            code: EmergencyControlReasonCode::InvalidPayload.code(),
            message: format!("{field} must be normalized (trimmed lowercase)"),
        });
    }
}

fn emergency_timestamp_delta_seconds(
    from_utc: &str,
    to_utc: &str,
) -> Result<f64, EmergencyControlContractError> {
    let start = parse_utc_timestamp(from_utc).map_err(|_| {
        EmergencyControlContractError::invalid_payload("from_utc must be an RFC3339 UTC timestamp")
    })?;
    let end = parse_utc_timestamp(to_utc).map_err(|_| {
        EmergencyControlContractError::invalid_payload("to_utc must be an RFC3339 UTC timestamp")
    })?;
    let delta_seconds = (end - start).as_seconds_f64();
    if !delta_seconds.is_finite() || delta_seconds < 0.0 {
        return Err(EmergencyControlContractError::invalid_payload(
            "timestamp ordering must be monotonically increasing",
        ));
    }
    Ok(delta_seconds)
}

fn validate_non_negative_freshness_age(
    field_errors: &mut Vec<FreshnessGateValidationIssue>,
    field: &'static str,
    value: Option<f64>,
    required: bool,
) {
    if value.is_none() && required {
        field_errors.push(FreshnessGateValidationIssue {
            field,
            code: FreshnessGateReasonCode::InvalidPayload.code(),
            message: format!("{field} is required"),
        });
        return;
    }

    if let Some(value) = value {
        if !value.is_finite() {
            field_errors.push(FreshnessGateValidationIssue {
                field,
                code: FreshnessGateReasonCode::InvalidPayload.code(),
                message: format!("{field} must be finite"),
            });
        } else if value < 0.0 {
            field_errors.push(FreshnessGateValidationIssue {
                field,
                code: FreshnessGateReasonCode::InvalidPayload.code(),
                message: format!("{field} must be greater than or equal to 0"),
            });
        }
    }
}

fn validate_freshness_timestamp_field(
    field_errors: &mut Vec<FreshnessGateValidationIssue>,
    field: &'static str,
    value: &str,
) {
    if parse_utc_timestamp(value).is_err() {
        field_errors.push(FreshnessGateValidationIssue {
            field,
            code: FreshnessGateReasonCode::InvalidPayload.code(),
            message: format!("{field} must be an RFC3339 UTC timestamp"),
        });
    }
}

fn validate_freshness_non_empty(
    field_errors: &mut Vec<FreshnessGateValidationIssue>,
    field: &'static str,
    value: &str,
) {
    if value.trim().is_empty() {
        field_errors.push(FreshnessGateValidationIssue {
            field,
            code: FreshnessGateReasonCode::InvalidPayload.code(),
            message: format!("{field} cannot be blank"),
        });
    }
}

fn compute_elapsed_seconds(
    start_utc: &str,
    end_utc: &str,
) -> Result<f64, FreshnessGateContractError> {
    let start = parse_utc_timestamp(start_utc).map_err(|_| {
        FreshnessGateContractError::invalid_payload("start timestamp must be RFC3339 UTC")
    })?;
    let end = parse_utc_timestamp(end_utc).map_err(|_| {
        FreshnessGateContractError::invalid_payload("end timestamp must be RFC3339 UTC")
    })?;
    if end < start {
        return Err(FreshnessGateContractError::invalid_payload(
            "end timestamp cannot be earlier than start timestamp",
        ));
    }
    Ok((end - start).as_seconds_f64())
}

fn validate_non_empty_user_stream_field(
    field_errors: &mut Vec<UserStreamValidationIssue>,
    field: &'static str,
    value: &str,
) {
    if value.trim().is_empty() {
        field_errors.push(UserStreamValidationIssue {
            field,
            code: UserStreamReasonCode::InvalidPayload.code(),
            message: format!("{field} cannot be blank"),
        });
    }
}

fn validate_non_negative_user_stream_offset(
    field_errors: &mut Vec<UserStreamValidationIssue>,
    field: &'static str,
    value: i64,
) {
    if value < 0 {
        field_errors.push(UserStreamValidationIssue {
            field,
            code: UserStreamReasonCode::InvalidPayload.code(),
            message: format!("{field} must be greater than or equal to 0"),
        });
    }
}

fn validate_non_negative_user_stream_value(
    field_errors: &mut Vec<UserStreamValidationIssue>,
    field: &'static str,
    value: f64,
) {
    if !value.is_finite() {
        field_errors.push(UserStreamValidationIssue {
            field,
            code: UserStreamReasonCode::InvalidPayload.code(),
            message: format!("{field} must be finite"),
        });
    } else if value < 0.0 {
        field_errors.push(UserStreamValidationIssue {
            field,
            code: UserStreamReasonCode::InvalidPayload.code(),
            message: format!("{field} must be greater than or equal to 0"),
        });
    }
}

fn validate_user_stream_timestamp_field(
    field_errors: &mut Vec<UserStreamValidationIssue>,
    field: &'static str,
    value: &str,
) {
    if parse_utc_timestamp(value).is_err() {
        field_errors.push(UserStreamValidationIssue {
            field,
            code: UserStreamReasonCode::InvalidPayload.code(),
            message: format!("{field} must be an RFC3339 UTC timestamp"),
        });
    }
}

fn validate_non_empty_stream_field(
    field_errors: &mut Vec<MarketStreamValidationIssue>,
    field: &'static str,
    value: &str,
) {
    if value.trim().is_empty() {
        field_errors.push(MarketStreamValidationIssue {
            field,
            code: MarketStreamReasonCode::InvalidPayload.code(),
            message: format!("{field} cannot be blank"),
        });
    }
}

fn validate_non_negative_stream_value(
    field_errors: &mut Vec<MarketStreamValidationIssue>,
    field: &'static str,
    value: f64,
) {
    if !value.is_finite() {
        field_errors.push(MarketStreamValidationIssue {
            field,
            code: MarketStreamReasonCode::InvalidPayload.code(),
            message: format!("{field} must be finite"),
        });
    } else if value < 0.0 {
        field_errors.push(MarketStreamValidationIssue {
            field,
            code: MarketStreamReasonCode::InvalidPayload.code(),
            message: format!("{field} must be greater than or equal to 0"),
        });
    }
}

fn validate_positive_stream_value(
    field_errors: &mut Vec<MarketStreamValidationIssue>,
    field: &'static str,
    value: f64,
) {
    if !value.is_finite() {
        field_errors.push(MarketStreamValidationIssue {
            field,
            code: MarketStreamReasonCode::InvalidPayload.code(),
            message: format!("{field} must be finite"),
        });
    } else if value <= 0.0 {
        field_errors.push(MarketStreamValidationIssue {
            field,
            code: MarketStreamReasonCode::InvalidPayload.code(),
            message: format!("{field} must be greater than 0"),
        });
    }
}

fn validate_market_depth_levels(
    field_errors: &mut Vec<MarketStreamValidationIssue>,
    field: &'static str,
    levels: &[MarketDepthLevel],
) {
    if levels.is_empty() {
        field_errors.push(MarketStreamValidationIssue {
            field,
            code: MarketStreamReasonCode::UnsupportedEventShape.code(),
            message: format!("{field} must contain at least one level"),
        });
        return;
    }
    if levels.len() > MARKET_STREAM_MAX_DEPTH_LEVELS {
        field_errors.push(MarketStreamValidationIssue {
            field,
            code: MarketStreamReasonCode::UnsupportedEventShape.code(),
            message: format!(
                "{field} cannot contain more than {MARKET_STREAM_MAX_DEPTH_LEVELS} levels"
            ),
        });
    }

    for (index, level) in levels.iter().enumerate() {
        if !level.price.is_finite() || level.price < 0.0 {
            field_errors.push(MarketStreamValidationIssue {
                field,
                code: MarketStreamReasonCode::InvalidPayload.code(),
                message: format!("{field}[{index}].price must be finite and non-negative"),
            });
        }
        if !level.size.is_finite() || level.size < 0.0 {
            field_errors.push(MarketStreamValidationIssue {
                field,
                code: MarketStreamReasonCode::InvalidPayload.code(),
                message: format!("{field}[{index}].size must be finite and non-negative"),
            });
        }
    }
}

fn validate_stream_timestamp_field(
    field_errors: &mut Vec<MarketStreamValidationIssue>,
    field: &'static str,
    value: &str,
) {
    if parse_utc_timestamp(value).is_err() {
        field_errors.push(MarketStreamValidationIssue {
            field,
            code: MarketStreamReasonCode::InvalidPayload.code(),
            message: format!("{field} must be an RFC3339 UTC timestamp"),
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_scope_limit(
    field_errors: &mut Vec<RiskLimitValidationIssue>,
    scope_limit: &RiskScopeLimit,
    expected_scope: RiskLimitScope,
    scope_field: &'static str,
    scope_id_field: &'static str,
    max_notional_field: &'static str,
    max_inventory_field: &'static str,
    max_concentration_field: &'static str,
) {
    if scope_limit.scope != expected_scope {
        field_errors.push(RiskLimitValidationIssue {
            field: scope_field,
            code: RiskLimitReasonCode::InvalidScopeInvariant.code(),
            message: format!("{scope_field} must be `{}`", expected_scope.as_str()),
        });
    }

    validate_non_empty_risk_limit_field(field_errors, scope_id_field, &scope_limit.scope_id);
    validate_non_negative_risk_limit_value(
        field_errors,
        max_notional_field,
        scope_limit.max_notional_usd,
    );
    validate_non_negative_risk_limit_value(
        field_errors,
        max_inventory_field,
        scope_limit.max_inventory_units,
    );
    validate_risk_limit_percent_range(
        field_errors,
        max_concentration_field,
        scope_limit.max_concentration_pct_nav,
    );
}

fn validate_non_empty_risk_limit_field(
    field_errors: &mut Vec<RiskLimitValidationIssue>,
    field: &'static str,
    value: &str,
) {
    if value.trim().is_empty() {
        field_errors.push(RiskLimitValidationIssue {
            field,
            code: RiskLimitReasonCode::InvalidPayload.code(),
            message: format!("{field} cannot be blank"),
        });
    }
}

fn validate_non_negative_risk_limit_value(
    field_errors: &mut Vec<RiskLimitValidationIssue>,
    field: &'static str,
    value: f64,
) {
    if !value.is_finite() {
        field_errors.push(RiskLimitValidationIssue {
            field,
            code: RiskLimitReasonCode::InvalidThreshold.code(),
            message: format!("{field} must be finite"),
        });
        return;
    }
    if value < 0.0 {
        field_errors.push(RiskLimitValidationIssue {
            field,
            code: RiskLimitReasonCode::InvalidThreshold.code(),
            message: format!("{field} must be greater than or equal to 0"),
        });
    }
}

fn validate_risk_limit_percent_range(
    field_errors: &mut Vec<RiskLimitValidationIssue>,
    field: &'static str,
    value: f64,
) {
    if !value.is_finite() {
        field_errors.push(RiskLimitValidationIssue {
            field,
            code: RiskLimitReasonCode::InvalidThreshold.code(),
            message: format!("{field} must be finite"),
        });
        return;
    }
    if !(RISK_LIMIT_PERCENT_MIN_PCT_NAV..=RISK_LIMIT_PERCENT_MAX_PCT_NAV).contains(&value) {
        field_errors.push(RiskLimitValidationIssue {
            field,
            code: RiskLimitReasonCode::InvalidThreshold.code(),
            message: format!(
                "{field} must be between {RISK_LIMIT_PERCENT_MIN_PCT_NAV} and {RISK_LIMIT_PERCENT_MAX_PCT_NAV}"
            ),
        });
    }
}

fn validate_risk_limit_timestamp_field(
    field_errors: &mut Vec<RiskLimitValidationIssue>,
    field: &'static str,
    value: &str,
) {
    if parse_utc_timestamp(value).is_err() {
        field_errors.push(RiskLimitValidationIssue {
            field,
            code: RiskLimitReasonCode::InvalidPayload.code(),
            message: format!("{field} must be an RFC3339 UTC timestamp"),
        });
    }
}

fn validate_child_not_above_parent(
    field_errors: &mut Vec<RiskLimitValidationIssue>,
    child_field: &'static str,
    child_value: f64,
    parent_field: &'static str,
    parent_value: f64,
) {
    if child_value > parent_value {
        field_errors.push(RiskLimitValidationIssue {
            field: child_field,
            code: RiskLimitReasonCode::InvalidScopeInvariant.code(),
            message: format!(
                "{child_field} cannot exceed {parent_field}; equality is allowed but strict greater-than is rejected"
            ),
        });
    }
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

fn validate_market_bucket_non_empty_field(
    field_errors: &mut Vec<MarketBucketValidationIssue>,
    field: &'static str,
    value: &str,
) {
    if value.trim().is_empty() {
        field_errors.push(MarketBucketValidationIssue {
            field,
            code: MarketBucketReasonCode::InvalidPayload.code(),
            message: format!("{field} cannot be blank"),
        });
    }
}

fn validate_market_bucket_timestamp_field(
    field_errors: &mut Vec<MarketBucketValidationIssue>,
    field: &'static str,
    value: &str,
) {
    if parse_utc_timestamp(value).is_err() {
        field_errors.push(MarketBucketValidationIssue {
            field,
            code: MarketBucketReasonCode::InvalidPayload.code(),
            message: format!("{field} must be an RFC3339 UTC timestamp"),
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

fn validate_reward_risk_non_empty_field(
    field_errors: &mut Vec<RewardRiskValidationIssue>,
    field: &'static str,
    value: &str,
) {
    if value.trim().is_empty() {
        field_errors.push(RewardRiskValidationIssue {
            field,
            code: RewardRiskReasonCode::InvalidPayload.code(),
            message: format!("{field} cannot be blank"),
        });
    }
}

fn validate_reward_risk_finite_field(
    field_errors: &mut Vec<RewardRiskValidationIssue>,
    field: &'static str,
    value: f64,
) {
    if !value.is_finite() {
        field_errors.push(RewardRiskValidationIssue {
            field,
            code: RewardRiskReasonCode::InvalidThreshold.code(),
            message: format!("{field} must be finite"),
        });
    }
}

fn validate_reward_risk_threshold_field(
    field_errors: &mut Vec<RewardRiskValidationIssue>,
    field: &'static str,
    value: f64,
) {
    if !value.is_finite() {
        field_errors.push(RewardRiskValidationIssue {
            field,
            code: RewardRiskReasonCode::InvalidThreshold.code(),
            message: format!("{field} must be finite"),
        });
        return;
    }
    if value < 0.0 {
        field_errors.push(RewardRiskValidationIssue {
            field,
            code: RewardRiskReasonCode::InvalidThreshold.code(),
            message: format!("{field} must be greater than or equal to 0"),
        });
    }
}

fn validate_reward_risk_volatility_field(
    field_errors: &mut Vec<RewardRiskValidationIssue>,
    field: &'static str,
    value: f64,
) {
    if !value.is_finite() {
        field_errors.push(RewardRiskValidationIssue {
            field,
            code: RewardRiskReasonCode::InvalidThreshold.code(),
            message: format!("{field} must be finite"),
        });
        return;
    }
    if value <= REWARD_RISK_MIN_VOLATILITY_BPS {
        field_errors.push(RewardRiskValidationIssue {
            field,
            code: RewardRiskReasonCode::InvalidThreshold.code(),
            message: format!(
                "{field} must be greater than {REWARD_RISK_MIN_VOLATILITY_BPS} to avoid unstable score division"
            ),
        });
    }
}

fn validate_reward_risk_timestamp_field(
    field_errors: &mut Vec<RewardRiskValidationIssue>,
    field: &'static str,
    value: &str,
) {
    if parse_utc_timestamp(value).is_err() {
        field_errors.push(RewardRiskValidationIssue {
            field,
            code: RewardRiskReasonCode::InvalidPayload.code(),
            message: format!("{field} must be an RFC3339 UTC timestamp"),
        });
    }
}

fn validate_regime_shift_thresholds(
    thresholds: &RegimeShiftThresholds,
) -> Result<(), RegimeShiftContractError> {
    let mut field_errors = Vec::new();
    validate_regime_shift_threshold_field(
        &mut field_errors,
        "thresholds.rebate_delta_bps",
        thresholds.rebate_delta_bps,
    );
    validate_regime_shift_threshold_field(
        &mut field_errors,
        "thresholds.spread_widening_bps",
        thresholds.spread_widening_bps,
    );
    if field_errors.is_empty() {
        return Ok(());
    }
    Err(RegimeShiftContractError::invalid_payload_with_issues(
        "FR40 thresholds are invalid",
        field_errors,
    ))
}

fn validate_regime_shift_threshold_field(
    field_errors: &mut Vec<RegimeShiftValidationIssue>,
    field: &'static str,
    value: f64,
) {
    if !value.is_finite() {
        field_errors.push(RegimeShiftValidationIssue {
            field,
            code: RegimeShiftReasonCode::InvalidPayload.code(),
            message: format!("{field} must be finite"),
        });
        return;
    }
    if value < 0.0 {
        field_errors.push(RegimeShiftValidationIssue {
            field,
            code: RegimeShiftReasonCode::InvalidPayload.code(),
            message: format!("{field} must be greater than or equal to 0"),
        });
    }
}

fn validate_regime_shift_identifiers(
    previous: &MarketSnapshot,
    current: &MarketSnapshot,
    correlation_id: &str,
) -> Result<(), RegimeShiftContractError> {
    let mut field_errors = Vec::new();
    validate_regime_shift_identifier_field(
        &mut field_errors,
        "previous.market_id",
        &previous.market_id,
    );
    validate_regime_shift_identifier_field(
        &mut field_errors,
        "current.market_id",
        &current.market_id,
    );
    validate_regime_shift_identifier_field(
        &mut field_errors,
        "previous.cluster_id",
        &previous.cluster_id,
    );
    validate_regime_shift_identifier_field(
        &mut field_errors,
        "current.cluster_id",
        &current.cluster_id,
    );
    validate_regime_shift_identifier_field(&mut field_errors, "correlation_id", correlation_id);
    if previous
        .market_id
        .trim()
        .eq_ignore_ascii_case(current.market_id.trim())
    {
        // Keep going.
    } else {
        field_errors.push(RegimeShiftValidationIssue {
            field: "market_id",
            code: RegimeShiftReasonCode::InvalidPayload.code(),
            message: "previous and current snapshots must reference the same market_id".to_string(),
        });
    }
    if previous
        .cluster_id
        .trim()
        .eq_ignore_ascii_case(current.cluster_id.trim())
    {
        // Keep going.
    } else {
        field_errors.push(RegimeShiftValidationIssue {
            field: "cluster_id",
            code: RegimeShiftReasonCode::InvalidPayload.code(),
            message: "previous and current snapshots must reference the same cluster_id"
                .to_string(),
        });
    }
    if field_errors.is_empty() {
        return Ok(());
    }
    Err(RegimeShiftContractError::invalid_payload_with_issues(
        "FR40 regime-shift identifier validation failed",
        field_errors,
    ))
}

fn validate_regime_shift_identifier_field(
    field_errors: &mut Vec<RegimeShiftValidationIssue>,
    field: &'static str,
    value: &str,
) {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        field_errors.push(RegimeShiftValidationIssue {
            field,
            code: RegimeShiftReasonCode::InvalidPayload.code(),
            message: format!("{field} cannot be blank"),
        });
        return;
    }
    if !is_canonical_identifier(&normalized) {
        field_errors.push(RegimeShiftValidationIssue {
            field,
            code: RegimeShiftReasonCode::InvalidPayload.code(),
            message: format!("{field} must contain 3-120 canonical characters"),
        });
    }
}

fn validate_regime_shift_observed_timestamp(
    field: &'static str,
    value: &str,
) -> Result<OffsetDateTime, RegimeShiftContractError> {
    parse_utc_timestamp(value).map_err(|_| {
        RegimeShiftContractError::invalid_payload_with_issues(
            format!("{field} must be an RFC3339 UTC timestamp"),
            vec![RegimeShiftValidationIssue {
                field,
                code: RegimeShiftReasonCode::InvalidPayload.code(),
                message: format!("{field} must be an RFC3339 UTC timestamp"),
            }],
        )
    })
}

fn require_regime_shift_f64_option(
    field_errors: &mut Vec<RegimeShiftValidationIssue>,
    field: &'static str,
    value: Option<f64>,
) -> Option<f64> {
    match value {
        Some(value) if value.is_finite() => Some(value),
        Some(_) => {
            field_errors.push(RegimeShiftValidationIssue {
                field,
                code: RegimeShiftReasonCode::DependencyUnavailable.code(),
                message: format!("{field} must be finite"),
            });
            None
        }
        None => {
            field_errors.push(RegimeShiftValidationIssue {
                field,
                code: RegimeShiftReasonCode::DependencyUnavailable.code(),
                message: format!("{field} is required for FR40 evaluation"),
            });
            None
        }
    }
}

fn require_regime_shift_eligibility_state(
    field_errors: &mut Vec<RegimeShiftValidationIssue>,
    field: &'static str,
    value: Option<VenueEligibilityState>,
) -> Option<VenueEligibilityState> {
    match value {
        Some(value) => Some(value),
        None => {
            field_errors.push(RegimeShiftValidationIssue {
                field,
                code: RegimeShiftReasonCode::DependencyUnavailable.code(),
                message: format!("{field} is required for FR40 eligibility-transition evaluation"),
            });
            None
        }
    }
}

fn validate_participation_guardrail_identifier_field(
    field_errors: &mut Vec<ParticipationGuardrailValidationIssue>,
    field: &'static str,
    value: &str,
) {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        field_errors.push(ParticipationGuardrailValidationIssue {
            field,
            code: ParticipationGuardrailReasonCode::InvalidPayload.code(),
            message: format!("{field} cannot be blank"),
        });
        return;
    }
    if !is_canonical_identifier(&normalized) {
        field_errors.push(ParticipationGuardrailValidationIssue {
            field,
            code: ParticipationGuardrailReasonCode::InvalidPayload.code(),
            message: format!("{field} must contain 3-120 canonical characters"),
        });
    }
}

fn validate_participation_guardrail_timestamp_field(
    field_errors: &mut Vec<ParticipationGuardrailValidationIssue>,
    field: &'static str,
    value: &str,
) {
    if parse_utc_timestamp(value).is_err() {
        field_errors.push(ParticipationGuardrailValidationIssue {
            field,
            code: ParticipationGuardrailReasonCode::InvalidPayload.code(),
            message: format!("{field} must be an RFC3339 UTC timestamp"),
        });
    }
}

fn validate_participation_guardrail_non_empty_field(
    field_errors: &mut Vec<ParticipationGuardrailValidationIssue>,
    field: &'static str,
    value: &str,
) {
    if value.trim().is_empty() {
        field_errors.push(ParticipationGuardrailValidationIssue {
            field,
            code: ParticipationGuardrailReasonCode::InvalidPayload.code(),
            message: format!("{field} cannot be blank"),
        });
    }
}

fn validate_participation_guardrail_finite_field(
    field_errors: &mut Vec<ParticipationGuardrailValidationIssue>,
    field: &'static str,
    value: f64,
    strictly_positive: bool,
) {
    if !value.is_finite() {
        field_errors.push(ParticipationGuardrailValidationIssue {
            field,
            code: ParticipationGuardrailReasonCode::InvalidPayload.code(),
            message: format!("{field} must be finite"),
        });
        return;
    }
    if strictly_positive && value <= 0.0 {
        field_errors.push(ParticipationGuardrailValidationIssue {
            field,
            code: ParticipationGuardrailReasonCode::InvalidPayload.code(),
            message: format!("{field} must be > 0"),
        });
    } else if !strictly_positive && value < 0.0 {
        field_errors.push(ParticipationGuardrailValidationIssue {
            field,
            code: ParticipationGuardrailReasonCode::InvalidPayload.code(),
            message: format!("{field} must be >= 0"),
        });
    }
}

fn require_participation_guardrail_signal(
    field_errors: &mut Vec<ParticipationGuardrailValidationIssue>,
    field: &'static str,
    value: Option<f64>,
    strictly_positive: bool,
) -> Option<f64> {
    match value {
        Some(value) if value.is_finite() && (!strictly_positive || value > 0.0) && value >= 0.0 => {
            Some(value)
        }
        Some(value) if !value.is_finite() => {
            field_errors.push(ParticipationGuardrailValidationIssue {
                field,
                code: ParticipationGuardrailReasonCode::DependencyUnavailable.code(),
                message: format!("{field} must be finite"),
            });
            None
        }
        Some(_) if strictly_positive => {
            field_errors.push(ParticipationGuardrailValidationIssue {
                field,
                code: ParticipationGuardrailReasonCode::DependencyUnavailable.code(),
                message: format!("{field} must be > 0"),
            });
            None
        }
        Some(_) => {
            field_errors.push(ParticipationGuardrailValidationIssue {
                field,
                code: ParticipationGuardrailReasonCode::DependencyUnavailable.code(),
                message: format!("{field} must be >= 0"),
            });
            None
        }
        None => {
            field_errors.push(ParticipationGuardrailValidationIssue {
                field,
                code: ParticipationGuardrailReasonCode::DependencyUnavailable.code(),
                message: format!("{field} is required for FR41 evaluation"),
            });
            None
        }
    }
}

fn parse_utc_timestamp(value: &str) -> Result<OffsetDateTime, ()> {
    let parsed = OffsetDateTime::parse(value, &Rfc3339).map_err(|_| ())?;
    if parsed.offset() != UtcOffset::UTC {
        return Err(());
    }
    Ok(parsed)
}

fn is_canonical_identifier(value: &str) -> bool {
    let length = value.len();
    if !(3..=120).contains(&length) {
        return false;
    }
    value.chars().all(|character| {
        character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | ':' | '.')
    })
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

    fn sample_risk_scope(
        scope: RiskLimitScope,
        scope_id: &str,
        max_notional_usd: f64,
        max_inventory_units: f64,
        max_concentration_pct_nav: f64,
    ) -> RiskScopeLimit {
        RiskScopeLimit {
            scope,
            scope_id: scope_id.to_string(),
            max_notional_usd,
            max_inventory_units,
            max_concentration_pct_nav,
        }
    }

    fn sample_risk_profile_version() -> RiskLimitProfileVersion {
        RiskLimitProfileVersion {
            profile_key: "default".to_string(),
            version: 1,
            portfolio: sample_risk_scope(
                RiskLimitScope::Portfolio,
                "portfolio::default",
                1000.0,
                600.0,
                50.0,
            ),
            market: sample_risk_scope(RiskLimitScope::Market, "market::sports", 600.0, 300.0, 40.0),
            strategy: sample_risk_scope(
                RiskLimitScope::Strategy,
                "strategy::maker",
                300.0,
                150.0,
                25.0,
            ),
            status: RiskLimitProfileStatus::Active,
            approval_reference: None,
            actor_id: "ops-1".to_string(),
            reason_code: RiskLimitReasonCode::ProfileApplied.code().to_string(),
            correlation_id: "corr-risk-limit-001".to_string(),
            updated_at_utc: "2026-04-06T00:00:00Z".to_string(),
        }
    }

    fn sample_inventory_rule() -> InventoryLimitRule {
        InventoryLimitRule {
            rule_id: "rule::market::sports".to_string(),
            profile_key: "default".to_string(),
            profile_version: 1,
            scope: RiskLimitScope::Market,
            scope_id: "market::sports".to_string(),
            max_position_units: 250.0,
            max_order_size_units: 40.0,
            max_concentration_pct_nav: 30.0,
            actor_id: "ops-1".to_string(),
            correlation_id: "corr-risk-limit-001".to_string(),
            updated_at_utc: "2026-04-06T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn risk_limit_profile_validation_accepts_equal_boundaries() {
        let mut profile = sample_risk_profile_version();
        profile.market.max_notional_usd = profile.portfolio.max_notional_usd;
        profile.strategy.max_notional_usd = profile.market.max_notional_usd;
        profile.market.max_inventory_units = profile.portfolio.max_inventory_units;
        profile.strategy.max_inventory_units = profile.market.max_inventory_units;
        profile.market.max_concentration_pct_nav = profile.portfolio.max_concentration_pct_nav;
        profile.strategy.max_concentration_pct_nav = profile.market.max_concentration_pct_nav;

        assert!(validate_risk_limit_profile_version(&profile).is_ok());
    }

    #[test]
    fn risk_limit_profile_validation_rejects_child_scope_exceeding_parent() {
        let mut profile = sample_risk_profile_version();
        profile.market.max_notional_usd = profile.portfolio.max_notional_usd + 0.01;

        let error = validate_risk_limit_profile_version(&profile)
            .expect_err("market > portfolio must fail strict child-scope invariant");

        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "market.max_notional_usd")
        );
        assert_eq!(error.code, RiskLimitReasonCode::InvalidPayload.code());
    }

    #[test]
    fn risk_limit_reason_code_parse_round_trip_is_deterministic() {
        let codes = [
            RiskLimitReasonCode::ProfileApplied,
            RiskLimitReasonCode::ProfilePendingApproval,
            RiskLimitReasonCode::ProfileDenied,
            RiskLimitReasonCode::ApprovalRequired,
            RiskLimitReasonCode::PolicyStateUnavailable,
        ];

        for code in codes {
            let parsed = RiskLimitReasonCode::parse(code.code()).expect("known reason must parse");
            assert_eq!(parsed, code);
        }
    }

    #[test]
    fn inventory_rule_validation_rejects_portfolio_scope_and_negative_units() {
        let mut rule = sample_inventory_rule();
        rule.scope = RiskLimitScope::Portfolio;
        rule.max_position_units = -1.0;

        let error =
            validate_inventory_limit_rule(&rule).expect_err("invalid inventory rule must fail");
        let fields: Vec<_> = error.field_errors.iter().map(|issue| issue.field).collect();
        assert!(fields.contains(&"scope"));
        assert!(fields.contains(&"max_position_units"));
    }

    #[test]
    fn risk_limit_increase_requires_approval_only_for_strict_increase() {
        let baseline = sample_risk_profile_version();
        let mut equal = baseline.clone();
        equal.version = 2;
        assert!(!requires_risk_limit_increase_approval(
            Some(&baseline),
            &equal
        ));

        let mut increase = baseline.clone();
        increase.version = 2;
        increase.strategy.max_notional_usd += 1.0;
        assert!(requires_risk_limit_increase_approval(
            Some(&baseline),
            &increase
        ));
    }

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
            inactivity_gap_seconds: Some(300.0),
            spread_bps: 2.5,
            reward_score: 0.7,
            expected_reward_bps: Some(80.0),
            maker_rebate_bps: Some(10.0),
            expected_cost_bps: Some(20.0),
            expected_volatility_bps: Some(100.0),
            venue_eligibility_state: Some(VenueEligibilityState::Eligible),
            projected_exposure_pct_nav: 12.0,
            observed_at_utc: "2026-04-06T00:00:00Z".to_string(),
        }
    }

    fn sample_bucket_profile() -> MarketBucketProfile {
        MarketBucketProfile {
            profile_id: "bucket::market_yes_no_1::cluster_alpha".to_string(),
            market_id: "market_yes_no_1".to_string(),
            cluster_id: "cluster_alpha".to_string(),
            bucket_type: "core".to_string(),
            risk_policy_key: "core-risk-default".to_string(),
            allocation_policy_key: "core-allocation-default".to_string(),
            is_active: true,
            actor_id: "ops-1".to_string(),
            correlation_id: "corr-bucket-001".to_string(),
            updated_at_utc: "2026-04-07T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn market_bucket_type_parse_accepts_core_and_satellite_only() {
        let core = MarketBucketType::parse(" core ")
            .expect("core should parse after canonical normalization");
        let satellite = MarketBucketType::parse("SATELLITE")
            .expect("satellite should parse after canonical normalization");

        assert_eq!(core, MarketBucketType::Core);
        assert_eq!(satellite, MarketBucketType::Satellite);
        assert!(MarketBucketType::parse("hedged").is_err());
    }

    #[test]
    fn market_bucket_profile_validation_rejects_unknown_bucket_type() {
        let mut profile = sample_bucket_profile();
        profile.bucket_type = "growth".to_string();
        let error = validate_market_bucket_profile(&profile)
            .expect_err("unsupported bucket type should fail validation");

        assert_eq!(error.code, MarketBucketReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "bucket_type")
        );
    }

    #[test]
    fn market_bucket_profile_validation_accepts_core_satellite_boundaries() {
        let core = sample_bucket_profile();
        assert!(validate_market_bucket_profile(&core).is_ok());

        let mut satellite = sample_bucket_profile();
        satellite.profile_id = "bucket::market_yes_no_2::cluster_alpha".to_string();
        satellite.market_id = "market_yes_no_2".to_string();
        satellite.bucket_type = "satellite".to_string();
        assert!(validate_market_bucket_profile(&satellite).is_ok());
    }

    #[test]
    fn resolve_market_bucket_policy_links_returns_canonical_active_mapping() {
        let mut profile = sample_bucket_profile();
        profile.profile_id = " Bucket::Market_Yes_No_1::Cluster_Alpha ".to_string();
        profile.market_id = " Market_Yes_No_1 ".to_string();
        profile.cluster_id = " Cluster_Alpha ".to_string();
        profile.risk_policy_key = " Core-Risk-Default ".to_string();
        profile.allocation_policy_key = " Core-Allocation-Default ".to_string();

        let resolved = resolve_market_bucket_policy_links(
            "MARKET_YES_NO_1",
            "cluster_alpha",
            &[profile],
        )
        .expect("active canonical mapping should resolve");

        assert_eq!(resolved.profile_id, "bucket::market_yes_no_1::cluster_alpha");
        assert_eq!(resolved.market_id, "market_yes_no_1");
        assert_eq!(resolved.cluster_id, "cluster_alpha");
        assert_eq!(resolved.bucket_type, "core");
        assert_eq!(resolved.risk_policy_key, "core-risk-default");
        assert_eq!(resolved.allocation_policy_key, "core-allocation-default");
    }

    #[test]
    fn resolve_market_bucket_policy_links_fails_closed_when_mapping_missing() {
        let error = resolve_market_bucket_policy_links("market_yes_no_1", "cluster_alpha", &[])
            .expect_err("missing mapping should fail closed");

        assert_eq!(error.code, MarketBucketReasonCode::MappingUnavailable.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "market_id")
        );
    }

    #[test]
    fn resolve_market_bucket_policy_links_rejects_duplicate_active_mappings() {
        let mut left = sample_bucket_profile();
        left.profile_id = "bucket::market_yes_no_1::cluster_alpha::v1".to_string();
        let mut right = sample_bucket_profile();
        right.profile_id = "bucket::market_yes_no_1::cluster_alpha::v2".to_string();

        let error = resolve_market_bucket_policy_links(
            "market_yes_no_1",
            "cluster_alpha",
            &[left, right],
        )
        .expect_err("duplicate active mappings must be deterministic conflict");

        assert_eq!(error.code, MarketBucketReasonCode::MappingConflict.code());
    }

    #[test]
    fn reward_risk_formula_computation_is_deterministic() {
        let score = compute_reward_per_risk_score(&RewardRiskScoreInput {
            expected_reward_bps: 110.0,
            maker_rebate_bps: 15.0,
            expected_cost_bps: 25.0,
            expected_volatility_bps: 100.0,
        })
        .expect("valid FR39 inputs should compute score");

        assert_eq!(score, 1.0);
    }

    #[test]
    fn reward_risk_threshold_defaults_to_story_boundary_when_policy_missing() {
        assert_eq!(
            reward_risk_threshold_for_policy(None),
            REWARD_RISK_DEFAULT_THRESHOLD
        );
    }

    #[test]
    fn reward_risk_score_input_from_snapshot_rejects_missing_components_as_unavailable_state() {
        let mut snapshot = sample_snapshot();
        snapshot.expected_volatility_bps = None;

        let error = reward_risk_score_input_from_snapshot(&snapshot)
            .expect_err("missing FR39 component should fail closed");
        assert_eq!(
            error.code,
            RewardRiskReasonCode::ScoreStateUnavailable.code()
        );
    }

    #[test]
    fn reward_risk_validation_rejects_non_finite_inputs_and_near_zero_volatility() {
        let error = validate_reward_risk_score_input(&RewardRiskScoreInput {
            expected_reward_bps: f64::NAN,
            maker_rebate_bps: 0.0,
            expected_cost_bps: 0.0,
            expected_volatility_bps: 0.0,
        })
        .expect_err("invalid FR39 score inputs should fail validation");
        assert_eq!(error.code, RewardRiskReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "expected_reward_bps")
        );
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "expected_volatility_bps")
        );
    }

    #[test]
    fn reward_risk_threshold_boundary_score_equal_is_eligible() {
        let score = compute_reward_per_risk_score(&RewardRiskScoreInput {
            expected_reward_bps: 120.0,
            maker_rebate_bps: 0.0,
            expected_cost_bps: 0.0,
            expected_volatility_bps: 100.0,
        })
        .expect("boundary inputs should compute score");
        assert_eq!(score, REWARD_RISK_DEFAULT_THRESHOLD);
        assert!(score >= REWARD_RISK_DEFAULT_THRESHOLD);
    }

    #[test]
    fn reward_risk_reason_code_parse_round_trip_is_deterministic() {
        let codes = [
            RewardRiskReasonCode::ScoreEligible,
            RewardRiskReasonCode::ScoreBelowThreshold,
            RewardRiskReasonCode::ScoreStateUnavailable,
            RewardRiskReasonCode::DefaultThresholdApplied,
            RewardRiskReasonCode::PolicyUpdated,
        ];

        for code in codes {
            let parsed = RewardRiskReasonCode::parse(code.code())
                .expect("known reward-risk reason code should parse");
            assert_eq!(parsed, code);
        }
    }

    fn sample_regime_snapshot(
        maker_rebate_bps: Option<f64>,
        spread_bps: f64,
        eligibility_state: Option<VenueEligibilityState>,
    ) -> MarketSnapshot {
        MarketSnapshot {
            market_id: "market_yes_no_1".to_string(),
            cluster_id: "cluster_alpha".to_string(),
            liquidity_depth_usd: 700.0,
            inactivity_gap_seconds: Some(300.0),
            spread_bps,
            reward_score: 0.7,
            expected_reward_bps: Some(80.0),
            maker_rebate_bps,
            expected_cost_bps: Some(20.0),
            expected_volatility_bps: Some(100.0),
            projected_exposure_pct_nav: 12.0,
            observed_at_utc: "2026-04-06T00:00:00Z".to_string(),
            venue_eligibility_state: eligibility_state,
        }
    }

    #[test]
    fn fr40_regime_shift_threshold_boundaries_are_strict_greater_than() {
        let previous =
            sample_regime_snapshot(Some(10.0), 120.0, Some(VenueEligibilityState::Eligible));
        let current =
            sample_regime_snapshot(Some(30.0), 170.0, Some(VenueEligibilityState::Eligible));
        let detections =
            evaluate_fr40_regime_shift(&previous, &current, "corr-fr40-boundary", None)
                .expect("boundary payload should validate");
        assert!(
            detections.is_empty(),
            "exact +20 rebate delta and +50 spread widening must not trigger"
        );
    }

    #[test]
    fn fr40_regime_shift_detects_rebate_spread_and_eligibility_transitions() {
        let previous =
            sample_regime_snapshot(Some(8.0), 80.0, Some(VenueEligibilityState::Eligible));
        let current =
            sample_regime_snapshot(Some(30.5), 145.5, Some(VenueEligibilityState::Restricted));

        let detections = evaluate_fr40_regime_shift(&previous, &current, "corr-fr40-trigger", None)
            .expect("valid payload should emit detections");
        let reason_codes: Vec<&str> = detections
            .iter()
            .map(|detection| detection.reason_code.as_str())
            .collect();
        assert!(reason_codes.contains(&RegimeShiftReasonCode::RebateDeltaExceeded.code()));
        assert!(reason_codes.contains(&RegimeShiftReasonCode::SpreadWideningExceeded.code()));
        assert!(reason_codes.contains(&RegimeShiftReasonCode::EligibilityTransition.code()));
    }

    #[test]
    fn fr40_regime_shift_fails_closed_when_required_baseline_data_is_missing() {
        let previous =
            sample_regime_snapshot(Some(10.0), 80.0, Some(VenueEligibilityState::Eligible));
        let current = sample_regime_snapshot(None, 140.0, Some(VenueEligibilityState::Restricted));
        let error =
            evaluate_fr40_regime_shift(&previous, &current, "corr-fr40-missing-baseline", None)
                .expect_err("missing economics baseline should fail closed");
        assert_eq!(
            error.code,
            RegimeShiftReasonCode::DependencyUnavailable.code()
        );
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "current.maker_rebate_bps")
        );
    }

    #[test]
    fn fr40_regime_shift_rejects_out_of_order_observation_timestamps() {
        let mut previous =
            sample_regime_snapshot(Some(10.0), 80.0, Some(VenueEligibilityState::Eligible));
        previous.observed_at_utc = "2026-04-06T00:01:00Z".to_string();
        let mut current =
            sample_regime_snapshot(Some(32.0), 150.0, Some(VenueEligibilityState::Restricted));
        current.observed_at_utc = "2026-04-06T00:00:00Z".to_string();

        let error = evaluate_fr40_regime_shift(&previous, &current, "corr-fr40-chronology", None)
            .expect_err("out-of-order observations must fail validation");
        assert_eq!(error.code, RegimeShiftReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "current.observed_at_utc")
        );
    }

    #[test]
    fn venue_eligibility_state_parse_rejects_unsupported_values() {
        let error = VenueEligibilityState::parse("partially_eligible")
            .expect_err("unsupported state should be rejected");
        assert_eq!(error.code, RegimeShiftReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "eligibility_state")
        );
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

    fn sample_market_stream_tick() -> MarketStreamTick {
        MarketStreamTick {
            tick_id: "tick-1".to_string(),
            market_id: "market_yes_no_1".to_string(),
            asset_id:
                "106585164761922456203746651621390029417453862034640469075081961934906147433548"
                    .to_string(),
            cluster_id: "cluster_alpha".to_string(),
            best_bid: 0.51,
            best_ask: 0.53,
            top_bid_depth: vec![
                MarketDepthLevel {
                    price: 0.51,
                    size: 1200.0,
                },
                MarketDepthLevel {
                    price: 0.50,
                    size: 900.0,
                },
            ],
            top_ask_depth: vec![
                MarketDepthLevel {
                    price: 0.53,
                    size: 1000.0,
                },
                MarketDepthLevel {
                    price: 0.54,
                    size: 800.0,
                },
            ],
            last_trade_price: 0.52,
            tick_size: 0.01,
            market_status: MarketStatus::Trading,
            correlation_id: "corr-market-stream-001".to_string(),
            observed_at_utc: "2026-04-06T00:00:00Z".to_string(),
            ingested_at_utc: "2026-04-06T00:00:01Z".to_string(),
            ingestion_latency_seconds: 1.0,
        }
    }

    fn sample_quarantined_market_event() -> QuarantinedMarketEvent {
        QuarantinedMarketEvent {
            event_id: "quarantine-1".to_string(),
            reason_code: MarketStreamReasonCode::InvalidPayload.code().to_string(),
            correlation_id: "corr-market-stream-001".to_string(),
            observed_at_utc: "2026-04-06T00:00:00Z".to_string(),
            quarantined_at_utc: "2026-04-06T00:00:01Z".to_string(),
            ingestion_latency_seconds: 1.0,
            raw_payload: serde_json::json!({
                "event_type": "book",
                "market": "0x1234"
            }),
        }
    }

    #[test]
    fn market_status_parse_rejects_unknown_values() {
        let error =
            MarketStatus::parse("auction").expect_err("unknown market status must be rejected");
        assert_eq!(error.code, MarketStreamReasonCode::InvalidPayload.code());
    }

    #[test]
    fn market_stream_tick_validation_rejects_more_than_top_five_levels() {
        let mut tick = sample_market_stream_tick();
        tick.top_bid_depth = vec![
            MarketDepthLevel {
                price: 0.51,
                size: 1.0,
            };
            MARKET_STREAM_MAX_DEPTH_LEVELS + 1
        ];
        let error = validate_market_stream_tick(&tick).expect_err("depth > 5 must be rejected");
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "top_bid_depth")
        );
    }

    #[test]
    fn market_stream_tick_to_snapshot_is_deterministic() {
        let tick = sample_market_stream_tick();
        let snapshot = market_stream_tick_to_snapshot(&tick, 0.7, 12.0)
            .expect("valid tick should translate to snapshot");

        assert_eq!(snapshot.market_id, tick.market_id);
        assert_eq!(snapshot.cluster_id, tick.cluster_id);
        assert_eq!(snapshot.observed_at_utc, tick.observed_at_utc);
        assert!(snapshot.spread_bps > 0.0);
        assert!(snapshot.liquidity_depth_usd > 0.0);
    }

    #[test]
    fn assess_market_stream_health_handles_5s_and_10s_boundaries_deterministically() {
        let warning =
            assess_market_stream_health(5.0, 0.0, 4.0, 15.0).expect("5s backlog should be valid");
        assert_eq!(warning.status, MarketStreamHealthStatus::Healthy);
        assert_eq!(
            warning.reason_code,
            MarketStreamReasonCode::BacklogWarning.code()
        );

        let boundary = assess_market_stream_health(10.0, 30.0, 4.0, 15.0)
            .expect("10s backlog boundary should stay deterministic");
        assert_eq!(boundary.status, MarketStreamHealthStatus::Healthy);
        assert_eq!(
            boundary.reason_code,
            MarketStreamReasonCode::BacklogWarning.code()
        );
    }

    #[test]
    fn assess_market_stream_health_enters_degraded_only_after_sustained_backlog_gate() {
        let short_sustained = assess_market_stream_health(10.1, 30.0, 4.0, 15.0)
            .expect("backlog > 10 without sustained > 30 should be valid");
        assert_eq!(short_sustained.status, MarketStreamHealthStatus::Degraded);
        assert_eq!(
            short_sustained.reason_code,
            MarketStreamReasonCode::BacklogExceeded.code()
        );

        let sustained = assess_market_stream_health(10.1, 30.1, 4.0, 15.0)
            .expect("backlog > 10 for > 30 should degrade");
        assert_eq!(sustained.status, MarketStreamHealthStatus::Degraded);
        assert_eq!(
            sustained.reason_code,
            MarketStreamReasonCode::BacklogSustainedExceeded.code()
        );
    }

    #[test]
    fn assess_market_stream_health_degrades_on_heartbeat_timeout() {
        let assessment = assess_market_stream_health(1.0, 0.0, 16.0, 15.0)
            .expect("heartbeat timeout inputs should be valid");
        assert_eq!(assessment.status, MarketStreamHealthStatus::Degraded);
        assert_eq!(
            assessment.reason_code,
            MarketStreamReasonCode::HeartbeatTimeout.code()
        );
    }

    #[test]
    fn ingestion_latency_slo_uses_99th_percentile_boundary() {
        let mut passing = vec![1.5; 100];
        passing[99] = 2.0;
        assert!(ingestion_latency_slo_met(&passing));

        let mut failing = vec![1.5; 100];
        failing[98] = 2.1;
        failing[99] = 2.2;
        assert!(!ingestion_latency_slo_met(&failing));
    }

    #[test]
    fn quarantined_market_event_validation_requires_known_reason_code() {
        let mut event = sample_quarantined_market_event();
        event.reason_code = "market_stream_unknown_reason".to_string();
        let error = validate_quarantined_market_event(&event)
            .expect_err("unknown reason code should fail validation");
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "reason_code")
        );
    }

    fn sample_user_stream_event() -> UserStreamEvent {
        UserStreamEvent {
            event_id: "order::order-1::100".to_string(),
            event_kind: UserStreamEventKind::Order,
            event_status: UserStreamEventStatus::Placement,
            market_id: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .to_string(),
            asset_id:
                "106585164761922456203746651621390029417453862034640469075081961934906147433548"
                    .to_string(),
            order_id: "order-1".to_string(),
            trade_id: None,
            partition_key: "order-1".to_string(),
            idempotency_key: "order-1::100::placement".to_string(),
            event_offset: 100,
            event_timestamp_utc: "2026-04-06T00:00:00Z".to_string(),
            observed_at_utc: "2026-04-06T00:00:00Z".to_string(),
            ingested_at_utc: "2026-04-06T00:00:01Z".to_string(),
            ingestion_latency_seconds: 1.0,
            correlation_id: "corr-user-stream-001".to_string(),
            reason_code: UserStreamReasonCode::EventAccepted.code().to_string(),
        }
    }

    fn sample_user_stream_cursor() -> OrderEventOffsetCursor {
        OrderEventOffsetCursor {
            partition_key: "order-1".to_string(),
            last_event_id: "order::order-1::100".to_string(),
            last_event_key: "order-1::100::placement".to_string(),
            last_event_offset: 100,
            auth_state: UserStreamAuthState::Authenticated,
            block_new_intents: false,
            reason_code: UserStreamReasonCode::EventAccepted.code().to_string(),
            correlation_id: "corr-user-stream-001".to_string(),
            updated_at_utc: "2026-04-06T00:00:01Z".to_string(),
        }
    }

    #[test]
    fn user_stream_event_validation_rejects_non_utc_timestamp() {
        let mut event = sample_user_stream_event();
        event.event_timestamp_utc = "2026-04-06T00:00:00+01:00".to_string();
        let error =
            validate_user_stream_event(&event).expect_err("non-UTC timestamp should be rejected");
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "event_timestamp_utc")
        );
    }

    #[test]
    fn user_stream_ordering_classifies_duplicate_equal_offset_events() {
        let event = sample_user_stream_event();
        let cursor = sample_user_stream_cursor();
        let decision = evaluate_user_stream_ordering(Some(&cursor), &event)
            .expect("equal offset + same key should classify as duplicate");
        assert_eq!(
            decision,
            UserStreamOrderingDecision::Duplicate {
                reason_code: UserStreamReasonCode::DuplicateEvent.code(),
            }
        );
    }

    #[test]
    fn user_stream_ordering_rejects_lower_offset_events() {
        let mut event = sample_user_stream_event();
        event.event_offset = 99;
        let cursor = sample_user_stream_cursor();
        let decision = evaluate_user_stream_ordering(Some(&cursor), &event)
            .expect("lower offset should classify as out-of-order");
        assert_eq!(
            decision,
            UserStreamOrderingDecision::OutOfOrder {
                reason_code: UserStreamReasonCode::OutOfOrderEvent.code(),
            }
        );
    }

    #[test]
    fn user_stream_ordering_equal_offset_tie_break_is_deterministic() {
        let mut event = sample_user_stream_event();
        event.idempotency_key = "order-1::100::update".to_string();
        let mut cursor = sample_user_stream_cursor();
        cursor.last_event_key = "order-1::100::placement".to_string();
        let accepted = evaluate_user_stream_ordering(Some(&cursor), &event)
            .expect("lexically larger key should win equal-offset tie-break");
        assert_eq!(
            accepted,
            UserStreamOrderingDecision::Accept {
                reason_code: UserStreamReasonCode::EqualOffsetTieBreakAccepted.code(),
            }
        );

        cursor.last_event_key = "order-1::100::zzz".to_string();
        let rejected = evaluate_user_stream_ordering(Some(&cursor), &event)
            .expect("lexically smaller key should be rejected for equal-offset tie-break");
        assert_eq!(
            rejected,
            UserStreamOrderingDecision::OutOfOrder {
                reason_code: UserStreamReasonCode::EqualOffsetTieBreakRejected.code(),
            }
        );
    }

    #[test]
    fn user_stream_auth_transition_enforces_fail_closed_auth_expired_block() {
        let transition = UserStreamAuthTransition {
            transition_id: "transition-1".to_string(),
            transition_offset: 1,
            auth_state: UserStreamAuthState::AuthExpired,
            block_new_intents: false,
            reason_code: UserStreamReasonCode::AuthExpired.code().to_string(),
            correlation_id: "corr-auth-001".to_string(),
            observed_at_utc: "2026-04-06T00:00:01Z".to_string(),
        };
        let error = validate_user_stream_auth_transition(&transition)
            .expect_err("auth_expired transition must enforce block_new_intents=true");
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "block_new_intents")
        );
    }

    #[test]
    fn user_stream_normalize_idempotency_key_trims_and_lowercases() {
        assert_eq!(
            normalize_user_stream_idempotency_key("  Order-1::100::PLACEMENT  "),
            "order-1::100::placement"
        );
    }

    #[test]
    fn user_stream_latency_slo_uses_99th_percentile_boundary() {
        let mut passing = vec![1.4; 100];
        passing[99] = USER_STREAM_LATENCY_TARGET_SECONDS;
        assert!(user_stream_latency_slo_met(&passing));

        let mut failing = vec![1.4; 100];
        failing[98] = USER_STREAM_LATENCY_TARGET_SECONDS + 0.1;
        failing[99] = USER_STREAM_LATENCY_TARGET_SECONDS + 0.2;
        assert!(!user_stream_latency_slo_met(&failing));
    }

    fn sample_freshness_policy() -> FreshnessGatePolicy {
        FreshnessGatePolicy {
            stale_threshold_seconds: 30.0,
            stability_window_seconds: 10.0,
            max_breach_to_pause_seconds: 5.0,
        }
    }

    fn sample_freshness_snapshot(
        market_age: Option<f64>,
        user_age: Option<f64>,
        sampled_at_utc: &str,
    ) -> FreshnessSnapshot {
        FreshnessSnapshot {
            market_data_age_seconds: market_age,
            user_data_age_seconds: user_age,
            sampled_at_utc: sampled_at_utc.to_string(),
            correlation_id: "corr-freshness-001".to_string(),
        }
    }

    fn sample_paused_freshness_state() -> FreshnessGateState {
        FreshnessGateState {
            pause_active: true,
            reason_code: FreshnessGateReasonCode::StaleBreach.code().to_string(),
            stale_breach_detected_at_utc: Some("2026-04-06T00:00:00Z".to_string()),
            pause_activated_at_utc: Some("2026-04-06T00:00:00Z".to_string()),
            recovery_window_started_at_utc: None,
            last_evaluated_at_utc: "2026-04-06T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn freshness_gate_boundary_29s_and_30s_remain_unpaused() {
        let policy = sample_freshness_policy();
        let snapshot = sample_freshness_snapshot(Some(29.0), Some(30.0), "2026-04-06T00:00:30Z");

        let outcome = evaluate_freshness_gate(&snapshot, None, &policy)
            .expect("boundary-safe snapshot should evaluate cleanly");

        assert!(!outcome.pause_active);
        assert!(!outcome.block_new_order_creation);
        assert_eq!(
            outcome.reason_code,
            FreshnessGateReasonCode::BoundarySafe.code()
        );
        assert_eq!(outcome.transition, None);
    }

    #[test]
    fn freshness_gate_pauses_immediately_when_age_exceeds_30s() {
        let policy = sample_freshness_policy();
        let snapshot = sample_freshness_snapshot(Some(31.0), Some(30.0), "2026-04-06T00:01:00Z");

        let outcome = evaluate_freshness_gate(&snapshot, None, &policy)
            .expect("stale breach should evaluate cleanly");

        assert!(outcome.pause_active);
        assert!(outcome.block_new_order_creation);
        assert_eq!(
            outcome.transition,
            Some(FreshnessGateTransition::PauseActivated)
        );
        assert_eq!(
            outcome.reason_code,
            FreshnessGateReasonCode::StaleBreach.code()
        );
        assert!(
            outcome
                .breach_to_pause_latency_seconds
                .is_some_and(|latency| latency <= 5.0)
        );
    }

    #[test]
    fn freshness_gate_missing_inputs_fail_closed_with_state_unavailable_reason() {
        let policy = sample_freshness_policy();
        let snapshot = sample_freshness_snapshot(None, Some(12.0), "2026-04-06T00:01:05Z");

        let outcome = evaluate_freshness_gate(&snapshot, None, &policy).expect(
            "missing freshness signal should still produce deterministic fail-closed outcome",
        );

        assert!(outcome.pause_active);
        assert!(outcome.block_new_order_creation);
        assert_eq!(
            outcome.reason_code,
            FreshnessGateReasonCode::StateUnavailable.code()
        );
        assert_eq!(
            outcome.transition,
            Some(FreshnessGateTransition::PauseActivated)
        );
    }

    #[test]
    fn freshness_gate_recovery_window_minus_one_stays_paused_and_window_unlocks() {
        let policy = sample_freshness_policy();
        let paused_state = sample_paused_freshness_state();

        let pending_start_snapshot =
            sample_freshness_snapshot(Some(30.0), Some(29.0), "2026-04-06T00:00:40Z");
        let pending_start =
            evaluate_freshness_gate(&pending_start_snapshot, Some(&paused_state), &policy)
                .expect("fresh snapshot after pause should enter recovery_pending");
        assert_eq!(
            pending_start.transition,
            Some(FreshnessGateTransition::RecoveryPending)
        );
        assert!(pending_start.pause_active);

        let pending_state = pending_start.to_state();
        let window_minus_one_snapshot =
            sample_freshness_snapshot(Some(30.0), Some(30.0), "2026-04-06T00:00:49Z");
        let window_minus_one =
            evaluate_freshness_gate(&window_minus_one_snapshot, Some(&pending_state), &policy)
                .expect("window-1s should remain paused");
        assert_eq!(
            window_minus_one.transition,
            Some(FreshnessGateTransition::RecoveryPending)
        );
        assert!(window_minus_one.pause_active);

        let confirmed_snapshot =
            sample_freshness_snapshot(Some(29.0), Some(30.0), "2026-04-06T00:00:50Z");
        let confirmed = evaluate_freshness_gate(
            &confirmed_snapshot,
            Some(&window_minus_one.to_state()),
            &policy,
        )
        .expect("full recovery window should clear pause");
        assert_eq!(
            confirmed.transition,
            Some(FreshnessGateTransition::RecoveryConfirmed)
        );
        assert!(!confirmed.pause_active);
        assert!(!confirmed.block_new_order_creation);
        assert_eq!(
            confirmed.reason_code,
            FreshnessGateReasonCode::RecoveryConfirmed.code()
        );
    }

    #[test]
    fn freshness_gate_event_conversion_preserves_machine_readable_fields() {
        let policy = sample_freshness_policy();
        let snapshot = sample_freshness_snapshot(Some(31.0), Some(31.0), "2026-04-06T00:01:10Z");
        let outcome = evaluate_freshness_gate(&snapshot, None, &policy)
            .expect("stale breach should evaluate");
        let event = outcome
            .to_event("freshness::event-1", &policy)
            .expect("pause transition should emit an event payload");

        assert_eq!(event.transition, FreshnessGateTransition::PauseActivated);
        assert_eq!(
            event.reason_code,
            FreshnessGateReasonCode::StaleBreach.code()
        );
        assert!(event.pause_active);
        assert!(event.block_new_order_creation);
        assert!(validate_freshness_gate_event(&event).is_ok());
    }

    #[test]
    fn freshness_gate_repause_after_recovery_uses_new_breach_timestamp() {
        let policy = sample_freshness_policy();
        let paused_state = sample_paused_freshness_state();
        let pending_start_snapshot =
            sample_freshness_snapshot(Some(30.0), Some(30.0), "2026-04-06T00:00:40Z");
        let pending_start =
            evaluate_freshness_gate(&pending_start_snapshot, Some(&paused_state), &policy)
                .expect("fresh snapshot after pause should enter recovery_pending");
        let confirmed_snapshot =
            sample_freshness_snapshot(Some(30.0), Some(30.0), "2026-04-06T00:00:50Z");
        let confirmed = evaluate_freshness_gate(
            &confirmed_snapshot,
            Some(&pending_start.to_state()),
            &policy,
        )
        .expect("full recovery window should clear pause");
        assert_eq!(
            confirmed.transition,
            Some(FreshnessGateTransition::RecoveryConfirmed)
        );
        assert!(!confirmed.pause_active);

        let missing_signal_snapshot =
            sample_freshness_snapshot(Some(0.5), None, "2026-04-06T00:00:51Z");
        let repaused = evaluate_freshness_gate(
            &missing_signal_snapshot,
            Some(&confirmed.to_state()),
            &policy,
        )
        .expect("missing signal after recovery should re-activate pause");
        assert_eq!(
            repaused.transition,
            Some(FreshnessGateTransition::PauseActivated)
        );
        assert_eq!(
            repaused.stale_breach_detected_at_utc.as_deref(),
            Some("2026-04-06T00:00:51Z")
        );
        assert_eq!(
            repaused.pause_activated_at_utc.as_deref(),
            Some("2026-04-06T00:00:51Z")
        );
        assert!(
            repaused
                .breach_to_pause_latency_seconds
                .is_some_and(|latency| latency <= policy.max_breach_to_pause_seconds)
        );

        let event = repaused
            .to_event("freshness::event-2", &policy)
            .expect("pause transition should emit an event payload");
        assert!(validate_freshness_gate_event(&event).is_ok());
    }

    fn sample_pretrade_gate_result(
        gate: PreTradeGateDimension,
        passed: bool,
        reason_code: PreTradeReasonCode,
        evaluated_at_utc: &str,
    ) -> PreTradeGateResult {
        PreTradeGateResult {
            gate,
            passed,
            reason_code: reason_code.code().to_string(),
            evaluated_at_utc: evaluated_at_utc.to_string(),
        }
    }

    fn sample_fr41_input() -> ParticipationGuardrailEvaluationInput {
        ParticipationGuardrailEvaluationInput {
            market_id: "market_yes_no_1".to_string(),
            cluster_id: "cluster_alpha".to_string(),
            correlation_id: "corr-fr41-001".to_string(),
            observed_at_utc: "2026-04-06T00:00:00Z".to_string(),
            liquidity_depth_usd: Some(12_500.0),
            inactivity_gap_seconds: Some(300.0),
            normal_max_order_size_units: Some(40.0),
        }
    }

    #[test]
    fn fr41_participation_low_liquidity_threshold_is_strictly_less_than_10k() {
        let mut low_liquidity = sample_fr41_input();
        low_liquidity.liquidity_depth_usd = Some(9_999.99);
        let low_liquidity_outcome = evaluate_fr41_participation_guardrail(&low_liquidity)
            .expect("depth below 10k should trigger low-liquidity pause");
        assert_eq!(
            low_liquidity_outcome.guardrail_mode,
            ParticipationGuardrailMode::Pause
        );
        assert_eq!(
            low_liquidity_outcome.reason_code,
            ParticipationGuardrailReasonCode::LowLiquidityPause.code()
        );

        let mut boundary = sample_fr41_input();
        boundary.liquidity_depth_usd = Some(10_000.0);
        let boundary_outcome = evaluate_fr41_participation_guardrail(&boundary)
            .expect("depth equality boundary should remain pass");
        assert_eq!(
            boundary_outcome.guardrail_mode,
            ParticipationGuardrailMode::Pass
        );
        assert_eq!(
            boundary_outcome.reason_code,
            ParticipationGuardrailReasonCode::NoTrigger.code()
        );
    }

    #[test]
    fn fr41_participation_inactivity_pause_and_overnight_cap_boundaries_are_deterministic() {
        let mut at_15_min_boundary = sample_fr41_input();
        at_15_min_boundary.inactivity_gap_seconds = Some(900.0);
        let at_15_min_outcome = evaluate_fr41_participation_guardrail(&at_15_min_boundary)
            .expect("15 minute equality boundary should not trigger inactivity pause");
        assert_eq!(
            at_15_min_outcome.guardrail_mode,
            ParticipationGuardrailMode::Pass
        );

        let mut pause_window = sample_fr41_input();
        pause_window.inactivity_gap_seconds = Some(901.0);
        let pause_outcome = evaluate_fr41_participation_guardrail(&pause_window)
            .expect("inactivity above 15 minutes should pause participation");
        assert_eq!(
            pause_outcome.guardrail_mode,
            ParticipationGuardrailMode::Pause
        );
        assert_eq!(
            pause_outcome.reason_code,
            ParticipationGuardrailReasonCode::InactivityPause.code()
        );

        let mut at_4h_boundary = sample_fr41_input();
        at_4h_boundary.inactivity_gap_seconds = Some(14_400.0);
        let at_4h_outcome = evaluate_fr41_participation_guardrail(&at_4h_boundary)
            .expect("4h equality boundary should remain inactivity-pause window");
        assert_eq!(
            at_4h_outcome.guardrail_mode,
            ParticipationGuardrailMode::Pause
        );
        assert_eq!(
            at_4h_outcome.reason_code,
            ParticipationGuardrailReasonCode::InactivityPause.code()
        );

        let mut overnight = sample_fr41_input();
        overnight.inactivity_gap_seconds = Some(14_401.0);
        let overnight_outcome = evaluate_fr41_participation_guardrail(&overnight)
            .expect("inactivity above 4h should trigger 25% overnight size cap");
        assert_eq!(
            overnight_outcome.guardrail_mode,
            ParticipationGuardrailMode::SizeCap
        );
        assert_eq!(
            overnight_outcome.reason_code,
            ParticipationGuardrailReasonCode::OvernightSizeCapActive.code()
        );
        assert_eq!(overnight_outcome.normal_max_order_size_units, Some(40.0));
        assert_eq!(overnight_outcome.capped_max_order_size_units, Some(10.0));
    }

    #[test]
    fn fr41_participation_precedence_prefers_low_liquidity_pause_over_overnight_size_cap() {
        let mut input = sample_fr41_input();
        input.liquidity_depth_usd = Some(9_000.0);
        input.inactivity_gap_seconds = Some(15_000.0);
        let outcome = evaluate_fr41_participation_guardrail(&input)
            .expect("low-liquidity pause should supersede inactivity-derived cap");

        assert_eq!(outcome.guardrail_mode, ParticipationGuardrailMode::Pause);
        assert_eq!(
            outcome.reason_code,
            ParticipationGuardrailReasonCode::LowLiquidityPause.code()
        );
        assert_eq!(outcome.normal_max_order_size_units, None);
        assert_eq!(outcome.capped_max_order_size_units, None);
    }

    #[test]
    fn fr41_participation_fails_closed_when_overnight_baseline_is_missing() {
        let mut input = sample_fr41_input();
        input.inactivity_gap_seconds = Some(15_000.0);
        input.normal_max_order_size_units = None;

        let error = evaluate_fr41_participation_guardrail(&input)
            .expect_err("missing overnight baseline should fail closed");
        assert_eq!(
            error.code,
            ParticipationGuardrailReasonCode::DependencyUnavailable.code()
        );
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "normal_max_order_size_units")
        );
    }

    #[test]
    fn pretrade_reason_code_parse_round_trip_is_deterministic() {
        let codes = [
            PreTradeReasonCode::Pass,
            PreTradeReasonCode::FreshnessStaleBreach,
            PreTradeReasonCode::StreamHealthDegraded,
            PreTradeReasonCode::RiskLimitStateUnavailable,
            PreTradeReasonCode::StratificationStateUnavailable,
            PreTradeReasonCode::DrawdownStopTriggered,
            PreTradeReasonCode::VenueIneligible,
            PreTradeReasonCode::RewardRiskBelowThreshold,
            PreTradeReasonCode::ParticipationGuardrailUnavailable,
            PreTradeReasonCode::ParticipationGuardrailLowLiquidityPause,
            PreTradeReasonCode::ParticipationGuardrailInactivityPause,
            PreTradeReasonCode::ParticipationGuardrailOvernightCapExceeded,
        ];

        for code in codes {
            let parsed = PreTradeReasonCode::parse(code.code())
                .expect("known pre-trade reason code should parse");
            assert_eq!(parsed, code);
        }
    }

    #[test]
    fn pretrade_all_gates_pass_returns_allow_decision() {
        let gate_results = vec![
            sample_pretrade_gate_result(
                PreTradeGateDimension::Freshness,
                true,
                PreTradeReasonCode::Pass,
                "2026-04-06T00:00:00Z",
            ),
            sample_pretrade_gate_result(
                PreTradeGateDimension::StreamHealth,
                true,
                PreTradeReasonCode::Pass,
                "2026-04-06T00:00:00Z",
            ),
            sample_pretrade_gate_result(
                PreTradeGateDimension::ExposureLimitState,
                true,
                PreTradeReasonCode::Pass,
                "2026-04-06T00:00:00Z",
            ),
            sample_pretrade_gate_result(
                PreTradeGateDimension::DrawdownStop,
                true,
                PreTradeReasonCode::Pass,
                "2026-04-06T00:00:00Z",
            ),
            sample_pretrade_gate_result(
                PreTradeGateDimension::StrategyApproval,
                true,
                PreTradeReasonCode::Pass,
                "2026-04-06T00:00:00Z",
            ),
            sample_pretrade_gate_result(
                PreTradeGateDimension::VenueEligibility,
                true,
                PreTradeReasonCode::Pass,
                "2026-04-06T00:00:00Z",
            ),
        ];

        let decision = adjudicate_pretrade_gate_results(
            " intent-1 ",
            " market_yes_no_1 ",
            " cluster_alpha ",
            " default ",
            " corr-pretrade-1 ",
            "2026-04-06T00:00:00Z",
            gate_results,
            None,
            false,
        )
        .expect("all passing gates should produce allow decision");

        assert_eq!(decision.outcome, PreTradeDecisionOutcome::Allow);
        assert_eq!(decision.reason_code, PreTradeReasonCode::Pass.code());
        assert_eq!(decision.intent_id, "intent-1");
        assert_eq!(decision.market_id, "market_yes_no_1");
        assert_eq!(decision.cluster_id, "cluster_alpha");
        assert!(!decision.protective_mode_active);
        assert!(validate_pretrade_gate_decision(&decision).is_ok());
    }

    #[test]
    fn pretrade_single_gate_failure_returns_deterministic_deny_reason() {
        let gate_results = vec![
            sample_pretrade_gate_result(
                PreTradeGateDimension::Freshness,
                true,
                PreTradeReasonCode::Pass,
                "2026-04-06T00:00:01Z",
            ),
            sample_pretrade_gate_result(
                PreTradeGateDimension::StreamHealth,
                false,
                PreTradeReasonCode::StreamHealthDegraded,
                "2026-04-06T00:00:01Z",
            ),
            sample_pretrade_gate_result(
                PreTradeGateDimension::ExposureLimitState,
                true,
                PreTradeReasonCode::Pass,
                "2026-04-06T00:00:01Z",
            ),
        ];

        let decision = adjudicate_pretrade_gate_results(
            "intent-2",
            "market_yes_no_2",
            "cluster_beta",
            "default",
            "corr-pretrade-2",
            "2026-04-06T00:00:01Z",
            gate_results,
            None,
            false,
        )
        .expect("single failed gate should produce deny decision");

        assert_eq!(decision.outcome, PreTradeDecisionOutcome::Deny);
        assert_eq!(
            decision.reason_code,
            PreTradeReasonCode::StreamHealthDegraded.code()
        );
        assert_eq!(
            decision
                .gate_results
                .iter()
                .filter(|result| !result.passed)
                .count(),
            1
        );
        assert!(!decision.protective_mode_active);
        assert!(validate_pretrade_gate_decision(&decision).is_ok());
    }

    #[test]
    fn pretrade_drawdown_boundary_equality_triggers_protective_mode_deny() {
        let triggers = drawdown_stop_triggered(8.5, 8.5)
            .expect("drawdown boundary comparison should evaluate deterministically");
        assert!(triggers);

        let gate_results = vec![
            sample_pretrade_gate_result(
                PreTradeGateDimension::Freshness,
                true,
                PreTradeReasonCode::Pass,
                "2026-04-06T00:00:02Z",
            ),
            sample_pretrade_gate_result(
                PreTradeGateDimension::StreamHealth,
                true,
                PreTradeReasonCode::Pass,
                "2026-04-06T00:00:02Z",
            ),
            sample_pretrade_gate_result(
                PreTradeGateDimension::ExposureLimitState,
                true,
                PreTradeReasonCode::Pass,
                "2026-04-06T00:00:02Z",
            ),
            sample_pretrade_gate_result(
                PreTradeGateDimension::DrawdownStop,
                false,
                PreTradeReasonCode::DrawdownStopTriggered,
                "2026-04-06T00:00:02Z",
            ),
        ];

        let decision = adjudicate_pretrade_gate_results(
            "intent-3",
            "market_yes_no_3",
            "cluster_gamma",
            "default",
            "corr-pretrade-3",
            "2026-04-06T00:00:02Z",
            gate_results,
            None,
            true,
        )
        .expect("drawdown stop boundary should deny with protective mode");

        assert_eq!(decision.outcome, PreTradeDecisionOutcome::Deny);
        assert_eq!(
            decision.reason_code,
            PreTradeReasonCode::DrawdownStopTriggered.code()
        );
        assert!(decision.protective_mode_active);
        assert!(validate_pretrade_gate_decision(&decision).is_ok());
    }

    #[test]
    fn pretrade_protective_mode_requires_drawdown_reason() {
        let gate_results = vec![sample_pretrade_gate_result(
            PreTradeGateDimension::StreamHealth,
            false,
            PreTradeReasonCode::StreamHealthDegraded,
            "2026-04-06T00:00:03Z",
        )];

        let validation_error = adjudicate_pretrade_gate_results(
            "intent-4",
            "market_yes_no_4",
            "cluster_delta",
            "default",
            "corr-pretrade-4",
            "2026-04-06T00:00:03Z",
            gate_results,
            None,
            true,
        )
        .expect_err("protective mode must be restricted to drawdown-triggered denies");
        assert!(
            validation_error
                .field_errors
                .iter()
                .any(|issue| issue.field == "protective_mode_active")
        );
    }

    #[test]
    fn emergency_control_reason_code_parse_round_trip_is_deterministic() {
        let codes = [
            EmergencyControlReasonCode::PauseActivated,
            EmergencyControlReasonCode::ReduceOnlyActivated,
            EmergencyControlReasonCode::CancelAllAccepted,
            EmergencyControlReasonCode::StaleFeedTriggered,
            EmergencyControlReasonCode::ReconciliationCriticalTriggered,
            EmergencyControlReasonCode::ControlUncertaintyTriggered,
        ];

        for code in codes {
            let parsed = EmergencyControlReasonCode::parse(code.code())
                .expect("known emergency control reason should parse");
            assert_eq!(parsed, code);
        }
    }

    #[test]
    fn emergency_control_latency_boundaries_are_inclusive_and_deterministic() {
        let boundary = evaluate_emergency_control_latency_boundaries(1.0, 5.0)
            .expect("inclusive emergency boundaries should pass");
        assert!(boundary.acknowledgement_within_boundary);
        assert!(boundary.state_reflection_within_boundary);

        let exceeded = evaluate_emergency_control_latency_boundaries(1.01, 5.01)
            .expect("finite latency values should evaluate deterministically");
        assert!(!exceeded.acknowledgement_within_boundary);
        assert!(!exceeded.state_reflection_within_boundary);
    }

    #[test]
    fn emergency_control_stale_feed_trigger_threshold_is_strictly_greater_than_30s() {
        assert!(
            !stale_feed_triggers_safe_state(30.0)
                .expect("boundary threshold evaluation should succeed")
        );
        assert!(
            stale_feed_triggers_safe_state(30.0001)
                .expect("values above threshold should trigger safe-state")
        );
    }

    #[test]
    fn emergency_control_command_validation_requires_manual_actor_fields() {
        let error = validate_emergency_control_command(&EmergencyControlCommand {
            action_id: "emergency::001".to_string(),
            action: EmergencyControlAction::Pause,
            source: EmergencyControlSource::Manual,
            trigger_source: EmergencyControlTriggerSource::OperatorCommand,
            actor_id: None,
            actor_role: None,
            correlation_id: "corr-emergency-001".to_string(),
            requested_at_utc: "2026-04-06T00:00:00Z".to_string(),
        })
        .expect_err("manual controls without actor metadata must fail");
        assert_eq!(
            error.code,
            EmergencyControlReasonCode::InvalidPayload.code()
        );
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "actor_id")
        );
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "actor_role")
        );
    }

    #[test]
    fn emergency_control_action_record_validates_required_evidence_fields_and_latency() {
        let valid = SafetyControlActionRecord {
            action_id: "emergency::pause::001".to_string(),
            source: EmergencyControlSource::Manual,
            action: EmergencyControlAction::Pause,
            trigger_source: EmergencyControlTriggerSource::OperatorCommand,
            actor_id: Some("ops-1".to_string()),
            actor_role: Some("operational_control".to_string()),
            resulting_mode: EmergencyControlMode::Paused,
            reason_code: EmergencyControlReasonCode::PauseActivated
                .code()
                .to_string(),
            correlation_id: "corr-emergency-002".to_string(),
            audit_reference: "audit::emergency::001".to_string(),
            dedupe_key: "manual::pause::corr-emergency-002".to_string(),
            requested_at_utc: "2026-04-06T00:00:00Z".to_string(),
            acknowledged_at_utc: "2026-04-06T00:00:01Z".to_string(),
            effective_at_utc: "2026-04-06T00:00:05Z".to_string(),
            completed_at_utc: "2026-04-06T00:00:05Z".to_string(),
        };
        assert!(validate_safety_control_action_record(&valid).is_ok());

        let mut invalid = valid.clone();
        invalid.audit_reference = "   ".to_string();
        invalid.acknowledged_at_utc = "2026-04-06T00:00:02Z".to_string();
        let error = validate_safety_control_action_record(&invalid)
            .expect_err("missing evidence and boundary violations must fail");
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "audit_reference")
        );
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "acknowledged_at_utc")
        );
    }

    #[test]
    fn emergency_control_mode_compatibility_respects_reduce_only_vs_limit() {
        assert!(emergency_mode_allows_order_mode(
            EmergencyControlMode::Normal,
            crate::order::OrderMode::Limit,
        ));
        assert!(!emergency_mode_allows_order_mode(
            EmergencyControlMode::Paused,
            crate::order::OrderMode::ReduceOnly,
        ));
        assert!(!emergency_mode_allows_order_mode(
            EmergencyControlMode::ReduceOnly,
            crate::order::OrderMode::Limit,
        ));
        assert!(emergency_mode_allows_order_mode(
            EmergencyControlMode::ReduceOnly,
            crate::order::OrderMode::ReduceOnly,
        ));
    }
}
