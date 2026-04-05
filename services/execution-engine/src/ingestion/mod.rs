pub mod freshness_gate;
pub mod user_stream;

use domain::risk::{
    MARKET_STREAM_BACKLOG_DEGRADED_SECONDS, MarketDepthLevel, MarketStatus, MarketStreamHealth,
    MarketStreamHealthStatus, MarketStreamReasonCode, MarketStreamTick, QuarantinedMarketEvent,
    assess_market_stream_health, ingestion_latency_slo_met, validate_market_stream_tick,
};
use futures::StreamExt;
use persistence::postgres::market_stream::{
    insert_accepted_market_tick, insert_market_stream_health, insert_quarantined_market_event,
};
use polymarket_client_sdk::clob::ws::types::response::OrderBookLevel;
use polymarket_client_sdk::clob::ws::{BookUpdate, Client as PolymarketWsClient};
use polymarket_client_sdk::types::U256;
use serde::Serialize;
use serde_json::json;
use sqlx::PgPool;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::str::FromStr;
#[cfg(test)]
use std::sync::{Arc, Mutex};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tokio::time::{Duration, sleep};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketIngestionError {
    pub code: &'static str,
    pub message: String,
}

impl MarketIngestionError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl Display for MarketIngestionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for MarketIngestionError {}

pub trait MarketStreamStore: Send + Sync {
    async fn persist_accepted_tick(
        &self,
        tick: &MarketStreamTick,
    ) -> Result<(), MarketIngestionError>;
    async fn persist_quarantined_event(
        &self,
        event: &QuarantinedMarketEvent,
    ) -> Result<(), MarketIngestionError>;
    async fn persist_health(&self, health: &MarketStreamHealth)
    -> Result<(), MarketIngestionError>;
}

#[derive(Clone)]
pub struct PostgresMarketStreamStore {
    pool: PgPool,
}

impl PostgresMarketStreamStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl MarketStreamStore for PostgresMarketStreamStore {
    async fn persist_accepted_tick(
        &self,
        tick: &MarketStreamTick,
    ) -> Result<(), MarketIngestionError> {
        insert_accepted_market_tick(&self.pool, tick)
            .await
            .map_err(|error| {
                MarketIngestionError::new(
                    MarketStreamReasonCode::PersistenceUnavailable.code(),
                    error.to_string(),
                )
            })
    }

    async fn persist_quarantined_event(
        &self,
        event: &QuarantinedMarketEvent,
    ) -> Result<(), MarketIngestionError> {
        insert_quarantined_market_event(&self.pool, event)
            .await
            .map_err(|error| {
                MarketIngestionError::new(
                    MarketStreamReasonCode::PersistenceUnavailable.code(),
                    error.to_string(),
                )
            })
    }

    async fn persist_health(
        &self,
        health: &MarketStreamHealth,
    ) -> Result<(), MarketIngestionError> {
        insert_market_stream_health(&self.pool, health)
            .await
            .map_err(|error| {
                MarketIngestionError::new(
                    MarketStreamReasonCode::PersistenceUnavailable.code(),
                    error.to_string(),
                )
            })
    }
}

#[derive(Debug, Clone, Default)]
#[cfg(test)]
pub struct InMemoryMarketStreamStore {
    accepted_ticks: Arc<Mutex<Vec<MarketStreamTick>>>,
    quarantined_events: Arc<Mutex<Vec<QuarantinedMarketEvent>>>,
    health_events: Arc<Mutex<Vec<MarketStreamHealth>>>,
}

#[cfg(test)]
impl InMemoryMarketStreamStore {
    pub fn accepted_ticks(&self) -> Vec<MarketStreamTick> {
        self.accepted_ticks
            .lock()
            .expect("accepted tick list should not be poisoned")
            .clone()
    }

    pub fn quarantined_events(&self) -> Vec<QuarantinedMarketEvent> {
        self.quarantined_events
            .lock()
            .expect("quarantine event list should not be poisoned")
            .clone()
    }

    pub fn health_events(&self) -> Vec<MarketStreamHealth> {
        self.health_events
            .lock()
            .expect("health event list should not be poisoned")
            .clone()
    }
}

#[cfg(test)]
impl MarketStreamStore for InMemoryMarketStreamStore {
    async fn persist_accepted_tick(
        &self,
        tick: &MarketStreamTick,
    ) -> Result<(), MarketIngestionError> {
        self.accepted_ticks
            .lock()
            .map_err(|_| {
                MarketIngestionError::new(
                    MarketStreamReasonCode::PersistenceUnavailable.code(),
                    "accepted tick list poisoned",
                )
            })?
            .push(tick.clone());
        Ok(())
    }

    async fn persist_quarantined_event(
        &self,
        event: &QuarantinedMarketEvent,
    ) -> Result<(), MarketIngestionError> {
        self.quarantined_events
            .lock()
            .map_err(|_| {
                MarketIngestionError::new(
                    MarketStreamReasonCode::PersistenceUnavailable.code(),
                    "quarantine event list poisoned",
                )
            })?
            .push(event.clone());
        Ok(())
    }

    async fn persist_health(
        &self,
        health: &MarketStreamHealth,
    ) -> Result<(), MarketIngestionError> {
        self.health_events
            .lock()
            .map_err(|_| {
                MarketIngestionError::new(
                    MarketStreamReasonCode::PersistenceUnavailable.code(),
                    "health event list poisoned",
                )
            })?
            .push(health.clone());
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct MarketStreamRuntimeConfig {
    pub stream_name: String,
    pub stream_endpoint: String,
    pub cluster_id: String,
    pub asset_ids: Vec<String>,
    pub default_tick_size: f64,
    pub heartbeat_timeout_seconds: f64,
    pub reconnect_backoff_seconds: f64,
    pub max_reconnect_attempts: u32,
}

impl Default for MarketStreamRuntimeConfig {
    fn default() -> Self {
        Self {
            stream_name: "polymarket_market_stream".to_string(),
            stream_endpoint: "wss://ws-subscriptions-clob.polymarket.com".to_string(),
            cluster_id: "cluster_default".to_string(),
            asset_ids: Vec::new(),
            default_tick_size: 0.01,
            heartbeat_timeout_seconds: 15.0,
            reconnect_backoff_seconds: 1.0,
            max_reconnect_attempts: 5,
        }
    }
}

impl MarketStreamRuntimeConfig {
    pub fn from_env() -> Result<Self, MarketIngestionError> {
        let mut config = Self::default();
        if let Ok(value) = std::env::var("EXECUTION_MARKET_STREAM_NAME") {
            config.stream_name = value.trim().to_string();
        }
        if let Ok(value) = std::env::var("EXECUTION_MARKET_STREAM_ENDPOINT") {
            config.stream_endpoint = value.trim().to_string();
        }
        if let Ok(value) = std::env::var("EXECUTION_MARKET_STREAM_CLUSTER_ID") {
            config.cluster_id = value.trim().to_ascii_lowercase();
        }
        if let Ok(value) = std::env::var("EXECUTION_MARKET_STREAM_DEFAULT_TICK_SIZE") {
            config.default_tick_size =
                parse_positive_f64("EXECUTION_MARKET_STREAM_DEFAULT_TICK_SIZE", &value)?;
        }
        if let Ok(value) = std::env::var("EXECUTION_MARKET_STREAM_HEARTBEAT_TIMEOUT_SECONDS") {
            config.heartbeat_timeout_seconds =
                parse_positive_f64("EXECUTION_MARKET_STREAM_HEARTBEAT_TIMEOUT_SECONDS", &value)?;
        }
        if let Ok(value) = std::env::var("EXECUTION_MARKET_STREAM_RECONNECT_BACKOFF_SECONDS") {
            config.reconnect_backoff_seconds =
                parse_positive_f64("EXECUTION_MARKET_STREAM_RECONNECT_BACKOFF_SECONDS", &value)?;
        }
        if let Ok(value) = std::env::var("EXECUTION_MARKET_STREAM_MAX_RECONNECT_ATTEMPTS") {
            config.max_reconnect_attempts =
                parse_positive_u32("EXECUTION_MARKET_STREAM_MAX_RECONNECT_ATTEMPTS", &value)?;
        }

        let asset_ids = std::env::var("EXECUTION_MARKET_STREAM_ASSET_IDS").map_err(|_| {
            MarketIngestionError::new(
                MarketStreamReasonCode::InvalidPayload.code(),
                "EXECUTION_MARKET_STREAM_ASSET_IDS must provide comma-separated asset ids",
            )
        })?;
        config.asset_ids = asset_ids
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .collect();
        if config.asset_ids.is_empty() {
            return Err(MarketIngestionError::new(
                MarketStreamReasonCode::InvalidPayload.code(),
                "EXECUTION_MARKET_STREAM_ASSET_IDS cannot be blank",
            ));
        }
        if config.cluster_id.is_empty() {
            return Err(MarketIngestionError::new(
                MarketStreamReasonCode::InvalidPayload.code(),
                "EXECUTION_MARKET_STREAM_CLUSTER_ID cannot be blank",
            ));
        }
        if config.stream_name.is_empty() {
            return Err(MarketIngestionError::new(
                MarketStreamReasonCode::InvalidPayload.code(),
                "EXECUTION_MARKET_STREAM_NAME cannot be blank",
            ));
        }
        if config.stream_endpoint.is_empty() {
            return Err(MarketIngestionError::new(
                MarketStreamReasonCode::InvalidPayload.code(),
                "EXECUTION_MARKET_STREAM_ENDPOINT cannot be blank",
            ));
        }
        Ok(config)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngestionDisposition {
    Accepted,
    Quarantined,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketIngestionOutcome {
    pub disposition: IngestionDisposition,
    pub reason_code: String,
    pub correlation_id: String,
}

#[derive(Debug, Clone)]
struct IngestionRuntimeState {
    latencies_seconds: Vec<f64>,
    last_observed_at: Option<OffsetDateTime>,
    last_ingested_at: Option<OffsetDateTime>,
    sustained_backlog_seconds: f64,
    backlog_above_threshold: bool,
    last_health_signature: Option<(MarketStreamHealthStatus, String)>,
}

impl Default for IngestionRuntimeState {
    fn default() -> Self {
        Self {
            latencies_seconds: Vec::new(),
            last_observed_at: None,
            last_ingested_at: None,
            sustained_backlog_seconds: 0.0,
            backlog_above_threshold: false,
            last_health_signature: None,
        }
    }
}

pub struct MarketStreamIngestionRuntime<S: MarketStreamStore> {
    store: S,
    config: MarketStreamRuntimeConfig,
    state: IngestionRuntimeState,
    freshness_signals: Option<freshness_gate::SharedFreshnessSignals>,
}

impl<S: MarketStreamStore> MarketStreamIngestionRuntime<S> {
    pub fn new(store: S, config: MarketStreamRuntimeConfig) -> Self {
        Self {
            store,
            config,
            state: IngestionRuntimeState::default(),
            freshness_signals: None,
        }
    }

    pub fn attach_freshness_signals(&mut self, signals: freshness_gate::SharedFreshnessSignals) {
        self.freshness_signals = Some(signals);
    }

    pub fn latency_slo_met(&self) -> bool {
        ingestion_latency_slo_met(&self.state.latencies_seconds)
    }

    pub async fn process_orderbook_event(
        &mut self,
        event: &BookUpdate,
        ingested_at_utc: &str,
    ) -> Result<MarketIngestionOutcome, MarketIngestionError> {
        let ingested_at = parse_rfc3339_utc(ingested_at_utc)?;
        match self.book_update_to_tick(event, ingested_at_utc, ingested_at) {
            Ok(tick) => {
                if let Err(error) = validate_market_stream_tick(&tick) {
                    return self
                        .quarantine_book_event(
                            event,
                            &tick.correlation_id,
                            ingested_at_utc,
                            error.code,
                            error.message,
                        )
                        .await;
                }
                if let Err(error) = self.store.persist_accepted_tick(&tick).await {
                    emit_persistence_failure_telemetry(
                        &tick.correlation_id,
                        &tick.ingested_at_utc,
                        error.code,
                        Some(tick.ingestion_latency_seconds),
                        Some(self.state.sustained_backlog_seconds),
                        None,
                    );
                    return Err(error);
                }
                if let Some(signals) = self.freshness_signals.as_ref() {
                    signals
                        .record_market_update(&tick.observed_at_utc)
                        .map_err(|error| MarketIngestionError::new(error.code, error.message))?;
                }
                self.state
                    .latencies_seconds
                    .push(tick.ingestion_latency_seconds);
                if self.state.latencies_seconds.len() > 2_000 {
                    let _ = self.state.latencies_seconds.remove(0);
                }

                let observed_at = parse_rfc3339_utc(&tick.observed_at_utc)?;
                let heartbeat_gap_seconds = self.update_runtime_state(
                    observed_at,
                    ingested_at,
                    tick.ingestion_latency_seconds,
                );
                self.persist_health_transition(
                    &tick,
                    heartbeat_gap_seconds,
                    &tick.correlation_id,
                    &tick.observed_at_utc,
                )
                .await?;

                if !self.latency_slo_met() {
                    emit_market_stream_telemetry(MarketStreamTelemetryEvent {
                        event_name: "execution_market_stream_latency_slo_v1",
                        outcome: "degrade",
                        reason_code: MarketStreamReasonCode::LatencySloBreached.code(),
                        correlation_id: &tick.correlation_id,
                        timestamp_utc: ingested_at_utc,
                        latency_seconds: Some(tick.ingestion_latency_seconds),
                        sustained_backlog_seconds: Some(self.state.sustained_backlog_seconds),
                        heartbeat_gap_seconds: Some(heartbeat_gap_seconds),
                    });
                }

                emit_market_stream_telemetry(MarketStreamTelemetryEvent {
                    event_name: "execution_market_stream_tick_ingested_v1",
                    outcome: "allow",
                    reason_code: MarketStreamReasonCode::TickAccepted.code(),
                    correlation_id: &tick.correlation_id,
                    timestamp_utc: &tick.ingested_at_utc,
                    latency_seconds: Some(tick.ingestion_latency_seconds),
                    sustained_backlog_seconds: Some(self.state.sustained_backlog_seconds),
                    heartbeat_gap_seconds: Some(heartbeat_gap_seconds),
                });
                Ok(MarketIngestionOutcome {
                    disposition: IngestionDisposition::Accepted,
                    reason_code: MarketStreamReasonCode::TickAccepted.code().to_string(),
                    correlation_id: tick.correlation_id,
                })
            }
            Err(error) => {
                let correlation_id = format!("market-stream-quarantine::{}", event.timestamp);
                self.quarantine_book_event(
                    event,
                    &correlation_id,
                    ingested_at_utc,
                    error.code,
                    error.message,
                )
                .await
            }
        }
    }

    pub async fn record_stream_disconnect(
        &mut self,
        correlation_id: &str,
        observed_at_utc: &str,
    ) -> Result<(), MarketIngestionError> {
        let observed_at = parse_rfc3339_utc(observed_at_utc)?;
        let heartbeat_gap_seconds = self
            .state
            .last_observed_at
            .map(|last_observed| (observed_at - last_observed).as_seconds_f64().max(0.0))
            .unwrap_or(self.config.heartbeat_timeout_seconds + 1.0);

        let health = MarketStreamHealth {
            health_event_id: health_event_id(
                &self.config.stream_name,
                "disconnected",
                observed_at_utc,
                correlation_id,
            ),
            stream_name: self.config.stream_name.clone(),
            status: MarketStreamHealthStatus::Degraded,
            reason_code: MarketStreamReasonCode::StreamDisconnected
                .code()
                .to_string(),
            backlog_seconds: self.state.latencies_seconds.last().copied().unwrap_or(0.0),
            sustained_backlog_seconds: self.state.sustained_backlog_seconds,
            heartbeat_gap_seconds,
            correlation_id: correlation_id.to_string(),
            observed_at_utc: observed_at_utc.to_string(),
        };
        if let Err(error) = self.store.persist_health(&health).await {
            emit_persistence_failure_telemetry(
                correlation_id,
                observed_at_utc,
                error.code,
                None,
                Some(self.state.sustained_backlog_seconds),
                Some(heartbeat_gap_seconds),
            );
            return Err(error);
        }
        self.state.last_health_signature = Some((
            MarketStreamHealthStatus::Degraded,
            health.reason_code.clone(),
        ));

        emit_market_stream_telemetry(MarketStreamTelemetryEvent {
            event_name: "execution_market_stream_connection_v1",
            outcome: "degrade",
            reason_code: MarketStreamReasonCode::StreamDisconnected.code(),
            correlation_id,
            timestamp_utc: observed_at_utc,
            latency_seconds: None,
            sustained_backlog_seconds: Some(self.state.sustained_backlog_seconds),
            heartbeat_gap_seconds: Some(heartbeat_gap_seconds),
        });
        Ok(())
    }

    fn update_runtime_state(
        &mut self,
        observed_at: OffsetDateTime,
        ingested_at: OffsetDateTime,
        backlog_seconds: f64,
    ) -> f64 {
        let heartbeat_gap_seconds = self
            .state
            .last_observed_at
            .map(|last_observed| (observed_at - last_observed).as_seconds_f64().max(0.0))
            .unwrap_or(0.0);

        if backlog_seconds > MARKET_STREAM_BACKLOG_DEGRADED_SECONDS {
            if self.state.backlog_above_threshold {
                let elapsed = self
                    .state
                    .last_ingested_at
                    .map(|last_ingested| (ingested_at - last_ingested).as_seconds_f64().max(0.0))
                    .unwrap_or(0.0);
                self.state.sustained_backlog_seconds += elapsed;
            } else {
                self.state.sustained_backlog_seconds = 0.0;
            }
            self.state.backlog_above_threshold = true;
        } else {
            self.state.sustained_backlog_seconds = 0.0;
            self.state.backlog_above_threshold = false;
        }

        self.state.last_observed_at = Some(observed_at);
        self.state.last_ingested_at = Some(ingested_at);
        heartbeat_gap_seconds
    }

    async fn persist_health_transition(
        &mut self,
        tick: &MarketStreamTick,
        heartbeat_gap_seconds: f64,
        correlation_id: &str,
        observed_at_utc: &str,
    ) -> Result<(), MarketIngestionError> {
        let assessment = assess_market_stream_health(
            tick.ingestion_latency_seconds,
            self.state.sustained_backlog_seconds,
            heartbeat_gap_seconds,
            self.config.heartbeat_timeout_seconds,
        )
        .map_err(|error| MarketIngestionError::new(error.code, error.message))?;

        let signature = (assessment.status, assessment.reason_code.clone());
        if self
            .state
            .last_health_signature
            .as_ref()
            .is_some_and(|previous| previous == &signature)
        {
            return Ok(());
        }

        let health = MarketStreamHealth {
            health_event_id: health_event_id(
                &self.config.stream_name,
                assessment.status.as_str(),
                observed_at_utc,
                correlation_id,
            ),
            stream_name: self.config.stream_name.clone(),
            status: assessment.status,
            reason_code: assessment.reason_code.clone(),
            backlog_seconds: tick.ingestion_latency_seconds,
            sustained_backlog_seconds: self.state.sustained_backlog_seconds,
            heartbeat_gap_seconds,
            correlation_id: correlation_id.to_string(),
            observed_at_utc: observed_at_utc.to_string(),
        };
        if let Err(error) = self.store.persist_health(&health).await {
            emit_persistence_failure_telemetry(
                correlation_id,
                observed_at_utc,
                error.code,
                Some(tick.ingestion_latency_seconds),
                Some(self.state.sustained_backlog_seconds),
                Some(heartbeat_gap_seconds),
            );
            return Err(error);
        }
        self.state.last_health_signature = Some(signature);
        emit_market_stream_telemetry(MarketStreamTelemetryEvent {
            event_name: "execution_market_stream_health_transition_v1",
            outcome: if assessment.status == MarketStreamHealthStatus::Healthy {
                "allow"
            } else {
                "degrade"
            },
            reason_code: &assessment.reason_code,
            correlation_id,
            timestamp_utc: observed_at_utc,
            latency_seconds: Some(tick.ingestion_latency_seconds),
            sustained_backlog_seconds: Some(self.state.sustained_backlog_seconds),
            heartbeat_gap_seconds: Some(heartbeat_gap_seconds),
        });
        Ok(())
    }

    async fn quarantine_book_event(
        &self,
        event: &BookUpdate,
        correlation_id: &str,
        ingested_at_utc: &str,
        reason_code: &'static str,
        reason_message: String,
    ) -> Result<MarketIngestionOutcome, MarketIngestionError> {
        let observed_at_utc = timestamp_millis_to_rfc3339_utc(event.timestamp)
            .unwrap_or_else(|_| ingested_at_utc.to_string());
        let latency_seconds = parse_rfc3339_utc(ingested_at_utc)
            .and_then(|ingested| {
                parse_rfc3339_utc(&observed_at_utc)
                    .map(|observed| (ingested - observed).as_seconds_f64().max(0.0))
            })
            .unwrap_or(0.0);
        let quarantined_payload = json!({
            "reason_message": reason_message,
            "event": serialize_book_event_payload(event)
        });
        let quarantined = QuarantinedMarketEvent {
            event_id: format!(
                "quarantine::{}::{}::{}",
                event.market,
                event.timestamp,
                compact_timestamp_token(ingested_at_utc)
            ),
            reason_code: reason_code.to_string(),
            correlation_id: correlation_id.to_string(),
            observed_at_utc,
            quarantined_at_utc: ingested_at_utc.to_string(),
            ingestion_latency_seconds: latency_seconds,
            raw_payload: quarantined_payload,
        };
        if let Err(error) = self.store.persist_quarantined_event(&quarantined).await {
            emit_persistence_failure_telemetry(
                &quarantined.correlation_id,
                &quarantined.quarantined_at_utc,
                error.code,
                Some(quarantined.ingestion_latency_seconds),
                None,
                None,
            );
            return Err(error);
        }
        emit_market_stream_telemetry(MarketStreamTelemetryEvent {
            event_name: "execution_market_stream_quarantine_v1",
            outcome: "deny",
            reason_code: &quarantined.reason_code,
            correlation_id: &quarantined.correlation_id,
            timestamp_utc: &quarantined.quarantined_at_utc,
            latency_seconds: Some(quarantined.ingestion_latency_seconds),
            sustained_backlog_seconds: None,
            heartbeat_gap_seconds: None,
        });
        Ok(MarketIngestionOutcome {
            disposition: IngestionDisposition::Quarantined,
            reason_code: quarantined.reason_code.clone(),
            correlation_id: quarantined.correlation_id,
        })
    }

    fn book_update_to_tick(
        &self,
        event: &BookUpdate,
        ingested_at_utc: &str,
        ingested_at: OffsetDateTime,
    ) -> Result<MarketStreamTick, MarketIngestionError> {
        let observed_at_utc = timestamp_millis_to_rfc3339_utc(event.timestamp)?;
        let observed_at = parse_rfc3339_utc(&observed_at_utc)?;
        let ingestion_latency_seconds = (ingested_at - observed_at).as_seconds_f64();
        if ingestion_latency_seconds < 0.0 {
            return Err(MarketIngestionError::new(
                MarketStreamReasonCode::InvalidPayload.code(),
                "observed_at_utc is later than ingested_at_utc",
            ));
        }

        let top_bid_depth = normalize_depth_levels("bids", &event.bids)?;
        let top_ask_depth = normalize_depth_levels("asks", &event.asks)?;
        let best_bid = top_bid_depth
            .first()
            .map(|level| level.price)
            .ok_or_else(|| {
                MarketIngestionError::new(
                    MarketStreamReasonCode::UnsupportedEventShape.code(),
                    "orderbook event missing best bid",
                )
            })?;
        let best_ask = top_ask_depth
            .first()
            .map(|level| level.price)
            .ok_or_else(|| {
                MarketIngestionError::new(
                    MarketStreamReasonCode::UnsupportedEventShape.code(),
                    "orderbook event missing best ask",
                )
            })?;
        let last_trade_price = (best_bid + best_ask) / 2.0;
        let tick_size = derive_tick_size(
            &top_bid_depth,
            &top_ask_depth,
            self.config.default_tick_size,
        );
        let market_id = event.market.to_string();
        let correlation_id = format!("market-stream::{}::{}", market_id, event.timestamp);

        Ok(MarketStreamTick {
            tick_id: format!(
                "tick::{}::{}::{}",
                market_id, event.asset_id, event.timestamp
            ),
            market_id,
            asset_id: event.asset_id.to_string(),
            cluster_id: self.config.cluster_id.clone(),
            best_bid,
            best_ask,
            top_bid_depth,
            top_ask_depth,
            last_trade_price,
            tick_size,
            market_status: MarketStatus::Trading,
            correlation_id,
            observed_at_utc,
            ingested_at_utc: ingested_at_utc.to_string(),
            ingestion_latency_seconds,
        })
    }
}

pub async fn run_polymarket_ws_ingestion<S: MarketStreamStore>(
    runtime: &mut MarketStreamIngestionRuntime<S>,
) -> Result<(), MarketIngestionError> {
    let asset_ids = runtime
        .config
        .asset_ids
        .iter()
        .map(|asset_id| {
            U256::from_str(asset_id).map_err(|_| {
                MarketIngestionError::new(
                    MarketStreamReasonCode::InvalidPayload.code(),
                    format!("invalid market stream asset id `{asset_id}`"),
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let reconnect_backoff = Duration::from_secs_f64(runtime.config.reconnect_backoff_seconds);
    let max_reconnect_attempts = runtime.config.max_reconnect_attempts;
    let mut reconnect_attempts = 0_u32;

    'reconnect: loop {
        let ws_client = match PolymarketWsClient::new(
            &runtime.config.stream_endpoint,
            polymarket_client_sdk::ws::config::Config::default(),
        ) {
            Ok(client) => client,
            Err(error) => {
                let disconnect_timestamp = now_utc_rfc3339()?;
                let correlation_id = format!("market-stream-disconnect::{disconnect_timestamp}");
                runtime
                    .record_stream_disconnect(&correlation_id, &disconnect_timestamp)
                    .await?;
                reconnect_attempts += 1;
                if reconnect_attempts > max_reconnect_attempts {
                    return Err(MarketIngestionError::new(
                        MarketStreamReasonCode::StreamDisconnected.code(),
                        format!(
                            "unable to initialize market websocket client after {max_reconnect_attempts} reconnect attempts: {error}"
                        ),
                    ));
                }
                sleep(reconnect_backoff).await;
                continue;
            }
        };

        let stream = match ws_client.subscribe_orderbook(asset_ids.clone()) {
            Ok(stream) => stream,
            Err(error) => {
                let disconnect_timestamp = now_utc_rfc3339()?;
                let correlation_id = format!("market-stream-disconnect::{disconnect_timestamp}");
                runtime
                    .record_stream_disconnect(&correlation_id, &disconnect_timestamp)
                    .await?;
                reconnect_attempts += 1;
                if reconnect_attempts > max_reconnect_attempts {
                    return Err(MarketIngestionError::new(
                        MarketStreamReasonCode::StreamDisconnected.code(),
                        format!(
                            "unable to subscribe to market websocket stream after {max_reconnect_attempts} reconnect attempts: {error}"
                        ),
                    ));
                }
                sleep(reconnect_backoff).await;
                continue;
            }
        };
        tokio::pin!(stream);

        while let Some(next_message) = stream.next().await {
            let ingested_at_utc = now_utc_rfc3339()?;
            match next_message {
                Ok(book_event) => {
                    runtime
                        .process_orderbook_event(&book_event, &ingested_at_utc)
                        .await?;
                    reconnect_attempts = 0;
                }
                Err(error) => {
                    let correlation_id = format!("market-stream-disconnect::{ingested_at_utc}");
                    runtime
                        .record_stream_disconnect(&correlation_id, &ingested_at_utc)
                        .await?;
                    reconnect_attempts += 1;
                    if reconnect_attempts > max_reconnect_attempts {
                        return Err(MarketIngestionError::new(
                            MarketStreamReasonCode::StreamDisconnected.code(),
                            format!(
                                "market websocket stream disconnected after {max_reconnect_attempts} reconnect attempts: {error}"
                            ),
                        ));
                    }
                    sleep(reconnect_backoff).await;
                    continue 'reconnect;
                }
            }
        }

        let disconnect_timestamp = now_utc_rfc3339()?;
        let correlation_id = format!("market-stream-disconnect::{disconnect_timestamp}");
        runtime
            .record_stream_disconnect(&correlation_id, &disconnect_timestamp)
            .await?;
        reconnect_attempts += 1;
        if reconnect_attempts > max_reconnect_attempts {
            return Err(MarketIngestionError::new(
                MarketStreamReasonCode::StreamDisconnected.code(),
                format!(
                    "market websocket stream terminated unexpectedly after {max_reconnect_attempts} reconnect attempts"
                ),
            ));
        }
        sleep(reconnect_backoff).await;
    }
}

fn normalize_depth_levels(
    field: &'static str,
    levels: &[OrderBookLevel],
) -> Result<Vec<MarketDepthLevel>, MarketIngestionError> {
    if levels.is_empty() {
        return Err(MarketIngestionError::new(
            MarketStreamReasonCode::UnsupportedEventShape.code(),
            format!("{field} cannot be empty"),
        ));
    }

    levels
        .iter()
        .take(5)
        .map(|level| {
            let price = decimal_to_f64(level.price, "price")?;
            let size = decimal_to_f64(level.size, "size")?;
            Ok(MarketDepthLevel { price, size })
        })
        .collect()
}

fn decimal_to_f64(
    value: polymarket_client_sdk::types::Decimal,
    field: &'static str,
) -> Result<f64, MarketIngestionError> {
    value.to_string().parse::<f64>().map_err(|error| {
        MarketIngestionError::new(
            MarketStreamReasonCode::InvalidPayload.code(),
            format!("unable to parse `{field}` decimal value: {error}"),
        )
    })
}

fn derive_tick_size(
    bids: &[MarketDepthLevel],
    asks: &[MarketDepthLevel],
    default_tick_size: f64,
) -> f64 {
    let mut increments = bids
        .windows(2)
        .map(|pair| (pair[0].price - pair[1].price).abs())
        .chain(
            asks.windows(2)
                .map(|pair| (pair[1].price - pair[0].price).abs()),
        )
        .filter(|value| value.is_finite() && *value > 0.0)
        .collect::<Vec<_>>();
    increments.sort_by(f64::total_cmp);
    increments.first().copied().unwrap_or(default_tick_size)
}

fn timestamp_millis_to_rfc3339_utc(timestamp_millis: i64) -> Result<String, MarketIngestionError> {
    let nanos = (timestamp_millis as i128)
        .checked_mul(1_000_000)
        .ok_or_else(|| {
            MarketIngestionError::new(
                MarketStreamReasonCode::InvalidPayload.code(),
                "market timestamp overflow",
            )
        })?;
    let observed = OffsetDateTime::from_unix_timestamp_nanos(nanos).map_err(|error| {
        MarketIngestionError::new(
            MarketStreamReasonCode::InvalidPayload.code(),
            format!("invalid market timestamp: {error}"),
        )
    })?;
    observed.format(&Rfc3339).map_err(|error| {
        MarketIngestionError::new(
            MarketStreamReasonCode::InvalidPayload.code(),
            format!("unable to format market timestamp: {error}"),
        )
    })
}

fn parse_rfc3339_utc(value: &str) -> Result<OffsetDateTime, MarketIngestionError> {
    let parsed = OffsetDateTime::parse(value, &Rfc3339).map_err(|error| {
        MarketIngestionError::new(
            MarketStreamReasonCode::InvalidPayload.code(),
            format!("invalid RFC3339 timestamp `{value}`: {error}"),
        )
    })?;
    if parsed.offset() != time::UtcOffset::UTC {
        return Err(MarketIngestionError::new(
            MarketStreamReasonCode::InvalidPayload.code(),
            "timestamps must use UTC `Z` offset",
        ));
    }
    Ok(parsed)
}

fn parse_positive_f64(key: &str, value: &str) -> Result<f64, MarketIngestionError> {
    let parsed = value.parse::<f64>().map_err(|_| {
        MarketIngestionError::new(
            MarketStreamReasonCode::InvalidPayload.code(),
            format!("{key} must be a floating-point number"),
        )
    })?;
    if !parsed.is_finite() || parsed <= 0.0 {
        return Err(MarketIngestionError::new(
            MarketStreamReasonCode::InvalidPayload.code(),
            format!("{key} must be finite and greater than 0"),
        ));
    }
    Ok(parsed)
}

fn parse_positive_u32(key: &str, value: &str) -> Result<u32, MarketIngestionError> {
    let parsed = value.parse::<u32>().map_err(|_| {
        MarketIngestionError::new(
            MarketStreamReasonCode::InvalidPayload.code(),
            format!("{key} must be an integer"),
        )
    })?;
    if parsed == 0 {
        return Err(MarketIngestionError::new(
            MarketStreamReasonCode::InvalidPayload.code(),
            format!("{key} must be greater than 0"),
        ));
    }
    Ok(parsed)
}

fn now_utc_rfc3339() -> Result<String, MarketIngestionError> {
    OffsetDateTime::now_utc().format(&Rfc3339).map_err(|error| {
        MarketIngestionError::new(
            MarketStreamReasonCode::InvalidPayload.code(),
            format!("unable to format UTC timestamp: {error}"),
        )
    })
}

fn compact_timestamp_token(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_digit())
        .collect()
}

fn health_event_id(
    stream_name: &str,
    suffix: &str,
    observed_at_utc: &str,
    correlation_id: &str,
) -> String {
    format!(
        "health::{}::{}::{}::{}",
        stream_name,
        suffix,
        observed_at_utc
            .replace(['-', ':'], "")
            .replace(['T', 'Z'], ""),
        correlation_id.replace(':', "_")
    )
}

fn serialize_book_event_payload(event: &BookUpdate) -> serde_json::Value {
    let bids = event
        .bids
        .iter()
        .map(|level| {
            json!({
                "price": level.price.to_string(),
                "size": level.size.to_string()
            })
        })
        .collect::<Vec<_>>();
    let asks = event
        .asks
        .iter()
        .map(|level| {
            json!({
                "price": level.price.to_string(),
                "size": level.size.to_string()
            })
        })
        .collect::<Vec<_>>();
    json!({
        "event_type": "book",
        "asset_id": event.asset_id.to_string(),
        "market": event.market.to_string(),
        "timestamp": event.timestamp,
        "bids": bids,
        "asks": asks
    })
}

fn emit_market_stream_telemetry(event: MarketStreamTelemetryEvent<'_>) {
    println!(
        "{}",
        serde_json::to_string(&event).expect("market stream telemetry should serialize")
    );
}

fn emit_persistence_failure_telemetry(
    correlation_id: &str,
    timestamp_utc: &str,
    reason_code: &str,
    latency_seconds: Option<f64>,
    sustained_backlog_seconds: Option<f64>,
    heartbeat_gap_seconds: Option<f64>,
) {
    emit_market_stream_telemetry(MarketStreamTelemetryEvent {
        event_name: "execution_market_stream_persistence_failure_v1",
        outcome: "deny",
        reason_code,
        correlation_id,
        timestamp_utc,
        latency_seconds,
        sustained_backlog_seconds,
        heartbeat_gap_seconds,
    });
}

#[derive(Debug, Serialize)]
struct MarketStreamTelemetryEvent<'a> {
    event_name: &'a str,
    outcome: &'a str,
    reason_code: &'a str,
    correlation_id: &'a str,
    timestamp_utc: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    latency_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sustained_backlog_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    heartbeat_gap_seconds: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[derive(Clone)]
    struct FailingAcceptedTickStore;

    impl MarketStreamStore for FailingAcceptedTickStore {
        async fn persist_accepted_tick(
            &self,
            _tick: &MarketStreamTick,
        ) -> Result<(), MarketIngestionError> {
            Err(MarketIngestionError::new(
                MarketStreamReasonCode::PersistenceUnavailable.code(),
                "forced accepted tick persistence failure",
            ))
        }

        async fn persist_quarantined_event(
            &self,
            _event: &QuarantinedMarketEvent,
        ) -> Result<(), MarketIngestionError> {
            Ok(())
        }

        async fn persist_health(
            &self,
            _health: &MarketStreamHealth,
        ) -> Result<(), MarketIngestionError> {
            Ok(())
        }
    }

    fn sample_config() -> MarketStreamRuntimeConfig {
        MarketStreamRuntimeConfig {
            stream_name: "polymarket_market_stream".to_string(),
            stream_endpoint: "wss://ws-subscriptions-clob.polymarket.com".to_string(),
            cluster_id: "cluster_alpha".to_string(),
            asset_ids: vec!["1".to_string()],
            default_tick_size: 0.01,
            heartbeat_timeout_seconds: 15.0,
            reconnect_backoff_seconds: 1.0,
            max_reconnect_attempts: 5,
        }
    }

    fn sample_book_update(timestamp_millis: i64, include_asks: bool) -> BookUpdate {
        serde_json::from_value(json!({
            "asset_id": "1",
            "market": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "timestamp": timestamp_millis.to_string(),
            "bids": [
                { "price": "0.51", "size": "1200" },
                { "price": "0.50", "size": "900" }
            ],
            "asks": if include_asks {
                json!([
                    { "price": "0.53", "size": "1000" },
                    { "price": "0.54", "size": "800" }
                ])
            } else {
                json!([])
            },
            "hash": null
        }))
        .expect("book update fixture should deserialize")
    }

    fn rfc3339_from_millis(timestamp_millis: i64) -> String {
        OffsetDateTime::from_unix_timestamp_nanos((timestamp_millis as i128) * 1_000_000)
            .expect("timestamp should parse")
            .format(&Rfc3339)
            .expect("timestamp should format")
    }

    #[tokio::test]
    async fn normal_load_path_meets_latency_target_boundary() {
        let store = InMemoryMarketStreamStore::default();
        let mut runtime = MarketStreamIngestionRuntime::new(store.clone(), sample_config());
        let start = 1_775_404_800_000_i64; // 2026-04-06T00:00:00Z

        for index in 0..100 {
            let observed = start + (index * 1_000);
            let ingested = observed + 1_500;
            let outcome = runtime
                .process_orderbook_event(
                    &sample_book_update(observed, true),
                    &rfc3339_from_millis(ingested),
                )
                .await
                .expect("normal-load event should ingest successfully");
            assert_eq!(outcome.disposition, IngestionDisposition::Accepted);
        }

        assert!(runtime.latency_slo_met());
        assert_eq!(store.accepted_ticks().len(), 100);
    }

    #[tokio::test]
    async fn malformed_payloads_are_quarantined_without_crashing_following_events() {
        let store = InMemoryMarketStreamStore::default();
        let mut runtime = MarketStreamIngestionRuntime::new(store.clone(), sample_config());
        let start = 1_775_404_800_000_i64;

        let malformed = runtime
            .process_orderbook_event(
                &sample_book_update(start, false),
                &rfc3339_from_millis(start + 1_200),
            )
            .await
            .expect("malformed event should quarantine cleanly");
        assert_eq!(malformed.disposition, IngestionDisposition::Quarantined);

        let valid = runtime
            .process_orderbook_event(
                &sample_book_update(start + 1_000, true),
                &rfc3339_from_millis(start + 2_200),
            )
            .await
            .expect("pipeline should continue with next valid event");
        assert_eq!(valid.disposition, IngestionDisposition::Accepted);
        assert_eq!(store.quarantined_events().len(), 1);
        assert_eq!(store.accepted_ticks().len(), 1);
    }

    #[tokio::test]
    async fn burst_backlog_enters_degraded_mode_after_sustained_threshold() {
        let store = InMemoryMarketStreamStore::default();
        let mut runtime = MarketStreamIngestionRuntime::new(store.clone(), sample_config());
        let start = 1_775_404_800_000_i64;

        for step in 0..4 {
            let observed = start + (step * 12_000);
            let ingested = observed + 11_000;
            let outcome = runtime
                .process_orderbook_event(
                    &sample_book_update(observed, true),
                    &rfc3339_from_millis(ingested),
                )
                .await
                .expect("burst event should process");
            assert_eq!(outcome.disposition, IngestionDisposition::Accepted);
        }

        let health_events = store.health_events();
        assert!(
            health_events.iter().any(|event| event.reason_code
                == MarketStreamReasonCode::BacklogSustainedExceeded.code())
        );
    }

    #[tokio::test]
    async fn heartbeat_timeout_transitions_stream_health_to_degraded() {
        let store = InMemoryMarketStreamStore::default();
        let mut runtime = MarketStreamIngestionRuntime::new(store.clone(), sample_config());
        let start = 1_775_404_800_000_i64;

        runtime
            .process_orderbook_event(
                &sample_book_update(start, true),
                &rfc3339_from_millis(start + 1_000),
            )
            .await
            .expect("baseline event should ingest");

        runtime
            .process_orderbook_event(
                &sample_book_update(start + 20_000, true),
                &rfc3339_from_millis(start + 21_000),
            )
            .await
            .expect("heartbeat-gap event should ingest");

        let health_events = store.health_events();
        assert!(health_events.iter().any(|event| {
            event.status == MarketStreamHealthStatus::Degraded
                && event.reason_code == MarketStreamReasonCode::HeartbeatTimeout.code()
        }));
    }

    #[tokio::test]
    async fn record_stream_disconnect_persists_degraded_reason_code() {
        let store = InMemoryMarketStreamStore::default();
        let mut runtime = MarketStreamIngestionRuntime::new(store.clone(), sample_config());
        let observed_at_utc = "2026-04-06T00:00:45Z";

        runtime
            .record_stream_disconnect("corr-disconnect-001", observed_at_utc)
            .await
            .expect("disconnect should persist degraded health event");

        let health_events = store.health_events();
        assert_eq!(health_events.len(), 1);
        let disconnect_event = health_events
            .first()
            .expect("disconnect health event should be recorded");
        assert_eq!(disconnect_event.status, MarketStreamHealthStatus::Degraded);
        assert_eq!(
            disconnect_event.reason_code,
            MarketStreamReasonCode::StreamDisconnected.code()
        );
        assert_eq!(disconnect_event.correlation_id, "corr-disconnect-001");
        assert_eq!(disconnect_event.observed_at_utc, observed_at_utc);
    }

    #[tokio::test]
    async fn accepted_tick_persistence_failure_surfaces_machine_readable_reason() {
        let mut runtime =
            MarketStreamIngestionRuntime::new(FailingAcceptedTickStore, sample_config());
        let start = 1_775_404_800_000_i64;

        let error = runtime
            .process_orderbook_event(
                &sample_book_update(start, true),
                &rfc3339_from_millis(start + 1_500),
            )
            .await
            .expect_err("accepted tick persistence failure should surface as error");

        assert_eq!(
            error.code,
            MarketStreamReasonCode::PersistenceUnavailable.code()
        );
        assert_eq!(error.message, "forced accepted tick persistence failure");
    }
}
