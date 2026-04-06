use domain::risk::{
    EmergencyControlAction, EmergencyControlMode, EmergencyControlReasonCode,
    EmergencyControlSource, EmergencyControlTriggerSource, EmergencyControlValidationIssue,
    SafetyControlActionRecord, normalize_emergency_control_identifier,
    validate_safety_control_action_record,
};
use persistence::postgres::safety_controls::{
    EffectiveSafetyControlMode, SafetyControlPersistenceError,
    insert_safety_control_action as pg_insert_safety_control_action,
    load_current_effective_safety_mode as pg_load_current_effective_safety_mode,
    load_latest_safety_control_action_by_action_id as pg_load_latest_by_action_id,
    load_latest_safety_control_action_by_correlation as pg_load_latest_by_correlation,
};
use serde::Serialize;
use sqlx::PgPool;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SafetyControlServiceError {
    pub code: &'static str,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub field_errors: Vec<EmergencyControlValidationIssue>,
}

impl SafetyControlServiceError {
    pub fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<EmergencyControlValidationIssue>,
    ) -> Self {
        Self {
            code: EmergencyControlReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn unauthorized_role() -> Self {
        Self {
            code: EmergencyControlReasonCode::UnauthorizedRole.code(),
            message: "actor role is not authorized for emergency controls".to_string(),
            field_errors: Vec::new(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            code: EmergencyControlReasonCode::NotFound.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn orchestration_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: EmergencyControlReasonCode::OrchestrationUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    fn persistence_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: EmergencyControlReasonCode::PersistenceUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }
}

impl Display for SafetyControlServiceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for SafetyControlServiceError {}

fn map_persistence_error(error: SafetyControlPersistenceError) -> SafetyControlServiceError {
    match error.code {
        "safety_control_query_failed"
        | "safety_control_row_decode_failed"
        | "safety_control_runtime_unavailable" => {
            SafetyControlServiceError::persistence_unavailable(error.message)
        }
        _ => SafetyControlServiceError {
            code: error.code,
            message: error.message,
            field_errors: error.field_errors,
        },
    }
}

#[derive(Debug, Clone)]
pub struct ExecuteManualSafetyControlInput {
    pub action: EmergencyControlAction,
    pub actor_id: String,
    pub actor_role: String,
    pub correlation_id: String,
    pub requested_at_utc: String,
    pub audit_reference: Option<String>,
}

#[derive(Debug, Clone)]
pub struct HandleAutomaticSafetyTriggerInput {
    pub trigger_source: EmergencyControlTriggerSource,
    pub correlation_id: String,
    pub requested_at_utc: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EffectiveSafetyControlModeEvidence {
    pub action_id: String,
    pub correlation_id: String,
    pub resulting_mode: String,
    pub reason_code: String,
    pub effective_at_utc: String,
}

pub trait SafetyControlRepositoryPort: Send + Sync {
    fn insert_action(
        &self,
        action: SafetyControlActionRecord,
    ) -> Result<(), SafetyControlServiceError>;
    fn load_latest_by_action_id(
        &self,
        action_id: &str,
    ) -> Result<Option<SafetyControlActionRecord>, SafetyControlServiceError>;
    fn load_latest_by_correlation(
        &self,
        correlation_id: &str,
    ) -> Result<Option<SafetyControlActionRecord>, SafetyControlServiceError>;
    fn load_current_mode(
        &self,
    ) -> Result<Option<EffectiveSafetyControlMode>, SafetyControlServiceError>;
}

pub trait EmergencyExecutionContainmentPort: Send + Sync {
    fn execute_cancel_all(
        &self,
        correlation_id: &str,
        requested_at_utc: &str,
    ) -> Result<(), SafetyControlServiceError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoopEmergencyExecutionContainmentPort;

impl EmergencyExecutionContainmentPort for NoopEmergencyExecutionContainmentPort {
    fn execute_cancel_all(
        &self,
        _correlation_id: &str,
        _requested_at_utc: &str,
    ) -> Result<(), SafetyControlServiceError> {
        Ok(())
    }
}

pub trait SafetyControlOrchestrator: Send + Sync {
    fn execute_manual_control(
        &self,
        input: ExecuteManualSafetyControlInput,
    ) -> Result<SafetyControlActionRecord, SafetyControlServiceError>;
    fn handle_automatic_trigger(
        &self,
        input: HandleAutomaticSafetyTriggerInput,
    ) -> Result<SafetyControlActionRecord, SafetyControlServiceError>;
    fn get_action_result(
        &self,
        action_id: &str,
    ) -> Result<SafetyControlActionRecord, SafetyControlServiceError>;
    fn get_action_result_by_correlation(
        &self,
        correlation_id: &str,
    ) -> Result<Option<SafetyControlActionRecord>, SafetyControlServiceError>;
    fn get_current_mode(
        &self,
    ) -> Result<Option<EffectiveSafetyControlModeEvidence>, SafetyControlServiceError>;
}

#[derive(Clone)]
pub struct SafetyControlService {
    repository: Arc<dyn SafetyControlRepositoryPort>,
    containment_port: Arc<dyn EmergencyExecutionContainmentPort>,
    operation_lock: Arc<Mutex<()>>,
}

impl SafetyControlService {
    pub fn new(
        repository: Arc<dyn SafetyControlRepositoryPort>,
        containment_port: Arc<dyn EmergencyExecutionContainmentPort>,
    ) -> Self {
        Self {
            repository,
            containment_port,
            operation_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn in_memory() -> Self {
        Self::new(
            Arc::new(InMemorySafetyControlRepository::default()),
            Arc::new(NoopEmergencyExecutionContainmentPort),
        )
    }

    pub fn postgres(pool: PgPool) -> Self {
        Self::new(
            Arc::new(PostgresSafetyControlRepository::new(pool)),
            Arc::new(NoopEmergencyExecutionContainmentPort),
        )
    }

    pub fn postgres_with_containment(
        pool: PgPool,
        containment_port: Arc<dyn EmergencyExecutionContainmentPort>,
    ) -> Self {
        Self::new(
            Arc::new(PostgresSafetyControlRepository::new(pool)),
            containment_port,
        )
    }

    fn lock_operations(&self) -> Result<std::sync::MutexGuard<'_, ()>, SafetyControlServiceError> {
        self.operation_lock.lock().map_err(|_| {
            SafetyControlServiceError::persistence_unavailable(
                "safety control operation lock poisoned by prior panic",
            )
        })
    }
}

impl Default for SafetyControlService {
    fn default() -> Self {
        Self::in_memory()
    }
}

impl SafetyControlOrchestrator for SafetyControlService {
    fn execute_manual_control(
        &self,
        input: ExecuteManualSafetyControlInput,
    ) -> Result<SafetyControlActionRecord, SafetyControlServiceError> {
        validate_manual_role(&input.actor_role)?;
        validate_non_empty("actor_id", &input.actor_id)?;
        validate_non_empty("correlation_id", &input.correlation_id)?;
        validate_non_empty("requested_at_utc", &input.requested_at_utc)?;
        let _lock = self.lock_operations()?;
        let action = build_action_record(ActionRecordBuildInput {
            action: input.action,
            source: EmergencyControlSource::Manual,
            trigger_source: EmergencyControlTriggerSource::OperatorCommand,
            actor_id: Some(input.actor_id),
            actor_role: Some(input.actor_role),
            correlation_id: normalize_emergency_control_identifier(&input.correlation_id),
            requested_at_utc: input.requested_at_utc,
            audit_reference: input.audit_reference,
        })?;

        if let Some(existing) = self
            .repository
            .load_latest_by_correlation(&action.correlation_id)?
            .filter(|existing| existing.dedupe_key == action.dedupe_key)
        {
            return Ok(existing);
        }

        if matches!(action.action, EmergencyControlAction::CancelAll) {
            self.containment_port
                .execute_cancel_all(&action.correlation_id, &action.requested_at_utc)?;
        }

        self.repository.insert_action(action.clone())?;
        emit_safety_control_telemetry("governance_manual_emergency_control_v1", &action);
        Ok(action)
    }

    fn handle_automatic_trigger(
        &self,
        input: HandleAutomaticSafetyTriggerInput,
    ) -> Result<SafetyControlActionRecord, SafetyControlServiceError> {
        validate_non_empty("correlation_id", &input.correlation_id)?;
        validate_non_empty("requested_at_utc", &input.requested_at_utc)?;
        if input.trigger_source == EmergencyControlTriggerSource::OperatorCommand {
            return Err(SafetyControlServiceError::invalid_payload(
                "automatic trigger cannot use operator_command source",
                Vec::new(),
            ));
        }

        let _lock = self.lock_operations()?;
        let action = build_action_record(ActionRecordBuildInput {
            action: EmergencyControlAction::Pause,
            source: EmergencyControlSource::Automatic,
            trigger_source: input.trigger_source,
            actor_id: None,
            actor_role: None,
            correlation_id: normalize_emergency_control_identifier(&input.correlation_id),
            requested_at_utc: input.requested_at_utc,
            audit_reference: None,
        })?;
        self.repository.insert_action(action.clone())?;
        emit_safety_control_telemetry("governance_automatic_safe_state_trigger_v1", &action);
        Ok(action)
    }

    fn get_action_result(
        &self,
        action_id: &str,
    ) -> Result<SafetyControlActionRecord, SafetyControlServiceError> {
        validate_non_empty("action_id", action_id)?;
        let normalized = normalize_emergency_control_identifier(action_id);
        self.repository
            .load_latest_by_action_id(&normalized)?
            .ok_or_else(|| {
                SafetyControlServiceError::not_found(format!(
                    "safety control action `{normalized}` was not found"
                ))
            })
    }

    fn get_action_result_by_correlation(
        &self,
        correlation_id: &str,
    ) -> Result<Option<SafetyControlActionRecord>, SafetyControlServiceError> {
        validate_non_empty("correlation_id", correlation_id)?;
        self.repository
            .load_latest_by_correlation(&normalize_emergency_control_identifier(correlation_id))
    }

    fn get_current_mode(
        &self,
    ) -> Result<Option<EffectiveSafetyControlModeEvidence>, SafetyControlServiceError> {
        let current = self.repository.load_current_mode()?;
        Ok(current.map(|mode| EffectiveSafetyControlModeEvidence {
            action_id: mode.action_id,
            correlation_id: mode.correlation_id,
            resulting_mode: mode.resulting_mode.as_str().to_string(),
            reason_code: mode.reason_code,
            effective_at_utc: mode.effective_at_utc,
        }))
    }
}

struct ActionRecordBuildInput {
    action: EmergencyControlAction,
    source: EmergencyControlSource,
    trigger_source: EmergencyControlTriggerSource,
    actor_id: Option<String>,
    actor_role: Option<String>,
    correlation_id: String,
    requested_at_utc: String,
    audit_reference: Option<String>,
}

fn build_action_record(
    input: ActionRecordBuildInput,
) -> Result<SafetyControlActionRecord, SafetyControlServiceError> {
    let ActionRecordBuildInput {
        action,
        source,
        trigger_source,
        actor_id,
        actor_role,
        correlation_id,
        requested_at_utc,
        audit_reference,
    } = input;
    let resulting_mode = match action {
        EmergencyControlAction::Pause => EmergencyControlMode::Paused,
        EmergencyControlAction::ReduceOnly => EmergencyControlMode::ReduceOnly,
        EmergencyControlAction::CancelAll => EmergencyControlMode::Paused,
    };
    if source == EmergencyControlSource::Automatic && resulting_mode != EmergencyControlMode::Paused
    {
        return Err(SafetyControlServiceError::invalid_payload(
            "automatic triggers must transition to paused mode",
            Vec::new(),
        ));
    }
    let reason_code = match source {
        EmergencyControlSource::Manual => match action {
            EmergencyControlAction::Pause => EmergencyControlReasonCode::PauseActivated,
            EmergencyControlAction::ReduceOnly => EmergencyControlReasonCode::ReduceOnlyActivated,
            EmergencyControlAction::CancelAll => EmergencyControlReasonCode::CancelAllAccepted,
        },
        EmergencyControlSource::Automatic => match trigger_source {
            EmergencyControlTriggerSource::StaleFeed => {
                EmergencyControlReasonCode::StaleFeedTriggered
            }
            EmergencyControlTriggerSource::ReconciliationCritical => {
                EmergencyControlReasonCode::ReconciliationCriticalTriggered
            }
            EmergencyControlTriggerSource::ControlUncertainty => {
                EmergencyControlReasonCode::ControlUncertaintyTriggered
            }
            EmergencyControlTriggerSource::OperatorCommand => {
                return Err(SafetyControlServiceError::invalid_payload(
                    "automatic triggers cannot use operator_command",
                    Vec::new(),
                ));
            }
        },
    };
    let compact_timestamp = compact_utc_timestamp_token(&requested_at_utc);
    let action_id = match source {
        EmergencyControlSource::Manual => format!(
            "emergency::manual::{}::{}::{}",
            action.as_str(),
            correlation_id,
            compact_timestamp
        ),
        EmergencyControlSource::Automatic => format!(
            "emergency::automatic::{}::{}::{}",
            trigger_source.as_str(),
            correlation_id,
            compact_timestamp
        ),
    };
    let dedupe_key = match source {
        EmergencyControlSource::Manual => {
            format!("manual::{}::{}", action.as_str(), correlation_id)
        }
        EmergencyControlSource::Automatic => {
            format!("automatic::{}::{}", trigger_source.as_str(), correlation_id)
        }
    };

    let action_record = SafetyControlActionRecord {
        action_id,
        source,
        action,
        trigger_source,
        actor_id: actor_id.map(|value| normalize_emergency_control_identifier(&value)),
        actor_role: actor_role.map(|value| normalize_emergency_control_identifier(&value)),
        resulting_mode,
        reason_code: reason_code.code().to_string(),
        correlation_id,
        audit_reference: audit_reference
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| format!("audit::safety-control::{compact_timestamp}")),
        dedupe_key,
        requested_at_utc: requested_at_utc.clone(),
        acknowledged_at_utc: requested_at_utc.clone(),
        effective_at_utc: requested_at_utc.clone(),
        completed_at_utc: requested_at_utc,
    };
    validate_safety_control_action_record(&action_record).map_err(|error| {
        SafetyControlServiceError::invalid_payload(error.message, error.field_errors)
    })?;
    Ok(action_record)
}

fn compact_utc_timestamp_token(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_digit())
        .collect()
}

fn validate_manual_role(role: &str) -> Result<(), SafetyControlServiceError> {
    match role {
        "operational_control" | "administrative_actions" => Ok(()),
        _ => Err(SafetyControlServiceError::unauthorized_role()),
    }
}

fn validate_non_empty(field: &'static str, value: &str) -> Result<(), SafetyControlServiceError> {
    if value.trim().is_empty() {
        return Err(SafetyControlServiceError::invalid_payload(
            format!("{field} cannot be blank"),
            Vec::new(),
        ));
    }
    Ok(())
}

fn emit_safety_control_telemetry(event_name: &'static str, action: &SafetyControlActionRecord) {
    let event = SafetyControlTelemetryEvent {
        event_name,
        action_id: &action.action_id,
        source: action.source.as_str(),
        action: action.action.as_str(),
        trigger_source: action.trigger_source.as_str(),
        resulting_mode: action.resulting_mode.as_str(),
        reason_code: &action.reason_code,
        correlation_id: &action.correlation_id,
        actor_id: action.actor_id.as_deref(),
        actor_role: action.actor_role.as_deref(),
        audit_reference: &action.audit_reference,
        timestamp_utc: &action.completed_at_utc,
    };
    println!(
        "{}",
        serde_json::to_string(&event).expect("safety control telemetry event should serialize")
    );
}

#[derive(Debug, Serialize)]
struct SafetyControlTelemetryEvent<'a> {
    event_name: &'a str,
    action_id: &'a str,
    source: &'a str,
    action: &'a str,
    trigger_source: &'a str,
    resulting_mode: &'a str,
    reason_code: &'a str,
    correlation_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    actor_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    actor_role: Option<&'a str>,
    audit_reference: &'a str,
    timestamp_utc: &'a str,
}

#[derive(Debug, Clone)]
pub struct PostgresSafetyControlRepository {
    pool: PgPool,
}

impl PostgresSafetyControlRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn run_with_runtime<F, T>(&self, future: F) -> Result<T, SafetyControlServiceError>
    where
        F: Future<Output = Result<T, SafetyControlPersistenceError>>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future))
                .map_err(map_persistence_error),
            Err(_) => tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| {
                    SafetyControlServiceError::persistence_unavailable(format!(
                        "failed to initialize async runtime: {error}"
                    ))
                })?
                .block_on(future)
                .map_err(map_persistence_error),
        }
    }
}

impl SafetyControlRepositoryPort for PostgresSafetyControlRepository {
    fn insert_action(
        &self,
        action: SafetyControlActionRecord,
    ) -> Result<(), SafetyControlServiceError> {
        self.run_with_runtime(pg_insert_safety_control_action(&self.pool, &action))
    }

    fn load_latest_by_action_id(
        &self,
        action_id: &str,
    ) -> Result<Option<SafetyControlActionRecord>, SafetyControlServiceError> {
        self.run_with_runtime(pg_load_latest_by_action_id(&self.pool, action_id))
    }

    fn load_latest_by_correlation(
        &self,
        correlation_id: &str,
    ) -> Result<Option<SafetyControlActionRecord>, SafetyControlServiceError> {
        self.run_with_runtime(pg_load_latest_by_correlation(&self.pool, correlation_id))
    }

    fn load_current_mode(
        &self,
    ) -> Result<Option<EffectiveSafetyControlMode>, SafetyControlServiceError> {
        self.run_with_runtime(pg_load_current_effective_safety_mode(&self.pool))
    }
}

#[derive(Debug, Default)]
pub struct InMemorySafetyControlRepository {
    actions: Mutex<BTreeMap<String, SafetyControlActionRecord>>,
}

impl SafetyControlRepositoryPort for InMemorySafetyControlRepository {
    fn insert_action(
        &self,
        action: SafetyControlActionRecord,
    ) -> Result<(), SafetyControlServiceError> {
        let mut actions = self
            .actions
            .lock()
            .expect("in-memory safety control map should not be poisoned");
        actions.insert(action.action_id.clone(), action);
        Ok(())
    }

    fn load_latest_by_action_id(
        &self,
        action_id: &str,
    ) -> Result<Option<SafetyControlActionRecord>, SafetyControlServiceError> {
        let actions = self
            .actions
            .lock()
            .expect("in-memory safety control map should not be poisoned");
        Ok(actions.get(action_id).cloned())
    }

    fn load_latest_by_correlation(
        &self,
        correlation_id: &str,
    ) -> Result<Option<SafetyControlActionRecord>, SafetyControlServiceError> {
        let actions = self
            .actions
            .lock()
            .expect("in-memory safety control map should not be poisoned");
        let normalized = normalize_emergency_control_identifier(correlation_id);
        Ok(actions
            .values()
            .filter(|action| action.correlation_id == normalized)
            .max_by(|left, right| {
                left.effective_at_utc
                    .cmp(&right.effective_at_utc)
                    .then(left.action_id.cmp(&right.action_id))
            })
            .cloned())
    }

    fn load_current_mode(
        &self,
    ) -> Result<Option<EffectiveSafetyControlMode>, SafetyControlServiceError> {
        let actions = self
            .actions
            .lock()
            .expect("in-memory safety control map should not be poisoned");
        Ok(actions
            .values()
            .max_by(|left, right| {
                left.effective_at_utc
                    .cmp(&right.effective_at_utc)
                    .then(left.action_id.cmp(&right.action_id))
            })
            .map(|action| EffectiveSafetyControlMode {
                action_id: action.action_id.clone(),
                correlation_id: action.correlation_id.clone(),
                resulting_mode: action.resulting_mode,
                reason_code: action.reason_code.clone(),
                effective_at_utc: action.effective_at_utc.clone(),
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Debug, Default, Clone)]
    struct RecordingContainmentPort {
        calls: Arc<Mutex<Vec<(String, String)>>>,
        fail: bool,
    }

    impl RecordingContainmentPort {
        fn calls(&self) -> Vec<(String, String)> {
            self.calls
                .lock()
                .expect("containment calls lock should not be poisoned")
                .clone()
        }
    }

    impl EmergencyExecutionContainmentPort for RecordingContainmentPort {
        fn execute_cancel_all(
            &self,
            correlation_id: &str,
            requested_at_utc: &str,
        ) -> Result<(), SafetyControlServiceError> {
            if self.fail {
                return Err(SafetyControlServiceError::orchestration_unavailable(
                    "execution containment dependency unavailable",
                ));
            }
            self.calls
                .lock()
                .expect("containment calls lock should not be poisoned")
                .push((correlation_id.to_string(), requested_at_utc.to_string()));
            Ok(())
        }
    }

    fn service_with_containment(
        containment_port: Arc<dyn EmergencyExecutionContainmentPort>,
    ) -> SafetyControlService {
        SafetyControlService::new(
            Arc::new(InMemorySafetyControlRepository::default()),
            containment_port,
        )
    }

    #[test]
    fn manual_control_rejects_unauthorized_role() {
        let service = SafetyControlService::default();
        let error = service
            .execute_manual_control(ExecuteManualSafetyControlInput {
                action: EmergencyControlAction::Pause,
                actor_id: "reader-1".to_string(),
                actor_role: "read_only_analytics".to_string(),
                correlation_id: "corr-safety-authz-001".to_string(),
                requested_at_utc: "2026-04-06T10:00:00Z".to_string(),
                audit_reference: None,
            })
            .expect_err("unauthorized role should fail closed");
        assert_eq!(
            error.code,
            EmergencyControlReasonCode::UnauthorizedRole.code()
        );
    }

    #[test]
    fn manual_pause_control_persists_auditable_evidence() {
        let service = SafetyControlService::default();
        let action = service
            .execute_manual_control(ExecuteManualSafetyControlInput {
                action: EmergencyControlAction::Pause,
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                correlation_id: "corr-safety-manual-001".to_string(),
                requested_at_utc: "2026-04-06T10:00:00Z".to_string(),
                audit_reference: Some("audit::manual::001".to_string()),
            })
            .expect("manual pause should persist");

        assert_eq!(action.source, EmergencyControlSource::Manual);
        assert_eq!(action.action, EmergencyControlAction::Pause);
        assert_eq!(action.resulting_mode, EmergencyControlMode::Paused);
        assert_eq!(
            action.reason_code,
            EmergencyControlReasonCode::PauseActivated.code()
        );
        assert_eq!(action.audit_reference, "audit::manual::001");
        let loaded = service
            .get_action_result(&action.action_id)
            .expect("persisted action should be queryable");
        assert_eq!(loaded.action_id, action.action_id);
    }

    #[test]
    fn automatic_trigger_transitions_to_paused_with_trigger_reason() {
        let service = SafetyControlService::default();
        let action = service
            .handle_automatic_trigger(HandleAutomaticSafetyTriggerInput {
                trigger_source: EmergencyControlTriggerSource::StaleFeed,
                correlation_id: "corr-safety-auto-001".to_string(),
                requested_at_utc: "2026-04-06T10:00:01Z".to_string(),
            })
            .expect("automatic trigger should persist paused transition");

        assert_eq!(action.source, EmergencyControlSource::Automatic);
        assert_eq!(action.action, EmergencyControlAction::Pause);
        assert_eq!(action.resulting_mode, EmergencyControlMode::Paused);
        assert_eq!(
            action.reason_code,
            EmergencyControlReasonCode::StaleFeedTriggered.code()
        );
    }

    #[test]
    fn cancel_all_manual_control_routes_through_execution_containment_port() {
        let containment = Arc::new(RecordingContainmentPort::default());
        let service = service_with_containment(containment.clone());
        let action = service
            .execute_manual_control(ExecuteManualSafetyControlInput {
                action: EmergencyControlAction::CancelAll,
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                correlation_id: "corr-safety-cancel-all-001".to_string(),
                requested_at_utc: "2026-04-06T10:00:02Z".to_string(),
                audit_reference: None,
            })
            .expect("cancel-all should route through containment port");

        assert_eq!(action.action, EmergencyControlAction::CancelAll);
        assert_eq!(
            action.reason_code,
            EmergencyControlReasonCode::CancelAllAccepted.code()
        );
        assert_eq!(
            containment.calls(),
            vec![(
                "corr-safety-cancel-all-001".to_string(),
                "2026-04-06T10:00:02Z".to_string(),
            )]
        );
    }

    #[test]
    fn containment_port_failures_surface_orchestration_errors_without_persistence_side_effects() {
        let containment = Arc::new(RecordingContainmentPort {
            fail: true,
            ..Default::default()
        });
        let service = service_with_containment(containment.clone());
        let error = service
            .execute_manual_control(ExecuteManualSafetyControlInput {
                action: EmergencyControlAction::CancelAll,
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                correlation_id: "corr-safety-cancel-all-failure-001".to_string(),
                requested_at_utc: "2026-04-06T10:00:03Z".to_string(),
                audit_reference: None,
            })
            .expect_err("containment failures must be surfaced");
        assert_eq!(
            error.code,
            EmergencyControlReasonCode::OrchestrationUnavailable.code()
        );
        assert!(containment.calls().is_empty());
    }

    #[test]
    fn cancel_all_invalid_payload_fails_before_containment_side_effects() {
        let containment = Arc::new(RecordingContainmentPort::default());
        let service = service_with_containment(containment.clone());
        let error = service
            .execute_manual_control(ExecuteManualSafetyControlInput {
                action: EmergencyControlAction::CancelAll,
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                correlation_id: "corr-safety-invalid-payload-001".to_string(),
                requested_at_utc: "not-a-timestamp".to_string(),
                audit_reference: None,
            })
            .expect_err("invalid payloads must not execute containment side effects");
        assert_eq!(
            error.code,
            EmergencyControlReasonCode::InvalidPayload.code()
        );
        assert!(containment.calls().is_empty());
    }

    #[test]
    fn cancel_all_manual_control_reuses_existing_action_for_idempotent_retries() {
        let containment = Arc::new(RecordingContainmentPort::default());
        let service = service_with_containment(containment.clone());
        let first = service
            .execute_manual_control(ExecuteManualSafetyControlInput {
                action: EmergencyControlAction::CancelAll,
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                correlation_id: "corr-safety-idempotent-001".to_string(),
                requested_at_utc: "2026-04-06T10:00:06Z".to_string(),
                audit_reference: None,
            })
            .expect("first cancel-all action should succeed");
        let second = service
            .execute_manual_control(ExecuteManualSafetyControlInput {
                action: EmergencyControlAction::CancelAll,
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                correlation_id: "corr-safety-idempotent-001".to_string(),
                requested_at_utc: "2026-04-06T10:00:07Z".to_string(),
                audit_reference: Some("audit::manual::retry".to_string()),
            })
            .expect("idempotent retry should resolve to the original action");
        assert_eq!(first.action_id, second.action_id);
        assert_eq!(containment.calls().len(), 1);
    }

    #[test]
    fn current_mode_returns_latest_effective_action() {
        let service = SafetyControlService::default();
        let _ = service
            .execute_manual_control(ExecuteManualSafetyControlInput {
                action: EmergencyControlAction::ReduceOnly,
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                correlation_id: "corr-safety-mode-001".to_string(),
                requested_at_utc: "2026-04-06T10:00:04Z".to_string(),
                audit_reference: None,
            })
            .expect("reduce-only control should persist");
        let _ = service
            .execute_manual_control(ExecuteManualSafetyControlInput {
                action: EmergencyControlAction::Pause,
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                correlation_id: "corr-safety-mode-002".to_string(),
                requested_at_utc: "2026-04-06T10:00:05Z".to_string(),
                audit_reference: None,
            })
            .expect("pause control should persist");

        let current = service
            .get_current_mode()
            .expect("current mode lookup should succeed")
            .expect("current mode should be present");
        assert_eq!(
            current.resulting_mode,
            EmergencyControlMode::Paused.as_str()
        );
        assert_eq!(
            current.reason_code,
            EmergencyControlReasonCode::PauseActivated.code()
        );
    }
}
