use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AlertSeverity {
    Warning,
    Critical,
}

impl AlertSeverity {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Warning => "warning",
            Self::Critical => "critical",
        }
    }

    pub fn parse(value: &str) -> Result<Self, AlertContractError> {
        match value {
            "warning" => Ok(Self::Warning),
            "critical" => Ok(Self::Critical),
            _ => Err(AlertContractError::invalid_payload(format!(
                "unknown alert severity `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AlertDispatchStatus {
    Pending,
    Delivered,
    Failed,
}

impl AlertDispatchStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Delivered => "delivered",
            Self::Failed => "failed",
        }
    }

    pub fn parse(value: &str) -> Result<Self, AlertContractError> {
        match value {
            "pending" => Ok(Self::Pending),
            "delivered" => Ok(Self::Delivered),
            "failed" => Ok(Self::Failed),
            _ => Err(AlertContractError::invalid_payload(format!(
                "unknown alert dispatch status `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AlertDeliveryChannel {
    PagerDuty,
    Slack,
    Email,
}

impl AlertDeliveryChannel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PagerDuty => "pagerduty",
            Self::Slack => "slack",
            Self::Email => "email",
        }
    }

    pub fn parse(value: &str) -> Result<Self, AlertContractError> {
        match value {
            "pagerduty" => Ok(Self::PagerDuty),
            "slack" => Ok(Self::Slack),
            "email" => Ok(Self::Email),
            _ => Err(AlertContractError::invalid_payload(format!(
                "unknown alert delivery channel `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AlertDeliveryOutcome {
    Delivered,
    Failed,
}

impl AlertDeliveryOutcome {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Delivered => "delivered",
            Self::Failed => "failed",
        }
    }

    pub fn parse(value: &str) -> Result<Self, AlertContractError> {
        match value {
            "delivered" => Ok(Self::Delivered),
            "failed" => Ok(Self::Failed),
            _ => Err(AlertContractError::invalid_payload(format!(
                "unknown alert delivery outcome `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AlertReasonCode {
    Ready,
    NoTrigger,
    InvalidPayload,
    Unauthorized,
    DependencyUnavailable,
    StaleEvidence,
    DuplicateSuppressed,
    DrawdownLimitExceeded,
    StreamDisconnectExceeded,
    ReconciliationLagExceeded,
    StaleDataDetected,
    PolicyBypassAttempt,
    RegimeRebateDeltaExceeded,
    RegimeSpreadWideningExceeded,
    RegimeEligibilityTransition,
    DeliveryPrimaryFailed,
    DeliveryFallbackFailed,
    DeliverySlaBreached,
    AlphaHealthThresholdBreach,
}

impl AlertReasonCode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Ready => "alert_ready",
            Self::NoTrigger => "alert_no_trigger",
            Self::InvalidPayload => "alert_invalid_payload",
            Self::Unauthorized => "alert_unauthorized",
            Self::DependencyUnavailable => "alert_dependency_unavailable",
            Self::StaleEvidence => "alert_stale_evidence",
            Self::DuplicateSuppressed => "alert_duplicate_suppressed",
            Self::DrawdownLimitExceeded => "alert_drawdown_limit_exceeded",
            Self::StreamDisconnectExceeded => "alert_stream_disconnect_exceeded",
            Self::ReconciliationLagExceeded => "alert_reconciliation_lag_exceeded",
            Self::StaleDataDetected => "alert_stale_data_detected",
            Self::PolicyBypassAttempt => "alert_policy_bypass_attempt",
            Self::RegimeRebateDeltaExceeded => "alert_regime_rebate_delta_exceeded",
            Self::RegimeSpreadWideningExceeded => "alert_regime_spread_widening_exceeded",
            Self::RegimeEligibilityTransition => "alert_regime_eligibility_transition",
            Self::DeliveryPrimaryFailed => "alert_delivery_primary_failed",
            Self::DeliveryFallbackFailed => "alert_delivery_fallback_failed",
            Self::DeliverySlaBreached => "alert_delivery_sla_breached",
            Self::AlphaHealthThresholdBreach => "alert_alpha_health_threshold_breach",
        }
    }

    pub fn parse(value: &str) -> Result<Self, AlertContractError> {
        match value {
            "alert_ready" => Ok(Self::Ready),
            "alert_no_trigger" => Ok(Self::NoTrigger),
            "alert_invalid_payload" => Ok(Self::InvalidPayload),
            "alert_unauthorized" => Ok(Self::Unauthorized),
            "alert_dependency_unavailable" => Ok(Self::DependencyUnavailable),
            "alert_stale_evidence" => Ok(Self::StaleEvidence),
            "alert_duplicate_suppressed" => Ok(Self::DuplicateSuppressed),
            "alert_drawdown_limit_exceeded" => Ok(Self::DrawdownLimitExceeded),
            "alert_stream_disconnect_exceeded" => Ok(Self::StreamDisconnectExceeded),
            "alert_reconciliation_lag_exceeded" => Ok(Self::ReconciliationLagExceeded),
            "alert_stale_data_detected" => Ok(Self::StaleDataDetected),
            "alert_policy_bypass_attempt" => Ok(Self::PolicyBypassAttempt),
            "alert_regime_rebate_delta_exceeded" => Ok(Self::RegimeRebateDeltaExceeded),
            "alert_regime_spread_widening_exceeded" => Ok(Self::RegimeSpreadWideningExceeded),
            "alert_regime_eligibility_transition" => Ok(Self::RegimeEligibilityTransition),
            "alert_delivery_primary_failed" => Ok(Self::DeliveryPrimaryFailed),
            "alert_delivery_fallback_failed" => Ok(Self::DeliveryFallbackFailed),
            "alert_delivery_sla_breached" => Ok(Self::DeliverySlaBreached),
            "alert_alpha_health_threshold_breach" => Ok(Self::AlphaHealthThresholdBreach),
            _ => Err(AlertContractError::invalid_payload(format!(
                "unknown alert reason code `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AlertValidationIssue {
    pub field: &'static str,
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AlertContractError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<AlertValidationIssue>,
}

impl AlertContractError {
    pub fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: AlertReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn invalid_payload_with_issues(
        message: impl Into<String>,
        field_errors: Vec<AlertValidationIssue>,
    ) -> Self {
        Self {
            code: AlertReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            code: AlertReasonCode::Unauthorized.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn dependency_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: AlertReasonCode::DependencyUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn stale_evidence(message: impl Into<String>) -> Self {
        Self {
            code: AlertReasonCode::StaleEvidence.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn duplicate_suppressed(message: impl Into<String>) -> Self {
        Self {
            code: AlertReasonCode::DuplicateSuppressed.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlertTriggerInput {
    pub drawdown_pct_of_daily_limit: f64,
    pub stream_disconnect_seconds: i64,
    pub reconciliation_lag_seconds: i64,
    pub stale_data_detected: bool,
    pub policy_bypass_attempt: bool,
    pub correlation_id: String,
    pub observed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AlertTriggerDecision {
    pub severity: AlertSeverity,
    pub reason_code: AlertReasonCode,
    pub impacted_subsystem: String,
    pub cause: String,
    pub recommended_next_action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IncidentAlert {
    pub alert_id: String,
    pub severity: AlertSeverity,
    pub impacted_subsystem: String,
    pub cause: String,
    pub recommended_next_action: String,
    pub evidence_link: String,
    pub issued_at: String,
    pub correlation_id: String,
    pub reason_code: String,
    pub status: AlertDispatchStatus,
    pub delivered_at: Option<String>,
    pub failed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AlertDeliveryAttempt {
    pub alert_id: String,
    pub attempt_number: u16,
    pub channel: AlertDeliveryChannel,
    pub outcome: AlertDeliveryOutcome,
    pub reason_code: String,
    pub correlation_id: String,
    pub attempted_at: String,
    pub delivered_at: Option<String>,
    pub failed_at: Option<String>,
}

pub fn normalize_alert_identifier(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}

pub fn compose_alert_identifier(
    correlation_id: &str,
    reason_code: AlertReasonCode,
    issued_at: &str,
) -> Result<String, AlertContractError> {
    let normalized_correlation = normalize_alert_identifier(correlation_id);
    if !is_canonical_identifier(&normalized_correlation) {
        return Err(AlertContractError::invalid_payload_with_issues(
            "correlation_id must contain 3-120 canonical characters",
            vec![AlertValidationIssue {
                field: "correlation_id",
                code: AlertReasonCode::InvalidPayload.code(),
                message: "correlation_id must contain 3-120 canonical characters".to_string(),
            }],
        ));
    }
    let issued = parse_timestamp_or_error("issued_at", issued_at)?;
    let unix = issued.unix_timestamp();
    Ok(format!(
        "incident::alert::{}::{}::{}",
        reason_code.code(),
        normalized_correlation,
        unix
    ))
}

pub fn evaluate_fr29_trigger(
    input: &AlertTriggerInput,
) -> Result<Option<AlertTriggerDecision>, AlertContractError> {
    validate_trigger_input(input)?;

    if input.policy_bypass_attempt {
        return Ok(Some(AlertTriggerDecision {
            severity: AlertSeverity::Critical,
            reason_code: AlertReasonCode::PolicyBypassAttempt,
            impacted_subsystem: "pretrade-policy".to_string(),
            cause: "Policy bypass attempt detected on privileged mutation path.".to_string(),
            recommended_next_action:
                "Pause trading and review authorization + policy audit trail immediately."
                    .to_string(),
        }));
    }
    if input.drawdown_pct_of_daily_limit > 80.0 {
        return Ok(Some(AlertTriggerDecision {
            severity: AlertSeverity::Critical,
            reason_code: AlertReasonCode::DrawdownLimitExceeded,
            impacted_subsystem: "portfolio-risk".to_string(),
            cause: "Drawdown exceeded 80% of the configured daily loss limit.".to_string(),
            recommended_next_action:
                "Trigger reduce-only containment and verify portfolio risk-limit overrides."
                    .to_string(),
        }));
    }
    if input.stream_disconnect_seconds > 300 {
        return Ok(Some(AlertTriggerDecision {
            severity: AlertSeverity::Critical,
            reason_code: AlertReasonCode::StreamDisconnectExceeded,
            impacted_subsystem: "market-stream".to_string(),
            cause: "Market stream disconnect exceeded 5 minutes.".to_string(),
            recommended_next_action:
                "Pause submissions and validate stream recovery before resuming control actions."
                    .to_string(),
        }));
    }
    if input.stale_data_detected {
        return Ok(Some(AlertTriggerDecision {
            severity: AlertSeverity::Critical,
            reason_code: AlertReasonCode::StaleDataDetected,
            impacted_subsystem: "freshness-gate".to_string(),
            cause: "Freshness gate detected stale market or user data feed evidence.".to_string(),
            recommended_next_action:
                "Keep containment active and restore fresh feed evidence before continuing."
                    .to_string(),
        }));
    }
    if input.reconciliation_lag_seconds > 60 {
        return Ok(Some(AlertTriggerDecision {
            severity: AlertSeverity::Warning,
            reason_code: AlertReasonCode::ReconciliationLagExceeded,
            impacted_subsystem: "reconciliation".to_string(),
            cause: "Reconciliation lag exceeded 60 seconds.".to_string(),
            recommended_next_action:
                "Inspect reconciliation backlog and compare lifecycle drift before escalation."
                    .to_string(),
        }));
    }

    Ok(None)
}

pub fn build_incident_alert(
    trigger: &AlertTriggerDecision,
    alert_id: &str,
    issued_at: &str,
    correlation_id: &str,
    evidence_link: &str,
    recommended_next_action: Option<&str>,
) -> Result<IncidentAlert, AlertContractError> {
    let normalized_alert_id = normalize_alert_identifier(alert_id);
    if !is_canonical_identifier(&normalized_alert_id) {
        return Err(AlertContractError::invalid_payload_with_issues(
            "alert_id must contain 3-120 canonical characters",
            vec![AlertValidationIssue {
                field: "alert_id",
                code: AlertReasonCode::InvalidPayload.code(),
                message: "alert_id must contain 3-120 canonical characters".to_string(),
            }],
        ));
    }

    let normalized_correlation_id = normalize_alert_identifier(correlation_id);
    if !is_canonical_identifier(&normalized_correlation_id) {
        return Err(AlertContractError::invalid_payload_with_issues(
            "correlation_id must contain 3-120 canonical characters",
            vec![AlertValidationIssue {
                field: "correlation_id",
                code: AlertReasonCode::InvalidPayload.code(),
                message: "correlation_id must contain 3-120 canonical characters".to_string(),
            }],
        ));
    }

    parse_timestamp_or_error("issued_at", issued_at)?;
    validate_evidence_link(evidence_link)?;

    let recommended_next_action = recommended_next_action
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(trigger.recommended_next_action.as_str())
        .to_string();
    if recommended_next_action.trim().is_empty() {
        return Err(AlertContractError::invalid_payload_with_issues(
            "recommended_next_action cannot be blank",
            vec![AlertValidationIssue {
                field: "recommended_next_action",
                code: AlertReasonCode::InvalidPayload.code(),
                message: "recommended_next_action cannot be blank".to_string(),
            }],
        ));
    }

    let alert = IncidentAlert {
        alert_id: normalized_alert_id,
        severity: trigger.severity,
        impacted_subsystem: trigger.impacted_subsystem.trim().to_string(),
        cause: trigger.cause.trim().to_string(),
        recommended_next_action,
        evidence_link: evidence_link.trim().to_string(),
        issued_at: issued_at.trim().to_string(),
        correlation_id: normalized_correlation_id,
        reason_code: trigger.reason_code.code().to_string(),
        status: AlertDispatchStatus::Pending,
        delivered_at: None,
        failed_at: None,
    };
    validate_incident_alert(&alert)?;
    Ok(alert)
}

pub fn should_emit_alert(
    prior_alerts: &[IncidentAlert],
    candidate_reason_code: &str,
    correlation_id: &str,
    issued_at: &str,
    dedupe_window_seconds: i64,
) -> Result<bool, AlertContractError> {
    if dedupe_window_seconds < 0 {
        return Err(AlertContractError::invalid_payload_with_issues(
            "dedupe_window_seconds must be >= 0",
            vec![AlertValidationIssue {
                field: "dedupe_window_seconds",
                code: AlertReasonCode::InvalidPayload.code(),
                message: "dedupe_window_seconds must be >= 0".to_string(),
            }],
        ));
    }
    let normalized_correlation = normalize_alert_identifier(correlation_id);
    if !is_canonical_identifier(&normalized_correlation) {
        return Err(AlertContractError::invalid_payload_with_issues(
            "correlation_id must contain 3-120 canonical characters",
            vec![AlertValidationIssue {
                field: "correlation_id",
                code: AlertReasonCode::InvalidPayload.code(),
                message: "correlation_id must contain 3-120 canonical characters".to_string(),
            }],
        ));
    }

    let issued = parse_timestamp_or_error("issued_at", issued_at)?;
    let reason = AlertReasonCode::parse(candidate_reason_code)?;
    let dedupe_window = Duration::seconds(dedupe_window_seconds);
    for alert in prior_alerts {
        if normalize_alert_identifier(&alert.correlation_id) != normalized_correlation {
            continue;
        }
        if AlertReasonCode::parse(&alert.reason_code)? != reason {
            continue;
        }
        let prior_issued = parse_timestamp_or_error("issued_at", &alert.issued_at)?;
        let distance = if issued >= prior_issued {
            issued - prior_issued
        } else {
            prior_issued - issued
        };
        if distance <= dedupe_window {
            return Ok(false);
        }
    }
    Ok(true)
}

pub fn validate_incident_alert(alert: &IncidentAlert) -> Result<(), AlertContractError> {
    let mut field_errors = Vec::new();
    validate_non_empty(&mut field_errors, "alert_id", &alert.alert_id);
    validate_non_empty(
        &mut field_errors,
        "impacted_subsystem",
        &alert.impacted_subsystem,
    );
    validate_non_empty(&mut field_errors, "cause", &alert.cause);
    validate_non_empty(
        &mut field_errors,
        "recommended_next_action",
        &alert.recommended_next_action,
    );
    validate_non_empty(&mut field_errors, "reason_code", &alert.reason_code);
    validate_non_empty(&mut field_errors, "correlation_id", &alert.correlation_id);

    validate_canonical_identifier(&mut field_errors, "alert_id", &alert.alert_id);
    validate_canonical_identifier(&mut field_errors, "correlation_id", &alert.correlation_id);

    validate_timestamp_utc(&mut field_errors, "issued_at", &alert.issued_at);
    if let Some(delivered_at) = alert.delivered_at.as_deref() {
        validate_timestamp_utc(&mut field_errors, "delivered_at", delivered_at);
    }
    if let Some(failed_at) = alert.failed_at.as_deref() {
        validate_timestamp_utc(&mut field_errors, "failed_at", failed_at);
    }

    if AlertReasonCode::parse(&alert.reason_code).is_err() {
        field_errors.push(AlertValidationIssue {
            field: "reason_code",
            code: AlertReasonCode::InvalidPayload.code(),
            message: "reason_code is not a supported alert reason code".to_string(),
        });
    }

    if let Err(error) = validate_evidence_link(alert.evidence_link.as_str()) {
        field_errors.extend(error.field_errors);
    }

    match alert.status {
        AlertDispatchStatus::Pending => {
            if alert.delivered_at.is_some() || alert.failed_at.is_some() {
                field_errors.push(AlertValidationIssue {
                    field: "status",
                    code: AlertReasonCode::InvalidPayload.code(),
                    message: "pending alerts must not set delivered_at or failed_at".to_string(),
                });
            }
        }
        AlertDispatchStatus::Delivered => {
            if alert.delivered_at.is_none() || alert.failed_at.is_some() {
                field_errors.push(AlertValidationIssue {
                    field: "status",
                    code: AlertReasonCode::InvalidPayload.code(),
                    message: "delivered alerts require delivered_at and must not set failed_at"
                        .to_string(),
                });
            }
        }
        AlertDispatchStatus::Failed => {
            if alert.failed_at.is_none() || alert.delivered_at.is_some() {
                field_errors.push(AlertValidationIssue {
                    field: "status",
                    code: AlertReasonCode::InvalidPayload.code(),
                    message: "failed alerts require failed_at and must not set delivered_at"
                        .to_string(),
                });
            }
        }
    }

    if field_errors.is_empty() {
        return Ok(());
    }
    Err(AlertContractError::invalid_payload_with_issues(
        "incident alert failed validation",
        field_errors,
    ))
}

pub fn validate_alert_delivery_attempt(
    attempt: &AlertDeliveryAttempt,
) -> Result<(), AlertContractError> {
    let mut field_errors = Vec::new();
    if attempt.attempt_number == 0 {
        field_errors.push(AlertValidationIssue {
            field: "attempt_number",
            code: AlertReasonCode::InvalidPayload.code(),
            message: "attempt_number must be greater than 0".to_string(),
        });
    }

    validate_non_empty(&mut field_errors, "alert_id", &attempt.alert_id);
    validate_non_empty(&mut field_errors, "reason_code", &attempt.reason_code);
    validate_non_empty(&mut field_errors, "correlation_id", &attempt.correlation_id);
    validate_canonical_identifier(&mut field_errors, "alert_id", &attempt.alert_id);
    validate_canonical_identifier(&mut field_errors, "correlation_id", &attempt.correlation_id);

    validate_timestamp_utc(&mut field_errors, "attempted_at", &attempt.attempted_at);
    if let Some(delivered_at) = attempt.delivered_at.as_deref() {
        validate_timestamp_utc(&mut field_errors, "delivered_at", delivered_at);
    }
    if let Some(failed_at) = attempt.failed_at.as_deref() {
        validate_timestamp_utc(&mut field_errors, "failed_at", failed_at);
    }

    if AlertReasonCode::parse(&attempt.reason_code).is_err() {
        field_errors.push(AlertValidationIssue {
            field: "reason_code",
            code: AlertReasonCode::InvalidPayload.code(),
            message: "reason_code is not a supported alert reason code".to_string(),
        });
    }

    match attempt.outcome {
        AlertDeliveryOutcome::Delivered => {
            if attempt.delivered_at.is_none() || attempt.failed_at.is_some() {
                field_errors.push(AlertValidationIssue {
                    field: "outcome",
                    code: AlertReasonCode::InvalidPayload.code(),
                    message: "delivered attempt requires delivered_at and no failed_at".to_string(),
                });
            }
        }
        AlertDeliveryOutcome::Failed => {
            if attempt.failed_at.is_none() || attempt.delivered_at.is_some() {
                field_errors.push(AlertValidationIssue {
                    field: "outcome",
                    code: AlertReasonCode::InvalidPayload.code(),
                    message: "failed attempt requires failed_at and no delivered_at".to_string(),
                });
            }
        }
    }

    if field_errors.is_empty() {
        return Ok(());
    }
    Err(AlertContractError::invalid_payload_with_issues(
        "alert delivery attempt failed validation",
        field_errors,
    ))
}

pub fn critical_dispatch_within_sla(
    alert: &IncidentAlert,
    delivered_at: &str,
    target_seconds: i64,
) -> Result<bool, AlertContractError> {
    if target_seconds <= 0 {
        return Err(AlertContractError::invalid_payload_with_issues(
            "target_seconds must be greater than 0",
            vec![AlertValidationIssue {
                field: "target_seconds",
                code: AlertReasonCode::InvalidPayload.code(),
                message: "target_seconds must be greater than 0".to_string(),
            }],
        ));
    }
    if alert.severity != AlertSeverity::Critical {
        return Ok(true);
    }
    let issued = parse_timestamp_or_error("issued_at", &alert.issued_at)?;
    let delivered = parse_timestamp_or_error("delivered_at", delivered_at)?;
    if delivered < issued {
        return Err(AlertContractError::invalid_payload_with_issues(
            "delivered_at must be >= issued_at",
            vec![AlertValidationIssue {
                field: "delivered_at",
                code: AlertReasonCode::InvalidPayload.code(),
                message: "delivered_at must be >= issued_at".to_string(),
            }],
        ));
    }
    Ok((delivered - issued) <= Duration::seconds(target_seconds))
}

pub fn validate_evidence_link(link: &str) -> Result<(), AlertContractError> {
    let normalized = link.trim();
    if normalized.is_empty() {
        return Err(AlertContractError::invalid_payload_with_issues(
            "evidence_link cannot be blank",
            vec![AlertValidationIssue {
                field: "evidence_link",
                code: AlertReasonCode::InvalidPayload.code(),
                message: "evidence_link cannot be blank".to_string(),
            }],
        ));
    }
    if normalized.contains(char::is_whitespace)
        || !(normalized.starts_with("https://") || normalized.starts_with("http://"))
    {
        return Err(AlertContractError::invalid_payload_with_issues(
            "evidence_link must be an absolute http(s) URL",
            vec![AlertValidationIssue {
                field: "evidence_link",
                code: AlertReasonCode::InvalidPayload.code(),
                message: "evidence_link must be an absolute http(s) URL".to_string(),
            }],
        ));
    }
    Ok(())
}

fn validate_trigger_input(input: &AlertTriggerInput) -> Result<(), AlertContractError> {
    let mut field_errors = Vec::new();
    if !input.drawdown_pct_of_daily_limit.is_finite() {
        field_errors.push(AlertValidationIssue {
            field: "drawdown_pct_of_daily_limit",
            code: AlertReasonCode::InvalidPayload.code(),
            message: "drawdown_pct_of_daily_limit must be finite".to_string(),
        });
    }
    if input.stream_disconnect_seconds < 0 {
        field_errors.push(AlertValidationIssue {
            field: "stream_disconnect_seconds",
            code: AlertReasonCode::InvalidPayload.code(),
            message: "stream_disconnect_seconds must be >= 0".to_string(),
        });
    }
    if input.reconciliation_lag_seconds < 0 {
        field_errors.push(AlertValidationIssue {
            field: "reconciliation_lag_seconds",
            code: AlertReasonCode::InvalidPayload.code(),
            message: "reconciliation_lag_seconds must be >= 0".to_string(),
        });
    }

    let normalized_correlation = normalize_alert_identifier(&input.correlation_id);
    if !is_canonical_identifier(&normalized_correlation) {
        field_errors.push(AlertValidationIssue {
            field: "correlation_id",
            code: AlertReasonCode::InvalidPayload.code(),
            message: "correlation_id must contain 3-120 canonical characters".to_string(),
        });
    }
    if parse_timestamp(&input.observed_at).is_err() {
        field_errors.push(AlertValidationIssue {
            field: "observed_at",
            code: AlertReasonCode::InvalidPayload.code(),
            message: "observed_at must be an RFC3339 UTC timestamp".to_string(),
        });
    }
    if field_errors.is_empty() {
        return Ok(());
    }
    Err(AlertContractError::invalid_payload_with_issues(
        "alert trigger payload failed validation",
        field_errors,
    ))
}

fn validate_non_empty(
    field_errors: &mut Vec<AlertValidationIssue>,
    field: &'static str,
    value: &str,
) {
    if value.trim().is_empty() {
        field_errors.push(AlertValidationIssue {
            field,
            code: AlertReasonCode::InvalidPayload.code(),
            message: format!("{field} cannot be blank"),
        });
    }
}

fn validate_canonical_identifier(
    field_errors: &mut Vec<AlertValidationIssue>,
    field: &'static str,
    value: &str,
) {
    if !is_canonical_identifier(value) {
        field_errors.push(AlertValidationIssue {
            field,
            code: AlertReasonCode::InvalidPayload.code(),
            message: format!("{field} must contain 3-120 canonical characters"),
        });
    }
}

fn validate_timestamp_utc(
    field_errors: &mut Vec<AlertValidationIssue>,
    field: &'static str,
    value: &str,
) {
    if parse_timestamp(value).is_err() {
        field_errors.push(AlertValidationIssue {
            field,
            code: AlertReasonCode::InvalidPayload.code(),
            message: format!("{field} must be an RFC3339 UTC timestamp"),
        });
    }
}

fn is_canonical_identifier(value: &str) -> bool {
    let normalized = normalize_alert_identifier(value);
    (3..=120).contains(&normalized.len())
        && normalized.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || "._:-".contains(character)
        })
}

fn parse_timestamp(value: &str) -> Result<OffsetDateTime, ()> {
    let parsed = OffsetDateTime::parse(value.trim(), &Rfc3339).map_err(|_| ())?;
    if parsed.offset() != UtcOffset::UTC {
        return Err(());
    }
    Ok(parsed)
}

fn parse_timestamp_or_error(
    field: &'static str,
    value: &str,
) -> Result<OffsetDateTime, AlertContractError> {
    parse_timestamp(value).map_err(|_| {
        AlertContractError::invalid_payload_with_issues(
            format!("{field} must be an RFC3339 UTC timestamp"),
            vec![AlertValidationIssue {
                field,
                code: AlertReasonCode::InvalidPayload.code(),
                message: format!("{field} must be an RFC3339 UTC timestamp"),
            }],
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_trigger_input() -> AlertTriggerInput {
        AlertTriggerInput {
            drawdown_pct_of_daily_limit: 0.0,
            stream_disconnect_seconds: 0,
            reconciliation_lag_seconds: 0,
            stale_data_detected: false,
            policy_bypass_attempt: false,
            correlation_id: "corr-alert-001".to_string(),
            observed_at: "2026-04-06T16:00:00Z".to_string(),
        }
    }

    fn sample_trigger_decision() -> AlertTriggerDecision {
        AlertTriggerDecision {
            severity: AlertSeverity::Critical,
            reason_code: AlertReasonCode::DrawdownLimitExceeded,
            impacted_subsystem: "portfolio-risk".to_string(),
            cause: "Drawdown exceeded 80% of daily risk limit.".to_string(),
            recommended_next_action: "Trigger reduce-only mode and review risk limit overrides."
                .to_string(),
        }
    }

    fn sample_alert(status: AlertDispatchStatus) -> IncidentAlert {
        IncidentAlert {
            alert_id: "incident::alert::alert_drawdown_limit_exceeded::corr-alert-001::1"
                .to_string(),
            severity: AlertSeverity::Critical,
            impacted_subsystem: "portfolio-risk".to_string(),
            cause: "Drawdown exceeded 80% of daily risk limit.".to_string(),
            recommended_next_action: "Trigger reduce-only mode and review risk limit overrides."
                .to_string(),
            evidence_link: "https://docs.example.com/operations/severity-alert-delivery#drawdown"
                .to_string(),
            issued_at: "2026-04-06T16:00:00Z".to_string(),
            correlation_id: "corr-alert-001".to_string(),
            reason_code: AlertReasonCode::DrawdownLimitExceeded.code().to_string(),
            status,
            delivered_at: None,
            failed_at: None,
        }
    }

    #[test]
    fn alert_reason_code_parse_accepts_fr29_fr40_and_delivery_codes() {
        assert_eq!(
            AlertReasonCode::parse("alert_policy_bypass_attempt")
                .expect("policy bypass reason should parse"),
            AlertReasonCode::PolicyBypassAttempt
        );
        assert_eq!(
            AlertReasonCode::parse("alert_regime_rebate_delta_exceeded")
                .expect("FR40 rebate shift reason should parse"),
            AlertReasonCode::RegimeRebateDeltaExceeded
        );
        assert_eq!(
            AlertReasonCode::parse("alert_regime_spread_widening_exceeded")
                .expect("FR40 spread shift reason should parse"),
            AlertReasonCode::RegimeSpreadWideningExceeded
        );
        assert_eq!(
            AlertReasonCode::parse("alert_regime_eligibility_transition")
                .expect("FR40 eligibility transition reason should parse"),
            AlertReasonCode::RegimeEligibilityTransition
        );
        assert_eq!(
            AlertReasonCode::parse("alert_delivery_primary_failed")
                .expect("primary delivery fail should parse"),
            AlertReasonCode::DeliveryPrimaryFailed
        );
        assert_eq!(
            AlertReasonCode::parse("alert_alpha_health_threshold_breach")
                .expect("alpha-health threshold breach reason should parse"),
            AlertReasonCode::AlphaHealthThresholdBreach
        );
        assert_eq!(
            AlertReasonCode::parse("alert_unknown")
                .expect_err("unknown reason should fail")
                .code,
            AlertReasonCode::InvalidPayload.code()
        );
    }

    #[test]
    fn fr29_trigger_boundaries_use_strict_greater_than_semantics() {
        let mut input = sample_trigger_input();

        input.drawdown_pct_of_daily_limit = 80.0;
        assert!(
            evaluate_fr29_trigger(&input)
                .expect("boundary evaluation should succeed")
                .is_none()
        );
        input.drawdown_pct_of_daily_limit = 80.0001;
        assert_eq!(
            evaluate_fr29_trigger(&input)
                .expect("drawdown breach should trigger")
                .expect("decision should exist")
                .reason_code,
            AlertReasonCode::DrawdownLimitExceeded
        );

        input = sample_trigger_input();
        input.stream_disconnect_seconds = 300;
        assert!(
            evaluate_fr29_trigger(&input)
                .expect("boundary evaluation should succeed")
                .is_none()
        );
        input.stream_disconnect_seconds = 301;
        assert_eq!(
            evaluate_fr29_trigger(&input)
                .expect("stream disconnect breach should trigger")
                .expect("decision should exist")
                .reason_code,
            AlertReasonCode::StreamDisconnectExceeded
        );

        input = sample_trigger_input();
        input.reconciliation_lag_seconds = 60;
        assert!(
            evaluate_fr29_trigger(&input)
                .expect("boundary evaluation should succeed")
                .is_none()
        );
        input.reconciliation_lag_seconds = 61;
        assert_eq!(
            evaluate_fr29_trigger(&input)
                .expect("reconciliation lag breach should trigger")
                .expect("decision should exist")
                .reason_code,
            AlertReasonCode::ReconciliationLagExceeded
        );
    }

    #[test]
    fn fr29_trigger_priority_prefers_policy_bypass_when_multiple_signals_present() {
        let mut input = sample_trigger_input();
        input.policy_bypass_attempt = true;
        input.drawdown_pct_of_daily_limit = 90.0;
        input.stream_disconnect_seconds = 420;
        let decision = evaluate_fr29_trigger(&input)
            .expect("trigger evaluation should succeed")
            .expect("decision should exist");
        assert_eq!(decision.reason_code, AlertReasonCode::PolicyBypassAttempt);
        assert_eq!(decision.severity, AlertSeverity::Critical);
    }

    #[test]
    fn build_incident_alert_rejects_malformed_evidence_link() {
        let error = build_incident_alert(
            &sample_trigger_decision(),
            "incident::alert::drawdown::001",
            "2026-04-06T16:00:00Z",
            "corr-alert-001",
            "docs.local/runbook",
            None,
        )
        .expect_err("non-http evidence link should fail");
        assert_eq!(error.code, AlertReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "evidence_link")
        );
    }

    #[test]
    fn dedupe_suppresses_same_reason_code_within_window() {
        let prior = vec![sample_alert(AlertDispatchStatus::Delivered)];
        let should_emit = should_emit_alert(
            &prior,
            AlertReasonCode::DrawdownLimitExceeded.code(),
            "corr-alert-001",
            "2026-04-06T16:00:20Z",
            60,
        )
        .expect("dedupe evaluation should succeed");
        assert!(!should_emit);
    }

    #[test]
    fn dedupe_allows_new_alert_outside_window_or_different_reason() {
        let prior = vec![sample_alert(AlertDispatchStatus::Delivered)];
        let outside_window = should_emit_alert(
            &prior,
            AlertReasonCode::DrawdownLimitExceeded.code(),
            "corr-alert-001",
            "2026-04-06T16:02:10Z",
            60,
        )
        .expect("dedupe evaluation should succeed");
        assert!(outside_window);

        let different_reason = should_emit_alert(
            &prior,
            AlertReasonCode::PolicyBypassAttempt.code(),
            "corr-alert-001",
            "2026-04-06T16:00:20Z",
            60,
        )
        .expect("dedupe evaluation should succeed");
        assert!(different_reason);
    }

    #[test]
    fn critical_dispatch_sla_enforces_thirty_second_budget() {
        let alert = sample_alert(AlertDispatchStatus::Pending);
        assert!(
            critical_dispatch_within_sla(&alert, "2026-04-06T16:00:30Z", 30)
                .expect("SLA evaluation should succeed")
        );
        assert!(
            !critical_dispatch_within_sla(&alert, "2026-04-06T16:00:31Z", 30)
                .expect("SLA evaluation should succeed")
        );
    }

    #[test]
    fn alert_validation_enforces_status_timestamp_contract() {
        let mut alert = sample_alert(AlertDispatchStatus::Delivered);
        let error = validate_incident_alert(&alert).expect_err("delivered alert missing timestamp");
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "status")
        );

        alert.delivered_at = Some("2026-04-06T16:00:05Z".to_string());
        assert!(validate_incident_alert(&alert).is_ok());
    }

    #[test]
    fn alert_delivery_attempt_validation_requires_outcome_timestamps() {
        let attempt = AlertDeliveryAttempt {
            alert_id: "incident::alert::drawdown::001".to_string(),
            attempt_number: 1,
            channel: AlertDeliveryChannel::PagerDuty,
            outcome: AlertDeliveryOutcome::Failed,
            reason_code: AlertReasonCode::DeliveryPrimaryFailed.code().to_string(),
            correlation_id: "corr-alert-001".to_string(),
            attempted_at: "2026-04-06T16:00:01Z".to_string(),
            delivered_at: None,
            failed_at: None,
        };

        let error = validate_alert_delivery_attempt(&attempt)
            .expect_err("failed attempts require failed_at");
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "outcome")
        );
    }

    #[test]
    fn compose_alert_identifier_is_deterministic_and_canonical() {
        let alert_id = compose_alert_identifier(
            "Corr-Alert-001",
            AlertReasonCode::DrawdownLimitExceeded,
            "2026-04-06T16:00:00Z",
        )
        .expect("identifier composition should succeed");
        assert!(
            alert_id
                .starts_with("incident::alert::alert_drawdown_limit_exceeded::corr-alert-001::")
        );
    }
}
