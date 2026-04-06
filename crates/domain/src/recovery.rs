use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

use crate::reconciliation::RECONCILIATION_CRITICAL_MISMATCH_THRESHOLD;
use crate::risk::{
    FRESHNESS_STALE_THRESHOLD_SECONDS, InventoryLimitRule, RiskLimitContractError,
    RiskLimitProfileVersion, RiskScopeLimit, normalize_risk_limit_identifier,
    validate_inventory_limit_rule, validate_risk_limit_profile_version,
};

const CHECKSUM_HEX_LEN: usize = 64;

pub const RECOVERY_FRESHNESS_MAX_AGE_SECONDS: f64 = FRESHNESS_STALE_THRESHOLD_SECONDS;
pub const RECOVERY_RECONCILIATION_MAX_MISMATCH_RATE_EXCLUSIVE: f64 =
    RECONCILIATION_CRITICAL_MISMATCH_THRESHOLD;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryReadinessStatus {
    Approved,
    Blocked,
}

impl RecoveryReadinessStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Approved => "approved",
            Self::Blocked => "blocked",
        }
    }

    pub fn parse(value: &str) -> Result<Self, RecoveryContractError> {
        match value {
            "approved" => Ok(Self::Approved),
            "blocked" => Ok(Self::Blocked),
            _ => Err(RecoveryContractError::invalid_payload(format!(
                "unknown recovery readiness status `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryGateName {
    Freshness,
    Reconciliation,
    RiskChecksum,
    OperatorSignoff,
}

impl RecoveryGateName {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Freshness => "freshness",
            Self::Reconciliation => "reconciliation",
            Self::RiskChecksum => "risk_checksum",
            Self::OperatorSignoff => "operator_signoff",
        }
    }

    pub fn parse(value: &str) -> Result<Self, RecoveryContractError> {
        match value {
            "freshness" => Ok(Self::Freshness),
            "reconciliation" => Ok(Self::Reconciliation),
            "risk_checksum" => Ok(Self::RiskChecksum),
            "operator_signoff" => Ok(Self::OperatorSignoff),
            _ => Err(RecoveryContractError::invalid_payload(format!(
                "unknown recovery gate `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryReasonCode {
    ResumeApproved,
    ResumeBlocked,
    FreshnessPass,
    FreshnessStale,
    ReconciliationPass,
    ReconciliationMismatch,
    ChecksumMatch,
    ChecksumMismatch,
    SignoffRecorded,
    SignoffMissing,
    InvalidPayload,
    DependencyUnavailable,
    Unauthorized,
    PersistenceUnavailable,
    NotFound,
    StaleEvidence,
    RehearsalSuccess,
    RehearsalChecksumMismatch,
    RehearsalReconciliationSanityFailure,
    RehearsalDeterministicReplayMismatch,
    RehearsalSignatureContractError,
    RehearsalMissingOrFailed,
}

impl RecoveryReasonCode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::ResumeApproved => "recovery_resume_approved",
            Self::ResumeBlocked => "recovery_resume_blocked",
            Self::FreshnessPass => "recovery_freshness_pass",
            Self::FreshnessStale => "recovery_freshness_stale",
            Self::ReconciliationPass => "recovery_reconciliation_pass",
            Self::ReconciliationMismatch => "recovery_reconciliation_mismatch",
            Self::ChecksumMatch => "recovery_checksum_match",
            Self::ChecksumMismatch => "recovery_checksum_mismatch",
            Self::SignoffRecorded => "recovery_signoff_recorded",
            Self::SignoffMissing => "recovery_signoff_missing",
            Self::InvalidPayload => "recovery_invalid_payload",
            Self::DependencyUnavailable => "recovery_dependency_unavailable",
            Self::Unauthorized => "recovery_unauthorized",
            Self::PersistenceUnavailable => "recovery_persistence_unavailable",
            Self::NotFound => "recovery_not_found",
            Self::StaleEvidence => "recovery_stale_evidence",
            Self::RehearsalSuccess => "recovery_rehearsal_success",
            Self::RehearsalChecksumMismatch => "recovery_rehearsal_checksum_mismatch",
            Self::RehearsalReconciliationSanityFailure => {
                "recovery_rehearsal_reconciliation_sanity_failure"
            }
            Self::RehearsalDeterministicReplayMismatch => {
                "recovery_rehearsal_deterministic_replay_mismatch"
            }
            Self::RehearsalSignatureContractError => "recovery_rehearsal_signature_contract_error",
            Self::RehearsalMissingOrFailed => "recovery_rehearsal_missing_or_failed",
        }
    }

    pub fn parse(value: &str) -> Result<Self, RecoveryContractError> {
        match value {
            "recovery_resume_approved" => Ok(Self::ResumeApproved),
            "recovery_resume_blocked" => Ok(Self::ResumeBlocked),
            "recovery_freshness_pass" => Ok(Self::FreshnessPass),
            "recovery_freshness_stale" => Ok(Self::FreshnessStale),
            "recovery_reconciliation_pass" => Ok(Self::ReconciliationPass),
            "recovery_reconciliation_mismatch" => Ok(Self::ReconciliationMismatch),
            "recovery_checksum_match" => Ok(Self::ChecksumMatch),
            "recovery_checksum_mismatch" => Ok(Self::ChecksumMismatch),
            "recovery_signoff_recorded" => Ok(Self::SignoffRecorded),
            "recovery_signoff_missing" => Ok(Self::SignoffMissing),
            "recovery_invalid_payload" => Ok(Self::InvalidPayload),
            "recovery_dependency_unavailable" => Ok(Self::DependencyUnavailable),
            "recovery_unauthorized" => Ok(Self::Unauthorized),
            "recovery_persistence_unavailable" => Ok(Self::PersistenceUnavailable),
            "recovery_not_found" => Ok(Self::NotFound),
            "recovery_stale_evidence" => Ok(Self::StaleEvidence),
            "recovery_rehearsal_success" => Ok(Self::RehearsalSuccess),
            "recovery_rehearsal_checksum_mismatch" => Ok(Self::RehearsalChecksumMismatch),
            "recovery_rehearsal_reconciliation_sanity_failure" => {
                Ok(Self::RehearsalReconciliationSanityFailure)
            }
            "recovery_rehearsal_deterministic_replay_mismatch" => {
                Ok(Self::RehearsalDeterministicReplayMismatch)
            }
            "recovery_rehearsal_signature_contract_error" => {
                Ok(Self::RehearsalSignatureContractError)
            }
            "recovery_rehearsal_missing_or_failed" => Ok(Self::RehearsalMissingOrFailed),
            _ => Err(RecoveryContractError::invalid_payload(format!(
                "unknown recovery reason code `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecoveryValidationIssue {
    pub field: &'static str,
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecoveryContractError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<RecoveryValidationIssue>,
}

impl RecoveryContractError {
    pub fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: RecoveryReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn invalid_payload_with_issues(
        message: impl Into<String>,
        field_errors: Vec<RecoveryValidationIssue>,
    ) -> Self {
        Self {
            code: RecoveryReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    pub fn dependency_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: RecoveryReasonCode::DependencyUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            code: RecoveryReasonCode::Unauthorized.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn persistence_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: RecoveryReasonCode::PersistenceUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            code: RecoveryReasonCode::NotFound.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn stale_evidence(message: impl Into<String>) -> Self {
        Self {
            code: RecoveryReasonCode::StaleEvidence.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecoveryReadinessRequest {
    pub actor_id: String,
    pub actor_role: String,
    pub correlation_id: String,
    pub requested_at_utc: String,
    pub profile_key: String,
    pub reconciliation_run_id: String,
    pub approved_checksum: String,
    pub signoff_intent: Option<String>,
    pub audit_reference: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecoveryOperatorSignoff {
    pub actor_id: String,
    pub actor_role: String,
    pub signoff_intent: String,
    pub signed_at_utc: String,
    pub audit_reference: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecoveryGateOutcome {
    pub gate: RecoveryGateName,
    pub passed: bool,
    pub reason_code: String,
    pub trigger: String,
    pub context: String,
    pub action: String,
    pub verification: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecoveryGateRunEvidence {
    pub run_id: String,
    pub correlation_id: String,
    pub readiness_status: RecoveryReadinessStatus,
    pub reason_code: String,
    pub actor_id: String,
    pub actor_role: String,
    pub profile_key: String,
    pub requested_at_utc: String,
    pub evaluated_at_utc: String,
    pub resumed_at_utc: Option<String>,
    pub freshness_age_seconds: Option<f64>,
    pub freshness_observed_at_utc: Option<String>,
    pub reconciliation_run_id: Option<String>,
    pub reconciliation_mismatch_rate: Option<f64>,
    pub approved_checksum: Option<String>,
    pub computed_checksum: Option<String>,
    pub signoff: Option<RecoveryOperatorSignoff>,
    pub gate_outcomes: Vec<RecoveryGateOutcome>,
    pub failing_gate_codes: Vec<String>,
    pub audit_reference: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecoveryResumeVerificationEnvelope {
    pub run_id: String,
    pub correlation_id: String,
    pub readiness_status: RecoveryReadinessStatus,
    pub reason_code: String,
    pub verified_at_utc: String,
}

#[derive(Debug, Clone, Serialize)]
struct CanonicalRiskLimitBundle {
    profile_key: String,
    version: i64,
    status: String,
    approval_reference: Option<String>,
    actor_id: String,
    reason_code: String,
    correlation_id: String,
    updated_at_utc: String,
    portfolio: CanonicalScopeLimit,
    market: CanonicalScopeLimit,
    strategy: CanonicalScopeLimit,
    inventory_rules: Vec<CanonicalInventoryRule>,
}

#[derive(Debug, Clone, Serialize)]
struct CanonicalScopeLimit {
    scope: String,
    scope_id: String,
    max_notional_usd: f64,
    max_inventory_units: f64,
    max_concentration_pct_nav: f64,
}

#[derive(Debug, Clone, Serialize)]
struct CanonicalInventoryRule {
    rule_id: String,
    scope: String,
    scope_id: String,
    max_position_units: f64,
    max_order_size_units: f64,
    max_concentration_pct_nav: f64,
    actor_id: String,
    correlation_id: String,
    updated_at_utc: String,
}

pub fn validate_recovery_readiness_request(
    request: &RecoveryReadinessRequest,
) -> Result<(), RecoveryContractError> {
    let mut field_errors = Vec::new();
    validate_non_empty_field(&mut field_errors, "actor_id", &request.actor_id);
    validate_non_empty_field(&mut field_errors, "actor_role", &request.actor_role);
    validate_non_empty_field(&mut field_errors, "correlation_id", &request.correlation_id);
    validate_non_empty_field(&mut field_errors, "profile_key", &request.profile_key);
    validate_non_empty_field(
        &mut field_errors,
        "reconciliation_run_id",
        &request.reconciliation_run_id,
    );
    validate_timestamp_field(
        &mut field_errors,
        "requested_at_utc",
        &request.requested_at_utc,
    );
    if request
        .signoff_intent
        .as_deref()
        .map(str::trim)
        .is_none_or(str::is_empty)
    {
        field_errors.push(RecoveryValidationIssue {
            field: "signoff_intent",
            code: RecoveryReasonCode::SignoffMissing.code(),
            message: "operator sign-off intent is required".to_string(),
        });
    }
    if validate_checksum_digest(&request.approved_checksum).is_err() {
        field_errors.push(RecoveryValidationIssue {
            field: "approved_checksum",
            code: RecoveryReasonCode::InvalidPayload.code(),
            message: "approved_checksum must be a lowercase 64-char hex sha256 digest".to_string(),
        });
    }
    if !field_errors.is_empty() {
        return Err(RecoveryContractError::invalid_payload_with_issues(
            "recovery readiness request payload is invalid",
            field_errors,
        ));
    }
    Ok(())
}

pub fn build_operator_signoff(
    request: &RecoveryReadinessRequest,
) -> Result<RecoveryOperatorSignoff, RecoveryContractError> {
    validate_recovery_readiness_request(request)?;
    let signoff_intent = request
        .signoff_intent
        .as_deref()
        .expect("signoff_intent verified by validate_recovery_readiness_request")
        .trim()
        .to_string();
    Ok(RecoveryOperatorSignoff {
        actor_id: normalize_risk_limit_identifier(&request.actor_id),
        actor_role: normalize_risk_limit_identifier(&request.actor_role),
        signoff_intent,
        signed_at_utc: request.requested_at_utc.clone(),
        audit_reference: normalize_optional_field(request.audit_reference.as_deref()),
    })
}

pub fn signoff_recorded(signoff: Option<&RecoveryOperatorSignoff>) -> bool {
    signoff.is_some_and(|value| {
        !value.actor_id.trim().is_empty()
            && !value.actor_role.trim().is_empty()
            && !value.signoff_intent.trim().is_empty()
            && parse_utc_timestamp(&value.signed_at_utc).is_ok()
    })
}

pub fn validate_operator_signoff(
    signoff: &RecoveryOperatorSignoff,
) -> Result<(), RecoveryContractError> {
    let mut field_errors = Vec::new();
    validate_non_empty_field(&mut field_errors, "signoff.actor_id", &signoff.actor_id);
    validate_non_empty_field(&mut field_errors, "signoff.actor_role", &signoff.actor_role);
    validate_non_empty_field(
        &mut field_errors,
        "signoff.signoff_intent",
        &signoff.signoff_intent,
    );
    validate_timestamp_field(
        &mut field_errors,
        "signoff.signed_at_utc",
        &signoff.signed_at_utc,
    );
    if !field_errors.is_empty() {
        return Err(RecoveryContractError::invalid_payload_with_issues(
            "recovery operator sign-off payload is invalid",
            field_errors,
        ));
    }
    Ok(())
}

pub fn evaluate_freshness_readiness(age_seconds: f64) -> Result<bool, RecoveryContractError> {
    if !age_seconds.is_finite() || age_seconds < 0.0 {
        return Err(RecoveryContractError::invalid_payload_with_issues(
            "freshness age seconds must be finite and non-negative",
            vec![RecoveryValidationIssue {
                field: "freshness_age_seconds",
                code: RecoveryReasonCode::InvalidPayload.code(),
                message: "freshness_age_seconds must be finite and >= 0".to_string(),
            }],
        ));
    }
    Ok(age_seconds <= RECOVERY_FRESHNESS_MAX_AGE_SECONDS)
}

pub fn evaluate_reconciliation_readiness(
    mismatch_rate: f64,
) -> Result<bool, RecoveryContractError> {
    if !mismatch_rate.is_finite() || !(0.0..=1.0).contains(&mismatch_rate) {
        return Err(RecoveryContractError::invalid_payload_with_issues(
            "reconciliation mismatch rate must be finite and between 0 and 1",
            vec![RecoveryValidationIssue {
                field: "reconciliation_mismatch_rate",
                code: RecoveryReasonCode::InvalidPayload.code(),
                message: "reconciliation_mismatch_rate must be finite and between 0 and 1"
                    .to_string(),
            }],
        ));
    }
    Ok(mismatch_rate < RECOVERY_RECONCILIATION_MAX_MISMATCH_RATE_EXCLUSIVE)
}

pub fn validate_checksum_digest(checksum: &str) -> Result<String, RecoveryContractError> {
    let normalized = checksum.trim().to_ascii_lowercase();
    if normalized.len() != CHECKSUM_HEX_LEN
        || !normalized.chars().all(|value| value.is_ascii_hexdigit())
    {
        return Err(RecoveryContractError::invalid_payload_with_issues(
            "checksum digest must be lowercase 64-character hex",
            vec![RecoveryValidationIssue {
                field: "checksum",
                code: RecoveryReasonCode::InvalidPayload.code(),
                message: "checksum digest must be lowercase 64-character hex".to_string(),
            }],
        ));
    }
    Ok(normalized)
}

pub fn evaluate_checksum_gate(
    approved_checksum: &str,
    computed_checksum: &str,
) -> Result<bool, RecoveryContractError> {
    let approved = validate_checksum_digest(approved_checksum)?;
    let computed = validate_checksum_digest(computed_checksum)?;
    Ok(approved == computed)
}

pub fn canonicalize_risk_limit_bundle(
    profile: &RiskLimitProfileVersion,
    inventory_rules: &[InventoryLimitRule],
) -> Result<String, RecoveryContractError> {
    validate_risk_limit_profile_version(profile).map_err(map_risk_limit_error)?;

    let normalized_profile_key = normalize_risk_limit_identifier(&profile.profile_key);
    let mut canonical_rules = Vec::with_capacity(inventory_rules.len());
    for rule in inventory_rules {
        validate_inventory_limit_rule(rule).map_err(map_risk_limit_error)?;
        if normalize_risk_limit_identifier(&rule.profile_key) != normalized_profile_key {
            return Err(RecoveryContractError::invalid_payload_with_issues(
                "inventory rule profile_key does not match profile",
                vec![RecoveryValidationIssue {
                    field: "inventory_rules.profile_key",
                    code: RecoveryReasonCode::InvalidPayload.code(),
                    message: "inventory rule profile_key must match profile profile_key"
                        .to_string(),
                }],
            ));
        }
        if rule.profile_version != profile.version {
            return Err(RecoveryContractError::invalid_payload_with_issues(
                "inventory rule profile_version does not match profile version",
                vec![RecoveryValidationIssue {
                    field: "inventory_rules.profile_version",
                    code: RecoveryReasonCode::InvalidPayload.code(),
                    message: "inventory rule profile_version must match profile version"
                        .to_string(),
                }],
            ));
        }
        canonical_rules.push(CanonicalInventoryRule {
            rule_id: normalize_risk_limit_identifier(&rule.rule_id),
            scope: rule.scope.as_str().to_string(),
            scope_id: normalize_risk_limit_identifier(&rule.scope_id),
            max_position_units: rule.max_position_units,
            max_order_size_units: rule.max_order_size_units,
            max_concentration_pct_nav: rule.max_concentration_pct_nav,
            actor_id: normalize_risk_limit_identifier(&rule.actor_id),
            correlation_id: normalize_risk_limit_identifier(&rule.correlation_id),
            updated_at_utc: rule.updated_at_utc.clone(),
        });
    }

    canonical_rules.sort_by(|left, right| {
        left.scope
            .cmp(&right.scope)
            .then(left.scope_id.cmp(&right.scope_id))
            .then(left.rule_id.cmp(&right.rule_id))
    });

    let canonical_bundle = CanonicalRiskLimitBundle {
        profile_key: normalized_profile_key,
        version: profile.version,
        status: profile.status.as_str().to_string(),
        approval_reference: normalize_optional_field(profile.approval_reference.as_deref()),
        actor_id: normalize_risk_limit_identifier(&profile.actor_id),
        reason_code: profile.reason_code.clone(),
        correlation_id: normalize_risk_limit_identifier(&profile.correlation_id),
        updated_at_utc: profile.updated_at_utc.clone(),
        portfolio: canonical_scope_limit(&profile.portfolio),
        market: canonical_scope_limit(&profile.market),
        strategy: canonical_scope_limit(&profile.strategy),
        inventory_rules: canonical_rules,
    };

    serde_json::to_string(&canonical_bundle).map_err(|error| {
        RecoveryContractError::invalid_payload(format!(
            "failed to serialize canonical risk limit bundle: {error}"
        ))
    })
}

pub fn compute_risk_limit_bundle_checksum(
    profile: &RiskLimitProfileVersion,
    inventory_rules: &[InventoryLimitRule],
) -> Result<String, RecoveryContractError> {
    let canonical = canonicalize_risk_limit_bundle(profile, inventory_rules)?;
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn validate_recovery_gate_outcome(
    outcome: &RecoveryGateOutcome,
) -> Result<(), RecoveryContractError> {
    let mut field_errors = Vec::new();
    validate_non_empty_field(&mut field_errors, "reason_code", &outcome.reason_code);
    validate_non_empty_field(&mut field_errors, "trigger", &outcome.trigger);
    validate_non_empty_field(&mut field_errors, "context", &outcome.context);
    validate_non_empty_field(&mut field_errors, "action", &outcome.action);
    validate_non_empty_field(&mut field_errors, "verification", &outcome.verification);
    let parsed_reason = match RecoveryReasonCode::parse(&outcome.reason_code) {
        Ok(value) => Some(value),
        Err(_) => {
            field_errors.push(RecoveryValidationIssue {
                field: "reason_code",
                code: RecoveryReasonCode::InvalidPayload.code(),
                message: "reason_code must be a known recovery reason".to_string(),
            });
            None
        }
    };
    if let Some(reason_code) = parsed_reason {
        let is_pass_reason = matches!(
            reason_code,
            RecoveryReasonCode::FreshnessPass
                | RecoveryReasonCode::ReconciliationPass
                | RecoveryReasonCode::ChecksumMatch
                | RecoveryReasonCode::SignoffRecorded
        );
        let is_gate_reason_match = matches!(
            (outcome.gate, reason_code),
            (
                RecoveryGateName::Freshness,
                RecoveryReasonCode::FreshnessPass
            ) | (
                RecoveryGateName::Freshness,
                RecoveryReasonCode::FreshnessStale
            ) | (
                RecoveryGateName::Reconciliation,
                RecoveryReasonCode::ReconciliationPass
            ) | (
                RecoveryGateName::Reconciliation,
                RecoveryReasonCode::ReconciliationMismatch
            ) | (
                RecoveryGateName::RiskChecksum,
                RecoveryReasonCode::ChecksumMatch
            ) | (
                RecoveryGateName::RiskChecksum,
                RecoveryReasonCode::ChecksumMismatch
            ) | (
                RecoveryGateName::OperatorSignoff,
                RecoveryReasonCode::SignoffRecorded
            ) | (
                RecoveryGateName::OperatorSignoff,
                RecoveryReasonCode::SignoffMissing
            )
        );
        if !is_gate_reason_match {
            field_errors.push(RecoveryValidationIssue {
                field: "reason_code",
                code: RecoveryReasonCode::InvalidPayload.code(),
                message: "reason_code must match the declared gate".to_string(),
            });
        }
        if outcome.passed != is_pass_reason {
            field_errors.push(RecoveryValidationIssue {
                field: "passed",
                code: RecoveryReasonCode::InvalidPayload.code(),
                message: "passed flag must align with reason_code".to_string(),
            });
        }
    }
    if !field_errors.is_empty() {
        return Err(RecoveryContractError::invalid_payload_with_issues(
            "recovery gate outcome payload is invalid",
            field_errors,
        ));
    }
    Ok(())
}

pub fn validate_recovery_gate_run_evidence(
    run: &RecoveryGateRunEvidence,
) -> Result<(), RecoveryContractError> {
    let mut field_errors = Vec::new();
    validate_non_empty_field(&mut field_errors, "run_id", &run.run_id);
    validate_non_empty_field(&mut field_errors, "correlation_id", &run.correlation_id);
    validate_non_empty_field(&mut field_errors, "reason_code", &run.reason_code);
    validate_non_empty_field(&mut field_errors, "actor_id", &run.actor_id);
    validate_non_empty_field(&mut field_errors, "actor_role", &run.actor_role);
    validate_non_empty_field(&mut field_errors, "profile_key", &run.profile_key);
    validate_timestamp_field(&mut field_errors, "requested_at_utc", &run.requested_at_utc);
    validate_timestamp_field(&mut field_errors, "evaluated_at_utc", &run.evaluated_at_utc);
    if let Some(resumed_at_utc) = run.resumed_at_utc.as_deref() {
        validate_timestamp_field(&mut field_errors, "resumed_at_utc", resumed_at_utc);
    }
    if let Some(observed_at_utc) = run.freshness_observed_at_utc.as_deref() {
        validate_timestamp_field(
            &mut field_errors,
            "freshness_observed_at_utc",
            observed_at_utc,
        );
    }
    if let Some(age_seconds) = run.freshness_age_seconds {
        if evaluate_freshness_readiness(age_seconds).is_err() {
            field_errors.push(RecoveryValidationIssue {
                field: "freshness_age_seconds",
                code: RecoveryReasonCode::InvalidPayload.code(),
                message: "freshness_age_seconds must be finite and >= 0".to_string(),
            });
        }
    }
    if let Some(mismatch_rate) = run.reconciliation_mismatch_rate {
        if evaluate_reconciliation_readiness(mismatch_rate).is_err() {
            field_errors.push(RecoveryValidationIssue {
                field: "reconciliation_mismatch_rate",
                code: RecoveryReasonCode::InvalidPayload.code(),
                message: "reconciliation_mismatch_rate must be finite and between 0 and 1"
                    .to_string(),
            });
        }
    }
    match (
        run.approved_checksum.as_deref(),
        run.computed_checksum.as_deref(),
    ) {
        (Some(approved), Some(computed)) => {
            if validate_checksum_digest(approved).is_err() {
                field_errors.push(RecoveryValidationIssue {
                    field: "approved_checksum",
                    code: RecoveryReasonCode::InvalidPayload.code(),
                    message: "approved_checksum must be a lowercase 64-char hex digest".to_string(),
                });
            }
            if validate_checksum_digest(computed).is_err() {
                field_errors.push(RecoveryValidationIssue {
                    field: "computed_checksum",
                    code: RecoveryReasonCode::InvalidPayload.code(),
                    message: "computed_checksum must be a lowercase 64-char hex digest".to_string(),
                });
            }
        }
        (None, None) => {}
        _ => field_errors.push(RecoveryValidationIssue {
            field: "approved_checksum",
            code: RecoveryReasonCode::InvalidPayload.code(),
            message: "approved_checksum and computed_checksum must both be present or absent"
                .to_string(),
        }),
    }
    if let Some(signoff) = run.signoff.as_ref() {
        if let Err(error) = validate_operator_signoff(signoff) {
            field_errors.extend(error.field_errors);
        }
    }
    if RecoveryReasonCode::parse(&run.reason_code).is_err() {
        field_errors.push(RecoveryValidationIssue {
            field: "reason_code",
            code: RecoveryReasonCode::InvalidPayload.code(),
            message: "reason_code must be a known recovery reason".to_string(),
        });
    }
    if run.gate_outcomes.is_empty() {
        field_errors.push(RecoveryValidationIssue {
            field: "gate_outcomes",
            code: RecoveryReasonCode::InvalidPayload.code(),
            message: "gate_outcomes must include all recovery gate checks".to_string(),
        });
    }
    let mut failing_from_outcomes = BTreeSet::new();
    for outcome in &run.gate_outcomes {
        if let Err(error) = validate_recovery_gate_outcome(outcome) {
            field_errors.extend(error.field_errors);
        }
        if !outcome.passed {
            failing_from_outcomes.insert(outcome.reason_code.clone());
        }
    }
    let mut failing_declared = BTreeSet::new();
    for code in &run.failing_gate_codes {
        if RecoveryReasonCode::parse(code).is_err() {
            field_errors.push(RecoveryValidationIssue {
                field: "failing_gate_codes",
                code: RecoveryReasonCode::InvalidPayload.code(),
                message: "failing_gate_codes entries must be known recovery reasons".to_string(),
            });
            continue;
        }
        failing_declared.insert(code.clone());
    }
    if failing_from_outcomes != failing_declared {
        field_errors.push(RecoveryValidationIssue {
            field: "failing_gate_codes",
            code: RecoveryReasonCode::InvalidPayload.code(),
            message: "failing_gate_codes must match gate_outcomes failed reasons".to_string(),
        });
    }
    match run.readiness_status {
        RecoveryReadinessStatus::Approved => {
            if !run.gate_outcomes.iter().all(|outcome| outcome.passed) {
                field_errors.push(RecoveryValidationIssue {
                    field: "gate_outcomes",
                    code: RecoveryReasonCode::InvalidPayload.code(),
                    message: "approved runs cannot include failed gate outcomes".to_string(),
                });
            }
            if !run.failing_gate_codes.is_empty() {
                field_errors.push(RecoveryValidationIssue {
                    field: "failing_gate_codes",
                    code: RecoveryReasonCode::InvalidPayload.code(),
                    message: "approved runs cannot include failing_gate_codes".to_string(),
                });
            }
            if !signoff_recorded(run.signoff.as_ref()) {
                field_errors.push(RecoveryValidationIssue {
                    field: "signoff",
                    code: RecoveryReasonCode::SignoffMissing.code(),
                    message: "approved runs must include recorded operator sign-off".to_string(),
                });
            }
        }
        RecoveryReadinessStatus::Blocked => {
            if run.resumed_at_utc.is_some() {
                field_errors.push(RecoveryValidationIssue {
                    field: "resumed_at_utc",
                    code: RecoveryReasonCode::InvalidPayload.code(),
                    message: "blocked runs cannot include resumed_at_utc".to_string(),
                });
            }
            if run.gate_outcomes.iter().all(|outcome| outcome.passed)
                && run.failing_gate_codes.is_empty()
            {
                field_errors.push(RecoveryValidationIssue {
                    field: "gate_outcomes",
                    code: RecoveryReasonCode::InvalidPayload.code(),
                    message: "blocked runs must include at least one failed gate".to_string(),
                });
            }
        }
    }
    if !field_errors.is_empty() {
        return Err(RecoveryContractError::invalid_payload_with_issues(
            "recovery gate run evidence payload is invalid",
            field_errors,
        ));
    }
    Ok(())
}

pub fn build_recovery_resume_verification_envelope(
    run: &RecoveryGateRunEvidence,
    verified_at_utc: &str,
) -> Result<RecoveryResumeVerificationEnvelope, RecoveryContractError> {
    validate_recovery_gate_run_evidence(run)?;
    parse_utc_timestamp(verified_at_utc).map_err(|_| {
        RecoveryContractError::invalid_payload_with_issues(
            "verified_at_utc must be RFC3339 UTC",
            vec![RecoveryValidationIssue {
                field: "verified_at_utc",
                code: RecoveryReasonCode::InvalidPayload.code(),
                message: "verified_at_utc must be an RFC3339 UTC timestamp".to_string(),
            }],
        )
    })?;
    Ok(RecoveryResumeVerificationEnvelope {
        run_id: run.run_id.clone(),
        correlation_id: run.correlation_id.clone(),
        readiness_status: run.readiness_status,
        reason_code: run.reason_code.clone(),
        verified_at_utc: verified_at_utc.to_string(),
    })
}

fn map_risk_limit_error(error: RiskLimitContractError) -> RecoveryContractError {
    RecoveryContractError {
        code: RecoveryReasonCode::InvalidPayload.code(),
        message: error.message,
        field_errors: error
            .field_errors
            .into_iter()
            .map(|issue| RecoveryValidationIssue {
                field: issue.field,
                code: issue.code,
                message: issue.message,
            })
            .collect(),
    }
}

fn canonical_scope_limit(limit: &RiskScopeLimit) -> CanonicalScopeLimit {
    CanonicalScopeLimit {
        scope: limit.scope.as_str().to_string(),
        scope_id: normalize_risk_limit_identifier(&limit.scope_id),
        max_notional_usd: limit.max_notional_usd,
        max_inventory_units: limit.max_inventory_units,
        max_concentration_pct_nav: limit.max_concentration_pct_nav,
    }
}

fn validate_non_empty_field(
    issues: &mut Vec<RecoveryValidationIssue>,
    field: &'static str,
    value: &str,
) {
    if value.trim().is_empty() {
        issues.push(RecoveryValidationIssue {
            field,
            code: RecoveryReasonCode::InvalidPayload.code(),
            message: format!("{field} must not be empty"),
        });
    }
}

fn validate_timestamp_field(
    issues: &mut Vec<RecoveryValidationIssue>,
    field: &'static str,
    value: &str,
) {
    if value.trim().is_empty() {
        issues.push(RecoveryValidationIssue {
            field,
            code: RecoveryReasonCode::InvalidPayload.code(),
            message: format!("{field} must not be empty"),
        });
        return;
    }
    if parse_utc_timestamp(value).is_err() {
        issues.push(RecoveryValidationIssue {
            field,
            code: RecoveryReasonCode::InvalidPayload.code(),
            message: format!("{field} must be an RFC3339 UTC timestamp"),
        });
    }
}

fn parse_utc_timestamp(value: &str) -> Result<OffsetDateTime, RecoveryContractError> {
    let parsed = OffsetDateTime::parse(value, &Rfc3339).map_err(|error| {
        RecoveryContractError::invalid_payload(format!(
            "timestamp `{value}` must be RFC3339 UTC: {error}"
        ))
    })?;
    if parsed.offset() != UtcOffset::UTC {
        return Err(RecoveryContractError::invalid_payload(format!(
            "timestamp `{value}` must be UTC"
        )));
    }
    Ok(parsed)
}

fn normalize_optional_field(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|candidate| !candidate.is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::risk::{
        RiskLimitProfileStatus, RiskLimitReasonCode, RiskLimitScope, RiskScopeLimit,
    };

    #[test]
    fn freshness_gate_passes_at_boundary_and_fails_above() {
        assert!(evaluate_freshness_readiness(30.0).expect("boundary should pass"));
        assert!(!evaluate_freshness_readiness(30.000_001).expect("above boundary should fail"));
    }

    #[test]
    fn reconciliation_gate_requires_strictly_less_than_point_one_percent() {
        assert!(evaluate_reconciliation_readiness(0.000_999).expect("strictly below threshold"));
        assert!(!evaluate_reconciliation_readiness(0.001).expect("equal threshold must fail"));
    }

    #[test]
    fn checksum_gate_requires_exact_digest_equality() {
        let digest_a = "a".repeat(64);
        let digest_b = "b".repeat(64);
        assert!(evaluate_checksum_gate(&digest_a, &digest_a).expect("matching digest"));
        assert!(!evaluate_checksum_gate(&digest_a, &digest_b).expect("mismatched digest"));
        assert!(evaluate_checksum_gate("not-a-digest", &digest_a).is_err());
    }

    #[test]
    fn readiness_request_requires_explicit_signoff() {
        let request = RecoveryReadinessRequest {
            actor_id: "ops-123".to_string(),
            actor_role: "operator".to_string(),
            correlation_id: "corr-123".to_string(),
            requested_at_utc: "2026-04-06T12:00:00Z".to_string(),
            profile_key: "default".to_string(),
            reconciliation_run_id: "recon-1".to_string(),
            approved_checksum: "a".repeat(64),
            signoff_intent: None,
            audit_reference: None,
        };
        let error = validate_recovery_readiness_request(&request).expect_err("missing signoff");
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "signoff_intent"
                    && issue.code == RecoveryReasonCode::SignoffMissing.code())
        );
    }

    #[test]
    fn checksum_is_stable_for_equivalent_rule_ordering() {
        let profile = sample_profile();
        let market_rule = sample_inventory_rule("market-rule", RiskLimitScope::Market, "market-a");
        let strategy_rule =
            sample_inventory_rule("strategy-rule", RiskLimitScope::Strategy, "strategy-a");
        let checksum_a = compute_risk_limit_bundle_checksum(
            &profile,
            &[market_rule.clone(), strategy_rule.clone()],
        )
        .expect("checksum should compute");
        let checksum_b =
            compute_risk_limit_bundle_checksum(&profile, &[strategy_rule, market_rule])
                .expect("checksum should compute");
        assert_eq!(checksum_a, checksum_b);
    }

    #[test]
    fn approved_run_allows_pending_resume_but_rejects_failed_gates() {
        let mut run = sample_approved_run();
        run.resumed_at_utc = None;
        validate_recovery_gate_run_evidence(&run)
            .expect("approved readiness can be persisted before resume execution");

        run.failing_gate_codes = vec![RecoveryReasonCode::FreshnessStale.code().to_string()];
        let error = validate_recovery_gate_run_evidence(&run).expect_err("approved run invalid");
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "failing_gate_codes")
        );
    }

    fn sample_profile() -> RiskLimitProfileVersion {
        RiskLimitProfileVersion {
            profile_key: "default".to_string(),
            version: 4,
            portfolio: RiskScopeLimit {
                scope: RiskLimitScope::Portfolio,
                scope_id: "portfolio".to_string(),
                max_notional_usd: 2_000_000.0,
                max_inventory_units: 20_000.0,
                max_concentration_pct_nav: 40.0,
            },
            market: RiskScopeLimit {
                scope: RiskLimitScope::Market,
                scope_id: "market".to_string(),
                max_notional_usd: 1_000_000.0,
                max_inventory_units: 10_000.0,
                max_concentration_pct_nav: 25.0,
            },
            strategy: RiskScopeLimit {
                scope: RiskLimitScope::Strategy,
                scope_id: "strategy".to_string(),
                max_notional_usd: 500_000.0,
                max_inventory_units: 5_000.0,
                max_concentration_pct_nav: 15.0,
            },
            status: RiskLimitProfileStatus::Active,
            approval_reference: Some("ARB-2026-0001".to_string()),
            actor_id: "operator-admin".to_string(),
            reason_code: RiskLimitReasonCode::ProfileApplied.code().to_string(),
            correlation_id: "corr-123".to_string(),
            updated_at_utc: "2026-04-06T11:00:00Z".to_string(),
        }
    }

    fn sample_inventory_rule(
        rule_id: &str,
        scope: RiskLimitScope,
        scope_id: &str,
    ) -> InventoryLimitRule {
        InventoryLimitRule {
            rule_id: rule_id.to_string(),
            profile_key: "default".to_string(),
            profile_version: 4,
            scope,
            scope_id: scope_id.to_string(),
            max_position_units: 500.0,
            max_order_size_units: 100.0,
            max_concentration_pct_nav: 5.0,
            actor_id: "operator-admin".to_string(),
            correlation_id: "corr-123".to_string(),
            updated_at_utc: "2026-04-06T11:00:00Z".to_string(),
        }
    }

    fn sample_approved_run() -> RecoveryGateRunEvidence {
        RecoveryGateRunEvidence {
            run_id: "run-1".to_string(),
            correlation_id: "corr-123".to_string(),
            readiness_status: RecoveryReadinessStatus::Approved,
            reason_code: RecoveryReasonCode::ResumeApproved.code().to_string(),
            actor_id: "operator-admin".to_string(),
            actor_role: "operator".to_string(),
            profile_key: "default".to_string(),
            requested_at_utc: "2026-04-06T12:00:00Z".to_string(),
            evaluated_at_utc: "2026-04-06T12:00:01Z".to_string(),
            resumed_at_utc: Some("2026-04-06T12:00:02Z".to_string()),
            freshness_age_seconds: Some(10.0),
            freshness_observed_at_utc: Some("2026-04-06T11:59:51Z".to_string()),
            reconciliation_run_id: Some("recon-1".to_string()),
            reconciliation_mismatch_rate: Some(0.0001),
            approved_checksum: Some("a".repeat(64)),
            computed_checksum: Some("a".repeat(64)),
            signoff: Some(RecoveryOperatorSignoff {
                actor_id: "operator-admin".to_string(),
                actor_role: "operator".to_string(),
                signoff_intent: "I approve controlled recovery resume".to_string(),
                signed_at_utc: "2026-04-06T12:00:00Z".to_string(),
                audit_reference: Some("ARB-2026-0001".to_string()),
            }),
            gate_outcomes: vec![
                RecoveryGateOutcome {
                    gate: RecoveryGateName::Freshness,
                    passed: true,
                    reason_code: RecoveryReasonCode::FreshnessPass.code().to_string(),
                    trigger: "freshness_age_seconds <= 30".to_string(),
                    context: "age_seconds=10".to_string(),
                    action: "allow resume freshness gate".to_string(),
                    verification: "freshness telemetry validated".to_string(),
                },
                RecoveryGateOutcome {
                    gate: RecoveryGateName::Reconciliation,
                    passed: true,
                    reason_code: RecoveryReasonCode::ReconciliationPass.code().to_string(),
                    trigger: "mismatch_rate < 0.1%".to_string(),
                    context: "mismatch_rate=0.01%".to_string(),
                    action: "allow resume reconciliation gate".to_string(),
                    verification: "reconciliation report validated".to_string(),
                },
                RecoveryGateOutcome {
                    gate: RecoveryGateName::RiskChecksum,
                    passed: true,
                    reason_code: RecoveryReasonCode::ChecksumMatch.code().to_string(),
                    trigger: "approved checksum equals computed checksum".to_string(),
                    context: "checksum comparison executed".to_string(),
                    action: "allow resume checksum gate".to_string(),
                    verification: "risk profile digest validated".to_string(),
                },
                RecoveryGateOutcome {
                    gate: RecoveryGateName::OperatorSignoff,
                    passed: true,
                    reason_code: RecoveryReasonCode::SignoffRecorded.code().to_string(),
                    trigger: "explicit signoff intent recorded".to_string(),
                    context: "operator signoff attached".to_string(),
                    action: "allow resume signoff gate".to_string(),
                    verification: "audit reference recorded".to_string(),
                },
            ],
            failing_gate_codes: Vec::new(),
            audit_reference: Some("ARB-2026-0001".to_string()),
        }
    }
}
