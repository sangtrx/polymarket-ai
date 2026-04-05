use domain::risk::{
    FreshnessGateContractError, FreshnessGateEvaluationOutcome, FreshnessGateEvent,
    FreshnessGatePolicy, FreshnessGateReasonCode, FreshnessGateState, FreshnessSnapshot,
    evaluate_freshness_gate, validate_freshness_gate_event,
};
use persistence::postgres::freshness_gate::{
    FreshnessGatePersistenceError, insert_freshness_gate_event, load_latest_freshness_gate_event,
};
use serde::Serialize;
use sqlx::PgPool;
use std::error::Error;
use std::fmt::{Display, Formatter};
#[cfg(test)]
use std::sync::Mutex;
use std::sync::{Arc, RwLock};
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};
use tokio::time::{Duration, MissedTickBehavior};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FreshnessGateRuntimeError {
    pub code: &'static str,
    pub message: String,
}

impl FreshnessGateRuntimeError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl Display for FreshnessGateRuntimeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for FreshnessGateRuntimeError {}

pub trait FreshnessGateStore: Send + Sync {
    async fn persist_event(
        &self,
        event: &FreshnessGateEvent,
    ) -> Result<(), FreshnessGateRuntimeError>;
    async fn load_latest_event(
        &self,
    ) -> Result<Option<FreshnessGateEvent>, FreshnessGateRuntimeError>;
}

#[derive(Clone)]
pub struct PostgresFreshnessGateStore {
    pool: PgPool,
}

impl PostgresFreshnessGateStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl FreshnessGateStore for PostgresFreshnessGateStore {
    async fn persist_event(
        &self,
        event: &FreshnessGateEvent,
    ) -> Result<(), FreshnessGateRuntimeError> {
        insert_freshness_gate_event(&self.pool, event)
            .await
            .map_err(map_persistence_error)
    }

    async fn load_latest_event(
        &self,
    ) -> Result<Option<FreshnessGateEvent>, FreshnessGateRuntimeError> {
        load_latest_freshness_gate_event(&self.pool)
            .await
            .map_err(map_persistence_error)
    }
}

fn map_persistence_error(error: FreshnessGatePersistenceError) -> FreshnessGateRuntimeError {
    FreshnessGateRuntimeError::new(error.code, error.to_string())
}

#[derive(Debug, Clone)]
pub struct FreshnessGateRuntimeConfig {
    pub evaluation_interval_seconds: f64,
    pub policy: FreshnessGatePolicy,
}

impl Default for FreshnessGateRuntimeConfig {
    fn default() -> Self {
        Self {
            evaluation_interval_seconds: 1.0,
            policy: FreshnessGatePolicy::default(),
        }
    }
}

impl FreshnessGateRuntimeConfig {
    pub fn from_env() -> Result<Self, FreshnessGateRuntimeError> {
        let mut config = Self::default();
        if let Ok(value) = std::env::var("EXECUTION_FRESHNESS_EVALUATION_INTERVAL_SECONDS") {
            config.evaluation_interval_seconds =
                parse_positive_f64("EXECUTION_FRESHNESS_EVALUATION_INTERVAL_SECONDS", &value)?;
        }
        if let Ok(value) = std::env::var("EXECUTION_FRESHNESS_STALE_THRESHOLD_SECONDS") {
            config.policy.stale_threshold_seconds =
                parse_positive_f64("EXECUTION_FRESHNESS_STALE_THRESHOLD_SECONDS", &value)?;
        }
        if let Ok(value) = std::env::var("EXECUTION_FRESHNESS_STABILITY_WINDOW_SECONDS") {
            config.policy.stability_window_seconds =
                parse_positive_f64("EXECUTION_FRESHNESS_STABILITY_WINDOW_SECONDS", &value)?;
        }
        if let Ok(value) = std::env::var("EXECUTION_FRESHNESS_MAX_BREACH_TO_PAUSE_SECONDS") {
            config.policy.max_breach_to_pause_seconds =
                parse_positive_f64("EXECUTION_FRESHNESS_MAX_BREACH_TO_PAUSE_SECONDS", &value)?;
        }

        domain::risk::validate_freshness_gate_policy(&config.policy).map_err(map_contract_error)?;
        Ok(config)
    }
}

#[derive(Debug, Clone, Default)]
pub struct SharedFreshnessSignals {
    state: Arc<RwLock<FreshnessSignalState>>,
}

#[derive(Debug, Clone, Default)]
struct FreshnessSignalState {
    market_last_observed_at_utc: Option<String>,
    user_last_observed_at_utc: Option<String>,
}

impl SharedFreshnessSignals {
    pub fn record_market_update(
        &self,
        observed_at_utc: &str,
    ) -> Result<(), FreshnessGateRuntimeError> {
        parse_rfc3339_utc(observed_at_utc)?;
        self.state
            .write()
            .map_err(|_| {
                FreshnessGateRuntimeError::new(
                    FreshnessGateReasonCode::PersistenceUnavailable.code(),
                    "shared freshness signal state is poisoned",
                )
            })?
            .market_last_observed_at_utc = Some(observed_at_utc.to_string());
        Ok(())
    }

    pub fn record_user_update(
        &self,
        observed_at_utc: &str,
    ) -> Result<(), FreshnessGateRuntimeError> {
        parse_rfc3339_utc(observed_at_utc)?;
        self.state
            .write()
            .map_err(|_| {
                FreshnessGateRuntimeError::new(
                    FreshnessGateReasonCode::PersistenceUnavailable.code(),
                    "shared freshness signal state is poisoned",
                )
            })?
            .user_last_observed_at_utc = Some(observed_at_utc.to_string());
        Ok(())
    }

    pub fn snapshot(
        &self,
        sampled_at_utc: &str,
        correlation_id: &str,
    ) -> Result<FreshnessSnapshot, FreshnessGateRuntimeError> {
        let sampled_at = parse_rfc3339_utc(sampled_at_utc)?;
        let state = self.state.read().map_err(|_| {
            FreshnessGateRuntimeError::new(
                FreshnessGateReasonCode::PersistenceUnavailable.code(),
                "shared freshness signal state is poisoned",
            )
        })?;
        let market_data_age_seconds = state
            .market_last_observed_at_utc
            .as_deref()
            .map(|value| age_seconds(sampled_at, value))
            .transpose()?;
        let user_data_age_seconds = state
            .user_last_observed_at_utc
            .as_deref()
            .map(|value| age_seconds(sampled_at, value))
            .transpose()?;

        Ok(FreshnessSnapshot {
            market_data_age_seconds,
            user_data_age_seconds,
            sampled_at_utc: sampled_at_utc.to_string(),
            correlation_id: correlation_id.to_string(),
        })
    }
}

pub struct FreshnessGateController<S: FreshnessGateStore> {
    store: S,
    signals: SharedFreshnessSignals,
    config: FreshnessGateRuntimeConfig,
    state: Option<FreshnessGateState>,
}

impl<S: FreshnessGateStore> FreshnessGateController<S> {
    pub fn new(
        store: S,
        signals: SharedFreshnessSignals,
        config: FreshnessGateRuntimeConfig,
    ) -> Self {
        Self {
            store,
            signals,
            config,
            state: None,
        }
    }

    pub async fn hydrate_latest_state(&mut self) -> Result<(), FreshnessGateRuntimeError> {
        if let Some(event) = self.store.load_latest_event().await? {
            self.state = Some(FreshnessGateState {
                pause_active: event.pause_active,
                reason_code: event.reason_code,
                stale_breach_detected_at_utc: event.stale_breach_detected_at_utc,
                pause_activated_at_utc: event.pause_activated_at_utc,
                recovery_window_started_at_utc: event.recovery_window_started_at_utc,
                last_evaluated_at_utc: event.evaluated_at_utc,
            });
        }
        Ok(())
    }

    pub async fn evaluate_once(
        &mut self,
        sampled_at_utc: &str,
        correlation_id: &str,
    ) -> Result<FreshnessGateEvaluationOutcome, FreshnessGateRuntimeError> {
        let snapshot = self.signals.snapshot(sampled_at_utc, correlation_id)?;
        let outcome = evaluate_freshness_gate(&snapshot, self.state.as_ref(), &self.config.policy)
            .map_err(map_contract_error)?;

        if let Some(event) = outcome.to_event(freshness_event_id(&outcome), &self.config.policy) {
            validate_freshness_gate_event(&event).map_err(map_contract_error)?;
            self.store.persist_event(&event).await?;
        }

        self.state = Some(outcome.to_state());
        emit_freshness_gate_telemetry(FreshnessGateTelemetryEvent {
            event_name: "execution_freshness_gate_evaluation_v1",
            outcome: if outcome.pause_active {
                "deny"
            } else {
                "allow"
            },
            transition: outcome
                .transition
                .map(|transition| transition.as_str())
                .unwrap_or("no_transition"),
            reason_code: &outcome.reason_code,
            correlation_id: &outcome.correlation_id,
            timestamp_utc: &outcome.evaluated_at_utc,
            pause_active: outcome.pause_active,
            block_new_order_creation: outcome.block_new_order_creation,
            market_data_age_seconds: outcome.market_data_age_seconds,
            user_data_age_seconds: outcome.user_data_age_seconds,
            breach_to_pause_latency_seconds: outcome.breach_to_pause_latency_seconds,
            stale_breach_detected_at_utc: outcome.stale_breach_detected_at_utc.as_deref(),
            pause_activated_at_utc: outcome.pause_activated_at_utc.as_deref(),
            recovery_window_started_at_utc: outcome.recovery_window_started_at_utc.as_deref(),
            recovery_confirmed_at_utc: outcome.recovery_confirmed_at_utc.as_deref(),
        });

        Ok(outcome)
    }
}

pub async fn run_freshness_gate_loop<S: FreshnessGateStore>(
    controller: &mut FreshnessGateController<S>,
) -> Result<(), FreshnessGateRuntimeError> {
    controller.hydrate_latest_state().await?;
    let mut interval = tokio::time::interval(Duration::from_secs_f64(
        controller.config.evaluation_interval_seconds,
    ));
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        let sampled_at_utc = now_utc_rfc3339()?;
        let correlation_id = format!(
            "freshness-gate-eval::{}",
            compact_timestamp_token(&sampled_at_utc)
        );
        controller
            .evaluate_once(&sampled_at_utc, &correlation_id)
            .await?;
    }
}

fn parse_positive_f64(key: &str, value: &str) -> Result<f64, FreshnessGateRuntimeError> {
    let parsed = value.parse::<f64>().map_err(|_| {
        FreshnessGateRuntimeError::new(
            FreshnessGateReasonCode::InvalidPayload.code(),
            format!("{key} must be a floating-point number"),
        )
    })?;
    if !parsed.is_finite() || parsed <= 0.0 {
        return Err(FreshnessGateRuntimeError::new(
            FreshnessGateReasonCode::InvalidPayload.code(),
            format!("{key} must be finite and greater than 0"),
        ));
    }
    Ok(parsed)
}

fn parse_rfc3339_utc(value: &str) -> Result<OffsetDateTime, FreshnessGateRuntimeError> {
    let parsed = OffsetDateTime::parse(value, &Rfc3339).map_err(|error| {
        FreshnessGateRuntimeError::new(
            FreshnessGateReasonCode::InvalidPayload.code(),
            format!("invalid freshness timestamp `{value}`: {error}"),
        )
    })?;
    if parsed.offset() != UtcOffset::UTC {
        return Err(FreshnessGateRuntimeError::new(
            FreshnessGateReasonCode::InvalidPayload.code(),
            "freshness timestamps must use UTC `Z` offset",
        ));
    }
    Ok(parsed)
}

fn age_seconds(
    sampled_at: OffsetDateTime,
    last_observed_at_utc: &str,
) -> Result<f64, FreshnessGateRuntimeError> {
    let last_observed_at = parse_rfc3339_utc(last_observed_at_utc)?;
    if sampled_at < last_observed_at {
        return Err(FreshnessGateRuntimeError::new(
            FreshnessGateReasonCode::InvalidPayload.code(),
            "sampled freshness timestamp cannot be earlier than latest observed timestamp",
        ));
    }
    Ok((sampled_at - last_observed_at).as_seconds_f64())
}

fn map_contract_error(error: FreshnessGateContractError) -> FreshnessGateRuntimeError {
    FreshnessGateRuntimeError::new(error.code, error.message)
}

fn freshness_event_id(outcome: &FreshnessGateEvaluationOutcome) -> String {
    let transition = outcome
        .transition
        .map(|value| value.as_str())
        .unwrap_or("no_transition");
    format!(
        "freshness::{}::{}::{}",
        transition,
        compact_timestamp_token(&outcome.evaluated_at_utc),
        outcome.correlation_id.replace(':', "_")
    )
}

fn now_utc_rfc3339() -> Result<String, FreshnessGateRuntimeError> {
    OffsetDateTime::now_utc().format(&Rfc3339).map_err(|error| {
        FreshnessGateRuntimeError::new(
            FreshnessGateReasonCode::EvaluationError.code(),
            format!("unable to format UTC freshness timestamp: {error}"),
        )
    })
}

fn compact_timestamp_token(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_digit())
        .collect()
}

fn emit_freshness_gate_telemetry(event: FreshnessGateTelemetryEvent<'_>) {
    println!(
        "{}",
        serde_json::to_string(&event).expect("freshness gate telemetry should serialize")
    );
}

#[derive(Debug, Serialize)]
struct FreshnessGateTelemetryEvent<'a> {
    event_name: &'a str,
    outcome: &'a str,
    transition: &'a str,
    reason_code: &'a str,
    correlation_id: &'a str,
    timestamp_utc: &'a str,
    pause_active: bool,
    block_new_order_creation: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    market_data_age_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_data_age_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    breach_to_pause_latency_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stale_breach_detected_at_utc: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pause_activated_at_utc: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    recovery_window_started_at_utc: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    recovery_confirmed_at_utc: Option<&'a str>,
}

#[cfg(test)]
#[derive(Debug, Clone, Default)]
pub struct InMemoryFreshnessGateStore {
    events: Arc<Mutex<Vec<FreshnessGateEvent>>>,
}

#[cfg(test)]
impl InMemoryFreshnessGateStore {
    pub fn events(&self) -> Vec<FreshnessGateEvent> {
        self.events
            .lock()
            .expect("freshness event list should not be poisoned")
            .clone()
    }
}

#[cfg(test)]
impl FreshnessGateStore for InMemoryFreshnessGateStore {
    async fn persist_event(
        &self,
        event: &FreshnessGateEvent,
    ) -> Result<(), FreshnessGateRuntimeError> {
        validate_freshness_gate_event(event).map_err(map_contract_error)?;
        self.events
            .lock()
            .map_err(|_| {
                FreshnessGateRuntimeError::new(
                    FreshnessGateReasonCode::PersistenceUnavailable.code(),
                    "freshness event list should not be poisoned",
                )
            })?
            .push(event.clone());
        Ok(())
    }

    async fn load_latest_event(
        &self,
    ) -> Result<Option<FreshnessGateEvent>, FreshnessGateRuntimeError> {
        Ok(self
            .events
            .lock()
            .map_err(|_| {
                FreshnessGateRuntimeError::new(
                    FreshnessGateReasonCode::PersistenceUnavailable.code(),
                    "freshness event list should not be poisoned",
                )
            })?
            .iter()
            .max_by(|a, b| {
                a.evaluated_at_utc
                    .cmp(&b.evaluated_at_utc)
                    .then_with(|| a.event_id.cmp(&b.event_id))
            })
            .cloned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct FailingPersistStore;

    impl FreshnessGateStore for FailingPersistStore {
        async fn persist_event(
            &self,
            _event: &FreshnessGateEvent,
        ) -> Result<(), FreshnessGateRuntimeError> {
            Err(FreshnessGateRuntimeError::new(
                FreshnessGateReasonCode::PersistenceUnavailable.code(),
                "forced freshness gate persistence failure",
            ))
        }

        async fn load_latest_event(
            &self,
        ) -> Result<Option<FreshnessGateEvent>, FreshnessGateRuntimeError> {
            Ok(None)
        }
    }

    #[derive(Clone)]
    struct InvalidHydratedStateStore {
        latest_event: FreshnessGateEvent,
    }

    impl FreshnessGateStore for InvalidHydratedStateStore {
        async fn persist_event(
            &self,
            _event: &FreshnessGateEvent,
        ) -> Result<(), FreshnessGateRuntimeError> {
            Ok(())
        }

        async fn load_latest_event(
            &self,
        ) -> Result<Option<FreshnessGateEvent>, FreshnessGateRuntimeError> {
            Ok(Some(self.latest_event.clone()))
        }
    }

    fn sample_config() -> FreshnessGateRuntimeConfig {
        FreshnessGateRuntimeConfig {
            evaluation_interval_seconds: 1.0,
            policy: FreshnessGatePolicy {
                stale_threshold_seconds: 30.0,
                stability_window_seconds: 10.0,
                max_breach_to_pause_seconds: 5.0,
            },
        }
    }

    fn sample_pause_event() -> FreshnessGateEvent {
        FreshnessGateEvent {
            event_id: "freshness::pause_activated::20260406000100".to_string(),
            transition: domain::risk::FreshnessGateTransition::PauseActivated,
            reason_code: FreshnessGateReasonCode::StaleBreach.code().to_string(),
            pause_active: true,
            block_new_order_creation: true,
            market_data_age_seconds: Some(31.0),
            user_data_age_seconds: Some(31.0),
            stale_threshold_seconds: 30.0,
            stability_window_seconds: 10.0,
            max_breach_to_pause_seconds: 5.0,
            stale_breach_detected_at_utc: Some("2026-04-06T00:01:00Z".to_string()),
            pause_activated_at_utc: Some("2026-04-06T00:01:00Z".to_string()),
            recovery_window_started_at_utc: None,
            recovery_confirmed_at_utc: None,
            breach_to_pause_latency_seconds: Some(0.0),
            evaluated_at_utc: "2026-04-06T00:01:00Z".to_string(),
            correlation_id: "corr-freshness-hydrated".to_string(),
        }
    }

    #[tokio::test]
    async fn boundary_safe_inputs_keep_pause_inactive() {
        let store = InMemoryFreshnessGateStore::default();
        let signals = SharedFreshnessSignals::default();
        let mut controller = FreshnessGateController::new(store, signals.clone(), sample_config());

        signals
            .record_market_update("2026-04-06T00:00:01Z")
            .expect("market signal should be recorded");
        signals
            .record_user_update("2026-04-06T00:00:00Z")
            .expect("user signal should be recorded");

        let outcome = controller
            .evaluate_once("2026-04-06T00:00:30Z", "corr-freshness-boundary")
            .await
            .expect("boundary-safe evaluation should pass");

        assert!(!outcome.pause_active);
        assert!(!outcome.block_new_order_creation);
        assert_eq!(
            outcome.reason_code,
            FreshnessGateReasonCode::BoundarySafe.code()
        );
        assert_eq!(outcome.transition, None);
    }

    #[tokio::test]
    async fn stale_breach_activates_pause_and_persists_event_with_nfr5_latency() {
        let store = InMemoryFreshnessGateStore::default();
        let signals = SharedFreshnessSignals::default();
        let mut controller =
            FreshnessGateController::new(store.clone(), signals.clone(), sample_config());

        signals
            .record_market_update("2026-04-06T00:00:00Z")
            .expect("market signal should be recorded");
        signals
            .record_user_update("2026-04-06T00:00:00Z")
            .expect("user signal should be recorded");

        let outcome = controller
            .evaluate_once("2026-04-06T00:00:31Z", "corr-freshness-stale")
            .await
            .expect("stale breach should evaluate");

        assert!(outcome.pause_active);
        assert_eq!(
            outcome.transition,
            Some(domain::risk::FreshnessGateTransition::PauseActivated)
        );
        assert!(
            outcome
                .breach_to_pause_latency_seconds
                .is_some_and(|latency| latency <= 5.0)
        );

        let events = store.events();
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].transition,
            domain::risk::FreshnessGateTransition::PauseActivated
        );
    }

    #[tokio::test]
    async fn evaluate_once_rejects_non_utc_sample_timestamp() {
        let store = InMemoryFreshnessGateStore::default();
        let signals = SharedFreshnessSignals::default();
        let mut controller = FreshnessGateController::new(store, signals.clone(), sample_config());

        signals
            .record_market_update("2026-04-06T00:00:00Z")
            .expect("market signal should be recorded");
        signals
            .record_user_update("2026-04-06T00:00:00Z")
            .expect("user signal should be recorded");

        let error = controller
            .evaluate_once("2026-04-06T00:00:31+01:00", "corr-freshness-non-utc")
            .await
            .expect_err("non-UTC sample timestamp should fail closed");
        assert_eq!(error.code, FreshnessGateReasonCode::InvalidPayload.code());
        assert!(error.message.contains("freshness timestamps must use UTC `Z` offset"));
    }

    #[tokio::test]
    async fn evaluate_once_surfaces_persistence_failure_with_machine_readable_code() {
        let store = FailingPersistStore;
        let signals = SharedFreshnessSignals::default();
        let mut controller = FreshnessGateController::new(store, signals.clone(), sample_config());

        signals
            .record_market_update("2026-04-06T00:00:00Z")
            .expect("market signal should be recorded");
        signals
            .record_user_update("2026-04-06T00:00:00Z")
            .expect("user signal should be recorded");

        let error = controller
            .evaluate_once("2026-04-06T00:00:31Z", "corr-freshness-persist-failure")
            .await
            .expect_err("persistence failure should fail closed");
        assert_eq!(
            error.code,
            FreshnessGateReasonCode::PersistenceUnavailable.code()
        );
        assert!(error.message.contains("forced freshness gate persistence failure"));
    }

    #[tokio::test]
    async fn invalid_hydrated_state_returns_machine_readable_evaluation_error() {
        let mut invalid_event = sample_pause_event();
        invalid_event.evaluated_at_utc = "2026-04-06T00:01:00+01:00".to_string();
        let store = InvalidHydratedStateStore {
            latest_event: invalid_event,
        };
        let signals = SharedFreshnessSignals::default();
        let mut controller = FreshnessGateController::new(store, signals.clone(), sample_config());
        controller
            .hydrate_latest_state()
            .await
            .expect("hydration should load latest state");

        signals
            .record_market_update("2026-04-06T00:01:00Z")
            .expect("market signal should be recorded");
        signals
            .record_user_update("2026-04-06T00:01:00Z")
            .expect("user signal should be recorded");

        let error = controller
            .evaluate_once("2026-04-06T00:01:05Z", "corr-freshness-invalid-state")
            .await
            .expect_err("invalid hydrated state should fail closed");
        assert_eq!(error.code, FreshnessGateReasonCode::InvalidPayload.code());
        assert!(error.message.contains("freshness gate state is invalid"));
    }

    #[tokio::test]
    async fn missing_signal_input_is_fail_closed_and_state_unavailable() {
        let store = InMemoryFreshnessGateStore::default();
        let signals = SharedFreshnessSignals::default();
        let mut controller =
            FreshnessGateController::new(store.clone(), signals.clone(), sample_config());

        signals
            .record_market_update("2026-04-06T00:00:00Z")
            .expect("market signal should be recorded");

        let outcome = controller
            .evaluate_once("2026-04-06T00:00:10Z", "corr-freshness-missing")
            .await
            .expect("missing user signal should still evaluate fail-closed");

        assert!(outcome.pause_active);
        assert_eq!(
            outcome.reason_code,
            FreshnessGateReasonCode::StateUnavailable.code()
        );

        let events = store.events();
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].reason_code,
            FreshnessGateReasonCode::StateUnavailable.code()
        );
    }

    #[tokio::test]
    async fn recovery_requires_full_stability_window_before_unpause() {
        let store = InMemoryFreshnessGateStore::default();
        let signals = SharedFreshnessSignals::default();
        let mut controller =
            FreshnessGateController::new(store.clone(), signals.clone(), sample_config());

        signals
            .record_market_update("2026-04-06T00:00:00Z")
            .expect("market signal should be recorded");
        signals
            .record_user_update("2026-04-06T00:00:00Z")
            .expect("user signal should be recorded");

        controller
            .evaluate_once("2026-04-06T00:00:31Z", "corr-freshness-start")
            .await
            .expect("stale breach should activate pause");

        signals
            .record_market_update("2026-04-06T00:00:40Z")
            .expect("market signal should recover");
        signals
            .record_user_update("2026-04-06T00:00:40Z")
            .expect("user signal should recover");

        let pending = controller
            .evaluate_once("2026-04-06T00:00:40Z", "corr-freshness-pending-start")
            .await
            .expect("recovery pending should evaluate");
        assert_eq!(
            pending.transition,
            Some(domain::risk::FreshnessGateTransition::RecoveryPending)
        );
        assert!(pending.pause_active);

        let window_minus_one = controller
            .evaluate_once(
                "2026-04-06T00:00:49Z",
                "corr-freshness-pending-window-minus-1",
            )
            .await
            .expect("window-1 should remain pending");
        assert_eq!(
            window_minus_one.transition,
            Some(domain::risk::FreshnessGateTransition::RecoveryPending)
        );
        assert!(window_minus_one.pause_active);

        let confirmed = controller
            .evaluate_once("2026-04-06T00:00:50Z", "corr-freshness-confirmed")
            .await
            .expect("full stability window should confirm recovery");
        assert_eq!(
            confirmed.transition,
            Some(domain::risk::FreshnessGateTransition::RecoveryConfirmed)
        );
        assert!(!confirmed.pause_active);
        assert!(!confirmed.block_new_order_creation);

        let events = store.events();
        assert_eq!(events.len(), 4);
    }
}
