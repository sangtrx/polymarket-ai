use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

const MARKET_EXPOSURE_MIN_PCT_NAV: f64 = 0.0;
const MARKET_EXPOSURE_MAX_PCT_NAV: f64 = 100.0;
pub const MARKET_STREAM_LATENCY_TARGET_SECONDS: f64 = 2.0;
pub const MARKET_STREAM_BACKLOG_WARNING_SECONDS: f64 = 5.0;
pub const MARKET_STREAM_BACKLOG_DEGRADED_SECONDS: f64 = 10.0;
pub const MARKET_STREAM_BACKLOG_SUSTAINED_DURATION_SECONDS: f64 = 30.0;
pub const MARKET_STREAM_MAX_DEPTH_LEVELS: usize = 5;

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
        projected_exposure_pct_nav,
        observed_at_utc: tick.observed_at_utc.clone(),
    })
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
}
