use domain::governance::{
    CredentialRotationContractError, CredentialRotationDecisionOutcome, CredentialRotationEvidence,
    CredentialRotationReasonCode, CredentialRotationState, CredentialRotationTrigger,
    emergency_rotation_deadline_utc, emergency_rotation_within_deadline, scheduled_rotation_due,
    validate_rotation_metadata,
};
use persistence::postgres::credential_rotation::{
    CredentialRotationPersistenceError,
    create_credential_rotation_event as pg_create_credential_rotation_event,
    load_credential_rotation_event as pg_load_credential_rotation_event,
    query_credential_rotation_events as pg_query_credential_rotation_events,
    update_credential_rotation_event as pg_update_credential_rotation_event,
};
use serde::Serialize;
use serde_json::Value;
use sqlx::PgPool;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialRotationServiceError {
    pub code: &'static str,
    pub message: String,
}

impl CredentialRotationServiceError {
    pub fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: CredentialRotationReasonCode::InvalidPayload.code(),
            message: message.into(),
        }
    }

    pub fn missing_metadata(message: impl Into<String>) -> Self {
        Self {
            code: CredentialRotationReasonCode::MissingMetadata.code(),
            message: message.into(),
        }
    }

    pub fn from_contract_error(error: CredentialRotationContractError) -> Self {
        Self {
            code: error.code,
            message: error.message,
        }
    }

    pub fn unauthorized_role() -> Self {
        Self {
            code: CredentialRotationReasonCode::UnauthorizedRole.code(),
            message: "actor role is not authorized for credential rotation workflow".to_string(),
        }
    }

    pub fn persistence_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: "credential_rotation_persistence_unavailable",
            message: message.into(),
        }
    }
}

impl Display for CredentialRotationServiceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for CredentialRotationServiceError {}

fn map_persistence_error(
    error: CredentialRotationPersistenceError,
) -> CredentialRotationServiceError {
    match error.code {
        "credential_rotation_query_failed" | "credential_rotation_row_decode_failed" => {
            CredentialRotationServiceError::persistence_unavailable(error.message)
        }
        _ => CredentialRotationServiceError {
            code: error.code,
            message: error.message,
        },
    }
}

#[derive(Debug, Clone)]
pub struct TriggerScheduledRotationInput {
    pub actor_id: String,
    pub actor_role: String,
    pub credential_scope: String,
    pub credential_reference: String,
    pub metadata: Value,
    pub correlation_id: String,
    pub now_utc: String,
    pub last_rotated_at_utc: String,
}

#[derive(Debug, Clone)]
pub struct TriggerEmergencyRotationInput {
    pub actor_id: String,
    pub actor_role: String,
    pub credential_scope: String,
    pub credential_reference: String,
    pub metadata: Value,
    pub correlation_id: String,
    pub now_utc: String,
    pub compromise_triggered_at_utc: String,
}

pub trait CredentialRotationRepositoryPort: Send + Sync {
    fn create_event(
        &self,
        event: CredentialRotationEvidence,
    ) -> Result<(), CredentialRotationServiceError>;
    fn load_event(
        &self,
        rotation_id: &str,
    ) -> Result<Option<CredentialRotationEvidence>, CredentialRotationServiceError>;
    fn update_event(
        &self,
        event: CredentialRotationEvidence,
    ) -> Result<(), CredentialRotationServiceError>;
    fn query_events(
        &self,
        trigger_type: Option<CredentialRotationTrigger>,
        status: Option<CredentialRotationState>,
        limit: u32,
    ) -> Result<Vec<CredentialRotationEvidence>, CredentialRotationServiceError>;
}

#[derive(Debug, Clone)]
pub struct SecretRotationRequest {
    pub trigger_type: CredentialRotationTrigger,
    pub credential_scope: String,
    pub credential_reference: String,
    pub correlation_id: String,
    pub requested_at_utc: String,
    pub deadline_at_utc: Option<String>,
    pub metadata: Value,
}

#[derive(Debug, Clone)]
pub struct SecretRotationResult {
    pub rotation_reference: String,
    pub completed_at_utc: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretRotationPortErrorCode {
    ProviderUnavailable,
    RuntimeUnavailable,
}

impl SecretRotationPortErrorCode {
    pub const fn reason_code(self) -> CredentialRotationReasonCode {
        match self {
            Self::ProviderUnavailable => CredentialRotationReasonCode::ProviderUnavailable,
            Self::RuntimeUnavailable => CredentialRotationReasonCode::RuntimeUnavailable,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretRotationPortError {
    pub code: SecretRotationPortErrorCode,
    pub message: String,
}

impl SecretRotationPortError {
    pub fn provider_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: SecretRotationPortErrorCode::ProviderUnavailable,
            message: message.into(),
        }
    }

    pub fn runtime_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: SecretRotationPortErrorCode::RuntimeUnavailable,
            message: message.into(),
        }
    }
}

pub trait SecretRotationPort: Send + Sync {
    fn rotate(
        &self,
        request: SecretRotationRequest,
    ) -> Result<SecretRotationResult, SecretRotationPortError>;
}

pub trait CredentialRotationOrchestrator: Send + Sync {
    fn trigger_scheduled_rotation(
        &self,
        input: TriggerScheduledRotationInput,
    ) -> Result<CredentialRotationEvidence, CredentialRotationServiceError>;
    fn trigger_emergency_rotation(
        &self,
        input: TriggerEmergencyRotationInput,
    ) -> Result<CredentialRotationEvidence, CredentialRotationServiceError>;
}

#[derive(Clone)]
pub struct CredentialRotationService {
    repository: Arc<dyn CredentialRotationRepositoryPort>,
    secret_rotation_port: Arc<dyn SecretRotationPort>,
    operation_lock: Arc<Mutex<()>>,
}

impl CredentialRotationService {
    pub fn new(
        repository: Arc<dyn CredentialRotationRepositoryPort>,
        secret_rotation_port: Arc<dyn SecretRotationPort>,
    ) -> Self {
        Self {
            repository,
            secret_rotation_port,
            operation_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn in_memory() -> Self {
        Self::new(
            Arc::new(InMemoryCredentialRotationRepository::default()),
            Arc::new(InMemorySecretRotationPort),
        )
    }

    pub fn postgres(pool: PgPool) -> Self {
        Self::new(
            Arc::new(PostgresCredentialRotationRepository::new(pool)),
            Arc::new(InMemorySecretRotationPort),
        )
    }

    pub fn postgres_with_port(
        pool: PgPool,
        secret_rotation_port: Arc<dyn SecretRotationPort>,
    ) -> Self {
        Self::new(
            Arc::new(PostgresCredentialRotationRepository::new(pool)),
            secret_rotation_port,
        )
    }

    fn lock_operations(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, ()>, CredentialRotationServiceError> {
        self.operation_lock.lock().map_err(|_| {
            CredentialRotationServiceError::persistence_unavailable(
                "credential rotation operation lock poisoned by prior panic",
            )
        })
    }
}

impl Default for CredentialRotationService {
    fn default() -> Self {
        Self::in_memory()
    }
}

impl CredentialRotationOrchestrator for CredentialRotationService {
    fn trigger_scheduled_rotation(
        &self,
        input: TriggerScheduledRotationInput,
    ) -> Result<CredentialRotationEvidence, CredentialRotationServiceError> {
        validate_rotation_role(&input.actor_role)?;
        validate_common_inputs(
            &input.actor_id,
            &input.credential_scope,
            &input.credential_reference,
            &input.correlation_id,
            &input.now_utc,
            &input.metadata,
        )?;

        let rotation_id = generate_rotation_id(
            CredentialRotationTrigger::ScheduledCadence,
            &input.credential_scope,
            &input.correlation_id,
        );
        if let Some(existing) = self.repository.load_event(&rotation_id)? {
            return Ok(existing);
        }

        let due = scheduled_rotation_due(&input.last_rotated_at_utc, &input.now_utc)
            .map_err(|error| CredentialRotationServiceError::invalid_payload(error.message))?;

        let _lock = self.lock_operations()?;
        let pending = build_pending_event(
            &rotation_id,
            &input.actor_id,
            CredentialRotationTrigger::ScheduledCadence,
            &input.credential_scope,
            &input.credential_reference,
            &input.correlation_id,
            &input.now_utc,
            input.metadata.clone(),
            None,
        );
        self.repository.create_event(pending.clone())?;
        emit_rotation_telemetry(&pending);

        if readiness_is_ambiguous(&input.metadata) {
            let denied = build_terminal_event(
                &pending,
                CredentialRotationDecisionOutcome::Deny,
                CredentialRotationState::Denied,
                CredentialRotationReasonCode::ReadinessAmbiguous,
                None,
                input.now_utc.clone(),
            );
            self.repository.update_event(denied.clone())?;
            emit_rotation_telemetry(&denied);
            return Ok(denied);
        }

        if !due {
            let denied = build_terminal_event(
                &pending,
                CredentialRotationDecisionOutcome::Deny,
                CredentialRotationState::Denied,
                CredentialRotationReasonCode::ScheduledNotDue,
                None,
                input.now_utc.clone(),
            );
            self.repository.update_event(denied.clone())?;
            emit_rotation_telemetry(&denied);
            return Ok(denied);
        }

        let in_progress = build_pending_like_event(
            &pending,
            CredentialRotationState::InProgress,
            CredentialRotationReasonCode::RotationPending,
        );
        self.repository.update_event(in_progress.clone())?;
        emit_rotation_telemetry(&in_progress);

        let rotation_result = self.secret_rotation_port.rotate(SecretRotationRequest {
            trigger_type: CredentialRotationTrigger::ScheduledCadence,
            credential_scope: input.credential_scope,
            credential_reference: input.credential_reference,
            correlation_id: input.correlation_id,
            requested_at_utc: input.now_utc.clone(),
            deadline_at_utc: None,
            metadata: input.metadata,
        });

        let terminal = match rotation_result {
            Ok(result) => build_terminal_event(
                &pending,
                CredentialRotationDecisionOutcome::Allow,
                CredentialRotationState::Succeeded,
                CredentialRotationReasonCode::RotationAllowed,
                Some(result.rotation_reference),
                result.completed_at_utc,
            ),
            Err(error) => build_terminal_event(
                &pending,
                CredentialRotationDecisionOutcome::Deny,
                CredentialRotationState::Failed,
                error.code.reason_code(),
                None,
                input.now_utc,
            ),
        };

        self.repository.update_event(terminal.clone())?;
        emit_rotation_telemetry(&terminal);
        Ok(terminal)
    }

    fn trigger_emergency_rotation(
        &self,
        input: TriggerEmergencyRotationInput,
    ) -> Result<CredentialRotationEvidence, CredentialRotationServiceError> {
        validate_rotation_role(&input.actor_role)?;
        validate_common_inputs(
            &input.actor_id,
            &input.credential_scope,
            &input.credential_reference,
            &input.correlation_id,
            &input.now_utc,
            &input.metadata,
        )?;

        let deadline_at_utc =
            emergency_rotation_deadline_utc(&input.compromise_triggered_at_utc)
                .map_err(|error| CredentialRotationServiceError::invalid_payload(error.message))?;
        let within_deadline =
            emergency_rotation_within_deadline(&input.compromise_triggered_at_utc, &input.now_utc)
                .map_err(|error| CredentialRotationServiceError::invalid_payload(error.message))?;

        let rotation_id = generate_rotation_id(
            CredentialRotationTrigger::EmergencyCompromise,
            &input.credential_scope,
            &input.correlation_id,
        );
        if let Some(existing) = self.repository.load_event(&rotation_id)? {
            return Ok(existing);
        }

        let _lock = self.lock_operations()?;
        let pending = build_pending_event(
            &rotation_id,
            &input.actor_id,
            CredentialRotationTrigger::EmergencyCompromise,
            &input.credential_scope,
            &input.credential_reference,
            &input.correlation_id,
            &input.now_utc,
            input.metadata.clone(),
            Some(deadline_at_utc.clone()),
        );
        self.repository.create_event(pending.clone())?;
        emit_rotation_telemetry(&pending);

        if readiness_is_ambiguous(&input.metadata) {
            let denied = build_terminal_event(
                &pending,
                CredentialRotationDecisionOutcome::Deny,
                CredentialRotationState::Denied,
                CredentialRotationReasonCode::ReadinessAmbiguous,
                None,
                input.now_utc.clone(),
            );
            self.repository.update_event(denied.clone())?;
            emit_rotation_telemetry(&denied);
            return Ok(denied);
        }

        if !within_deadline {
            let denied = build_terminal_event(
                &pending,
                CredentialRotationDecisionOutcome::Deny,
                CredentialRotationState::Denied,
                CredentialRotationReasonCode::EmergencyWindowExpired,
                None,
                input.now_utc.clone(),
            );
            self.repository.update_event(denied.clone())?;
            emit_rotation_telemetry(&denied);
            return Ok(denied);
        }

        let in_progress = build_pending_like_event(
            &pending,
            CredentialRotationState::InProgress,
            CredentialRotationReasonCode::RotationPending,
        );
        self.repository.update_event(in_progress.clone())?;
        emit_rotation_telemetry(&in_progress);

        let rotation_result = self.secret_rotation_port.rotate(SecretRotationRequest {
            trigger_type: CredentialRotationTrigger::EmergencyCompromise,
            credential_scope: input.credential_scope,
            credential_reference: input.credential_reference,
            correlation_id: input.correlation_id,
            requested_at_utc: input.now_utc.clone(),
            deadline_at_utc: Some(deadline_at_utc),
            metadata: input.metadata,
        });

        let terminal = match rotation_result {
            Ok(result) => {
                let within_completion_deadline = emergency_rotation_within_deadline(
                    &input.compromise_triggered_at_utc,
                    &result.completed_at_utc,
                )
                .map_err(|error| CredentialRotationServiceError::invalid_payload(error.message))?;
                if !within_completion_deadline {
                    build_terminal_event(
                        &pending,
                        CredentialRotationDecisionOutcome::Deny,
                        CredentialRotationState::Failed,
                        CredentialRotationReasonCode::EmergencyWindowExpired,
                        None,
                        result.completed_at_utc,
                    )
                } else {
                    build_terminal_event(
                        &pending,
                        CredentialRotationDecisionOutcome::Allow,
                        CredentialRotationState::Succeeded,
                        CredentialRotationReasonCode::RotationAllowed,
                        Some(result.rotation_reference),
                        result.completed_at_utc,
                    )
                }
            }
            Err(error) => build_terminal_event(
                &pending,
                CredentialRotationDecisionOutcome::Deny,
                CredentialRotationState::Failed,
                error.code.reason_code(),
                None,
                input.now_utc,
            ),
        };

        self.repository.update_event(terminal.clone())?;
        emit_rotation_telemetry(&terminal);
        Ok(terminal)
    }
}

fn validate_rotation_role(role: &str) -> Result<(), CredentialRotationServiceError> {
    match role {
        "operational_control" | "administrative_actions" => Ok(()),
        _ => Err(CredentialRotationServiceError::unauthorized_role()),
    }
}

fn validate_common_inputs(
    actor_id: &str,
    credential_scope: &str,
    credential_reference: &str,
    correlation_id: &str,
    now_utc: &str,
    metadata: &Value,
) -> Result<(), CredentialRotationServiceError> {
    validate_non_empty("actor_id", actor_id)?;
    validate_non_empty("credential_scope", credential_scope)?;
    validate_non_empty("credential_reference", credential_reference)?;
    validate_non_empty("correlation_id", correlation_id)?;
    validate_non_empty("now_utc", now_utc)?;
    validate_rotation_metadata(metadata)
        .map_err(CredentialRotationServiceError::from_contract_error)?;
    validate_required_metadata_fields(metadata)?;
    Ok(())
}

fn validate_required_metadata_fields(
    metadata: &Value,
) -> Result<(), CredentialRotationServiceError> {
    let Some(metadata) = metadata.as_object() else {
        return Err(CredentialRotationServiceError::missing_metadata(
            "rotation metadata must be a JSON object",
        ));
    };

    if !matches!(
        metadata.get("crypto_posture_verified"),
        Some(Value::Bool(_))
    ) {
        return Err(CredentialRotationServiceError::missing_metadata(
            "rotation metadata must include boolean `crypto_posture_verified`",
        ));
    }
    if metadata
        .get("runtime_injection_mode")
        .and_then(Value::as_str)
        .is_none_or(str::is_empty)
    {
        return Err(CredentialRotationServiceError::missing_metadata(
            "rotation metadata must include non-empty `runtime_injection_mode`",
        ));
    }
    if metadata
        .get("provider_ref")
        .and_then(Value::as_str)
        .is_none_or(str::is_empty)
    {
        return Err(CredentialRotationServiceError::missing_metadata(
            "rotation metadata must include non-empty `provider_ref`",
        ));
    }

    Ok(())
}

fn validate_non_empty(
    field: &'static str,
    value: &str,
) -> Result<(), CredentialRotationServiceError> {
    if value.trim().is_empty() {
        return Err(CredentialRotationServiceError::invalid_payload(format!(
            "{field} cannot be blank"
        )));
    }
    Ok(())
}

fn readiness_is_ambiguous(metadata: &Value) -> bool {
    let Some(metadata) = metadata.as_object() else {
        return true;
    };
    let crypto_ok = metadata
        .get("crypto_posture_verified")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let runtime_ok = metadata
        .get("runtime_injection_mode")
        .and_then(Value::as_str)
        .map(|value| value == "runtime_only")
        .unwrap_or(false);
    !(crypto_ok && runtime_ok)
}

#[allow(clippy::too_many_arguments)]
fn build_pending_event(
    rotation_id: &str,
    actor_id: &str,
    trigger_type: CredentialRotationTrigger,
    credential_scope: &str,
    credential_reference: &str,
    correlation_id: &str,
    initiated_at_utc: &str,
    metadata: Value,
    deadline_at_utc: Option<String>,
) -> CredentialRotationEvidence {
    CredentialRotationEvidence {
        rotation_id: rotation_id.to_string(),
        actor_id: actor_id.to_string(),
        trigger_type,
        credential_scope: credential_scope.to_string(),
        credential_reference: credential_reference.to_string(),
        outcome: CredentialRotationDecisionOutcome::Pending,
        status: CredentialRotationState::Pending,
        reason_code: CredentialRotationReasonCode::RotationPending
            .code()
            .to_string(),
        correlation_id: correlation_id.to_string(),
        initiated_at_utc: initiated_at_utc.to_string(),
        deadline_at_utc,
        completed_at_utc: None,
        rotation_reference: None,
        metadata,
    }
}

fn build_pending_like_event(
    base: &CredentialRotationEvidence,
    status: CredentialRotationState,
    reason_code: CredentialRotationReasonCode,
) -> CredentialRotationEvidence {
    let mut next = base.clone();
    next.status = status;
    next.reason_code = reason_code.code().to_string();
    next.outcome = CredentialRotationDecisionOutcome::Pending;
    next.completed_at_utc = None;
    next.rotation_reference = None;
    next
}

fn build_terminal_event(
    base: &CredentialRotationEvidence,
    outcome: CredentialRotationDecisionOutcome,
    status: CredentialRotationState,
    reason_code: CredentialRotationReasonCode,
    rotation_reference: Option<String>,
    completed_at_utc: String,
) -> CredentialRotationEvidence {
    let mut next = base.clone();
    next.outcome = outcome;
    next.status = status;
    next.reason_code = reason_code.code().to_string();
    next.rotation_reference = rotation_reference;
    next.completed_at_utc = Some(completed_at_utc);
    next
}

fn generate_rotation_id(
    trigger_type: CredentialRotationTrigger,
    credential_scope: &str,
    correlation_id: &str,
) -> String {
    format!(
        "rot_{}_{}_{}",
        trigger_type.as_str(),
        sanitize_for_identifier(credential_scope),
        sanitize_for_identifier(correlation_id)
    )
}

fn sanitize_for_identifier(raw: &str) -> String {
    let sanitized: String = raw
        .trim()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() {
        "unknown".to_string()
    } else {
        sanitized
    }
}

#[derive(Debug, Clone, Default)]
pub struct InMemorySecretRotationPort;

impl SecretRotationPort for InMemorySecretRotationPort {
    fn rotate(
        &self,
        request: SecretRotationRequest,
    ) -> Result<SecretRotationResult, SecretRotationPortError> {
        if request
            .metadata
            .get("force_provider_failure")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Err(SecretRotationPortError::provider_unavailable(
                "secret provider dependency unavailable",
            ));
        }
        if request
            .metadata
            .get("force_runtime_failure")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Err(SecretRotationPortError::runtime_unavailable(
                "runtime adapter verification failed",
            ));
        }

        if request
            .deadline_at_utc
            .as_deref()
            .is_some_and(str::is_empty)
        {
            return Err(SecretRotationPortError::runtime_unavailable(
                "deadline timestamp payload is invalid",
            ));
        }

        Ok(SecretRotationResult {
            rotation_reference: format!(
                "rotref://{}/{}/{}",
                sanitize_for_identifier(&request.credential_scope),
                request.trigger_type.as_str(),
                sanitize_for_identifier(&request.correlation_id)
            ),
            completed_at_utc: request.requested_at_utc,
        })
    }
}

#[derive(Debug, Clone)]
pub struct PostgresCredentialRotationRepository {
    pool: PgPool,
}

impl PostgresCredentialRotationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn run_with_runtime<F, T>(&self, future: F) -> Result<T, CredentialRotationServiceError>
    where
        F: Future<Output = Result<T, CredentialRotationPersistenceError>>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future))
                .map_err(map_persistence_error),
            Err(_) => tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| {
                    CredentialRotationServiceError::persistence_unavailable(format!(
                        "failed to initialize async runtime: {error}"
                    ))
                })?
                .block_on(future)
                .map_err(map_persistence_error),
        }
    }
}

impl CredentialRotationRepositoryPort for PostgresCredentialRotationRepository {
    fn create_event(
        &self,
        event: CredentialRotationEvidence,
    ) -> Result<(), CredentialRotationServiceError> {
        self.run_with_runtime(pg_create_credential_rotation_event(&self.pool, &event))
    }

    fn load_event(
        &self,
        rotation_id: &str,
    ) -> Result<Option<CredentialRotationEvidence>, CredentialRotationServiceError> {
        self.run_with_runtime(pg_load_credential_rotation_event(&self.pool, rotation_id))
    }

    fn update_event(
        &self,
        event: CredentialRotationEvidence,
    ) -> Result<(), CredentialRotationServiceError> {
        self.run_with_runtime(pg_update_credential_rotation_event(&self.pool, &event))
    }

    fn query_events(
        &self,
        trigger_type: Option<CredentialRotationTrigger>,
        status: Option<CredentialRotationState>,
        limit: u32,
    ) -> Result<Vec<CredentialRotationEvidence>, CredentialRotationServiceError> {
        self.run_with_runtime(pg_query_credential_rotation_events(
            &self.pool,
            trigger_type,
            status,
            limit,
        ))
    }
}

#[derive(Debug, Default)]
pub struct InMemoryCredentialRotationRepository {
    events: Mutex<BTreeMap<String, CredentialRotationEvidence>>,
}

impl CredentialRotationRepositoryPort for InMemoryCredentialRotationRepository {
    fn create_event(
        &self,
        event: CredentialRotationEvidence,
    ) -> Result<(), CredentialRotationServiceError> {
        let mut events = self
            .events
            .lock()
            .expect("in-memory credential-rotation events lock should not be poisoned");
        if events.contains_key(&event.rotation_id) {
            return Err(CredentialRotationServiceError {
                code: "credential_rotation_duplicate_rotation_id",
                message: format!("rotation event `{}` already exists", event.rotation_id),
            });
        }
        if let Some(reference) = event.rotation_reference.as_deref()
            && events
                .values()
                .any(|existing| existing.rotation_reference.as_deref() == Some(reference))
        {
            return Err(CredentialRotationServiceError {
                code: "credential_rotation_duplicate_reference",
                message: format!("rotation reference `{reference}` already exists"),
            });
        }
        events.insert(event.rotation_id.clone(), event);
        Ok(())
    }

    fn load_event(
        &self,
        rotation_id: &str,
    ) -> Result<Option<CredentialRotationEvidence>, CredentialRotationServiceError> {
        let events = self
            .events
            .lock()
            .expect("in-memory credential-rotation events lock should not be poisoned");
        Ok(events.get(rotation_id).cloned())
    }

    fn update_event(
        &self,
        event: CredentialRotationEvidence,
    ) -> Result<(), CredentialRotationServiceError> {
        let mut events = self
            .events
            .lock()
            .expect("in-memory credential-rotation events lock should not be poisoned");
        let Some(existing) = events.get(&event.rotation_id) else {
            return Err(CredentialRotationServiceError {
                code: "credential_rotation_not_found",
                message: format!("rotation event `{}` does not exist", event.rotation_id),
            });
        };

        if !is_valid_rotation_transition(existing.status, event.status) {
            return Err(CredentialRotationServiceError {
                code: CredentialRotationReasonCode::InvalidStateTransition.code(),
                message: format!(
                    "invalid state transition from `{}` to `{}`",
                    existing.status.as_str(),
                    event.status.as_str()
                ),
            });
        }

        if let (Some(existing_reference), Some(next_reference)) = (
            existing.rotation_reference.as_deref(),
            event.rotation_reference.as_deref(),
        ) && existing_reference != next_reference
        {
            return Err(CredentialRotationServiceError {
                code: CredentialRotationReasonCode::InvalidStateTransition.code(),
                message: "rotation_reference cannot change once assigned".to_string(),
            });
        }

        if let Some(reference) = event.rotation_reference.as_deref()
            && events
                .values()
                .filter(|candidate| candidate.rotation_id != event.rotation_id)
                .any(|candidate| candidate.rotation_reference.as_deref() == Some(reference))
        {
            return Err(CredentialRotationServiceError {
                code: "credential_rotation_duplicate_reference",
                message: format!("rotation reference `{reference}` already exists"),
            });
        }

        events.insert(event.rotation_id.clone(), event);
        Ok(())
    }

    fn query_events(
        &self,
        trigger_type: Option<CredentialRotationTrigger>,
        status: Option<CredentialRotationState>,
        limit: u32,
    ) -> Result<Vec<CredentialRotationEvidence>, CredentialRotationServiceError> {
        let events = self
            .events
            .lock()
            .expect("in-memory credential-rotation events lock should not be poisoned");
        let mut collected: Vec<_> = events
            .values()
            .filter(|event| trigger_type.is_none_or(|needle| event.trigger_type == needle))
            .filter(|event| status.is_none_or(|needle| event.status == needle))
            .cloned()
            .collect();

        collected.sort_by(|left, right| {
            right
                .initiated_at_utc
                .cmp(&left.initiated_at_utc)
                .then(left.rotation_id.cmp(&right.rotation_id))
        });
        collected.truncate(limit as usize);
        Ok(collected)
    }
}

fn is_valid_rotation_transition(
    current: CredentialRotationState,
    next: CredentialRotationState,
) -> bool {
    match current {
        CredentialRotationState::Pending => matches!(
            next,
            CredentialRotationState::Pending
                | CredentialRotationState::InProgress
                | CredentialRotationState::Succeeded
                | CredentialRotationState::Denied
                | CredentialRotationState::Failed
        ),
        CredentialRotationState::InProgress => matches!(
            next,
            CredentialRotationState::InProgress
                | CredentialRotationState::Succeeded
                | CredentialRotationState::Denied
                | CredentialRotationState::Failed
        ),
        CredentialRotationState::Succeeded => next == CredentialRotationState::Succeeded,
        CredentialRotationState::Denied => next == CredentialRotationState::Denied,
        CredentialRotationState::Failed => next == CredentialRotationState::Failed,
    }
}

fn emit_rotation_telemetry(evidence: &CredentialRotationEvidence) {
    let telemetry = CredentialRotationTelemetryEvent {
        event_name: "credential_rotation_decision_v1",
        rotation_id: &evidence.rotation_id,
        actor_id: &evidence.actor_id,
        trigger_type: evidence.trigger_type.as_str(),
        credential_scope: &evidence.credential_scope,
        outcome: evidence.outcome.as_str(),
        status: evidence.status.as_str(),
        reason_code: &evidence.reason_code,
        correlation_id: &evidence.correlation_id,
        timestamp_utc: evidence
            .completed_at_utc
            .as_deref()
            .unwrap_or(evidence.initiated_at_utc.as_str()),
        rotation_reference: evidence.rotation_reference.as_deref(),
        security_signal: match evidence.outcome {
            CredentialRotationDecisionOutcome::Deny => Some(CredentialRotationSecuritySignal {
                name: "credential_rotation_denied_v1",
                severity: "high",
                alert_compatible: true,
                alert_target_seconds: 30,
            }),
            _ => None,
        },
    };
    println!(
        "{}",
        serde_json::to_string(&telemetry)
            .expect("credential rotation telemetry event should serialize")
    );
}

#[derive(Debug, Serialize)]
struct CredentialRotationTelemetryEvent<'a> {
    event_name: &'a str,
    rotation_id: &'a str,
    actor_id: &'a str,
    trigger_type: &'a str,
    credential_scope: &'a str,
    outcome: &'a str,
    status: &'a str,
    reason_code: &'a str,
    correlation_id: &'a str,
    timestamp_utc: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    rotation_reference: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    security_signal: Option<CredentialRotationSecuritySignal<'a>>,
}

#[derive(Debug, Serialize)]
struct CredentialRotationSecuritySignal<'a> {
    name: &'a str,
    severity: &'a str,
    alert_compatible: bool,
    alert_target_seconds: u16,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn scheduled_input(
        last_rotated_at_utc: &str,
        metadata: Value,
    ) -> TriggerScheduledRotationInput {
        TriggerScheduledRotationInput {
            actor_id: "ops-1".to_string(),
            actor_role: "operational_control".to_string(),
            credential_scope: "control_api".to_string(),
            credential_reference: "vault://control-api/prod".to_string(),
            metadata,
            correlation_id: "corr-scheduled-001".to_string(),
            now_utc: "2026-04-05T00:00:00Z".to_string(),
            last_rotated_at_utc: last_rotated_at_utc.to_string(),
        }
    }

    fn emergency_input(now_utc: &str, metadata: Value) -> TriggerEmergencyRotationInput {
        TriggerEmergencyRotationInput {
            actor_id: "ops-1".to_string(),
            actor_role: "operational_control".to_string(),
            credential_scope: "control_api".to_string(),
            credential_reference: "vault://control-api/prod".to_string(),
            metadata,
            correlation_id: "corr-emergency-001".to_string(),
            now_utc: now_utc.to_string(),
            compromise_triggered_at_utc: "2026-04-05T00:00:00Z".to_string(),
        }
    }

    fn compliant_metadata() -> Value {
        json!({
            "crypto_posture_verified": true,
            "runtime_injection_mode": "runtime_only",
            "provider_ref": "vault://control-api/prod"
        })
    }

    #[test]
    fn scheduled_rotation_denies_when_not_due() {
        let service = CredentialRotationService::default();
        let decision = service
            .trigger_scheduled_rotation(scheduled_input(
                "2026-01-07T00:00:01Z",
                compliant_metadata(),
            ))
            .expect("not-due rotation should return deterministic denial");

        assert_eq!(decision.outcome, CredentialRotationDecisionOutcome::Deny);
        assert_eq!(decision.status, CredentialRotationState::Denied);
        assert_eq!(
            decision.reason_code,
            CredentialRotationReasonCode::ScheduledNotDue.code()
        );
    }

    #[test]
    fn emergency_rotation_denies_when_trigger_window_is_expired() {
        let service = CredentialRotationService::default();
        let decision = service
            .trigger_emergency_rotation(emergency_input(
                "2026-04-05T00:31:00Z",
                compliant_metadata(),
            ))
            .expect("expired emergency trigger should deny");

        assert_eq!(decision.outcome, CredentialRotationDecisionOutcome::Deny);
        assert_eq!(decision.status, CredentialRotationState::Denied);
        assert_eq!(
            decision.reason_code,
            CredentialRotationReasonCode::EmergencyWindowExpired.code()
        );
    }

    #[test]
    fn provider_failure_is_fail_closed_with_machine_reason() {
        let service = CredentialRotationService::default();
        let decision = service
            .trigger_scheduled_rotation(scheduled_input(
                "2025-12-01T00:00:00Z",
                json!({
                    "crypto_posture_verified": true,
                    "runtime_injection_mode": "runtime_only",
                    "provider_ref": "vault://control-api/prod",
                    "force_provider_failure": true
                }),
            ))
            .expect("provider failures should return fail-closed denial");

        assert_eq!(decision.outcome, CredentialRotationDecisionOutcome::Deny);
        assert_eq!(decision.status, CredentialRotationState::Failed);
        assert_eq!(
            decision.reason_code,
            CredentialRotationReasonCode::ProviderUnavailable.code()
        );
        assert!(decision.rotation_reference.is_none());
    }

    #[test]
    fn emergency_rotation_returns_success_evidence_with_reference() {
        let service = CredentialRotationService::default();
        let decision = service
            .trigger_emergency_rotation(emergency_input(
                "2026-04-05T00:10:00Z",
                compliant_metadata(),
            ))
            .expect("emergency rotation should succeed inside deadline");

        assert_eq!(decision.outcome, CredentialRotationDecisionOutcome::Allow);
        assert_eq!(decision.status, CredentialRotationState::Succeeded);
        assert_eq!(
            decision.reason_code,
            CredentialRotationReasonCode::RotationAllowed.code()
        );
        assert!(decision.rotation_reference.is_some());
    }

    #[test]
    fn ambiguous_readiness_state_is_denied_fail_closed() {
        let service = CredentialRotationService::default();
        let decision = service
            .trigger_scheduled_rotation(scheduled_input(
                "2025-12-01T00:00:00Z",
                json!({
                    "crypto_posture_verified": false,
                    "runtime_injection_mode": "runtime_only",
                    "provider_ref": "vault://control-api/prod"
                }),
            ))
            .expect("ambiguous readiness should be denied");

        assert_eq!(decision.outcome, CredentialRotationDecisionOutcome::Deny);
        assert_eq!(decision.status, CredentialRotationState::Denied);
        assert_eq!(
            decision.reason_code,
            CredentialRotationReasonCode::ReadinessAmbiguous.code()
        );
    }

    #[test]
    fn scheduled_rotation_rejects_missing_required_metadata_fields() {
        let service = CredentialRotationService::default();
        let error = service
            .trigger_scheduled_rotation(scheduled_input(
                "2025-12-01T00:00:00Z",
                json!({
                    "crypto_posture_verified": true,
                    "runtime_injection_mode": "runtime_only"
                }),
            ))
            .expect_err("missing provider_ref must fail with machine-readable error");

        assert_eq!(
            error.code,
            CredentialRotationReasonCode::MissingMetadata.code()
        );
    }

    #[test]
    fn scheduled_rotation_preserves_secret_material_reason_code() {
        let service = CredentialRotationService::default();
        let error = service
            .trigger_scheduled_rotation(scheduled_input(
                "2025-12-01T00:00:00Z",
                json!({
                    "crypto_posture_verified": true,
                    "runtime_injection_mode": "runtime_only",
                    "provider_ref": "vault://control-api/prod",
                    "token_value": "abc123"
                }),
            ))
            .expect_err("secret-like metadata should retain secret-material reason code");

        assert_eq!(
            error.code,
            CredentialRotationReasonCode::SecretMaterialRejected.code()
        );
    }

    #[test]
    fn query_events_is_deterministic_by_time_then_rotation_id() {
        let repository = InMemoryCredentialRotationRepository::default();
        let mut event_a = build_pending_event(
            "rot-a",
            "ops-1",
            CredentialRotationTrigger::ScheduledCadence,
            "scope-a",
            "vault://scope-a",
            "corr-a",
            "2026-04-05T00:00:00Z",
            compliant_metadata(),
            None,
        );
        event_a.status = CredentialRotationState::Succeeded;
        event_a.outcome = CredentialRotationDecisionOutcome::Allow;
        event_a.reason_code = CredentialRotationReasonCode::RotationAllowed
            .code()
            .to_string();
        event_a.completed_at_utc = Some("2026-04-05T00:00:00Z".to_string());
        event_a.rotation_reference = Some("rotref://a".to_string());

        let mut event_b = event_a.clone();
        event_b.rotation_id = "rot-b".to_string();
        event_b.initiated_at_utc = "2026-04-05T00:01:00Z".to_string();
        event_b.rotation_reference = Some("rotref://b".to_string());

        let mut event_c = event_a.clone();
        event_c.rotation_id = "rot-c".to_string();
        event_c.initiated_at_utc = "2026-04-05T00:01:00Z".to_string();
        event_c.rotation_reference = Some("rotref://c".to_string());

        repository
            .create_event(event_a)
            .expect("event a should insert");
        repository
            .create_event(event_b)
            .expect("event b should insert");
        repository
            .create_event(event_c)
            .expect("event c should insert");

        let listed = repository
            .query_events(None, None, 10)
            .expect("query should succeed");
        let ids: Vec<_> = listed.into_iter().map(|event| event.rotation_id).collect();
        assert_eq!(ids, vec!["rot-b", "rot-c", "rot-a"]);
    }
}
