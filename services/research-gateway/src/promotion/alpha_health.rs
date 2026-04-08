use domain::alerts::{
    AlertContractError, AlertReasonCode, AlertSeverity, AlertTriggerDecision, IncidentAlert,
    build_incident_alert, compose_alert_identifier, should_emit_alert,
};
use domain::research::{
    AlphaHealthContractError, AlphaHealthMetricRecord, AlphaHealthReasonCode,
    AlphaHealthThresholdDefinition, AlphaHealthValidationIssue, AlphaThresholdBreachRecord,
    CounterfactualReplayRunState, PromotionDecisionState, ShadowEvaluationState,
    canonicalize_alpha_health_metric_record, canonicalize_alpha_threshold_breach_record,
    evaluate_alpha_health_thresholds, normalize_research_identifier,
    parse_alpha_health_utc_timestamp,
};
use persistence::postgres::alpha_health_metrics::{
    AlphaHealthPersistenceError,
    list_alpha_health_metrics_by_alpha as pg_list_alpha_health_metrics,
    list_alpha_threshold_breaches_by_alpha as pg_list_alpha_threshold_breaches,
    load_alpha_health_metric as pg_load_alpha_health_metric,
    load_alpha_threshold_breach as pg_load_alpha_threshold_breach,
    upsert_alpha_health_metric as pg_upsert_alpha_health_metric,
    upsert_alpha_health_metric_with_breaches as pg_upsert_alpha_health_metric_with_breaches,
    upsert_alpha_threshold_breach as pg_upsert_alpha_threshold_breach,
};
use persistence::postgres::counterfactual_replay_runs::{
    CounterfactualReplayPersistenceError,
    list_counterfactual_replay_runs_by_candidate as pg_list_counterfactual_replay_runs,
};
use persistence::postgres::incident_alerts::{
    AlertPersistenceError, create_incident_alert, load_recent_incident_alerts,
};
use persistence::postgres::promotion_decisions::{
    PromotionDecisionPersistenceError,
    list_promotion_decisions_by_candidate as pg_list_promotion_decisions,
};
use persistence::postgres::shadow_evaluations::{
    ShadowEvaluationPersistenceError,
    list_shadow_evaluations_by_candidate as pg_list_shadow_evaluations,
};
use serde::Serialize;
use sqlx::PgPool;
use std::collections::{BTreeMap, hash_map::DefaultHasher};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

const DEFAULT_LIST_LIMIT: i64 = 25;
const MAX_LIST_LIMIT: i64 = 200;
const ALERT_DEDUPE_WINDOW_SECONDS: i64 = 60;
const ALPHA_HEALTH_EVIDENCE_LINK: &str =
    "https://docs.example.com/operations/alpha-live-health-threshold-monitoring";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AlphaHealthServiceError {
    pub code: &'static str,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub field_errors: Vec<AlphaHealthValidationIssue>,
}

impl AlphaHealthServiceError {
    pub fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<AlphaHealthValidationIssue>,
    ) -> Self {
        Self {
            code: AlphaHealthReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    fn unauthorized_mutation_role() -> Self {
        Self {
            code: AlphaHealthReasonCode::UnauthorizedRole.code(),
            message: "actor role is not authorized for alpha-health mutations".to_string(),
            field_errors: Vec::new(),
        }
    }

    fn unauthorized_read_role() -> Self {
        Self {
            code: AlphaHealthReasonCode::UnauthorizedRole.code(),
            message: "actor role is not authorized for alpha-health reads".to_string(),
            field_errors: Vec::new(),
        }
    }

    fn metric_not_found(metric_id: &str) -> Self {
        Self {
            code: AlphaHealthReasonCode::MetricNotFound.code(),
            message: format!("alpha-health metric `{metric_id}` was not found"),
            field_errors: Vec::new(),
        }
    }

    fn breach_not_found(breach_id: &str) -> Self {
        Self {
            code: AlphaHealthReasonCode::BreachNotFound.code(),
            message: format!("alpha-health breach `{breach_id}` was not found"),
            field_errors: Vec::new(),
        }
    }

    fn dependency_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: AlphaHealthReasonCode::DependencyUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    fn state_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: AlphaHealthReasonCode::StateUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    fn persistence_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: AlphaHealthReasonCode::PersistenceUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }
}

impl Display for AlphaHealthServiceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for AlphaHealthServiceError {}

#[derive(Debug, Clone)]
pub struct StartAlphaHealthMetricInput {
    pub actor_id: String,
    pub actor_role: String,
    pub alpha_id: String,
    pub rolling_sharpe: f64,
    pub rolling_hit_rate: f64,
    pub rolling_drawdown: f64,
    pub stability_score: f64,
    pub windows: Vec<domain::research::AlphaHealthAttributionWindowMetrics>,
    pub thresholds: Vec<AlphaHealthThresholdDefinition>,
    pub correlation_id: String,
    pub requested_at_utc: String,
}

#[derive(Debug, Clone)]
pub struct ReadAlphaHealthMetricInput {
    pub actor_id: String,
    pub actor_role: String,
    pub metric_id: String,
    pub correlation_id: String,
    pub queried_at_utc: String,
}

#[derive(Debug, Clone)]
pub struct ListAlphaHealthMetricsInput {
    pub actor_id: String,
    pub actor_role: String,
    pub alpha_id: String,
    pub limit: Option<i64>,
    pub recorded_after_utc: Option<String>,
    pub recorded_before_utc: Option<String>,
    pub correlation_id: String,
    pub queried_at_utc: String,
}

#[derive(Debug, Clone)]
pub struct ReadAlphaThresholdBreachInput {
    pub actor_id: String,
    pub actor_role: String,
    pub breach_id: String,
    pub correlation_id: String,
    pub queried_at_utc: String,
}

#[derive(Debug, Clone)]
pub struct ListAlphaThresholdBreachesInput {
    pub actor_id: String,
    pub actor_role: String,
    pub alpha_id: String,
    pub limit: Option<i64>,
    pub breached_after_utc: Option<String>,
    pub breached_before_utc: Option<String>,
    pub correlation_id: String,
    pub queried_at_utc: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AlphaHealthMetricEvidence {
    pub metric: AlphaHealthMetricRecord,
    pub breaches: Vec<AlphaThresholdBreachRecord>,
    pub reason_code: String,
    pub alert_emitted: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AlphaThresholdBreachEvidence {
    pub breach: AlphaThresholdBreachRecord,
    pub reason_code: String,
}

pub trait AlphaHealthOrchestrator: Send + Sync {
    fn start_alpha_health_metric(
        &self,
        input: StartAlphaHealthMetricInput,
    ) -> Result<AlphaHealthMetricEvidence, AlphaHealthServiceError>;

    fn read_alpha_health_metric(
        &self,
        input: ReadAlphaHealthMetricInput,
    ) -> Result<AlphaHealthMetricEvidence, AlphaHealthServiceError>;

    fn list_alpha_health_metrics(
        &self,
        input: ListAlphaHealthMetricsInput,
    ) -> Result<Vec<AlphaHealthMetricRecord>, AlphaHealthServiceError>;

    fn read_alpha_threshold_breach(
        &self,
        input: ReadAlphaThresholdBreachInput,
    ) -> Result<AlphaThresholdBreachEvidence, AlphaHealthServiceError>;

    fn list_alpha_threshold_breaches(
        &self,
        input: ListAlphaThresholdBreachesInput,
    ) -> Result<Vec<AlphaThresholdBreachRecord>, AlphaHealthServiceError>;
}

pub trait AlphaHealthRepositoryPort: Send + Sync {
    fn upsert_metric(&self, metric: AlphaHealthMetricRecord)
    -> Result<(), AlphaHealthServiceError>;
    fn upsert_metric_and_breaches(
        &self,
        metric: AlphaHealthMetricRecord,
        breaches: Vec<AlphaThresholdBreachRecord>,
    ) -> Result<(), AlphaHealthServiceError>;
    fn load_metric(
        &self,
        metric_id: &str,
    ) -> Result<Option<AlphaHealthMetricRecord>, AlphaHealthServiceError>;
    fn list_metrics(
        &self,
        alpha_id: &str,
        recorded_after_utc: Option<&str>,
        recorded_before_utc: Option<&str>,
        limit: i64,
    ) -> Result<Vec<AlphaHealthMetricRecord>, AlphaHealthServiceError>;
    fn upsert_breach(
        &self,
        breach: AlphaThresholdBreachRecord,
    ) -> Result<(), AlphaHealthServiceError>;
    fn load_breach(
        &self,
        breach_id: &str,
    ) -> Result<Option<AlphaThresholdBreachRecord>, AlphaHealthServiceError>;
    fn list_breaches(
        &self,
        alpha_id: &str,
        breached_after_utc: Option<&str>,
        breached_before_utc: Option<&str>,
        limit: i64,
    ) -> Result<Vec<AlphaThresholdBreachRecord>, AlphaHealthServiceError>;
}

pub trait LiveMonitoringContextPort: Send + Sync {
    fn ensure_live_monitoring_scope(&self, alpha_id: &str) -> Result<(), AlphaHealthServiceError>;
}

pub trait IncidentAlertPort: Send + Sync {
    fn maybe_emit_breach_alert(
        &self,
        breach: &AlphaThresholdBreachRecord,
    ) -> Result<bool, AlphaHealthServiceError>;
}

#[derive(Clone)]
pub struct AlphaHealthService {
    repository: Arc<dyn AlphaHealthRepositoryPort>,
    live_monitoring_context: Arc<dyn LiveMonitoringContextPort>,
    incident_alert_port: Arc<dyn IncidentAlertPort>,
    operation_lock: Arc<Mutex<()>>,
}

impl AlphaHealthService {
    pub fn new(
        repository: Arc<dyn AlphaHealthRepositoryPort>,
        live_monitoring_context: Arc<dyn LiveMonitoringContextPort>,
        incident_alert_port: Arc<dyn IncidentAlertPort>,
    ) -> Self {
        Self {
            repository,
            live_monitoring_context,
            incident_alert_port,
            operation_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn in_memory() -> Self {
        Self::new(
            Arc::new(InMemoryAlphaHealthRepository::default()),
            Arc::new(StaticLiveMonitoringContextPort),
            Arc::new(InMemoryIncidentAlertPort::default()),
        )
    }

    pub fn postgres(pool: PgPool) -> Self {
        Self::new(
            Arc::new(PostgresAlphaHealthRepository::new(pool.clone())),
            Arc::new(PostgresLiveMonitoringContextPort::new(pool.clone())),
            Arc::new(PostgresIncidentAlertPort::new(pool)),
        )
    }

    pub fn with_live_monitoring_context_port(
        mut self,
        live_monitoring_context: Arc<dyn LiveMonitoringContextPort>,
    ) -> Self {
        self.live_monitoring_context = live_monitoring_context;
        self
    }

    pub fn with_incident_alert_port(
        mut self,
        incident_alert_port: Arc<dyn IncidentAlertPort>,
    ) -> Self {
        self.incident_alert_port = incident_alert_port;
        self
    }

    fn lock_operations(&self) -> Result<std::sync::MutexGuard<'_, ()>, AlphaHealthServiceError> {
        self.operation_lock.lock().map_err(|_| {
            AlphaHealthServiceError::persistence_unavailable(
                "alpha-health operation lock poisoned by prior panic",
            )
        })
    }
}

impl Default for AlphaHealthService {
    fn default() -> Self {
        Self::in_memory()
    }
}

impl AlphaHealthOrchestrator for AlphaHealthService {
    fn start_alpha_health_metric(
        &self,
        input: StartAlphaHealthMetricInput,
    ) -> Result<AlphaHealthMetricEvidence, AlphaHealthServiceError> {
        if let Err(error) = validate_mutation_role(&input.actor_role) {
            emit_alpha_health_telemetry(
                "alpha_health_start_v1",
                "alpha_health_start",
                "deny",
                &input.actor_id,
                &input.alpha_id,
                None,
                None,
                None,
                error.code,
                &input.correlation_id,
                &input.requested_at_utc,
            );
            return Err(error);
        }
        validate_non_empty("actor_id", &input.actor_id)?;
        validate_non_empty("alpha_id", &input.alpha_id)?;
        validate_non_empty("correlation_id", &input.correlation_id)?;
        validate_non_empty("requested_at_utc", &input.requested_at_utc)?;
        validate_utc_timestamp("requested_at_utc", &input.requested_at_utc)?;

        let normalized_alpha_id = normalize_research_identifier(&input.alpha_id);
        if normalized_alpha_id.is_empty() {
            return Err(AlphaHealthServiceError::invalid_payload(
                "alpha_id cannot be blank",
                vec![AlphaHealthValidationIssue {
                    field: "alpha_id".to_string(),
                    code: AlphaHealthReasonCode::InvalidPayload.code(),
                    message: "alpha_id cannot be blank".to_string(),
                }],
            ));
        }
        let metric_id = domain::research::compose_alpha_health_metric_id(
            &normalized_alpha_id,
            &input.requested_at_utc,
        )
        .map_err(map_contract_error)?;
        let _requested_at = parse_alpha_health_utc_timestamp(&input.requested_at_utc)
            .map_err(map_contract_error)?;
        let _lock = self.lock_operations()?;

        if let Err(error) = self
            .live_monitoring_context
            .ensure_live_monitoring_scope(&normalized_alpha_id)
        {
            emit_alpha_health_telemetry(
                "alpha_health_start_v1",
                "alpha_health_start",
                "fail_closed",
                &input.actor_id,
                &normalized_alpha_id,
                Some(metric_id.as_str()),
                None,
                None,
                error.code,
                &input.correlation_id,
                &input.requested_at_utc,
            );
            return Err(error);
        }

        let metric = match canonicalize_alpha_health_metric_record(&AlphaHealthMetricRecord {
            metric_id,
            alpha_id: normalized_alpha_id.clone(),
            rolling_sharpe: input.rolling_sharpe,
            rolling_hit_rate: input.rolling_hit_rate,
            rolling_drawdown: input.rolling_drawdown,
            stability_score: input.stability_score,
            windows: input.windows,
            reason_code: AlphaHealthReasonCode::MetricRecorded.code().to_string(),
            actor_id: input.actor_id.clone(),
            correlation_id: input.correlation_id.clone(),
            recorded_at_utc: input.requested_at_utc.clone(),
        }) {
            Ok(metric) => metric,
            Err(error) => {
                let error = map_contract_error(error);
                emit_alpha_health_telemetry(
                    "alpha_health_start_v1",
                    "alpha_health_start",
                    "fail_closed",
                    &input.actor_id,
                    &normalized_alpha_id,
                    None,
                    None,
                    None,
                    error.code,
                    &input.correlation_id,
                    &input.requested_at_utc,
                );
                return Err(error);
            }
        };

        let breaches = match evaluate_alpha_health_thresholds(&metric, &input.thresholds) {
            Ok(breaches) => breaches,
            Err(error) => {
                let error = map_contract_error(error);
                emit_alpha_health_telemetry(
                    "alpha_health_start_v1",
                    "alpha_health_start",
                    "fail_closed",
                    &input.actor_id,
                    &metric.alpha_id,
                    Some(metric.metric_id.as_str()),
                    None,
                    None,
                    error.code,
                    &input.correlation_id,
                    &input.requested_at_utc,
                );
                return Err(error);
            }
        };
        if let Err(error) = self
            .repository
            .upsert_metric_and_breaches(metric.clone(), breaches.clone())
        {
            emit_alpha_health_telemetry(
                "alpha_health_start_v1",
                "alpha_health_start",
                "fail_closed",
                &input.actor_id,
                &metric.alpha_id,
                Some(metric.metric_id.as_str()),
                None,
                None,
                error.code,
                &input.correlation_id,
                &input.requested_at_utc,
            );
            return Err(error);
        }
        let mut alert_emitted = false;
        for breach in &breaches {
            match self.incident_alert_port.maybe_emit_breach_alert(breach) {
                Ok(emitted) => {
                    if emitted {
                        alert_emitted = true;
                    }
                }
                Err(error) => {
                    emit_alpha_health_telemetry(
                        "alpha_health_start_v1",
                        "alpha_health_start",
                        "fail_closed",
                        &input.actor_id,
                        &metric.alpha_id,
                        Some(metric.metric_id.as_str()),
                        Some(breach.metric_key.as_str()),
                        Some(breach.breach_id.as_str()),
                        error.code,
                        &input.correlation_id,
                        &input.requested_at_utc,
                    );
                    return Err(error);
                }
            }
        }

        let reason_code = if breaches.is_empty() {
            AlphaHealthReasonCode::ThresholdSatisfied.code().to_string()
        } else {
            AlphaHealthReasonCode::ThresholdBreachDetected
                .code()
                .to_string()
        };
        emit_alpha_health_telemetry(
            "alpha_health_start_v1",
            "alpha_health_start",
            if breaches.is_empty() {
                "allow"
            } else {
                "breach"
            },
            &input.actor_id,
            &metric.alpha_id,
            Some(metric.metric_id.as_str()),
            breaches.first().map(|breach| breach.metric_key.as_str()),
            breaches.first().map(|breach| breach.breach_id.as_str()),
            &reason_code,
            &input.correlation_id,
            &input.requested_at_utc,
        );

        Ok(AlphaHealthMetricEvidence {
            metric,
            breaches,
            reason_code,
            alert_emitted,
        })
    }

    fn read_alpha_health_metric(
        &self,
        input: ReadAlphaHealthMetricInput,
    ) -> Result<AlphaHealthMetricEvidence, AlphaHealthServiceError> {
        if let Err(error) = validate_read_role(&input.actor_role) {
            emit_alpha_health_telemetry(
                "alpha_health_read_metric_v1",
                "alpha_health_read_metric",
                "deny",
                &input.actor_id,
                "unknown_alpha_id",
                Some(input.metric_id.as_str()),
                None,
                None,
                error.code,
                &input.correlation_id,
                &input.queried_at_utc,
            );
            return Err(error);
        }
        validate_non_empty("actor_id", &input.actor_id)?;
        validate_non_empty("metric_id", &input.metric_id)?;
        validate_non_empty("correlation_id", &input.correlation_id)?;
        validate_non_empty("queried_at_utc", &input.queried_at_utc)?;
        validate_utc_timestamp("queried_at_utc", &input.queried_at_utc)?;

        let normalized_metric_id = normalize_research_identifier(&input.metric_id);
        let Some(metric) = self.repository.load_metric(&normalized_metric_id)? else {
            return Err(AlphaHealthServiceError::metric_not_found(
                &normalized_metric_id,
            ));
        };

        emit_alpha_health_telemetry(
            "alpha_health_read_metric_v1",
            "alpha_health_read_metric",
            "allow",
            &input.actor_id,
            &metric.alpha_id,
            Some(metric.metric_id.as_str()),
            None,
            None,
            AlphaHealthReasonCode::MetricRead.code(),
            &input.correlation_id,
            &input.queried_at_utc,
        );
        Ok(AlphaHealthMetricEvidence {
            metric,
            breaches: Vec::new(),
            reason_code: AlphaHealthReasonCode::MetricRead.code().to_string(),
            alert_emitted: false,
        })
    }

    fn list_alpha_health_metrics(
        &self,
        input: ListAlphaHealthMetricsInput,
    ) -> Result<Vec<AlphaHealthMetricRecord>, AlphaHealthServiceError> {
        if let Err(error) = validate_read_role(&input.actor_role) {
            emit_alpha_health_telemetry(
                "alpha_health_list_metrics_v1",
                "alpha_health_list_metrics",
                "deny",
                &input.actor_id,
                &input.alpha_id,
                None,
                None,
                None,
                error.code,
                &input.correlation_id,
                &input.queried_at_utc,
            );
            return Err(error);
        }
        validate_non_empty("actor_id", &input.actor_id)?;
        validate_non_empty("alpha_id", &input.alpha_id)?;
        validate_non_empty("correlation_id", &input.correlation_id)?;
        validate_non_empty("queried_at_utc", &input.queried_at_utc)?;
        validate_utc_timestamp("queried_at_utc", &input.queried_at_utc)?;
        if let Some(limit) = input.limit
            && limit <= 0
        {
            return Err(AlphaHealthServiceError::invalid_payload(
                "limit must be greater than 0",
                vec![AlphaHealthValidationIssue {
                    field: "limit".to_string(),
                    code: AlphaHealthReasonCode::InvalidPayload.code(),
                    message: "limit must be greater than 0".to_string(),
                }],
            ));
        }

        let normalized_after = normalize_optional_timestamp(
            "recorded_after_utc",
            input.recorded_after_utc.as_deref(),
        )?;
        let normalized_before = normalize_optional_timestamp(
            "recorded_before_utc",
            input.recorded_before_utc.as_deref(),
        )?;
        if let (Some(recorded_after), Some(recorded_before)) =
            (normalized_after.as_deref(), normalized_before.as_deref())
        {
            let recorded_after_ts =
                parse_alpha_health_utc_timestamp(recorded_after).map_err(map_contract_error)?;
            let recorded_before_ts =
                parse_alpha_health_utc_timestamp(recorded_before).map_err(map_contract_error)?;
            if recorded_before_ts <= recorded_after_ts {
                return Err(AlphaHealthServiceError::invalid_payload(
                    "recorded_before_utc must be greater than recorded_after_utc",
                    vec![AlphaHealthValidationIssue {
                        field: "recorded_before_utc".to_string(),
                        code: AlphaHealthReasonCode::InvalidPayload.code(),
                        message: "recorded_before_utc must be greater than recorded_after_utc"
                            .to_string(),
                    }],
                ));
            }
        }

        let normalized_alpha_id = normalize_research_identifier(&input.alpha_id);
        if normalized_alpha_id.is_empty() {
            return Err(AlphaHealthServiceError::invalid_payload(
                "alpha_id cannot be blank",
                vec![AlphaHealthValidationIssue {
                    field: "alpha_id".to_string(),
                    code: AlphaHealthReasonCode::InvalidPayload.code(),
                    message: "alpha_id cannot be blank".to_string(),
                }],
            ));
        }
        let limit = input
            .limit
            .unwrap_or(DEFAULT_LIST_LIMIT)
            .clamp(1, MAX_LIST_LIMIT);
        let metrics = self.repository.list_metrics(
            &normalized_alpha_id,
            normalized_after.as_deref(),
            normalized_before.as_deref(),
            limit,
        )?;
        emit_alpha_health_telemetry(
            "alpha_health_list_metrics_v1",
            "alpha_health_list_metrics",
            "allow",
            &input.actor_id,
            &normalized_alpha_id,
            None,
            None,
            None,
            AlphaHealthReasonCode::MetricListed.code(),
            &input.correlation_id,
            &input.queried_at_utc,
        );
        Ok(metrics)
    }

    fn read_alpha_threshold_breach(
        &self,
        input: ReadAlphaThresholdBreachInput,
    ) -> Result<AlphaThresholdBreachEvidence, AlphaHealthServiceError> {
        if let Err(error) = validate_read_role(&input.actor_role) {
            emit_alpha_health_telemetry(
                "alpha_health_read_breach_v1",
                "alpha_health_read_breach",
                "deny",
                &input.actor_id,
                "unknown_alpha_id",
                None,
                None,
                Some(input.breach_id.as_str()),
                error.code,
                &input.correlation_id,
                &input.queried_at_utc,
            );
            return Err(error);
        }
        validate_non_empty("actor_id", &input.actor_id)?;
        validate_non_empty("breach_id", &input.breach_id)?;
        validate_non_empty("correlation_id", &input.correlation_id)?;
        validate_non_empty("queried_at_utc", &input.queried_at_utc)?;
        validate_utc_timestamp("queried_at_utc", &input.queried_at_utc)?;

        let normalized_breach_id = normalize_research_identifier(&input.breach_id);
        let Some(breach) = self.repository.load_breach(&normalized_breach_id)? else {
            return Err(AlphaHealthServiceError::breach_not_found(
                &normalized_breach_id,
            ));
        };

        emit_alpha_health_telemetry(
            "alpha_health_read_breach_v1",
            "alpha_health_read_breach",
            "allow",
            &input.actor_id,
            &breach.alpha_id,
            Some(breach.metric_id.as_str()),
            Some(breach.metric_key.as_str()),
            Some(breach.breach_id.as_str()),
            AlphaHealthReasonCode::ThresholdBreachRead.code(),
            &input.correlation_id,
            &input.queried_at_utc,
        );
        Ok(AlphaThresholdBreachEvidence {
            breach,
            reason_code: AlphaHealthReasonCode::ThresholdBreachRead
                .code()
                .to_string(),
        })
    }

    fn list_alpha_threshold_breaches(
        &self,
        input: ListAlphaThresholdBreachesInput,
    ) -> Result<Vec<AlphaThresholdBreachRecord>, AlphaHealthServiceError> {
        if let Err(error) = validate_read_role(&input.actor_role) {
            emit_alpha_health_telemetry(
                "alpha_health_list_breaches_v1",
                "alpha_health_list_breaches",
                "deny",
                &input.actor_id,
                &input.alpha_id,
                None,
                None,
                None,
                error.code,
                &input.correlation_id,
                &input.queried_at_utc,
            );
            return Err(error);
        }
        validate_non_empty("actor_id", &input.actor_id)?;
        validate_non_empty("alpha_id", &input.alpha_id)?;
        validate_non_empty("correlation_id", &input.correlation_id)?;
        validate_non_empty("queried_at_utc", &input.queried_at_utc)?;
        validate_utc_timestamp("queried_at_utc", &input.queried_at_utc)?;
        if let Some(limit) = input.limit
            && limit <= 0
        {
            return Err(AlphaHealthServiceError::invalid_payload(
                "limit must be greater than 0",
                vec![AlphaHealthValidationIssue {
                    field: "limit".to_string(),
                    code: AlphaHealthReasonCode::InvalidPayload.code(),
                    message: "limit must be greater than 0".to_string(),
                }],
            ));
        }

        let normalized_after = normalize_optional_timestamp(
            "breached_after_utc",
            input.breached_after_utc.as_deref(),
        )?;
        let normalized_before = normalize_optional_timestamp(
            "breached_before_utc",
            input.breached_before_utc.as_deref(),
        )?;
        if let (Some(breached_after), Some(breached_before)) =
            (normalized_after.as_deref(), normalized_before.as_deref())
        {
            let breached_after_ts =
                parse_alpha_health_utc_timestamp(breached_after).map_err(map_contract_error)?;
            let breached_before_ts =
                parse_alpha_health_utc_timestamp(breached_before).map_err(map_contract_error)?;
            if breached_before_ts <= breached_after_ts {
                return Err(AlphaHealthServiceError::invalid_payload(
                    "breached_before_utc must be greater than breached_after_utc",
                    vec![AlphaHealthValidationIssue {
                        field: "breached_before_utc".to_string(),
                        code: AlphaHealthReasonCode::InvalidPayload.code(),
                        message: "breached_before_utc must be greater than breached_after_utc"
                            .to_string(),
                    }],
                ));
            }
        }

        let normalized_alpha_id = normalize_research_identifier(&input.alpha_id);
        if normalized_alpha_id.is_empty() {
            return Err(AlphaHealthServiceError::invalid_payload(
                "alpha_id cannot be blank",
                vec![AlphaHealthValidationIssue {
                    field: "alpha_id".to_string(),
                    code: AlphaHealthReasonCode::InvalidPayload.code(),
                    message: "alpha_id cannot be blank".to_string(),
                }],
            ));
        }
        let limit = input
            .limit
            .unwrap_or(DEFAULT_LIST_LIMIT)
            .clamp(1, MAX_LIST_LIMIT);
        let breaches = self.repository.list_breaches(
            &normalized_alpha_id,
            normalized_after.as_deref(),
            normalized_before.as_deref(),
            limit,
        )?;
        emit_alpha_health_telemetry(
            "alpha_health_list_breaches_v1",
            "alpha_health_list_breaches",
            "allow",
            &input.actor_id,
            &normalized_alpha_id,
            None,
            None,
            None,
            AlphaHealthReasonCode::ThresholdBreachListed.code(),
            &input.correlation_id,
            &input.queried_at_utc,
        );
        Ok(breaches)
    }
}

fn validate_mutation_role(role: &str) -> Result<(), AlphaHealthServiceError> {
    match role {
        "operational_control" | "administrative_actions" => Ok(()),
        _ => Err(AlphaHealthServiceError::unauthorized_mutation_role()),
    }
}

fn validate_read_role(role: &str) -> Result<(), AlphaHealthServiceError> {
    match role {
        "read_only_analytics" | "operational_control" | "administrative_actions" => Ok(()),
        _ => Err(AlphaHealthServiceError::unauthorized_read_role()),
    }
}

fn validate_non_empty(field: &str, value: &str) -> Result<(), AlphaHealthServiceError> {
    if value.trim().is_empty() {
        return Err(AlphaHealthServiceError::invalid_payload(
            format!("{field} cannot be blank"),
            vec![AlphaHealthValidationIssue {
                field: field.to_string(),
                code: AlphaHealthReasonCode::InvalidPayload.code(),
                message: format!("{field} cannot be blank"),
            }],
        ));
    }
    Ok(())
}

fn validate_utc_timestamp(field: &str, value: &str) -> Result<(), AlphaHealthServiceError> {
    parse_alpha_health_utc_timestamp(value).map_err(|_| {
        AlphaHealthServiceError::invalid_payload(
            format!("{field} must be RFC3339 UTC"),
            vec![AlphaHealthValidationIssue {
                field: field.to_string(),
                code: AlphaHealthReasonCode::InvalidPayload.code(),
                message: format!("{field} must be RFC3339 UTC"),
            }],
        )
    })?;
    Ok(())
}

fn normalize_optional_timestamp(
    field: &str,
    value: Option<&str>,
) -> Result<Option<String>, AlphaHealthServiceError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    validate_utc_timestamp(field, trimmed)?;
    Ok(Some(trimmed.to_string()))
}

fn map_contract_error(error: AlphaHealthContractError) -> AlphaHealthServiceError {
    AlphaHealthServiceError::invalid_payload(error.message, error.field_errors)
}

fn map_alert_contract_error(error: AlertContractError) -> AlphaHealthServiceError {
    AlphaHealthServiceError::invalid_payload(
        error.message,
        error
            .field_errors
            .into_iter()
            .map(|issue| AlphaHealthValidationIssue {
                field: issue.field.to_string(),
                code: issue.code,
                message: issue.message,
            })
            .collect(),
    )
}

#[allow(clippy::too_many_arguments)]
fn emit_alpha_health_telemetry(
    event_name: &'static str,
    action: &'static str,
    outcome: &'static str,
    actor_id: &str,
    alpha_id: &str,
    metric_id: Option<&str>,
    metric_key: Option<&str>,
    breach_id: Option<&str>,
    reason_code: &str,
    correlation_id: &str,
    timestamp_utc: &str,
) {
    let event = AlphaHealthTelemetryEvent {
        event_name,
        action,
        outcome,
        actor_id,
        alpha_id,
        metric_id,
        metric_key,
        breach_id,
        reason_code,
        correlation_id,
        timestamp_utc,
        security_signal: if outcome == "deny" {
            Some(AlphaHealthSecuritySignal {
                name: "alpha_health_monitoring_denied_v1",
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
        serde_json::to_string(&event).expect("alpha-health telemetry should serialize")
    );
}

#[derive(Debug, Serialize)]
struct AlphaHealthTelemetryEvent<'a> {
    event_name: &'a str,
    action: &'a str,
    outcome: &'a str,
    actor_id: &'a str,
    alpha_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    metric_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metric_key: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    breach_id: Option<&'a str>,
    reason_code: &'a str,
    correlation_id: &'a str,
    timestamp_utc: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    security_signal: Option<AlphaHealthSecuritySignal<'a>>,
}

#[derive(Debug, Serialize)]
struct AlphaHealthSecuritySignal<'a> {
    name: &'a str,
    severity: &'a str,
    alert_compatible: bool,
    alert_target_seconds: u16,
}

#[derive(Debug, Clone)]
pub struct PostgresAlphaHealthRepository {
    pool: PgPool,
}

impl PostgresAlphaHealthRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn run_future<F, T>(&self, future: F) -> Result<T, AlphaHealthServiceError>
    where
        F: Future<Output = Result<T, AlphaHealthPersistenceError>>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future))
                .map_err(map_alpha_health_persistence_error),
            Err(_) => tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| {
                    AlphaHealthServiceError::persistence_unavailable(format!(
                        "failed to initialize async runtime: {error}"
                    ))
                })?
                .block_on(future)
                .map_err(map_alpha_health_persistence_error),
        }
    }
}

impl AlphaHealthRepositoryPort for PostgresAlphaHealthRepository {
    fn upsert_metric(
        &self,
        metric: AlphaHealthMetricRecord,
    ) -> Result<(), AlphaHealthServiceError> {
        self.run_future(pg_upsert_alpha_health_metric(&self.pool, &metric))
    }

    fn upsert_metric_and_breaches(
        &self,
        metric: AlphaHealthMetricRecord,
        breaches: Vec<AlphaThresholdBreachRecord>,
    ) -> Result<(), AlphaHealthServiceError> {
        self.run_future(pg_upsert_alpha_health_metric_with_breaches(
            &self.pool, &metric, &breaches,
        ))
    }

    fn load_metric(
        &self,
        metric_id: &str,
    ) -> Result<Option<AlphaHealthMetricRecord>, AlphaHealthServiceError> {
        self.run_future(pg_load_alpha_health_metric(&self.pool, metric_id))
    }

    fn list_metrics(
        &self,
        alpha_id: &str,
        recorded_after_utc: Option<&str>,
        recorded_before_utc: Option<&str>,
        limit: i64,
    ) -> Result<Vec<AlphaHealthMetricRecord>, AlphaHealthServiceError> {
        self.run_future(pg_list_alpha_health_metrics(
            &self.pool,
            alpha_id,
            recorded_after_utc,
            recorded_before_utc,
            limit,
        ))
    }

    fn upsert_breach(
        &self,
        breach: AlphaThresholdBreachRecord,
    ) -> Result<(), AlphaHealthServiceError> {
        self.run_future(pg_upsert_alpha_threshold_breach(&self.pool, &breach))
    }

    fn load_breach(
        &self,
        breach_id: &str,
    ) -> Result<Option<AlphaThresholdBreachRecord>, AlphaHealthServiceError> {
        self.run_future(pg_load_alpha_threshold_breach(&self.pool, breach_id))
    }

    fn list_breaches(
        &self,
        alpha_id: &str,
        breached_after_utc: Option<&str>,
        breached_before_utc: Option<&str>,
        limit: i64,
    ) -> Result<Vec<AlphaThresholdBreachRecord>, AlphaHealthServiceError> {
        self.run_future(pg_list_alpha_threshold_breaches(
            &self.pool,
            alpha_id,
            breached_after_utc,
            breached_before_utc,
            limit,
        ))
    }
}

fn map_alpha_health_persistence_error(
    error: AlphaHealthPersistenceError,
) -> AlphaHealthServiceError {
    match error.code {
        "alpha_health_query_failed" | "alpha_health_row_decode_failed" => {
            AlphaHealthServiceError::persistence_unavailable(error.message)
        }
        _ => AlphaHealthServiceError {
            code: error.code,
            message: error.message,
            field_errors: error.field_errors,
        },
    }
}

#[derive(Debug, Clone)]
pub struct PostgresLiveMonitoringContextPort {
    pool: PgPool,
}

impl PostgresLiveMonitoringContextPort {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn run_promotion_future<F, T>(&self, future: F) -> Result<T, AlphaHealthServiceError>
    where
        F: Future<Output = Result<T, PromotionDecisionPersistenceError>>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future))
                .map_err(map_promotion_decision_persistence_error),
            Err(_) => tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| {
                    AlphaHealthServiceError::dependency_unavailable(format!(
                        "failed to initialize async runtime: {error}"
                    ))
                })?
                .block_on(future)
                .map_err(map_promotion_decision_persistence_error),
        }
    }

    fn run_replay_future<F, T>(&self, future: F) -> Result<T, AlphaHealthServiceError>
    where
        F: Future<Output = Result<T, CounterfactualReplayPersistenceError>>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future))
                .map_err(map_replay_persistence_error),
            Err(_) => tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| {
                    AlphaHealthServiceError::dependency_unavailable(format!(
                        "failed to initialize async runtime: {error}"
                    ))
                })?
                .block_on(future)
                .map_err(map_replay_persistence_error),
        }
    }

    fn run_shadow_future<F, T>(&self, future: F) -> Result<T, AlphaHealthServiceError>
    where
        F: Future<Output = Result<T, ShadowEvaluationPersistenceError>>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future))
                .map_err(map_shadow_persistence_error),
            Err(_) => tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| {
                    AlphaHealthServiceError::dependency_unavailable(format!(
                        "failed to initialize async runtime: {error}"
                    ))
                })?
                .block_on(future)
                .map_err(map_shadow_persistence_error),
        }
    }
}

impl LiveMonitoringContextPort for PostgresLiveMonitoringContextPort {
    fn ensure_live_monitoring_scope(&self, alpha_id: &str) -> Result<(), AlphaHealthServiceError> {
        let latest_decision = self.run_promotion_future(pg_list_promotion_decisions(
            &self.pool, alpha_id, None, None, 1,
        ))?;
        let Some(decision) = latest_decision.into_iter().next() else {
            return Err(AlphaHealthServiceError::state_unavailable(
                "alpha_id must have an existing promotion decision for live monitoring",
            ));
        };
        if decision.decision_state != PromotionDecisionState::Allowed {
            return Err(AlphaHealthServiceError::state_unavailable(
                "alpha_id must have an allowed promotion decision for live monitoring",
            ));
        }

        let latest_replay = self.run_replay_future(pg_list_counterfactual_replay_runs(
            &self.pool, alpha_id, None, None, 1,
        ))?;
        let Some(replay) = latest_replay.into_iter().next() else {
            return Err(AlphaHealthServiceError::state_unavailable(
                "counterfactual replay evidence is required for live monitoring",
            ));
        };
        if replay.run_state != CounterfactualReplayRunState::Completed {
            return Err(AlphaHealthServiceError::state_unavailable(
                "counterfactual replay run must be completed for live monitoring",
            ));
        }

        let latest_shadow = self.run_shadow_future(pg_list_shadow_evaluations(
            &self.pool, alpha_id, None, None, 1,
        ))?;
        let Some(shadow) = latest_shadow.into_iter().next() else {
            return Err(AlphaHealthServiceError::state_unavailable(
                "shadow evaluation evidence is required for live monitoring",
            ));
        };
        if shadow.evaluation_state != ShadowEvaluationState::Completed {
            return Err(AlphaHealthServiceError::state_unavailable(
                "shadow evaluation must be completed for live monitoring",
            ));
        }
        Ok(())
    }
}

fn map_promotion_decision_persistence_error(
    error: PromotionDecisionPersistenceError,
) -> AlphaHealthServiceError {
    match error.code {
        "promotion_decision_query_failed" => {
            AlphaHealthServiceError::dependency_unavailable(error.message)
        }
        "promotion_decision_row_decode_failed" => {
            AlphaHealthServiceError::state_unavailable(error.message)
        }
        _ => AlphaHealthServiceError::invalid_payload(
            error.message,
            error
                .field_errors
                .into_iter()
                .map(|issue| AlphaHealthValidationIssue {
                    field: issue.field,
                    code: issue.code,
                    message: issue.message,
                })
                .collect(),
        ),
    }
}

fn map_replay_persistence_error(
    error: CounterfactualReplayPersistenceError,
) -> AlphaHealthServiceError {
    match error.code {
        "counterfactual_replay_run_query_failed" => {
            AlphaHealthServiceError::dependency_unavailable(error.message)
        }
        "counterfactual_replay_run_row_decode_failed" => {
            AlphaHealthServiceError::state_unavailable(error.message)
        }
        _ => AlphaHealthServiceError::invalid_payload(
            error.message,
            error
                .field_errors
                .into_iter()
                .map(|issue| AlphaHealthValidationIssue {
                    field: issue.field,
                    code: issue.code,
                    message: issue.message,
                })
                .collect(),
        ),
    }
}

fn map_shadow_persistence_error(
    error: ShadowEvaluationPersistenceError,
) -> AlphaHealthServiceError {
    match error.code {
        "shadow_evaluation_query_failed" => {
            AlphaHealthServiceError::dependency_unavailable(error.message)
        }
        "shadow_evaluation_row_decode_failed" => {
            AlphaHealthServiceError::state_unavailable(error.message)
        }
        _ => AlphaHealthServiceError::invalid_payload(
            error.message,
            error
                .field_errors
                .into_iter()
                .map(|issue| AlphaHealthValidationIssue {
                    field: issue.field,
                    code: issue.code,
                    message: issue.message,
                })
                .collect(),
        ),
    }
}

#[derive(Debug, Clone)]
pub struct PostgresIncidentAlertPort {
    pool: PgPool,
}

impl PostgresIncidentAlertPort {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn run_future<F, T>(&self, future: F) -> Result<T, AlphaHealthServiceError>
    where
        F: Future<Output = Result<T, AlertPersistenceError>>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future))
                .map_err(map_alert_persistence_error),
            Err(_) => tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| {
                    AlphaHealthServiceError::dependency_unavailable(format!(
                        "failed to initialize async runtime: {error}"
                    ))
                })?
                .block_on(future)
                .map_err(map_alert_persistence_error),
        }
    }
}

impl IncidentAlertPort for PostgresIncidentAlertPort {
    fn maybe_emit_breach_alert(
        &self,
        breach: &AlphaThresholdBreachRecord,
    ) -> Result<bool, AlphaHealthServiceError> {
        let dedupe_correlation_key = alpha_health_alert_correlation_key(breach);
        let prior_alerts = self.run_future(load_recent_incident_alerts(&self.pool, 200))?;
        let should_emit = should_emit_alert(
            &prior_alerts,
            AlertReasonCode::AlphaHealthThresholdBreach.code(),
            &dedupe_correlation_key,
            &breach.breached_at_utc,
            ALERT_DEDUPE_WINDOW_SECONDS,
        )
        .map_err(map_alert_contract_error)?;
        if !should_emit {
            return Ok(false);
        }
        let alert = alpha_health_alert_for_breach(breach)?;
        self.run_future(create_incident_alert(&self.pool, &alert))?;
        Ok(true)
    }
}

fn map_alert_persistence_error(error: AlertPersistenceError) -> AlphaHealthServiceError {
    match error.code {
        "alert_query_failed" => AlphaHealthServiceError::dependency_unavailable(error.message),
        "alert_row_decode_failed" => AlphaHealthServiceError::state_unavailable(error.message),
        "alert_constraint_violation" => {
            AlphaHealthServiceError::persistence_unavailable(error.message)
        }
        _ => AlphaHealthServiceError::invalid_payload(
            error.message,
            error
                .field_errors
                .into_iter()
                .map(|issue| AlphaHealthValidationIssue {
                    field: issue.field.to_string(),
                    code: issue.code,
                    message: issue.message,
                })
                .collect(),
        ),
    }
}

fn alpha_health_alert_for_breach(
    breach: &AlphaThresholdBreachRecord,
) -> Result<IncidentAlert, AlphaHealthServiceError> {
    let dedupe_correlation_key = alpha_health_alert_correlation_key(breach);
    let trigger = AlertTriggerDecision {
        severity: if breach.metric_key == domain::research::AlphaHealthMetricKey::RollingDrawdown {
            AlertSeverity::Critical
        } else {
            AlertSeverity::Warning
        },
        reason_code: AlertReasonCode::AlphaHealthThresholdBreach,
        impacted_subsystem: "alpha_health_monitoring".to_string(),
        cause: format!(
            "alpha `{}` breached `{}` threshold: {}",
            breach.alpha_id,
            breach.metric_key.as_str(),
            breach.breach_reason
        ),
        recommended_next_action:
            "Review live alpha telemetry and apply promotion lifecycle controls if degradation persists."
                .to_string(),
    };
    let alert_id = compose_alert_identifier(
        &dedupe_correlation_key,
        AlertReasonCode::AlphaHealthThresholdBreach,
        &breach.breached_at_utc,
    )
    .map_err(map_alert_contract_error)?;
    build_incident_alert(
        &trigger,
        &alert_id,
        &breach.breached_at_utc,
        &breach.correlation_id,
        ALPHA_HEALTH_EVIDENCE_LINK,
        None,
    )
    .map_err(map_alert_contract_error)
}

fn alpha_health_alert_correlation_key(breach: &AlphaThresholdBreachRecord) -> String {
    let normalized_correlation = domain::alerts::normalize_alert_identifier(&breach.correlation_id);
    let mut hasher = DefaultHasher::new();
    breach.breach_id.hash(&mut hasher);
    let suffix = format!("{:016x}", hasher.finish());
    let max_prefix_len = 120usize.saturating_sub(suffix.len() + 1);
    let prefix = if normalized_correlation.len() > max_prefix_len {
        &normalized_correlation[..max_prefix_len]
    } else {
        normalized_correlation.as_str()
    };
    format!("{prefix}-{suffix}")
}

#[derive(Debug, Default)]
pub struct InMemoryAlphaHealthRepository {
    metrics: Mutex<BTreeMap<String, AlphaHealthMetricRecord>>,
    breaches: Mutex<BTreeMap<String, AlphaThresholdBreachRecord>>,
}

impl AlphaHealthRepositoryPort for InMemoryAlphaHealthRepository {
    fn upsert_metric(
        &self,
        metric: AlphaHealthMetricRecord,
    ) -> Result<(), AlphaHealthServiceError> {
        let canonical =
            canonicalize_alpha_health_metric_record(&metric).map_err(map_contract_error)?;
        let mut metrics = self.metrics.lock().map_err(|_| {
            AlphaHealthServiceError::persistence_unavailable(
                "in-memory alpha-health metric store lock poisoned",
            )
        })?;
        metrics.insert(canonical.metric_id.clone(), canonical);
        Ok(())
    }

    fn upsert_metric_and_breaches(
        &self,
        metric: AlphaHealthMetricRecord,
        breaches: Vec<AlphaThresholdBreachRecord>,
    ) -> Result<(), AlphaHealthServiceError> {
        let canonical_metric =
            canonicalize_alpha_health_metric_record(&metric).map_err(map_contract_error)?;
        let canonical_breaches = breaches
            .into_iter()
            .map(|breach| {
                canonicalize_alpha_threshold_breach_record(&breach).map_err(map_contract_error)
            })
            .collect::<Result<Vec<_>, _>>()?;

        let mut metrics = self.metrics.lock().map_err(|_| {
            AlphaHealthServiceError::persistence_unavailable(
                "in-memory alpha-health metric store lock poisoned",
            )
        })?;
        let mut stored_breaches = self.breaches.lock().map_err(|_| {
            AlphaHealthServiceError::persistence_unavailable(
                "in-memory alpha-health breach store lock poisoned",
            )
        })?;

        metrics.insert(canonical_metric.metric_id.clone(), canonical_metric);
        for breach in canonical_breaches {
            stored_breaches.insert(breach.breach_id.clone(), breach);
        }
        Ok(())
    }

    fn load_metric(
        &self,
        metric_id: &str,
    ) -> Result<Option<AlphaHealthMetricRecord>, AlphaHealthServiceError> {
        let normalized_metric_id = normalize_research_identifier(metric_id);
        let metrics = self.metrics.lock().map_err(|_| {
            AlphaHealthServiceError::persistence_unavailable(
                "in-memory alpha-health metric store lock poisoned",
            )
        })?;
        metrics
            .get(&normalized_metric_id)
            .cloned()
            .map(|record| {
                canonicalize_alpha_health_metric_record(&record).map_err(map_contract_error)
            })
            .transpose()
    }

    fn list_metrics(
        &self,
        alpha_id: &str,
        recorded_after_utc: Option<&str>,
        recorded_before_utc: Option<&str>,
        limit: i64,
    ) -> Result<Vec<AlphaHealthMetricRecord>, AlphaHealthServiceError> {
        if limit <= 0 {
            return Err(AlphaHealthServiceError::invalid_payload(
                "limit must be greater than 0",
                vec![AlphaHealthValidationIssue {
                    field: "limit".to_string(),
                    code: AlphaHealthReasonCode::InvalidPayload.code(),
                    message: "limit must be greater than 0".to_string(),
                }],
            ));
        }
        let normalized_alpha_id = normalize_research_identifier(alpha_id);
        let recorded_after = recorded_after_utc
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| parse_alpha_health_utc_timestamp(value).map_err(map_contract_error))
            .transpose()?;
        let recorded_before = recorded_before_utc
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| parse_alpha_health_utc_timestamp(value).map_err(map_contract_error))
            .transpose()?;

        let metrics = self.metrics.lock().map_err(|_| {
            AlphaHealthServiceError::persistence_unavailable(
                "in-memory alpha-health metric store lock poisoned",
            )
        })?;
        let mut values = metrics
            .values()
            .filter(|metric| metric.alpha_id == normalized_alpha_id)
            .cloned()
            .collect::<Vec<_>>();
        values.sort_by(|left, right| {
            right
                .recorded_at_utc
                .cmp(&left.recorded_at_utc)
                .then_with(|| left.metric_id.cmp(&right.metric_id))
        });

        values
            .into_iter()
            .filter(|metric| {
                let recorded_at = parse_alpha_health_utc_timestamp(&metric.recorded_at_utc);
                if let Ok(recorded_at) = recorded_at {
                    let after_ok = recorded_after
                        .map(|after| recorded_at >= after)
                        .unwrap_or(true);
                    let before_ok = recorded_before
                        .map(|before| recorded_at < before)
                        .unwrap_or(true);
                    after_ok && before_ok
                } else {
                    false
                }
            })
            .take(limit as usize)
            .map(|metric| {
                canonicalize_alpha_health_metric_record(&metric).map_err(map_contract_error)
            })
            .collect()
    }

    fn upsert_breach(
        &self,
        breach: AlphaThresholdBreachRecord,
    ) -> Result<(), AlphaHealthServiceError> {
        let canonical =
            canonicalize_alpha_threshold_breach_record(&breach).map_err(map_contract_error)?;
        let mut breaches = self.breaches.lock().map_err(|_| {
            AlphaHealthServiceError::persistence_unavailable(
                "in-memory alpha-health breach store lock poisoned",
            )
        })?;
        breaches.insert(canonical.breach_id.clone(), canonical);
        Ok(())
    }

    fn load_breach(
        &self,
        breach_id: &str,
    ) -> Result<Option<AlphaThresholdBreachRecord>, AlphaHealthServiceError> {
        let normalized_breach_id = normalize_research_identifier(breach_id);
        let breaches = self.breaches.lock().map_err(|_| {
            AlphaHealthServiceError::persistence_unavailable(
                "in-memory alpha-health breach store lock poisoned",
            )
        })?;
        breaches
            .get(&normalized_breach_id)
            .cloned()
            .map(|record| {
                canonicalize_alpha_threshold_breach_record(&record).map_err(map_contract_error)
            })
            .transpose()
    }

    fn list_breaches(
        &self,
        alpha_id: &str,
        breached_after_utc: Option<&str>,
        breached_before_utc: Option<&str>,
        limit: i64,
    ) -> Result<Vec<AlphaThresholdBreachRecord>, AlphaHealthServiceError> {
        if limit <= 0 {
            return Err(AlphaHealthServiceError::invalid_payload(
                "limit must be greater than 0",
                vec![AlphaHealthValidationIssue {
                    field: "limit".to_string(),
                    code: AlphaHealthReasonCode::InvalidPayload.code(),
                    message: "limit must be greater than 0".to_string(),
                }],
            ));
        }
        let normalized_alpha_id = normalize_research_identifier(alpha_id);
        let breached_after = breached_after_utc
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| parse_alpha_health_utc_timestamp(value).map_err(map_contract_error))
            .transpose()?;
        let breached_before = breached_before_utc
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| parse_alpha_health_utc_timestamp(value).map_err(map_contract_error))
            .transpose()?;

        let breaches = self.breaches.lock().map_err(|_| {
            AlphaHealthServiceError::persistence_unavailable(
                "in-memory alpha-health breach store lock poisoned",
            )
        })?;
        let mut values = breaches
            .values()
            .filter(|breach| breach.alpha_id == normalized_alpha_id)
            .cloned()
            .collect::<Vec<_>>();
        values.sort_by(|left, right| {
            right
                .breached_at_utc
                .cmp(&left.breached_at_utc)
                .then_with(|| left.breach_id.cmp(&right.breach_id))
        });

        values
            .into_iter()
            .filter(|breach| {
                let breached_at = parse_alpha_health_utc_timestamp(&breach.breached_at_utc);
                if let Ok(breached_at) = breached_at {
                    let after_ok = breached_after
                        .map(|after| breached_at >= after)
                        .unwrap_or(true);
                    let before_ok = breached_before
                        .map(|before| breached_at < before)
                        .unwrap_or(true);
                    after_ok && before_ok
                } else {
                    false
                }
            })
            .take(limit as usize)
            .map(|breach| {
                canonicalize_alpha_threshold_breach_record(&breach).map_err(map_contract_error)
            })
            .collect()
    }
}

#[derive(Debug, Clone, Default)]
pub struct StaticLiveMonitoringContextPort;

impl LiveMonitoringContextPort for StaticLiveMonitoringContextPort {
    fn ensure_live_monitoring_scope(&self, _alpha_id: &str) -> Result<(), AlphaHealthServiceError> {
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct InMemoryIncidentAlertPort {
    alerts: Mutex<Vec<IncidentAlert>>,
}

impl IncidentAlertPort for InMemoryIncidentAlertPort {
    fn maybe_emit_breach_alert(
        &self,
        breach: &AlphaThresholdBreachRecord,
    ) -> Result<bool, AlphaHealthServiceError> {
        let mut alerts = self.alerts.lock().map_err(|_| {
            AlphaHealthServiceError::persistence_unavailable(
                "in-memory alpha-health alert store lock poisoned",
            )
        })?;
        let dedupe_correlation_key = alpha_health_alert_correlation_key(breach);
        let should_emit = should_emit_alert(
            alerts.as_slice(),
            AlertReasonCode::AlphaHealthThresholdBreach.code(),
            &dedupe_correlation_key,
            &breach.breached_at_utc,
            ALERT_DEDUPE_WINDOW_SECONDS,
        )
        .map_err(map_alert_contract_error)?;
        if !should_emit {
            return Ok(false);
        }
        let alert = alpha_health_alert_for_breach(breach)?;
        alerts.push(alert);
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::research::{
        AlphaHealthAttributionWindowMetrics, AlphaHealthMetricKey, AlphaHealthMetricWindow,
        expected_alpha_health_comparator,
    };

    fn sample_windows() -> Vec<AlphaHealthAttributionWindowMetrics> {
        vec![
            AlphaHealthAttributionWindowMetrics {
                window: AlphaHealthMetricWindow::OneHour,
                net_pnl: 11.0,
                rolling_sharpe: 1.1,
                rolling_hit_rate: 0.58,
                rolling_drawdown: 0.03,
                stability_score: 0.8,
            },
            AlphaHealthAttributionWindowMetrics {
                window: AlphaHealthMetricWindow::TwentyFourHours,
                net_pnl: 74.0,
                rolling_sharpe: 1.08,
                rolling_hit_rate: 0.57,
                rolling_drawdown: 0.05,
                stability_score: 0.79,
            },
            AlphaHealthAttributionWindowMetrics {
                window: AlphaHealthMetricWindow::ThirtyDays,
                net_pnl: 302.0,
                rolling_sharpe: 1.05,
                rolling_hit_rate: 0.56,
                rolling_drawdown: 0.06,
                stability_score: 0.78,
            },
        ]
    }

    fn sample_start_input() -> StartAlphaHealthMetricInput {
        StartAlphaHealthMetricInput {
            actor_id: "ops-1".to_string(),
            actor_role: "operational_control".to_string(),
            alpha_id: "alpha::mean-reversion".to_string(),
            rolling_sharpe: 1.05,
            rolling_hit_rate: 0.56,
            rolling_drawdown: 0.06,
            stability_score: 0.78,
            windows: sample_windows(),
            thresholds: vec![
                AlphaHealthThresholdDefinition {
                    metric_key: AlphaHealthMetricKey::RollingSharpe,
                    comparator: expected_alpha_health_comparator(
                        AlphaHealthMetricKey::RollingSharpe,
                    ),
                    threshold_value: 1.0,
                },
                AlphaHealthThresholdDefinition {
                    metric_key: AlphaHealthMetricKey::RollingDrawdown,
                    comparator: expected_alpha_health_comparator(
                        AlphaHealthMetricKey::RollingDrawdown,
                    ),
                    threshold_value: 0.08,
                },
            ],
            correlation_id: "corr-alpha-health-001".to_string(),
            requested_at_utc: "2026-04-08T01:00:00Z".to_string(),
        }
    }

    #[derive(Debug)]
    struct FailingContextPort;

    impl LiveMonitoringContextPort for FailingContextPort {
        fn ensure_live_monitoring_scope(
            &self,
            _alpha_id: &str,
        ) -> Result<(), AlphaHealthServiceError> {
            Err(AlphaHealthServiceError::state_unavailable(
                "live-monitoring context unavailable",
            ))
        }
    }

    #[test]
    fn alpha_health_start_boundary_equal_to_threshold_is_allow_path() {
        let mut input = sample_start_input();
        input.rolling_sharpe = 1.0;
        input.rolling_drawdown = 0.08;
        let service = AlphaHealthService::default();
        let evidence = service
            .start_alpha_health_metric(input)
            .expect("boundary metric should succeed");
        assert!(evidence.breaches.is_empty());
        assert!(!evidence.alert_emitted);
        assert_eq!(
            evidence.reason_code,
            AlphaHealthReasonCode::ThresholdSatisfied.code()
        );
    }

    #[test]
    fn alpha_health_start_records_breaches_and_emits_alerts() {
        let mut input = sample_start_input();
        input.rolling_sharpe = 0.9;
        input.rolling_drawdown = 0.11;
        let alert_port = Arc::new(InMemoryIncidentAlertPort::default());
        let service = AlphaHealthService::default().with_incident_alert_port(alert_port.clone());
        let evidence = service
            .start_alpha_health_metric(input)
            .expect("breach metrics should persist");
        assert_eq!(evidence.breaches.len(), 2);
        assert!(evidence.alert_emitted);
        assert_eq!(
            evidence.reason_code,
            AlphaHealthReasonCode::ThresholdBreachDetected.code()
        );
        let alerts = alert_port
            .alerts
            .lock()
            .expect("alert store lock should not be poisoned");
        assert_eq!(alerts.len(), 2);
        assert!(
            alerts.iter().all(|alert| {
                alert.reason_code == AlertReasonCode::AlphaHealthThresholdBreach.code()
            }),
            "each breach should emit an alpha-health threshold-breach alert"
        );
    }

    #[test]
    fn alpha_health_start_fails_closed_when_live_context_is_unavailable() {
        let service = AlphaHealthService::default()
            .with_live_monitoring_context_port(Arc::new(FailingContextPort));
        let error = service
            .start_alpha_health_metric(sample_start_input())
            .expect_err("missing context should fail closed");
        assert_eq!(error.code, AlphaHealthReasonCode::StateUnavailable.code());
    }

    #[test]
    fn alpha_health_list_orders_metrics_deterministically() {
        let service = AlphaHealthService::default();

        let mut first = sample_start_input();
        first.requested_at_utc = "2026-04-08T01:00:00Z".to_string();
        service
            .start_alpha_health_metric(first)
            .expect("first metric should persist");

        let mut second = sample_start_input();
        second.requested_at_utc = "2026-04-08T01:05:00Z".to_string();
        second.correlation_id = "corr-alpha-health-002".to_string();
        service
            .start_alpha_health_metric(second)
            .expect("second metric should persist");

        let listed = service
            .list_alpha_health_metrics(ListAlphaHealthMetricsInput {
                actor_id: "analyst-1".to_string(),
                actor_role: "read_only_analytics".to_string(),
                alpha_id: "alpha::mean-reversion".to_string(),
                limit: Some(10),
                recorded_after_utc: None,
                recorded_before_utc: None,
                correlation_id: "corr-alpha-health-list-001".to_string(),
                queried_at_utc: "2026-04-08T01:06:00Z".to_string(),
            })
            .expect("list should succeed");

        assert_eq!(listed.len(), 2);
        assert!(listed[0].recorded_at_utc >= listed[1].recorded_at_utc);
    }

    #[test]
    fn alpha_health_list_breaches_applies_time_windows_deterministically() {
        let service = AlphaHealthService::default();
        let mut old = sample_start_input();
        old.requested_at_utc = "2026-04-08T00:00:00Z".to_string();
        old.rolling_sharpe = 0.9;
        old.rolling_drawdown = 0.09;
        service
            .start_alpha_health_metric(old)
            .expect("older breach metric should persist");

        let mut recent = sample_start_input();
        recent.requested_at_utc = "2026-04-08T02:00:00Z".to_string();
        recent.correlation_id = "corr-alpha-health-003".to_string();
        recent.rolling_sharpe = 0.88;
        recent.rolling_drawdown = 0.1;
        service
            .start_alpha_health_metric(recent)
            .expect("recent breach metric should persist");

        let breaches = service
            .list_alpha_threshold_breaches(ListAlphaThresholdBreachesInput {
                actor_id: "analyst-1".to_string(),
                actor_role: "read_only_analytics".to_string(),
                alpha_id: "alpha::mean-reversion".to_string(),
                limit: Some(10),
                breached_after_utc: Some("2026-04-08T01:00:00Z".to_string()),
                breached_before_utc: None,
                correlation_id: "corr-alpha-health-breach-list-001".to_string(),
                queried_at_utc: "2026-04-08T02:10:00Z".to_string(),
            })
            .expect("breach list should succeed");

        assert_eq!(breaches.len(), 2);
        assert!(
            breaches
                .iter()
                .all(|breach| breach.breached_at_utc.as_str() >= "2026-04-08T01:00:00Z")
        );
    }
}
