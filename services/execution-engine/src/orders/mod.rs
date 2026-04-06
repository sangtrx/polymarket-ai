#![cfg_attr(not(test), allow(dead_code))]

use domain::order::{
    OrderLifecycleReasonCode, OrderLifecycleState, OrderMode, OrderRecord, OrderStateTransition,
    normalize_order_batch_cancel_idempotency_key, normalize_order_cancel_idempotency_key,
    normalize_order_idempotency_key, normalize_order_submission_idempotency_key,
};
use domain::risk::{
    PreTradeDecisionOutcome, PreTradeReasonCode, UserStreamEvent, UserStreamEventStatus,
};
use persistence::postgres::orders::{
    OrderLifecyclePersistDisposition, OrderLifecyclePersistOutcome, OrderLifecyclePersistenceError,
    OrderLifecycleTransitionCommand, load_order, load_order_state_transitions,
    persist_order_transition,
};
use persistence::postgres::pretrade_gate::{
    PreTradeGatePersistenceError, load_latest_pretrade_gate_decision_by_intent,
};
use serde::Serialize;
use sqlx::PgPool;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

#[cfg(test)]
use domain::order::{
    validate_order_record, validate_order_state_transition, validate_order_transition,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderLifecycleRuntimeError {
    pub code: &'static str,
    pub message: String,
}

impl OrderLifecycleRuntimeError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl Display for OrderLifecycleRuntimeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for OrderLifecycleRuntimeError {}

pub const DEFAULT_PRETRADE_ADJUDICATION_TIMEOUT_MS: u64 = 1_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreTradeAdjudicationRequest {
    pub order_id: String,
    pub market_id: String,
    pub correlation_id: String,
    pub requested_at_utc: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreTradeAdjudicationDecision {
    pub allowed: bool,
    pub reason_code: String,
    pub decided_at_utc: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreTradeAdjudicationPortError {
    pub code: &'static str,
    pub message: String,
}

impl PreTradeAdjudicationPortError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

pub trait RiskAdjudicationPort: Send + Sync {
    fn adjudicate_submit<'a>(
        &'a self,
        request: &'a PreTradeAdjudicationRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<PreTradeAdjudicationDecision, PreTradeAdjudicationPortError>>
                + Send
                + 'a,
        >,
    >;
}

#[derive(Debug, Default)]
pub struct AllowAllRiskAdjudicationPort;

impl RiskAdjudicationPort for AllowAllRiskAdjudicationPort {
    fn adjudicate_submit<'a>(
        &'a self,
        request: &'a PreTradeAdjudicationRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<PreTradeAdjudicationDecision, PreTradeAdjudicationPortError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            Ok(PreTradeAdjudicationDecision {
                allowed: true,
                reason_code: PreTradeReasonCode::Pass.code().to_string(),
                decided_at_utc: request.requested_at_utc.clone(),
            })
        })
    }
}

#[derive(Debug, Default)]
pub struct UnavailableRiskAdjudicationPort;

impl RiskAdjudicationPort for UnavailableRiskAdjudicationPort {
    fn adjudicate_submit<'a>(
        &'a self,
        _request: &'a PreTradeAdjudicationRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<PreTradeAdjudicationDecision, PreTradeAdjudicationPortError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async {
            Err(PreTradeAdjudicationPortError::new(
                PreTradeReasonCode::AdjudicationUnavailable.code(),
                "pre-trade adjudication port is unavailable",
            ))
        })
    }
}

#[derive(Clone)]
pub struct PostgresRiskAdjudicationPort {
    pool: PgPool,
}

impl PostgresRiskAdjudicationPort {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl RiskAdjudicationPort for PostgresRiskAdjudicationPort {
    fn adjudicate_submit<'a>(
        &'a self,
        request: &'a PreTradeAdjudicationRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<PreTradeAdjudicationDecision, PreTradeAdjudicationPortError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let persisted_decision =
                load_latest_pretrade_gate_decision_by_intent(&self.pool, &request.order_id)
                    .await
                    .map_err(map_pretrade_lookup_error_to_adjudication_error)?
                    .ok_or_else(|| {
                        PreTradeAdjudicationPortError::new(
                            PreTradeReasonCode::AdjudicationUnavailable.code(),
                            format!(
                                "no persisted pre-trade decision found for intent `{}`",
                                request.order_id.trim()
                            ),
                        )
                    })?;

            Ok(PreTradeAdjudicationDecision {
                allowed: matches!(persisted_decision.outcome, PreTradeDecisionOutcome::Allow),
                reason_code: persisted_decision.reason_code,
                decided_at_utc: persisted_decision.evaluated_at_utc,
            })
        })
    }
}

pub trait OrderLifecycleStore: Send + Sync {
    fn persist_transition<'a>(
        &'a self,
        command: &'a OrderLifecycleTransitionCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<OrderLifecyclePersistOutcome, OrderLifecycleRuntimeError>>
                + Send
                + 'a,
        >,
    >;

    fn load_order<'a>(
        &'a self,
        order_id: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Option<OrderRecord>, OrderLifecycleRuntimeError>>
                + Send
                + 'a,
        >,
    >;

    fn load_order_state_transitions<'a>(
        &'a self,
        order_id: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<OrderStateTransition>, OrderLifecycleRuntimeError>>
                + Send
                + 'a,
        >,
    >;
}

#[derive(Clone)]
pub struct PostgresOrderLifecycleStore {
    pool: PgPool,
}

impl PostgresOrderLifecycleStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl OrderLifecycleStore for PostgresOrderLifecycleStore {
    fn persist_transition<'a>(
        &'a self,
        command: &'a OrderLifecycleTransitionCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<OrderLifecyclePersistOutcome, OrderLifecycleRuntimeError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            persist_order_transition(&self.pool, command)
                .await
                .map_err(map_persistence_error)
        })
    }

    fn load_order<'a>(
        &'a self,
        order_id: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Option<OrderRecord>, OrderLifecycleRuntimeError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            load_order(&self.pool, order_id)
                .await
                .map_err(map_persistence_error)
        })
    }

    fn load_order_state_transitions<'a>(
        &'a self,
        order_id: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<OrderStateTransition>, OrderLifecycleRuntimeError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            load_order_state_transitions(&self.pool, order_id)
                .await
                .map_err(map_persistence_error)
        })
    }
}

fn map_persistence_error(error: OrderLifecyclePersistenceError) -> OrderLifecycleRuntimeError {
    OrderLifecycleRuntimeError::new(error.code, error.to_string())
}

fn map_pretrade_lookup_error_to_adjudication_error(
    error: PreTradeGatePersistenceError,
) -> PreTradeAdjudicationPortError {
    let normalized_code = PreTradeReasonCode::parse(error.code)
        .map(|reason| reason.code())
        .unwrap_or(PreTradeReasonCode::AdjudicationUnavailable.code());
    PreTradeAdjudicationPortError::new(normalized_code, error.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderLifecycleCommandDisposition {
    Applied,
    Duplicate,
    Rejected,
    AlreadyTerminal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderLifecycleCommandOutcome {
    pub disposition: OrderLifecycleCommandDisposition,
    pub reason_code: String,
    pub order_id: String,
    pub market_id: String,
    pub idempotency_key: String,
    pub from_state: Option<OrderLifecycleState>,
    pub to_state: Option<OrderLifecycleState>,
    pub correlation_id: String,
    pub timestamp_utc: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitOrderCommand {
    pub order_id: String,
    pub market_id: String,
    pub mode: OrderMode,
    pub idempotency_key: String,
    pub correlation_id: String,
    pub requested_at_utc: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CancelOrderCommand {
    pub order_id: String,
    pub market_id: String,
    pub mode: OrderMode,
    pub idempotency_key: String,
    pub correlation_id: String,
    pub requested_at_utc: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchCancelCommand {
    pub market_id: String,
    pub correlation_id: String,
    pub requested_at_utc: String,
    pub batch_idempotency_key: String,
    pub orders: Vec<BatchCancelOrderRequest>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchCancelOrderRequest {
    pub order_id: String,
    pub mode: OrderMode,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchCancelOutcomeKind {
    Canceled,
    AlreadyTerminal,
    RetryableFailure,
    HardFailure,
}

impl BatchCancelOutcomeKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Canceled => "canceled",
            Self::AlreadyTerminal => "already_terminal",
            Self::RetryableFailure => "retryable_failure",
            Self::HardFailure => "hard_failure",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchCancelOrderOutcome {
    pub order_id: String,
    pub market_id: String,
    pub outcome: BatchCancelOutcomeKind,
    pub reason_code: String,
    pub idempotency_key: String,
    pub correlation_id: String,
    pub timestamp_utc: String,
}

pub trait OrderLifecycleUpdatePort: Send + Sync {
    fn apply_user_stream_event<'a>(
        &'a self,
        event: &'a UserStreamEvent,
    ) -> Pin<Box<dyn Future<Output = Result<(), OrderLifecycleRuntimeError>> + Send + 'a>>;
}

pub struct OrderLifecycleRuntime<S: OrderLifecycleStore> {
    store: S,
    risk_adjudication_port: Arc<dyn RiskAdjudicationPort>,
    pretrade_adjudication_timeout: Duration,
}

impl<S: OrderLifecycleStore> OrderLifecycleRuntime<S> {
    pub fn new(store: S) -> Self {
        Self {
            store,
            risk_adjudication_port: Arc::new(UnavailableRiskAdjudicationPort),
            pretrade_adjudication_timeout: Duration::from_millis(
                DEFAULT_PRETRADE_ADJUDICATION_TIMEOUT_MS,
            ),
        }
    }

    pub fn with_risk_adjudication_port(
        store: S,
        risk_adjudication_port: Arc<dyn RiskAdjudicationPort>,
        pretrade_adjudication_timeout: Duration,
    ) -> Self {
        Self {
            store,
            risk_adjudication_port,
            pretrade_adjudication_timeout,
        }
    }

    pub async fn submit_order(
        &self,
        command: SubmitOrderCommand,
    ) -> Result<OrderLifecycleCommandOutcome, OrderLifecycleRuntimeError> {
        validate_runtime_command_fields(
            &command.order_id,
            &command.market_id,
            &command.correlation_id,
            &command.requested_at_utc,
        )?;
        let adjudication_request = PreTradeAdjudicationRequest {
            order_id: command.order_id.clone(),
            market_id: command.market_id.clone(),
            correlation_id: command.correlation_id.clone(),
            requested_at_utc: command.requested_at_utc.clone(),
        };
        let adjudication_decision = match self
            .adjudicate_submit_order_request(&adjudication_request)
            .await
        {
            Ok(decision) => decision,
            Err(error) => {
                let reason_code = normalize_pretrade_reason_code(error.code);
                emit_pretrade_submit_deny_telemetry(&adjudication_request, reason_code);
                return Err(OrderLifecycleRuntimeError::new(reason_code, error.message));
            }
        };
        if !adjudication_decision.allowed {
            let reason_code = normalize_pretrade_reason_code(&adjudication_decision.reason_code);
            emit_pretrade_submit_deny_telemetry(&adjudication_request, reason_code);
            return Err(OrderLifecycleRuntimeError::new(
                reason_code,
                format!(
                    "pre-trade adjudication denied submit for `{}` with reason `{}`",
                    adjudication_request.order_id, reason_code
                ),
            ));
        }

        let idempotency_key =
            normalize_order_submission_idempotency_key(&command.order_id, &command.idempotency_key);
        let transition_command = OrderLifecycleTransitionCommand {
            order_id: command.order_id,
            market_id: command.market_id,
            mode: command.mode,
            target_state: OrderLifecycleState::Pending,
            reason_code: OrderLifecycleReasonCode::SubmissionAccepted
                .code()
                .to_string(),
            correlation_id: command.correlation_id,
            idempotency_key,
            transitioned_at_utc: command.requested_at_utc,
        };
        self.persist_with_telemetry("execution_order_lifecycle_submit_v1", transition_command)
            .await
    }

    pub async fn cancel_order(
        &self,
        command: CancelOrderCommand,
    ) -> Result<OrderLifecycleCommandOutcome, OrderLifecycleRuntimeError> {
        validate_runtime_command_fields(
            &command.order_id,
            &command.market_id,
            &command.correlation_id,
            &command.requested_at_utc,
        )?;
        let idempotency_key =
            normalize_order_cancel_idempotency_key(&command.order_id, &command.idempotency_key);
        let transition_command = OrderLifecycleTransitionCommand {
            order_id: command.order_id,
            market_id: command.market_id,
            mode: command.mode,
            target_state: OrderLifecycleState::Canceled,
            reason_code: OrderLifecycleReasonCode::CancelAccepted.code().to_string(),
            correlation_id: command.correlation_id,
            idempotency_key,
            transitioned_at_utc: command.requested_at_utc,
        };
        self.persist_with_telemetry("execution_order_lifecycle_cancel_v1", transition_command)
            .await
    }

    pub async fn batch_cancel(
        &self,
        command: BatchCancelCommand,
    ) -> Result<Vec<BatchCancelOrderOutcome>, OrderLifecycleRuntimeError> {
        validate_non_empty("market_id", &command.market_id)?;
        validate_non_empty("correlation_id", &command.correlation_id)?;
        validate_non_empty("requested_at_utc", &command.requested_at_utc)?;
        validate_timestamp_utc("requested_at_utc", &command.requested_at_utc)?;
        if command.orders.is_empty() {
            return Err(OrderLifecycleRuntimeError::new(
                OrderLifecycleReasonCode::InvalidPayload.code(),
                "batch-cancel requires at least one order",
            ));
        }

        let mut outcomes = Vec::with_capacity(command.orders.len());
        for request in &command.orders {
            validate_non_empty("order_id", &request.order_id)?;
            let idempotency_key = request
                .idempotency_key
                .as_deref()
                .map(normalize_order_idempotency_key)
                .unwrap_or_else(|| {
                    normalize_order_batch_cancel_idempotency_key(
                        &command.batch_idempotency_key,
                        &request.order_id,
                    )
                });
            let transition_command = OrderLifecycleTransitionCommand {
                order_id: request.order_id.clone(),
                market_id: command.market_id.clone(),
                mode: request.mode,
                target_state: OrderLifecycleState::Canceled,
                reason_code: OrderLifecycleReasonCode::BatchCancelAccepted
                    .code()
                    .to_string(),
                correlation_id: command.correlation_id.clone(),
                idempotency_key: idempotency_key.clone(),
                transitioned_at_utc: command.requested_at_utc.clone(),
            };

            match self
                .persist_with_telemetry(
                    "execution_order_lifecycle_batch_cancel_v1",
                    transition_command,
                )
                .await
            {
                Ok(outcome) => {
                    let kind = match outcome.disposition {
                        OrderLifecycleCommandDisposition::Applied
                        | OrderLifecycleCommandDisposition::Duplicate => {
                            BatchCancelOutcomeKind::Canceled
                        }
                        OrderLifecycleCommandDisposition::AlreadyTerminal => {
                            BatchCancelOutcomeKind::AlreadyTerminal
                        }
                        OrderLifecycleCommandDisposition::Rejected => {
                            BatchCancelOutcomeKind::HardFailure
                        }
                    };
                    let batch_outcome = BatchCancelOrderOutcome {
                        order_id: request.order_id.clone(),
                        market_id: command.market_id.clone(),
                        outcome: kind,
                        reason_code: outcome.reason_code,
                        idempotency_key,
                        correlation_id: command.correlation_id.clone(),
                        timestamp_utc: command.requested_at_utc.clone(),
                    };
                    emit_batch_cancel_outcome(&batch_outcome);
                    outcomes.push(batch_outcome);
                }
                Err(error) => {
                    let kind = if is_retryable_runtime_error(&error) {
                        BatchCancelOutcomeKind::RetryableFailure
                    } else {
                        BatchCancelOutcomeKind::HardFailure
                    };
                    let batch_outcome = BatchCancelOrderOutcome {
                        order_id: request.order_id.clone(),
                        market_id: command.market_id.clone(),
                        outcome: kind,
                        reason_code: error.code.to_string(),
                        idempotency_key,
                        correlation_id: command.correlation_id.clone(),
                        timestamp_utc: command.requested_at_utc.clone(),
                    };
                    emit_batch_cancel_outcome(&batch_outcome);
                    outcomes.push(batch_outcome);
                }
            }
        }
        Ok(outcomes)
    }

    pub async fn hydrate_order(
        &self,
        order_id: &str,
    ) -> Result<Option<OrderRecord>, OrderLifecycleRuntimeError> {
        self.store.load_order(order_id).await
    }

    pub async fn replay_order(
        &self,
        order_id: &str,
    ) -> Result<Vec<OrderStateTransition>, OrderLifecycleRuntimeError> {
        self.store.load_order_state_transitions(order_id).await
    }

    pub async fn apply_user_stream_event(
        &self,
        event: &UserStreamEvent,
    ) -> Result<OrderLifecycleCommandOutcome, OrderLifecycleRuntimeError> {
        validate_non_empty("event.order_id", &event.order_id)?;
        validate_non_empty("event.market_id", &event.market_id)?;
        validate_non_empty("event.correlation_id", &event.correlation_id)?;
        validate_non_empty("event.idempotency_key", &event.idempotency_key)?;
        validate_non_empty("event.observed_at_utc", &event.observed_at_utc)?;
        validate_timestamp_utc("event.observed_at_utc", &event.observed_at_utc)?;

        let target_state = map_user_stream_event_state(event.event_status)?;
        let mode = self
            .store
            .load_order(&event.order_id)
            .await?
            .map(|order| order.mode)
            .unwrap_or(OrderMode::Limit);
        let reason_code = match event.event_status {
            UserStreamEventStatus::Cancellation => OrderLifecycleReasonCode::CancelAccepted.code(),
            UserStreamEventStatus::Placement
            | UserStreamEventStatus::Update
            | UserStreamEventStatus::Matched
            | UserStreamEventStatus::Mined
            | UserStreamEventStatus::Confirmed => {
                OrderLifecycleReasonCode::VenueUpdateAccepted.code()
            }
        };
        let transition_command = OrderLifecycleTransitionCommand {
            order_id: event.order_id.clone(),
            market_id: event.market_id.clone(),
            mode,
            target_state,
            reason_code: reason_code.to_string(),
            correlation_id: event.correlation_id.clone(),
            idempotency_key: normalize_order_idempotency_key(&event.idempotency_key),
            transitioned_at_utc: event.observed_at_utc.clone(),
        };
        let outcome = self
            .persist_with_telemetry(
                "execution_order_lifecycle_user_stream_transition_v1",
                transition_command,
            )
            .await?;

        Ok(outcome)
    }

    async fn adjudicate_submit_order_request(
        &self,
        request: &PreTradeAdjudicationRequest,
    ) -> Result<PreTradeAdjudicationDecision, OrderLifecycleRuntimeError> {
        let adjudication_future = self.risk_adjudication_port.adjudicate_submit(request);
        let adjudication_result =
            tokio::time::timeout(self.pretrade_adjudication_timeout, adjudication_future).await;

        match adjudication_result {
            Err(_) => Err(OrderLifecycleRuntimeError::new(
                PreTradeReasonCode::AdjudicationTimeout.code(),
                format!(
                    "pre-trade adjudication timed out after {}ms for order `{}`",
                    self.pretrade_adjudication_timeout.as_millis(),
                    request.order_id
                ),
            )),
            Ok(Err(error)) => {
                let normalized_code = normalize_pretrade_reason_code(error.code);
                Err(OrderLifecycleRuntimeError::new(
                    normalized_code,
                    error.message,
                ))
            }
            Ok(Ok(decision)) => {
                let normalized_reason_code = normalize_pretrade_reason_code(&decision.reason_code);
                if decision.allowed && normalized_reason_code != PreTradeReasonCode::Pass.code() {
                    return Err(OrderLifecycleRuntimeError::new(
                        PreTradeReasonCode::InvalidPayload.code(),
                        format!(
                            "pre-trade adjudication allow decision must use `{}` reason_code",
                            PreTradeReasonCode::Pass.code()
                        ),
                    ));
                }
                if !decision.allowed && normalized_reason_code == PreTradeReasonCode::Pass.code() {
                    return Err(OrderLifecycleRuntimeError::new(
                        PreTradeReasonCode::InvalidPayload.code(),
                        "pre-trade adjudication deny decision cannot use pretrade_gate_pass reason_code",
                    ));
                }
                Ok(PreTradeAdjudicationDecision {
                    allowed: decision.allowed,
                    reason_code: normalized_reason_code.to_string(),
                    decided_at_utc: decision.decided_at_utc,
                })
            }
        }
    }

    async fn persist_with_telemetry(
        &self,
        event_name: &'static str,
        command: OrderLifecycleTransitionCommand,
    ) -> Result<OrderLifecycleCommandOutcome, OrderLifecycleRuntimeError> {
        let prior_state = self
            .store
            .load_order(&command.order_id)
            .await?
            .map(|order| order.state);
        let persist_outcome = self.store.persist_transition(&command).await?;
        let disposition = match persist_outcome.disposition {
            OrderLifecyclePersistDisposition::Applied => OrderLifecycleCommandDisposition::Applied,
            OrderLifecyclePersistDisposition::Duplicate => {
                OrderLifecycleCommandDisposition::Duplicate
            }
            OrderLifecyclePersistDisposition::Rejected => {
                OrderLifecycleCommandDisposition::Rejected
            }
            OrderLifecyclePersistDisposition::AlreadyTerminal => {
                OrderLifecycleCommandDisposition::AlreadyTerminal
            }
        };
        let event_id = persist_outcome
            .transition
            .as_ref()
            .map(|transition| transition.transition_id.clone())
            .unwrap_or_else(|| {
                fallback_event_id(event_name, &command.order_id, &command.transitioned_at_utc)
            });

        emit_order_lifecycle_telemetry(OrderLifecycleTelemetryEvent {
            event_name,
            event_id: &event_id,
            outcome: match disposition {
                OrderLifecycleCommandDisposition::Applied => "allow",
                OrderLifecycleCommandDisposition::Duplicate => "ignore",
                OrderLifecycleCommandDisposition::Rejected
                | OrderLifecycleCommandDisposition::AlreadyTerminal => "deny",
            },
            correlation_id: &command.correlation_id,
            order_id: &command.order_id,
            market_id: &command.market_id,
            from_state: prior_state.map(OrderLifecycleState::as_str),
            to_state: Some(persist_outcome.order.state.as_str()),
            reason_code: &persist_outcome.reason_code,
            timestamp_utc: &command.transitioned_at_utc,
        });

        Ok(OrderLifecycleCommandOutcome {
            disposition,
            reason_code: persist_outcome.reason_code,
            order_id: command.order_id,
            market_id: command.market_id,
            idempotency_key: command.idempotency_key,
            from_state: prior_state,
            to_state: Some(persist_outcome.order.state),
            correlation_id: command.correlation_id,
            timestamp_utc: command.transitioned_at_utc,
        })
    }
}

impl<S: OrderLifecycleStore> OrderLifecycleUpdatePort for OrderLifecycleRuntime<S> {
    fn apply_user_stream_event<'a>(
        &'a self,
        event: &'a UserStreamEvent,
    ) -> Pin<Box<dyn Future<Output = Result<(), OrderLifecycleRuntimeError>> + Send + 'a>> {
        Box::pin(async move {
            let _ = self.apply_user_stream_event(event).await?;
            Ok(())
        })
    }
}

fn map_user_stream_event_state(
    status: UserStreamEventStatus,
) -> Result<OrderLifecycleState, OrderLifecycleRuntimeError> {
    let mapped = match status {
        UserStreamEventStatus::Placement | UserStreamEventStatus::Update => {
            OrderLifecycleState::Live
        }
        UserStreamEventStatus::Matched => OrderLifecycleState::PartiallyFilled,
        UserStreamEventStatus::Mined | UserStreamEventStatus::Confirmed => {
            OrderLifecycleState::Filled
        }
        UserStreamEventStatus::Cancellation => OrderLifecycleState::Canceled,
    };
    Ok(mapped)
}

fn normalize_pretrade_reason_code(reason_code: &str) -> &'static str {
    PreTradeReasonCode::parse(reason_code)
        .map(|reason| reason.code())
        .unwrap_or(PreTradeReasonCode::InvalidPayload.code())
}

fn is_retryable_runtime_error(error: &OrderLifecycleRuntimeError) -> bool {
    matches!(
        error.code,
        code if code == OrderLifecycleReasonCode::PersistenceUnavailable.code()
            || code == OrderLifecycleReasonCode::VenueUnavailable.code()
    )
}

fn validate_runtime_command_fields(
    order_id: &str,
    market_id: &str,
    correlation_id: &str,
    requested_at_utc: &str,
) -> Result<(), OrderLifecycleRuntimeError> {
    validate_non_empty("order_id", order_id)?;
    validate_non_empty("market_id", market_id)?;
    validate_non_empty("correlation_id", correlation_id)?;
    validate_non_empty("requested_at_utc", requested_at_utc)?;
    validate_timestamp_utc("requested_at_utc", requested_at_utc)?;
    Ok(())
}

fn validate_non_empty(field: &'static str, value: &str) -> Result<(), OrderLifecycleRuntimeError> {
    if value.trim().is_empty() {
        return Err(OrderLifecycleRuntimeError::new(
            OrderLifecycleReasonCode::InvalidPayload.code(),
            format!("{field} cannot be blank"),
        ));
    }
    Ok(())
}

fn validate_timestamp_utc(
    field: &'static str,
    value: &str,
) -> Result<(), OrderLifecycleRuntimeError> {
    let parsed = OffsetDateTime::parse(value, &Rfc3339).map_err(|error| {
        OrderLifecycleRuntimeError::new(
            OrderLifecycleReasonCode::InvalidPayload.code(),
            format!("{field} must be an RFC3339 UTC timestamp: {error}"),
        )
    })?;
    if parsed.offset() != UtcOffset::UTC {
        return Err(OrderLifecycleRuntimeError::new(
            OrderLifecycleReasonCode::InvalidPayload.code(),
            format!("{field} must use UTC `Z` offset"),
        ));
    }
    Ok(())
}

fn fallback_event_id(event_name: &str, order_id: &str, timestamp_utc: &str) -> String {
    let compact_timestamp: String = timestamp_utc
        .chars()
        .filter(|character| character.is_ascii_digit())
        .collect();
    format!("{event_name}::{order_id}::{compact_timestamp}")
}

fn emit_pretrade_submit_deny_telemetry(
    request: &PreTradeAdjudicationRequest,
    reason_code: &str,
) {
    let deny_event_id = fallback_event_id(
        "execution_order_lifecycle_submit_pretrade_deny_v1",
        &request.order_id,
        &request.requested_at_utc,
    );
    emit_order_lifecycle_telemetry(OrderLifecycleTelemetryEvent {
        event_name: "execution_order_lifecycle_submit_pretrade_deny_v1",
        event_id: &deny_event_id,
        outcome: "deny",
        correlation_id: &request.correlation_id,
        order_id: &request.order_id,
        market_id: &request.market_id,
        from_state: None,
        to_state: None,
        reason_code,
        timestamp_utc: &request.requested_at_utc,
    });
}

fn emit_batch_cancel_outcome(outcome: &BatchCancelOrderOutcome) {
    emit_order_lifecycle_telemetry(OrderLifecycleTelemetryEvent {
        event_name: "execution_order_lifecycle_batch_cancel_outcome_v1",
        event_id: &format!(
            "batch-cancel::{}::{}",
            outcome.order_id,
            compact_timestamp_token(&outcome.timestamp_utc)
        ),
        outcome: outcome.outcome.as_str(),
        correlation_id: &outcome.correlation_id,
        order_id: &outcome.order_id,
        market_id: &outcome.market_id,
        from_state: None,
        to_state: None,
        reason_code: &outcome.reason_code,
        timestamp_utc: &outcome.timestamp_utc,
    });
}

fn emit_order_lifecycle_telemetry(event: OrderLifecycleTelemetryEvent<'_>) {
    println!(
        "{}",
        serde_json::to_string(&event).expect("order lifecycle telemetry should serialize")
    );
}

fn compact_timestamp_token(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_digit())
        .collect()
}

#[derive(Debug, Serialize)]
struct OrderLifecycleTelemetryEvent<'a> {
    event_name: &'a str,
    event_id: &'a str,
    outcome: &'a str,
    correlation_id: &'a str,
    order_id: &'a str,
    market_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    from_state: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    to_state: Option<&'a str>,
    reason_code: &'a str,
    timestamp_utc: &'a str,
}

#[cfg(test)]
#[derive(Debug, Clone, Default)]
pub struct InMemoryOrderLifecycleStore {
    state: std::sync::Arc<std::sync::Mutex<InMemoryOrderLifecycleState>>,
}

#[cfg(test)]
#[derive(Debug, Default)]
struct InMemoryOrderLifecycleState {
    orders: std::collections::BTreeMap<String, OrderRecord>,
    transitions: std::collections::BTreeMap<String, Vec<OrderStateTransition>>,
    idempotency_keys: std::collections::BTreeMap<String, std::collections::BTreeSet<String>>,
}

#[cfg(test)]
impl OrderLifecycleStore for InMemoryOrderLifecycleStore {
    fn persist_transition<'a>(
        &'a self,
        command: &'a OrderLifecycleTransitionCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<OrderLifecyclePersistOutcome, OrderLifecycleRuntimeError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let mut state = self.state.lock().map_err(|_| {
                OrderLifecycleRuntimeError::new(
                    OrderLifecycleReasonCode::PersistenceUnavailable.code(),
                    "in-memory order lifecycle state is poisoned",
                )
            })?;
            let normalized_idempotency_key =
                normalize_order_idempotency_key(&command.idempotency_key);
            let order_id = command.order_id.trim().to_string();

            let duplicate = state
                .idempotency_keys
                .get(&order_id)
                .is_some_and(|keys| keys.contains(&normalized_idempotency_key));
            if duplicate {
                let order = state.orders.get(&order_id).cloned().ok_or_else(|| {
                    OrderLifecycleRuntimeError::new(
                        OrderLifecycleReasonCode::PersistenceUnavailable.code(),
                        "duplicate idempotency key found for missing order",
                    )
                })?;
                return Ok(OrderLifecyclePersistOutcome {
                    disposition: OrderLifecyclePersistDisposition::Duplicate,
                    reason_code: OrderLifecycleReasonCode::DuplicateIdempotencyKey
                        .code()
                        .to_string(),
                    order,
                    transition: None,
                });
            }

            match state.orders.get(&order_id).cloned() {
                Some(order) if order.state.is_terminal() => Ok(OrderLifecyclePersistOutcome {
                    disposition: OrderLifecyclePersistDisposition::AlreadyTerminal,
                    reason_code: OrderLifecycleReasonCode::AlreadyTerminal.code().to_string(),
                    order,
                    transition: None,
                }),
                Some(current_order) => {
                    if validate_order_transition(Some(current_order.state), command.target_state)
                        .is_err()
                    {
                        return Ok(OrderLifecyclePersistOutcome {
                            disposition: OrderLifecyclePersistDisposition::Rejected,
                            reason_code: OrderLifecycleReasonCode::TransitionRejected
                                .code()
                                .to_string(),
                            order: current_order,
                            transition: None,
                        });
                    }

                    let next_sequence = current_order.last_transition_sequence + 1;
                    let mut cancel_key = current_order.cancel_idempotency_key.clone();
                    if command.target_state == OrderLifecycleState::Canceled {
                        cancel_key = Some(normalize_order_cancel_idempotency_key(
                            &command.order_id,
                            &normalized_idempotency_key,
                        ));
                    }
                    let next_order = OrderRecord {
                        order_id: order_id.clone(),
                        market_id: current_order.market_id.clone(),
                        mode: current_order.mode,
                        state: command.target_state,
                        submission_idempotency_key: current_order
                            .submission_idempotency_key
                            .clone(),
                        cancel_idempotency_key: cancel_key,
                        last_transition_sequence: next_sequence,
                        last_reason_code: command.reason_code.clone(),
                        correlation_id: command.correlation_id.clone(),
                        created_at_utc: current_order.created_at_utc.clone(),
                        updated_at_utc: command.transitioned_at_utc.clone(),
                    };
                    validate_order_record(&next_order).map_err(|error| {
                        OrderLifecycleRuntimeError::new(error.code, error.message)
                    })?;

                    let transition = OrderStateTransition {
                        transition_id: format!("transition::{order_id}::{next_sequence}"),
                        order_id: order_id.clone(),
                        market_id: command.market_id.clone(),
                        mode: command.mode,
                        from_state: Some(current_order.state),
                        to_state: command.target_state,
                        reason_code: command.reason_code.clone(),
                        idempotency_key: normalized_idempotency_key.clone(),
                        correlation_id: command.correlation_id.clone(),
                        transition_sequence: next_sequence,
                        transitioned_at_utc: command.transitioned_at_utc.clone(),
                    };
                    validate_order_state_transition(&transition).map_err(|error| {
                        OrderLifecycleRuntimeError::new(error.code, error.message)
                    })?;

                    state.orders.insert(order_id.clone(), next_order.clone());
                    state
                        .transitions
                        .entry(order_id.clone())
                        .or_default()
                        .push(transition.clone());
                    state
                        .idempotency_keys
                        .entry(order_id)
                        .or_default()
                        .insert(normalized_idempotency_key);
                    Ok(OrderLifecyclePersistOutcome {
                        disposition: OrderLifecyclePersistDisposition::Applied,
                        reason_code: command.reason_code.clone(),
                        order: next_order,
                        transition: Some(transition),
                    })
                }
                None => {
                    validate_order_transition(None, command.target_state).map_err(|error| {
                        OrderLifecycleRuntimeError::new(error.code, error.message)
                    })?;
                    let cancel_idempotency_key =
                        if command.target_state == OrderLifecycleState::Canceled {
                            Some(normalize_order_cancel_idempotency_key(
                                &command.order_id,
                                &normalized_idempotency_key,
                            ))
                        } else {
                            None
                        };
                    let order = OrderRecord {
                        order_id: order_id.clone(),
                        market_id: command.market_id.clone(),
                        mode: command.mode,
                        state: command.target_state,
                        submission_idempotency_key: normalize_order_submission_idempotency_key(
                            &command.order_id,
                            &normalized_idempotency_key,
                        ),
                        cancel_idempotency_key,
                        last_transition_sequence: 1,
                        last_reason_code: command.reason_code.clone(),
                        correlation_id: command.correlation_id.clone(),
                        created_at_utc: command.transitioned_at_utc.clone(),
                        updated_at_utc: command.transitioned_at_utc.clone(),
                    };
                    validate_order_record(&order).map_err(|error| {
                        OrderLifecycleRuntimeError::new(error.code, error.message)
                    })?;
                    let transition = OrderStateTransition {
                        transition_id: format!("transition::{order_id}::1"),
                        order_id: order_id.clone(),
                        market_id: command.market_id.clone(),
                        mode: command.mode,
                        from_state: None,
                        to_state: command.target_state,
                        reason_code: command.reason_code.clone(),
                        idempotency_key: normalized_idempotency_key.clone(),
                        correlation_id: command.correlation_id.clone(),
                        transition_sequence: 1,
                        transitioned_at_utc: command.transitioned_at_utc.clone(),
                    };
                    validate_order_state_transition(&transition).map_err(|error| {
                        OrderLifecycleRuntimeError::new(error.code, error.message)
                    })?;

                    state.orders.insert(order_id.clone(), order.clone());
                    state
                        .transitions
                        .entry(order_id.clone())
                        .or_default()
                        .push(transition.clone());
                    state
                        .idempotency_keys
                        .entry(order_id)
                        .or_default()
                        .insert(normalized_idempotency_key);
                    Ok(OrderLifecyclePersistOutcome {
                        disposition: OrderLifecyclePersistDisposition::Applied,
                        reason_code: command.reason_code.clone(),
                        order,
                        transition: Some(transition),
                    })
                }
            }
        })
    }

    fn load_order<'a>(
        &'a self,
        order_id: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Option<OrderRecord>, OrderLifecycleRuntimeError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let state = self.state.lock().map_err(|_| {
                OrderLifecycleRuntimeError::new(
                    OrderLifecycleReasonCode::PersistenceUnavailable.code(),
                    "in-memory order lifecycle state is poisoned",
                )
            })?;
            Ok(state.orders.get(order_id).cloned())
        })
    }

    fn load_order_state_transitions<'a>(
        &'a self,
        order_id: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<OrderStateTransition>, OrderLifecycleRuntimeError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let state = self.state.lock().map_err(|_| {
                OrderLifecycleRuntimeError::new(
                    OrderLifecycleReasonCode::PersistenceUnavailable.code(),
                    "in-memory order lifecycle state is poisoned",
                )
            })?;
            Ok(state.transitions.get(order_id).cloned().unwrap_or_default())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::risk::{UserStreamEventKind, UserStreamReasonCode};
    use tokio::time::sleep;

    fn sample_submit_command(order_id: &str) -> SubmitOrderCommand {
        SubmitOrderCommand {
            order_id: order_id.to_string(),
            market_id: "market-1".to_string(),
            mode: OrderMode::Limit,
            idempotency_key: format!("submit::{order_id}"),
            correlation_id: format!("corr-submit-{order_id}"),
            requested_at_utc: "2026-04-06T00:00:00Z".to_string(),
        }
    }

    fn sample_user_stream_event(
        order_id: &str,
        status: UserStreamEventStatus,
        offset: i64,
    ) -> UserStreamEvent {
        UserStreamEvent {
            event_id: format!("order::{order_id}::{offset}"),
            event_kind: UserStreamEventKind::Order,
            event_status: status,
            market_id: "market-1".to_string(),
            asset_id: "asset-1".to_string(),
            order_id: order_id.to_string(),
            trade_id: None,
            partition_key: order_id.to_string(),
            idempotency_key: format!("venue::{order_id}::{offset}"),
            event_offset: offset,
            event_timestamp_utc: "2026-04-06T00:00:00Z".to_string(),
            observed_at_utc: "2026-04-06T00:00:00Z".to_string(),
            ingested_at_utc: "2026-04-06T00:00:01Z".to_string(),
            ingestion_latency_seconds: 1.0,
            correlation_id: format!("corr-user-stream-{order_id}-{offset}"),
            reason_code: UserStreamReasonCode::EventAccepted.code().to_string(),
        }
    }

    #[derive(Debug, Default)]
    struct DenyRiskAdjudicationPort;

    impl RiskAdjudicationPort for DenyRiskAdjudicationPort {
        fn adjudicate_submit<'a>(
            &'a self,
            request: &'a PreTradeAdjudicationRequest,
        ) -> Pin<
            Box<
                dyn Future<
                        Output = Result<
                            PreTradeAdjudicationDecision,
                            PreTradeAdjudicationPortError,
                        >,
                    > + Send
                    + 'a,
            >,
        > {
            Box::pin(async move {
                Ok(PreTradeAdjudicationDecision {
                    allowed: false,
                    reason_code: PreTradeReasonCode::VenueIneligible.code().to_string(),
                    decided_at_utc: request.requested_at_utc.clone(),
                })
            })
        }
    }

    #[derive(Debug, Default)]
    struct UnavailableRiskAdjudicationPort;

    impl RiskAdjudicationPort for UnavailableRiskAdjudicationPort {
        fn adjudicate_submit<'a>(
            &'a self,
            _request: &'a PreTradeAdjudicationRequest,
        ) -> Pin<
            Box<
                dyn Future<
                        Output = Result<
                            PreTradeAdjudicationDecision,
                            PreTradeAdjudicationPortError,
                        >,
                    > + Send
                    + 'a,
            >,
        > {
            Box::pin(async move {
                Err(PreTradeAdjudicationPortError::new(
                    PreTradeReasonCode::AdjudicationUnavailable.code(),
                    "risk adjudication service unavailable",
                ))
            })
        }
    }

    #[derive(Debug, Default)]
    struct SlowRiskAdjudicationPort;

    impl RiskAdjudicationPort for SlowRiskAdjudicationPort {
        fn adjudicate_submit<'a>(
            &'a self,
            request: &'a PreTradeAdjudicationRequest,
        ) -> Pin<
            Box<
                dyn Future<
                        Output = Result<
                            PreTradeAdjudicationDecision,
                            PreTradeAdjudicationPortError,
                        >,
                    > + Send
                    + 'a,
            >,
        > {
            Box::pin(async move {
                sleep(Duration::from_millis(50)).await;
                Ok(PreTradeAdjudicationDecision {
                    allowed: true,
                    reason_code: PreTradeReasonCode::Pass.code().to_string(),
                    decided_at_utc: request.requested_at_utc.clone(),
                })
            })
        }
    }

    #[derive(Debug, Default)]
    struct InvalidDenyReasonRiskAdjudicationPort;

    impl RiskAdjudicationPort for InvalidDenyReasonRiskAdjudicationPort {
        fn adjudicate_submit<'a>(
            &'a self,
            request: &'a PreTradeAdjudicationRequest,
        ) -> Pin<
            Box<
                dyn Future<
                        Output = Result<
                            PreTradeAdjudicationDecision,
                            PreTradeAdjudicationPortError,
                        >,
                    > + Send
                    + 'a,
            >,
        > {
            Box::pin(async move {
                Ok(PreTradeAdjudicationDecision {
                    allowed: false,
                    reason_code: PreTradeReasonCode::Pass.code().to_string(),
                    decided_at_utc: request.requested_at_utc.clone(),
                })
            })
        }
    }

    #[tokio::test]
    async fn submit_order_with_allowed_pretrade_adjudication_preserves_lifecycle_behavior() {
        let store = InMemoryOrderLifecycleStore::default();
        let runtime = OrderLifecycleRuntime::with_risk_adjudication_port(
            store,
            Arc::new(AllowAllRiskAdjudicationPort),
            Duration::from_millis(100),
        );

        let outcome = runtime
            .submit_order(sample_submit_command("order-pretrade-allow"))
            .await
            .expect("allowed pre-trade adjudication should preserve submit transition");

        assert_eq!(
            outcome.disposition,
            OrderLifecycleCommandDisposition::Applied
        );
        assert_eq!(
            outcome.reason_code,
            OrderLifecycleReasonCode::SubmissionAccepted.code()
        );
    }

    #[tokio::test]
    async fn submit_order_denied_by_pretrade_adjudication_has_no_side_effects() {
        let store = InMemoryOrderLifecycleStore::default();
        let runtime = OrderLifecycleRuntime::with_risk_adjudication_port(
            store,
            Arc::new(DenyRiskAdjudicationPort),
            Duration::from_millis(100),
        );

        let error = runtime
            .submit_order(sample_submit_command("order-pretrade-deny"))
            .await
            .expect_err("denied adjudication must fail submit path closed");
        assert_eq!(error.code, PreTradeReasonCode::VenueIneligible.code());
        let hydrated = runtime
            .hydrate_order("order-pretrade-deny")
            .await
            .expect("hydrate should succeed");
        assert!(hydrated.is_none());
    }

    #[tokio::test]
    async fn submit_order_unavailable_pretrade_adjudication_has_no_side_effects() {
        let store = InMemoryOrderLifecycleStore::default();
        let runtime = OrderLifecycleRuntime::with_risk_adjudication_port(
            store,
            Arc::new(UnavailableRiskAdjudicationPort),
            Duration::from_millis(100),
        );

        let error = runtime
            .submit_order(sample_submit_command("order-pretrade-unavailable"))
            .await
            .expect_err("unavailable adjudication must fail submit path closed");
        assert_eq!(
            error.code,
            PreTradeReasonCode::AdjudicationUnavailable.code()
        );
        let hydrated = runtime
            .hydrate_order("order-pretrade-unavailable")
            .await
            .expect("hydrate should succeed");
        assert!(hydrated.is_none());
    }

    #[tokio::test]
    async fn submit_order_default_runtime_fails_closed_when_pretrade_port_is_unavailable() {
        let store = InMemoryOrderLifecycleStore::default();
        let runtime = OrderLifecycleRuntime::new(store);

        let error = runtime
            .submit_order(sample_submit_command("order-pretrade-default-unavailable"))
            .await
            .expect_err("default runtime must fail closed when no adjudication integration exists");
        assert_eq!(
            error.code,
            PreTradeReasonCode::AdjudicationUnavailable.code()
        );
        let hydrated = runtime
            .hydrate_order("order-pretrade-default-unavailable")
            .await
            .expect("hydrate should succeed");
        assert!(hydrated.is_none());
    }

    #[tokio::test]
    async fn submit_order_timeout_pretrade_adjudication_has_no_side_effects() {
        let store = InMemoryOrderLifecycleStore::default();
        let runtime = OrderLifecycleRuntime::with_risk_adjudication_port(
            store,
            Arc::new(SlowRiskAdjudicationPort),
            Duration::from_millis(10),
        );

        let error = runtime
            .submit_order(sample_submit_command("order-pretrade-timeout"))
            .await
            .expect_err("timed out adjudication must fail submit path closed");
        assert_eq!(error.code, PreTradeReasonCode::AdjudicationTimeout.code());
        let hydrated = runtime
            .hydrate_order("order-pretrade-timeout")
            .await
            .expect("hydrate should succeed");
        assert!(hydrated.is_none());
    }

    #[tokio::test]
    async fn submit_order_denied_with_pass_reason_is_rejected_as_invalid_payload() {
        let store = InMemoryOrderLifecycleStore::default();
        let runtime = OrderLifecycleRuntime::with_risk_adjudication_port(
            store,
            Arc::new(InvalidDenyReasonRiskAdjudicationPort),
            Duration::from_millis(100),
        );

        let error = runtime
            .submit_order(sample_submit_command("order-pretrade-invalid-deny-reason"))
            .await
            .expect_err("deny decision with pass reason code must be rejected");
        assert_eq!(error.code, PreTradeReasonCode::InvalidPayload.code());
        let hydrated = runtime
            .hydrate_order("order-pretrade-invalid-deny-reason")
            .await
            .expect("hydrate should succeed");
        assert!(hydrated.is_none());
    }

    #[test]
    fn pretrade_lookup_errors_normalize_to_fail_closed_adjudication_codes() {
        let unknown =
            map_pretrade_lookup_error_to_adjudication_error(PreTradeGatePersistenceError {
                code: "unexpected_pretrade_code",
                message: "unexpected failure".to_string(),
                field_errors: Vec::new(),
            });
        assert_eq!(
            unknown.code,
            PreTradeReasonCode::AdjudicationUnavailable.code()
        );

        let persistence =
            map_pretrade_lookup_error_to_adjudication_error(PreTradeGatePersistenceError {
                code: PreTradeReasonCode::PersistenceUnavailable.code(),
                message: "db unavailable".to_string(),
                field_errors: Vec::new(),
            });
        assert_eq!(
            persistence.code,
            PreTradeReasonCode::PersistenceUnavailable.code()
        );
    }

    #[tokio::test]
    async fn user_stream_progression_tracks_terminal_resolution() {
        let store = InMemoryOrderLifecycleStore::default();
        let runtime = OrderLifecycleRuntime::with_risk_adjudication_port(
            store,
            Arc::new(AllowAllRiskAdjudicationPort),
            Duration::from_millis(100),
        );
        runtime
            .submit_order(sample_submit_command("order-1"))
            .await
            .expect("submit should succeed");

        runtime
            .apply_user_stream_event(&sample_user_stream_event(
                "order-1",
                UserStreamEventStatus::Placement,
                1,
            ))
            .await
            .expect("placement event should map to live");
        runtime
            .apply_user_stream_event(&sample_user_stream_event(
                "order-1",
                UserStreamEventStatus::Matched,
                2,
            ))
            .await
            .expect("matched event should map to partially filled");
        runtime
            .apply_user_stream_event(&sample_user_stream_event(
                "order-1",
                UserStreamEventStatus::Confirmed,
                3,
            ))
            .await
            .expect("confirmed event should map to filled");

        let order = runtime
            .hydrate_order("order-1")
            .await
            .expect("load should succeed")
            .expect("order should exist");
        assert_eq!(order.state, OrderLifecycleState::Filled);
        let transitions = runtime
            .replay_order("order-1")
            .await
            .expect("replay should succeed");
        assert_eq!(transitions.len(), 4);
        assert_eq!(
            transitions
                .last()
                .expect("latest transition should exist")
                .to_state,
            OrderLifecycleState::Filled
        );
    }

    #[tokio::test]
    async fn terminal_to_non_terminal_regression_is_rejected_without_state_mutation() {
        let store = InMemoryOrderLifecycleStore::default();
        let runtime = OrderLifecycleRuntime::with_risk_adjudication_port(
            store,
            Arc::new(AllowAllRiskAdjudicationPort),
            Duration::from_millis(100),
        );
        runtime
            .submit_order(sample_submit_command("order-2"))
            .await
            .expect("submit should succeed");
        runtime
            .cancel_order(CancelOrderCommand {
                order_id: "order-2".to_string(),
                market_id: "market-1".to_string(),
                mode: OrderMode::Limit,
                idempotency_key: "cancel::order-2".to_string(),
                correlation_id: "corr-cancel-order-2".to_string(),
                requested_at_utc: "2026-04-06T00:00:05Z".to_string(),
            })
            .await
            .expect("cancel should succeed");

        let regression_outcome = runtime
            .apply_user_stream_event(&sample_user_stream_event(
                "order-2",
                UserStreamEventStatus::Placement,
                6,
            ))
            .await
            .expect("terminal regression should be captured as non-mutating outcome");
        assert_eq!(
            regression_outcome.disposition,
            OrderLifecycleCommandDisposition::AlreadyTerminal
        );

        let order = runtime
            .hydrate_order("order-2")
            .await
            .expect("load should succeed")
            .expect("order should exist");
        assert_eq!(order.state, OrderLifecycleState::Canceled);
        let transitions = runtime
            .replay_order("order-2")
            .await
            .expect("replay should succeed");
        assert_eq!(transitions.len(), 2);
    }

    #[tokio::test]
    async fn batch_cancel_surfaces_mixed_outcomes_with_retry_and_hard_failures() {
        #[derive(Clone)]
        struct FailingStore;

        impl OrderLifecycleStore for FailingStore {
            fn persist_transition<'a>(
                &'a self,
                command: &'a OrderLifecycleTransitionCommand,
            ) -> Pin<
                Box<
                    dyn Future<
                            Output = Result<
                                OrderLifecyclePersistOutcome,
                                OrderLifecycleRuntimeError,
                            >,
                        > + Send
                        + 'a,
                >,
            > {
                Box::pin(async move {
                    if command.order_id == "order-retry" {
                        return Err(OrderLifecycleRuntimeError::new(
                            OrderLifecycleReasonCode::PersistenceUnavailable.code(),
                            "simulated persistence outage",
                        ));
                    }
                    if command.order_id == "order-hard" {
                        return Err(OrderLifecycleRuntimeError::new(
                            OrderLifecycleReasonCode::InvalidPayload.code(),
                            "simulated hard validation failure",
                        ));
                    }
                    Ok(OrderLifecyclePersistOutcome {
                        disposition: OrderLifecyclePersistDisposition::AlreadyTerminal,
                        reason_code: OrderLifecycleReasonCode::AlreadyTerminal.code().to_string(),
                        order: OrderRecord {
                            order_id: command.order_id.clone(),
                            market_id: command.market_id.clone(),
                            mode: command.mode,
                            state: OrderLifecycleState::Canceled,
                            submission_idempotency_key: "submit".to_string(),
                            cancel_idempotency_key: Some(command.idempotency_key.clone()),
                            last_transition_sequence: 2,
                            last_reason_code: command.reason_code.clone(),
                            correlation_id: command.correlation_id.clone(),
                            created_at_utc: command.transitioned_at_utc.clone(),
                            updated_at_utc: command.transitioned_at_utc.clone(),
                        },
                        transition: None,
                    })
                })
            }

            fn load_order<'a>(
                &'a self,
                _order_id: &'a str,
            ) -> Pin<
                Box<
                    dyn Future<Output = Result<Option<OrderRecord>, OrderLifecycleRuntimeError>>
                        + Send
                        + 'a,
                >,
            > {
                Box::pin(async move { Ok(None) })
            }

            fn load_order_state_transitions<'a>(
                &'a self,
                _order_id: &'a str,
            ) -> Pin<
                Box<
                    dyn Future<
                            Output = Result<Vec<OrderStateTransition>, OrderLifecycleRuntimeError>,
                        > + Send
                        + 'a,
                >,
            > {
                Box::pin(async move { Ok(Vec::new()) })
            }
        }

        let runtime = OrderLifecycleRuntime::new(FailingStore);
        let outcomes = runtime
            .batch_cancel(BatchCancelCommand {
                market_id: "market-1".to_string(),
                correlation_id: "corr-batch-1".to_string(),
                requested_at_utc: "2026-04-06T00:00:10Z".to_string(),
                batch_idempotency_key: "batch-1".to_string(),
                orders: vec![
                    BatchCancelOrderRequest {
                        order_id: "order-already-terminal".to_string(),
                        mode: OrderMode::Limit,
                        idempotency_key: None,
                    },
                    BatchCancelOrderRequest {
                        order_id: "order-retry".to_string(),
                        mode: OrderMode::Limit,
                        idempotency_key: None,
                    },
                    BatchCancelOrderRequest {
                        order_id: "order-hard".to_string(),
                        mode: OrderMode::Limit,
                        idempotency_key: None,
                    },
                ],
            })
            .await
            .expect("batch cancel should return per-order outcomes");

        assert_eq!(outcomes.len(), 3);
        assert_eq!(outcomes[0].outcome, BatchCancelOutcomeKind::AlreadyTerminal);
        assert_eq!(
            outcomes[1].outcome,
            BatchCancelOutcomeKind::RetryableFailure
        );
        assert_eq!(outcomes[2].outcome, BatchCancelOutcomeKind::HardFailure);
        assert_eq!(
            outcomes[1].reason_code,
            OrderLifecycleReasonCode::PersistenceUnavailable.code()
        );
        assert_eq!(
            outcomes[2].reason_code,
            OrderLifecycleReasonCode::InvalidPayload.code()
        );
    }

    #[tokio::test]
    async fn batch_cancel_retry_preserves_idempotency_keys_and_deterministic_outcomes() {
        let store = InMemoryOrderLifecycleStore::default();
        let runtime = OrderLifecycleRuntime::with_risk_adjudication_port(
            store,
            Arc::new(AllowAllRiskAdjudicationPort),
            Duration::from_millis(100),
        );

        runtime
            .submit_order(sample_submit_command("order-retry-safe"))
            .await
            .expect("submit should succeed");
        runtime
            .submit_order(sample_submit_command("order-terminal"))
            .await
            .expect("submit should succeed");
        runtime
            .cancel_order(CancelOrderCommand {
                order_id: "order-terminal".to_string(),
                market_id: "market-1".to_string(),
                mode: OrderMode::Limit,
                idempotency_key: "cancel::order-terminal".to_string(),
                correlation_id: "corr-cancel-order-terminal".to_string(),
                requested_at_utc: "2026-04-06T00:00:05Z".to_string(),
            })
            .await
            .expect("order should transition to terminal canceled state");

        let batch_command = BatchCancelCommand {
            market_id: "market-1".to_string(),
            correlation_id: "corr-batch-retry".to_string(),
            requested_at_utc: "2026-04-06T00:00:10Z".to_string(),
            batch_idempotency_key: " Batch-Retry ".to_string(),
            orders: vec![
                BatchCancelOrderRequest {
                    order_id: "order-retry-safe".to_string(),
                    mode: OrderMode::Limit,
                    idempotency_key: None,
                },
                BatchCancelOrderRequest {
                    order_id: "order-terminal".to_string(),
                    mode: OrderMode::Limit,
                    idempotency_key: None,
                },
            ],
        };

        let first_attempt = runtime
            .batch_cancel(batch_command.clone())
            .await
            .expect("first batch-cancel should return per-order outcomes");
        let retry_attempt = runtime
            .batch_cancel(batch_command)
            .await
            .expect("retry batch-cancel should remain deterministic");

        assert_eq!(first_attempt.len(), 2);
        assert_eq!(retry_attempt.len(), 2);
        assert_eq!(first_attempt[0].outcome, BatchCancelOutcomeKind::Canceled);
        assert_eq!(
            first_attempt[1].outcome,
            BatchCancelOutcomeKind::AlreadyTerminal
        );
        assert_eq!(retry_attempt[0].outcome, BatchCancelOutcomeKind::Canceled);
        assert_eq!(
            retry_attempt[1].outcome,
            BatchCancelOutcomeKind::AlreadyTerminal
        );

        assert_eq!(
            retry_attempt[0].reason_code,
            OrderLifecycleReasonCode::DuplicateIdempotencyKey.code()
        );
        assert_eq!(
            retry_attempt[1].reason_code,
            OrderLifecycleReasonCode::AlreadyTerminal.code()
        );

        let retry_safe_key = "batch-cancel::batch-retry::order-retry-safe";
        let terminal_key = "batch-cancel::batch-retry::order-terminal";
        assert_eq!(first_attempt[0].idempotency_key, retry_safe_key);
        assert_eq!(retry_attempt[0].idempotency_key, retry_safe_key);
        assert_eq!(first_attempt[1].idempotency_key, terminal_key);
        assert_eq!(retry_attempt[1].idempotency_key, terminal_key);
    }
}
