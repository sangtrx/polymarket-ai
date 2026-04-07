use domain::research::{
    AlphaHypothesisReasonCode, AlphaHypothesisRegistration, AlphaHypothesisValidationIssue,
    canonicalize_alpha_hypothesis_registration, normalize_research_identifier, parse_utc_timestamp,
};
use persistence::postgres::alpha_hypotheses::{
    AlphaHypothesisPersistenceError, load_alpha_hypothesis as pg_load_alpha_hypothesis,
    upsert_alpha_hypothesis as pg_upsert_alpha_hypothesis,
};
use serde::Serialize;
use serde_json::Value;
use sqlx::PgPool;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct HypothesisRegistryServiceError {
    pub code: &'static str,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub field_errors: Vec<AlphaHypothesisValidationIssue>,
}

impl HypothesisRegistryServiceError {
    pub fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<AlphaHypothesisValidationIssue>,
    ) -> Self {
        Self {
            code: AlphaHypothesisReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn unauthorized_role() -> Self {
        Self {
            code: AlphaHypothesisReasonCode::UnauthorizedRole.code(),
            message: "actor role is not authorized for alpha hypothesis mutation".to_string(),
            field_errors: Vec::new(),
        }
    }

    fn unauthorized_read_role() -> Self {
        Self {
            code: AlphaHypothesisReasonCode::UnauthorizedRole.code(),
            message: "actor role is not authorized for alpha hypothesis reads".to_string(),
            field_errors: Vec::new(),
        }
    }

    fn not_found(hypothesis_id: &str) -> Self {
        Self {
            code: AlphaHypothesisReasonCode::NotFound.code(),
            message: format!("alpha hypothesis `{hypothesis_id}` was not found"),
            field_errors: Vec::new(),
        }
    }

    fn dataset_snapshot_unresolved(feature_set_version: &str) -> Self {
        Self {
            code: AlphaHypothesisReasonCode::DatasetSnapshotUnresolved.code(),
            message: format!(
                "feature_set_version `{feature_set_version}` is not registered in the dataset snapshot registry"
            ),
            field_errors: vec![AlphaHypothesisValidationIssue {
                field: "feature_set_version".to_string(),
                code: AlphaHypothesisReasonCode::DatasetSnapshotUnresolved.code(),
                message: "feature_set_version must reference a registered dataset snapshot"
                    .to_string(),
            }],
        }
    }

    fn dataset_snapshot_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: AlphaHypothesisReasonCode::DatasetSnapshotUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    fn persistence_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: AlphaHypothesisReasonCode::PersistenceUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }
}

impl Display for HypothesisRegistryServiceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for HypothesisRegistryServiceError {}

fn map_persistence_error(error: AlphaHypothesisPersistenceError) -> HypothesisRegistryServiceError {
    match error.code {
        "alpha_hypothesis_query_failed" | "alpha_hypothesis_row_decode_failed" => {
            HypothesisRegistryServiceError::persistence_unavailable(error.message)
        }
        _ => HypothesisRegistryServiceError {
            code: error.code,
            message: error.message,
            field_errors: error.field_errors,
        },
    }
}

#[derive(Debug, Clone)]
pub struct UpsertAlphaHypothesisInput {
    pub actor_id: String,
    pub actor_role: String,
    pub hypothesis_id: String,
    pub feature_set_version: String,
    pub target_regime: String,
    pub expected_edge_source: String,
    pub training_window_start_utc: String,
    pub training_window_end_utc: String,
    pub risk_assumptions: Value,
    pub correlation_id: String,
    pub updated_at_utc: String,
}

#[derive(Debug, Clone)]
pub struct ReadAlphaHypothesisInput {
    pub actor_id: String,
    pub actor_role: String,
    pub hypothesis_id: String,
    pub correlation_id: String,
    pub queried_at_utc: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AlphaHypothesisEvidence {
    pub hypothesis_id: String,
    pub feature_set_version: String,
    pub target_regime: String,
    pub expected_edge_source: String,
    pub training_window_start_utc: String,
    pub training_window_end_utc: String,
    pub risk_assumptions: Value,
    pub actor_id: String,
    pub reason_code: String,
    pub correlation_id: String,
    pub updated_at_utc: String,
}

pub trait AlphaHypothesisRepositoryPort: Send + Sync {
    fn upsert(
        &self,
        registration: AlphaHypothesisRegistration,
    ) -> Result<(), HypothesisRegistryServiceError>;
    fn load(
        &self,
        hypothesis_id: &str,
    ) -> Result<Option<AlphaHypothesisRegistration>, HypothesisRegistryServiceError>;
}

pub trait DatasetSnapshotRegistryPort: Send + Sync {
    fn is_registered(
        &self,
        feature_set_version: &str,
    ) -> Result<bool, HypothesisRegistryServiceError>;
}

pub trait HypothesisRegistryOrchestrator: Send + Sync {
    fn upsert_alpha_hypothesis(
        &self,
        input: UpsertAlphaHypothesisInput,
    ) -> Result<AlphaHypothesisEvidence, HypothesisRegistryServiceError>;

    fn read_alpha_hypothesis(
        &self,
        input: ReadAlphaHypothesisInput,
    ) -> Result<AlphaHypothesisEvidence, HypothesisRegistryServiceError>;
}

#[derive(Clone)]
pub struct HypothesisRegistryService {
    repository: Arc<dyn AlphaHypothesisRepositoryPort>,
    dataset_registry: Arc<dyn DatasetSnapshotRegistryPort>,
    operation_lock: Arc<Mutex<()>>,
}

impl HypothesisRegistryService {
    pub fn new(
        repository: Arc<dyn AlphaHypothesisRepositoryPort>,
        dataset_registry: Arc<dyn DatasetSnapshotRegistryPort>,
    ) -> Self {
        Self {
            repository,
            dataset_registry,
            operation_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn in_memory() -> Self {
        Self::new(
            Arc::new(InMemoryAlphaHypothesisRepository::default()),
            Arc::new(StaticDatasetSnapshotRegistry::default()),
        )
    }

    pub fn postgres(pool: PgPool) -> Self {
        Self::new(
            Arc::new(PostgresAlphaHypothesisRepository::new(pool)),
            Arc::new(StaticDatasetSnapshotRegistry::default()),
        )
    }

    pub fn with_dataset_registry(
        mut self,
        dataset_registry: Arc<dyn DatasetSnapshotRegistryPort>,
    ) -> Self {
        self.dataset_registry = dataset_registry;
        self
    }

    fn lock_operations(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, ()>, HypothesisRegistryServiceError> {
        self.operation_lock.lock().map_err(|_| {
            HypothesisRegistryServiceError::persistence_unavailable(
                "alpha hypothesis operation lock poisoned by prior panic",
            )
        })
    }
}

impl Default for HypothesisRegistryService {
    fn default() -> Self {
        Self::in_memory()
    }
}

impl HypothesisRegistryOrchestrator for HypothesisRegistryService {
    fn upsert_alpha_hypothesis(
        &self,
        input: UpsertAlphaHypothesisInput,
    ) -> Result<AlphaHypothesisEvidence, HypothesisRegistryServiceError> {
        if let Err(error) = validate_mutation_role(&input.actor_role) {
            emit_hypothesis_registry_telemetry(
                "alpha_hypothesis_upsert_v1",
                "deny",
                &input.actor_id,
                &input.hypothesis_id,
                error.code,
                &input.correlation_id,
                &input.updated_at_utc,
            );
            return Err(error);
        }
        validate_non_empty_with_telemetry(
            "actor_id",
            &input.actor_id,
            "alpha_hypothesis_upsert_v1",
            &input.actor_id,
            &input.hypothesis_id,
            &input.correlation_id,
            &input.updated_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "hypothesis_id",
            &input.hypothesis_id,
            "alpha_hypothesis_upsert_v1",
            &input.actor_id,
            &input.hypothesis_id,
            &input.correlation_id,
            &input.updated_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "feature_set_version",
            &input.feature_set_version,
            "alpha_hypothesis_upsert_v1",
            &input.actor_id,
            &input.hypothesis_id,
            &input.correlation_id,
            &input.updated_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "correlation_id",
            &input.correlation_id,
            "alpha_hypothesis_upsert_v1",
            &input.actor_id,
            &input.hypothesis_id,
            &input.correlation_id,
            &input.updated_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "updated_at_utc",
            &input.updated_at_utc,
            "alpha_hypothesis_upsert_v1",
            &input.actor_id,
            &input.hypothesis_id,
            &input.correlation_id,
            &input.updated_at_utc,
        )?;

        let canonical_registration =
            canonicalize_alpha_hypothesis_registration(&AlphaHypothesisRegistration {
                hypothesis_id: input.hypothesis_id.clone(),
                feature_set_version: input.feature_set_version.clone(),
                target_regime: input.target_regime.clone(),
                expected_edge_source: input.expected_edge_source.clone(),
                training_window_start_utc: input.training_window_start_utc.clone(),
                training_window_end_utc: input.training_window_end_utc.clone(),
                risk_assumptions: input.risk_assumptions.clone(),
                actor_id: input.actor_id.clone(),
                correlation_id: input.correlation_id.clone(),
                updated_at_utc: input.updated_at_utc.clone(),
            })
            .map_err(|error| {
                HypothesisRegistryServiceError::invalid_payload(error.message, error.field_errors)
            })
            .inspect_err(|error| {
                emit_hypothesis_registry_telemetry(
                    "alpha_hypothesis_upsert_v1",
                    "deny",
                    &input.actor_id,
                    &input.hypothesis_id,
                    error.code,
                    &input.correlation_id,
                    &input.updated_at_utc,
                );
            })?;

        match self
            .dataset_registry
            .is_registered(&canonical_registration.feature_set_version)
        {
            Ok(false) => {
                let error = HypothesisRegistryServiceError::dataset_snapshot_unresolved(
                    &canonical_registration.feature_set_version,
                );
                emit_hypothesis_registry_telemetry(
                    "alpha_hypothesis_upsert_v1",
                    "deny",
                    &input.actor_id,
                    &canonical_registration.hypothesis_id,
                    error.code,
                    &input.correlation_id,
                    &input.updated_at_utc,
                );
                return Err(error);
            }
            Err(error) => {
                emit_hypothesis_registry_telemetry(
                    "alpha_hypothesis_upsert_v1",
                    "deny",
                    &input.actor_id,
                    &canonical_registration.hypothesis_id,
                    error.code,
                    &input.correlation_id,
                    &input.updated_at_utc,
                );
                return Err(error);
            }
            Ok(true) => {}
        }

        let _lock = self.lock_operations().inspect_err(|error| {
            emit_hypothesis_registry_telemetry(
                "alpha_hypothesis_upsert_v1",
                "deny",
                &input.actor_id,
                &canonical_registration.hypothesis_id,
                error.code,
                &input.correlation_id,
                &input.updated_at_utc,
            );
        })?;
        let existed = self
            .repository
            .load(&canonical_registration.hypothesis_id)
            .inspect_err(|error| {
                emit_hypothesis_registry_telemetry(
                    "alpha_hypothesis_upsert_v1",
                    "deny",
                    &input.actor_id,
                    &canonical_registration.hypothesis_id,
                    error.code,
                    &input.correlation_id,
                    &input.updated_at_utc,
                );
            })?
            .is_some();
        self.repository
            .upsert(canonical_registration.clone())
            .inspect_err(|error| {
                emit_hypothesis_registry_telemetry(
                    "alpha_hypothesis_upsert_v1",
                    "deny",
                    &input.actor_id,
                    &canonical_registration.hypothesis_id,
                    error.code,
                    &input.correlation_id,
                    &input.updated_at_utc,
                );
            })?;

        let reason_code = if existed {
            AlphaHypothesisReasonCode::Updated.code()
        } else {
            AlphaHypothesisReasonCode::Registered.code()
        };
        emit_hypothesis_registry_telemetry(
            "alpha_hypothesis_upsert_v1",
            "allow",
            &input.actor_id,
            &canonical_registration.hypothesis_id,
            reason_code,
            &input.correlation_id,
            &input.updated_at_utc,
        );

        Ok(AlphaHypothesisEvidence {
            hypothesis_id: canonical_registration.hypothesis_id,
            feature_set_version: canonical_registration.feature_set_version,
            target_regime: canonical_registration.target_regime,
            expected_edge_source: canonical_registration.expected_edge_source,
            training_window_start_utc: canonical_registration.training_window_start_utc,
            training_window_end_utc: canonical_registration.training_window_end_utc,
            risk_assumptions: canonical_registration.risk_assumptions,
            actor_id: canonical_registration.actor_id,
            reason_code: reason_code.to_string(),
            correlation_id: canonical_registration.correlation_id,
            updated_at_utc: canonical_registration.updated_at_utc,
        })
    }

    fn read_alpha_hypothesis(
        &self,
        input: ReadAlphaHypothesisInput,
    ) -> Result<AlphaHypothesisEvidence, HypothesisRegistryServiceError> {
        if let Err(error) = validate_read_role(&input.actor_role) {
            emit_hypothesis_registry_telemetry(
                "alpha_hypothesis_read_v1",
                "deny",
                &input.actor_id,
                &input.hypothesis_id,
                error.code,
                &input.correlation_id,
                &input.queried_at_utc,
            );
            return Err(error);
        }
        validate_non_empty_with_telemetry(
            "actor_id",
            &input.actor_id,
            "alpha_hypothesis_read_v1",
            &input.actor_id,
            &input.hypothesis_id,
            &input.correlation_id,
            &input.queried_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "hypothesis_id",
            &input.hypothesis_id,
            "alpha_hypothesis_read_v1",
            &input.actor_id,
            &input.hypothesis_id,
            &input.correlation_id,
            &input.queried_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "correlation_id",
            &input.correlation_id,
            "alpha_hypothesis_read_v1",
            &input.actor_id,
            &input.hypothesis_id,
            &input.correlation_id,
            &input.queried_at_utc,
        )?;
        validate_non_empty_with_telemetry(
            "queried_at_utc",
            &input.queried_at_utc,
            "alpha_hypothesis_read_v1",
            &input.actor_id,
            &input.hypothesis_id,
            &input.correlation_id,
            &input.queried_at_utc,
        )?;
        validate_utc_timestamp_with_telemetry(
            "queried_at_utc",
            &input.queried_at_utc,
            "alpha_hypothesis_read_v1",
            &input.actor_id,
            &input.hypothesis_id,
            &input.correlation_id,
            &input.queried_at_utc,
        )?;

        let normalized_hypothesis_id = normalize_research_identifier(&input.hypothesis_id);
        let Some(registration) = self
            .repository
            .load(&normalized_hypothesis_id)
            .inspect_err(|error| {
                emit_hypothesis_registry_telemetry(
                    "alpha_hypothesis_read_v1",
                    "deny",
                    &input.actor_id,
                    &normalized_hypothesis_id,
                    error.code,
                    &input.correlation_id,
                    &input.queried_at_utc,
                );
            })?
        else {
            let error = HypothesisRegistryServiceError::not_found(&normalized_hypothesis_id);
            emit_hypothesis_registry_telemetry(
                "alpha_hypothesis_read_v1",
                "deny",
                &input.actor_id,
                &normalized_hypothesis_id,
                error.code,
                &input.correlation_id,
                &input.queried_at_utc,
            );
            return Err(error);
        };

        emit_hypothesis_registry_telemetry(
            "alpha_hypothesis_read_v1",
            "allow",
            &input.actor_id,
            &registration.hypothesis_id,
            AlphaHypothesisReasonCode::Read.code(),
            &input.correlation_id,
            &input.queried_at_utc,
        );

        Ok(AlphaHypothesisEvidence {
            hypothesis_id: registration.hypothesis_id,
            feature_set_version: registration.feature_set_version,
            target_regime: registration.target_regime,
            expected_edge_source: registration.expected_edge_source,
            training_window_start_utc: registration.training_window_start_utc,
            training_window_end_utc: registration.training_window_end_utc,
            risk_assumptions: registration.risk_assumptions,
            actor_id: input.actor_id,
            reason_code: AlphaHypothesisReasonCode::Read.code().to_string(),
            correlation_id: input.correlation_id,
            updated_at_utc: input.queried_at_utc,
        })
    }
}

fn validate_mutation_role(role: &str) -> Result<(), HypothesisRegistryServiceError> {
    match role {
        "operational_control" | "administrative_actions" => Ok(()),
        _ => Err(HypothesisRegistryServiceError::unauthorized_role()),
    }
}

fn validate_read_role(role: &str) -> Result<(), HypothesisRegistryServiceError> {
    match role {
        "read_only_analytics" | "operational_control" | "administrative_actions" => Ok(()),
        _ => Err(HypothesisRegistryServiceError::unauthorized_read_role()),
    }
}

fn validate_non_empty(field: &str, value: &str) -> Result<(), HypothesisRegistryServiceError> {
    if value.trim().is_empty() {
        return Err(HypothesisRegistryServiceError::invalid_payload(
            format!("{field} cannot be blank"),
            vec![AlphaHypothesisValidationIssue {
                field: field.to_string(),
                code: AlphaHypothesisReasonCode::InvalidPayload.code(),
                message: format!("{field} cannot be blank"),
            }],
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_non_empty_with_telemetry(
    field: &str,
    value: &str,
    event_name: &'static str,
    actor_id: &str,
    hypothesis_id: &str,
    correlation_id: &str,
    timestamp_utc: &str,
) -> Result<(), HypothesisRegistryServiceError> {
    validate_non_empty(field, value).inspect_err(|error| {
        emit_hypothesis_registry_telemetry(
            event_name,
            "deny",
            actor_id,
            hypothesis_id,
            error.code,
            correlation_id,
            timestamp_utc,
        );
    })
}

#[allow(clippy::too_many_arguments)]
fn validate_utc_timestamp_with_telemetry(
    field: &str,
    value: &str,
    event_name: &'static str,
    actor_id: &str,
    hypothesis_id: &str,
    correlation_id: &str,
    timestamp_utc: &str,
) -> Result<(), HypothesisRegistryServiceError> {
    parse_utc_timestamp(value)
        .map_err(|error| {
            HypothesisRegistryServiceError::invalid_payload(
                error.message.clone(),
                vec![AlphaHypothesisValidationIssue {
                    field: field.to_string(),
                    code: AlphaHypothesisReasonCode::InvalidPayload.code(),
                    message: error.message,
                }],
            )
        })
        .inspect_err(|error| {
            emit_hypothesis_registry_telemetry(
                event_name,
                "deny",
                actor_id,
                hypothesis_id,
                error.code,
                correlation_id,
                timestamp_utc,
            );
        })?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn emit_hypothesis_registry_telemetry(
    event_name: &'static str,
    outcome: &'static str,
    actor_id: &str,
    hypothesis_id: &str,
    reason_code: &str,
    correlation_id: &str,
    timestamp_utc: &str,
) {
    let event = HypothesisRegistryTelemetryEvent {
        event_name,
        action: match event_name {
            "alpha_hypothesis_read_v1" => "alpha_hypothesis_read",
            _ => "alpha_hypothesis_upsert",
        },
        outcome,
        actor_id,
        hypothesis_id,
        reason_code,
        correlation_id,
        timestamp_utc,
        security_signal: if outcome == "deny" {
            Some(HypothesisRegistrySecuritySignal {
                name: "alpha_hypothesis_registry_denied_v1",
                severity: "high",
                alert_compatible: true,
                alert_target_seconds: 30,
            })
        } else {
            None
        },
    };
    println!(
        "{}",
        serde_json::to_string(&event).expect("alpha hypothesis telemetry event should serialize")
    );
}

#[derive(Debug, Serialize)]
struct HypothesisRegistryTelemetryEvent<'a> {
    event_name: &'a str,
    action: &'a str,
    outcome: &'a str,
    actor_id: &'a str,
    hypothesis_id: &'a str,
    reason_code: &'a str,
    correlation_id: &'a str,
    timestamp_utc: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    security_signal: Option<HypothesisRegistrySecuritySignal<'a>>,
}

#[derive(Debug, Serialize)]
struct HypothesisRegistrySecuritySignal<'a> {
    name: &'a str,
    severity: &'a str,
    alert_compatible: bool,
    alert_target_seconds: u16,
}

#[derive(Debug, Clone)]
pub struct PostgresAlphaHypothesisRepository {
    pool: PgPool,
}

impl PostgresAlphaHypothesisRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn run_with_runtime<F, T>(&self, future: F) -> Result<T, HypothesisRegistryServiceError>
    where
        F: Future<Output = Result<T, AlphaHypothesisPersistenceError>>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future))
                .map_err(map_persistence_error),
            Err(_) => tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| {
                    HypothesisRegistryServiceError::persistence_unavailable(format!(
                        "failed to initialize async runtime: {error}"
                    ))
                })?
                .block_on(future)
                .map_err(map_persistence_error),
        }
    }
}

impl AlphaHypothesisRepositoryPort for PostgresAlphaHypothesisRepository {
    fn upsert(
        &self,
        registration: AlphaHypothesisRegistration,
    ) -> Result<(), HypothesisRegistryServiceError> {
        self.run_with_runtime(pg_upsert_alpha_hypothesis(&self.pool, &registration))
    }

    fn load(
        &self,
        hypothesis_id: &str,
    ) -> Result<Option<AlphaHypothesisRegistration>, HypothesisRegistryServiceError> {
        self.run_with_runtime(pg_load_alpha_hypothesis(&self.pool, hypothesis_id))
    }
}

#[derive(Debug, Default)]
pub struct InMemoryAlphaHypothesisRepository {
    records: Mutex<BTreeMap<String, AlphaHypothesisRegistration>>,
}

impl AlphaHypothesisRepositoryPort for InMemoryAlphaHypothesisRepository {
    fn upsert(
        &self,
        registration: AlphaHypothesisRegistration,
    ) -> Result<(), HypothesisRegistryServiceError> {
        self.records
            .lock()
            .expect("in-memory alpha hypothesis repository lock should not be poisoned")
            .insert(registration.hypothesis_id.clone(), registration);
        Ok(())
    }

    fn load(
        &self,
        hypothesis_id: &str,
    ) -> Result<Option<AlphaHypothesisRegistration>, HypothesisRegistryServiceError> {
        let normalized = normalize_research_identifier(hypothesis_id);
        Ok(self
            .records
            .lock()
            .expect("in-memory alpha hypothesis repository lock should not be poisoned")
            .get(&normalized)
            .cloned())
    }
}

#[derive(Debug, Clone)]
pub struct StaticDatasetSnapshotRegistry {
    // Story 6.1 seam: until a dedicated registry service/table lands in Stories 6.2/6.3,
    // registration state is sourced from deterministic in-memory snapshots.
    registered_versions: BTreeSet<String>,
    available: bool,
}

impl StaticDatasetSnapshotRegistry {
    pub fn new(registered_versions: impl IntoIterator<Item = String>) -> Self {
        let mut normalized = BTreeSet::new();
        for version in registered_versions {
            let canonical = normalize_research_identifier(&version);
            if !canonical.is_empty() {
                normalized.insert(canonical);
            }
        }
        Self {
            registered_versions: normalized,
            available: true,
        }
    }

    pub fn unavailable() -> Self {
        Self {
            registered_versions: BTreeSet::new(),
            available: false,
        }
    }
}

impl Default for StaticDatasetSnapshotRegistry {
    fn default() -> Self {
        let from_env = std::env::var("RESEARCH_DATASET_SNAPSHOT_VERSIONS")
            .ok()
            .map(|value| {
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|item| !item.is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        if from_env.is_empty() {
            return Self::new(["dataset::v1".to_string()]);
        }

        Self::new(from_env)
    }
}

impl DatasetSnapshotRegistryPort for StaticDatasetSnapshotRegistry {
    fn is_registered(
        &self,
        feature_set_version: &str,
    ) -> Result<bool, HypothesisRegistryServiceError> {
        if !self.available {
            return Err(
                HypothesisRegistryServiceError::dataset_snapshot_unavailable(
                    "dataset snapshot registry dependency is unavailable",
                ),
            );
        }
        let normalized = normalize_research_identifier(feature_set_version);
        Ok(self.registered_versions.contains(&normalized))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_upsert_input() -> UpsertAlphaHypothesisInput {
        UpsertAlphaHypothesisInput {
            actor_id: "ops-1".to_string(),
            actor_role: "operational_control".to_string(),
            hypothesis_id: "alpha::mean-reversion".to_string(),
            feature_set_version: "dataset::v1".to_string(),
            target_regime: "overnight".to_string(),
            expected_edge_source: "liquidity_dislocation".to_string(),
            training_window_start_utc: "2026-04-01T00:00:00Z".to_string(),
            training_window_end_utc: "2026-04-02T00:00:00Z".to_string(),
            risk_assumptions: json!({
                "max_drawdown_pct": 2.5,
                "min_depth_usd": 10000.0
            }),
            correlation_id: "corr-hypothesis-upsert-001".to_string(),
            updated_at_utc: "2026-04-07T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn upsert_rejects_unauthorized_role() {
        let service = HypothesisRegistryService::default();
        let mut input = sample_upsert_input();
        input.actor_role = "read_only_analytics".to_string();

        let error = service
            .upsert_alpha_hypothesis(input)
            .expect_err("unauthorized mutation role should fail closed");
        assert_eq!(
            error.code,
            AlphaHypothesisReasonCode::UnauthorizedRole.code()
        );
    }

    #[test]
    fn upsert_rejects_unregistered_dataset_snapshot() {
        let service = HypothesisRegistryService::in_memory().with_dataset_registry(Arc::new(
            StaticDatasetSnapshotRegistry::new(["dataset::v2".to_string()]),
        ));

        let error = service
            .upsert_alpha_hypothesis(sample_upsert_input())
            .expect_err("unregistered feature_set_version should be denied");
        assert_eq!(
            error.code,
            AlphaHypothesisReasonCode::DatasetSnapshotUnresolved.code()
        );
    }

    #[test]
    fn upsert_rejects_unavailable_dataset_snapshot_registry() {
        let service = HypothesisRegistryService::in_memory()
            .with_dataset_registry(Arc::new(StaticDatasetSnapshotRegistry::unavailable()));

        let error = service
            .upsert_alpha_hypothesis(sample_upsert_input())
            .expect_err("unavailable registry should fail closed");
        assert_eq!(
            error.code,
            AlphaHypothesisReasonCode::DatasetSnapshotUnavailable.code()
        );
    }

    #[test]
    fn upsert_returns_registered_then_updated_reason_codes() {
        let service = HypothesisRegistryService::default();

        let first = service
            .upsert_alpha_hypothesis(sample_upsert_input())
            .expect("first upsert should register");
        assert_eq!(
            first.reason_code,
            AlphaHypothesisReasonCode::Registered.code()
        );

        let second = service
            .upsert_alpha_hypothesis(sample_upsert_input())
            .expect("second upsert should update");
        assert_eq!(
            second.reason_code,
            AlphaHypothesisReasonCode::Updated.code()
        );
    }

    #[test]
    fn read_returns_not_found_error_for_missing_hypothesis() {
        let service = HypothesisRegistryService::default();
        let error = service
            .read_alpha_hypothesis(ReadAlphaHypothesisInput {
                actor_id: "ops-1".to_string(),
                actor_role: "operational_control".to_string(),
                hypothesis_id: "alpha::missing".to_string(),
                correlation_id: "corr-read-001".to_string(),
                queried_at_utc: "2026-04-07T00:05:00Z".to_string(),
            })
            .expect_err("missing hypothesis should return explicit not found error");
        assert_eq!(error.code, AlphaHypothesisReasonCode::NotFound.code());
    }

    #[test]
    fn read_allows_read_only_analytics_role() {
        let service = HypothesisRegistryService::default();
        service
            .upsert_alpha_hypothesis(sample_upsert_input())
            .expect("seed upsert should succeed");

        let evidence = service
            .read_alpha_hypothesis(ReadAlphaHypothesisInput {
                actor_id: "reader-1".to_string(),
                actor_role: "read_only_analytics".to_string(),
                hypothesis_id: "alpha::mean-reversion".to_string(),
                correlation_id: "corr-read-002".to_string(),
                queried_at_utc: "2026-04-07T00:10:00Z".to_string(),
            })
            .expect("read_only_analytics should read hypothesis");
        assert_eq!(evidence.hypothesis_id, "alpha::mean-reversion");
        assert_eq!(evidence.reason_code, AlphaHypothesisReasonCode::Read.code());
    }
}
