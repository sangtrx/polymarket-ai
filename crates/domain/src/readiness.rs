use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReadinessReasonCode {
    Ready,
    Caution,
    NotReady,
    InvalidPayload,
    DependencyUnavailable,
    PersistenceUnavailable,
    WaiverActive,
    WaiverExpired,
    WaiverRevoked,
}

impl ReadinessReasonCode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Ready => "readiness_ready",
            Self::Caution => "readiness_caution",
            Self::NotReady => "readiness_not_ready",
            Self::InvalidPayload => "readiness_invalid_payload",
            Self::DependencyUnavailable => "readiness_dependency_unavailable",
            Self::PersistenceUnavailable => "readiness_persistence_unavailable",
            Self::WaiverActive => "readiness_waiver_active",
            Self::WaiverExpired => "readiness_waiver_expired",
            Self::WaiverRevoked => "readiness_waiver_revoked",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ReadinessContractError> {
        match normalize_readiness_identifier(value).as_str() {
            "readiness_ready" => Ok(Self::Ready),
            "readiness_caution" => Ok(Self::Caution),
            "readiness_not_ready" => Ok(Self::NotReady),
            "readiness_invalid_payload" => Ok(Self::InvalidPayload),
            "readiness_dependency_unavailable" => Ok(Self::DependencyUnavailable),
            "readiness_persistence_unavailable" => Ok(Self::PersistenceUnavailable),
            "readiness_waiver_active" => Ok(Self::WaiverActive),
            "readiness_waiver_expired" => Ok(Self::WaiverExpired),
            "readiness_waiver_revoked" => Ok(Self::WaiverRevoked),
            other => Err(ReadinessContractError::invalid_payload(format!(
                "unknown readiness reason code `{other}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReadinessValidationIssue {
    pub field: &'static str,
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReadinessContractError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<ReadinessValidationIssue>,
}

impl ReadinessContractError {
    pub fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: ReadinessReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn invalid_payload_with_issues(
        message: impl Into<String>,
        field_errors: Vec<ReadinessValidationIssue>,
    ) -> Self {
        Self {
            code: ReadinessReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReadinessState {
    Ready,
    Caution,
    NotReady,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WaiverState {
    Active,
    Expired,
    Revoked,
}

impl WaiverState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Expired => "expired",
            Self::Revoked => "revoked",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ReadinessContractError> {
        match normalize_readiness_identifier(value).as_str() {
            "active" => Ok(Self::Active),
            "expired" => Ok(Self::Expired),
            "revoked" => Ok(Self::Revoked),
            _ => Err(ReadinessContractError::invalid_payload_with_issues(
                "waiver_state must be one of: active, expired, revoked",
                vec![ReadinessValidationIssue {
                    field: "waiver_state",
                    code: ReadinessReasonCode::InvalidPayload.code(),
                    message: "waiver_state must be one of: active, expired, revoked".to_string(),
                }],
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WaiverRecord {
    pub waiver_id: String,
    pub canonical_requirement_id: String,
    pub owner: String,
    pub reason_code: String,
    pub justification: String,
    pub approved_by: String,
    pub created_at_utc: String,
    pub expires_at_utc: String,
    pub revoked_at_utc: Option<String>,
}

pub fn normalize_readiness_identifier(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}

pub fn parse_utc_timestamp(
    field: &'static str,
    value: &str,
) -> Result<OffsetDateTime, ReadinessContractError> {
    let timestamp = OffsetDateTime::parse(value.trim(), &Rfc3339).map_err(|_| {
        ReadinessContractError::invalid_payload_with_issues(
            format!("{field} must be an RFC3339 UTC timestamp"),
            vec![ReadinessValidationIssue {
                field,
                code: ReadinessReasonCode::InvalidPayload.code(),
                message: format!("{field} must be an RFC3339 UTC timestamp"),
            }],
        )
    })?;
    if timestamp.offset() != UtcOffset::UTC {
        return Err(ReadinessContractError::invalid_payload_with_issues(
            format!("{field} must be UTC (offset Z)"),
            vec![ReadinessValidationIssue {
                field,
                code: ReadinessReasonCode::InvalidPayload.code(),
                message: format!("{field} must be UTC (offset Z)"),
            }],
        ));
    }
    Ok(timestamp.to_offset(UtcOffset::UTC))
}

pub fn validate_waiver(waiver: &WaiverRecord) -> Result<(), ReadinessContractError> {
    let mut field_errors = Vec::new();

    validate_identifier_issue(
        &mut field_errors,
        "waiver_id",
        waiver.waiver_id.as_str(),
        (3, 200),
    );
    validate_identifier_issue(
        &mut field_errors,
        "canonical_requirement_id",
        waiver.canonical_requirement_id.as_str(),
        (3, 200),
    );
    validate_identifier_issue(&mut field_errors, "owner", waiver.owner.as_str(), (3, 200));
    validate_identifier_issue(
        &mut field_errors,
        "reason_code",
        waiver.reason_code.as_str(),
        (3, 200),
    );
    validate_identifier_issue(
        &mut field_errors,
        "approved_by",
        waiver.approved_by.as_str(),
        (3, 200),
    );

    if waiver.justification.trim().is_empty() || waiver.justification.trim().len() > 2_000 {
        field_errors.push(ReadinessValidationIssue {
            field: "justification",
            code: ReadinessReasonCode::InvalidPayload.code(),
            message: "justification must contain 1-2000 characters".to_string(),
        });
    }

    let created_at = parse_timestamp_issue(&mut field_errors, "created_at_utc", &waiver.created_at_utc);
    let expires_at = parse_timestamp_issue(&mut field_errors, "expires_at_utc", &waiver.expires_at_utc);
    let revoked_at = parse_optional_timestamp_issue(
        &mut field_errors,
        "revoked_at_utc",
        waiver.revoked_at_utc.as_deref(),
    );

    if let (Some(created_at), Some(expires_at)) = (created_at, expires_at)
        && expires_at <= created_at
    {
        field_errors.push(ReadinessValidationIssue {
            field: "expires_at_utc",
            code: ReadinessReasonCode::InvalidPayload.code(),
            message: "expires_at_utc must be greater than created_at_utc".to_string(),
        });
    }

    if let (Some(created_at), Some(revoked_at)) = (created_at, revoked_at)
        && revoked_at < created_at
    {
        field_errors.push(ReadinessValidationIssue {
            field: "revoked_at_utc",
            code: ReadinessReasonCode::InvalidPayload.code(),
            message: "revoked_at_utc must be greater than or equal to created_at_utc".to_string(),
        });
    }

    if !field_errors.is_empty() {
        return Err(ReadinessContractError::invalid_payload_with_issues(
            "waiver failed validation",
            field_errors,
        ));
    }

    Ok(())
}

pub fn resolve_waiver_state(
    waiver: &WaiverRecord,
    as_of_utc: &str,
) -> Result<WaiverState, ReadinessContractError> {
    validate_waiver(waiver)?;
    let as_of = parse_utc_timestamp("as_of_utc", as_of_utc)?;
    let expires_at = parse_utc_timestamp("expires_at_utc", &waiver.expires_at_utc)?;
    if let Some(revoked_at_utc) = waiver.revoked_at_utc.as_deref() {
        let revoked_at = parse_utc_timestamp("revoked_at_utc", revoked_at_utc)?;
        if revoked_at <= as_of {
            return Ok(WaiverState::Revoked);
        }
    }
    if expires_at <= as_of {
        return Ok(WaiverState::Expired);
    }
    Ok(WaiverState::Active)
}

fn validate_identifier_issue(
    issues: &mut Vec<ReadinessValidationIssue>,
    field: &'static str,
    value: &str,
    (min_len, max_len): (usize, usize),
) {
    let normalized = normalize_readiness_identifier(value);
    if normalized.len() < min_len || normalized.len() > max_len {
        issues.push(ReadinessValidationIssue {
            field,
            code: ReadinessReasonCode::InvalidPayload.code(),
            message: format!("{field} must contain {min_len}-{max_len} canonical characters"),
        });
        return;
    }
    if !normalized.chars().all(|character| {
        character.is_ascii_lowercase() || character.is_ascii_digit() || "._:-".contains(character)
    }) {
        issues.push(ReadinessValidationIssue {
            field,
            code: ReadinessReasonCode::InvalidPayload.code(),
            message: format!("{field} contains unsupported characters"),
        });
    }
}

fn parse_timestamp_issue(
    issues: &mut Vec<ReadinessValidationIssue>,
    field: &'static str,
    value: &str,
) -> Option<OffsetDateTime> {
    match parse_utc_timestamp(field, value) {
        Ok(parsed) => Some(parsed),
        Err(error) => {
            issues.extend(error.field_errors);
            None
        }
    }
}

fn parse_optional_timestamp_issue(
    issues: &mut Vec<ReadinessValidationIssue>,
    field: &'static str,
    value: Option<&str>,
) -> Option<OffsetDateTime> {
    value.and_then(|candidate| parse_timestamp_issue(issues, field, candidate))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_waiver() -> WaiverRecord {
        WaiverRecord {
            waiver_id: "waiver-001".to_string(),
            canonical_requirement_id: "gate-03".to_string(),
            owner: "ops-owner".to_string(),
            reason_code: "approved_exception".to_string(),
            justification: "mitigation active".to_string(),
            approved_by: "ops-approver".to_string(),
            created_at_utc: "2026-04-09T00:00:00Z".to_string(),
            expires_at_utc: "2026-04-12T00:00:00Z".to_string(),
            revoked_at_utc: None,
        }
    }

    #[test]
    fn waiver_state_enum_includes_active_expired_revoked() {
        assert_eq!(WaiverState::Active, WaiverState::Active);
        assert_eq!(WaiverState::Expired, WaiverState::Expired);
        assert_eq!(WaiverState::Revoked, WaiverState::Revoked);
    }

    #[test]
    fn validate_waiver_rejects_missing_required_metadata() {
        let mut waiver = sample_waiver();
        waiver.owner = String::new();
        let error = validate_waiver(&waiver).expect_err("owner is required");
        assert_eq!(error.code, "readiness_invalid_payload");
    }

    #[test]
    fn parse_utc_timestamp_rejects_non_z_offsets() {
        let error = parse_utc_timestamp("created_at_utc", "2026-04-09T00:00:00+01:00")
            .expect_err("offset timestamps must fail closed");
        assert_eq!(error.code, "readiness_invalid_payload");
    }

    #[test]
    fn reason_codes_roundtrip_is_canonical() {
        for reason_code in [
            ReadinessReasonCode::Ready,
            ReadinessReasonCode::Caution,
            ReadinessReasonCode::NotReady,
            ReadinessReasonCode::InvalidPayload,
            ReadinessReasonCode::DependencyUnavailable,
            ReadinessReasonCode::PersistenceUnavailable,
            ReadinessReasonCode::WaiverActive,
            ReadinessReasonCode::WaiverExpired,
            ReadinessReasonCode::WaiverRevoked,
        ] {
            assert_eq!(
                ReadinessReasonCode::parse(reason_code.code()).expect("code should parse"),
                reason_code
            );
        }
    }

    #[test]
    fn validate_waiver_rejects_invalid_timeline() {
        let mut waiver = sample_waiver();
        waiver.expires_at_utc = "2026-04-09T00:00:00Z".to_string();

        let error = validate_waiver(&waiver).expect_err("expires must be after created");
        assert_eq!(error.code, "readiness_invalid_payload");
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "expires_at_utc")
        );
    }

    #[test]
    fn resolves_waiver_state_deterministically() {
        let waiver = sample_waiver();
        assert_eq!(
            resolve_waiver_state(&waiver, "2026-04-09T12:00:00Z").expect("active"),
            WaiverState::Active
        );

        assert_eq!(
            resolve_waiver_state(&waiver, "2026-04-13T00:00:00Z").expect("expired"),
            WaiverState::Expired
        );

        let mut revoked = waiver.clone();
        revoked.revoked_at_utc = Some("2026-04-09T10:00:00Z".to_string());
        assert_eq!(
            resolve_waiver_state(&revoked, "2026-04-09T12:00:00Z").expect("revoked"),
            WaiverState::Revoked
        );
    }
}
