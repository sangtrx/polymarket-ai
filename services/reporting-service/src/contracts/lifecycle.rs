use domain::reporting::{
    ReportingContractError, ReportingReasonCode, ReportingValidationIssue,
    normalize_reporting_identifier, parse_utc_timestamp,
};
use persistence::postgres::api_contract_versions::ApiContractVersionRecord;
use serde::{Deserialize, Serialize};
use time::Duration;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContractLifecycleStatus {
    Active,
    Deprecated,
    Replaced,
    Sunset,
}

impl ContractLifecycleStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Deprecated => "deprecated",
            Self::Replaced => "replaced",
            Self::Sunset => "sunset",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ReportingContractError> {
        match normalize_reporting_identifier(value).as_str() {
            "active" => Ok(Self::Active),
            "deprecated" => Ok(Self::Deprecated),
            "replaced" => Ok(Self::Replaced),
            "sunset" => Ok(Self::Sunset),
            _ => Err(ReportingContractError::invalid_payload_with_issues(
                "lifecycle status must be one of: active, deprecated, replaced, sunset",
                vec![ReportingValidationIssue {
                    field: "lifecycle_status",
                    code: ReportingReasonCode::InvalidPayload.code(),
                    message:
                        "lifecycle status must be one of: active, deprecated, replaced, sunset"
                            .to_string(),
                }],
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContractLifecycleWindow {
    pub release_at_utc: String,
    pub deprecation_notice_at_utc: Option<String>,
    pub backward_compatible_until_utc: Option<String>,
    pub sunset_at_utc: Option<String>,
    pub replacement_contract_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReportingContractVersionDescriptor {
    pub contract_key: String,
    pub contract_version: String,
    pub lifecycle_status: ContractLifecycleStatus,
    pub is_active: bool,
    pub lifecycle_window: ContractLifecycleWindow,
    pub schema_artifact_path: String,
    pub changelog_artifact_path: String,
    pub schema_checksum_sha256: String,
    pub changelog_checksum_sha256: String,
    pub record_checksum_sha256: String,
}

impl ReportingContractVersionDescriptor {
    pub fn from_record(record: ApiContractVersionRecord) -> Result<Self, ReportingContractError> {
        let descriptor = Self {
            contract_key: normalize_identifier("contract_key", &record.contract_key)?,
            contract_version: normalize_contract_version(&record.contract_version)?,
            lifecycle_status: ContractLifecycleStatus::parse(&record.lifecycle_status)?,
            is_active: record.is_active,
            lifecycle_window: ContractLifecycleWindow {
                release_at_utc: record.release_at_utc,
                deprecation_notice_at_utc: record.deprecation_notice_at_utc,
                backward_compatible_until_utc: record.backward_compatible_until_utc,
                sunset_at_utc: record.sunset_at_utc,
                replacement_contract_version: record.replacement_contract_version,
            },
            schema_artifact_path: normalize_artifact_path(
                "schema_artifact_path",
                &record.schema_artifact_path,
            )?,
            changelog_artifact_path: normalize_artifact_path(
                "changelog_artifact_path",
                &record.changelog_artifact_path,
            )?,
            schema_checksum_sha256: normalize_sha256(
                "schema_checksum_sha256",
                &record.schema_checksum_sha256,
            )?,
            changelog_checksum_sha256: normalize_sha256(
                "changelog_checksum_sha256",
                &record.changelog_checksum_sha256,
            )?,
            record_checksum_sha256: normalize_sha256(
                "record_checksum_sha256",
                &record.record_checksum_sha256,
            )?,
        };
        validate_contract_lifecycle(&descriptor, None)?;
        Ok(descriptor)
    }
}

pub fn validate_contract_lifecycle(
    descriptor: &ReportingContractVersionDescriptor,
    replacement_release_at_utc: Option<&str>,
) -> Result<(), ReportingContractError> {
    let release_at = parse_utc_timestamp(
        "release_at_utc",
        descriptor.lifecycle_window.release_at_utc.as_str(),
    )?;
    let deprecation_notice_at = parse_optional_timestamp(
        "deprecation_notice_at_utc",
        descriptor
            .lifecycle_window
            .deprecation_notice_at_utc
            .as_deref(),
    )?;
    let backward_compatible_until = parse_optional_timestamp(
        "backward_compatible_until_utc",
        descriptor
            .lifecycle_window
            .backward_compatible_until_utc
            .as_deref(),
    )?;
    let sunset_at = parse_optional_timestamp(
        "sunset_at_utc",
        descriptor.lifecycle_window.sunset_at_utc.as_deref(),
    )?;
    let replacement_release_at =
        parse_optional_timestamp("replacement_release_at_utc", replacement_release_at_utc)?;

    if descriptor.is_active && descriptor.lifecycle_status != ContractLifecycleStatus::Active {
        return Err(ReportingContractError::invalid_payload_with_issues(
            "is_active=true requires lifecycle_status=active",
            vec![ReportingValidationIssue {
                field: "is_active",
                code: ReportingReasonCode::InvalidPayload.code(),
                message: "active contracts must have lifecycle_status=active".to_string(),
            }],
        ));
    }

    if descriptor.lifecycle_status != ContractLifecycleStatus::Active
        && deprecation_notice_at.is_none()
    {
        return Err(ReportingContractError::invalid_payload_with_issues(
            "deprecation_notice_at_utc is required for deprecated/replaced/sunset lifecycle statuses",
            vec![ReportingValidationIssue {
                field: "deprecation_notice_at_utc",
                code: ReportingReasonCode::InvalidPayload.code(),
                message:
                    "deprecation_notice_at_utc is required for deprecated/replaced/sunset lifecycle statuses"
                        .to_string(),
            }],
        ));
    }

    if let Some(deprecation_notice_at) = deprecation_notice_at
        && deprecation_notice_at < release_at + Duration::days(90)
    {
        return Err(ReportingContractError::invalid_payload_with_issues(
            "deprecation_notice_at_utc must be at least 90 days after release_at_utc",
            vec![ReportingValidationIssue {
                field: "deprecation_notice_at_utc",
                code: ReportingReasonCode::InvalidPayload.code(),
                message: "deprecation_notice_at_utc must be >= release_at_utc + 90 days"
                    .to_string(),
            }],
        ));
    }

    let replacement_contract_version = descriptor
        .lifecycle_window
        .replacement_contract_version
        .as_deref()
        .map(|value| normalize_contract_version(value))
        .transpose()?;
    if descriptor.lifecycle_status == ContractLifecycleStatus::Replaced
        && replacement_contract_version.is_none()
    {
        return Err(ReportingContractError::invalid_payload_with_issues(
            "replacement_contract_version is required when lifecycle_status=replaced",
            vec![ReportingValidationIssue {
                field: "replacement_contract_version",
                code: ReportingReasonCode::InvalidPayload.code(),
                message: "replacement_contract_version is required when lifecycle_status=replaced"
                    .to_string(),
            }],
        ));
    }

    if replacement_contract_version.is_some() && backward_compatible_until.is_none() {
        return Err(ReportingContractError::invalid_payload_with_issues(
            "backward_compatible_until_utc is required when replacement_contract_version is set",
            vec![ReportingValidationIssue {
                field: "backward_compatible_until_utc",
                code: ReportingReasonCode::InvalidPayload.code(),
                message:
                    "backward_compatible_until_utc is required when replacement_contract_version is set"
                        .to_string(),
            }],
        ));
    }

    if let (Some(backward_compatible_until), Some(sunset_at)) =
        (backward_compatible_until, sunset_at)
        && sunset_at < backward_compatible_until
    {
        return Err(ReportingContractError::invalid_payload_with_issues(
            "sunset_at_utc cannot precede backward_compatible_until_utc",
            vec![ReportingValidationIssue {
                field: "sunset_at_utc",
                code: ReportingReasonCode::InvalidPayload.code(),
                message: "sunset_at_utc must be >= backward_compatible_until_utc".to_string(),
            }],
        ));
    }

    if let (Some(replacement_release_at), Some(backward_compatible_until)) =
        (replacement_release_at, backward_compatible_until)
        && backward_compatible_until < replacement_release_at + Duration::days(180)
    {
        return Err(ReportingContractError::invalid_payload_with_issues(
            "backward_compatible_until_utc must preserve support for at least 6 months after replacement release",
            vec![ReportingValidationIssue {
                field: "backward_compatible_until_utc",
                code: ReportingReasonCode::InvalidPayload.code(),
                message:
                    "backward_compatible_until_utc must be >= replacement_release_at_utc + 180 days"
                        .to_string(),
            }],
        ));
    }

    if let (Some(replacement_release_at), Some(deprecation_notice_at)) =
        (replacement_release_at, deprecation_notice_at)
        && replacement_release_at < deprecation_notice_at + Duration::days(90)
    {
        return Err(ReportingContractError::invalid_payload_with_issues(
            "replacement release must honor at least 90 days of deprecation notice",
            vec![ReportingValidationIssue {
                field: "deprecation_notice_at_utc",
                code: ReportingReasonCode::InvalidPayload.code(),
                message:
                    "replacement_release_at_utc must be >= deprecation_notice_at_utc + 90 days"
                        .to_string(),
            }],
        ));
    }

    Ok(())
}

fn normalize_identifier(
    field: &'static str,
    value: &str,
) -> Result<String, ReportingContractError> {
    let normalized = normalize_reporting_identifier(value);
    if normalized.len() < 3 || normalized.len() > 160 {
        return Err(ReportingContractError::invalid_payload_with_issues(
            format!("{field} must contain 3-160 canonical characters"),
            vec![ReportingValidationIssue {
                field,
                code: ReportingReasonCode::InvalidPayload.code(),
                message: format!("{field} must contain 3-160 canonical characters"),
            }],
        ));
    }
    if !normalized.chars().all(|character| {
        character.is_ascii_lowercase() || character.is_ascii_digit() || "._:-".contains(character)
    }) {
        return Err(ReportingContractError::invalid_payload_with_issues(
            format!("{field} contains unsupported characters"),
            vec![ReportingValidationIssue {
                field,
                code: ReportingReasonCode::InvalidPayload.code(),
                message: format!("{field} contains unsupported characters"),
            }],
        ));
    }
    Ok(normalized)
}

fn normalize_contract_version(value: &str) -> Result<String, ReportingContractError> {
    let normalized = normalize_reporting_identifier(value);
    if !is_semverish_version(&normalized) {
        return Err(ReportingContractError::invalid_payload_with_issues(
            "contract_version must use version shape like v1, v1.0, or v1.0.1",
            vec![ReportingValidationIssue {
                field: "contract_version",
                code: ReportingReasonCode::InvalidPayload.code(),
                message: "contract_version must use version shape like v1, v1.0, or v1.0.1"
                    .to_string(),
            }],
        ));
    }
    Ok(normalized)
}

fn normalize_artifact_path(
    field: &'static str,
    value: &str,
) -> Result<String, ReportingContractError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ReportingContractError::invalid_payload_with_issues(
            format!("{field} cannot be blank"),
            vec![ReportingValidationIssue {
                field,
                code: ReportingReasonCode::InvalidPayload.code(),
                message: format!("{field} cannot be blank"),
            }],
        ));
    }
    Ok(trimmed.to_string())
}

fn normalize_sha256(field: &'static str, value: &str) -> Result<String, ReportingContractError> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.len() != 64
        || !normalized
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(ReportingContractError::invalid_payload_with_issues(
            format!("{field} must be a 64-character SHA-256 hex digest"),
            vec![ReportingValidationIssue {
                field,
                code: ReportingReasonCode::InvalidPayload.code(),
                message: format!("{field} must be a 64-character SHA-256 hex digest"),
            }],
        ));
    }
    Ok(normalized)
}

fn parse_optional_timestamp(
    field: &'static str,
    value: Option<&str>,
) -> Result<Option<time::OffsetDateTime>, ReportingContractError> {
    let Some(raw) = value else {
        return Ok(None);
    };
    Ok(Some(parse_utc_timestamp(field, raw)?))
}

fn is_semverish_version(value: &str) -> bool {
    let Some(stripped) = value.strip_prefix('v') else {
        return false;
    };
    if stripped.is_empty() {
        return false;
    }
    let mut segment_count = 0usize;
    for segment in stripped.split('.') {
        if segment.is_empty() || !segment.chars().all(|character| character.is_ascii_digit()) {
            return false;
        }
        segment_count += 1;
    }
    (1..=3).contains(&segment_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor() -> ReportingContractVersionDescriptor {
        ReportingContractVersionDescriptor {
            contract_key: "reporting.trades".to_string(),
            contract_version: "v1".to_string(),
            lifecycle_status: ContractLifecycleStatus::Replaced,
            is_active: false,
            lifecycle_window: ContractLifecycleWindow {
                release_at_utc: "2026-04-07T03:30:11Z".to_string(),
                deprecation_notice_at_utc: Some("2026-07-10T00:00:00Z".to_string()),
                backward_compatible_until_utc: Some("2027-05-01T00:00:00Z".to_string()),
                sunset_at_utc: Some("2027-06-01T00:00:00Z".to_string()),
                replacement_contract_version: Some("v2".to_string()),
            },
            schema_artifact_path: "contracts/v1/trades.schema.json".to_string(),
            changelog_artifact_path: "contracts/v1/changelog.json".to_string(),
            schema_checksum_sha256:
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            changelog_checksum_sha256:
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            record_checksum_sha256:
                "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_string(),
        }
    }

    #[test]
    fn lifecycle_validation_accepts_valid_replacement_windows() {
        let descriptor = descriptor();
        let result = validate_contract_lifecycle(&descriptor, Some("2026-10-20T00:00:00Z"));
        assert!(result.is_ok());
    }

    #[test]
    fn lifecycle_validation_rejects_notice_shorter_than_ninety_days() {
        let mut invalid = descriptor();
        invalid.lifecycle_window.deprecation_notice_at_utc =
            Some("2026-05-20T00:00:00Z".to_string());
        let error = validate_contract_lifecycle(&invalid, Some("2026-10-20T00:00:00Z"))
            .expect_err("notice shorter than 90 days must fail");
        assert_eq!(error.code, ReportingReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "deprecation_notice_at_utc")
        );
    }

    #[test]
    fn lifecycle_validation_rejects_support_window_shorter_than_six_months_after_replacement() {
        let mut invalid = descriptor();
        invalid.lifecycle_window.backward_compatible_until_utc =
            Some("2027-02-01T00:00:00Z".to_string());
        let error = validate_contract_lifecycle(&invalid, Some("2026-10-20T00:00:00Z"))
            .expect_err("support window shorter than 6 months must fail");
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "backward_compatible_until_utc")
        );
    }

    #[test]
    fn lifecycle_validation_rejects_active_flag_for_non_active_lifecycle_status() {
        let mut invalid = descriptor();
        invalid.is_active = true;
        let error = validate_contract_lifecycle(&invalid, Some("2026-10-20T00:00:00Z"))
            .expect_err("non-active lifecycle cannot be marked active");
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "is_active")
        );
    }
}
