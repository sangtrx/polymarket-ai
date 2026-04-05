use domain::governance::PrivilegedAuditRecord;
use serde::Serialize;
use serde_json::{Map, Value};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::sync::{Arc, Mutex};
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditAppendError {
    pub code: &'static str,
    pub message: String,
}

impl AuditAppendError {
    pub fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: "audit_invalid_payload",
            message: message.into(),
        }
    }

    pub fn persistence_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: "audit_persistence_unavailable",
            message: message.into(),
        }
    }

    pub fn append_constraint_violation(message: impl Into<String>) -> Self {
        Self {
            code: "audit_append_constraint_violation",
            message: message.into(),
        }
    }
}

impl Display for AuditAppendError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for AuditAppendError {}

pub trait AuditAppendPort: Send + Sync {
    fn append_record(&self, record: &PrivilegedAuditRecord) -> Result<(), AuditAppendError>;
}

pub trait PrivilegedAuditAppender: Send + Sync {
    fn append_privileged_audit(
        &self,
        record: PrivilegedAuditRecord,
    ) -> Result<(), AuditAppendError>;
}

#[derive(Debug, Default)]
pub struct InMemoryAuditAppendPort {
    records: Arc<Mutex<Vec<PrivilegedAuditRecord>>>,
}

impl InMemoryAuditAppendPort {
    pub fn from_shared_records(records: Arc<Mutex<Vec<PrivilegedAuditRecord>>>) -> Self {
        Self { records }
    }

    pub fn snapshot(&self) -> Vec<PrivilegedAuditRecord> {
        self.records
            .lock()
            .expect("in-memory audit append records lock should not be poisoned")
            .clone()
    }
}

impl AuditAppendPort for InMemoryAuditAppendPort {
    fn append_record(&self, record: &PrivilegedAuditRecord) -> Result<(), AuditAppendError> {
        self.records
            .lock()
            .expect("in-memory audit append records lock should not be poisoned")
            .push(record.clone());
        Ok(())
    }
}

#[derive(Clone)]
pub struct GovernanceAuditService {
    append_port: Arc<dyn AuditAppendPort>,
}

impl GovernanceAuditService {
    pub fn new(append_port: Arc<dyn AuditAppendPort>) -> Self {
        Self { append_port }
    }
}

impl PrivilegedAuditAppender for GovernanceAuditService {
    fn append_privileged_audit(
        &self,
        mut record: PrivilegedAuditRecord,
    ) -> Result<(), AuditAppendError> {
        record.parameters = redact_sensitive_parameters(record.parameters);
        validate_record(&record)?;

        if let Err(error) = self.append_port.append_record(&record) {
            emit_append_telemetry(&record, "failed", Some(error.code));
            return Err(error);
        }

        emit_append_telemetry(&record, "succeeded", None);
        Ok(())
    }
}

fn redact_sensitive_parameters(parameters: Value) -> Value {
    match parameters {
        Value::Object(map) => Value::Object(redact_object(map)),
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(redact_sensitive_parameters)
                .collect::<Vec<_>>(),
        ),
        scalar => scalar,
    }
}

fn redact_object(map: Map<String, Value>) -> Map<String, Value> {
    map.into_iter()
        .map(|(key, value)| {
            if is_sensitive_key(&key) {
                (key, Value::String("[REDACTED]".to_string()))
            } else {
                (key, redact_sensitive_parameters(value))
            }
        })
        .collect()
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase();
    [
        "authorization",
        "credential",
        "password",
        "private_key",
        "secret",
        "token",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
}

fn validate_record(record: &PrivilegedAuditRecord) -> Result<(), AuditAppendError> {
    if record.actor_id.trim().is_empty() {
        return Err(AuditAppendError::invalid_payload("actor_id is required"));
    }
    if record.role.trim().is_empty() {
        return Err(AuditAppendError::invalid_payload("role is required"));
    }
    if record.action_type.trim().is_empty() {
        return Err(AuditAppendError::invalid_payload("action_type is required"));
    }
    if !record.parameters.is_object() {
        return Err(AuditAppendError::invalid_payload(
            "parameters must be a JSON object",
        ));
    }
    if let Some(approval_reference) = &record.approval_reference
        && approval_reference.trim().is_empty()
    {
        return Err(AuditAppendError::invalid_payload(
            "approval_reference cannot be blank when provided",
        ));
    }
    if record.reason_code.trim().is_empty() {
        return Err(AuditAppendError::invalid_payload("reason_code is required"));
    }
    if record.correlation_id.trim().is_empty() {
        return Err(AuditAppendError::invalid_payload(
            "correlation_id is required",
        ));
    }
    if record.authentication_outcome.trim().is_empty() {
        return Err(AuditAppendError::invalid_payload(
            "authentication_outcome is required",
        ));
    }
    let timestamp = OffsetDateTime::parse(&record.timestamp, &Rfc3339).map_err(|error| {
        AuditAppendError::invalid_payload(format!("timestamp must be RFC3339 UTC: {error}"))
    })?;
    if timestamp.offset() != UtcOffset::UTC {
        return Err(AuditAppendError::invalid_payload(
            "timestamp must use UTC offset `Z`",
        ));
    }
    Ok(())
}

fn emit_append_telemetry(
    record: &PrivilegedAuditRecord,
    append_status: &str,
    error_code: Option<&str>,
) {
    let telemetry_event = AuditAppendTelemetryEvent {
        event_name: "privileged_audit_append_v1",
        actor_id: &record.actor_id,
        role: &record.role,
        action_type: &record.action_type,
        outcome: record.outcome.as_str(),
        reason_code: &record.reason_code,
        correlation_id: &record.correlation_id,
        append_status,
        error_code,
        timestamp: &record.timestamp,
    };

    println!(
        "{}",
        serde_json::to_string(&telemetry_event)
            .expect("audit append telemetry event should serialize")
    );
}

#[derive(Debug, Serialize)]
struct AuditAppendTelemetryEvent<'a> {
    event_name: &'a str,
    actor_id: &'a str,
    role: &'a str,
    action_type: &'a str,
    outcome: &'a str,
    reason_code: &'a str,
    correlation_id: &'a str,
    append_status: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_code: Option<&'a str>,
    timestamp: &'a str,
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::governance::{PrivilegedAuditOutcome, PrivilegedAuditRecord};
    use serde_json::json;

    fn sample_record() -> PrivilegedAuditRecord {
        PrivilegedAuditRecord {
            actor_id: "ops-1".to_string(),
            role: "operational_control".to_string(),
            action_type: "execute_control_plane_action".to_string(),
            parameters: json!({
                "endpoint": "/control/rebalance",
                "authorization_token": "top-secret-token",
                "nested": {
                    "api_secret": "should-not-leak"
                }
            }),
            approval_reference: None,
            timestamp: "2026-04-05T00:00:00Z".to_string(),
            outcome: PrivilegedAuditOutcome::Allow,
            reason_code: "authorization_allowed".to_string(),
            authentication_outcome: "authenticated".to_string(),
            correlation_id: "corr-audit-append-001".to_string(),
        }
    }

    #[test]
    fn append_privileged_audit_redacts_sensitive_parameter_boundaries() {
        let shared_records = Arc::new(Mutex::new(Vec::new()));
        let append_port = Arc::new(InMemoryAuditAppendPort::from_shared_records(
            shared_records.clone(),
        ));
        let service = GovernanceAuditService::new(append_port);

        service
            .append_privileged_audit(sample_record())
            .expect("append should succeed");

        let stored = shared_records
            .lock()
            .expect("shared records lock should not be poisoned")
            .clone();
        assert_eq!(stored.len(), 1);
        let parameters = &stored[0].parameters;
        assert_eq!(parameters["authorization_token"], "[REDACTED]");
        assert_eq!(parameters["nested"]["api_secret"], "[REDACTED]");
    }

    #[test]
    fn append_privileged_audit_rejects_invalid_payloads() {
        let service = GovernanceAuditService::new(Arc::new(InMemoryAuditAppendPort::default()));
        let mut record = sample_record();
        record.parameters = json!(["invalid", "shape"]);

        let error = service
            .append_privileged_audit(record)
            .expect_err("invalid payload must fail");
        assert_eq!(error.code, "audit_invalid_payload");
    }

    #[test]
    fn append_privileged_audit_rejects_non_utc_timestamp() {
        let service = GovernanceAuditService::new(Arc::new(InMemoryAuditAppendPort::default()));
        let mut record = sample_record();
        record.timestamp = "2026-04-05T01:00:00+01:00".to_string();

        let error = service
            .append_privileged_audit(record)
            .expect_err("non-utc timestamp must fail");
        assert_eq!(error.code, "audit_invalid_payload");
    }

    #[derive(Debug)]
    struct FailingPort;

    impl AuditAppendPort for FailingPort {
        fn append_record(&self, _record: &PrivilegedAuditRecord) -> Result<(), AuditAppendError> {
            Err(AuditAppendError::persistence_unavailable(
                "database unavailable",
            ))
        }
    }

    #[test]
    fn append_privileged_audit_propagates_machine_readable_port_errors() {
        let service = GovernanceAuditService::new(Arc::new(FailingPort));

        let error = service
            .append_privileged_audit(sample_record())
            .expect_err("port errors should be returned");
        assert_eq!(error.code, "audit_persistence_unavailable");
    }
}
