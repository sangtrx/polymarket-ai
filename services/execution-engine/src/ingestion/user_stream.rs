#[cfg(test)]
use domain::risk::USER_STREAM_AUTH_STATE_PARTITION_KEY;
use domain::risk::{
    OrderEventOffsetCursor, UserStreamAuthState, UserStreamAuthTransition, UserStreamEvent,
    UserStreamEventKind, UserStreamEventStatus, UserStreamReasonCode,
    normalize_user_stream_idempotency_key, user_stream_latency_slo_met,
};
use futures::StreamExt;
use persistence::postgres::user_stream::{
    UserStreamPersistDisposition, UserStreamPersistOutcome, UserStreamPersistenceError,
    load_user_stream_auth_cursor, persist_user_stream_auth_transition, persist_user_stream_event,
};
use polymarket_client_sdk::auth::{Credentials, Uuid};
use polymarket_client_sdk::clob::types::OrderStatusType;
use polymarket_client_sdk::clob::ws::Client as PolymarketWsClient;
use polymarket_client_sdk::clob::ws::types::response::{
    OrderMessage, OrderMessageType, TradeMessage, TradeMessageStatus,
};
use polymarket_client_sdk::types::{Address, B256};
use polymarket_client_sdk::ws::WsError;
use serde::Serialize;
use serde_json::json;
use sqlx::PgPool;
#[cfg(test)]
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::str::FromStr;
#[cfg(test)]
use std::sync::{Arc, Mutex};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tokio::time::{Duration, sleep};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserStreamIngestionError {
    pub code: &'static str,
    pub message: String,
}

impl UserStreamIngestionError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl Display for UserStreamIngestionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for UserStreamIngestionError {}

pub trait UserStreamStore: Send + Sync {
    async fn persist_event(
        &self,
        event: &UserStreamEvent,
        raw_payload: &serde_json::Value,
        auth_state: UserStreamAuthState,
        block_new_intents: bool,
    ) -> Result<UserStreamPersistOutcome, UserStreamIngestionError>;

    async fn persist_auth_transition(
        &self,
        transition: &UserStreamAuthTransition,
    ) -> Result<(), UserStreamIngestionError>;

    async fn load_auth_cursor(
        &self,
    ) -> Result<Option<OrderEventOffsetCursor>, UserStreamIngestionError>;
}

#[derive(Clone)]
pub struct PostgresUserStreamStore {
    pool: PgPool,
}

impl PostgresUserStreamStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl UserStreamStore for PostgresUserStreamStore {
    async fn persist_event(
        &self,
        event: &UserStreamEvent,
        raw_payload: &serde_json::Value,
        auth_state: UserStreamAuthState,
        block_new_intents: bool,
    ) -> Result<UserStreamPersistOutcome, UserStreamIngestionError> {
        persist_user_stream_event(
            &self.pool,
            event,
            raw_payload,
            auth_state,
            block_new_intents,
        )
        .await
        .map_err(map_persistence_error)
    }

    async fn persist_auth_transition(
        &self,
        transition: &UserStreamAuthTransition,
    ) -> Result<(), UserStreamIngestionError> {
        persist_user_stream_auth_transition(&self.pool, transition)
            .await
            .map_err(map_persistence_error)
    }

    async fn load_auth_cursor(
        &self,
    ) -> Result<Option<OrderEventOffsetCursor>, UserStreamIngestionError> {
        load_user_stream_auth_cursor(&self.pool)
            .await
            .map_err(map_persistence_error)
    }
}

fn map_persistence_error(error: UserStreamPersistenceError) -> UserStreamIngestionError {
    UserStreamIngestionError::new(error.code, error.to_string())
}

#[derive(Debug, Clone)]
pub struct UserStreamRuntimeConfig {
    pub stream_name: String,
    pub stream_endpoint: String,
    pub market_ids: Vec<String>,
    pub api_key: String,
    pub api_secret: String,
    pub api_passphrase: String,
    pub address: String,
    pub reconnect_backoff_seconds: f64,
    pub max_reconnect_attempts: u32,
}

impl Default for UserStreamRuntimeConfig {
    fn default() -> Self {
        Self {
            stream_name: "polymarket_user_stream".to_string(),
            stream_endpoint: "wss://ws-subscriptions-clob.polymarket.com".to_string(),
            market_ids: Vec::new(),
            api_key: String::new(),
            api_secret: String::new(),
            api_passphrase: String::new(),
            address: String::new(),
            reconnect_backoff_seconds: 1.0,
            max_reconnect_attempts: 5,
        }
    }
}

impl UserStreamRuntimeConfig {
    pub fn from_env() -> Result<Self, UserStreamIngestionError> {
        let mut config = Self::default();
        if let Ok(value) = std::env::var("EXECUTION_USER_STREAM_NAME") {
            config.stream_name = value.trim().to_string();
        }
        if let Ok(value) = std::env::var("EXECUTION_USER_STREAM_ENDPOINT") {
            config.stream_endpoint = value.trim().to_string();
        }
        if let Ok(value) = std::env::var("EXECUTION_USER_STREAM_RECONNECT_BACKOFF_SECONDS") {
            config.reconnect_backoff_seconds =
                parse_positive_f64("EXECUTION_USER_STREAM_RECONNECT_BACKOFF_SECONDS", &value)?;
        }
        if let Ok(value) = std::env::var("EXECUTION_USER_STREAM_MAX_RECONNECT_ATTEMPTS") {
            config.max_reconnect_attempts =
                parse_positive_u32("EXECUTION_USER_STREAM_MAX_RECONNECT_ATTEMPTS", &value)?;
        }

        let market_ids = std::env::var("EXECUTION_USER_STREAM_MARKET_IDS").map_err(|_| {
            UserStreamIngestionError::new(
                UserStreamReasonCode::InvalidPayload.code(),
                "EXECUTION_USER_STREAM_MARKET_IDS must provide comma-separated market ids",
            )
        })?;
        config.market_ids = market_ids
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .collect();
        if config.market_ids.is_empty() {
            return Err(UserStreamIngestionError::new(
                UserStreamReasonCode::InvalidPayload.code(),
                "EXECUTION_USER_STREAM_MARKET_IDS cannot be blank",
            ));
        }

        config.api_key = required_env("EXECUTION_POLYMARKET_API_KEY")?;
        config.api_secret = required_env("EXECUTION_POLYMARKET_API_SECRET")?;
        config.api_passphrase = required_env("EXECUTION_POLYMARKET_API_PASSPHRASE")?;
        config.address = required_env("EXECUTION_POLYMARKET_ADDRESS")?;

        if config.stream_name.trim().is_empty() {
            return Err(UserStreamIngestionError::new(
                UserStreamReasonCode::InvalidPayload.code(),
                "EXECUTION_USER_STREAM_NAME cannot be blank",
            ));
        }
        if config.stream_endpoint.trim().is_empty() {
            return Err(UserStreamIngestionError::new(
                UserStreamReasonCode::InvalidPayload.code(),
                "EXECUTION_USER_STREAM_ENDPOINT cannot be blank",
            ));
        }

        Ok(config)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserIngestionDisposition {
    Accepted,
    Duplicate,
    OutOfOrder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserStreamIngestionOutcome {
    pub disposition: UserIngestionDisposition,
    pub reason_code: String,
    pub correlation_id: String,
}

#[derive(Debug, Clone)]
struct UserStreamRuntimeState {
    latencies_seconds: Vec<f64>,
    auth_state: UserStreamAuthState,
    block_new_intents: bool,
    recovery_pending_first_event: bool,
    next_transition_offset: i64,
}

impl Default for UserStreamRuntimeState {
    fn default() -> Self {
        Self {
            latencies_seconds: Vec::new(),
            auth_state: UserStreamAuthState::Authenticated,
            block_new_intents: false,
            recovery_pending_first_event: false,
            next_transition_offset: 1,
        }
    }
}

pub struct UserStreamIngestionRuntime<S: UserStreamStore> {
    store: S,
    config: UserStreamRuntimeConfig,
    state: UserStreamRuntimeState,
}

impl<S: UserStreamStore> UserStreamIngestionRuntime<S> {
    pub fn new(store: S, config: UserStreamRuntimeConfig) -> Self {
        Self {
            store,
            config,
            state: UserStreamRuntimeState::default(),
        }
    }

    pub fn latency_slo_met(&self) -> bool {
        user_stream_latency_slo_met(&self.state.latencies_seconds)
    }

    #[cfg(test)]
    pub fn block_new_intents(&self) -> bool {
        self.state.block_new_intents
    }

    async fn hydrate_auth_state_from_store(&mut self) -> Result<(), UserStreamIngestionError> {
        if let Some(cursor) = self.store.load_auth_cursor().await? {
            self.state.auth_state = cursor.auth_state;
            self.state.block_new_intents = cursor.block_new_intents;
            self.state.recovery_pending_first_event =
                cursor.auth_state == UserStreamAuthState::Authenticated && cursor.block_new_intents;
            self.state.next_transition_offset = cursor.last_event_offset.saturating_add(1);
        }
        Ok(())
    }

    async fn persist_auth_transition_state(
        &mut self,
        auth_state: UserStreamAuthState,
        block_new_intents: bool,
        reason_code: &'static str,
        correlation_id: &str,
        observed_at_utc: &str,
    ) -> Result<(), UserStreamIngestionError> {
        let transition = UserStreamAuthTransition {
            transition_id: format!(
                "auth::{}::{}",
                auth_state.as_str(),
                compact_timestamp_token(observed_at_utc)
            ),
            transition_offset: self.state.next_transition_offset,
            auth_state,
            block_new_intents,
            reason_code: reason_code.to_string(),
            correlation_id: correlation_id.to_string(),
            observed_at_utc: observed_at_utc.to_string(),
        };
        self.store.persist_auth_transition(&transition).await?;
        self.state.next_transition_offset = self.state.next_transition_offset.saturating_add(1);
        self.state.auth_state = auth_state;
        self.state.block_new_intents = block_new_intents;
        self.state.recovery_pending_first_event =
            auth_state == UserStreamAuthState::Authenticated && block_new_intents;
        emit_user_stream_telemetry(UserStreamTelemetryEvent {
            event_name: "execution_user_stream_auth_state_v1",
            outcome: if block_new_intents { "deny" } else { "allow" },
            reason_code,
            correlation_id,
            timestamp_utc: observed_at_utc,
            latency_seconds: None,
            block_new_intents: Some(block_new_intents),
        });
        Ok(())
    }

    pub async fn process_order_message(
        &mut self,
        order: &OrderMessage,
        ingested_at_utc: &str,
    ) -> Result<UserStreamIngestionOutcome, UserStreamIngestionError> {
        let ingested_at = parse_rfc3339_utc(ingested_at_utc)?;
        let (event, raw_payload) =
            self.order_message_to_event(order, ingested_at_utc, ingested_at)?;
        self.persist_event(event, raw_payload).await
    }

    pub async fn process_trade_message(
        &mut self,
        trade: &TradeMessage,
        ingested_at_utc: &str,
    ) -> Result<UserStreamIngestionOutcome, UserStreamIngestionError> {
        let ingested_at = parse_rfc3339_utc(ingested_at_utc)?;
        let (event, raw_payload) =
            self.trade_message_to_event(trade, ingested_at_utc, ingested_at)?;
        self.persist_event(event, raw_payload).await
    }

    async fn persist_event(
        &mut self,
        event: UserStreamEvent,
        raw_payload: serde_json::Value,
    ) -> Result<UserStreamIngestionOutcome, UserStreamIngestionError> {
        let outcome = self
            .store
            .persist_event(
                &event,
                &raw_payload,
                self.state.auth_state,
                self.state.block_new_intents,
            )
            .await?;

        match outcome.disposition {
            UserStreamPersistDisposition::Accepted => {
                self.state
                    .latencies_seconds
                    .push(event.ingestion_latency_seconds);
                if self.state.latencies_seconds.len() > 2_000 {
                    let _ = self.state.latencies_seconds.remove(0);
                }
                emit_user_stream_telemetry(UserStreamTelemetryEvent {
                    event_name: "execution_user_stream_event_persisted_v1",
                    outcome: "allow",
                    reason_code: &outcome.reason_code,
                    correlation_id: &event.correlation_id,
                    timestamp_utc: &event.ingested_at_utc,
                    latency_seconds: Some(event.ingestion_latency_seconds),
                    block_new_intents: Some(self.state.block_new_intents),
                });

                if !self.latency_slo_met() {
                    emit_user_stream_telemetry(UserStreamTelemetryEvent {
                        event_name: "execution_user_stream_latency_slo_v1",
                        outcome: "degrade",
                        reason_code: UserStreamReasonCode::LatencySloBreached.code(),
                        correlation_id: &event.correlation_id,
                        timestamp_utc: &event.ingested_at_utc,
                        latency_seconds: Some(event.ingestion_latency_seconds),
                        block_new_intents: Some(self.state.block_new_intents),
                    });
                }

                if self.state.recovery_pending_first_event {
                    let recovery_correlation = format!(
                        "user-stream-auth-recovery::{}",
                        compact_timestamp_token(&event.ingested_at_utc)
                    );
                    self.persist_auth_transition_state(
                        UserStreamAuthState::Authenticated,
                        false,
                        UserStreamReasonCode::Authenticated.code(),
                        &recovery_correlation,
                        &event.ingested_at_utc,
                    )
                    .await?;
                }

                Ok(UserStreamIngestionOutcome {
                    disposition: UserIngestionDisposition::Accepted,
                    reason_code: outcome.reason_code,
                    correlation_id: event.correlation_id,
                })
            }
            UserStreamPersistDisposition::Duplicate => {
                emit_user_stream_telemetry(UserStreamTelemetryEvent {
                    event_name: "execution_user_stream_duplicate_suppressed_v1",
                    outcome: "ignore",
                    reason_code: &outcome.reason_code,
                    correlation_id: &event.correlation_id,
                    timestamp_utc: &event.ingested_at_utc,
                    latency_seconds: Some(event.ingestion_latency_seconds),
                    block_new_intents: Some(self.state.block_new_intents),
                });
                Ok(UserStreamIngestionOutcome {
                    disposition: UserIngestionDisposition::Duplicate,
                    reason_code: outcome.reason_code,
                    correlation_id: event.correlation_id,
                })
            }
            UserStreamPersistDisposition::OutOfOrder => {
                emit_user_stream_telemetry(UserStreamTelemetryEvent {
                    event_name: "execution_user_stream_out_of_order_rejected_v1",
                    outcome: "ignore",
                    reason_code: &outcome.reason_code,
                    correlation_id: &event.correlation_id,
                    timestamp_utc: &event.ingested_at_utc,
                    latency_seconds: Some(event.ingestion_latency_seconds),
                    block_new_intents: Some(self.state.block_new_intents),
                });
                Ok(UserStreamIngestionOutcome {
                    disposition: UserIngestionDisposition::OutOfOrder,
                    reason_code: outcome.reason_code,
                    correlation_id: event.correlation_id,
                })
            }
        }
    }

    fn order_message_to_event(
        &self,
        order: &OrderMessage,
        ingested_at_utc: &str,
        ingested_at: OffsetDateTime,
    ) -> Result<(UserStreamEvent, serde_json::Value), UserStreamIngestionError> {
        let event_offset = order.timestamp.ok_or_else(|| {
            UserStreamIngestionError::new(
                UserStreamReasonCode::InvalidPayload.code(),
                "order event missing timestamp offset",
            )
        })?;
        let event_timestamp_utc = timestamp_millis_to_rfc3339_utc(event_offset)?;
        let observed_at = parse_rfc3339_utc(&event_timestamp_utc)?;
        let ingestion_latency_seconds = (ingested_at - observed_at).as_seconds_f64();
        if ingestion_latency_seconds < 0.0 {
            return Err(UserStreamIngestionError::new(
                UserStreamReasonCode::InvalidPayload.code(),
                "order event observed_at_utc is later than ingested_at_utc",
            ));
        }

        let event_status = derive_order_event_status(order)?;
        let partition_key = normalize_user_stream_idempotency_key(&order.id);
        let idempotency_key = normalize_user_stream_idempotency_key(&format!(
            "order::{}::{}::{}",
            order.id,
            event_offset,
            event_status.as_str()
        ));
        let correlation_id = format!("user-stream::order::{}::{}", order.id, event_offset);

        let raw_payload = json!({
            "event_type": "order",
            "id": order.id,
            "market": order.market.to_string(),
            "asset_id": order.asset_id.to_string(),
            "status": format_order_status(&order.status),
            "message_type": format_order_message_type(&order.msg_type),
            "timestamp": event_offset,
            "size_matched": order.size_matched.as_ref().map(|value| value.to_string()),
            "original_size": order.original_size.as_ref().map(|value| value.to_string()),
            "side": format!("{:?}", order.side).to_ascii_lowercase()
        });
        let event = UserStreamEvent {
            event_id: format!("order::{}::{}", order.id, event_offset),
            event_kind: UserStreamEventKind::Order,
            event_status,
            market_id: order.market.to_string(),
            asset_id: order.asset_id.to_string(),
            order_id: order.id.clone(),
            trade_id: None,
            partition_key,
            idempotency_key,
            event_offset,
            event_timestamp_utc: event_timestamp_utc.clone(),
            observed_at_utc: event_timestamp_utc,
            ingested_at_utc: ingested_at_utc.to_string(),
            ingestion_latency_seconds,
            correlation_id,
            reason_code: UserStreamReasonCode::EventAccepted.code().to_string(),
        };
        Ok((event, raw_payload))
    }

    fn trade_message_to_event(
        &self,
        trade: &TradeMessage,
        ingested_at_utc: &str,
        ingested_at: OffsetDateTime,
    ) -> Result<(UserStreamEvent, serde_json::Value), UserStreamIngestionError> {
        let event_offset = trade
            .timestamp
            .or(trade.matchtime)
            .or(trade.last_update)
            .ok_or_else(|| {
                UserStreamIngestionError::new(
                    UserStreamReasonCode::InvalidPayload.code(),
                    "trade event missing timestamp offset",
                )
            })?;
        let event_timestamp_utc = timestamp_millis_to_rfc3339_utc(event_offset)?;
        let observed_at = parse_rfc3339_utc(&event_timestamp_utc)?;
        let ingestion_latency_seconds = (ingested_at - observed_at).as_seconds_f64();
        if ingestion_latency_seconds < 0.0 {
            return Err(UserStreamIngestionError::new(
                UserStreamReasonCode::InvalidPayload.code(),
                "trade event observed_at_utc is later than ingested_at_utc",
            ));
        }

        let event_status = derive_trade_event_status(trade)?;
        let (order_id, partition_key) = derive_trade_partition_key(trade);
        let idempotency_key = normalize_user_stream_idempotency_key(&format!(
            "trade::{}::{}::{}",
            trade.id,
            event_offset,
            event_status.as_str()
        ));
        let correlation_id = format!("user-stream::trade::{}::{}", trade.id, event_offset);

        let raw_payload = json!({
            "event_type": "trade",
            "id": trade.id,
            "market": trade.market.to_string(),
            "asset_id": trade.asset_id.to_string(),
            "status": format!("{:?}", trade.status).to_ascii_lowercase(),
            "timestamp": event_offset,
            "side": format!("{:?}", trade.side).to_ascii_lowercase(),
            "size": trade.size.to_string(),
            "price": trade.price.to_string(),
            "taker_order_id": trade.taker_order_id,
            "maker_order_count": trade.maker_orders.len(),
            "trader_side": trade
                .trader_side
                .as_ref()
                .map(|value| format!("{value:?}").to_ascii_lowercase())
        });
        let event = UserStreamEvent {
            event_id: format!("trade::{}::{}", trade.id, event_offset),
            event_kind: UserStreamEventKind::Trade,
            event_status,
            market_id: trade.market.to_string(),
            asset_id: trade.asset_id.to_string(),
            order_id,
            trade_id: Some(trade.id.clone()),
            partition_key,
            idempotency_key,
            event_offset,
            event_timestamp_utc: event_timestamp_utc.clone(),
            observed_at_utc: event_timestamp_utc,
            ingested_at_utc: ingested_at_utc.to_string(),
            ingestion_latency_seconds,
            correlation_id,
            reason_code: UserStreamReasonCode::EventAccepted.code().to_string(),
        };
        Ok((event, raw_payload))
    }
}

pub async fn run_polymarket_user_ws_ingestion<S: UserStreamStore>(
    runtime: &mut UserStreamIngestionRuntime<S>,
) -> Result<(), UserStreamIngestionError> {
    runtime.hydrate_auth_state_from_store().await?;
    let markets = parse_market_targets(&runtime.config.market_ids)?;
    let credentials = parse_credentials(&runtime.config)?;
    let address = parse_address(&runtime.config.address)?;
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
                let observed_at_utc = now_utc_rfc3339()?;
                let correlation_id = format!(
                    "user-stream-disconnect::{}",
                    compact_timestamp_token(&observed_at_utc)
                );
                runtime
                    .persist_auth_transition_state(
                        UserStreamAuthState::AuthExpired,
                        true,
                        UserStreamReasonCode::StreamDisconnected.code(),
                        &correlation_id,
                        &observed_at_utc,
                    )
                    .await?;
                reconnect_attempts += 1;
                if reconnect_attempts > max_reconnect_attempts {
                    return Err(UserStreamIngestionError::new(
                        UserStreamReasonCode::StreamDisconnected.code(),
                        format!(
                            "unable to initialize user websocket client after {max_reconnect_attempts} reconnect attempts: {error}"
                        ),
                    ));
                }
                sleep(reconnect_backoff).await;
                continue;
            }
        };

        let authenticated_client = match ws_client.authenticate(credentials.clone(), address) {
            Ok(client) => {
                if runtime.state.block_new_intents {
                    let observed_at_utc = now_utc_rfc3339()?;
                    let correlation_id = format!(
                        "user-stream-auth-recovered::{}",
                        compact_timestamp_token(&observed_at_utc)
                    );
                    runtime
                        .persist_auth_transition_state(
                            UserStreamAuthState::Authenticated,
                            true,
                            UserStreamReasonCode::AuthRecoveredPendingEvent.code(),
                            &correlation_id,
                            &observed_at_utc,
                        )
                        .await?;
                }
                client
            }
            Err(error) => {
                let observed_at_utc = now_utc_rfc3339()?;
                let correlation_id = format!(
                    "user-stream-auth-expired::{}",
                    compact_timestamp_token(&observed_at_utc)
                );
                runtime
                    .persist_auth_transition_state(
                        UserStreamAuthState::AuthExpired,
                        true,
                        UserStreamReasonCode::AuthExpired.code(),
                        &correlation_id,
                        &observed_at_utc,
                    )
                    .await?;
                reconnect_attempts += 1;
                if reconnect_attempts > max_reconnect_attempts {
                    return Err(UserStreamIngestionError::new(
                        UserStreamReasonCode::AuthExpired.code(),
                        format!(
                            "unable to authenticate user websocket client after {max_reconnect_attempts} reconnect attempts: {error}"
                        ),
                    ));
                }
                sleep(reconnect_backoff).await;
                continue;
            }
        };

        let order_stream = match authenticated_client.subscribe_orders(markets.clone()) {
            Ok(stream) => stream,
            Err(error) => {
                let observed_at_utc = now_utc_rfc3339()?;
                let (reason_code, correlation_prefix) = if is_authentication_failure(&error) {
                    (
                        UserStreamReasonCode::AuthExpired.code(),
                        "user-stream-auth-expired",
                    )
                } else {
                    (
                        UserStreamReasonCode::StreamDisconnected.code(),
                        "user-stream-disconnect",
                    )
                };
                let correlation_id = format!(
                    "{correlation_prefix}::{}",
                    compact_timestamp_token(&observed_at_utc)
                );
                runtime
                    .persist_auth_transition_state(
                        UserStreamAuthState::AuthExpired,
                        true,
                        reason_code,
                        &correlation_id,
                        &observed_at_utc,
                    )
                    .await?;
                reconnect_attempts += 1;
                if reconnect_attempts > max_reconnect_attempts {
                    return Err(UserStreamIngestionError::new(
                        reason_code,
                        format!(
                            "unable to subscribe to authenticated order stream after {max_reconnect_attempts} reconnect attempts: {error}"
                        ),
                    ));
                }
                sleep(reconnect_backoff).await;
                continue;
            }
        };

        let trade_stream = match authenticated_client.subscribe_trades(markets.clone()) {
            Ok(stream) => stream,
            Err(error) => {
                let observed_at_utc = now_utc_rfc3339()?;
                let (reason_code, correlation_prefix) = if is_authentication_failure(&error) {
                    (
                        UserStreamReasonCode::AuthExpired.code(),
                        "user-stream-auth-expired",
                    )
                } else {
                    (
                        UserStreamReasonCode::StreamDisconnected.code(),
                        "user-stream-disconnect",
                    )
                };
                let correlation_id = format!(
                    "{correlation_prefix}::{}",
                    compact_timestamp_token(&observed_at_utc)
                );
                runtime
                    .persist_auth_transition_state(
                        UserStreamAuthState::AuthExpired,
                        true,
                        reason_code,
                        &correlation_id,
                        &observed_at_utc,
                    )
                    .await?;
                reconnect_attempts += 1;
                if reconnect_attempts > max_reconnect_attempts {
                    return Err(UserStreamIngestionError::new(
                        reason_code,
                        format!(
                            "unable to subscribe to authenticated trade stream after {max_reconnect_attempts} reconnect attempts: {error}"
                        ),
                    ));
                }
                sleep(reconnect_backoff).await;
                continue;
            }
        };

        tokio::pin!(order_stream);
        tokio::pin!(trade_stream);

        loop {
            tokio::select! {
                next_order = order_stream.next() => {
                    let observed_at_utc = now_utc_rfc3339()?;
                    match next_order {
                        Some(Ok(order_message)) => {
                            runtime.process_order_message(&order_message, &observed_at_utc).await?;
                            reconnect_attempts = 0;
                        }
                        Some(Err(error)) => {
                            let (reason_code, correlation_prefix) = if is_authentication_failure(&error) {
                                (UserStreamReasonCode::AuthExpired.code(), "user-stream-auth-expired")
                            } else {
                                (UserStreamReasonCode::StreamDisconnected.code(), "user-stream-disconnect")
                            };
                            let correlation_id = format!(
                                "{correlation_prefix}::{}",
                                compact_timestamp_token(&observed_at_utc)
                            );
                            runtime
                                .persist_auth_transition_state(
                                    UserStreamAuthState::AuthExpired,
                                    true,
                                    reason_code,
                                    &correlation_id,
                                    &observed_at_utc,
                                )
                                .await?;
                            emit_user_stream_telemetry(UserStreamTelemetryEvent {
                                event_name: "execution_user_stream_subscription_error_v1",
                                outcome: "deny",
                                reason_code,
                                correlation_id: &correlation_id,
                                timestamp_utc: &observed_at_utc,
                                latency_seconds: None,
                                block_new_intents: Some(true),
                            });
                            reconnect_attempts += 1;
                            if reconnect_attempts > max_reconnect_attempts {
                                return Err(UserStreamIngestionError::new(
                                    reason_code,
                                    format!(
                                        "authenticated order stream disconnected after {max_reconnect_attempts} reconnect attempts: {error}"
                                    ),
                                ));
                            }
                            sleep(reconnect_backoff).await;
                            continue 'reconnect;
                        }
                        None => {
                            let correlation_id = format!(
                                "user-stream-disconnect::{}",
                                compact_timestamp_token(&observed_at_utc)
                            );
                            runtime
                                .persist_auth_transition_state(
                                    UserStreamAuthState::AuthExpired,
                                    true,
                                    UserStreamReasonCode::StreamDisconnected.code(),
                                    &correlation_id,
                                    &observed_at_utc,
                                )
                                .await?;
                            reconnect_attempts += 1;
                            if reconnect_attempts > max_reconnect_attempts {
                                return Err(UserStreamIngestionError::new(
                                    UserStreamReasonCode::StreamDisconnected.code(),
                                    format!(
                                        "authenticated order stream terminated unexpectedly after {max_reconnect_attempts} reconnect attempts"
                                    ),
                                ));
                            }
                            sleep(reconnect_backoff).await;
                            continue 'reconnect;
                        }
                    }
                }
                next_trade = trade_stream.next() => {
                    let observed_at_utc = now_utc_rfc3339()?;
                    match next_trade {
                        Some(Ok(trade_message)) => {
                            runtime.process_trade_message(&trade_message, &observed_at_utc).await?;
                            reconnect_attempts = 0;
                        }
                        Some(Err(error)) => {
                            let (reason_code, correlation_prefix) = if is_authentication_failure(&error) {
                                (UserStreamReasonCode::AuthExpired.code(), "user-stream-auth-expired")
                            } else {
                                (UserStreamReasonCode::StreamDisconnected.code(), "user-stream-disconnect")
                            };
                            let correlation_id = format!(
                                "{correlation_prefix}::{}",
                                compact_timestamp_token(&observed_at_utc)
                            );
                            runtime
                                .persist_auth_transition_state(
                                    UserStreamAuthState::AuthExpired,
                                    true,
                                    reason_code,
                                    &correlation_id,
                                    &observed_at_utc,
                                )
                                .await?;
                            emit_user_stream_telemetry(UserStreamTelemetryEvent {
                                event_name: "execution_user_stream_subscription_error_v1",
                                outcome: "deny",
                                reason_code,
                                correlation_id: &correlation_id,
                                timestamp_utc: &observed_at_utc,
                                latency_seconds: None,
                                block_new_intents: Some(true),
                            });
                            reconnect_attempts += 1;
                            if reconnect_attempts > max_reconnect_attempts {
                                return Err(UserStreamIngestionError::new(
                                    reason_code,
                                    format!(
                                        "authenticated trade stream disconnected after {max_reconnect_attempts} reconnect attempts: {error}"
                                    ),
                                ));
                            }
                            sleep(reconnect_backoff).await;
                            continue 'reconnect;
                        }
                        None => {
                            let correlation_id = format!(
                                "user-stream-disconnect::{}",
                                compact_timestamp_token(&observed_at_utc)
                            );
                            runtime
                                .persist_auth_transition_state(
                                    UserStreamAuthState::AuthExpired,
                                    true,
                                    UserStreamReasonCode::StreamDisconnected.code(),
                                    &correlation_id,
                                    &observed_at_utc,
                                )
                                .await?;
                            reconnect_attempts += 1;
                            if reconnect_attempts > max_reconnect_attempts {
                                return Err(UserStreamIngestionError::new(
                                    UserStreamReasonCode::StreamDisconnected.code(),
                                    format!(
                                        "authenticated trade stream terminated unexpectedly after {max_reconnect_attempts} reconnect attempts"
                                    ),
                                ));
                            }
                            sleep(reconnect_backoff).await;
                            continue 'reconnect;
                        }
                    }
                }
            }
        }
    }
}

fn parse_credentials(
    config: &UserStreamRuntimeConfig,
) -> Result<Credentials, UserStreamIngestionError> {
    let api_key = Uuid::parse_str(config.api_key.trim()).map_err(|error| {
        UserStreamIngestionError::new(
            UserStreamReasonCode::InvalidPayload.code(),
            format!("EXECUTION_POLYMARKET_API_KEY must be a UUID: {error}"),
        )
    })?;
    Ok(Credentials::new(
        api_key,
        config.api_secret.clone(),
        config.api_passphrase.clone(),
    ))
}

fn parse_address(value: &str) -> Result<Address, UserStreamIngestionError> {
    Address::from_str(value.trim()).map_err(|error| {
        UserStreamIngestionError::new(
            UserStreamReasonCode::InvalidPayload.code(),
            format!("EXECUTION_POLYMARKET_ADDRESS must be a valid address: {error}"),
        )
    })
}

fn parse_market_targets(market_ids: &[String]) -> Result<Vec<B256>, UserStreamIngestionError> {
    market_ids
        .iter()
        .map(|market_id| {
            B256::from_str(market_id).map_err(|error| {
                UserStreamIngestionError::new(
                    UserStreamReasonCode::InvalidPayload.code(),
                    format!("invalid user stream market id `{market_id}`: {error}"),
                )
            })
        })
        .collect()
}

fn derive_order_event_status(
    order: &OrderMessage,
) -> Result<UserStreamEventStatus, UserStreamIngestionError> {
    if let Some(message_type) = &order.msg_type {
        return match message_type {
            OrderMessageType::Placement => Ok(UserStreamEventStatus::Placement),
            OrderMessageType::Update => Ok(UserStreamEventStatus::Update),
            OrderMessageType::Cancellation => Ok(UserStreamEventStatus::Cancellation),
            OrderMessageType::Unknown(value) => Err(UserStreamIngestionError::new(
                UserStreamReasonCode::InvalidPayload.code(),
                format!("unsupported order message type `{value}`"),
            )),
            _ => Err(UserStreamIngestionError::new(
                UserStreamReasonCode::InvalidPayload.code(),
                "unsupported non-exhaustive order message type",
            )),
        };
    }

    match &order.status {
        Some(OrderStatusType::Live | OrderStatusType::Unmatched) => {
            Ok(UserStreamEventStatus::Placement)
        }
        Some(OrderStatusType::Matched) => Ok(UserStreamEventStatus::Matched),
        Some(OrderStatusType::Canceled) => Ok(UserStreamEventStatus::Cancellation),
        Some(OrderStatusType::Delayed) => Ok(UserStreamEventStatus::Update),
        Some(OrderStatusType::Unknown(value)) => Err(UserStreamIngestionError::new(
            UserStreamReasonCode::InvalidPayload.code(),
            format!("unsupported order status `{value}`"),
        )),
        Some(_) => Err(UserStreamIngestionError::new(
            UserStreamReasonCode::InvalidPayload.code(),
            "unsupported non-exhaustive order status",
        )),
        None => Err(UserStreamIngestionError::new(
            UserStreamReasonCode::InvalidPayload.code(),
            "order event requires either message type or status",
        )),
    }
}

fn derive_trade_event_status(
    trade: &TradeMessage,
) -> Result<UserStreamEventStatus, UserStreamIngestionError> {
    match &trade.status {
        TradeMessageStatus::Matched => Ok(UserStreamEventStatus::Matched),
        TradeMessageStatus::Mined => Ok(UserStreamEventStatus::Mined),
        TradeMessageStatus::Confirmed => Ok(UserStreamEventStatus::Confirmed),
        TradeMessageStatus::Unknown(value) => Err(UserStreamIngestionError::new(
            UserStreamReasonCode::InvalidPayload.code(),
            format!("unsupported trade status `{value}`"),
        )),
        _ => Err(UserStreamIngestionError::new(
            UserStreamReasonCode::InvalidPayload.code(),
            "unsupported non-exhaustive trade status",
        )),
    }
}

fn derive_trade_partition_key(trade: &TradeMessage) -> (String, String) {
    if let Some(order_id) = trade
        .taker_order_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        let normalized = normalize_user_stream_idempotency_key(order_id);
        return (order_id.to_string(), normalized);
    }
    if let Some(order_id) = trade
        .maker_orders
        .first()
        .map(|value| value.order_id.as_str())
        .filter(|value| !value.trim().is_empty())
    {
        let normalized = normalize_user_stream_idempotency_key(order_id);
        return (order_id.to_string(), normalized);
    }
    let normalized = normalize_user_stream_idempotency_key(&trade.id);
    (trade.id.clone(), normalized)
}

fn format_order_message_type(message_type: &Option<OrderMessageType>) -> String {
    match message_type {
        Some(OrderMessageType::Placement) => "placement".to_string(),
        Some(OrderMessageType::Update) => "update".to_string(),
        Some(OrderMessageType::Cancellation) => "cancellation".to_string(),
        Some(OrderMessageType::Unknown(value)) => value.trim().to_ascii_lowercase(),
        Some(_) => "unknown".to_string(),
        None => "unknown".to_string(),
    }
}

fn format_order_status(status: &Option<OrderStatusType>) -> String {
    match status {
        Some(OrderStatusType::Live) => "live".to_string(),
        Some(OrderStatusType::Matched) => "matched".to_string(),
        Some(OrderStatusType::Canceled) => "canceled".to_string(),
        Some(OrderStatusType::Delayed) => "delayed".to_string(),
        Some(OrderStatusType::Unmatched) => "unmatched".to_string(),
        Some(OrderStatusType::Unknown(value)) => value.trim().to_ascii_lowercase(),
        Some(_) => "unknown".to_string(),
        None => "unknown".to_string(),
    }
}

fn is_authentication_failure(error: &polymarket_client_sdk::error::Error) -> bool {
    if error
        .downcast_ref::<WsError>()
        .is_some_and(|ws_error| matches!(ws_error, WsError::AuthenticationFailed))
    {
        return true;
    }
    error.to_string().to_ascii_lowercase().contains("auth")
}

fn required_env(key: &str) -> Result<String, UserStreamIngestionError> {
    let value = std::env::var(key).map_err(|_| {
        UserStreamIngestionError::new(
            UserStreamReasonCode::InvalidPayload.code(),
            format!("{key} must be set"),
        )
    })?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(UserStreamIngestionError::new(
            UserStreamReasonCode::InvalidPayload.code(),
            format!("{key} cannot be blank"),
        ));
    }
    Ok(trimmed.to_string())
}

fn parse_positive_f64(key: &str, value: &str) -> Result<f64, UserStreamIngestionError> {
    let parsed = value.parse::<f64>().map_err(|_| {
        UserStreamIngestionError::new(
            UserStreamReasonCode::InvalidPayload.code(),
            format!("{key} must be a floating-point number"),
        )
    })?;
    if !parsed.is_finite() || parsed <= 0.0 {
        return Err(UserStreamIngestionError::new(
            UserStreamReasonCode::InvalidPayload.code(),
            format!("{key} must be finite and greater than 0"),
        ));
    }
    Ok(parsed)
}

fn parse_positive_u32(key: &str, value: &str) -> Result<u32, UserStreamIngestionError> {
    let parsed = value.parse::<u32>().map_err(|_| {
        UserStreamIngestionError::new(
            UserStreamReasonCode::InvalidPayload.code(),
            format!("{key} must be an integer"),
        )
    })?;
    if parsed == 0 {
        return Err(UserStreamIngestionError::new(
            UserStreamReasonCode::InvalidPayload.code(),
            format!("{key} must be greater than 0"),
        ));
    }
    Ok(parsed)
}

fn timestamp_millis_to_rfc3339_utc(
    timestamp_millis: i64,
) -> Result<String, UserStreamIngestionError> {
    let nanos = (timestamp_millis as i128)
        .checked_mul(1_000_000)
        .ok_or_else(|| {
            UserStreamIngestionError::new(
                UserStreamReasonCode::InvalidPayload.code(),
                "user stream timestamp overflow",
            )
        })?;
    let observed = OffsetDateTime::from_unix_timestamp_nanos(nanos).map_err(|error| {
        UserStreamIngestionError::new(
            UserStreamReasonCode::InvalidPayload.code(),
            format!("invalid user stream timestamp: {error}"),
        )
    })?;
    observed.format(&Rfc3339).map_err(|error| {
        UserStreamIngestionError::new(
            UserStreamReasonCode::InvalidPayload.code(),
            format!("unable to format user stream timestamp: {error}"),
        )
    })
}

fn parse_rfc3339_utc(value: &str) -> Result<OffsetDateTime, UserStreamIngestionError> {
    let parsed = OffsetDateTime::parse(value, &Rfc3339).map_err(|error| {
        UserStreamIngestionError::new(
            UserStreamReasonCode::InvalidPayload.code(),
            format!("invalid RFC3339 timestamp `{value}`: {error}"),
        )
    })?;
    if parsed.offset() != time::UtcOffset::UTC {
        return Err(UserStreamIngestionError::new(
            UserStreamReasonCode::InvalidPayload.code(),
            "timestamps must use UTC `Z` offset",
        ));
    }
    Ok(parsed)
}

fn now_utc_rfc3339() -> Result<String, UserStreamIngestionError> {
    OffsetDateTime::now_utc().format(&Rfc3339).map_err(|error| {
        UserStreamIngestionError::new(
            UserStreamReasonCode::InvalidPayload.code(),
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

fn emit_user_stream_telemetry(event: UserStreamTelemetryEvent<'_>) {
    println!(
        "{}",
        serde_json::to_string(&event).expect("user stream telemetry should serialize")
    );
}

#[derive(Debug, Serialize)]
struct UserStreamTelemetryEvent<'a> {
    event_name: &'a str,
    outcome: &'a str,
    reason_code: &'a str,
    correlation_id: &'a str,
    timestamp_utc: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    latency_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    block_new_intents: Option<bool>,
}

#[cfg(test)]
#[derive(Debug, Clone, Default)]
pub struct InMemoryUserStreamStore {
    state: Arc<Mutex<InMemoryUserStreamState>>,
}

#[cfg(test)]
#[derive(Debug, Default)]
struct InMemoryUserStreamState {
    events: Vec<UserStreamEvent>,
    cursors: BTreeMap<String, OrderEventOffsetCursor>,
    auth_cursor: Option<OrderEventOffsetCursor>,
}

#[cfg(test)]
impl InMemoryUserStreamStore {
    pub fn events(&self) -> Vec<UserStreamEvent> {
        self.state
            .lock()
            .expect("user stream in-memory state should not be poisoned")
            .events
            .clone()
    }

    pub fn cursor_for_partition(&self, partition_key: &str) -> Option<OrderEventOffsetCursor> {
        self.state
            .lock()
            .expect("user stream in-memory state should not be poisoned")
            .cursors
            .get(&normalize_user_stream_idempotency_key(partition_key))
            .cloned()
    }

    pub fn auth_cursor(&self) -> Option<OrderEventOffsetCursor> {
        self.state
            .lock()
            .expect("user stream in-memory state should not be poisoned")
            .auth_cursor
            .clone()
    }
}

#[cfg(test)]
impl UserStreamStore for InMemoryUserStreamStore {
    async fn persist_event(
        &self,
        event: &UserStreamEvent,
        _raw_payload: &serde_json::Value,
        auth_state: UserStreamAuthState,
        block_new_intents: bool,
    ) -> Result<UserStreamPersistOutcome, UserStreamIngestionError> {
        let mut state = self.state.lock().map_err(|_| {
            UserStreamIngestionError::new(
                UserStreamReasonCode::PersistenceUnavailable.code(),
                "in-memory user stream state is poisoned",
            )
        })?;
        let partition_key = normalize_user_stream_idempotency_key(&event.partition_key);
        let cursor = state.cursors.get(&partition_key).cloned();
        let decision = domain::risk::evaluate_user_stream_ordering(cursor.as_ref(), event)
            .map_err(|error| UserStreamIngestionError::new(error.code, error.message))?;

        match decision {
            domain::risk::UserStreamOrderingDecision::Accept { reason_code } => {
                let normalized_key = normalize_user_stream_idempotency_key(&event.idempotency_key);
                state.events.push(event.clone());
                state.cursors.insert(
                    partition_key,
                    OrderEventOffsetCursor {
                        partition_key: normalize_user_stream_idempotency_key(&event.partition_key),
                        last_event_id: event.event_id.clone(),
                        last_event_key: normalized_key,
                        last_event_offset: event.event_offset,
                        auth_state,
                        block_new_intents,
                        reason_code: reason_code.to_string(),
                        correlation_id: event.correlation_id.clone(),
                        updated_at_utc: event.ingested_at_utc.clone(),
                    },
                );
                Ok(UserStreamPersistOutcome {
                    disposition: UserStreamPersistDisposition::Accepted,
                    reason_code: reason_code.to_string(),
                })
            }
            domain::risk::UserStreamOrderingDecision::Duplicate { reason_code } => {
                Ok(UserStreamPersistOutcome {
                    disposition: UserStreamPersistDisposition::Duplicate,
                    reason_code: reason_code.to_string(),
                })
            }
            domain::risk::UserStreamOrderingDecision::OutOfOrder { reason_code } => {
                Ok(UserStreamPersistOutcome {
                    disposition: UserStreamPersistDisposition::OutOfOrder,
                    reason_code: reason_code.to_string(),
                })
            }
        }
    }

    async fn persist_auth_transition(
        &self,
        transition: &UserStreamAuthTransition,
    ) -> Result<(), UserStreamIngestionError> {
        domain::risk::validate_user_stream_auth_transition(transition)
            .map_err(|error| UserStreamIngestionError::new(error.code, error.message))?;
        let mut state = self.state.lock().map_err(|_| {
            UserStreamIngestionError::new(
                UserStreamReasonCode::PersistenceUnavailable.code(),
                "in-memory user stream state is poisoned",
            )
        })?;
        state.auth_cursor = Some(OrderEventOffsetCursor {
            partition_key: USER_STREAM_AUTH_STATE_PARTITION_KEY.to_string(),
            last_event_id: transition.transition_id.clone(),
            last_event_key: "auth_state_transition".to_string(),
            last_event_offset: transition.transition_offset,
            auth_state: transition.auth_state,
            block_new_intents: transition.block_new_intents,
            reason_code: transition.reason_code.clone(),
            correlation_id: transition.correlation_id.clone(),
            updated_at_utc: transition.observed_at_utc.clone(),
        });
        Ok(())
    }

    async fn load_auth_cursor(
        &self,
    ) -> Result<Option<OrderEventOffsetCursor>, UserStreamIngestionError> {
        Ok(self.auth_cursor())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_config() -> UserStreamRuntimeConfig {
        UserStreamRuntimeConfig {
            stream_name: "polymarket_user_stream".to_string(),
            stream_endpoint: "wss://ws-subscriptions-clob.polymarket.com".to_string(),
            market_ids: vec![
                "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            ],
            api_key: "8ed27e94-3fcd-43ea-a95f-bf5945f18847".to_string(),
            api_secret: "secret".to_string(),
            api_passphrase: "passphrase".to_string(),
            address: "0x0000000000000000000000000000000000000001".to_string(),
            reconnect_backoff_seconds: 1.0,
            max_reconnect_attempts: 5,
        }
    }

    fn sample_order_message(order_id: &str, timestamp_millis: i64) -> OrderMessage {
        serde_json::from_value(json!({
            "event_type": "order",
            "id": order_id,
            "market": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "asset_id": "106585164761922456203746651621390029417453862034640469075081961934906147433548",
            "side": "BUY",
            "price": "0.53",
            "type": "PLACEMENT",
            "status": "LIVE",
            "timestamp": timestamp_millis.to_string(),
            "original_size": "100.0",
            "size_matched": "0.0"
        }))
        .expect("order message fixture should deserialize")
    }

    fn sample_order_message_with_status_only(
        order_id: &str,
        timestamp_millis: i64,
        status: &str,
    ) -> OrderMessage {
        serde_json::from_value(json!({
            "event_type": "order",
            "id": order_id,
            "market": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "asset_id": "106585164761922456203746651621390029417453862034640469075081961934906147433548",
            "side": "BUY",
            "price": "0.53",
            "status": status,
            "timestamp": timestamp_millis.to_string(),
            "original_size": "100.0",
            "size_matched": "0.0"
        }))
        .expect("status-only order fixture should deserialize")
    }

    fn rfc3339_from_millis(timestamp_millis: i64) -> String {
        OffsetDateTime::from_unix_timestamp_nanos((timestamp_millis as i128) * 1_000_000)
            .expect("timestamp should parse")
            .format(&Rfc3339)
            .expect("timestamp should format")
    }

    #[tokio::test]
    async fn normal_load_path_meets_user_stream_latency_target_boundary() {
        let store = InMemoryUserStreamStore::default();
        let mut runtime = UserStreamIngestionRuntime::new(store.clone(), sample_config());
        let start = 1_775_404_800_000_i64;

        for index in 0..100 {
            let observed = start + (index * 1_000);
            let ingested = observed + 1_500;
            let order_id = format!("order-{index}");
            let outcome = runtime
                .process_order_message(
                    &sample_order_message(&order_id, observed),
                    &rfc3339_from_millis(ingested),
                )
                .await
                .expect("normal-load event should ingest successfully");
            assert_eq!(outcome.disposition, UserIngestionDisposition::Accepted);
        }

        assert!(runtime.latency_slo_met());
        assert_eq!(store.events().len(), 100);
    }

    #[tokio::test]
    async fn duplicates_and_out_of_order_events_are_ignored_without_state_regression() {
        let store = InMemoryUserStreamStore::default();
        let mut runtime = UserStreamIngestionRuntime::new(store.clone(), sample_config());
        let start = 1_775_404_800_000_i64;

        let accepted = runtime
            .process_order_message(
                &sample_order_message("order-1", start),
                &rfc3339_from_millis(start + 1_100),
            )
            .await
            .expect("initial order event should be accepted");
        assert_eq!(accepted.disposition, UserIngestionDisposition::Accepted);
        assert_eq!(
            accepted.reason_code,
            UserStreamReasonCode::EventAccepted.code()
        );
        assert_eq!(
            accepted.correlation_id,
            format!("user-stream::order::order-1::{start}")
        );

        let duplicate = runtime
            .process_order_message(
                &sample_order_message("order-1", start),
                &rfc3339_from_millis(start + 1_200),
            )
            .await
            .expect("duplicate event should be ignored");
        assert_eq!(duplicate.disposition, UserIngestionDisposition::Duplicate);
        assert_eq!(
            duplicate.reason_code,
            UserStreamReasonCode::DuplicateEvent.code()
        );
        assert_eq!(duplicate.correlation_id, accepted.correlation_id);

        let out_of_order = runtime
            .process_order_message(
                &sample_order_message("order-1", start - 1_000),
                &rfc3339_from_millis(start + 1_300),
            )
            .await
            .expect("out-of-order event should be rejected");
        assert_eq!(
            out_of_order.disposition,
            UserIngestionDisposition::OutOfOrder
        );
        assert_eq!(
            out_of_order.reason_code,
            UserStreamReasonCode::OutOfOrderEvent.code()
        );
        assert_eq!(
            out_of_order.correlation_id,
            format!("user-stream::order::order-1::{}", start - 1_000)
        );

        let second_accepted = runtime
            .process_order_message(
                &sample_order_message("order-1", start + 2_000),
                &rfc3339_from_millis(start + 3_200),
            )
            .await
            .expect("newer event should be accepted");
        assert_eq!(
            second_accepted.disposition,
            UserIngestionDisposition::Accepted
        );
        assert_eq!(
            second_accepted.reason_code,
            UserStreamReasonCode::EventAccepted.code()
        );
        assert_eq!(
            second_accepted.correlation_id,
            format!("user-stream::order::order-1::{}", start + 2_000)
        );

        let events = store.events();
        assert_eq!(events.len(), 2);
        let cursor = store
            .cursor_for_partition("order-1")
            .expect("cursor should exist after accepted events");
        assert_eq!(cursor.last_event_offset, start + 2_000);
        assert_eq!(
            cursor.reason_code,
            UserStreamReasonCode::EventAccepted.code()
        );
        assert_eq!(cursor.correlation_id, second_accepted.correlation_id);
        assert_eq!(cursor.updated_at_utc, rfc3339_from_millis(start + 3_200));
    }

    #[tokio::test]
    async fn status_only_order_messages_cover_matched_and_cancellation_paths() {
        let store = InMemoryUserStreamStore::default();
        let mut runtime = UserStreamIngestionRuntime::new(store.clone(), sample_config());
        let start = 1_775_404_900_000_i64;

        let matched = runtime
            .process_order_message(
                &sample_order_message_with_status_only("order-status", start, "MATCHED"),
                &rfc3339_from_millis(start + 1_100),
            )
            .await
            .expect("matched status should map to accepted fill event");
        assert_eq!(matched.disposition, UserIngestionDisposition::Accepted);
        assert_eq!(
            matched.reason_code,
            UserStreamReasonCode::EventAccepted.code()
        );

        let cancellation_offset = start + 1_000;
        let cancellation = runtime
            .process_order_message(
                &sample_order_message_with_status_only(
                    "order-status",
                    cancellation_offset,
                    "CANCELED",
                ),
                &rfc3339_from_millis(cancellation_offset + 1_200),
            )
            .await
            .expect("canceled status should map to accepted cancellation event");
        assert_eq!(cancellation.disposition, UserIngestionDisposition::Accepted);
        assert_eq!(
            cancellation.reason_code,
            UserStreamReasonCode::EventAccepted.code()
        );

        let events = store.events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_status, UserStreamEventStatus::Matched);
        assert_eq!(events[1].event_status, UserStreamEventStatus::Cancellation);
    }

    #[tokio::test]
    async fn process_order_message_rejects_non_utc_ingest_timestamp() {
        let store = InMemoryUserStreamStore::default();
        let mut runtime = UserStreamIngestionRuntime::new(store, sample_config());
        let error = runtime
            .process_order_message(
                &sample_order_message("order-non-utc", 1_775_404_800_000_i64),
                "2026-04-06T00:00:01+01:00",
            )
            .await
            .expect_err("non-UTC ingest timestamp should fail closed");
        assert_eq!(error.code, UserStreamReasonCode::InvalidPayload.code());
        assert!(error.message.contains("timestamps must use UTC `Z` offset"));
    }

    #[tokio::test]
    async fn auth_expiry_blocks_intents_until_first_recovery_event_is_persisted() {
        let store = InMemoryUserStreamStore::default();
        let mut runtime = UserStreamIngestionRuntime::new(store.clone(), sample_config());
        let first_timestamp = "2026-04-06T00:00:01Z";

        runtime
            .persist_auth_transition_state(
                UserStreamAuthState::AuthExpired,
                true,
                UserStreamReasonCode::AuthExpired.code(),
                "corr-auth-expired-1",
                first_timestamp,
            )
            .await
            .expect("auth-expired transition should persist");
        assert!(runtime.block_new_intents());

        runtime
            .persist_auth_transition_state(
                UserStreamAuthState::Authenticated,
                true,
                UserStreamReasonCode::AuthRecoveredPendingEvent.code(),
                "corr-auth-recovered-1",
                "2026-04-06T00:00:02Z",
            )
            .await
            .expect("auth-recovered pending transition should persist");
        assert!(runtime.block_new_intents());

        let start = 1_775_404_800_000_i64;
        let outcome = runtime
            .process_order_message(
                &sample_order_message("order-recovery", start),
                &rfc3339_from_millis(start + 1_200),
            )
            .await
            .expect("first post-recovery event should persist");
        assert_eq!(outcome.disposition, UserIngestionDisposition::Accepted);
        assert!(!runtime.block_new_intents());

        let auth_cursor = store
            .auth_cursor()
            .expect("auth cursor should be present after transitions");
        assert_eq!(auth_cursor.auth_state, UserStreamAuthState::Authenticated);
        assert!(!auth_cursor.block_new_intents);
        assert_eq!(
            auth_cursor.reason_code,
            UserStreamReasonCode::Authenticated.code()
        );
    }
}
