use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

use crate::recovery::{
    RecoveryContractError, RecoveryReasonCode, RecoveryValidationIssue,
    evaluate_reconciliation_readiness, validate_checksum_digest,
};

const SIGNATURE_HEX_LEN: usize = 64;
const REHEARSAL_STATUS_PASSED: &str = "passed";
const REHEARSAL_STATUS_FAILED: &str = "failed";
const DETERMINISTIC_RATIO_DECIMALS: usize = 12;
const CANONICAL_SIGNATURE_EXCLUDED_FIELDS: [&str; 7] = [
    "run_id",
    "started_at_utc",
    "completed_at_utc",
    "duration_ms",
    "duration_seconds",
    "wall_clock_duration_ms",
    "wall_clock_duration_seconds",
];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RestoreRehearsalStatus {
    Passed,
    Failed,
}

impl RestoreRehearsalStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Passed => REHEARSAL_STATUS_PASSED,
            Self::Failed => REHEARSAL_STATUS_FAILED,
        }
    }

    pub fn parse(value: &str) -> Result<Self, RecoveryContractError> {
        match value {
            REHEARSAL_STATUS_PASSED => Ok(Self::Passed),
            REHEARSAL_STATUS_FAILED => Ok(Self::Failed),
            _ => Err(RecoveryContractError::invalid_payload(format!(
                "unknown restore rehearsal status `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackupIntegrityCheckItem {
    pub check_name: String,
    pub passed: bool,
    pub reason_code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_value: Option<String>,
    pub details: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeterministicReplaySignatureEvidence {
    pub deterministic_signature: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prior_deterministic_signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deterministic_match: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mismatch_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RestoreRehearsalRequest {
    pub actor_id: String,
    pub actor_role: String,
    pub correlation_id: String,
    pub requested_at_utc: String,
    pub artifact_id: String,
    pub artifact_checksum: String,
    pub restore_target: String,
    pub reconciliation_run_id: String,
    pub restore_output: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_checksum: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub incident_correlation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub incident_severity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audit_reference: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RestoreRehearsalRunEvidence {
    pub run_id: String,
    pub correlation_id: String,
    pub artifact_id: String,
    pub artifact_checksum: String,
    pub observed_checksum: String,
    pub restore_target: String,
    pub reconciliation_run_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reconciliation_mismatch_rate: Option<f64>,
    pub reconciliation_passed: bool,
    pub status: RestoreRehearsalStatus,
    pub reason_code: String,
    pub requested_at_utc: String,
    pub started_at_utc: String,
    pub completed_at_utc: String,
    pub integrity_checks: Vec<BackupIntegrityCheckItem>,
    pub deterministic_signature: DeterministicReplaySignatureEvidence,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub incident_correlation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub incident_severity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audit_reference: Option<String>,
}

pub fn normalize_incident_severity(value: &str) -> Option<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "severity_1" | "severity-1" | "sev1" | "sev_1" | "critical" => {
            Some("severity_1".to_string())
        }
        "severity_2" | "severity-2" | "sev2" | "sev_2" | "high" => Some("severity_2".to_string()),
        "severity_3" | "severity-3" | "sev3" | "sev_3" | "warning" => {
            Some("severity_3".to_string())
        }
        "severity_4" | "severity-4" | "sev4" | "sev_4" | "normal" => Some("severity_4".to_string()),
        _ => None,
    }
}

pub fn is_severe_incident(incident_severity: Option<&str>) -> bool {
    incident_severity
        .and_then(normalize_incident_severity)
        .is_some_and(|value| value == "severity_1" || value == "severity_2")
}

pub fn validate_restore_rehearsal_request(
    request: &RestoreRehearsalRequest,
) -> Result<(), RecoveryContractError> {
    let mut field_errors = Vec::new();
    validate_non_empty_field(&mut field_errors, "actor_id", &request.actor_id);
    validate_non_empty_field(&mut field_errors, "actor_role", &request.actor_role);
    validate_non_empty_field(&mut field_errors, "correlation_id", &request.correlation_id);
    validate_non_empty_field(&mut field_errors, "artifact_id", &request.artifact_id);
    validate_non_empty_field(
        &mut field_errors,
        "artifact_checksum",
        &request.artifact_checksum,
    );
    validate_non_empty_field(&mut field_errors, "restore_target", &request.restore_target);
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
    if validate_strict_checksum_digest(&request.artifact_checksum).is_err() {
        field_errors.push(RecoveryValidationIssue {
            field: "artifact_checksum",
            code: RecoveryReasonCode::InvalidPayload.code(),
            message: "artifact_checksum must be lowercase 64-char hex digest".to_string(),
        });
    }
    if let Some(observed_checksum) = request.observed_checksum.as_deref()
        && validate_strict_checksum_digest(observed_checksum).is_err()
    {
        field_errors.push(RecoveryValidationIssue {
            field: "observed_checksum",
            code: RecoveryReasonCode::InvalidPayload.code(),
            message: "observed_checksum must be lowercase 64-char hex digest".to_string(),
        });
    }
    if !request.restore_output.is_object() {
        field_errors.push(RecoveryValidationIssue {
            field: "restore_output",
            code: RecoveryReasonCode::InvalidPayload.code(),
            message: "restore_output must be a structured object payload".to_string(),
        });
    }
    if let Some(incident_severity) = request.incident_severity.as_deref()
        && normalize_incident_severity(incident_severity).is_none()
    {
        field_errors.push(RecoveryValidationIssue {
            field: "incident_severity",
            code: RecoveryReasonCode::InvalidPayload.code(),
            message:
                "incident_severity must normalize to severity_1, severity_2, severity_3, or severity_4"
                    .to_string(),
        });
    }
    if !field_errors.is_empty() {
        return Err(RecoveryContractError::invalid_payload_with_issues(
            "restore rehearsal request payload is invalid",
            field_errors,
        ));
    }
    Ok(())
}

pub fn validate_backup_integrity_check_item(
    item: &BackupIntegrityCheckItem,
) -> Result<(), RecoveryContractError> {
    let mut field_errors = Vec::new();
    validate_non_empty_field(&mut field_errors, "check_name", &item.check_name);
    validate_non_empty_field(&mut field_errors, "reason_code", &item.reason_code);
    validate_non_empty_field(&mut field_errors, "details", &item.details);
    if RecoveryReasonCode::parse(&item.reason_code).is_err() {
        field_errors.push(RecoveryValidationIssue {
            field: "reason_code",
            code: RecoveryReasonCode::InvalidPayload.code(),
            message: "reason_code must be a known recovery reason".to_string(),
        });
    }
    if item
        .check_name
        .trim()
        .eq_ignore_ascii_case("checksum_match")
    {
        if let Some(expected) = item.expected_value.as_deref()
            && validate_strict_checksum_digest(expected).is_err()
        {
            field_errors.push(RecoveryValidationIssue {
                field: "expected_value",
                code: RecoveryReasonCode::InvalidPayload.code(),
                message: "checksum check expected_value must be lowercase 64-char hex digest"
                    .to_string(),
            });
        }
        if let Some(observed) = item.observed_value.as_deref()
            && validate_strict_checksum_digest(observed).is_err()
        {
            field_errors.push(RecoveryValidationIssue {
                field: "observed_value",
                code: RecoveryReasonCode::InvalidPayload.code(),
                message: "checksum check observed_value must be lowercase 64-char hex digest"
                    .to_string(),
            });
        }
    }
    if !field_errors.is_empty() {
        return Err(RecoveryContractError::invalid_payload_with_issues(
            "backup integrity check item is invalid",
            field_errors,
        ));
    }
    Ok(())
}

pub fn validate_restore_rehearsal_run_evidence(
    run: &RestoreRehearsalRunEvidence,
) -> Result<(), RecoveryContractError> {
    let mut field_errors = Vec::new();
    validate_non_empty_field(&mut field_errors, "run_id", &run.run_id);
    validate_non_empty_field(&mut field_errors, "correlation_id", &run.correlation_id);
    validate_non_empty_field(&mut field_errors, "artifact_id", &run.artifact_id);
    validate_non_empty_field(
        &mut field_errors,
        "artifact_checksum",
        &run.artifact_checksum,
    );
    validate_non_empty_field(
        &mut field_errors,
        "observed_checksum",
        &run.observed_checksum,
    );
    validate_non_empty_field(&mut field_errors, "restore_target", &run.restore_target);
    validate_non_empty_field(
        &mut field_errors,
        "reconciliation_run_id",
        &run.reconciliation_run_id,
    );
    validate_non_empty_field(&mut field_errors, "reason_code", &run.reason_code);
    validate_timestamp_field(&mut field_errors, "requested_at_utc", &run.requested_at_utc);
    validate_timestamp_field(&mut field_errors, "started_at_utc", &run.started_at_utc);
    validate_timestamp_field(&mut field_errors, "completed_at_utc", &run.completed_at_utc);
    if validate_strict_checksum_digest(&run.artifact_checksum).is_err() {
        field_errors.push(RecoveryValidationIssue {
            field: "artifact_checksum",
            code: RecoveryReasonCode::InvalidPayload.code(),
            message: "artifact_checksum must be lowercase 64-char hex digest".to_string(),
        });
    }
    if validate_strict_checksum_digest(&run.observed_checksum).is_err() {
        field_errors.push(RecoveryValidationIssue {
            field: "observed_checksum",
            code: RecoveryReasonCode::InvalidPayload.code(),
            message: "observed_checksum must be lowercase 64-char hex digest".to_string(),
        });
    }
    if let Some(mismatch_rate) = run.reconciliation_mismatch_rate
        && evaluate_reconciliation_readiness(mismatch_rate).is_err()
    {
        field_errors.push(RecoveryValidationIssue {
            field: "reconciliation_mismatch_rate",
            code: RecoveryReasonCode::InvalidPayload.code(),
            message: "reconciliation_mismatch_rate must be finite and between 0 and 1".to_string(),
        });
    }
    if RecoveryReasonCode::parse(&run.reason_code).is_err() {
        field_errors.push(RecoveryValidationIssue {
            field: "reason_code",
            code: RecoveryReasonCode::InvalidPayload.code(),
            message: "reason_code must be a known recovery reason".to_string(),
        });
    }
    if let Some(incident_severity) = run.incident_severity.as_deref()
        && normalize_incident_severity(incident_severity).is_none()
    {
        field_errors.push(RecoveryValidationIssue {
            field: "incident_severity",
            code: RecoveryReasonCode::InvalidPayload.code(),
            message:
                "incident_severity must normalize to severity_1, severity_2, severity_3, or severity_4"
                    .to_string(),
        });
    }
    if run.integrity_checks.is_empty() {
        field_errors.push(RecoveryValidationIssue {
            field: "integrity_checks",
            code: RecoveryReasonCode::InvalidPayload.code(),
            message: "integrity_checks must contain per-check evidence".to_string(),
        });
    }
    for check in &run.integrity_checks {
        if let Err(error) = validate_backup_integrity_check_item(check) {
            field_errors.extend(error.field_errors);
        }
    }
    if !is_signature_hex_digest(&run.deterministic_signature.deterministic_signature) {
        field_errors.push(RecoveryValidationIssue {
            field: "deterministic_signature.deterministic_signature",
            code: RecoveryReasonCode::InvalidPayload.code(),
            message: "deterministic_signature must be lowercase 64-char hex digest".to_string(),
        });
    }
    if let Some(previous) = run
        .deterministic_signature
        .prior_deterministic_signature
        .as_deref()
        && !is_signature_hex_digest(previous)
    {
        field_errors.push(RecoveryValidationIssue {
            field: "deterministic_signature.prior_deterministic_signature",
            code: RecoveryReasonCode::InvalidPayload.code(),
            message: "prior_deterministic_signature must be lowercase 64-char hex digest"
                .to_string(),
        });
    }
    if run.deterministic_signature.deterministic_match == Some(false)
        && run
            .deterministic_signature
            .mismatch_summary
            .as_deref()
            .is_none_or(str::is_empty)
    {
        field_errors.push(RecoveryValidationIssue {
            field: "deterministic_signature.mismatch_summary",
            code: RecoveryReasonCode::InvalidPayload.code(),
            message: "mismatch_summary is required when deterministic_match is false".to_string(),
        });
    }
    if run.status == RestoreRehearsalStatus::Passed
        && run.deterministic_signature.deterministic_match == Some(false)
    {
        field_errors.push(RecoveryValidationIssue {
            field: "deterministic_signature.deterministic_match",
            code: RecoveryReasonCode::InvalidPayload.code(),
            message: "deterministic_match=false cannot be marked as a passed rehearsal"
                .to_string(),
        });
    }
    let failed_checks = run.integrity_checks.iter().any(|item| !item.passed);
    match run.status {
        RestoreRehearsalStatus::Passed => {
            if failed_checks {
                field_errors.push(RecoveryValidationIssue {
                    field: "integrity_checks",
                    code: RecoveryReasonCode::InvalidPayload.code(),
                    message: "passed rehearsal cannot include failed checks".to_string(),
                });
            }
            if run.reason_code != RecoveryReasonCode::RehearsalSuccess.code() {
                field_errors.push(RecoveryValidationIssue {
                    field: "reason_code",
                    code: RecoveryReasonCode::InvalidPayload.code(),
                    message: "passed rehearsal reason_code must be recovery_rehearsal_success"
                        .to_string(),
                });
            }
        }
        RestoreRehearsalStatus::Failed => {
            if !failed_checks {
                field_errors.push(RecoveryValidationIssue {
                    field: "integrity_checks",
                    code: RecoveryReasonCode::InvalidPayload.code(),
                    message: "failed rehearsal must include at least one failed check".to_string(),
                });
            }
            let allowed_failure_reason = matches!(
                RecoveryReasonCode::parse(&run.reason_code),
                Ok(RecoveryReasonCode::RehearsalChecksumMismatch
                    | RecoveryReasonCode::RehearsalReconciliationSanityFailure
                    | RecoveryReasonCode::RehearsalDeterministicReplayMismatch
                    | RecoveryReasonCode::DependencyUnavailable
                    | RecoveryReasonCode::InvalidPayload
                    | RecoveryReasonCode::Unauthorized
                    | RecoveryReasonCode::RehearsalSignatureContractError
                    | RecoveryReasonCode::RehearsalMissingOrFailed)
            );
            if !allowed_failure_reason {
                field_errors.push(RecoveryValidationIssue {
                    field: "reason_code",
                    code: RecoveryReasonCode::InvalidPayload.code(),
                    message: "failed rehearsal reason_code must map to explicit rehearsal failure"
                        .to_string(),
                });
            }
        }
    }
    if !field_errors.is_empty() {
        return Err(RecoveryContractError::invalid_payload_with_issues(
            "restore rehearsal evidence payload is invalid",
            field_errors,
        ));
    }
    Ok(())
}

pub fn canonicalize_restore_output(value: &Value) -> Result<String, RecoveryContractError> {
    let canonical = canonicalize_json_value(value, None)?;
    serde_json::to_string(&canonical).map_err(|error| RecoveryContractError {
        code: RecoveryReasonCode::RehearsalSignatureContractError.code(),
        message: format!("unable to serialize canonical restore output: {error}"),
        field_errors: Vec::new(),
    })
}

pub fn compute_deterministic_replay_signature(
    value: &Value,
) -> Result<String, RecoveryContractError> {
    let canonical_json = canonicalize_restore_output(value)?;
    let mut hasher = Sha256::new();
    hasher.update(canonical_json.as_bytes());
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn build_deterministic_replay_signature_evidence(
    output: &Value,
    prior_signature: Option<&str>,
) -> Result<DeterministicReplaySignatureEvidence, RecoveryContractError> {
    let deterministic_signature = compute_deterministic_replay_signature(output)?;
    let prior_deterministic_signature = prior_signature.map(ToOwned::to_owned);
    let deterministic_match = prior_signature.map(|prior| prior == deterministic_signature);
    let mismatch_summary = if deterministic_match == Some(false) {
        Some(
            "deterministic replay mismatch detected: canonical operational outputs differ"
                .to_string(),
        )
    } else {
        None
    };
    Ok(DeterministicReplaySignatureEvidence {
        deterministic_signature,
        prior_deterministic_signature,
        deterministic_match,
        mismatch_summary,
    })
}

fn canonicalize_json_value(
    value: &Value,
    current_key: Option<&str>,
) -> Result<Value, RecoveryContractError> {
    match value {
        Value::Null => Ok(Value::Null),
        Value::Bool(value) => Ok(Value::Bool(*value)),
        Value::String(value) => Ok(Value::String(value.clone())),
        Value::Array(values) => {
            let mut canonical = Vec::with_capacity(values.len());
            for entry in values {
                canonical.push(canonicalize_json_value(entry, None)?);
            }
            Ok(Value::Array(canonical))
        }
        Value::Object(values) => {
            let mut sorted = values.iter().collect::<Vec<_>>();
            sorted.sort_by(|left, right| left.0.cmp(right.0));
            let mut canonical = Map::new();
            for (key, entry) in sorted {
                if should_exclude_signature_field(key) {
                    continue;
                }
                canonical.insert(
                    key.clone(),
                    canonicalize_json_value(entry, Some(key.as_str()))?,
                );
            }
            Ok(Value::Object(canonical))
        }
        Value::Number(number) => {
            if is_ratio_key(current_key) {
                let ratio = number.as_f64().ok_or_else(|| RecoveryContractError {
                    code: RecoveryReasonCode::RehearsalSignatureContractError.code(),
                    message: format!(
                        "ratio field `{}` must be numeric",
                        current_key.unwrap_or("ratio")
                    ),
                    field_errors: vec![RecoveryValidationIssue {
                        field: "restore_output",
                        code: RecoveryReasonCode::RehearsalSignatureContractError.code(),
                        message: "ratio fields must be finite numeric values".to_string(),
                    }],
                })?;
                if !ratio.is_finite() {
                    return Err(RecoveryContractError {
                        code: RecoveryReasonCode::RehearsalSignatureContractError.code(),
                        message: format!(
                            "ratio field `{}` must be finite",
                            current_key.unwrap_or("ratio")
                        ),
                        field_errors: vec![RecoveryValidationIssue {
                            field: "restore_output",
                            code: RecoveryReasonCode::RehearsalSignatureContractError.code(),
                            message: "ratio fields must be finite numeric values".to_string(),
                        }],
                    });
                }
                return Ok(Value::String(format_ratio_decimal(ratio)));
            }
            Ok(Value::Number(number.clone()))
        }
    }
}

fn format_ratio_decimal(value: f64) -> String {
    let mut formatted = format!("{value:.DETERMINISTIC_RATIO_DECIMALS$}");
    while formatted.contains('.') && formatted.ends_with('0') {
        formatted.pop();
    }
    if formatted.ends_with('.') {
        formatted.push('0');
    }
    if formatted == "-0" {
        return "0".to_string();
    }
    formatted
}

fn should_exclude_signature_field(field: &str) -> bool {
    CANONICAL_SIGNATURE_EXCLUDED_FIELDS
        .iter()
        .any(|excluded| excluded.eq_ignore_ascii_case(field))
}

fn is_ratio_key(field: Option<&str>) -> bool {
    let Some(field) = field else {
        return false;
    };
    let normalized = field.to_ascii_lowercase();
    normalized.contains("ratio") || normalized.contains("mismatch_rate")
}

fn is_signature_hex_digest(value: &str) -> bool {
    let normalized = value.trim();
    normalized.len() == SIGNATURE_HEX_LEN
        && normalized
            .chars()
            .all(|character| character.is_ascii_hexdigit() && !character.is_ascii_uppercase())
}

fn validate_strict_checksum_digest(checksum: &str) -> Result<(), RecoveryContractError> {
    let trimmed = checksum.trim();
    if trimmed != checksum {
        return Err(RecoveryContractError::invalid_payload(
            "checksum digest must not contain surrounding whitespace",
        ));
    }
    // Keep compatibility with existing domain helper validation rules, but enforce
    // strict lowercase canonical representation for deterministic comparisons.
    let normalized = validate_checksum_digest(checksum)?;
    if normalized != checksum {
        return Err(RecoveryContractError::invalid_payload(
            "checksum digest must already be lowercase canonical hex",
        ));
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_signature_ignores_only_metadata_fields() {
        let first = serde_json::json!({
            "run_id": "run-a",
            "started_at_utc": "2026-04-06T12:00:00Z",
            "completed_at_utc": "2026-04-06T12:00:02Z",
            "artifact_id": "artifact-1",
            "reconciliation_ratio": 0.1000000000001,
            "nested": {
                "wall_clock_duration_ms": 2034,
                "result": "ok"
            }
        });
        let second = serde_json::json!({
            "completed_at_utc": "2026-04-06T12:20:02Z",
            "artifact_id": "artifact-1",
            "reconciliation_ratio": 0.1000000000001,
            "nested": {
                "result": "ok",
                "wall_clock_duration_ms": 1
            },
            "started_at_utc": "2026-04-06T12:20:00Z",
            "run_id": "run-b"
        });
        let first_signature =
            compute_deterministic_replay_signature(&first).expect("first signature should compute");
        let second_signature = compute_deterministic_replay_signature(&second)
            .expect("second signature should compute");
        assert_eq!(first_signature, second_signature);
    }

    #[test]
    fn deterministic_signature_changes_when_operational_output_changes() {
        let baseline = serde_json::json!({
            "artifact_id": "artifact-1",
            "reconciliation_ratio": 0.0123,
            "checks": {"integrity": true}
        });
        let drifted = serde_json::json!({
            "artifact_id": "artifact-1",
            "reconciliation_ratio": 0.0213,
            "checks": {"integrity": true}
        });
        let baseline_signature = compute_deterministic_replay_signature(&baseline)
            .expect("baseline signature should compute");
        let drifted_signature = compute_deterministic_replay_signature(&drifted)
            .expect("drifted signature should compute");
        assert_ne!(baseline_signature, drifted_signature);
    }

    #[test]
    fn canonicalization_sorts_keys_and_normalizes_ratio_values_to_strings() {
        let payload = serde_json::json!({
            "z": true,
            "a": 1,
            "mismatch_ratio": 0.010000000000,
            "nested": { "b": 2, "a": 1 }
        });
        let canonical =
            canonicalize_restore_output(&payload).expect("canonicalization should succeed");
        assert_eq!(
            canonical,
            r#"{"a":1,"mismatch_ratio":"0.01","nested":{"a":1,"b":2},"z":true}"#
        );
    }

    #[test]
    fn checksum_validation_requires_exact_lowercase_64_hex_boundary() {
        let mut request = sample_request();
        request.artifact_checksum = "A".repeat(64);
        let error = validate_restore_rehearsal_request(&request)
            .expect_err("uppercase checksum should fail strict boundary");
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "artifact_checksum")
        );
    }

    #[test]
    fn restore_output_requires_structured_object_payload() {
        let mut request = sample_request();
        request.restore_output = serde_json::json!(["unexpected", "array"]);
        let error = validate_restore_rehearsal_request(&request)
            .expect_err("non-object restore_output should fail strict payload contract");
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "restore_output")
        );
    }

    #[test]
    fn severe_incident_normalization_maps_expected_values() {
        assert!(is_severe_incident(Some("severity-1")));
        assert!(is_severe_incident(Some("critical")));
        assert!(is_severe_incident(Some("severity_2")));
        assert!(!is_severe_incident(Some("severity_3")));
        assert!(!is_severe_incident(Some("unknown")));
    }

    #[test]
    fn rehearsal_evidence_requires_pass_status_to_have_success_reason_and_checks() {
        let mut run = sample_run();
        run.status = RestoreRehearsalStatus::Passed;
        run.reason_code = RecoveryReasonCode::RehearsalSuccess.code().to_string();
        run.integrity_checks[0].passed = false;
        let error = validate_restore_rehearsal_run_evidence(&run)
            .expect_err("passed status with failed checks must be rejected");
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "integrity_checks")
        );
    }

    #[test]
    fn passed_rehearsal_cannot_report_deterministic_mismatch() {
        let mut run = sample_run();
        run.status = RestoreRehearsalStatus::Passed;
        run.reason_code = RecoveryReasonCode::RehearsalSuccess.code().to_string();
        run.deterministic_signature.deterministic_match = Some(false);
        run.deterministic_signature.mismatch_summary =
            Some("deterministic replay mismatch".to_string());
        let error = validate_restore_rehearsal_run_evidence(&run)
            .expect_err("passed rehearsal with deterministic mismatch must be rejected");
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "deterministic_signature.deterministic_match")
        );
    }

    fn sample_request() -> RestoreRehearsalRequest {
        RestoreRehearsalRequest {
            actor_id: "ops-1".to_string(),
            actor_role: "operational_control".to_string(),
            correlation_id: "corr-rehearsal-1".to_string(),
            requested_at_utc: "2026-04-06T12:00:00Z".to_string(),
            artifact_id: "artifact-1".to_string(),
            artifact_checksum: "a".repeat(64),
            restore_target: "sandbox-a".to_string(),
            reconciliation_run_id: "recon-1".to_string(),
            restore_output: serde_json::json!({"result": "ok"}),
            observed_checksum: Some("a".repeat(64)),
            incident_correlation_id: Some("incident-corr-1".to_string()),
            incident_severity: Some("severity_1".to_string()),
            audit_reference: Some("arb-2026-3008".to_string()),
        }
    }

    fn sample_run() -> RestoreRehearsalRunEvidence {
        RestoreRehearsalRunEvidence {
            run_id: "recovery::rehearsal::artifact-1::corr-rehearsal-1::20260406120000".to_string(),
            correlation_id: "corr-rehearsal-1".to_string(),
            artifact_id: "artifact-1".to_string(),
            artifact_checksum: "a".repeat(64),
            observed_checksum: "a".repeat(64),
            restore_target: "sandbox-a".to_string(),
            reconciliation_run_id: "recon-1".to_string(),
            reconciliation_mismatch_rate: Some(0.0001),
            reconciliation_passed: true,
            status: RestoreRehearsalStatus::Passed,
            reason_code: RecoveryReasonCode::RehearsalSuccess.code().to_string(),
            requested_at_utc: "2026-04-06T12:00:00Z".to_string(),
            started_at_utc: "2026-04-06T12:00:00Z".to_string(),
            completed_at_utc: "2026-04-06T12:00:03Z".to_string(),
            integrity_checks: vec![
                BackupIntegrityCheckItem {
                    check_name: "checksum_match".to_string(),
                    passed: true,
                    reason_code: RecoveryReasonCode::RehearsalSuccess.code().to_string(),
                    expected_value: Some("a".repeat(64)),
                    observed_value: Some("a".repeat(64)),
                    details: "artifact checksum matched restore output checksum".to_string(),
                },
                BackupIntegrityCheckItem {
                    check_name: "reconciliation_sanity".to_string(),
                    passed: true,
                    reason_code: RecoveryReasonCode::RehearsalSuccess.code().to_string(),
                    expected_value: Some("mismatch_rate<0.001".to_string()),
                    observed_value: Some("0.0001".to_string()),
                    details: "reconciliation sanity threshold passed".to_string(),
                },
            ],
            deterministic_signature: DeterministicReplaySignatureEvidence {
                deterministic_signature: "b".repeat(64),
                prior_deterministic_signature: Some("b".repeat(64)),
                deterministic_match: Some(true),
                mismatch_summary: None,
            },
            incident_correlation_id: Some("incident-corr-1".to_string()),
            incident_severity: Some("severity_1".to_string()),
            audit_reference: Some("arb-2026-3008".to_string()),
        }
    }
}
